# Preparing a Linux release

The release targets Ubuntu 24.04 and 26.04 amd64. Build on **24.04** so the
executable does not acquire a newer glibc requirement from the development host.
The same `.deb` must pass installation and runtime checks on both versions.

## Checks and measurements

Fetch the locked Cargo dependencies once, then all scripts use Cargo offline:

```sh
cargo fetch --locked
./scripts/check.sh
./scripts/validation/audio.sh renderer
```

Renderer validation covers 480 combinations of sample rates (44.1/48/96 kHz),
buffers (64/127/256/511/1024 frames), BPM (40/120/137/240), note units, and mono/stereo.
The long run simulates thirty minutes at fractional and maximum click rates.
These tests measure sample positions, not wall-clock precision or a sound card.

In each target's real desktop/audio session:

```sh
METRONOMO_VALIDATION_DIR="$PWD/dist/evidence/ubuntu-26.04-wayland" \
  ./scripts/validation/audio.sh device
cargo test --release --locked --offline native_window_controls_smoke -- --ignored --nocapture
```

The device test plays low-volume clicks for ten minutes, changes tempo/volume/
meter, mutes, stops, and restarts. It never reads or writes personal settings.
It records CPAL callback counters and predicted playback timeline observations.
Missing devices, incomplete durations, errors, queue loss, and polling stalls
fail validation; advisory realtime-scheduling denials are reported separately.
No acoustic, rendered-frame, or input-latency claim is made by this test.

Set the evidence directory for each actual OS/backend. GPUI selects Wayland when
`WAYLAND_DISPLAY` is set. To exercise X11, use a real Xorg session, or explicitly
label an XWayland-only check when using `env -u WAYLAND_DISPLAY` inside Wayland.

## Desktop recording protocol

1. Install the candidate on a clean desktop without Rust. Record `/etc/os-release`,
   session/backend, GPU driver, output device, sample rate, display refresh, source
   fingerprint, and package checksum in a copy of `validation/desktop.template.json`.
2. Record the application window with desktop video and **output loopback**, not
   the microphone. Keep capture timestamps on one clock. Avoid other playing audio.
   Recording via the desktop portal may require selecting the window interactively.
3. Practice for ten minutes: begin at 137 BPM, change to 40 and 240, use sixteenth
   notes and twelve beats, change sounds, mute/unmute, load a saved preset, stop/
   restart, minimize/restore, and close/reopen. Confirm presets and hidden sounds
   persist. Record any glitches, freezes, or missing/duplicate clicks.
4. Measure at least twenty interactions and twenty beats spanning the run. Record
   the maximum input-to-first-changed-frame delay (≤100 ms) and maximum difference
   between click onset in loopback and first corresponding visual frame (≤50 ms).
   Include measurement tables and the capture's timestamp/refresh uncertainty;
   an observation too imprecise to establish a limit stays pending.
5. Test unavailable output and a disconnected stream on disposable audio devices
   or an isolated audio session; restore output and retry Start. Do not stop the
   user's shared audio server to manufacture a failure. Record results and the
   existing automated error/retry tests separately.
6. Reinstall/remove the package. Confirm menu integration and that preferences
   survive. Save completed desktop JSON, raw device report, measurement tables,
   and recording paths under the corresponding evidence directory.

These steps require human observation and review. Headless containers cannot
complete the desktop acceptance matrix.

## Baseline package build

The container uses debhelper and `dpkg-shlibdeps`. Vulkan, Wayland client/cursor,
font libraries and a default font are also declared because dynamically loaded
dependencies are not all visible to the ELF dependency scanner.

```sh
podman build -f packaging/linux/containers/ubuntu-24.04.Containerfile -t localhost/metronomo-builder:24.04 .
podman run --rm --userns=keep-id \
  -v "$PWD:/workspace:rw" \
  -v "$HOME/.rustup/toolchains/1.99.1-x86_64-unknown-linux-gnu:/opt/rust:ro" \
  -v "$HOME/.cargo/registry:/cargo/registry:ro" \
  -w /workspace -e CARGO_HOME=/cargo \
  -e CARGO_TARGET_DIR=/workspace/target/ubuntu-24.04 \
  -e PATH=/opt/rust/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
  localhost/metronomo-builder:24.04 ./scripts/release/package-deb.sh
```

Run on a native Ubuntu 24.04 amd64 host with `./scripts/release/package-deb.sh` instead
when desired. The pinned toolchain, dependencies listed in `packaging/linux/debian/control`, and
pre-fetched Cargo cache must be available. The command refuses another host version.
The toolchain mount must contain the real pinned Linux toolchain, not rustup shims.

Source packaging metadata lives in `packaging/linux/debian/`; desktop integration
files live in `packaging/linux/desktop/`. The build script copies release inputs
into `target/deb-build.*/source/` and creates the standard top-level `debian/`
directory there before invoking `dpkg-buildpackage`. Generated packages go to
`dist/build/`, and the assembled delivery goes to `dist/<version>/`.

For clean installation/lifecycle checks, run the following once for each version,
replacing `24.04` with `26.04` for the second run. This script deliberately refuses
to install/remove packages outside a disposable container:

```sh
mkdir -p dist/evidence/install-ubuntu-24.04
podman run --rm \
  -v "$PWD/dist/build:/packages:ro" \
  -v "$PWD/scripts/validation:/checks:ro" \
  -v "$PWD/dist/evidence/install-ubuntu-24.04:/evidence:rw" \
  docker.io/library/ubuntu:24.04 \
  bash /checks/install-deb.sh /packages/metronomo_0.1.0-1_amd64.deb /evidence
```

These checks install without Rust, resolve linked libraries, reinstall, and remove
the package while preserving an isolated settings marker. Menu launching and real
preference loading still require the desktop recording protocol above.

License generation is offline. Texts absent from published crate archives are
stored under `packaging/licenses/<crate>-<version>/` with source and SHA256
provenance. Most are recovered at the exact upstream revision. Where an upstream
text is unavailable, provenance explicitly records a standard license text and
the authors declared by the published crate; it does not invent an upstream
copyright notice. Missing texts block the build. Dependency changes require
reviewing and updating these records; project MIT licensing does not relicense
third-party components. Runtime shared-library licenses remain system-provided.

## Reviewable delivery

```sh
python3 scripts/release/prepare-release.py \
  --package dist/build/metronomo_0.1.0-1_amd64.deb
cd dist/0.1.0
sha256sum --check SHA256SUMS
```

The directory contains the `.deb`, build metadata, release notes, license,
validation report, evidence, source archive/hashes/patch and `manifest.json`. The manifest records the base
commit, dirty working-tree state, source fingerprint, package provenance, and
pending acceptance criteria. A dirty candidate is reviewable, but regenerate it
from the final release commit before publication.

After all evidence passes, use `--require-ready` to make pending acceptance fail
the preparation command. Update release notes and the validation report, change
the two README roadmap checkboxes only after their corresponding acceptance
criteria pass, and regenerate/check the complete delivery. Publication and hosting
are handled by the tagged workflow described below.

## GitHub Actions and version tags

`.github/workflows/ci.yml` runs `scripts/check.sh` for every pull request,
including new commits and reopened PRs. It can also be run manually from Actions.
The shared setup action installs native Linux dependencies, reads the pinned Rust
toolchain, restores the Cargo/build cache, and fetches locked dependencies before
running the offline checks. Tests that require real audio or a desktop remain
opt-in; CI runs the device-free UI and audio tests.

`.github/workflows/release.yml` runs when a tag matching `v[0-9]*` is pushed. To
prepare a new version, for example `0.2.0`:

1. Update `package.version` in `Cargo.toml` and refresh `Cargo.lock` with
   `cargo check`.
2. Add a matching entry to `packaging/linux/debian/changelog`, for example
   `metronomo (0.2.0-1) unstable; urgency=medium`.
3. Add `docs/releases/0.2.0.md` and update `CHANGELOG.md` and the validation report.
4. Run the development checks, commit the version changes, then push the commit
   and its tag:

   ```sh
   git tag -a v0.2.0 -m "Metronomo 0.2.0"
   git push origin HEAD
   git push origin v0.2.0
   ```

The workflow rejects mismatched tag/Cargo/Debian versions and missing release
notes before compiling. Prerelease tags such as `v0.2.0-rc.1` require Cargo version
`0.2.0-rc.1`, Debian version `0.2.0~rc.1-1`, and notes named `0.2.0-rc.1.md`;
they are published with GitHub's prerelease flag.

The build runs on Ubuntu 24.04 amd64, repeats development checks, runs the
optimized offline renderer measurements, and builds the `.deb`. Clean Ubuntu
24.04 and 26.04 containers then check installation, shared-library resolution,
reinstallation, removal, and preference-marker retention. A failed check prevents
publication. Failure evidence is retained as an Actions artifact when available.

After success, a separate job publishes a GitHub Release with:

- `metronomo_<debian-version>_amd64.deb`;
- `metronomo-<version>-linux-amd64.tar.gz`, containing the complete delivery,
  licenses, source archive, manifest, validation report, and automated evidence;
- `SHA256SUMS` for both downloadable assets.

The workflow uses the repository's automatic `GITHUB_TOKEN`; no personal token
or Packages credentials are needed. Only the publication job requests
`contents: write`. Third-party Actions are pinned to commit SHAs. Releases are
created for existing tags and existing releases are not overwritten by retries.

The release commit must be reachable from `main`. Only the maintainer may create
`v*` tags, and version tags are protected against updates and deletion. The
workflow creates a draft, uploads all assets, and then publishes it so immutable
release protection locks a complete delivery. A failed upload leaves a draft
for investigation; the workflow does not overwrite an existing release.

Changes to `main` go through pull requests and required checks. Published
releases cannot be repaired by replacing their assets or moving their tags;
publish a new version instead. See [SECURITY.md](../SECURITY.md) and
[CONTRIBUTING.md](../CONTRIBUTING.md) for the security and contribution policies.

Tagged builds are distributed automatically. Their manifest still records missing
desktop/audio acceptance evidence, and automated container checks do not approve
those measurements. `--require-ready` remains available for a local delivery
that must satisfy the full acceptance protocol before distribution.
