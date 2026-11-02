#!/usr/bin/env python3
"""Run the real native Gamut navigation regression with a process-level timeout."""
import argparse
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--app", type=Path, default=Path("target/debug/xgpui-lab"))
parser.add_argument("--output", type=Path, default=Path("artifacts/gamut-navigation"))
parser.add_argument("--timeout", type=float, default=60)
args = parser.parse_args()
try:
    result = subprocess.run(
        [str(args.app.resolve()), "--gamut-benchmark", "navigation", "--output", str(args.output.resolve())],
        timeout=args.timeout,
        check=False,
    )
except subprocess.TimeoutExpired:
    print("FAIL: native Gamut navigation timed out (possible renderer deadlock)", file=sys.stderr)
    sys.exit(1)
sys.exit(result.returncode)
