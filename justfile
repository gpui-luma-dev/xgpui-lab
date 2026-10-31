set shell := ["bash", "-cu"]
set windows-shell := ["bash", "-cu"]
set positional-arguments

# Fast iteration for the standalone color lab; override with profile=release.
profile := "dev"

# List available recipes.
default:
    @just --list

# Run XGPUI Lab with release rendering performance.
run:
    cargo run --locked --release

# Build the release executable.
build:
    cargo build --locked --release

# Format application source.
fmt:
    cargo fmt --package xgpui-lab

# Check formatting without changing files.
fmt-check:
    cargo fmt --package xgpui-lab -- --check

# Check all targets, including headless test support.
clippy:
    cargo clippy --locked --all-targets --features test-support -- -D warnings

# Run headless tests.
test:
    cargo test --locked --all-targets --features test-support

# Run native atlas, Gamut, Surface, ownership, and lifecycle checks; retain reports and captures.
regression-tests:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p artifacts/regression-tests
    output=$(mktemp -d "$PWD/artifacts/regression-tests/$(date -u +%Y%m%dT%H%M%SZ)-XXXXXX")
    printf 'Regression results: %s\n' "$output"
    cargo build --locked 2>&1 | tee "$output/build.log"
    status=0
    ./target/debug/xgpui-lab --media-benchmark atlas --output "$output/atlas" 2>&1 | tee "$output/atlas.log" || status=1
    ./target/debug/xgpui-lab --gamut-benchmark texture --output "$output/gamut" 2>&1 | tee "$output/gamut.log" || status=1
    # The video workload includes the independent native PaintSurface assertions.
    ./target/debug/xgpui-lab --media-benchmark video --output "$output/native-surface" 2>&1 | tee "$output/native-surface.log" || status=1
    ./target/debug/xgpui-lab --media-benchmark lifecycle --output "$output/lifecycle" 2>&1 | tee "$output/lifecycle.log" || status=1
    printf 'Regression results: %s\n' "$output"
    exit "$status"

# Run the development checks.
check: fmt-check clippy test

# Render the reviewed objc2/Metal coverage matrix and validate its source references.
coverage-report:
    python3 .github/scripts/render_objc2_coverage.py

# Build a fresh registry-only consumer without local patches/configuration.
consumer-audit:
    python3 .github/scripts/audit_consumer.py

# CIE xy/u′v′, color storage inspector and native-wide/sRGB comparisons.
chromaticity *args:
    cargo run --profile "{{ profile }}" --bin chromaticity-lab -- "$@"

# Numerical and CIE dataset checks for the standalone color lab.
chromaticity-test *args:
    cargo test --bin chromaticity-lab -- "$@"
