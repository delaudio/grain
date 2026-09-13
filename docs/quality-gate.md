# Quality gate

Run `bash scripts/quality.sh` from any directory. The same command runs in CI
on Ubuntu 24.04 and macOS 15 using Rust 1.94.0, rustfmt and Clippy. The package
declares Rust 1.94 as its minimum supported version; support for older toolchains
is not claimed. Keep the package, toolchain file and workflow pin aligned.

The gate checks formatting and Clippy without blanket warning suppression,
generates a deterministic synthetic WAV, then runs unit/binary tests, integration
tests and doctests with locked dependencies. Tests run serially in an isolated
temporary copy. The gate does not format or rewrite the working tree.

Only source inputs are copied. The user's `.grain`, `.env`, home/configuration
directories and provider keys are excluded. Temporary files and test state are
removed on exit; Cargo and Rustup caches remain available for builds. Provider
execution is forced to the deterministic mock with `GRAIN_OFFLINE=1`.
`GRAIN_AI_MOCK=1` is a compatibility alias for the same execution override.

Linux requires a C compiler, pkg-config, ALSA development headers and OpenSSL
development headers; CI installs these explicitly. macOS requires the Xcode
Command Line Tools. QuickJS is embedded and built from the locked Rust
dependency, so Node is no longer required for sketch evaluation. No physical
audio device or live provider account is required for the automated tests.

The fixture generator is opt-in:

```sh
cargo run --locked --features internal-test-fixture --bin gen_fixture
```

Normal `cargo install --path . --locked` installs only `grain`. The CLI uses the
library's modules rather than compiling another copy of their unit tests.

This workflow is a gate definition, not evidence of a green run. The first
release remains blocked until the working tree passes it, the remote macOS and
Linux jobs pass, and the remaining release/PTY/graphics acceptance criteria are
verified. Workflow permissions are read-only; this gate publishes no release,
package, Homebrew formula or other artifact.
