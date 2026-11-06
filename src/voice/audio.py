# EN-US: Explicit local device tests and transactional audio changes; no server signaling.
# PT-BR: Testes locais explícitos e trocas transacionais de áudio, sem sinalização ao servidor.
class AudioTools:
    def init_audio_tools(self):
        self.output_device = None
        self.audio_capture = None
        self.output_branches = {}
        self.audio_pending = None
        self.local_test = False
        self.recording = bytearray()
        self.record_done = False
        self.synthetic_source = False
        self.audio_timers = []

    def local_audio_device(self, element):
        if element is None:
            raise ValueError("audio device unavailable")
        if self.local_test:
            # Tests must use Papo's mixer identity, rather than Python's saved
            # mute/volume/routing, just like the application's call cues.
            if element.find_property("stream-properties"):
                props = Gst.Structure.new_empty("props")
                props.set_value("application.id", "br.com.papo.gtk")
                props.set_value("application.name", "Papo")
                props.set_value("media.role", "communication")
                props.set_value("media.name", "Papo audio test")
                element.set_property("stream-properties", props)
            if element.find_property("client-name"):
                element.set_property("client-name", "Papo")
        return element

    def audio_source(self, identifier=None, synthetic=False):
        if synthetic:
            source = Gst.ElementFactory.make("audiotestsrc", None)
            source.set_property("is-live", True)
            source.set_property("volume", 0.25)
            return source, None, False
        if identifier:
            selected = select_device(identifier, device_snapshot(self.monitor, "Audio/Source"))
            if selected is None:
                raise ValueError("missing microphone")
            return self.local_audio_device(selected.create_element(None)), device_identifier(selected), default_alias(selected)
        factory = next((name for name in ("wasapi2src", "wasapisrc") if Gst.ElementFactory.find(name)), "autoaudiosrc") if sys.platform == "win32" else "autoaudiosrc"
        if self.local_test and sys.platform.startswith("linux") and Gst.ElementFactory.find("pulsesrc"):
            factory = "pulsesrc"
        return self.local_audio_device(Gst.ElementFactory.make(factory, None)), None, True

    def audio_sink(self, identifier=None):
        if self.test_sink:
            sink = Gst.ElementFactory.make("fakesink", None)
            # Local tests must exercise preroll and timed replay like hardware.
            sink.set_property("sync", self.local_test)
            return sink
        if identifier:
            selected = select_device(identifier, device_snapshot(self.monitor, "Audio/Sink"))
            if selected is None:
                raise ValueError("missing output")
            return self.local_audio_device(selected.create_element(None))
        factory = next((name for name in ("wasapi2sink", "wasapisink") if Gst.ElementFactory.find(name)), "autoaudiosink") if sys.platform == "win32" else "autoaudiosink"
        if self.local_test and sys.platform.startswith("linux") and Gst.ElementFactory.find("pulsesink"):
            # autoaudiosink can silently fall back to a fake sink when no real
            # output opens. An explicit audio-server sink reports that failure.
            factory = "pulsesink"
        return self.local_audio_device(Gst.ElementFactory.make(factory, None))

    def watch_audio_bus(self):
        bus = self.pipeline.get_bus()
        bus.add_signal_watch()
        bus.connect("message", self.bus_message)

    def start_audio_test(self, config):
        if self.pipeline:
            raise ValueError("already started")
        self.local_test = True
        if GLib.get_application_name() != "Papo":
            GLib.set_application_name("Papo")
        GLib.set_prgname("papo-gtk")
        self.test_sink = bool(config.get("test_sink"))
        self.synthetic_source = bool(config.get("test_source"))
        self.output_device = config.get("output_device")
        # Default routing is owned by the audio server. A local test must not
        # depend on camera/device enumeration, or fail on a newly added input.
        explicit_input = config.get("mode") == "microphone" and config.get("device") and not self.synthetic_source
        if explicit_input or (self.output_device and not self.test_sink):
            self.monitor_devices("Audio/Source", "Audio/Sink")
        self.pipeline = Gst.Pipeline.new("papo-local-audio-test")
        mode = config.get("mode")
        if mode == "microphone":
            source, self.microphone_id, self.default_microphone = self.audio_source(config.get("device"), self.synthetic_source)
            capture = Gst.parse_bin_from_description("audioconvert ! audioresample ! audio/x-raw,format=S16LE,rate=48000,channels=1 ! volume name=microphone ! level name=local_level post-messages=true interval=100000000 ! appsink name=record emit-signals=true sync=false max-buffers=4 drop=true", True)
            self.microphone = capture.get_by_name("microphone")
            self.set_microphone_gain(config.get("microphone_gain", 1.5))
            def sample(sink):
                sample = sink.emit("pull-sample")
                if sample and not self.record_done:
                    buffer = sample.get_buffer()
                    remaining = 48000 * 2 * 3 - len(self.recording)
                    if remaining > 0:
                        self.recording.extend(buffer.extract_dup(0, min(remaining, buffer.get_size())))
                    if len(self.recording) >= 48000 * 2 * 3:
                        self.record_done = True
                        GLib.idle_add(self.finish_recording)
                return Gst.FlowReturn.OK
            capture.get_by_name("record").connect("new-sample", sample)
            self.pipeline.add(source)
            self.pipeline.add(capture)
            if not source.link(capture):
                raise ValueError("microphone test link failed")
            emit({"type": "test_state", "state": "recording"})
            def finish():
                if not self.closed and not self.record_done:
                    self.record_done = True
                    self.finish_recording()
                return False
            self.audio_timers.append(GLib.timeout_add(3200, finish))
        elif mode == "playback":
            source = Gst.ElementFactory.make("audiotestsrc", None)
            # A finite output test must preroll the real sink before its clock
            # starts. A live source with a fake sink did not exercise this path.
            source.set_property("is-live", False)
            source.set_property("volume", 0.12)
            source.set_property("freq", 660.0)
            source.set_property("samplesperbuffer", 480)
            source.set_property("num-buffers", 200)
            self.test_playback(source)
            emit({"type": "test_state", "state": "playback"})
        else:
            raise ValueError("invalid local test")
        self.watch_audio_bus()
        emit({"type": "started", "pid": os.getpid()})
        if self.pipeline.set_state(Gst.State.PLAYING) == Gst.StateChangeReturn.FAILURE:
            self.fail("Não foi possível testar o dispositivo. Verifique as permissões e a seleção.")
        def deadline():
            if not self.closed:
                self.fail("O teste de áudio demorou demais.")
            return False
        self.audio_timers.append(GLib.timeout_add_seconds(12, deadline))

    def test_playback(self, source):
        # Keep the test duration independent of the hardware's negotiated rate.
        convert = Gst.parse_bin_from_description("capsfilter caps=audio/x-raw,rate=48000,channels=1 ! audioconvert ! audioresample", True)
        sink = self.audio_sink(self.output_device)
        for element in (source, convert, sink):
            self.pipeline.add(element)
        if not source.link(convert) or not convert.link(sink):
            raise ValueError("playback test link failed")

    def finish_recording(self):
        if self.closed:
            return False
        old = self.pipeline
        old.get_bus().remove_signal_watch()
        old.set_state(Gst.State.NULL)
        if not self.recording:
            self.fail("O microfone não forneceu áudio. Verifique as permissões.")
            return False
        if not any(self.recording):
            self.fail("Nenhum som foi detectado. Verifique se o microfone está silenciado ou escolha outro dispositivo.")
            return False
        self.pipeline = Gst.Pipeline.new("papo-local-audio-replay")
        self.watch_audio_bus()
        source = Gst.ElementFactory.make("appsrc", None)
        source.set_property("caps", Gst.Caps.from_string("audio/x-raw,format=S16LE,rate=48000,channels=1,layout=interleaved"))
        source.set_property("format", Gst.Format.TIME)
        self.test_playback(source)
        data = bytes(self.recording)
        self.recording.clear()
        buffer = Gst.Buffer.new_allocate(None, len(data), None)
        buffer.fill(0, data)
        buffer.pts = 0
        buffer.duration = len(data) * Gst.SECOND // (48000 * 2)
        emit({"type": "test_state", "state": "playback"})
        if self.pipeline.set_state(Gst.State.PLAYING) == Gst.StateChangeReturn.FAILURE:
            self.fail("Não foi possível reproduzir o teste. Verifique a saída de áudio.")
            return False
        if source.emit("push-buffer", buffer) != Gst.FlowReturn.OK:
            self.fail("Não foi possível reproduzir o teste. Verifique a saída de áudio.")
            return False
        source.emit("end-of-stream")
        return False

    def app_source(self, caps):
        source = Gst.ElementFactory.make("appsrc", None)
        source.set_property("is-live", True)
        source.set_property("format", Gst.Format.TIME)
        source.set_property("do-timestamp", True)
        source.set_property("block", False)
        source.set_property("max-bytes", 48000)
        if source.find_property("leaky-type"):
            source.set_property("leaky-type", 2)
        if caps:
            source.set_property("caps", caps)
        return source

    def push_audio(self, source, sample):
        if not sample or self.closed or source.get_property("current-level-bytes") >= 48000:
            return
        source.set_property("caps", sample.get_caps())
        buffer = sample.get_buffer().copy_deep()
        buffer.pts = Gst.CLOCK_TIME_NONE
        buffer.dts = Gst.CLOCK_TIME_NONE
        source.emit("push-buffer", buffer)

    def make_capture(self, source, identifier, default):
        pipeline = Gst.Pipeline.new(None)
        convert = Gst.parse_bin_from_description("audioconvert ! audioresample ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! appsink name=bridge emit-signals=true sync=false max-buffers=2 drop=true", True)
        pipeline.add(source)
        pipeline.add(convert)
        if not source.link(convert):
            raise ValueError("capture bridge link failed")
        capture = {"pipeline": pipeline, "source": source, "id": identifier, "default": default}
        def sample(sink):
            sample = sink.emit("pull-sample")
            pending = self.audio_pending
            if capture is self.audio_capture or (pending and pending.get("capture") is capture):
                self.push_audio(self.input, sample)
                if pending and pending.get("capture") is capture:
                    GLib.idle_add(self.commit_audio, pending["event"]["request"])
            return Gst.FlowReturn.OK
        convert.get_by_name("bridge").connect("new-sample", sample)
        self.device_bus(pipeline, capture)
        return capture

    def bridge_capture(self, source):
        # EN-US: Bounded raw-audio IPC inside this process decouples hardware lifetime from RTP/ICE.
        # PT-BR: Uma fila limitada de áudio bruto desacopla o dispositivo do ciclo de vida RTP/ICE.
        self.input = self.app_source(Gst.Caps.from_string("audio/x-raw,format=S16LE,rate=48000,channels=2,layout=interleaved"))
        self.audio_capture = self.make_capture(source, self.microphone_id, self.default_microphone)
        return self.input

    def make_output(self, identifier, caps=None):
        pipeline = Gst.Pipeline.new(None)
        source = self.app_source(caps)
        convert = Gst.parse_bin_from_description("audioconvert ! audioresample", True)
        sink = self.audio_sink(identifier)
        for element in (source, convert, sink):
            pipeline.add(element)
        if not source.link(convert) or not convert.link(sink):
            raise ValueError("output bridge link failed")
        output = {"pipeline": pipeline, "input": source, "sink": sink}
        self.device_bus(pipeline, output)
        return output

    def receive_audio_sample(self, sink, playback):
        sample = sink.emit("pull-sample")
        if self.closed:
            return Gst.FlowReturn.OK
        pending = self.audio_pending
        if pending and pending["event"]["kind"] == "output" and playback not in pending["outputs"]:
            return Gst.FlowReturn.OK
        output = pending.get("outputs", {}).get(playback) if pending else None
        output = output or self.output_branches.get(playback)
        if not output:
            try:
                output = self.make_output(self.output_device, sample.get_caps())
                self.output_branches[playback] = output
                output["pipeline"].set_state(Gst.State.PLAYING)
            except Exception:
                GLib.idle_add(self.fail, "Não foi possível abrir a saída de áudio. Verifique o dispositivo.")
                return Gst.FlowReturn.OK
        self.push_audio(output["input"], sample)
        return Gst.FlowReturn.OK

    def device_bus(self, pipeline, branch):
        bus = pipeline.get_bus()
        bus.add_signal_watch()
        def message(_bus, event):
            if self.closed:
                return
            pending = self.audio_pending
            candidate = pending and (pending.get("capture") is branch or any(b is branch for b in pending.get("outputs", {}).values()))
            active = self.audio_capture is branch or any(b is branch for b in self.output_branches.values())
            if event.type == Gst.MessageType.ERROR and (candidate or active):
                if pending:
                    self.rollback_audio()
                else:
                    self.fail("Falha no dispositivo de áudio. Verifique as permissões e a seleção.")
            elif candidate and pending["event"]["kind"] == "output" and event.type == Gst.MessageType.STATE_CHANGED and event.src == pipeline:
                _, state, _ = event.parse_state_changed()
                if state == Gst.State.PLAYING:
                    pending["ready"].add(id(branch))
                    if len(pending["ready"]) == len(pending["outputs"]):
                        self.commit_audio(pending["event"]["request"])
        bus.connect("message", message)
        branch["bus"] = bus

    def stop_branch(self, branch):
        branch["pipeline"].set_state(Gst.State.NULL)

    def drop_branch(self, branch):
        self.stop_branch(branch)
        branch["bus"].remove_signal_watch()

    def audio_result(self, event, success):
        emit({"type": "audio_device", "request": event["request"], "kind": event["kind"], "success": success, "device": event.get("device")})

    def switch_audio(self, event):
        if self.audio_pending or self.local_test:
            self.audio_result(event, False)
            return
        pending = {"event": event, "ready": set(), "timer": None, "outputs": {}}
        try:
            kind = event["kind"]
            if kind == "microphone":
                source, identifier, default = self.audio_source(event.get("device"), self.synthetic_source)
                pending["capture"] = self.make_capture(source, identifier, default)
            elif kind == "output":
                if event.get("device") and not self.test_sink and select_device(event["device"], device_snapshot(self.monitor, "Audio/Sink")) is None:
                    raise ValueError("missing output")
                for playback, old in list(self.output_branches.items()):
                    caps = old["input"].get_property("caps")
                    pending["outputs"][playback] = self.make_output(event.get("device"), caps)
                if not self.output_branches:
                    self.output_device = event.get("device")
                    self.audio_result(event, True)
                    return
            else:
                raise ValueError("invalid audio device type")
            self.audio_pending = pending
            pending["timer"] = GLib.timeout_add_seconds(5, self.audio_timeout, event["request"])
            if kind == "microphone":
                self.stop_branch(self.audio_capture)
                if pending["capture"]["pipeline"].set_state(Gst.State.PLAYING) == Gst.StateChangeReturn.FAILURE:
                    raise ValueError("microphone start failed")
            else:
                for playback, candidate in pending["outputs"].items():
                    self.stop_branch(self.output_branches[playback])
                    if candidate["pipeline"].set_state(Gst.State.PLAYING) == Gst.StateChangeReturn.FAILURE:
                        raise ValueError("output start failed")
                    # Silent preroll checks the new sink even if the remote participant is silent.
                    caps = candidate["input"].get_property("caps") or Gst.Caps.from_string("audio/x-raw,format=F32LE,rate=48000,channels=2,layout=interleaved")
                    candidate["input"].set_property("caps", caps)
                    buffer = Gst.Buffer.new_allocate(None, 3840, None)
                    buffer.memset(0, 0, 3840)
                    buffer.duration = 10 * Gst.MSECOND
                    candidate["input"].emit("push-buffer", buffer)
        except Exception:
            if self.audio_pending:
                self.rollback_audio()
            else:
                for branch in list(pending.get("outputs", {}).values()) + ([pending["capture"]] if "capture" in pending else []):
                    self.drop_branch(branch)
                self.audio_result(event, False)

    def commit_audio(self, request):
        pending = self.audio_pending
        if not pending or pending["event"]["request"] != request or self.closed:
            return False
        if pending["timer"]:
            GLib.source_remove(pending["timer"])
        event = pending["event"]
        if event["kind"] == "microphone":
            self.drop_branch(self.audio_capture)
            self.audio_capture = pending["capture"]
            self.source = self.audio_capture["source"]
            self.microphone_id = self.audio_capture["id"]
            self.default_microphone = self.audio_capture["default"]
        else:
            for playback, branch in pending["outputs"].items():
                self.drop_branch(self.output_branches[playback])
                self.output_branches[playback] = branch
            self.output_device = event.get("device")
        self.audio_pending = None
        self.audio_result(event, True)
        return False

    def audio_timeout(self, request):
        if self.audio_pending and self.audio_pending["event"]["request"] == request:
            self.audio_pending["timer"] = None
            self.rollback_audio()
        return False

    def rollback_audio(self):
        pending = self.audio_pending
        if not pending:
            return
        self.audio_pending = None
        if pending["timer"]:
            GLib.source_remove(pending["timer"])
        restored = True
        if "capture" in pending:
            self.drop_branch(pending["capture"])
            restored = self.audio_capture["pipeline"].set_state(Gst.State.PLAYING) != Gst.StateChangeReturn.FAILURE
        for playback, branch in pending["outputs"].items():
            self.drop_branch(branch)
            restored = (self.output_branches[playback]["pipeline"].set_state(Gst.State.PLAYING) != Gst.StateChangeReturn.FAILURE) and restored
        self.audio_result(pending["event"], False)
        if not restored:
            self.fail("Não foi possível restaurar o áudio. Entre novamente na chamada.")

    def audio_bus_message(self, message):
        if self.local_test:
            if message.type == Gst.MessageType.EOS:
                emit({"type": "test_state", "state": "completed"})
                self.close()
                return True
            if message.type == Gst.MessageType.ERROR:
                self.fail("Não foi possível testar o dispositivo. Verifique as permissões e a seleção.")
                return True
        return False
