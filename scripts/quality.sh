#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/grain-quality.XXXXXX")"
trap 'rm -rf "$work"' EXIT

# Copy only project inputs, never local .env, .grain or provider configuration.
for path in Cargo.toml Cargo.lock rust-toolchain.toml LICENSE README.md src tests examples fixtures docs scripts scenarios; do
    if [[ -e "$root/$path" ]]; then
        cp -R "$root/$path" "$work/$path"
    fi
done

# Preserve toolchain/cache locations before isolating HOME and temporary state.
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target}"
export HOME="$work/home"
export XDG_CONFIG_HOME="$work/config"
export XDG_CACHE_HOME="$work/cache"
export XDG_DATA_HOME="$work/data"
export TMPDIR="$work/tmp"
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_DATA_HOME" "$TMPDIR"
unset OPENAI_API_KEY GRAIN_AI_KEY ANTHROPIC_API_KEY CODEX_API_KEY
unset GRAIN_AI_PROVIDER GRAIN_GENERATOR_CMD OPENAI_BASE_URL
export GRAIN_OFFLINE=1
export GRAIN_PREVIEW_BACKEND=half-block

cd "$work"
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo run --locked --features internal-test-fixture --bin gen_fixture
cargo test --locked --lib --bins -- --test-threads=1
cargo test --locked --tests --features internal-test-fixture -- --test-threads=1
cargo test --locked --doc
