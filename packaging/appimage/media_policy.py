# EN-US: Bundle media plugins and dependencies while leaving GPU drivers to the host.
# PT-BR: Inclui plugins de mídia e dependências no pacote, mantendo os drivers de GPU no sistema hospedeiro.
"""Hardware media plugins are bundled; GPU-specific drivers stay on the host."""

import ctypes
from pathlib import Path
import re


HARDWARE_PLUGINS = {"va": "libgstva.so", "nvcodec": "libgstnvcodec.so"}
HOST_DRIVER = re.compile(
    r"^(?:lib(?:cuda|nvcuvid)\.so(?:\..*)?|libnvidia[^/]*\.so(?:\..*)?|"
    r"libgallium[^/]*\.so(?:\..*)?|[^/]+_(?:drv_video|dri)\.so(?:\..*)?)$"
)


def check_hardware_plugins(root, gst):
    """Validate loadable plugin files even in builders without GPU devices."""
    directory = Path(root) / "usr/lib64"
    for path in directory.rglob("*.so*"):
        assert not HOST_DRIVER.fullmatch(path.name), f"GPU driver must come from host: {path}"
    result = {}
    factories = gst.Registry.get().get_feature_list(gst.ElementFactory)
    for plugin, filename in HARDWARE_PLUGINS.items():
        path = directory / "gstreamer-1.0" / filename
        assert path.is_file(), f"Missing hardware media plugin: {filename}"
        # Loading dependencies does not require CUDA or a VA-capable device.
        # Element registration is allowed to be empty when those are absent.
        ctypes.CDLL(str(path))
        decoders = []
        for factory in factories:
            if factory.get_plugin_name() != plugin or "Decoder/Video/Hardware" not in factory.get_klass():
                continue
            location = Path(factory.get_plugin().get_filename()).resolve()
            assert location == path.resolve(), f"Hardware decoder loaded outside AppDir: {location}"
            decoders.append(factory.get_name())
        result[plugin] = {"bundled": True, "registered_decoders": sorted(decoders)}
    return result
