"""Native GStreamer audio/video worker. stdin/stdout are private JSON-line IPC.

No sockets for application signaling, credential arguments, recording, or webview.
Only the GTK process talks to the authenticated owning WebSocket.
"""
import json
import base64
import time
import faulthandler
faulthandler.enable()
import os
import ipaddress
import struct
import secrets
import sys
import threading
import traceback
from urllib.parse import quote

_output_lock = threading.Lock()

def emit(event):
    with _output_lock:
        print(json.dumps(event, separators=(",", ":")), flush=True)

try:
    import gi
    gi.require_version("Gst", "1.0")
    gi.require_version("GstWebRTC", "1.0")
    gi.require_version("GstSdp", "1.0")
    gi.require_version("GstRtp", "1.0")
    from gi.repository import Gst, GstWebRTC, GstSdp, GstRtp, GLib
    Gst.init(None)
except (ImportError, ValueError):
    emit({"type": "error", "message": "Instale Python GI e GStreamer WebRTC, SDP, Opus e libnice."})
    sys.exit(1)


if "ScreenPortal" not in globals():
    from screen_portal import ScreenPortal


def devices(kind="Audio/Source"):
    monitor = Gst.DeviceMonitor.new()
    monitor.add_filter(kind, None)
    monitor.start()
    result = []
    for device in monitor.get_devices():
        properties = device.get_properties()
        identifier = None
        for key in ("device.path", "api.v4l2.path", "device.name", "device", "node.name"):
            if properties and properties.has_field(key):
                identifier = str(properties.get_value(key))
                break
        result.append((identifier or device.get_display_name(), device.get_display_name(), device))
    monitor.stop()
    return result

if "--devices" in sys.argv or "--cameras" in sys.argv:
    try:
        emit({"type": "devices", "devices": [{"id": i, "name": n} for i, n, _ in devices("Video/Source" if "--cameras" in sys.argv else "Audio/Source")]})
    except Exception:
        emit({"type": "error", "message": "Não foi possível listar microfones."})
    sys.exit(0)


class Engine:
    def __init__(self):
        self.loop = GLib.MainLoop()
        self.pipeline = None
        self.remote_set = False
        self.pending_ice = []
        self.offering = False
        self.local_sent = False
        self.local_candidates = []
        self.routes = set()
        self.outputs = []
        self.track_ids = {}
        self.closed = False
        self.test_sink = False
        self.video = {}
        self.media_pending = None
        self.media_recovery = None
        self.retired_bins = []
        self.portal = None
        self.video_enabled = False
        self.video_epoch = 0
        self.frame_pending = False
        self.video_receivers = []
        self.last_frame = 0


    def fail(self, message):
        if not self.closed:
            emit({"type": "error", "message": message})
        self.close()

    def close(self):
        if self.closed:
            return
        self.closed = True
        if self.pipeline:
            self.pipeline.set_state(Gst.State.NULL)
        if self.portal:
            self.portal.close()
            self.portal = None
        self.loop.quit()

    def start(self, config):
        if self.pipeline:
            raise ValueError("already started")
        required = ("webrtcbin", "nicesrc", "nicesink", "opusenc", "opusdec", "rtpopuspay", "rtpopusdepay", "level", "volume")
        if any(not Gst.ElementFactory.find(name) for name in required):
            self.fail("Faltam plugins GStreamer: WebRTC, libnice, Opus ou áudio.")
            return
        self.test_sink = bool(config.get("test_sink"))
        self.pipeline = Gst.Pipeline.new("papo-audio")
        self.webrtc = Gst.ElementFactory.make("webrtcbin", "connection")
        self.webrtc.set_property("bundle-policy", GstWebRTC.WebRTCBundlePolicy.MAX_BUNDLE)
        if config.get("relay"):
            self.webrtc.set_property("ice-transport-policy", GstWebRTC.WebRTCICETransportPolicy.RELAY)
        for server in config.get("ice_servers", []):
            for url in server.get("urls", []):
                scheme, address = url.split(":", 1)
                address = address.lstrip("/")
                if scheme == "stun":
                    self.webrtc.set_property("stun-server", "stun://" + address)
                elif scheme in ("turn", "turns"):
                    username = quote(server.get("username", ""), safe="")
                    credential = quote(server.get("credential", ""), safe="")
                    uri = scheme + "://" + username + ":" + credential + "@" + address
                    if not self.webrtc.emit("add-turn-server", uri):
                        raise ValueError("TURN configuration rejected")
                else:
                    raise ValueError("unsupported ICE scheme")
        self.pipeline.add(self.webrtc)
        if config.get("test_source"):
            source = Gst.ElementFactory.make("audiotestsrc", "capture")
            source.set_property("is-live", True)
            source.set_property("freq", config.get("frequency", 440))
            source.set_property("volume", 0.25)
        elif config.get("device"):
            selected = next((d for i, _, d in devices() if i == config["device"]), None)
            if selected is None:
                self.fail("O microfone escolhido não está disponível.")
                return
            source = selected.create_element("capture")
        else:
            source = Gst.ElementFactory.make("autoaudiosrc", "capture")
        if not source:
            self.fail("Não foi possível abrir o microfone.")
            return
        capture = Gst.parse_bin_from_description(
            "audioconvert ! audioresample ! audio/x-raw,rate=48000,channels=2 ! "
            "volume name=microphone ! level name=local_level audio-level-meta=true post-messages=true interval=100000000 ! opusenc bitrate=32000 ! "
            'rtpopuspay name=pay pt=96 ! capsfilter name=rtpcaps caps="application/x-rtp,media=audio,encoding-name=OPUS,'
            'clock-rate=48000,encoding-params=(string)2,payload=(int)96"', True)
        # Advertise a stable sending SSRC before offer creation; Pion needs
        # it to bind inbound RTP when the MID extension is absent.
        ssrc = secrets.randbits(32) or 1
        capture.get_by_name("pay").set_property("ssrc", ssrc)
        caps = capture.get_by_name("rtpcaps").get_property("caps").copy()
        caps = Gst.Caps.from_string(caps.to_string() + ",ssrc=(uint)" + str(ssrc))
        capture.get_by_name("rtpcaps").set_property("caps", caps)
        self.pipeline.add(source)
        self.pipeline.add(capture)
        if not source.link(capture):
            raise ValueError("capture could not link")
        self.microphone = capture.get_by_name("microphone")
        self.microphone.set_property("mute", bool(config.get("muted", False)))
        extension = Gst.ElementFactory.make("rtphdrextclientaudiolevel")
        if extension:
            extension.set_id(1)
            capture.get_by_name("pay").emit("add-extension", extension)
        if self.test_sink:
            counter = [0]
            def captured(pad, info):
                counter[0] += 1
                if counter[0] % 50 == 0:
                    emit({"type": "captured", "buffers": counter[0]})
                return Gst.PadProbeReturn.OK
            capture.get_by_name("pay").get_static_pad("src").add_probe(Gst.PadProbeType.BUFFER, captured)
        pad = self.webrtc.request_pad_simple("sink_%u")
        if capture.get_static_pad("src").link(pad) != Gst.PadLinkReturn.OK:
            raise ValueError("RTP could not link")
        pad.get_property("transceiver").set_property("direction", GstWebRTC.WebRTCRTPTransceiverDirection.SENDONLY)
        # Backend preallocates slots. Reserve receive m-lines without publishing
        # extra microphone tracks; the backend rejects multiple audio publishers.
        for _ in range(config.get("audio_slots", 8)):
            caps = Gst.Caps.from_string("application/x-rtp,media=audio,encoding-name=OPUS,clock-rate=48000,encoding-params=(string)2,payload=(int)96")
            self.webrtc.emit("add-transceiver", GstWebRTC.WebRTCRTPTransceiverDirection.RECVONLY, caps)
        for _ in range(config.get("video_slots", 6)):
            self.webrtc.emit("add-transceiver", GstWebRTC.WebRTCRTPTransceiverDirection.RECVONLY, self.video_caps())
        # Let initial source caps (including RFC6464) settle before the first
        # offer. Later offers are explicit, ordered after media intent.
        self.webrtc.connect("on-negotiation-needed", lambda *_: self.offer() if not self.remote_set else None)
        self.webrtc.connect("on-ice-candidate", self.local_candidate)
        self.webrtc.connect("pad-added", self.receive)
        self.webrtc.connect("notify::connection-state", self.connection)
        bus = self.pipeline.get_bus()
        bus.add_signal_watch()
        bus.connect("message", self.bus_message)
        emit({"type": "started", "pid": os.getpid()})
        if self.pipeline.set_state(Gst.State.PLAYING) == Gst.StateChangeReturn.FAILURE:
            self.fail("Não foi possível iniciar áudio. Verifique o microfone e a saída.")

    def offer(self, *_):
        if self.offering or self.closed:
            return
        self.offering = True
        promise = Gst.Promise.new_with_change_func(self.offered, None, None)
        self.webrtc.emit("create-offer", None, promise)

    def offered(self, promise, *_):
        try:
            reply = promise.get_reply()
            description = reply.get_value("offer")
            if description is None:
                raise ValueError("missing offer")
            # Gst consumes the description; serialize while its promise owns it.
            text = description.sdp.as_text()
            def installed(p, *_):
                p.wait()
                emit({"type": "voice_offer", "sdp": text})
                self.local_sent = True
                for candidate in self.local_candidates:
                    emit(candidate)
                self.local_candidates.clear()
            self.webrtc.emit("set-local-description", description, Gst.Promise.new_with_change_func(installed, None, None))
        except Exception:
            GLib.idle_add(self.fail, "Não foi possível negociar a oferta de áudio.")

    def local_candidate(self, _, index, candidate):
        # Keep offer-before-ICE ordering even when libnice gathers immediately.
        fields = candidate.removeprefix("candidate:").split()
        if len(fields) < 7:
            return
        try:
            address = ipaddress.ip_address(fields[4])
            if address.is_loopback or address.is_unspecified:
                return
        except ValueError:
            if not fields[4].endswith(".local"):
                return
        event = {"type": "voice_ice_candidate", "sdp_mline_index": index, "candidate": candidate}
        if self.local_sent:
            emit(event)
        else:
            self.local_candidates.append(event)

    def description(self, event):
        kind = "offer" if event["type"] == "voice_offer" else "answer"
        result, sdp = GstSdp.SDPMessage.new()
        if result != GstSdp.SDPResult.OK or GstSdp.sdp_message_parse_buffer(event["sdp"].encode(), sdp) != GstSdp.SDPResult.OK:
            raise ValueError("invalid SDP")
        self.track_ids = {}
        for i in range(sdp.medias_len()):
            media = sdp.get_media(i)
            mid = media.get_attribute_val("mid")
            msid = media.get_attribute_val("msid")
            if msid and len(msid.split()) == 2:
                self.track_ids[mid] = msid.split()[1]
        description = GstWebRTC.WebRTCSessionDescription.new(getattr(GstWebRTC.WebRTCSDPType, kind.upper()), sdp)
        def installed(p, *_):
            p.wait()
            GLib.idle_add(self.remote_ready, kind)
        self.webrtc.emit("set-remote-description", description, Gst.Promise.new_with_change_func(installed, None, None))

    def remote_ready(self, kind):
        self.remote_set = True
        for event in self.pending_ice:
            self.candidate(event)
        self.pending_ice.clear()
        if kind == "answer":
            self.offering = False
            if self.media_recovery:
                kind_failed = self.media_recovery
                self.media_recovery = None
                self.media_pending = (kind_failed, False)
                self.intent(kind_failed, False)
                self.offer()
                return False
            if self.media_pending:
                kind_done, on = self.media_pending
                if on and self.video.get(kind_done, {}).get("bin"):
                    self.video[kind_done]["bin"].get_by_name("gate").set_property("drop", False)
                self.media_pending = None
                emit({"type": "media_state", "kind": kind_done, "on": on})
            emit({"type": "negotiated"})
        if kind == "offer":
            def answered(p, *_):
                description = p.get_reply().get_value("answer")
                text = description.sdp.as_text()
                self.webrtc.emit("set-local-description", description, Gst.Promise.new())
                emit({"type": "voice_answer", "sdp": text})
            self.webrtc.emit("create-answer", None, Gst.Promise.new_with_change_func(answered, None, None))
        return False

    def candidate(self, event):
        if not self.remote_set:
            self.pending_ice.append(event)
        else:
            self.webrtc.emit("add-ice-candidate", int(event.get("sdp_mline_index") or 0), event["candidate"])

    def receive(self, _, pad):
        if pad.get_direction() != Gst.PadDirection.SRC:
            return
        try:
            caps = pad.get_current_caps() or pad.query_caps(None)
            if caps.get_structure(0).get_string("media") == "video":
                self.receive_video(pad)
                return
            if caps.get_structure(0).get_string("media") != "audio":
                return
            transceiver = pad.get_property("transceiver")
            mid = transceiver.get_property("mid")
            track = self.track_ids.get(mid, "")
            suffix = "appsink name=playback emit-signals=true sync=false" if self.test_sink else "autoaudiosink"
            playback = Gst.parse_bin_from_description("queue ! rtpopusdepay ! opusdec ! audioconvert ! audioresample ! audio/x-raw,format=F32LE ! volume name=output ! " + suffix, True)
            volume = playback.get_by_name("output")
            volume.set_property("mute", track not in self.routes)
            self.outputs.append((track, volume))
            self.pipeline.add(playback)
            if pad.link(playback.get_static_pad("sink")) != Gst.PadLinkReturn.OK:
                raise ValueError("receive pad not linked")
            if self.test_sink:
                counter = [0]
                def sample(sink):
                    sample = sink.emit("pull-sample")
                    buffer = sample.get_buffer()
                    ok, data = buffer.map(Gst.MapFlags.READ)
                    if ok:
                        values = struct.unpack("<" + "f" * (len(data.data) // 4), data.data)
                        energy = sum(v*v for v in values) / max(len(values), 1)
                        buffer.unmap(data)
                        counter[0] += 1
                        if counter[0] % 25 == 0:
                            emit({"type": "audio", "track_id": track, "buffers": counter[0], "energy": energy})
                    return Gst.FlowReturn.OK
                playback.get_by_name("playback").connect("new-sample", sample)
            playback.sync_state_with_parent()
            emit({"type": "track", "track_id": track})
        except Exception:
            GLib.idle_add(self.fail, "Não foi possível reproduzir o áudio recebido.")

    @staticmethod
    def video_caps():
        return Gst.Caps.from_string("application/x-rtp,media=video,encoding-name=VP8,clock-rate=90000,payload=(int)97")

    def media_failed(self, kind, message):
        # Stop only this capture; the microphone and remote audio remain live.
        self.stop_capture(kind)
        if not self.offering:
            self.media_pending = (kind, False)
            self.intent(kind, False)
            self.offer()
        else:
            # remote_ready serializes the cleanup offer before making the UI
            # available again; never let a late failed-start answer reactivate it.
            self.media_recovery = kind
        emit({"type": "media_error", "kind": kind, "message": message})

    def intent(self, kind, on):
        emit({"type": "media_intent", "kind": kind, "on": on})

    def stop_capture(self, kind):
        item = self.video.get(kind)
        if item:
            # Keep the negotiated sender reusable; intent=false in the next
            # offer removes the active backend role, while NULL releases capture.
            capture = item.pop("bin", None)
            if capture:
                self.retired_bins = (self.retired_bins + [capture])[-4:]
                capture.set_state(Gst.State.NULL)
                src = capture.get_static_pad("src")
                if src and src.is_linked():
                    src.unlink(item["pad"])
                self.pipeline.remove(capture)
        if kind == "screen" and self.portal:
            self.portal.close()
            self.portal = None

    def media(self, event):
        kind, on = event["kind"], bool(event["on"])
        if kind not in ("video", "screen"):
            raise ValueError("invalid media kind")
        if self.offering or self.media_pending:
            emit({"type": "media_error", "kind": kind, "message": "Aguarde a negociação de mídia terminar."})
            return
        if not on:
            self.stop_capture(kind)
            self.media_pending = (kind, False)
            self.intent(kind, False)
            self.offer()
            return
        self.media_pending = (kind, True)
        try:
            if event.get("test_source"):
                source = Gst.ElementFactory.make("videotestsrc")
                source.set_property("is-live", True)
                source.set_property("pattern", event.get("pattern", 0))
                self.publish_video(kind, source)
            elif kind == "video":
                selected = next((d for i, _, d in devices("Video/Source") if i == event.get("device")), None)
                if selected is None:
                    raise ValueError("missing camera")
                self.publish_video(kind, selected.create_element(None))
            else:
                def ready(fd, node, serial):
                    try:
                        source = Gst.ElementFactory.make("pipewiresrc")
                        source.set_property("fd", fd)
                        if serial is not None and source.find_property("target-object"):
                            source.set_property("target-object", str(serial))
                        else:
                            source.set_property("path", str(node))
                        source.set_property("do-timestamp", True)
                        self.publish_video(kind, source)
                    except Exception:
                        self.media_failed(kind, "Não foi possível iniciar a captura PipeWire.")
                self.portal = ScreenPortal(ready, lambda: self.media_failed(kind, "Compartilhamento encerrado, cancelado ou negado pelo desktop."))
                self.portal.start()
        except Exception:
            self.media_failed(kind, "Câmera indisponível ou plugins de vídeo/portal ausentes.")

    def publish_video(self, kind, source):
        if self.closed:
            return
        # Keep each publishing MID. A restarted encoder needs a fresh SSRC so
        # its new RTP sequence space cannot collide with SRTP replay protection.
        item = self.video.get(kind)
        if item is None:
            item = {"pad": self.webrtc.request_pad_simple("sink_%u"), "ssrc": secrets.randbits(32) or 1}
            self.video[kind] = item
        item["ssrc"] = secrets.randbits(32) or 1
        capture = Gst.Bin.new(None)
        item["bin"] = capture
        self.pipeline.add(capture)
        codec = Gst.parse_bin_from_description(
            "queue max-size-buffers=2 leaky=downstream ! videoconvert ! videoscale ! videorate ! "
            "video/x-raw,format=I420,width=640,height=360,framerate=15/1 ! "
            "vp8enc deadline=1 cpu-used=8 target-bitrate=700000 keyframe-max-dist=15 ! "
            "rtpvp8pay name=pay pt=97 picture-id-mode=15-bit ! capsfilter name=rtpcaps ! valve name=gate drop=true drop-mode=forward-sticky-events", True)
        codec.get_by_name("pay").set_property("ssrc", item["ssrc"])
        codec.get_by_name("rtpcaps").set_property("caps", Gst.Caps.from_string(self.video_caps().to_string() + ",ssrc=(uint)" + str(item["ssrc"])))
        capture.add(source)
        capture.add(codec)
        if not source.link(codec):
            raise ValueError("video source could not link")
        capture.add_pad(Gst.GhostPad.new("src", codec.get_static_pad("src")))
        if capture.get_static_pad("src").link(item["pad"]) != Gst.PadLinkReturn.OK:
            raise ValueError("video RTP could not link")
        item["pad"].get_property("transceiver").set_property("direction", GstWebRTC.WebRTCRTPTransceiverDirection.SENDONLY)
        item["pad"].send_event(Gst.Event.new_stream_start("papo-" + kind))
        item["pad"].send_event(Gst.Event.new_caps(codec.get_by_name("rtpcaps").get_property("caps")))
        self.intent(kind, True)
        if not capture.sync_state_with_parent():
            raise ValueError("video source could not start")
        self.offer()

    def receive_video(self, pad):
        track = self.track_ids.get(pad.get_property("transceiver").get_property("mid"), "")
        playback = Gst.parse_bin_from_description(
            "queue max-size-buffers=2 leaky=downstream ! rtpvp8depay request-keyframe=true wait-for-keyframe=true ! "
            "vp8dec ! videoconvert ! videoscale ! videorate ! "
            "video/x-raw,width=640,height=360,framerate=10/1 ! jpegenc quality=65 ! "
            "appsink name=frames emit-signals=true sync=false max-buffers=1 drop=true", True)
        def frame(sink):
            sample = sink.emit("pull-sample")
            epoch = self.video_epoch
            if not self.video_enabled or self.frame_pending or time.monotonic() - self.last_frame < 0.09:
                return Gst.FlowReturn.OK
            buffer = sample.get_buffer()
            if buffer.get_size() > 90000:
                return Gst.FlowReturn.OK
            data = buffer.extract_dup(0, buffer.get_size())
            self.frame_pending = True
            self.last_frame = time.monotonic()
            emit({"type": "video_frame", "track_id": track, "epoch": epoch, "jpeg": base64.b64encode(data).decode("ascii")})
            return Gst.FlowReturn.OK
        playback.get_by_name("frames").connect("new-sample", frame)
        self.pipeline.add(playback)
        if pad.link(playback.get_static_pad("sink")) != Gst.PadLinkReturn.OK:
            raise ValueError("video receive pad not linked")
        self.video_receivers.append(playback)
        playback.sync_state_with_parent()

    def connection(self, connection, _):
        state = connection.get_property("connection-state").value_nick
        emit({"type": "connection", "state": state})
        if state in ("failed", "closed"):
            GLib.idle_add(self.fail, "A conexão de áudio foi encerrada.")
        elif state == "disconnected":
            def check():
                if not self.closed and self.webrtc.get_property("connection-state").value_nick == "disconnected":
                    self.fail("A conexão de áudio caiu.")
                return False
            GLib.timeout_add_seconds(5, check)

    def set_speaking(self, active):
        if getattr(self, "speaking", False) != active:
            self.speaking = active
            emit({"type": "speaking", "active": active})

    def bus_message(self, _, message):
        if message.type == Gst.MessageType.ELEMENT and message.src.get_name() == "local_level":
            structure = message.get_structure()
            if structure and structure.get_name() == "level":
                rms = structure.get_value("rms")
                now = time.monotonic()
                muted = self.microphone.get_property("mute")
                if not muted and rms and max(rms) > -45:
                    self.last_speech = now
                self.set_speaking(not muted and now - getattr(self, "last_speech", 0) < 0.3)
            return
        if message.type == Gst.MessageType.ERROR:
            if "--diagnostics" in sys.argv:
                print(message.src.get_path_string(), message.parse_error(), file=sys.stderr)
            # Never forward raw GStreamer diagnostics: they may contain TURN URIs.
            if any(message.src == b or message.src.has_as_ancestor(b) for b in self.retired_bins):
                return
            for kind, item in self.video.items():
                if item.get("bin") and (message.src == item["bin"] or message.src.has_as_ancestor(item["bin"])):
                    self.media_failed(kind, "Não foi possível abrir ou continuar a captura de vídeo.")
                    return
            self.fail("Falha na mídia. Verifique dispositivos e servidores ICE.")

    def dispatch(self, event):
        if self.closed:
            return False
        try:
            kind = event["type"]
            if kind == "start":
                self.start(event)
            elif kind == "stop":
                self.close()
            elif kind in ("voice_answer", "voice_offer"):
                self.description(event)
            elif kind == "voice_ice_candidate":
                self.candidate(event)
            elif kind == "media":
                self.media(event)
            elif kind == "video_watch":
                self.video_enabled = bool(event.get("enabled"))
                self.video_epoch = int(event.get("epoch", 0))
                self.last_frame = time.monotonic() + 0.3
                # A switch must start decoding a fresh keyframe, not retain the
                # previous publisher's picture/reference frames in the decoder.
                for playback in self.video_receivers:
                    playback.set_state(Gst.State.READY)
                    playback.sync_state_with_parent()
                emit({"type": "video_reset", "epoch": self.video_epoch})
            elif kind == "frame_ack":
                self.frame_pending = False
            elif kind == "mute":
                self.microphone.set_property("mute", bool(event["muted"]))
                if event["muted"]:
                    self.last_speech = 0
                    self.set_speaking(False)
            elif kind == "voice_audio_routes":
                self.routes = {route["track_id"] for route in event.get("routes", [])}
                for track, volume in self.outputs:
                    volume.set_property("mute", track not in self.routes)
        except Exception as error:
            for frame in traceback.extract_tb(error.__traceback__):
                print("worker failure at line", frame.lineno, frame.name, type(error).__name__, file=sys.stderr)
            self.fail("Configuração ou sinalização de áudio inválida.")
        return False

engine = Engine()
def reader():
    try:
        for line in sys.stdin:
            GLib.idle_add(engine.dispatch, json.loads(line))
    except (ValueError, OSError):
        GLib.idle_add(engine.fail, "Comando de áudio inválido.")
    finally:
        GLib.idle_add(engine.close)
threading.Thread(target=reader, daemon=True).start()
try:
    engine.loop.run()
finally:
    engine.close()
