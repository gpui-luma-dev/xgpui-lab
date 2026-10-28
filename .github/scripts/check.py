"""Run a named CI check, preserving its exit status and a complete log."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import time

COMMANDS = {
    "format": ["cargo", "fmt", "--package", "xgpui-lab", "--", "--check"],
    "compile": ["cargo", "check", "--locked", "--all-targets", "--features", "test-support"],
    "clippy": ["cargo", "clippy", "--locked", "--all-targets", "--features", "test-support", "--", "-D", "warnings"],
    "test": ["cargo", "test", "--locked", "--all-targets", "--features", "test-support"],
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("check", choices=COMMANDS)
    args = parser.parse_args()
    folder = Path("artifacts/ci")
    folder.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    command = COMMANDS[args.check]
    code = 1
    with (folder / f"{args.check}.log").open("w", encoding="utf-8") as log:
        print(f"Running: {' '.join(command)}", flush=True)
        try:
            with subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                  text=True, encoding="utf-8", errors="replace") as child:
                for line in child.stdout:
                    sys.stdout.write(line)
                    log.write(line)
                code = child.wait()
        except OSError as error:
            log.write(str(error))
            print(error, file=sys.stderr)
    (folder / f"{args.check}.json").write_text(json.dumps({
        "command": command, "exit_code": code, "elapsed_seconds": time.monotonic() - started,
    }, indent=2) + "\n", encoding="utf-8")
    raise SystemExit(code)


if __name__ == "__main__":
    main()
