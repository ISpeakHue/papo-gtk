#!/usr/bin/env python3
"""EN-US: Real VP8 through the SFU; synthetic by default, --windows-capture requires an operator.
PT-BR: VP8 real pelo SFU; sintético por padrão, --windows-capture exige acompanhamento.
"""
import asyncio
import base64
import json
import os
from pathlib import Path
import sys
from probe_runtime import python, sfu_binary, spawn_options
import gi
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf

ROOT = Path(__file__).resolve().parents[1]

async def send(p, e):
    p.stdin.write((json.dumps(e) + "\n").encode())
    await p.stdin.drain()

async def main():
    native_screen = '--windows-capture' in sys.argv
    if native_screen and (sys.platform != 'win32' or os.environ.get('PAPO_WINDOWS_CAPTURE_PREVIEW') != '1'):
        raise RuntimeError('--windows-capture requires Windows and PAPO_WINDOWS_CAPTURE_PREVIEW=1; opens a real picker')
    processes, tasks = [], []
    states, negotiated, frames, connected, offers = {}, {}, {}, set(), {}
    epochs = {"a": 0, "b": 0, "c": 0}
    patterns = {("a","video"):4,("a","screen"):5,("b","video"):6,("b","screen"):4}
    errors = []
    expected_error = False
    media_errors = []
    screen_starts = []
    audio = {p:0 for p in epochs}
    relay_candidates = set()
    acknowledgements = True
    config = []
    async def spawn(*args):
        p = await asyncio.create_subprocess_exec(*args, stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE, stderr=None if "--diagnostics" in sys.argv else asyncio.subprocess.DEVNULL, limit=256*1024, **spawn_options())
        processes.append(p)
        return p
    sfu = await spawn(sfu_binary())
    clients = {p: await spawn(python(), "-u", str(ROOT / "src/voice/engine.py"), *(["--diagnostics"] if "--diagnostics" in sys.argv else [])) for p in epochs}
    async def signal(peer, event):
        await send(sfu, {"peer": peer, "event": event})
    async def pump(label, process):
        nonlocal config
        while line := await process.stdout.readline():
            event = json.loads(line)
            if label == "sfu":
                peer, event = event["peer"], event["event"]
                kind = event["type"]
                if kind == "ready":
                    config = event["ice_servers"] if "--relay" in sys.argv else []
                    for p in clients:
                        await signal(p, {"type": "voice_join"})
                elif kind == "voice_joined":
                    await signal(peer, {"type":"voice_mute","muted":False})
                    await send(clients[peer], {"type": "start", "test_source": True, "test_sink": True, "muted": False,"ice_servers":config,"relay":"--relay" in sys.argv})
                elif kind in ("voice_answer", "voice_offer", "voice_ice_candidate", "voice_audio_routes"):
                    if kind == "voice_answer" and "--diagnostics" in sys.argv:
                        print(peer, "answer", [l for l in event["sdp"].splitlines() if l.startswith(("m=", "a=mid", "a=send", "a=recv", "a=inactive"))], file=sys.stderr)
                    await send(clients[peer], event)
                elif kind == "voice_state_update":
                    states[event["user_id"]] = event
                elif kind == "error":
                    errors.append("SFU: " + event.get("code", "unknown"))
            else:
                peer, kind = label, event["type"]
                if kind in ("voice_offer", "voice_answer", "voice_ice_candidate"):
                    if kind == "voice_offer":
                        offers[peer] = offers.get(peer, 0) + 1
                        assert event["sdp"].count("\nm=") <= 17, "publishing must reuse MIDs"
                        if "--diagnostics" in sys.argv:
                            print(peer, "offer", offers[peer], [l for l in event["sdp"].splitlines() if l.startswith(("m=", "a=mid", "a=send", "a=recv", "a=inactive", "a=ssrc:"))], file=sys.stderr)
                    if kind == "voice_ice_candidate" and " typ relay " in event["candidate"]:
                        relay_candidates.add(peer)
                    await signal(peer, event)
                elif kind == "media_intent":
                    if event['kind'] == 'screen' and event['on']:
                        screen_starts.append(peer)
                    await signal(peer, {"type": "voice_camera", "on": event["on"]} if event["kind"] == "video" else {"type": "screen_share_start" if event["on"] else "screen_share_stop"})
                elif kind == "negotiated":
                    negotiated[peer] = negotiated.get(peer, 0) + 1
                elif kind == "video_frame":
                    jpeg = base64.b64decode(event["jpeg"])
                    assert jpeg.startswith(b"\xff\xd8"), "decoded video must be JPEG"
                    if event["epoch"] == epochs[peer]:
                        loader = GdkPixbuf.PixbufLoader.new_with_type("jpeg")
                        loader.write(jpeg)
                        loader.close()
                        pixels = loader.get_pixbuf()
                        assert (pixels.get_width(), pixels.get_height()) == (640,360)
                        offset = 180*pixels.get_rowstride() + 320*pixels.get_n_channels()
                        rgb = pixels.get_pixels()[offset:offset+3]
                        expected = patterns[watches[peer]] - 4
                        if not (native_screen and watches[peer] == ('a', 'screen')):
                            assert rgb[expected] > 200 and all(v < 50 for i,v in enumerate(rgb) if i != expected), "wrong publisher/kind after subscription switch: " + repr((peer,watches[peer],tuple(rgb)))
                        frames[peer] = frames.get(peer, 0) + 1
                    if acknowledgements:
                        await send(process, {"type": "frame_ack"})
                elif kind == "connection" and event["state"] == "connected":
                    connected.add(peer)
                elif kind == "audio" and event["energy"] > 0.0001:
                    audio[peer] += 1
                elif kind == "media_error" and expected_error:
                    media_errors.append(event)
                elif kind in ("error", "media_error"):
                    errors.append("worker: " + event["message"])
        errors.append(label + " unexpectedly exited")
    async def until(predicate, message, timeout=15):
        deadline = asyncio.get_running_loop().time() + timeout
        while not predicate():
            if errors:
                raise AssertionError(str(errors))
            for task in tasks:
                if task.done() and task.exception():
                    raise task.exception()
            if asyncio.get_running_loop().time() > deadline:
                raise AssertionError(message + " timed out; states=" + repr(states))
            await asyncio.sleep(0.05)
    async def media(peer, kind, on):
        before = negotiated.get(peer, 0)
        real = native_screen and peer == 'a' and kind == 'screen'
        if real and on:
            print('Select a disposable test WINDOW in the Windows picker.', flush=True)
        await send(clients[peer], {"type": "media", "kind": kind, "on": on, "test_source": not real, "pattern": patterns[peer, kind]})
        await until(lambda: negotiated.get(peer, 0) > before, "renegotiation", 140 if real and on else 15)
        field = "camera_on" if kind == "video" else "screen_sharing"
        await until(lambda: states.get(peer, {}).get(field) == on, peer + " " + field + "=" + str(on))
    watches = {}
    async def watch(peer, publisher, kind):
        if peer in watches:
            old, old_kind = watches.pop(peer)
            await signal(peer, {"type": "track_unsubscribe", "publisher_id": old, "kind": old_kind})
        epochs[peer] += 1
        await send(clients[peer], {"type": "video_watch", "epoch": epochs[peer], "enabled": True})
        watches[peer] = publisher, kind
        before = frames.get(peer, 0)
        await signal(peer, {"type": "track_subscribe", "publisher_id": publisher, "kind": kind})
        if "--diagnostics" in sys.argv:
            print("watch", peer, publisher, kind, epochs[peer], file=sys.stderr)
        await until(lambda: frames.get(peer, 0) >= before + 3, "decoded " + publisher + " " + kind)
    try:
        tasks = [asyncio.create_task(pump("sfu", sfu))] + [asyncio.create_task(pump(p, c)) for p, c in clients.items()]
        await until(lambda: connected == set(clients) and all(negotiated.get(p) for p in clients), "initial connection")
        before = negotiated["a"]
        expected_error = True
        await send(clients["a"], {"type":"media","kind":"video","on":True,"device":"papo-nonexistent-camera"})
        await until(lambda: media_errors and negotiated["a"] > before, "recoverable missing camera")
        expected_error = False
        if native_screen:
            print('CANCEL the first Windows system picker. No screen publication may start.', flush=True)
            before, failures = negotiated['a'], len(media_errors)
            expected_error = True
            await send(clients['a'], {'type': 'media', 'kind': 'screen', 'on': True})
            await until(lambda: len(media_errors) > failures and negotiated['a'] > before, 'cancelled Windows picker', 140)
            assert not screen_starts and not frames, 'Cancelled picker must not publish a source or frames'
            expected_error = False
        for cycle in range(2):
            await media("a", "video", True)
            await watch("b", "a", "video")
            await watch("c", "a", "video")
            await media("a", "screen", True)
            await watch("b", "a", "screen")
            await watch("c", "a", "screen")
            if native_screen:
                if cycle == 0:
                    print('Resize/move the selected window between DPI scales now; continuing capture for 15 seconds.', flush=True)
                    before_frames = frames.get('b', 0)
                    await asyncio.sleep(15)
                    assert frames.get('b', 0) > before_frames + 10, 'Selected stream must continue through resize/DPI changes'
                else:
                    expected_error = True
                    failures = len(media_errors)
                    before_closed = negotiated['a']
                    print('CLOSE the selected test window now. Audio must stay connected.', flush=True)
                    await until(lambda: len(media_errors) > failures and negotiated['a'] > before_closed and not states.get('a', {}).get('screen_sharing'), 'selected window closure', 140)
                    expected_error = False
            await media("a", "video", False)
            await media("a", "screen", False)
            print("Camera + screen cycle", cycle + 1, "passed: real decoded VP8, subscription switch, stop")
        # Screen-first negotiation exercises the backend's intent-based MID roles.
        await media("b", "screen", True)
        await watch("a", "b", "screen")
        await watch("c", "b", "screen")
        await media("b", "video", True)
        await watch("a", "b", "video")
        await watch("c", "b", "video")
        # UI backpressure must bound video IPC without blocking signaling/audio.
        acknowledgements = False
        await asyncio.sleep(0.5)
        held = dict(frames)
        await asyncio.sleep(0.5)
        assert frames == held, "only one unacknowledged frame may be queued"
        acknowledgements = True
        for client in clients.values():
            await send(client, {"type":"frame_ack"})
        await until(lambda: all(audio[p] >= 3 for p in clients), "audio alongside video")
        before = frames.get("a", 0)
        await signal("b", {"type": "voice_leave"})
        await asyncio.sleep(1)
        settled = frames.get("a", 0)
        await asyncio.sleep(0.5)
        assert frames.get("a", 0) == settled, "removed publisher must stop RTP"
        assert not errors, errors
        assert "--relay" not in sys.argv or relay_candidates == set(clients), "all three peers must use TURN"
        print("Reverse publisher, screen-first, backpressure, audio and leave passed; bounded offers:", offers)
    finally:
        for p in reversed(processes):
            if p.returncode is None:
                p.stdin.close()
                try:
                    await asyncio.wait_for(p.wait(), 5)
                except asyncio.TimeoutError:
                    p.kill()
                    await p.wait()
        for t in tasks:
            t.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)

asyncio.run(main())
