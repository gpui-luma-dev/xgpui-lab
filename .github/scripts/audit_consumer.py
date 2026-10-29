"""Build an isolated registry-only consumer and record its legacy dependency paths."""
import argparse
from collections import deque
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import uuid

MANIFEST = '''[package]
name = "xgpui-clean-consumer"
version = "0.0.0"
edition = "2024"
publish = false
[workspace]
[dependencies]
gpui = { package = "xgpui", version = "=1.0.0" }
gpui_platform = { package = "xgpui-platform", version = "=1.0.0", features = ["font-kit"] }
xgpui-color = "=0.2.0"
xgpui-gamut = { version = "=0.2.0", features = ["instanced"] }
xgpui-media = "=0.2.0"
xgpui-parley = { version = "=0.2.0", features = ["system-fonts"] }
xgpui-shader = { version = "=0.2.0", features = ["portable-shaders"] }
xgpui-svg = "=0.2.0"
'''
LEGACY = {"metal", "objc", "block", "cocoa", "cocoa-foundation", "objc_exception", "objc_id", "core-foundation", "core-foundation-sys", "core-graphics", "core-text"}


def registry_violations(metadata):
    consumer = metadata["resolve"]["root"]
    return [p["id"] for p in metadata["packages"] if p["id"] != consumer and not (p["source"] or "").startswith("registry+")]


def audit(output):
    output.mkdir(parents=True, exist_ok=False)
    report = {"scope": "Published xgpui 1.0.0 and six extensions 0.2.0; not unpublished checkout changes", "outcome": "failed", "registry_only": False, "patches": [], "fresh_target": True, "commands": []}
    with tempfile.TemporaryDirectory(prefix="xgpui-clean-consumer-") as tmp:
        root = Path(tmp)
        project = root / "consumer"
        (project / "src").mkdir(parents=True)
        (project / "Cargo.toml").write_text(MANIFEST)
        (project / "src/main.rs").write_text('fn main() { let _application = gpui_platform::application; }\n')
        home = root / "cargo-home"
        home.mkdir()
        registry = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")) / "registry"
        if registry.exists():
            (home / "registry").symlink_to(registry.resolve(), target_is_directory=True)
        env = {k: v for k, v in os.environ.items() if not k.startswith("CARGO_") and k not in {"RUSTFLAGS", "RUSTDOCFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"}}
        env.update(CARGO_HOME=str(home), CARGO_TARGET_DIR=str(root / "target"), CARGO_BUILD_JOBS="4")
        inherited = [str(parent / ".cargo" / name) for parent in [project, *project.parents] for name in ["config", "config.toml"] if (parent / ".cargo" / name).exists()]
        report["inherited_cargo_configuration"] = inherited
        report["cargo_home_config_isolated"] = True
        report["registry_cache_reused"] = registry.exists()
        rust = subprocess.check_output(["rustc", "-vV"], env=env, cwd=project, text=True)
        report["rustc"] = rust
        target = next(line.split(": ", 1)[1] for line in rust.splitlines() if line.startswith("host:"))
        report["target"] = target
        shutil.copyfile(project / "Cargo.toml", output / "Cargo.toml")
        shutil.copyfile(project / "src/main.rs", output / "main.rs")

        def run(args, name):
            command = ["cargo", *args]
            report["commands"].append(command)
            result = subprocess.run(command, cwd=project, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            (output / (name + ".stdout.log")).write_text(result.stdout)
            (output / (name + ".stderr.log")).write_text(result.stderr)
            if result.returncode:
                raise RuntimeError(f"{name} exited {result.returncode}; see {name}.stderr.log")
            return result.stdout

        try:
            if inherited:
                raise RuntimeError(f"Unexpected inherited Cargo configuration: {inherited}")
            metadata = json.loads(run(["metadata", "--offline", "--format-version", "1", "--filter-platform", target], "metadata"))
            (output / "metadata.json").write_text(json.dumps(metadata, indent=2))
            packages = {p["id"]: p for p in metadata["packages"]}
            consumer = metadata["resolve"]["root"]
            bad = registry_violations(metadata)
            if bad:
                raise RuntimeError(f"Non-registry dependencies: {bad}")
            report["registry_only"] = True
            nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
            paths = {consumer: [consumer]}
            queue = deque([consumer])
            while queue:
                parent = queue.popleft()
                for dep in nodes[parent]["deps"]:
                    child = dep["pkg"]
                    if child not in paths:
                        paths[child] = paths[parent] + [child]
                        queue.append(child)
            report["legacy_dependencies"] = [{"name": p["name"], "version": p["version"], "source": p["source"], "path": [packages[i]["name"] + "@" + packages[i]["version"] for i in paths[ident]], "direct_dependents": sorted(packages[parent]["name"] + "@" + packages[parent]["version"] for parent in paths if any(dep["pkg"] == ident for dep in nodes[parent]["deps"]))} for ident, p in packages.items() if p["name"] in LEGACY and ident in paths]
            report["objc2_dependencies"] = [{"name": p["name"], "version": p["version"]} for ident, p in packages.items() if p["name"].startswith(("objc2", "block2")) and ident in paths]
            run(["build", "--locked", "--offline", "--target", target], "build")
            report["outcome"] = "passed"
        except Exception as error:
            report["error"] = str(error)
        finally:
            if (project / "Cargo.lock").exists():
                shutil.copyfile(project / "Cargo.lock", output / "Cargo.lock")
            future = root / "target/.future-incompat-report.json"
            if future.exists():
                shutil.copyfile(future, output / "future-incompat-report.json")
            (output / "report.json").write_text(json.dumps(report, indent=2))
    print(json.dumps({"output": str(output), "outcome": report["outcome"], "error": report.get("error"), "legacy": [{"name": p["name"], "version": p["version"]} for p in report.get("legacy_dependencies", [])]}), flush=True)
    return report["outcome"] == "passed"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("artifacts/consumer-audit") / (datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + uuid.uuid4().hex[:8]))
    args = parser.parse_args()
    raise SystemExit(0 if audit(args.output.resolve()) else 1)
