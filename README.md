# Metronomo

A desktop metronome built with Rust and GPUI, designed to help musicians practice with a steady pulse and straightforward controls.

## Project status

**Early development.** The application provides synthesized metronome clicks, BPM and volume controls, and a visual beat indicator in a theme-aware GPUI Kit window. The title bar provides desktop window controls. Closing the last window or using `Ctrl+Q` / `Cmd+Q` exits the application after pending settings saves finish.

Audio plays through the system's default output device. There is no published release.

## Goals

- Provide a focused desktop experience for everyday music practice.
- Make tempo adjustment and playback controls easy to use.
- Deliver consistent audible beats with clear visual feedback.
- Keep the application responsive during playback.

## Features

- Tempo from **40 to 240 BPM**, starting at **120 BPM**, with a fixed **quarter-note (1/4) reference**.
- Slider for quick adjustment and −/+ buttons for changes of one BPM.
- Tap Tempo button and `T` shortcut, with a rolling average of the last four tap intervals.
- Synthesized percussive click through the default audio output, using its sample rate and format.
- Start/stop button and numbered beat indicators driven by audio playback timestamps.
- Configurable time signature: **1–12 beats per bar**, with half, quarter, eighth, or sixteenth-note units; **4/4** by default.
- Independent **High**, **Medium**, **Low**, or **Muted** sound choice for every beat.
- Accented first beat, with a persistent highlight and current-beat count during playback.
- Volume from **0 to 100%**, starting at **60%**; zero mutes the click while the visual beat continues.
- Live tempo changes preserve progress toward the next beat. Volume changes use a short gain ramp.
- Keyboard operation, accessible control names, and light/dark theme support.
- Automatic persistence of tempo, volume, time signature, and all twelve beat-sound choices.
- A GPUI Kit title bar with minimize, maximize/restore, and close controls where supported by the desktop.

### Keyboard controls

| Key | Action |
| --- | --- |
| `↑` / `↓` outside the volume control | Increase / decrease tempo by one BPM |
| Arrow keys with volume focused | Adjust volume by five percentage points |
| `Space` / `Enter` | Start/stop when the main view or playback button has focus |
| `T` outside menus | Tap a quarter-note beat to set the tempo |
| `Tab` / `Shift+Tab` | Move between tempo, Tap Tempo, meter, beat-sound, volume, and playback controls |
| `Enter` / `Space` on a meter or beat-sound control | Open its menu |
| Arrow keys / `Enter` inside a menu | Navigate / select an option |
| `Escape` inside a menu | Close it and return focus to its control |
| `Space` / `Enter` on a −/+ button | Activate that button |
| `Space` / `Enter` on Tap Tempo | Record a tap without starting or stopping playback |
| `Ctrl+Q` / `Cmd+Q` | Quit |
| `Ctrl+W` / `Cmd+W` | Close the window and finish pending settings saves |
| `Ctrl+M` / `Cmd+M` | Minimize the window |

### Tap Tempo

Click **Tap tempo** or press **T** in time with quarter notes (1/4), regardless of the selected beat unit. The first tap starts measurement; the second sets the BPM. Further taps smooth the estimate using the last four intervals, rounded to a whole BPM and limited to 40–240 BPM. For example, taps 500 ms apart set 120 BPM.

After three seconds without tapping, the next tap starts a fresh sequence while preserving the current BPM. Manual tempo edits also clear the sequence. Accidental taps less than 100 ms apart and keyboard auto-repeat do not contribute to the estimate. Tap Tempo works while stopped or playing; it updates the running audio without restarting the bar or moving the audio-driven visual playhead.

### Meter and beat sounds

Use **Beats per bar** to choose the number of beats and **Beat unit** to choose the denominator. BPM always refers to quarter notes (1/4). The selected note unit scales each numbered beat's duration: at 120 BPM, half notes (1/2) last 1 second, quarter notes (1/4) last 500 ms, eighth notes (1/8) last 250 ms, and sixteenth notes (1/16) last 125 ms. For example, 6/8 produces six eighth-note clicks per bar at twice the displayed quarter-note BPM; it does not automatically group them into two dotted-quarter beats.

Select a numbered beat to choose its sound: High, Medium, Low, or Muted. The default pattern uses High on beat 1 and Medium on the remaining beats. Beat 1 has a stronger amplitude with any audible timbre; selecting Muted takes precedence over its accent. Muted beats still advance the count and visual indicator.

The active beat remains highlighted between pulses, and the counter shows its position in the bar. Changing the time signature starts a new bar on the audio callback; changing a sound preserves the current beat progression. Shrinking and expanding a bar preserves the sound choices of hidden beats, including across application restarts.

### Saved settings and window controls

BPM (including Tap Tempo changes), volume, beats per bar, beat unit, and all twelve sound slots save automatically when edited. On startup, the saved values are restored before the window opens. Playback always starts stopped; tap history and the current playhead are temporary.

Preferences use a versioned `settings.json` file in the user's configuration directory:

| Platform | Location |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/metronomo/settings.json`, or `~/.config/metronomo/settings.json` |
| macOS | `~/Library/Application Support/metronomo/settings.json` |
| Windows | `%APPDATA%\metronomo\settings.json` |

File reads and writes run off the UI and audio threads. Writes are serialized, coalesce pending changes, and atomically replace the previous file from a temporary file in the same directory. A missing file uses defaults. Invalid, unsupported, or unreadable settings show an inline warning and use defaults; the original file remains untouched until a setting is changed. A failed save leaves the live controls usable and retries on the next edit or when closing. Read/write errors also appear in the terminal with diagnostic details.

Use the title bar to drag, minimize, maximize/restore, or close the window. It follows the platform's supported controls; macOS uses its native window buttons, and Linux defers to system decorations if the compositor selects them. Minimizing keeps audio playing. Closing stops playback and waits for pending writes before removing the window; the last window exits the app.

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

Unit tests cover sample-frame timing, fractional BPM intervals, note-unit duration scaling, bar wrapping, per-beat timbres, first-beat accents, mute, volume, latency-aware visual positions, and settings validation and replacement. UI integration tests use the production view with a device-free audio session to cover controls, initialization cancellation, output failure/retry, meter and per-beat sound menus, keyboard focus restoration, layout with larger text, and persistence across closing and reopening. Persistence tests use temporary directories and never modify personal settings.

An opt-in Linux test opens a real window briefly, minimizes and restores it, and closes it through the title bar without playing audio or reading personal settings:

```sh
cargo test --locked native_window_controls_smoke -- --ignored --nocapture
```

An opt-in test opens the **real default output** and plays quiet clicks for about two seconds:

```sh
cargo test --locked default_output_audio_smoke -- --ignored --nocapture
```

The audio callback schedules beats independently of GPUI. The visual indicator uses CPAL's predicted playback time, including the reported output-buffer latency, and refreshes approximately every 16 ms. Visual pulses last up to 100 ms and shorten to half the click interval at faster note rates so successive beats remain distinct. Actual visual alignment is limited by backend timestamp accuracy and display refresh. The stream stays open and produces silence while stopped, and closes with the view.

## Project structure

```text
src/
├── main.rs              # Executable entry point
├── app/
│   └── mod.rs           # GPUI initialization, window options, and quit handling
└── metronome/
    ├── mod.rs           # Feature boundary and deliberate re-exports
    ├── meter.rs         # Time signature and per-beat sound model
    ├── tap_tempo.rs     # Monotonic tap timing and bounded tempo estimation
    ├── settings.rs      # Versioned settings, validation, and atomic file writes
    ├── settings/
    │   └── tests.rs     # Temporary-file persistence and recovery tests
    ├── audio.rs         # Default output, atomic controls, and playback timeline
    ├── audio/
    │   ├── renderer.rs  # Synthesized click and sample-frame beat scheduling
    │   └── tests.rs     # DSP, synchronization, and opt-in device tests
    ├── view.rs          # Tempo/volume controls and audio lifecycle
    ├── view/
    │   ├── meter_controls.rs # Meter menus and numbered beat indicators
    │   └── persistence.rs    # Background save coordination and close/quit flushing
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
- [x] Add time signatures, per-beat sounds, and first-beat accents.
- [x] Add tap tempo and keyboard shortcuts.
- [x] Persist preferences and add desktop window controls.
- [ ] Validate playback timing and responsiveness on supported platforms.
- [ ] Document installation and prepare the first release.

## Contributing

Contributions can help refine the roadmap and implement the metronome's core behavior.

Keep changes focused, explain their purpose, and update documentation when behavior or setup requirements change. Run the development checks before submitting changes and include relevant validation results; timing and audio changes should describe the platform and conditions used to test them.

## License

A license has not been selected yet. Licensing terms will be documented in a `LICENSE` file when chosen.
