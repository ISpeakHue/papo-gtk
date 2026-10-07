"""One non-persistent ScreenCast session, owned by the media worker's bus peer.

The portal alone chooses/authorizes the source. No X11/Wayland capture fallback.
https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html
"""
import os
import secrets
try:
    from gi.repository import Gio, GLib
except ImportError:
    # The embedded engine reports its localized native-runtime error.
    Gio = GLib = None


class ScreenPortal:
    DEST = "org.freedesktop.portal.Desktop"
    ROOT = "/org/freedesktop/portal/desktop"
    IFACE = "org.freedesktop.portal.ScreenCast"

    def __init__(self, ready, ended):
        self.ready, self.ended = ready, ended
        self.bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        self.session = None
        self.request = None
        self.signals = []
        self.fd = None
        self.closed = False
        self.timer = GLib.timeout_add_seconds(120, self.failed)

    def start(self):
        token = "papo" + secrets.token_hex(12)
        # Predict the session path so cancellation during CreateSession closes it.
        sender = self.bus.get_unique_name()[1:].replace(".", "_")
        self.session = self.ROOT + "/session/" + sender + "/" + token
        self.request_call("CreateSession", (), {"session_handle_token": GLib.Variant("s", token)}, self.created)

    def request_call(self, method, args, options, callback):
        token = "papo" + secrets.token_hex(12)
        sender = self.bus.get_unique_name()[1:].replace(".", "_")
        path = self.ROOT + "/request/" + sender + "/" + token
        self.request = path
        options["handle_token"] = GLib.Variant("s", token)
        signature = {"CreateSession": "(a{sv})", "SelectSources": "(oa{sv})", "Start": "(osa{sv})"}[method]
        def response(bus, sender, object_path, interface, signal, parameters):
            bus.signal_unsubscribe(subscription)
            self.signals.remove(subscription)
            self.request = None
            if self.closed:
                return
            code, results = parameters.unpack()
            if code != 0:
                self.failed()
            else:
                try:
                    callback(results)
                except Exception:
                    self.failed()
        subscription = self.bus.signal_subscribe(self.DEST, "org.freedesktop.portal.Request", "Response", path, None, Gio.DBusSignalFlags.NONE, response)
        self.signals.append(subscription)
        def called(bus, result):
            try:
                bus.call_finish(result)
            except GLib.Error:
                self.failed()
        self.bus.call(self.DEST, self.ROOT, self.IFACE, method, GLib.Variant(signature, (*args, options)), GLib.VariantType.new("(o)"), Gio.DBusCallFlags.NONE, 10000, None, called)

    def created(self, result):
        self.session = result["session_handle"]
        self.signals.append(self.bus.signal_subscribe(self.DEST, "org.freedesktop.portal.Session", "Closed", self.session, None, Gio.DBusSignalFlags.NONE, lambda *args: self.failed()))
        self.request_call("SelectSources", (self.session,), {"types": GLib.Variant("u", 3), "multiple": GLib.Variant("b", False)}, self.selected)

    def selected(self, _):
        self.request_call("Start", (self.session, ""), {}, self.started)

    def started(self, result):
        streams = result.get("streams", [])
        if len(streams) != 1:
            self.failed()
            return
        node, properties = streams[0]
        def opened(bus, response):
            try:
                value, fds = bus.call_with_unix_fd_list_finish(response)
                fd = fds.get(value.unpack()[0])
                if self.closed:
                    os.close(fd)
                    return
                self.fd = fd
                if self.timer:
                    GLib.source_remove(self.timer)
                    self.timer = None
                self.ready(fd, node, properties.get("pipewire-serial"))
            except Exception:
                self.failed()
        self.bus.call_with_unix_fd_list(self.DEST, self.ROOT, self.IFACE, "OpenPipeWireRemote", GLib.Variant("(oa{sv})", (self.session, {})), GLib.VariantType.new("(h)"), Gio.DBusCallFlags.NONE, 10000, None, None, opened)

    def failed(self):
        if not self.closed:
            self.close()
            self.ended()
        return False

    def close(self):
        if self.closed:
            return
        self.closed = True
        if self.timer:
            GLib.source_remove(self.timer)
            self.timer = None
        for subscription in self.signals:
            self.bus.signal_unsubscribe(subscription)
        self.signals.clear()
        for path, interface in ((self.request, "Request"), (self.session, "Session")):
            if path:
                self.bus.call(self.DEST, path, "org.freedesktop.portal." + interface, "Close", None, None, Gio.DBusCallFlags.NONE, 3000, None, None)
        if self.fd is not None:
            os.close(self.fd)
            self.fd = None
