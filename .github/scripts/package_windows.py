"""Package a checked Windows executable with provenance, notices, and hashes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import tomllib
import zipfile

import dependencies

TARGETS = {
    "x86_64-pc-windows-msvc": ("x64", 0x8664),
    "aarch64-pc-windows-msvc": ("arm64", 0xAA64),
}

INSTRUCTIONS = """XGPUI Lab — Windows portable preview

Extract this entire archive, then run xgpui-lab.exe. Use the archive matching
your machine's architecture. Build/version information is in build-info.json.
This is an unsigned executable archive, not an installer or automatic updater.

Runtime: a Windows desktop with supported Direct3D graphics and current drivers
is required. The CRT is statically linked; DLL import inspection is included.
Only Windows system DLLs are expected. The loader/version check is not a GPU
or interactive rendering certification; test the app on your actual hardware.

The generated IOSurface/NV12 video source is currently macOS-only. P3 output
depends on the platform/backend/display; use the app's reported presentation
status and labeled fallback rather than assuming physical P3 output.

Original source-worker Validation pages are unavailable in this portable
archive: no xgpui worker source, Rust toolchain, or external SVG corpus is
bundled. Use a source checkout for those workloads. Interactive extension labs,
built-in CPU benchmark pages, and standalone text-atlas diagnostic windows
remain part of the app. No developer checkout is needed to launch the GUI.

Source: https://github.com/gpui-luma-dev/xgpui-lab
Licenses and source attribution: LICENSE, LICENSE-APACHE, NOTICE, licenses/.
"""


def machine(path):
    with path.open("rb") as file:
        header = file.read(64)
        if len(header) != 64 or header[:2] != b"MZ":
            raise ValueError("Not a Windows PE executable")
        file.seek(struct.unpack_from("<I", header, 60)[0])
        coff = file.read(6)
        if len(coff) != 6 or coff[:4] != b"PE\0\0":
            raise ValueError("Invalid Windows PE header")
        return struct.unpack_from("<H", coff, 4)[0]


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def package(root, target, metadata):
    arch, expected_machine = TARGETS[target]
    executable = root / "target" / target / "release/xgpui-lab.exe"
    if machine(executable) != expected_machine:
        raise ValueError(f"Executable architecture does not match {target}")
    runtime_folder = root / "artifacts/build"
    runtime = json.loads((runtime_folder / "runtime.json").read_text(encoding="utf-8-sig"))
    if runtime.get("crt") != "static" or not runtime.get("imported_dlls"):
        raise ValueError("A verified static-CRT runtime inspection is required")
    metadata = dict(metadata, target=target, features=["portable-app"], runtime=runtime,
                    executable_sha256=sha256(executable))
    files = {
        "xgpui-lab.exe": executable,
        "LICENSE": root / "LICENSE",
        "LICENSE-APACHE": root / "LICENSE-APACHE",
        "NOTICE": root / "NOTICE",
        "dll-dependencies.txt": runtime_folder / "dll-dependencies.txt",
    }
    for path in sorted((root / "licenses").rglob("*")):
        if path.is_file():
            files[path.relative_to(root).as_posix()] = path
    # Read/validate everything before creating an archive, including notices.
    for path in files.values():
        if not path.is_file() or path.stat().st_size == 0:
            raise ValueError(f"Missing or empty archive input: {path}")
    output = root / "dist"
    output.mkdir(exist_ok=True)
    archive = output / f"xgpui-lab-windows-{arch}.zip"
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
        for name, path in files.items():
            bundle.write(path, name)
        bundle.writestr("README.txt", INSTRUCTIONS)
        bundle.writestr("build-info.json", json.dumps(metadata, indent=2) + "\n")
        bundle.writestr("SHA256SUMS", f"{sha256(executable)}  xgpui-lab.exe\n")
    checksum = output / f"{archive.name}.sha256"
    checksum.write_text(f"{sha256(archive)}  {archive.name}\n", encoding="utf-8")
    (output / f"{archive.stem}-build-info.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    return archive


def smoke(archive, target):
    # Launch only the extracted copy, outside the source tree. This verifies the
    # OS loader/CRT and selected architecture, without claiming a GPU test.
    with tempfile.TemporaryDirectory(prefix="xgpui-portable-smoke-") as folder:
        with zipfile.ZipFile(archive) as bundle:
            bundle.extractall(folder)
        result = subprocess.run([str(Path(folder) / "xgpui-lab.exe"), "--version"], cwd=folder,
                                text=True, capture_output=True, timeout=30, check=True)
        expected = "x86_64-windows" if TARGETS[target][0] == "x64" else "aarch64-windows"
        if expected not in result.stdout or "portable" not in result.stdout:
            raise ValueError(f"Unexpected portable executable version: {result.stdout!r}")
        print(result.stdout, end="")
        return {"scope": "extracted executable OS loader and --version only", "result": "passed",
                "stdout": result.stdout, "interactive_gpu_test": "not performed"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=TARGETS)
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    root = dependencies.ROOT
    pins = dependencies.configuration(root)
    package_info = tomllib.loads((root / "Cargo.toml").read_text())["package"]
    metadata = {
        "app": package_info["name"], "version": package_info["version"],
        "app_commit": dependencies.git(root, "rev-parse", "HEAD"),
        "dependency_pins": pins, "dependency_config_hash": dependencies.fingerprint(root),
        "rustc": subprocess.check_output(["rustc", "-Vv"], text=True),
        "cargo": subprocess.check_output(["cargo", "-V"], text=True).strip(),
        "run_url": f"https://github.com/{os.environ.get('GITHUB_REPOSITORY', '')}/actions/runs/{os.environ.get('GITHUB_RUN_ID', '')}",
        "run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT", "local"),
        "build_flags": ["--locked", "--release", "--package", "xgpui-lab", "--features", "portable-app", "--target", args.target],
        "rustflags": os.environ.get("RUSTFLAGS", ""),
        "signing": "unsigned", "source_workers": "not bundled; disabled in portable app",
    }
    archive = package(root, args.target, metadata)
    if args.smoke:
        report = smoke(archive, args.target)
        (archive.parent / f"{archive.stem}-smoke.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Packaged {archive.name}: {sha256(archive)}")


if __name__ == "__main__":
    main()
