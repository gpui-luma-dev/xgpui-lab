import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest
import zipfile

import package_windows


class WindowsPackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.target = "x86_64-pc-windows-msvc"
        self.exe = self.root / "target" / self.target / "release/xgpui-lab.exe"
        self.exe.parent.mkdir(parents=True)
        header = bytearray(128)
        header[:2] = b"MZ"
        struct.pack_into("<I", header, 60, 80)
        header[80:84] = b"PE\0\0"
        struct.pack_into("<H", header, 84, 0x8664)
        self.exe.write_bytes(header)
        for name in ["LICENSE", "LICENSE-APACHE", "NOTICE", "licenses/fonts/font.txt"]:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("license fixture\n")
        runtime = self.root / "artifacts/build"
        runtime.mkdir(parents=True)
        (runtime / "runtime.json").write_text(json.dumps({"crt": "static", "imported_dlls": ["KERNEL32.dll"]}))
        (runtime / "dll-dependencies.txt").write_text("KERNEL32.dll\n")

    def test_archive_contains_notices_provenance_and_matching_checksums(self):
        archive = package_windows.package(self.root, self.target, {"app_commit": "a" * 40})
        with zipfile.ZipFile(archive) as bundle:
            self.assertIsNone(bundle.testzip())
            for name in ["xgpui-lab.exe", "LICENSE", "LICENSE-APACHE", "NOTICE", "licenses/fonts/font.txt",
                         "README.txt", "build-info.json", "dll-dependencies.txt", "SHA256SUMS"]:
                self.assertIn(name, bundle.namelist())
            info = json.loads(bundle.read("build-info.json"))
            self.assertEqual(info["app_commit"], "a" * 40)
            self.assertEqual(info["target"], self.target)
            exe_hash = hashlib.sha256(bundle.read("xgpui-lab.exe")).hexdigest()
            self.assertEqual(info["executable_sha256"], exe_hash)
            self.assertIn(exe_hash, bundle.read("SHA256SUMS").decode())
        self.assertIn(package_windows.sha256(archive), archive.with_suffix(".zip.sha256").read_text())

    def test_wrong_architecture_does_not_produce_an_archive(self):
        header = bytearray(self.exe.read_bytes())
        struct.pack_into("<H", header, 84, 0xAA64)
        self.exe.write_bytes(header)
        with self.assertRaisesRegex(ValueError, "architecture"):
            package_windows.package(self.root, self.target, {})
        self.assertFalse((self.root / "dist").exists())

    def test_missing_executable_does_not_produce_an_archive(self):
        self.exe.unlink()
        with self.assertRaises(FileNotFoundError):
            package_windows.package(self.root, self.target, {})
        self.assertFalse((self.root / "dist").exists())

    def test_missing_runtime_inspection_does_not_produce_an_archive(self):
        (self.root / "artifacts/build/runtime.json").write_text('{}')
        with self.assertRaisesRegex(ValueError, "runtime inspection"):
            package_windows.package(self.root, self.target, {})
        self.assertFalse((self.root / "dist").exists())


if __name__ == "__main__":
    unittest.main()
