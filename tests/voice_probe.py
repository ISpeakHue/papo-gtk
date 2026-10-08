#!/usr/bin/env python3
"""Two native clients, real Go SFU and local TURN; never open the microphone.
Build tests/voice-sfu into /tmp/papo-voice-sfu before running this script.
"""
import asyncio
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]

async def send(process, event):
    process.stdin.write((json.dumps(event) + "\n").encode())
    await process.stdin.drain()

async def close(process):
    if process.returncode is not None:
        return
    process.stdin.close()
    try:
        await asyncio.wait_for(process.wait(), 5)
    except asyncio.TimeoutError:
        process.kill()
        await process.wait()

async def run(relay, device=None):
    queue = asyncio.Queue()
    processes = []
    pumps = []
    async def spawn(label, *args):
        process = await asyncio.create_subprocess_exec(*args, stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE, stderr=None if "--diagnostics" in sys.argv else asyncio.subprocess.DEVNULL)
        processes.append(process)
        async def pump():
            while line := await process.stdout.readline():
                await queue.put((label, json.loads(line)))
            await queue.put((label, {"type": "eof"}))
        pumps.append(asyncio.create_task(pump()))
        return process
    sfu = await spawn("sfu", "/tmp/papo-voice-sfu")
    clients = {peer: await spawn(peer, "/usr/bin/python3", "-u", str(ROOT / "src/voice/engine.py")) for peer in ("a", "b")}
    audible = set()
    connected = set()
    routes = {}
    speakers = set()
    local_speakers = set()
    local_muted = False
    candidates = set()
    muted = False
    left = False
    started = False
    deadline = asyncio.get_running_loop().time() + 40
    try:
        while asyncio.get_running_loop().time() < deadline:
            label, event = await asyncio.wait_for(queue.get(), 10)
            if "--diagnostics" in sys.argv:
                print(label, event.get("type", event.get("event", {}).get("type")), event.get("state", ""), file=sys.stderr)
            if label == "sfu":
                peer, event = event["peer"], event["event"]
                kind = event["type"]
                if kind == "voice_answer" and "--diagnostics" in sys.argv:
                    print("Answer directions:", [line for line in event["sdp"].splitlines() if line.startswith(("m=", "a=send", "a=recv", "a=inactive"))], file=sys.stderr)
                if kind == "ready":
                    config = event["ice_servers"] if relay else []
                    for peer in clients:
                        await send(sfu, {"peer": peer, "event": {"type": "voice_join"}})
                elif kind == "voice_joined":
                    await send(sfu, {"peer": peer, "event": {"type": "voice_mute", "muted": False}})
                    await send(clients[peer], {"type": "start", "ice_servers": config, "relay": relay, "test_source": device is None, "device": device, "test_sink": True, "frequency": 440 if peer == "a" else 880})
                elif kind in ("voice_answer", "voice_offer", "voice_ice_candidate", "voice_audio_routes"):
                    if kind == "voice_audio_routes":
                        routes[peer] = event["routes"]
                    await send(clients[peer], event)
                elif kind == "active_speaker_update":
                    speakers.update(event["user_ids"])
                elif kind == "error":
                    raise AssertionError("SFU rejected signaling: " + event.get("code", "unknown"))
                elif kind == "voice_state_update" and event["user_id"] == "a" and event["muted"]:
                    muted = True
                elif kind == "voice_leave" and event["user_id"] == "b":
                    left = True
            else:
                kind = event["type"]
                if kind == "voice_offer" and "--diagnostics" in sys.argv:
                    print("SDP directions:", [line for line in event["sdp"].splitlines() if line.startswith(("m=", "a=send", "a=recv", "a=extmap"))], file=sys.stderr)
                if kind in ("voice_offer", "voice_answer", "voice_ice_candidate"):
                    if kind == "voice_ice_candidate" and " typ relay " in event["candidate"]:
                        candidates.add(label)
                    await send(sfu, {"peer": label, "event": event})
                elif kind == "connection" and event["state"] == "connected":
                    connected.add(label)
                elif kind == "speaking":
                    if event["active"]:
                        local_speakers.add(label)
                    elif label == "a":
                        local_muted = True
                elif kind == "audio" and event["energy"] > 0.0001:
                    assert any(r["track_id"] == event["track_id"] for r in routes.get(label, [])), "audio must have a current SFU route"
                    audible.add(label)
                elif kind == "error":
                    raise AssertionError("Native worker: " + event["message"])
                elif kind == "eof":
                    raise AssertionError("native worker exited unexpectedly: " + str(await clients[label].wait()))
            if audible == connected == speakers == local_speakers == {"a", "b"} and not started:
                assert not relay or candidates == {"a", "b"}, "relay-only peers must use TURN"
                assert speakers == {"a", "b"}, "real RFC6464 active speaker detection"
                started = True
                await send(clients["a"], {"type": "mute", "muted": True})
                await send(sfu, {"peer": "a", "event": {"type": "voice_mute", "muted": True}})
            if started and muted and not routes.get("b") and not left:
                await send(sfu, {"peer": "b", "event": {"type": "voice_leave"}})
            if left and not routes.get("a") and local_muted:
                print(("TURN relay" if relay else "Direct ICE") + ": bidirectional decoded audio, routes, active speakers, mute and leave passed")
                return
        raise AssertionError("audio probe timed out")
    finally:
        for process in processes[1:]:
            await close(process)
        await close(sfu)
        for pump in pumps:
            pump.cancel()
        await asyncio.gather(*pumps, return_exceptions=True)

async def invalid_device():
    p = await asyncio.create_subprocess_exec("/usr/bin/python3", "-u", str(ROOT / "src/voice/engine.py"), stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE, stderr=None if "--diagnostics" in sys.argv else asyncio.subprocess.DEVNULL)
    try:
        await send(p, {"type": "start", "device": "papo-nonexistent-test-microphone"})
        event = json.loads(await asyncio.wait_for(p.stdout.readline(), 10))
        assert event["type"] == "error" and "microfone" in event["message"]
        await asyncio.wait_for(p.wait(), 5)
        print("Missing capture device: reported and resources released")
    finally:
        await close(p)

async def virtual_capture():
    # Private PulseAudio/PipeWire nodes carry a test tone, never the user's mic.
    name = "papo_voice_probe_" + str(__import__("os").getpid())
    modules = []
    tone = None
    async def pactl(*args):
        p = await asyncio.create_subprocess_exec("pactl", *args, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.DEVNULL)
        out, _ = await p.communicate()
        assert p.returncode == 0, "could not create private virtual capture fixture"
        return out.decode().strip()
    try:
        modules.append(await pactl("load-module", "module-null-sink", "sink_name=" + name))
        modules.append(await pactl("load-module", "module-remap-source", "master=" + name + ".monitor", "source_name=" + name + "_capture"))
        tone = await asyncio.create_subprocess_exec("gst-launch-1.0", "-q", "audiotestsrc", "is-live=true", "volume=0.25", "!", "audioconvert", "!", "audioresample", "!", "pulsesink", "device=" + name, stdout=asyncio.subprocess.DEVNULL, stderr=asyncio.subprocess.DEVNULL)
        await asyncio.sleep(0.3)
        p = await asyncio.create_subprocess_exec("/usr/bin/python3", str(ROOT / "src/voice/engine.py"), "--devices", stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.DEVNULL)
        out, _ = await p.communicate()
        devices = json.loads(out)["devices"]
        device = next(d["id"] for d in devices if name in d["id"])
        await run(False, device)
        print("Native device capture: private virtual source enumerated, selected and streamed through SFU")
    finally:
        if tone:
            tone.terminate()
            await tone.wait()
        for module in reversed(modules):
            await pactl("unload-module", module)

async def main():
    await run(False)
    await run(True)
    await invalid_device()
    if "--capture" in sys.argv:
        await virtual_capture()

asyncio.run(main())
