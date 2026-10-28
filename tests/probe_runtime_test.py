"""EN-US: Native paths without Windows hardware. PT-BR: Caminhos nativos sem hardware Windows."""
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import probe_runtime as runtime


class RuntimeTests(unittest.TestCase):
    def test_explicit_path_with_spaces(self):
        with tempfile.TemporaryDirectory(prefix="papo calls ") as directory:
            path = Path(directory) / "voice-sfu.exe"
            path.touch()
            self.assertEqual(runtime.sfu_binary(["--relay", "--sfu", str(path)]), str(path.resolve()))

    def test_missing_fixture_has_actionable_error(self):
        with self.assertRaisesRegex(FileNotFoundError, "--sfu PATH"):
            runtime.sfu_binary(["--sfu", "/nonexistent/papo-test-sfu"])

    def test_interpreter_and_windows_console_policy(self):
        with patch.dict(runtime.os.environ, {}, clear=True):
            self.assertEqual(runtime.python(), sys.executable)
        with patch.object(runtime.sys, "platform", "win32"), patch.object(runtime.subprocess, "CREATE_NO_WINDOW", 0x08000000, create=True):
            self.assertEqual(runtime.spawn_options(), {"creationflags": 0x08000000})


if __name__ == "__main__":
    unittest.main()
