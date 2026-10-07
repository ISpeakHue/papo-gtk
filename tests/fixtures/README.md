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
