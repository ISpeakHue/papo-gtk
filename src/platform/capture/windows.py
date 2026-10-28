# EN-US: Consent and COM stay in a native, worker-owned WGC bridge; Gst receives bounded BGRA copies.
# PT-BR: Consentimento e COM ficam na ponte WGC nativa do worker; Gst recebe cópias BGRA limitadas.
import ctypes
import os
from pathlib import Path
import sys
import time


class WindowsCaptureBridge:
    MAX_BYTES = 64 * 1024 * 1024

    def __init__(self):
        root = os.environ.get("PAPO_RUNTIME_DIR")
        path = Path(root) / "bin/papo-capture.dll" if root else Path(sys.executable).parent / "papo-capture.dll"
        if not path.is_absolute() or not path.is_file():
            raise RuntimeError("Packaged Windows capture bridge is missing")
        # EN-US: Only absolute app-owned DLLs and Windows system dependencies may be loaded.
        # PT-BR: Só DLLs absolutas do aplicativo e dependências do sistema Windows podem ser carregadas.
        self.dll = ctypes.WinDLL(str(path), winmode=0x900)
        for name, args, result in (
            ("abi", (), ctypes.c_int), ("supported", (), ctypes.c_int),
            ("start", (), ctypes.c_void_p), ("state", (ctypes.c_void_p,), ctypes.c_int),
            ("read", (ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint32,
                      ctypes.POINTER(ctypes.c_uint32), ctypes.POINTER(ctypes.c_uint32)), ctypes.c_int),
            ("close", (ctypes.c_void_p,), ctypes.c_int),
        ):
            function = getattr(self.dll, "papo_capture_" + name)
            function.argtypes, function.restype = args, result
        if self.dll.papo_capture_abi() != 1 or self.dll.papo_capture_supported() != 1:
            raise RuntimeError("Windows Graphics Capture is unavailable")
        self.buffer = None

    def start(self):
        handle = self.dll.papo_capture_start()
        if not handle:
            raise RuntimeError("Windows capture could not start")
        return handle

    def state(self, handle):
        return self.dll.papo_capture_state(handle)

    def read(self, handle):
        width, height = ctypes.c_uint32(), ctypes.c_uint32()
        capacity = len(self.buffer) if self.buffer is not None else 0
        length = self.dll.papo_capture_read(handle, self.buffer, capacity, ctypes.byref(width), ctypes.byref(height))
        if length == -2:
            needed = width.value * height.value * 4
            if not valid_frame_size(width.value, height.value) or needed > self.MAX_BYTES:
                raise ValueError("Capture frame exceeds bounds")
            self.buffer = ctypes.create_string_buffer(needed)
            length = self.dll.papo_capture_read(handle, self.buffer, needed, ctypes.byref(width), ctypes.byref(height))
            # EN-US: A resize between calls is retried on the next tick, without trusting old dimensions.
            # PT-BR: Mudança de tamanho entre chamadas é repetida no próximo ciclo, sem confiar nas dimensões antigas.
            if length == -2:
                return None
        if length == 0:
            return None
        if not valid_frame_size(width.value, height.value) or length != width.value * height.value * 4 or length > len(self.buffer):
            raise ValueError("Invalid capture frame")
        return width.value, height.value, ctypes.string_at(self.buffer, length)

    def close(self, handle):
        if self.dll.papo_capture_close(handle) != 1:
            # EN-US: A stuck driver must not retain capture after the call ends; Rust reaps the worker job.
            # PT-BR: Driver travado não pode manter captura após a chamada; Rust recolhe o Job do worker.
            os._exit(1)
        self.buffer = None


def valid_frame_size(width, height):
    return 0 < width <= 8192 and 0 < height <= 8192 and width * height * 4 <= 64 * 1024 * 1024


class WindowsCapture:
    def __init__(self, ready, ended, bridge=None, gst=None, glib=None):
        if gst is None or glib is None:
            from gi.repository import Gst, GLib
            gst, glib = Gst, GLib
        self.gst, self.glib = gst, glib
        self.bridge = bridge if bridge is not None else WindowsCaptureBridge()
        self.ready, self.ended = ready, ended
        self.handle = None
        self.timer = None
        self.source = None
        self.size = None
        self.closed = False
        self.delivered = False
        self.started_at = None
        self.last_frame = None

    def start(self):
        if self.closed or self.handle is not None:
            return
        source = self.gst.ElementFactory.make("appsrc")
        if source is None or not source.find_property("leaky-type"):
            raise RuntimeError("Bounded GStreamer appsrc is unavailable")
        for name, value in (("is-live", True), ("format", self.gst.Format.TIME), ("do-timestamp", True),
                            ("block", False), ("max-buffers", 1), ("max-bytes", 64 * 1024 * 1024),
                            ("leaky-type", 2), ("emit-signals", False)):
            source.set_property(name, value)
        self.source = source
        self.handle = self.bridge.start()
        self.timer = self.glib.timeout_add(66, self.poll)

    def poll(self):
        if self.closed:
            return False
        try:
            state = self.bridge.state(self.handle)
            if state not in (0, 1):
                return self.finish()
            if state == 0:
                return True
            if self.started_at is None:
                self.started_at = time.monotonic()
            frame = self.bridge.read(self.handle)
            if frame is None:
                if self.bridge.state(self.handle) != 1:
                    return self.finish()
                if not self.delivered and time.monotonic() - self.started_at > 15:
                    return self.finish()
                # EN-US: WGC may be idle on static content; keep VP8 keyframes available to new subscribers.
                # PT-BR: WGC pode parar em conteúdo estático; mantém quadros-chave VP8 para novos assinantes.
                frame = self.last_frame
                if frame is None:
                    return True
            width, height, data = frame
            if not valid_frame_size(width, height) or len(data) != width * height * 4:
                raise ValueError("Invalid capture frame")
            self.last_frame = frame
            if self.size != (width, height):
                caps = self.gst.Caps.from_string(
                    f"video/x-raw,format=BGRA,width={width},height={height},framerate=15/1,pixel-aspect-ratio=1/1")
                self.source.set_property("caps", caps)
                self.size = width, height
            if not self.delivered:
                self.delivered = True
                self.ready(self.source)
            if self.closed:
                return False
            buffer = self.gst.Buffer.new_allocate(None, len(data), None)
            buffer.fill(0, data)
            buffer.duration = self.gst.SECOND // 15
            if self.source.emit("push-buffer", buffer) != self.gst.FlowReturn.OK:
                return self.finish()
            return True
        except Exception:
            return self.finish()

    def finish(self):
        if not self.closed:
            self.close()
            self.ended()
        return False

    def close(self):
        if self.closed:
            return
        self.closed = True
        if self.timer is not None:
            self.glib.source_remove(self.timer)
            self.timer = None
        if self.handle is not None:
            self.bridge.close(self.handle)
            self.handle = None
        self.source = None
        self.last_frame = None


def create_screen_capture(ready, ended):
    # EN-US: Native desktop acceptance is deferred; preview requires explicit launch-time opt-in.
    # PT-BR: A aceitação no desktop nativo está pendente; a prévia exige opção explícita ao iniciar.
    if sys.platform != "win32" or os.environ.get("PAPO_WINDOWS_CAPTURE_PREVIEW") != "1":
        raise NotImplementedError("Windows capture preview is not enabled")
    return WindowsCapture(ready, ended)
