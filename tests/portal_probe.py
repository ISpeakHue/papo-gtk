#!/usr/bin/env python3
"""Portal lifecycle on a private D-Bus, including Unix-FD transfer. No capture."""
import os
import warnings
warnings.filterwarnings("ignore", category=DeprecationWarning)
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src/voice"))
from screen_portal import ScreenPortal
from gi.repository import Gio, GLib

XML = '''<node>
<interface name="org.freedesktop.portal.ScreenCast">
<method name="CreateSession"><arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method>
<method name="SelectSources"><arg type="o" direction="in"/><arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method>
<method name="Start"><arg type="o" direction="in"/><arg type="s" direction="in"/><arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method>
<method name="OpenPipeWireRemote"><arg type="o" direction="in"/><arg type="a{sv}" direction="in"/><arg type="h" direction="out"/></method>
</interface>
<interface name="org.freedesktop.portal.Session"><method name="Close"/><signal name="Closed"><arg type="a{sv}"/></signal></interface>
<interface name="org.freedesktop.portal.Request"><method name="Close"/><signal name="Response"><arg type="u"/><arg type="a{sv}"/></signal></interface>
</node>'''

class PortalTests(unittest.TestCase):
    def setUp(self):
        self.bus = Gio.DBusConnection.new_for_address_sync(os.environ["DBUS_SESSION_BUS_ADDRESS"], Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
        self.bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "RequestName", GLib.Variant("(su)", (ScreenPortal.DEST, 0)), None, Gio.DBusCallFlags.NONE, 1000, None)
        self.info = Gio.DBusNodeInfo.new_for_xml(XML)
        self.objects = [self.bus.register_object(ScreenPortal.ROOT, self.info.interfaces[0], self.method, None, None)]
        self.code = 0
        self.hold = False
        self.calls, self.closes, self.ready, self.ended = [], [], [], []
        self.portal = ScreenPortal(lambda *args: self.ready.append(args), lambda: self.ended.append(True))
    def tearDown(self):
        self.portal.close()
        self.pump(lambda: not self.portal.signals)
        for obj in self.objects:
            self.bus.unregister_object(obj)
        self.bus.close_sync(None)
    def pump(self, predicate):
        import time
        deadline = time.monotonic() + 3
        context = GLib.MainContext.default()
        while not predicate() and time.monotonic() < deadline:
            while context.pending():
                context.iteration(False)
            time.sleep(0.005)
        self.assertTrue(predicate(), "portal callback timed out")
    def method(self, connection, sender, path, interface, method, params, invocation):
        self.calls.append(method)
        if method == "Close":
            self.closes.append(path)
            invocation.return_value(None)
            return
        args = params.unpack()
        if method == "OpenPipeWireRemote":
            read, write = os.pipe()
            fds = Gio.UnixFDList.new()
            index = fds.append(read)
            os.close(read)
            os.close(write)
            invocation.return_value_with_unix_fd_list(GLib.Variant("(h)", (index,)), fds)
            return
        options = args[-1]
        request = ScreenPortal.ROOT + "/request/" + sender[1:].replace(".", "_") + "/" + options["handle_token"]
        self.objects.append(connection.register_object(request, self.info.interfaces[2], self.method, None, None))
        invocation.return_value(GLib.Variant("(o)", (request,)))
        result = {}
        if method == "CreateSession":
            self.session = ScreenPortal.ROOT + "/session/" + sender[1:].replace(".", "_") + "/" + options["session_handle_token"]
            self.objects.append(connection.register_object(self.session, self.info.interfaces[1], self.method, None, None))
            result["session_handle"] = GLib.Variant("s", self.session)
        elif method == "SelectSources":
            self.assertEqual(options["types"], 3)
            self.assertFalse(options["multiple"])
            self.assertNotIn("restore_token", options)
        elif method == "Start":
            result["streams"] = GLib.Variant("a(ua{sv})", [(42, {"pipewire-serial": GLib.Variant("t", 99)})])
            if self.hold:
                return
        def respond():
            connection.emit_signal(sender, request, "org.freedesktop.portal.Request", "Response", GLib.Variant("(ua{sv})", (self.code if method == "Start" else 0, result)))
            return False
        GLib.idle_add(respond)
    def test_success_fd_and_close(self):
        self.portal.start()
        self.pump(lambda: bool(self.ready))
        fd, node, serial = self.ready[0]
        self.assertEqual((node, serial), (42, 99))
        os.fstat(fd)
        self.portal.close()
        self.pump(lambda: self.session in self.closes)
        with self.assertRaises(OSError):
            os.fstat(fd)
        self.assertFalse(self.ended)
    def test_cancel_and_deny_are_recoverable(self):
        for code in (1, 2):
            self.code = code
            if self.portal.closed:
                self.portal = ScreenPortal(lambda *args: self.ready.append(args), lambda: self.ended.append(True))
            self.portal.start()
            self.pump(lambda: self.portal.closed)
            self.assertFalse(self.ready)
        self.assertEqual(len(self.ended), 2)
        self.assertNotIn("OpenPipeWireRemote", self.calls)
    def test_cancel_pending_request(self):
        self.hold = True
        self.portal.start()
        self.pump(lambda: "Start" in self.calls)
        request = self.portal.request
        self.portal.close()
        self.pump(lambda: request in self.closes and self.session in self.closes)
        self.assertFalse(self.ready)
    def test_desktop_revocation_closes_fd(self):
        self.portal.start()
        self.pump(lambda: bool(self.ready))
        self.bus.emit_signal(None, self.session, "org.freedesktop.portal.Session", "Closed", GLib.Variant("(a{sv})", ({},)))
        self.pump(lambda: bool(self.ended))
        self.assertTrue(self.portal.closed)
        with self.assertRaises(OSError):
            os.fstat(self.ready[0][0])

unittest.main()
