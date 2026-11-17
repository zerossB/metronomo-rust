# Development

[Back to the project](../README.md)

Metronomo uses Rust 2024, GPUI Kit for its native interface, CPAL for audio, and a bounded rtrb queue for playback events.

## Requirements

- Rust installed through `rustup`. The toolchain and development components are pinned in `rust-toolchain.toml`.
- A native compiler and linker for your operating system.
- A graphical desktop session and a compatible graphics driver.

On Linux, GPUI Kit enables both X11 and Wayland. Building requires `pkg-config` and development libraries for Fontconfig, FreeType, XKB, X11, and Wayland. Running requires a Vulkan-capable graphics driver. CPAL also requires the ALSA development library (`libasound2-dev` on Ubuntu/Debian). The shared check script additionally uses Python 3.11+, `desktop-file-validate` (`desktop-file-utils` on Ubuntu), and `appstreamcli` (`appstream`). Package names vary by distribution.

On Ubuntu 24.04/26.04, install the native build dependencies and development-check tools:

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config python3 desktop-file-utils appstream \
  libasound2-dev libfontconfig1-dev libfreetype-dev libxkbcommon-dev \
  libxkbcommon-x11-dev libx11-xcb-dev libwayland-dev
```

Development targets Linux. The release matrix covers Ubuntu 24.04/26.04 amd64 and X11/Wayland; outstanding checks are listed in the validation report. macOS and Windows have not been validated for this application; GPUI's platform requirements still apply.

## Run locally

From the project root:

```sh
cargo run --locked
```

The first build downloads and compiles the GPUI dependency tree and can take several minutes. The application opens the metronome controls. Press Start to open the default output and begin playback. The interface remains responsive while the device opens; Stop can cancel startup. Output failures are shown inline, and Start retries the connection.

To build an optimized executable:

```sh
cargo build --release --locked
```

## Development checks

After `cargo fetch --locked` populates the dependency cache, run the shared offline check script:

```sh
./scripts/check.sh
```

GitHub runs this script on pull requests and on `main`. Caches for downloaded
crates and compiled debug/release profiles are separate, with fallback reuse
after dependency updates. Trusted `main` builds populate caches for future PRs
and tags; a separate warm-up workflow prepares optimized package compilation.
See [release caching and recovery](release.md#build-caches-and-publication-recovery).

Its Rust checks are:

```sh
cargo fmt --all -- --check
cargo check --all-targets --locked
cargo test --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
```

Unit tests cover sample-frame timing, fractional BPM intervals, note-unit duration scaling, bar wrapping, per-beat timbres, first-beat accents, mute, volume, latency-aware visual positions, and settings validation and replacement. UI integration tests use the production view with a device-free audio session to cover controls, initialization cancellation, output failure/retry, meter and per-beat sound menus, keyboard focus restoration, layout with larger text, stable BPM geometry, beat-grid reading order, fixed-footer reachability, and persistence across closing and reopening. Persistence tests use temporary directories and never modify personal settings.

Window-free session tests verify that commands keep the model, running audio, and persisted snapshots coherent, preserve hidden sounds when loading presets or restoring defaults, isolate playback notifications, and flush the final snapshot on close.

Preset tests also cover snapshot fidelity, naming conflicts, rename/delete/undo, backward-compatible settings loading, restore-defaults persistence, live audio updates, and keyboard navigation through a full library at the minimum window size with larger text. To export rendered light/dark and zoomed main-window and dialog previews into the temporary directory:

```sh
cargo test --locked render_ -- --ignored --test-threads=1
```

An opt-in Linux test opens a real window briefly, minimizes and restores it, and closes it through the title bar without playing audio or reading personal settings:

```sh
cargo test --locked native_window_controls_smoke -- --ignored --nocapture
```

An opt-in test opens the **real default output** and plays quiet clicks for about two seconds:

```sh
cargo test --locked default_output_audio_smoke -- --ignored --nocapture
```

For the opt-in optimized timing matrix and thirty-minute renderer simulations:

```sh
./scripts/validation/audio.sh renderer
```

For a ten-minute real-device diagnostic run, which plays quiet clicks:

```sh
METRONOMO_VALIDATION_DIR="$PWD/dist/evidence/ubuntu-26.04-wayland" \
  ./scripts/validation/audio.sh device
```

Reports include device/sample format, environment, duration, musical changes, callback/error/event counters, and a source fingerprint. Offline sample precision, predicted playback timing, and actual desktop/acoustic observations remain separate evidence. See the [recording protocol](release.md) for the 100 ms responsiveness and 50 ms audio-visual acceptance checks.

The audio callback schedules beats independently of GPUI. The visual indicator uses CPAL's predicted playback time, including the reported output-buffer latency, and refreshes approximately every 16 ms. Visual pulses last up to 100 ms and shorten to half the click interval at faster note rates so successive beats remain distinct. Actual visual alignment is limited by backend timestamp accuracy and display refresh. The stream stays open and produces silence while stopped, and closes with the view.

## Project structure

Build, packaging, and validation files are grouped by purpose:

```text
.github/
├── workflows/               # PR checks and tagged GitHub Releases
└── actions/setup/           # Shared Linux dependencies, toolchain, and cache
packaging/
├── linux/
│   ├── desktop/             # Application menu entry and icon
│   ├── debian/              # Source metadata and rules for the .deb package
│   └── containers/          # Ubuntu baseline build environment
└── licenses/                # Versioned third-party license supplements
scripts/
├── check.sh                 # Development checks
├── release/                 # Package build, staging, notices, provenance, and delivery
│   └── tests/               # Release automation tests
└── validation/              # Audio measurements and package installation checks
docs/
├── images/                  # README screenshots and capture instructions
├── usage.md                 # Complete controls and settings guide
├── development.md           # Setup, checks, and architecture
├── release.md               # Build and release procedure
├── releases/                # Release notes by version
└── validation/              # Acceptance report and recording template
target/                      # Generated build output and temporary package sources (ignored)
dist/                        # Generated packages, deliveries, and evidence (ignored)
```

Run `./scripts/check.sh` for development checks,
`./scripts/release/package-deb.sh` to build the Ubuntu package, and
`./scripts/validation/audio.sh renderer` for offline audio measurements.
See [release preparation](release.md) for environment requirements and
device/installation validation commands.

The application source is organized as follows:

```text
src/
├── main.rs                  # Executable entry point
├── theme.rs                 # Product theme registration
├── theme/
│   ├── metronomo.json       # Light/dark semantic colors and amber interaction states
│   └── tests.rs             # Text/focus contrast and appearance-switching checks
├── app/
│   └── mod.rs               # GPUI bootstrap, window options, and quit routing
└── metronome/
    ├── mod.rs               # Feature boundary and deliberate re-exports
    ├── configuration.rs     # Musical snapshots, supported ranges, and defaults
    ├── meter.rs             # Time signatures and all twelve per-beat sound slots
    ├── presets.rs           # Named library, stable identity, and validation
    ├── presets/tests.rs     # Pure preset tests
    ├── tap_tempo.rs         # Pure quarter-note tempo estimation
    ├── session.rs           # Authoritative configuration, presets, and tap commands
    ├── session/tests.rs     # Window-free feature integration tests
    ├── playback.rs          # Device startup, cancellation, errors, and visual polling
    ├── settings.rs          # Versioned JSON document, validation, and startup loading
    ├── settings/
    │   ├── store.rs         # Bounded reads and atomic file replacement
    │   ├── writer.rs        # Save queue, error state, and close/quit flushing
    │   └── tests.rs         # Temporary-file persistence and compatibility tests
    ├── audio.rs             # CPAL output, atomic controls, and audible timeline
    ├── audio/
    │   ├── renderer.rs      # Synthesized clicks and sample-frame scheduling
    │   ├── diagnostics.rs   # Callback, error, and dropped-event counters
    │   ├── tests.rs         # DSP, synchronization, and opt-in device tests
    │   └── validation.rs    # Opt-in timing measurements and device reports
    ├── view.rs              # Window composition, control state, focus, and shortcuts
    ├── view/
    │   ├── header.rs        # Title bar and playback status
    │   ├── beat_block.rs    # Number, sound, accent, and audio-driven playhead display
    │   ├── tempo_controls.rs
    │   ├── meter_controls.rs
    │   ├── volume_controls.rs
    │   ├── transport_controls.rs
    │   ├── preset_controls.rs # Toolbar and dialog composition
    │   ├── presets_dialog.rs # Preset editing, draft, undo, and modal focus
    │   └── tests/           # UI flows grouped by responsibility and rendered previews
    └── test_support.rs      # Shared device-free audio fixture, compiled only for tests
Cargo.toml                  # Package metadata, dependencies, and lint policy
Cargo.lock                  # Resolved application dependencies; keep tracked
rust-toolchain.toml         # Reproducible Rust toolchain and development tools
```

### State and dependency flow

The application shell composes the window through the feature's `init`, `Metronome`, and `Preferences` exports. Within the feature, the UI sends commands to `MetronomeSession`; it does not own the musical configuration or preset library. The configuration, meter, preset library, and tap estimator have no GPUI or CPAL dependencies.

`MetronomeSession` owns the active configuration and tap history, delegates output to `PlaybackController`, and submits complete settings snapshots to `PreferencesWriter`. These entities retain their own tasks and subscriptions. Sliders mirror the session through silent value synchronization and submit the value carried by each user event, avoiding feedback loops and stale-value overwrites. The preset dialog targets session commands directly and retains only its draft, undo, focus, and scrolling state.

Configuration, preset-library, and tap events are separate from playback and save notifications. Audio pulses update the window without invalidating the preset editor. The playback controller retains the device-free `AudioPlayback`/`AudioFactory` seam for tests and performs device opening off the UI thread. Its configuration is an applied output snapshot; the session remains the authoritative source.

The writer coalesces queued snapshots and performs file I/O on workers. Closing stops playback and awaits pending saves; system quit captures the latest snapshot before window destruction and finishes independently of foreground entities. The settings location, JSON version, and legacy-file compatibility are preserved.

The audio callback reads one coherent atomic control snapshot and sends playback timestamps through a preallocated queue. It does not call GPUI, allocate, or acquire a mutex. Keep new behavior behind these existing seams and introduce another module only when it has a concrete responsibility.
