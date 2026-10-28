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

# Run the development checks.
check: fmt-check clippy test
