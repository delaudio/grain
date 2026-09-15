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

Windows audio backends are owned by one process-lifetime worker. CPAL's cached
WASAPI enumerator must not outlive its original COM apartment; see
[RustAudio/cpal#1302](https://github.com/RustAudio/cpal/issues/1302). Public player
handles send operations to that owner and release individual backends on it.
The idle owner remains parked until process termination, including between tests.
Queued requests expire after five seconds and cannot mutate playback afterwards.
Once a native operation starts, its actual result is awaited, preserving the
previous synchronous audio contract instead of reporting a false timeout. A stuck
native call therefore can still block its caller; audio interruption is not
guaranteed. Native lifecycle regressions run before the
full application suite, without disabling hardware initialization.
Unwinding operation panics retire only the affected backend; constructor and
destructor unwinding is contained without terminating the shared owner. Zero-fps
seeks are rejected before dispatch. This does not catch native access violations
or aborting panics, which remain fatal and must be treated as CI failures.

Adding this job is not evidence that it passes. The first native run must succeed
before claiming the CLI implementation is validated on Windows. Issue #51 also
requires terminal/audio/editor/browser acceptance, release artifacts and clean
installation smoke tests. Issue #14 still requires background generation,
cancellation integration and stale-result rejection. Both remain release blockers.
