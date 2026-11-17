# v0.1.0 validation report

Status: **incomplete — publication blocked**.

The target matrix is Ubuntu 24.04 and 26.04, amd64, with both X11 and Wayland.
The two remaining README roadmap items stay unchecked until the acceptance
criteria below have actual evidence. A package build alone does not complete them.

## Automated evidence

Reports live in `dist/evidence/` and are copied into `dist/0.1.0/evidence/` when
the release candidate is assembled. Each report includes its environment and a
fingerprint of the Rust source inputs; stale reports block publication.

| Check | Acceptance | Status |
| --- | --- | --- |
| Formatting, check, tests, Clippy | All pass | Passed: 72 Rust tests, 7 opt-in tests skipped, 7 script tests; no check/Clippy errors |
| Offline timing matrix | 480 cases; ≤1.01 frames maximum error; correct count/order | Passed with optimized Ubuntu 24.04 build; maximum error 1 frame |
| Offline long run | Three 30-minute simulations; same timing/count bounds | Passed with optimized Ubuntu 24.04 build; maximum error 1 frame |
| Ubuntu 24.04 build | Locked toolchain/dependencies; baseline package built successfully | Passed: Rust 1.99.1, Ubuntu 24.04.5 container; optimized suite also passed (72 tests) |
| Package dependencies | Install in clean 24.04/26.04 environments without Rust | Passed on both clean containers: install, linked libraries, desktop/icon/documentation files, reinstall and remove; isolated preference marker preserved |

## Desktop and real-output evidence

| Target | Ten-minute device diagnostics | Native window | Measured interaction / audio-visual alignment | Package lifecycle |
| --- | --- | --- | --- | --- |
| Ubuntu 24.04 / X11 | Pending | Pending | Pending | Pending |
| Ubuntu 24.04 / Wayland | Pending | Pending | Pending | Pending |
| Ubuntu 26.04 / X11 | Pending | XWayland optimized smoke passed; native Xorg pending | Pending | Pending |
| Ubuntu 26.04 / Wayland | Passed: 600 seconds on default ALSA output, 48 kHz stereo f32; zero reported underruns/errors/event loss | Optimized minimize/restore/close smoke passed | Pending | Pending |

X11 through XWayland must be labeled as such; it does not establish behavior
in a native Xorg desktop session. Container installation checks do not establish
desktop/audio behavior. Preview renders and test-platform frames do not establish
physical audio-visual synchronization or OS input latency.

Execution environment: Ubuntu 26.04.1 amd64, Wayland desktop with XWayland
available. Both native-window smoke tests used the optimized test executable
built in the Ubuntu 24.04 container and a device-free session; they do not prove
playback while minimized. Full desktop scenarios remain pending.

The ten-minute real-output run completed in 600.003 seconds while release builds
were running in the background. It rendered 28,802,048 frames in 56,254 callbacks
and scheduled 4,516 beats, including transport/meter restarts. The consumer
observed 4,514 timeline pulses; generation changes can discard obsolete queued
positions intentionally. Reported underruns, fatal stream errors, route changes,
realtime denials, visual queue overflow, and error queue overflow were all zero.
The largest consumer polling gap was 8.232 ms. This is a polling measurement,
**not** input-to-frame latency or a physical audio-visual alignment measurement.

The exact final `.deb` installed in both clean containers has SHA256
`a82aed5173ffe43d55a6cc68ef089fa405c3685e9ffc40bcbacee5e100eef996`.
Neither container had Rust. The Ubuntu minimal-image documentation filter was
overridden for this package inside the disposable containers so notices and
documentation could be checked. Library resolution, reinstall, removal, and
preference-marker retention all passed; these checks did not launch a desktop
or play audio inside the containers. Full package/menu desktop scenarios in the
matrix therefore remain pending. Raw results are in `install-ubuntu-24.04/` and
`install-ubuntu-26.04/` under the delivery's evidence directory.

The report installed inside the `.deb` is a build-time snapshot. The delivery's
`VALIDATION.md`, manifest, and evidence contain the post-build validation results.

The base commit is `d0a5590da775c7d78bbf0f14ddd7ca6e740a2836` with implementation
changes still uncommitted. Exact Rust-input fingerprints and package hashes are
stored in the generated reports/manifest; they identify the tested working tree
rather than claiming these changes are already part of that commit.

## Acceptance and review

- No reported underruns or fatal stream errors during the ten-minute run; no
  diagnostic queue loss. No observed missing/duplicate audible clicks or freezes.
- Maximum input-to-visible-frame response ≤100 ms, excluding asynchronous device
  opening. Maximum audio-visual difference ≤50 ms in ordinary local conditions.
- Record the capture method, clock alignment, refresh rate, and measurement
  uncertainty. Capture actual rendered frames and output-loopback audio;
  callback timestamps cannot substitute for either measurement.
- Verify meter/tempo/volume/presets, minimize/restore, stop/restart, error/retry,
  close/reopen persistence, and package install/menu/reinstall/remove behavior.
- Save completed `desktop.json`, `default-output-ten-minutes.json`, and recordings
  in each `dist/evidence/ubuntu-<version>-<backend>/` directory. Copy the desktop
  template and replace each pending field with observed evidence.
- A human reviews evidence and changes scenario status to `passed`. The preparation
  command keeps `ready_to_publish` false for missing, failed, or stale evidence.

See [release procedure](../release.md) for commands and recording protocol.
