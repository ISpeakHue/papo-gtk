# Implementation-plan steps 1–3 validation

Three fresh-process release runs completed on Fedora 44 x86_64, glibc 2.43,
GTK 4.22.5, private Xvnc displays and Cairo rendering. The exact samples, source
fingerprint, dirty working-tree status and test-binary SHA-256 are retained in
[baseline.json](baseline.json). They describe the implementation working tree on
top of `b6b880e`, not the previously published 1.1.1 binaries.

```sh
python3 scripts/performance-baseline.py --runs 3 \
  --output target/performance/steps123-baseline.json
```

| Scenario | Samples | Median | Range |
| --- | ---: | ---: | ---: |
| Test entry to painted login UI | 3 | 303.4 ms | 302.6–305.4 ms |
| Cold channel text, 30 messages | 3 | 145.4 ms | 145.2–204.1 ms |
| Cold channel and eight images ready | 3 | 911.4 ms | 903.0–913.6 ms |
| Warm channel return | 9 | 61.7 ms | 60.7–68.0 ms |

Every run made eight cold image requests and **zero additional image requests**
on three warm returns. Sustained synthetic wheel input produced 187–188 sampled
frames; GTK CPU paint p95 was 3.14–3.39 ms, with no sampled work above 16.667 ms.
This does not measure native event latency or compositor/GPU presentation.

History samples cover 100/1,000/5,000 messages. The media-residency scenario warms
eight channels with thirty 640×360 previews and 1 MiB compressed buffers per
channel. At the eighth channel it retained 95.8 MiB of decoded cache data and
64 MiB of compressed data. Process RSS at that checkpoint was 530.3–532.6 MiB,
after the preceding history and scrolling workloads in the same process. RSS
includes GTK, rendering, allocator high-water marks and other application state;
it is not the sum of cache counters and is not comparable to an isolated cache
fixture or a user's live session. Memory is sampled at checkpoints, not
continuously, so these values are not a peak-memory guarantee.

Startup timing excludes the process loader and authentication. Cold requests use
synthetic PNGs and a loopback backend, including normal request pacing. Three
runs are initial diagnostic samples, not statistical proof of an improvement.
No timing regression thresholds have been introduced before plan step 17.

## Functional verification

- `cargo check --locked --all-targets`: passed without compiler warnings.
- `cargo test --locked --release -- --test-threads=1`: 322 passed, six explicit
  display/native integration cases ignored by the regular suite.
- Python implementation tools: six passed; incomplete baselines and changed or
  missing reviewed backend source are rejected.
- Actionlint: both workflow files passed.
- Host private-display GTK cases passed: `account-scope`, `recovery`,
  `account-settings`, `voice-chat`, `media-cache`, `channel-cache`.
- `notification-filters` initially timed out in the native wheel assertion on the
  bare host Xvnc display. Its unchanged rerun in the existing Fedora CI container
  with private Xvfb/Openbox passed. The first failure remains in
  `target/steps123-gui-notification-filters.log`; the passing rerun is in
  `target/steps123-gui-notification-filters-wm.log`. This is not a production
  scrolling fix or a confirmed root-cause diagnosis.
- The backend source checker matches the reviewed local commit
  `eab626662bc4bfeb4ce1cd3165b01eccb4c2c5a5`; backend files were not changed.

Linux and Windows workflows now retain one debug baseline run and exercise the
account-scope GUI case. They have not been pushed or run remotely for this change.
Native Windows performance/device acceptance is still pending; Linux results do
not establish Windows timings. macOS still provides compile/link groundwork only.
