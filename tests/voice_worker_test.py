"""EN-US: Device policy with synthetic metadata. PT-BR: Política de dispositivos com metadados sintéticos."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location("voice_worker", Path(__file__).resolve().parents[1] / "src/voice/engine.py")
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


def device(identifier, name="USB microphone", kind="Audio/Source"):
    item = Mock()
    item.get_display_name.return_value = name
    item.get_device_class.return_value = kind
    properties = worker.Gst.Structure.new_empty("props")
    properties.set_value("device.strid", identifier)
    properties.set_value("device.name", name)
    item.get_properties.return_value = properties
    return item


class DeviceTests(unittest.TestCase):
    def test_microphone_picker_rejects_wasapi_loopback_without_opening_devices(self):
        microphone = device("mic")
        loopback = device("speakers", "Speakers (loopback)")
        loopback.get_properties().set_value("wasapi2.device.loopback", True)
        camera = device("cam", "Camera", "Video/Source")
        monitor = Mock()
        monitor.get_devices.return_value = [loopback, microphone, camera]
        entries = worker.device_snapshot(monitor, "Audio/Source")
        self.assertEqual([entry[0] for entry in entries], ["mic"])
        self.assertIsNone(worker.select_device("speakers", entries))
        self.assertEqual([entry[0] for entry in worker.device_snapshot(monitor, "Video/Source")], ["cam"])
        for item in (microphone, loopback, camera):
            item.create_element.assert_not_called()

    def test_system_default_alias_is_not_a_fixed_microphone_endpoint(self):
        alias = device("default-mic")
        props = alias.get_properties()
        props.set_value("device.api", "wasapi2")
        props.set_value("device.default", True)
        self.assertTrue(worker.default_alias(alias))
        props.set_value("device.default", False)
        self.assertFalse(worker.default_alias(alias))
        props.set_value("device.api", "wasapi")
        props.set_value("device.default", True)
        self.assertFalse(worker.default_alias(alias))

    def test_native_start_tracks_default_alias_and_gain_never_unmutes(self):
        engine = worker.Engine()
        engine.monitor_devices = Mock()
        engine.monitor = Mock()
        alias = device("default-mic")
        props = alias.get_properties()
        props.set_value("device.api", "wasapi2")
        props.set_value("device.default", True)
        alias.create_element.side_effect = lambda name: worker.Gst.ElementFactory.make("audiotestsrc", name)
        engine.monitor.get_devices.return_value = [alias]
        try:
            with patch.object(worker, "emit"):
                engine.start({"device": "default-mic", "test_sink": True, "audio_slots": 0, "video_slots": 0})
                self.assertTrue(engine.default_microphone)
                self.assertEqual(engine.microphone_id, "default-mic")
                self.assertTrue(engine.microphone.get_property("mute"), "omitted mute must start muted")
                for gain, expected in ((0, 0), (1.5, 1.5), (3, 3), (99, 3), (-1, 0)):
                    engine.dispatch({"type": "microphone_gain", "gain": gain})
                    self.assertEqual(engine.microphone.get_property("volume"), expected)
                    self.assertTrue(engine.microphone.get_property("mute"))
        finally:
            engine.close()

    def test_loopback_hotplug_does_not_interrupt_default_microphone(self):
        engine = self.engine()
        engine.default_microphone = True
        loopback = device("speakers")
        loopback.get_properties().set_value("wasapi2.device.loopback", True)
        with patch.object(worker, "emit") as emit:
            engine.device_message(None, self.message(worker.Gst.MessageType.DEVICE_ADDED, loopback))
        engine.fail.assert_not_called()
        emit.assert_not_called()

    def test_windows_endpoint_ids_survive_duplicate_friendly_names(self):
        a, b = device("{endpoint-a}"), device("{endpoint-b}")
        monitor = Mock()
        monitor.get_devices.return_value = [a, b, a]
        entries = worker.device_snapshot(monitor)
        self.assertEqual([entry[0] for entry in entries], ["{endpoint-a}", "{endpoint-b}"])
        self.assertIs(worker.select_device("{endpoint-b}", entries), b)
        self.assertIsNone(worker.select_device("USB microphone", entries))
        self.assertIs(worker.select_device("USB microphone", entries[:1]), a)
        a.create_element.assert_not_called()

    def test_windows_camera_symbolic_path_fallback_does_not_open_capture(self):
        item = device("unused", "USB camera", "Video/Source")
        item.get_properties.return_value = None
        source = item.create_element.return_value
        source.find_property.side_effect = lambda key: key == "device-path"
        source.get_property.return_value = "camera-symbolic-link"
        with patch.object(worker.sys, "platform", "win32"):
            self.assertEqual(worker.device_identifier(item), "camera-symbolic-link")
        source.set_state.assert_not_called()

    def test_stopped_or_closed_picker_callbacks_cannot_publish_or_renegotiate(self):
        for closed in (False, True):
            with self.subTest(closed=closed):
                engine = worker.Engine()
                engine.publish_video = Mock()
                engine.media_failed = Mock()
                callbacks = []
                def create(ready, ended):
                    callbacks.extend((ready, ended))
                    return Mock()
                with patch.object(worker, 'create_screen_capture', side_effect=create):
                    engine.media({'kind': 'screen', 'on': True})
                self.assertEqual(len(callbacks), 2)
                if closed:
                    engine.close()
                else:
                    engine.stop_capture('screen')
                callbacks[0](Mock())
                callbacks[1]()
                engine.publish_video.assert_not_called()
                engine.media_failed.assert_not_called()

    def test_current_picker_callback_passes_only_the_source_to_shared_encoder(self):
        engine = worker.Engine()
        engine.publish_video = Mock()
        callbacks = []
        with patch.object(worker, 'create_screen_capture', side_effect=lambda ready, ended: callbacks.append(ready) or Mock()):
            engine.media({'kind': 'screen', 'on': True})
        source = Mock()
        callbacks[0](source)
        engine.publish_video.assert_called_once_with('screen', source)
        engine.close()

    def test_enumeration_stops_monitor_on_failure(self):
        monitor = Mock()
        monitor.get_devices.side_effect = RuntimeError("enumeration failed")
        with patch.object(worker, "device_monitor", return_value=monitor):
            with self.assertRaises(RuntimeError):
                worker.devices()
        monitor.stop.assert_called_once()

    def engine(self):
        engine = worker.Engine()
        engine.fail = Mock()
        engine.media_failed = Mock()
        engine.microphone_id = "mic-a"
        engine.camera_id = "cam-a"
        engine.device_ids = {"mic-a", "cam-a"}
        return engine

    def message(self, kind, item):
        message = Mock(type=kind)
        message.parse_device_added.return_value = item
        message.parse_device_removed.return_value = item
        message.parse_device_changed.return_value = (item, item)
        return message

    def test_selected_microphone_unplug_requires_explicit_rejoin(self):
        engine = self.engine()
        with patch.object(worker, "emit") as emit:
            engine.device_message(None, self.message(worker.Gst.MessageType.DEVICE_REMOVED, device("mic-a")))
        engine.fail.assert_called_once()
        emit.assert_called_once_with({"type": "devices_changed"})
        engine.media_failed.assert_not_called()

    def test_unrelated_device_does_not_end_explicit_capture(self):
        engine = self.engine()
        with patch.object(worker, "emit"):
            engine.device_message(None, self.message(worker.Gst.MessageType.DEVICE_REMOVED, device("mic-b")))
        engine.fail.assert_not_called()

    def test_active_endpoint_identity_change_requires_rejoin(self):
        engine = self.engine()
        message = self.message(worker.Gst.MessageType.DEVICE_CHANGED, device("mic-new"))
        message.parse_device_changed.return_value = (device("mic-new"), device("mic-a"))
        with patch.object(worker, "emit"):
            engine.device_message(None, message)
        engine.fail.assert_called_once()

    def test_changed_device_retires_the_old_identity_before_replug(self):
        engine = self.engine()
        old, new = device("old"), device("new")
        engine.device_ids.add("old")
        message = self.message(worker.Gst.MessageType.DEVICE_CHANGED, new)
        message.parse_device_changed.return_value = (new, old)
        with patch.object(worker, "emit") as emit:
            engine.device_message(None, message)
            engine.device_message(None, self.message(worker.Gst.MessageType.DEVICE_ADDED, old))
        self.assertEqual(emit.call_count, 2, "a replugged old identity must refresh settings")

    def test_default_audio_endpoint_change_ends_call_without_silent_switch(self):
        engine = self.engine()
        engine.default_microphone = True
        with patch.object(worker, "emit"):
            engine.device_message(None, self.message(worker.Gst.MessageType.DEVICE_CHANGED, device("mic-b")))
        engine.fail.assert_called_once()

    def test_camera_unplug_preserves_audio(self):
        engine = self.engine()
        with patch.object(worker, "emit"):
            engine.device_message(None, self.message(worker.Gst.MessageType.DEVICE_REMOVED, device("cam-a", kind="Video/Source")))
        engine.fail.assert_not_called()
        self.assertEqual(engine.media_failed.call_args.args[0], "video")

    def test_camera_permission_error_is_recoverable(self):
        engine = self.engine()
        engine.monitor_devices = Mock()
        camera = device("cam-a", kind="Video/Source")
        camera.create_element.side_effect = PermissionError("synthetic permission denial")
        with patch.object(worker, "device_snapshot", return_value=[("cam-a", "Camera", camera)]):
            engine.media({"kind": "video", "on": True, "device": "cam-a"})
        engine.fail.assert_not_called()
        self.assertEqual(engine.media_failed.call_args.args[0], "video")

    def test_initial_monitor_snapshot_does_not_interrupt_default_capture(self):
        engine = self.engine()
        engine.default_microphone = True
        with patch.object(worker, "emit") as emit:
            engine.device_message(None, self.message(worker.Gst.MessageType.DEVICE_ADDED, device("mic-a")))
        engine.fail.assert_not_called()
        emit.assert_not_called()

    def test_monitor_is_stopped_on_idempotent_shutdown(self):
        engine = worker.Engine()
        engine.monitor, engine.monitor_bus = Mock(), Mock()
        monitor = engine.monitor
        engine.close()
        engine.close()
        monitor.stop.assert_called_once()
        engine.monitor_bus.remove_signal_watch.assert_called_once()

    def test_camera_chain_decodes_raw_and_mjpeg_without_a_camera(self):
        Gst = worker.Gst
        for encoding in ("", " ! jpegenc"):
            with self.subTest(encoding=encoding or "raw"):
                pipeline = Gst.Pipeline.new(None)
                source = Gst.parse_bin_from_description("videotestsrc num-buffers=3" + encoding, True)
                codec = Gst.parse_bin_from_description("videoconvert ! fakesink name=decoded signal-handoffs=true sync=false", True)
                frames = []
                codec.get_by_name("decoded").connect("handoff", lambda *_: frames.append(True))
                pipeline.add(source)
                pipeline.add(codec)
                failed = Mock()
                worker.link_camera(source, codec, pipeline, failed)
                try:
                    pipeline.set_state(Gst.State.PLAYING)
                    message = pipeline.get_bus().timed_pop_filtered(5 * Gst.SECOND, Gst.MessageType.EOS | Gst.MessageType.ERROR)
                    self.assertIsNotNone(message, "camera decoder timed out")
                    self.assertEqual(message.type, Gst.MessageType.EOS)
                    self.assertEqual(len(frames), 3)
                    failed.assert_not_called()
                finally:
                    pipeline.set_state(Gst.State.NULL)


if __name__ == "__main__":
    unittest.main()
