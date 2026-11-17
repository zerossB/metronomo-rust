# Contributing to Metronomo

Bug reports, documentation improvements, and focused pull requests are welcome.
Use [private vulnerability reporting](SECURITY.md) for security-sensitive reports.

## Development

Follow the [development guide](docs/development.md) to install the pinned Rust
toolchain and native dependencies, then run:

```sh
cargo fetch --locked
./scripts/check.sh
```

The checks cover formatting, compilation, Rust and Python tests, Clippy, and the
desktop entry. Device and native-window tests are opt-in. Audio changes should
describe the hardware, backend, and conditions used to validate them.

## Pull requests

1. Create a branch or fork from `main`.
2. Make a focused change with relevant validation and documentation.
3. Open a pull request explaining the problem, resulting behavior, and test results.
4. Resolve review conversations and wait for the required checks before merging.

`main` requires a pull request and successful checks, and blocks deletion and
force pushes. With a single maintainer, a second person's approval is not
mandatory. External fork workflows require a maintainer to approve execution.

Use existing state ownership and module boundaries. Keep credentials and personal
settings out of commits, logs, and screenshots. Workflow, packaging, and release
changes receive particular scrutiny because they control distributed binaries.
Update license supplements when changing locked dependencies.

## Releases

Only the maintainer creates version tags. Follow the [release guide](docs/release.md)
to update Cargo, Debian metadata, and release notes together. Published release
assets are immutable; corrections require a new version.
