# Native Windows quality gate

Run `./scripts/quality.ps1` from PowerShell on Windows. The matching CI job uses
`windows-2025`, Rust 1.94.0 with rustfmt and Clippy, and the runner's native MSVC
build tools. Dependencies are resolved from Cargo.lock with `--locked`.

The gate copies only project inputs to a temporary workspace, isolates the user
profile and application directories, removes provider credential/selection
variables, and forces offline sketch generation and the half-block preview.
It preserves the Cargo/Rustup caches, generates synthetic audio, and runs format,
all-target/all-feature Clippy, unit, integration and documentation checks.
Environment variables and the working directory are restored on failure as well
as success. No real provider credentials or audio input are required.

Windows process tests exercise the native Job Object runner with Rust subprocess
fixtures rather than relying on Unix commands. The deliberately ignored
`cli_fixture_helper` integration test is a subprocess fixture, explicitly invoked
by `test_agent_cli_generator_extracts_code`; it is not omitted functional coverage.

## Release status

Adding this job is not evidence that it passes. The first native run must succeed
before claiming the CLI implementation is validated on Windows. Issue #51 also
requires terminal/audio/editor/browser acceptance, release artifacts and clean
installation smoke tests. Issue #14 still requires background generation,
cancellation integration and stale-result rejection. Both remain release blockers.
