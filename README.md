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
- Large, stable-width BPM display and a fixed footer for volume, Start/Stop, and Tap Tempo.
- Numbered beat blocks driven by audio playback timestamps, with persistent sound labels, an accent star on beat 1, and a visible Playing marker.
- Restrained amber accents with dedicated light and dark product themes.
- Configurable time signature: **1–12 beats per bar**, with half, quarter, eighth, or sixteenth-note units; **4/4** by default.
- Independent **High**, **Medium**, **Low**, or **Muted** sound choice for every beat.
- Accented first beat, with a persistent highlight and current-beat count during playback.
- Volume from **0 to 100%**, starting at **60%**; zero mutes the click while the visual beat continues.
- Live tempo changes preserve progress toward the next beat. Volume changes use a short gain ramp.
- Keyboard operation, accessible control names, and light/dark theme support.
- Automatic persistence of tempo, volume, time signature, and all twelve beat-sound choices.
- Restore-defaults action and named presets with save, load, rename, delete, and undo-delete controls.
- A GPUI Kit title bar with minimize, maximize/restore, and close controls where supported by the desktop.

### Keyboard controls

| Key | Action |
| --- | --- |
| `↑` / `↓` outside the volume control | Increase / decrease tempo by one BPM |
| Arrow keys with volume focused | Adjust volume by five percentage points |
| `Space` / `Enter` | Start/stop when the main view or playback button has focus |
| `T` outside menus | Tap a quarter-note beat to set the tempo |
| `Tab` / `Shift+Tab` | Move between preset, restore-defaults, tempo, meter, beat-sound, volume, playback, and Tap Tempo controls |
| `Enter` / `Space` on a meter or beat-sound control | Open its menu |
| Arrow keys / `Enter` inside a menu | Navigate / select an option |
| `Escape` inside a menu | Close it and return focus to its control |
| `Space` / `Enter` on a −/+ button | Activate that button |
| `Space` / `Enter` on Tap Tempo | Record a tap without starting or stopping playback |
| `Ctrl+Q` / `Cmd+Q` | Quit |
| `Ctrl+W` / `Cmd+W` | Close the window and finish pending settings saves |
| `Ctrl+M` / `Cmd+M` | Minimize the window |
| `Enter` in the preset name field | Save the current configuration or finish renaming |
| `Enter` / `Space` on a preset dialog button | Activate that button |
| `Escape` in the presets dialog | Dismiss the topmost menu, or close the dialog and restore focus |

### Tap Tempo

Click **Tap tempo** or press **T** in time with quarter notes (1/4), regardless of the selected beat unit. The first tap starts measurement; the second sets the BPM. Further taps smooth the estimate using the last four intervals, rounded to a whole BPM and limited to 40–240 BPM. For example, taps 500 ms apart set 120 BPM.

After three seconds without tapping, the next tap starts a fresh sequence while preserving the current BPM. Manual tempo edits also clear the sequence. Accidental taps less than 100 ms apart and keyboard auto-repeat do not contribute to the estimate. Tap Tempo works while stopped or playing; it updates the running audio without restarting the bar or moving the audio-driven visual playhead.

### Meter and beat sounds

Use **Beats per bar** to choose the number of beats and **Beat unit** to choose the denominator. BPM always refers to quarter notes (1/4). The selected note unit scales each numbered beat's duration: at 120 BPM, half notes (1/2) last 1 second, quarter notes (1/4) last 500 ms, eighth notes (1/8) last 250 ms, and sixteenth notes (1/16) last 125 ms. For example, 6/8 produces six eighth-note clicks per bar at twice the displayed quarter-note BPM; it does not automatically group them into two dotted-quarter beats.

Each block shows a large beat number and its sound. Up to four beats occupy one row; longer meters use four columns in reading order. Select a numbered beat to open its native sound menu: High, Medium, Low, or Muted. The default pattern uses High on beat 1 and Medium on the remaining beats. Beat 1 has a stronger amplitude with any audible timbre; selecting Muted takes precedence over its accent. Muted beats still advance the count and visual indicator.

The star marks beat 1 independently of the playhead. The active beat retains its amber border, fill, and Playing marker between pulses, and the counter shows its position in the bar. Changing the time signature starts a new bar on the audio callback; changing a sound preserves the current beat progression. Shrinking and expanding a bar preserves the sound choices of hidden beats, including across application restarts.

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

### Restore defaults and presets

Select **Restore defaults** to return to **120 BPM**, **60% volume**, **4/4**, and the default sounds: High on beat 1 and Medium on all remaining slots. This resets all twelve sound slots and the Tap Tempo sequence, and updates the saved file automatically. Existing presets remain available. Playback keeps its current running or stopped state.

Open **Presets…**, enter a name, and select **Save preset** or press Enter. Each preset captures BPM, volume, time signature, and all twelve beat-sound choices, including hidden beats. Presets are snapshots: later metronome adjustments leave their saved configurations intact.

Select **Load** to apply a preset and return to the main controls. Loading updates the live audio and saved current configuration while preserving the current playback state. Each preset's **Actions** menu offers **Rename…** and **Delete**. A deletion offers **Undo delete** while the presets dialog remains open; closing the dialog dismisses that undo opportunity. Deleting a preset leaves the metronome's current configuration in place.

Names are trimmed, unique without regard to case, and limited to 64 characters. The library holds up to 64 presets. Name validation and file-save failures appear inside the dialog. Settings and presets are saved together in `settings.json` using the existing atomic write path, and older settings files load with an empty preset library.

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

The audio callback schedules beats independently of GPUI. The visual indicator uses CPAL's predicted playback time, including the reported output-buffer latency, and refreshes approximately every 16 ms. Visual pulses last up to 100 ms and shorten to half the click interval at faster note rates so successive beats remain distinct. Actual visual alignment is limited by backend timestamp accuracy and display refresh. The stream stays open and produces silence while stopped, and closes with the view.

## Project structure

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
    │   └── tests.rs         # DSP, synchronization, and opt-in device tests
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

## Roadmap

- [x] Initialize the Rust project and create a GPUI application window.
- [x] Implement tempo controls and start/stop behavior.
- [x] Add visual beat feedback.
- [x] Add audible beats, volume, and synchronized visual feedback.
- [x] Add time signatures, per-beat sounds, and first-beat accents.
- [x] Add tap tempo and keyboard shortcuts.
- [x] Persist preferences and add desktop window controls.
- [x] Restore defaults and manage named configuration presets.
- [ ] Validate playback timing and responsiveness on supported platforms.
- [ ] Document installation and prepare the first release.

## Contributing

Contributions can help refine the roadmap and implement the metronome's core behavior.

Keep changes focused, explain their purpose, and update documentation when behavior or setup requirements change. Run the development checks before submitting changes and include relevant validation results; timing and audio changes should describe the platform and conditions used to test them.

## License

A license has not been selected yet. Licensing terms will be documented in a `LICENSE` file when chosen.
