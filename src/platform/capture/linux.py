# EN-US: Keep portal permission/selection and PipeWire details outside the shared call engine.
# PT-BR: Mantém permissão/seleção do portal e detalhes do PipeWire fora do motor compartilhado de chamadas.
if "ScreenPortal" not in globals():
    from screen_portal import ScreenPortal


def create_screen_capture(ready, ended):
    from gi.repository import Gst

    def source_ready(fd, node, serial):
        try:
            source = Gst.ElementFactory.make("pipewiresrc")
            source.set_property("fd", fd)
            if serial is not None and source.find_property("target-object"):
                source.set_property("target-object", str(serial))
            else:
                source.set_property("path", str(node))
            source.set_property("do-timestamp", True)
            ready(source)
        except Exception:
            ended()

    return ScreenPortal(source_ready, ended)
