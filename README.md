# Metronomo

A desktop metronome built with Rust and GPUI, designed to help musicians practice with a steady pulse and straightforward controls.

## Project status

**Early development.** The Rust project is initialized with a GPUI Kit application window, theme-aware presentation, and application lifecycle handling. Closing the last window or using `Ctrl+Q` / `Cmd+Q` exits the application.

Playback, tempo controls, and audio output are not implemented yet. There is no published release.

The features below describe the intended direction, not functionality available today.

## Goals

- Provide a focused desktop experience for everyday music practice.
- Make tempo adjustment and playback controls easy to use.
- Deliver consistent audible beats with clear visual feedback.
- Keep the application responsive during playback.

## Planned features

- Adjustable tempo in beats per minute (BPM).
- Start and stop controls.
- Audible clicks and a visual beat indicator.
- Configurable beats per measure with an accented first beat.
- Tap tempo and keyboard shortcuts.

Feature scope and implementation order may change as development progresses.

## Technology

The application uses **Rust 2024**, **Cargo**, and **GPUI** through **GPUI Kit**. The `gpui-kit` dependency supplies the framework, platform backend, styled components, and default assets through one facade. Audio playback dependencies have not been selected yet.

## Getting started

### Requirements

- Rust installed through `rustup`. The toolchain and development components are pinned in `rust-toolchain.toml`.
- A native compiler and linker for your operating system.
- A graphical desktop session and a compatible graphics driver.

On Linux, GPUI Kit enables both X11 and Wayland. Building requires `pkg-config` and development libraries for Fontconfig, FreeType, XKB, X11, and Wayland. Running requires a Vulkan-capable graphics driver. Package names vary by distribution.

On Ubuntu, the XKB bridge for X11 is provided by `libxkbcommon-x11-dev`. If linking fails with `unable to find library -lxkbcommon-x11`, install it:

```sh
sudo apt-get install libxkbcommon-x11-dev
```

Development currently targets Linux. macOS and Windows have not been validated for this application; GPUI's platform requirements still apply.

### Run locally

From the project root:

```sh
cargo run --locked
```

The first build downloads and compiles the GPUI dependency tree and can take several minutes. The application opens a starter window; it does not produce sound yet.

To build an optimized executable:

```sh
cargo build --release --locked
```

### Development checks

```sh
cargo fmt --all -- --check
cargo check --all-targets --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
```

There are no automated behavior tests yet. Add focused tests alongside playback and tempo logic as those features are introduced.

## Project structure

```text
src/
├── main.rs              # Executable entry point
├── app/
│   └── mod.rs           # GPUI initialization, window options, and quit handling
└── metronome/
    ├── mod.rs           # Feature boundary and deliberate re-exports
    └── view.rs          # Metronome window content
Cargo.toml              # Package metadata, dependencies, and lint policy
Cargo.lock              # Resolved application dependencies; keep tracked
rust-toolchain.toml     # Reproducible Rust toolchain and development tools
```

The application shell owns window lifecycle. The `metronome` module owns the feature's presentation and will host its model and playback integration as they are implemented. Keep related behavior together and add modules when they have a concrete responsibility.

## Roadmap

- [x] Initialize the Rust project and create a GPUI application window.
- [ ] Implement tempo controls and start/stop behavior.
- [ ] Add audible beats and visual feedback.
- [ ] Add measure configuration and beat accents.
- [ ] Add tap tempo and keyboard shortcuts.
- [ ] Validate playback timing and responsiveness on supported platforms.
- [ ] Document installation and prepare the first release.

## Contributing

Contributions can help refine the roadmap and implement the metronome's core behavior.

Keep changes focused, explain their purpose, and update documentation when behavior or setup requirements change. Run the development checks before submitting changes and include relevant validation results; timing and audio changes should describe the platform and conditions used to test them.

## License

A license has not been selected yet. Licensing terms will be documented in a `LICENSE` file when chosen.
