# Metronomo

A desktop metronome built with Rust and GPUI, designed to help musicians practice with a steady pulse and straightforward controls.

## Project status

**Early development.** The application provides synthesized metronome clicks, BPM and volume controls, and a visual beat indicator in a theme-aware GPUI Kit window. Closing the last window or using `Ctrl+Q` / `Cmd+Q` exits the application.

Audio plays through the system's default output device. There is no published release.

## Goals

- Provide a focused desktop experience for everyday music practice.
- Make tempo adjustment and playback controls easy to use.
- Deliver consistent audible beats with clear visual feedback.
- Keep the application responsive during playback.

## Features

- Tempo from **40 to 240 BPM**, starting at **120 BPM**.
- Slider for quick adjustment and −/+ buttons for changes of one BPM.
- Synthesized percussive click through the default audio output, using its sample rate and format.
- Start/stop button and a visual indicator driven by audio playback timestamps.
- Volume from **0 to 100%**, starting at **60%**; zero mutes the click while the visual beat continues.
- Live tempo changes preserve progress toward the next beat. Volume changes use a short gain ramp.
- Keyboard operation, accessible control names, and light/dark theme support.

### Keyboard controls

| Key | Action |
| --- | --- |
| `↑` / `↓` outside the volume control | Increase / decrease tempo by one BPM |
| Arrow keys with volume focused | Adjust volume by five percentage points |
| `Space` / `Enter` | Start/stop when the main view or playback button has focus |
| `Tab` / `Shift+Tab` | Move between tempo buttons, volume, and playback |
| `Space` / `Enter` on a −/+ button | Activate that button |
| `Ctrl+Q` / `Cmd+Q` | Quit |

### Planned features

- Configurable beats per measure with an accented first beat.
- Tap tempo.

Feature scope and implementation order may change as development progresses.

## Technology

The application uses **Rust 2024**, **Cargo**, and **GPUI** through **GPUI Kit**. The `gpui-kit` dependency supplies the framework, platform backend, styled components, and default assets through one facade. Audio uses **CPAL**, with a precomputed click and sample-frame scheduling. A bounded **rtrb** queue carries beat timestamps to the UI without blocking the audio callback.

## Getting started

### Requirements

- Rust installed through `rustup`. The toolchain and development components are pinned in `rust-toolchain.toml`.
- A native compiler and linker for your operating system.
- A graphical desktop session and a compatible graphics driver.

On Linux, GPUI Kit enables both X11 and Wayland. Building requires `pkg-config` and development libraries for Fontconfig, FreeType, XKB, X11, and Wayland. Running requires a Vulkan-capable graphics driver. CPAL also requires the ALSA development library (`libasound2-dev` on Ubuntu/Debian). Package names vary by distribution.

On Ubuntu, the XKB bridge for X11 is provided by `libxkbcommon-x11-dev`. If linking fails with `unable to find library -lxkbcommon-x11`, install it:

```sh
sudo apt-get install libxkbcommon-x11-dev libasound2-dev
```

Development currently targets Linux. macOS and Windows have not been validated for this application; GPUI's platform requirements still apply.

### Run locally

From the project root:

```sh
cargo run --locked
```

The first build downloads and compiles the GPUI dependency tree and can take several minutes. The application opens the metronome controls. Press Start to open the default output and begin playback. The interface remains responsive while the device opens; Stop can cancel startup. Output failures are shown inline, and Start retries the connection.

To build an optimized executable:

```sh
cargo build --release --locked
```

### Development checks

```sh
cargo fmt --all -- --check
cargo check --all-targets --locked
cargo test --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
```

Unit tests cover sample-frame timing, fractional BPM intervals, channel output, volume, mute, and latency-aware visual events. UI integration tests use the production view with a device-free audio session to cover controls, initialization cancellation, output failure/retry, and layout with larger text.

An opt-in test opens the **real default output** and plays quiet clicks for about two seconds:

```sh
cargo test --locked default_output_audio_smoke -- --ignored --nocapture
```

The audio callback schedules beats independently of GPUI. The visual indicator uses CPAL's predicted playback time, including the reported output-buffer latency, and refreshes approximately every 16 ms. Actual visual alignment is limited by backend timestamp accuracy and display refresh. The stream stays open and produces silence while stopped, and closes with the view.

## Project structure

```text
src/
├── main.rs              # Executable entry point
├── app/
│   └── mod.rs           # GPUI initialization, window options, and quit handling
└── metronome/
    ├── mod.rs           # Feature boundary and deliberate re-exports
    ├── audio.rs         # Default output, atomic controls, and playback timeline
    ├── audio/
    │   ├── renderer.rs  # Synthesized click and sample-frame beat scheduling
    │   └── tests.rs     # DSP, synchronization, and opt-in device tests
    ├── view.rs          # Tempo/volume controls and audio lifecycle
    └── tests.rs         # UI integration tests with a device-free audio session
Cargo.toml              # Package metadata, dependencies, and lint policy
Cargo.lock              # Resolved application dependencies; keep tracked
rust-toolchain.toml     # Reproducible Rust toolchain and development tools
```

The application shell owns window lifecycle. The `metronome` module owns presentation and audio lifecycle. Opening the device runs off the UI thread. The audio callback reads one coherent atomic control snapshot and sends playback timestamps through a preallocated queue; it does not call GPUI, allocate, or acquire a mutex. The UI only follows the resulting playback timeline. Keep related behavior together and add modules when they have a concrete responsibility.

## Roadmap

- [x] Initialize the Rust project and create a GPUI application window.
- [x] Implement tempo controls and start/stop behavior.
- [x] Add visual beat feedback.
- [x] Add audible beats, volume, and synchronized visual feedback.
- [ ] Add measure configuration and beat accents.
- [ ] Add tap tempo and keyboard shortcuts.
- [ ] Validate playback timing and responsiveness on supported platforms.
- [ ] Document installation and prepare the first release.

## Contributing

Contributions can help refine the roadmap and implement the metronome's core behavior.

Keep changes focused, explain their purpose, and update documentation when behavior or setup requirements change. Run the development checks before submitting changes and include relevant validation results; timing and audio changes should describe the platform and conditions used to test them.

## License

A license has not been selected yet. Licensing terms will be documented in a `LICENSE` file when chosen.
