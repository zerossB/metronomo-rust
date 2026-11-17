# Changelog

## 0.1.2

- Align Cargo, Debian, release notes, and the version tag before building installers.
- Add AppStream developer, MIT license, release date, screenshots, and Brazilian Portuguese descriptions.
- Provide scalable and PNG desktop icons with a consistent application ID.
- Update `dirs` to version 7 and validate package metadata and installation lifecycle.
- Reuse trusted main-branch build caches and publish complete releases through a draft.

The immutable `v0.1.1` release has no installers: its tag references a commit
that still declares version `0.1.0`. Version `0.1.2` provides the aligned release
without modifying that existing tag.

## 0.1.0 — release candidate

- Desktop metronome with synthesized clicks, 40–240 BPM, volume, and audio-driven beat feedback.
- Configurable meters and twelve independent beat sounds, first-beat accent, Tap Tempo, and keyboard controls.
- Atomic preference persistence, restore defaults, named presets, and desktop window controls.
- Bounded audio diagnostics for underruns, stream failures, route changes, and discarded events.
- Opt-in renderer timing matrix, thirty-minute simulations, and ten-minute default-device diagnostics.
- Ubuntu 24.04 baseline `.deb` packaging for amd64, desktop integration, MIT license, and bundled dependency notices.

The release remains a candidate until all platform, installation, and measured desktop/audio acceptance checks in `docs/validation/report.md` pass.
