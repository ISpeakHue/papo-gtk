# Original media fixtures

`chat-video.webm` is a two-second original blue frame and synthesized sine tone,
encoded as VP8/Vorbis for native inline playback and cleanup regression checks.
It contains no downloaded or third-party media. Regenerate with:

```sh
ffmpeg -v error -f lavfi -i color=c=0x3078b4:s=160x90:r=15 -f lavfi -i sine=frequency=440:sample_rate=22050 -t 2 -c:v libvpx -deadline realtime -c:a libvorbis -shortest -n tests/fixtures/chat-video.webm
```

The application cue in `src/media/voice_join.wav` is an original 220 ms mono
PCM waveform at 22,050 Hz: 660 Hz followed by 880 Hz, with an attack and decay.
Tests mute native media while checking decoding. Neither fixture needs FFmpeg
at test runtime. Both are distributed under this repository's GPL-3.0 license.

`animated-emoji.png` (APNG) and `animated-emoji.webp` are original 64×64 red
and green frames with 40 ms and 80 ms delays. They exercise server emoji
animation decoding and downsampling. Regenerate using Pillow (needed only to
regenerate fixtures, not to run the application or tests):

```python
from PIL import Image
frames = [Image.new("RGBA", (64, 64), color) for color in ["red", "lime"]]
for ext in ["png", "webp"]:
    frames[0].save(f"tests/fixtures/animated-emoji.{ext}", save_all=True,
                   append_images=frames[1:], duration=[40, 80], loop=0,
                   lossless=True)
```

These fixtures are distributed under this repository's GPL-3.0 license.

`chat-h264.mp4` is an original two-second 160×96 test pattern and synthesized
330 Hz tone, encoded as H.264/AAC. AppImage validation decodes its video through
the software H.264 fallback to verify packaging on machines without GPU drivers.
Tests use a discard sink, with no audio playback. Regenerate with FFmpeg and its
OpenH264 encoder (needed only when regenerating):

```sh
ffmpeg -hide_banner -loglevel error -f lavfi -i testsrc2=size=160x96:rate=24 -f lavfi -i sine=frequency=330:sample_rate=44100 -t 2 -c:v libopenh264 -b:v 160k -pix_fmt yuv420p -c:a aac -n tests/fixtures/chat-h264.mp4
```

This fixture contains no downloaded or personal media and is distributed under
this repository's GPL-3.0 license.

`chat-video.mov` remuxes only the video track of `chat-h264.mp4` into a genuine
QuickTime container (`qt  ` brand). It has no audio, so the native MOV playback
regression cannot emit sound. It is original fixture media under the same
GPL-3.0 license. Regenerate without re-encoding:

```sh
ffmpeg -v error -i tests/fixtures/chat-h264.mp4 -map 0:v:0 -an -c:v copy -f mov -n tests/fixtures/chat-video.mov
```

The MOV fixture tests content validation, poster extraction, playback and cache
reuse when the backend reports either `video/quicktime` or generic binary data.
