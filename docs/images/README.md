# Application screenshots

These PNGs are captured from Metronomo's production GPUI views using the real
GPU renderer. They show example settings and presets in an isolated, in-memory
session. Playback indicators use the deterministic test audio session; the
capture does not play audio or read/write personal settings. Native desktop
window decorations are not included.

All four images are 1200 × 1560 pixels, rendered at a 600 × 780 logical window
size with a 2× rendering scale. The files are copied directly from the capture
output, without retouching.

## Regenerate

From the repository root, with a working GPU renderer and native dependencies:

```sh
cargo test --locked --offline render_ -- --ignored --test-threads=1 --nocapture
```

The tests write PNGs to the operating system's temporary directory (`/tmp` on
Linux). Copy the following outputs to this directory:

| Generated file | Repository file |
| --- | --- |
| `metronomo-meter-dark-default.png` | `metronomo-dark.png` |
| `metronomo-meter-light-default.png` | `metronomo-light.png` |
| `metronomo-meter-dark.png` | `metronomo-twelve-beats.png` |
| `metronomo-presets-dark-ready.png` | `metronomo-presets.png` |

Review all captures before replacing the images, especially text, clipping,
theme colors, active-beat state, and the presets dialog. Regenerate these files
when the interface changes; do not edit screenshots to represent a different UI.
