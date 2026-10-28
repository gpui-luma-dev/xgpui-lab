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
        (self.root / ".github/patches").mkdir(parents=True)
        self.pins = {}
        for name in ["xgpui", "gpui-luma"]:
            path = self.root.parent / name
            path.mkdir()
            self.run_git(path, "init", "-q")
            self.run_git(path, "config", "user.name", "CI fixture")
            self.run_git(path, "config", "user.email", "fixture@example.invalid")
            # Exercise git apply under Windows-style checkout line endings too.
            self.run_git(path, "config", "core.autocrlf", "true")
            (path / "Cargo.toml").write_text('dependency = "old"\n')
            self.run_git(path, "add", "Cargo.toml")
            self.run_git(path, "commit", "-qm", "fixture")
            self.run_git(path, "checkout-index", "--force", "--all")
            self.pins[name] = {"repository": f"gpui-luma-dev/{name}",
                               "revision": dependencies.git(path, "rev-parse", "HEAD")}
        self.save_pins()
        (self.root / ".github/patches/gpui-luma-xgpui.patch").write_text(
            '--- a/Cargo.toml\n+++ b/Cargo.toml\n@@ -1 +1 @@\n-dependency = "old"\n+dependency = "new"\n')
        for name in ["Cargo.lock", "rust-toolchain.toml"]:
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

    def test_rejects_wrong_commit_before_patching(self):
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

    def test_applies_bridge_to_clean_checkouts(self):
        with contextlib.redirect_stdout(io.StringIO()):
            dependencies.prepare(self.root)
        self.assertEqual((self.root.parent / "gpui-luma/Cargo.toml").read_text(), 'dependency = "new"\n')
        self.assertEqual(dependencies.git(self.root.parent / "xgpui", "status", "--porcelain"), "")

    def test_cache_identity_changes_with_patch_or_lock(self):
        initial = dependencies.fingerprint(self.root)
        with (self.root / "Cargo.lock").open("a") as file:
            file.write("new dependency\n")
        changed = dependencies.fingerprint(self.root)
        self.assertNotEqual(initial, changed)
        with (self.root / ".github/patches/gpui-luma-xgpui.patch").open("a") as file:
            file.write("\n")
        self.assertNotEqual(changed, dependencies.fingerprint(self.root))


if __name__ == "__main__":
    unittest.main()
