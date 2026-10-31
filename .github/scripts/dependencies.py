"""Pinned sibling checkout contract shared by CI and future app builders."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def configuration(root=ROOT):
    data = json.loads((root / ".github/dependencies.json").read_text())
    if set(data) != {"xgpui", "gpui-luma"}:
        raise ValueError("Expected exactly xgpui and gpui-luma dependency pins")
    for name, pin in data.items():
        if pin["repository"] != f"gpui-luma-dev/{name}":
            raise ValueError(f"Unexpected repository for {name}")
        if not re.fullmatch(r"[0-9a-f]{40}", pin["revision"]):
            raise ValueError(f"{name} must be pinned to a complete commit SHA")
    return data


def fingerprint(root=ROOT):
    digest = hashlib.sha256()
    for name in [".github/dependencies.json", "Cargo.toml",
                 "Cargo.lock", "rust-toolchain.toml"]:
        digest.update(name.encode())
        digest.update((root / name).read_bytes())
    return digest.hexdigest()


def git(path, *args):
    return subprocess.check_output(["git", "-C", str(path), *args], text=True).strip()


def prepare(root=ROOT):
    pins = configuration(root)
    # This command is intentionally for fresh CI checkouts, not dirty developer trees.
    for name, pin in pins.items():
        checkout = root.parent / name
        actual = git(checkout, "rev-parse", "HEAD")
        if actual != pin["revision"]:
            raise ValueError(f"{name}: expected {pin['revision']}, found {actual}")
        if git(checkout, "status", "--porcelain", "--untracked-files=normal"):
            raise ValueError(f"Unexpected dirty dependency checkout: {checkout}")
        print(f"{name}: {pin['repository']} @ {actual}", flush=True)


def align_luma(root=ROOT):
    """Use the pinned sibling xgpui with Luma's older published dependency declarations."""
    manifest = root.parent / "gpui-luma/Cargo.toml"
    text = manifest.read_text()
    replacements = {
        'gpui = { package = "xgpui", version = "=1.0.0" }':
            'gpui = { package = "xgpui", path = "../xgpui/gpui/crates/gpui" }',
        'package = "xgpui-platform",\n  version = "=1.0.0",':
            'package = "xgpui-platform",\n  path = "../xgpui/gpui/crates/gpui-platform",',
    }
    for old, new in replacements.items():
        if text.count(old) != 1:
            raise ValueError("Pinned Luma dependency declaration changed unexpectedly")
        text = text.replace(old, new)
    manifest.write_text(text)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["outputs", "prepare"])
    args = parser.parse_args()
    if args.command == "prepare":
        prepare()
        align_luma()
        return
    values = {f"{name}-{key}": value for name, pin in configuration().items()
              for key, value in pin.items()}
    values["cache-key"] = fingerprint()
    text = "".join(f"{key}={value}\n" for key, value in values.items())
    if output := os.environ.get("GITHUB_OUTPUT"):
        with open(output, "a", encoding="utf-8") as file:
            file.write(text)
    print(text, end="")


if __name__ == "__main__":
    main()
