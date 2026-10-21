#!/usr/bin/env python3
"""EN-US: Fail early with loader/namespace evidence, without disabling image isolation.
PT-BR: Falha cedo com evidências do carregador/namespaces, sem desativar o isolamento.
"""
import os
from pathlib import Path
import subprocess


def main():
    if os.geteuid() == 0:
        raise SystemExit("Run decoder/GUI tests as the dedicated unprivileged CI user.")
    for command in (["id"], ["rpm", "-q", "gtk4", "libadwaita", "glycin-loaders", "librsvg2", "bubblewrap"],
                    ["bwrap", "--version"]):
        subprocess.run(command, check=True)
    print("uid_map:", Path("/proc/self/uid_map").read_text(), flush=True)
    # EN-US: Exercise namespace creation before GTK, preserving the decoder's own seccomp boundary.
    # PT-BR: Exercita a criação de namespaces antes do GTK, preservando o seccomp do decodificador.
    subprocess.run(["bwrap", "--unshare-all", "--die-with-parent", "--ro-bind", "/usr", "/usr",
                    "--symlink", "usr/lib64", "/lib64", "--symlink", "usr/lib", "/lib",
                    "--dev", "/dev", "--tmpfs", "/tmp", "/usr/bin/true"], check=True)
    import gi
    gi.require_version("GdkPixbuf", "2.0")
    from gi.repository import GdkPixbuf
    root = Path(__file__).resolve().parents[2]
    for path in (root / "assets/icons/symbolic/zoom-out-symbolic.svg",
                 root / "assets/icons/hicolor/256x256/apps/br.com.papo.gtk.png"):
        pixels = GdkPixbuf.Pixbuf.new_from_file(str(path))
        print(f"Decoded {path.name}: {pixels.get_width()}x{pixels.get_height()}", flush=True)


if __name__ == "__main__":
    main()
