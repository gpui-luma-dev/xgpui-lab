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
    for name in [".github/dependencies.json", ".github/patches/gpui-luma-xgpui.patch",
                 "Cargo.lock", "rust-toolchain.toml"]:
        digest.update(name.encode())
        digest.update((root / name).read_bytes())
    return digest.hexdigest()


def git(path, *args):
    return subprocess.check_output(["git", "-C", str(path), *args], text=True).strip()


def prepare(root=ROOT):
    pins = configuration(root)
    patch = root / ".github/patches/gpui-luma-xgpui.patch"
    # This command is intentionally for fresh CI checkouts, not dirty developer trees.
    for name, pin in pins.items():
        checkout = root.parent / name
        actual = git(checkout, "rev-parse", "HEAD")
        if actual != pin["revision"]:
            raise ValueError(f"{name}: expected {pin['revision']}, found {actual}")
        if git(checkout, "status", "--porcelain", "--untracked-files=normal"):
            raise ValueError(f"Refusing to modify a dirty dependency checkout: {checkout}")
        print(f"{name}: {pin['repository']} @ {actual}", flush=True)
    luma = root.parent / "gpui-luma"
    subprocess.run(["git", "-C", str(luma), "apply", "--check", str(patch)], check=True)
    subprocess.run(["git", "-C", str(luma), "apply", str(patch)], check=True)
    print(f"Applied tracked Luma dependency bridge: {patch.name}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["outputs", "prepare"])
    args = parser.parse_args()
    if args.command == "prepare":
        prepare()
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
