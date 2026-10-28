import contextlib
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import check


class CheckRunnerTests(unittest.TestCase):
    def test_logs_and_preserves_failed_process_exit_code(self):
        previous = Path.cwd()
        with tempfile.TemporaryDirectory() as directory:
            try:
                os.chdir(directory)
                command = [sys.executable, "-c", "import sys; print('diagnostic'); sys.exit(7)"]
                with patch.dict(check.COMMANDS, {"test": command}), \
                     patch.object(sys, "argv", ["check.py", "test"]), \
                     contextlib.redirect_stdout(io.StringIO()):
                    with self.assertRaises(SystemExit) as result:
                        check.main()
                self.assertEqual(result.exception.code, 7)
                self.assertIn("diagnostic", Path("artifacts/ci/test.log").read_text())
                report = json.loads(Path("artifacts/ci/test.json").read_text())
                self.assertEqual(report["exit_code"], 7)
                self.assertEqual(report["command"], command)
            finally:
                os.chdir(previous)

    def test_failed_spawn_is_recorded_as_failure(self):
        previous = Path.cwd()
        with tempfile.TemporaryDirectory() as directory:
            try:
                os.chdir(directory)
                with patch.dict(check.COMMANDS, {"test": [str(Path(directory) / "missing-executable")]}), \
                     patch.object(sys, "argv", ["check.py", "test"]), \
                     contextlib.redirect_stdout(io.StringIO()), \
                     contextlib.redirect_stderr(io.StringIO()):
                    with self.assertRaises(SystemExit) as result:
                        check.main()
                self.assertEqual(result.exception.code, 1)
                self.assertEqual(json.loads(Path("artifacts/ci/test.json").read_text())["exit_code"], 1)
            finally:
                os.chdir(previous)


if __name__ == "__main__":
    unittest.main()
