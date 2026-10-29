import contextlib
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import dependencies


class DependencyContractTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "app"
        (self.root / ".github").mkdir(parents=True)
        self.pins = {}
        for name in ["xgpui", "gpui-luma"]:
            path = self.root.parent / name
            path.mkdir()
            self.run_git(path, "init", "-q")
            self.run_git(path, "config", "user.name", "CI fixture")
            self.run_git(path, "config", "user.email", "fixture@example.invalid")
            # Verify clean dependency checkouts with Windows-style line endings too.
            self.run_git(path, "config", "core.autocrlf", "true")
            (path / "Cargo.toml").write_text('dependency = "old"\n')
            self.run_git(path, "add", "Cargo.toml")
            self.run_git(path, "commit", "-qm", "fixture")
            self.run_git(path, "checkout-index", "--force", "--all")
            self.pins[name] = {"repository": f"gpui-luma-dev/{name}",
                               "revision": dependencies.git(path, "rev-parse", "HEAD")}
        self.save_pins()
        for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]:
            (self.root / name).write_text("fixture\n")

    def run_git(self, path, *args):
        subprocess.run(["git", "-C", str(path), *args], check=True,
                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    def save_pins(self):
        (self.root / ".github/dependencies.json").write_text(json.dumps(self.pins))

    def test_rejects_moving_branch_refs(self):
        self.pins["xgpui"]["revision"] = "main"
        self.save_pins()
        with self.assertRaisesRegex(ValueError, "complete commit SHA"):
            dependencies.configuration(self.root)

    def test_rejects_wrong_commit(self):
        self.pins["xgpui"]["revision"] = "a" * 40
        self.save_pins()
        with self.assertRaisesRegex(ValueError, "expected"):
            dependencies.prepare(self.root)

    def test_preserves_dirty_checkouts(self):
        path = self.root.parent / "gpui-luma/Cargo.toml"
        path.write_text("developer edits\n")
        with contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(ValueError, "dirty dependency"):
                dependencies.prepare(self.root)
        self.assertEqual(path.read_text(), "developer edits\n")

    def test_verifies_clean_checkouts_without_modifying_them(self):
        with contextlib.redirect_stdout(io.StringIO()):
            dependencies.prepare(self.root)
        self.assertEqual(dependencies.git(self.root.parent / "gpui-luma", "status", "--porcelain"), "")
        self.assertEqual(dependencies.git(self.root.parent / "xgpui", "status", "--porcelain"), "")

    def test_cache_identity_changes_with_manifest_or_lock(self):
        initial = dependencies.fingerprint(self.root)
        with (self.root / "Cargo.lock").open("a") as file:
            file.write("new dependency\n")
        changed = dependencies.fingerprint(self.root)
        self.assertNotEqual(initial, changed)
        with (self.root / "Cargo.toml").open("a") as file:
            file.write("\n")
        self.assertNotEqual(changed, dependencies.fingerprint(self.root))


if __name__ == "__main__":
    unittest.main()
