#!/usr/bin/env python3
"""EN-US: Read semantic controls only from the private synthetic AT-SPI fixture.
PT-BR: Lê controles semânticos somente da fixture privada e sintética de AT-SPI.
"""
import os
import sys
import time
import signal
import gi

if sys.platform != "linux" or os.environ.get("PAPO_ACCESSIBILITY_TEST") != "1":
    raise SystemExit("Private Linux accessibility fixture required")
gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, GLib

signal.signal(signal.SIGALRM, lambda *_: (_ for _ in ()).throw(SystemExit("Private AT-SPI probe timed out")))
signal.alarm(15)
Atspi.set_timeout(1000, 1000)
pid = int(sys.argv[1])
deadline = time.monotonic() + 12


def children(node):
    return [node.get_child_at_index(i) for i in range(node.get_child_count())]


def walk(node, depth=0):
    if node is None or depth > 40:
        return []
    result = [node]
    for child in children(node):
        result.extend(walk(child, depth + 1))
    return result


while time.monotonic() < deadline:
    context = GLib.MainContext.default()
    for _ in range(500):
        if not context.pending(): break
        context.iteration(False)
    apps = [a for a in children(Atspi.get_desktop(0)) if a and a.get_process_id() == pid]
    frames = [w for app in apps for w in children(app) if w and w.get_name() == "Papo accessibility fixture"]
    nodes = [n for app in apps for n in walk(app)]
    names = [n.get_name() for n in nodes]
    if "Papo accessibility fixture" in names and "Caixa de mensagem" in names and "Responder" in names:
        composer = next(n for n in nodes if n.get_name() == "Caixa de mensagem")
        assert composer.get_state_set().contains(Atspi.StateType.EDITABLE)
        assert composer.get_state_set().contains(Atspi.StateType.MULTI_LINE)
        assert any(name.startswith("Mensagem de ") for name in names)
        print("AT-SPI: named multiline editor, message rows and reply action exposed")
        break
    time.sleep(0.1)
else:
    raise SystemExit(f"Expected private fixture controls not exposed through AT-SPI; requested_pid={pid}; application_pids={[a.get_process_id() for a in children(Atspi.get_desktop(0)) if a]}; fixture_frames={len(frames)}; nodes={len(nodes)}; editor={'Caixa de mensagem' in names}; reply={'Responder' in names}")
