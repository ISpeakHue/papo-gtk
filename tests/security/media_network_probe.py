"""Regression: a local media playlist must not trigger network requests.

Requires Python GI and GStreamer, including adaptive-streaming plugins. All HTTP
traffic uses an ephemeral loopback fixture, without accounts or credentials.
The direct helper must reject the playlist even outside its mandatory sandbox.
"""
import http.server
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading


def main():
    root = Path(__file__).resolve().parents[2]
    fixture = (root / "tests/fixtures/chat-h264.mp4").read_bytes()
    seen = []

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            seen.append(self.path)
            self.send_response(200)
            self.send_header("Content-Type", "video/mp4")
            self.send_header("Content-Length", str(len(fixture)))
            self.end_headers()
            try:
                self.wfile.write(fixture)
            except (BrokenPipeError, ConnectionResetError):
                pass

        def log_message(self, *args):
            pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    # Keep this reproduction local even on desktops with a configured proxy.
    env = {
        key: value for key, value in os.environ.items()
        if key.lower() not in ("http_proxy", "https_proxy", "all_proxy")
    }
    env.update(NO_PROXY="*", no_proxy="*", GST_DEBUG="0")
    try:
        with tempfile.TemporaryDirectory(prefix="papo-security-probe-") as folder:
            playlist = Path(folder) / "media"
            playlist.write_text(
                "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n"
                "#EXT-X-MEDIA-SEQUENCE:0\n#EXTINF:2,\n"
                f"http://127.0.0.1:{server.server_port}/synthetic-segment.ts\n"
                "#EXT-X-ENDLIST\n"
            )
            try:
                result = subprocess.run(
                    ["/usr/bin/python3", "-u", str(root / "src/media/poster.py"), str(playlist)],
                    capture_output=True, timeout=12, env=env,
                )
                outcome = {"helper_exit": result.returncode}
            except subprocess.TimeoutExpired:
                outcome = {"timed_out": True}
            outcome.update(network_requests=seen, request_count=len(seen))
            print(json.dumps(outcome))
            assert outcome.get("helper_exit", 0) != 0, "playlist must be rejected"
            assert not seen, "local media must not make HTTP requests"
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


if __name__ == "__main__":
    main()
