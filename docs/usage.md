# Using Metronomo

[Back to the project](../README.md)

Adjust the tempo, choose a time signature, and press **Start** to practice. This guide describes the controls, saved settings, and presets.

## Keyboard controls

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

## Tap Tempo

Click **Tap tempo** or press **T** in time with quarter notes (1/4), regardless of the selected beat unit. The first tap starts measurement; the second sets the BPM. Further taps smooth the estimate using the last four intervals, rounded to a whole BPM and limited to 40–240 BPM. For example, taps 500 ms apart set 120 BPM.

After three seconds without tapping, the next tap starts a fresh sequence while preserving the current BPM. Manual tempo edits also clear the sequence. Accidental taps less than 100 ms apart and keyboard auto-repeat do not contribute to the estimate. Tap Tempo works while stopped or playing; it updates the running audio without restarting the bar or moving the audio-driven visual playhead.

## Meter and beat sounds

Use **Beats per bar** to choose the number of beats and **Beat unit** to choose the denominator. BPM always refers to quarter notes (1/4). The selected note unit scales each numbered beat's duration: at 120 BPM, half notes (1/2) last 1 second, quarter notes (1/4) last 500 ms, eighth notes (1/8) last 250 ms, and sixteenth notes (1/16) last 125 ms. For example, 6/8 produces six eighth-note clicks per bar at twice the displayed quarter-note BPM; it does not automatically group them into two dotted-quarter beats.

Each block shows a large beat number and its sound. Up to four beats occupy one row; longer meters use four columns in reading order. Select a numbered beat to open its native sound menu: High, Medium, Low, or Muted. The default pattern uses High on beat 1 and Medium on the remaining beats. Beat 1 has a stronger amplitude with any audible timbre; selecting Muted takes precedence over its accent. Muted beats still advance the count and visual indicator.

The star marks beat 1 independently of the playhead. The active beat retains its amber border, fill, and Playing marker between pulses, and the counter shows its position in the bar. Changing the time signature starts a new bar on the audio callback; changing a sound preserves the current beat progression. Shrinking and expanding a bar preserves the sound choices of hidden beats, including across application restarts.

## Saved settings and window controls

BPM (including Tap Tempo changes), volume, beats per bar, beat unit, and all twelve sound slots save automatically when edited. On startup, the saved values are restored before the window opens. Playback always starts stopped; tap history and the current playhead are temporary.

Preferences use a versioned `settings.json` file in the user's configuration directory:

| Platform | Location |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/metronomo/settings.json`, or `~/.config/metronomo/settings.json` |
| macOS | `~/Library/Application Support/metronomo/settings.json` |
| Windows | `%APPDATA%\metronomo\settings.json` |

File reads and writes run off the UI and audio threads. Writes are serialized, coalesce pending changes, and atomically replace the previous file from a temporary file in the same directory. A missing file uses defaults. Invalid, unsupported, or unreadable settings show an inline warning and use defaults; the original file remains untouched until a setting is changed. A failed save leaves the live controls usable and retries on the next edit or when closing. Read/write errors also appear in the terminal with diagnostic details.

Use the title bar to drag, minimize, maximize/restore, or close the window. It follows the platform's supported controls; macOS uses its native window buttons, and Linux defers to system decorations if the compositor selects them. Minimizing keeps audio playing. Closing stops playback and waits for pending writes before removing the window; the last window exits the app.

## Restore defaults and presets

Select **Restore defaults** to return to **120 BPM**, **60% volume**, **4/4**, and the default sounds: High on beat 1 and Medium on all remaining slots. This resets all twelve sound slots and the Tap Tempo sequence, and updates the saved file automatically. Existing presets remain available. Playback keeps its current running or stopped state.

Open **Presets…**, enter a name, and select **Save preset** or press Enter. Each preset captures BPM, volume, time signature, and all twelve beat-sound choices, including hidden beats. Presets are snapshots: later metronome adjustments leave their saved configurations intact.

Select **Load** to apply a preset and return to the main controls. Loading updates the live audio and saved current configuration while preserving the current playback state. Each preset's **Actions** menu offers **Rename…** and **Delete**. A deletion offers **Undo delete** while the presets dialog remains open; closing the dialog dismisses that undo opportunity. Deleting a preset leaves the metronome's current configuration in place.

Names are trimmed, unique without regard to case, and limited to 64 characters. The library holds up to 64 presets. Name validation and file-save failures appear inside the dialog. Settings and presets are saved together in `settings.json` using the existing atomic write path, and older settings files load with an empty preset library.

## Troubleshooting

- If Start reports an output failure, select a working default output in your desktop sound settings, connect speakers/headphones, and press Start again. The app does not select devices or restart the system audio server.
- If the window cannot open, check that a graphical session and Vulkan-capable driver are available. `libvulkan1` is the loader; a working GPU driver is still required. Headless containers do not establish desktop compatibility.
- Callback underruns, route changes, realtime-scheduling denials, and discarded events are counted internally and logged from the consumer thread. No logging or report-file I/O runs inside the audio callback. Absence of reported underruns is not proof of glitch-free physical output.
- If settings fail to load or save, the inline warning identifies the failure. Check permissions on the configuration directory; do not remove your settings to troubleshoot an unrelated audio failure.
