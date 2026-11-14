#![expect(
    clippy::expect_used,
    reason = "UI test setup and dispatch must succeed."
)]

use super::super::audio::{AudioError, AudioFactory, AudioPlayback};
use super::{Meter, Metronome, PlaybackPosition, PlaybackState};
use gpui_kit::test::{TestEventExt as _, TestQueryExt as _, TestWindowExt as _};
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size,
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[path = "view/preset_controls/tests.rs"]
mod preset_tests;

#[derive(Default)]
struct DeviceState {
    bpm: u16,
    volume: u8,
    playing: bool,
    pulse: bool,
    failed: bool,
    opens: usize,
    meter: Meter,
    beat: Option<u8>,
}

struct TestPlayback(Arc<Mutex<DeviceState>>);
impl AudioPlayback for TestPlayback {
    fn configure(&self, bpm: u16, volume: u8, playing: bool, meter: Meter) {
        let mut state = self.0.lock().expect("test device state");
        if playing != state.playing
            || meter.beats() != state.meter.beats()
            || meter.unit() != state.meter.unit()
        {
            state.beat = None;
            state.pulse = false;
        }
        state.meter = meter;
        state.bpm = bpm;
        state.volume = volume;
        state.playing = playing;
    }
    fn poll(&mut self, _: Instant) -> Result<PlaybackPosition, AudioError> {
        let state = self.0.lock().expect("test device state");
        if state.failed {
            Err(AudioError::StreamFailed(
                cpal::ErrorKind::DeviceNotAvailable,
            ))
        } else {
            Ok(if state.playing {
                state.beat.map_or_else(PlaybackPosition::idle, |beat| {
                    PlaybackPosition::at_beat(beat, state.pulse)
                })
            } else {
                PlaybackPosition::idle()
            })
        }
    }
}

type TestView = (AnyWindowHandle, Entity<Metronome>, Arc<Mutex<DeviceState>>);

fn open(cx: &mut TestAppContext) -> TestView {
    let state = Arc::new(Mutex::new(DeviceState::default()));
    let device = state.clone();
    let factory: AudioFactory = Arc::new(move || {
        let mut state = device.lock().expect("open test device");
        state.failed = false;
        state.opens += 1;
        drop(state);
        Ok(Box::new(TestPlayback(device.clone())))
    });
    let (handle, view) = open_with_factory(cx, factory);
    (handle, view, state)
}

fn open_with_factory(
    cx: &mut TestAppContext,
    factory: AudioFactory,
) -> (AnyWindowHandle, Entity<Metronome>) {
    open_with_preferences(cx, factory, super::Preferences::in_memory())
}

fn open_with_preferences(
    cx: &mut TestAppContext,
    factory: AudioFactory,
    preferences: super::Preferences,
) -> (AnyWindowHandle, Entity<Metronome>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        super::init(cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(600.), px(780.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Metronome::with_preferences(window, cx, factory, preferences)),
        )
        .expect("open UI test window")
    })
}

#[gpui_kit::test]
fn preferences_survive_closing_and_reopening_including_taps_and_hidden_sounds(
    cx: &mut TestAppContext,
) {
    use super::super::settings::{Preferences, SettingsStore};
    let folder = tempfile::tempdir().expect("temporary settings");
    let store = SettingsStore::new(folder.path().join("metronomo/settings.json"));
    let state = Arc::new(Mutex::new(DeviceState::default()));
    let device = state.clone();
    let factory: AudioFactory = Arc::new(move || {
        device.lock().expect("test device").opens += 1;
        Ok(Box::new(TestPlayback(device.clone())))
    });
    let (handle, view) =
        open_with_preferences(cx, factory.clone(), Preferences::from_store(store.clone()));
    choose_menu_item(cx, handle, "beats-per-bar", "12 beats");
    for (beat, sound) in [(1, "Low"), (2, "Muted"), (4, "High"), (12, "Low")] {
        // Reveal controls beyond the initial scroll viewport through keyboard navigation.
        if beat == 12 {
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                let target = window.find(("beat-sound", 12_u32)).bounds();
                window.scroll(
                    "practice-scroll",
                    gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), -target.bottom())),
                    cx,
                );
            })
            .expect("reveal hidden sound control");
        }
        choose_menu_item(cx, handle, ("beat-sound", beat as u32), sound);
    }
    // Return to the top before choosing the time signature.
    cx.update_window(handle, |_, window, cx| {
        window.scroll(
            "practice-scroll",
            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(2000.))),
            cx,
        );
    })
    .expect("return to tempo and meter controls");
    choose_menu_item(cx, handle, "beats-per-bar", "3 beats");
    choose_menu_item(cx, handle, "beat-unit", "1/8 · Eighth note");
    start(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        let slider = ("slider", view.read(cx).volume.entity_id());
        let bounds = window.find(slider).bounds();
        window.click_at(slider, point(px(0.1), bounds.size.height / 2.), cx);
        window.click("tap-tempo", cx);
    })
    .expect("mute and start tap measurement");
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(375));
    cx.update_window(handle, |_, window, cx| {
        window.press("t", cx);
        // Close immediately, before queued save work has a chance to run.
        window.click("increase-tempo", cx);
        window.click("increase-tempo", cx);
        window.click("increase-tempo", cx);
        window.click("decrease-tempo", cx);
        assert_eq!(window.find("tempo-value").label(), Some("162 BPM"));
        window.press("ctrl-w", cx);
    })
    .expect("save the last edits while closing");
    cx.run_until_parked();
    cx.update(|cx| assert!(!cx.windows().contains(&handle)));
    assert!(!state.lock().expect("stopped device").playing);
    assert_eq!(store.load().expect("saved settings").bpm(), 162);
    let (handle, restored) = open_with_preferences(cx, factory, Preferences::from_store(store));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("tempo-value").label(), Some("162 BPM"));
        assert_eq!(window.find("volume-value").label(), Some("0%"));
        assert_eq!(window.find("current-beat").label(), Some("3 / 8"));
        assert_eq!(window.find("playback-status").label(), Some("Stopped"));
        assert_eq!(
            window.find("tap-status").label(),
            Some("Tap quarter notes · T")
        );
        assert_eq!(
            window.find(("beat-sound", 1_u32)).label(),
            Some("Beat 1: Low")
        );
        assert_eq!(
            window.find(("beat-sound", 2_u32)).label(),
            Some("Beat 2: Muted")
        );
        assert_eq!(restored.read(cx).meter.sound(11), super::BeatSound::Low);
        assert_eq!(restored.read(cx).meter.sound(3), super::BeatSound::High);
    })
    .expect("restore all preferences without starting audio");
    assert_eq!(state.lock().expect("device count").opens, 1);
    start(cx, handle);
    let state = state.lock().expect("restored output");
    assert_eq!((state.bpm, state.volume), (162, 0));
    assert_eq!(state.meter.beats(), 3);
    assert_eq!(state.meter.unit(), super::BeatUnit::Eighth);
    drop(state);
    close(cx, handle);
}

#[gpui_kit::test]
fn invalid_saved_settings_show_defaults_and_recover_after_an_edit(cx: &mut TestAppContext) {
    use super::super::settings::{Preferences, SettingsStore};
    let folder = tempfile::tempdir().expect("temporary settings");
    let path = folder.path().join("settings.json");
    std::fs::write(&path, b"{broken").expect("invalid settings");
    let store = SettingsStore::new(path.clone());
    let factory: AudioFactory = Arc::new(|| Err(AudioError::NoDevice));
    let (handle, _) = open_with_preferences(cx, factory, Preferences::from_store(store.clone()));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("tempo-value").label(), Some("120 BPM"));
        assert_eq!(
            window.find("settings-message").label(),
            Some("Couldn’t load saved settings. Using defaults.")
        );
    })
    .expect("inspect recoverable load failure");
    assert_eq!(std::fs::read(&path).expect("original file"), b"{broken");
    cx.update_window(handle, |_, window, cx| window.click("increase-tempo", cx))
        .expect("edit a recovered preference");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("settings-message").is_none());
    })
    .expect("clear the error after a successful save");
    assert_eq!(store.load().expect("repaired settings").bpm(), 121);
    close(cx, handle);
}

#[gpui_kit::test]
fn save_failures_remain_visible_and_retry_on_the_next_change(cx: &mut TestAppContext) {
    use super::super::settings::{Preferences, SettingsStore};
    let folder = tempfile::tempdir().expect("temporary settings");
    let blocked = folder.path().join("blocked");
    std::fs::write(&blocked, b"not a folder").expect("unwritable config parent");
    let store = SettingsStore::new(blocked.join("settings.json"));
    let factory: AudioFactory = Arc::new(|| Err(AudioError::NoDevice));
    let (handle, _) = open_with_preferences(cx, factory, Preferences::from_store(store.clone()));
    cx.update_window(handle, |_, window, cx| window.click("increase-tempo", cx))
        .expect("attempt an autosave");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("settings-message").label(),
            Some("Couldn’t save settings. Check access to your configuration folder.")
        );
        assert_eq!(window.find("tempo-value").label(), Some("121 BPM"));
    })
    .expect("show a save failure without discarding the live setting");
    std::fs::remove_file(blocked).expect("restore write access");
    cx.update_window(handle, |_, window, cx| window.click("increase-tempo", cx))
        .expect("retry on another change");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("settings-message").is_none());
    })
    .expect("recover from a temporary save error");
    assert_eq!(store.load().expect("saved after retry").bpm(), 122);
    close(cx, handle);
}

#[gpui_kit::test]
fn system_quit_flushes_the_latest_snapshot_without_requiring_a_live_window(
    cx: &mut TestAppContext,
) {
    use super::super::settings::{Preferences, SettingsStore};
    let folder = tempfile::tempdir().expect("temporary settings");
    let store = SettingsStore::new(folder.path().join("settings.json"));
    let factory: AudioFactory = Arc::new(|| Err(AudioError::NoDevice));
    let (handle, _) = open_with_preferences(cx, factory, Preferences::from_store(store.clone()));
    cx.update_window(handle, |_, window, cx| {
        window.click("increase-tempo", cx);
        window.click("increase-tempo", cx);
    })
    .expect("edit immediately before system quit");
    cx.update(|cx| cx.shutdown());
    assert_eq!(store.load().expect("shutdown snapshot").bpm(), 122);
}

#[gpui_kit::test]
fn closing_without_edits_does_not_overwrite_an_invalid_settings_file(cx: &mut TestAppContext) {
    use super::super::settings::{Preferences, SettingsStore};
    let folder = tempfile::tempdir().expect("temporary settings");
    let path = folder.path().join("settings.json");
    std::fs::write(&path, b"{broken").expect("invalid settings");
    let factory: AudioFactory = Arc::new(|| Err(AudioError::NoDevice));
    let (handle, _) = open_with_preferences(
        cx,
        factory,
        Preferences::from_store(SettingsStore::new(path.clone())),
    );
    close(cx, handle);
    cx.update(|cx| cx.shutdown());
    assert_eq!(std::fs::read(path).expect("original settings"), b"{broken");
}

fn close(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.press("ctrl-w", cx))
        .expect("close the settings test window");
    cx.run_until_parked();
}

fn start(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("toggle-playback", cx);
    })
    .expect("start playback");
    cx.run_until_parked();
}

fn refresh_audio(cx: &mut TestAppContext) {
    cx.background_executor
        .advance_clock(Duration::from_millis(16));
    cx.run_until_parked();
}

#[gpui_kit::test]
fn adjusts_tempo_with_pointer_and_keyboard(cx: &mut TestAppContext) {
    let (handle, view, _) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("tempo-value").label(), Some("120 BPM"));
        window.click("increase-tempo", cx);
        assert_eq!(window.find("tempo-value").label(), Some("121 BPM"));
        window.press("up", cx);
        assert_eq!(window.find("tempo-value").label(), Some("122 BPM"));
        window.click("decrease-tempo", cx);
        window.press("down", cx);
        assert_eq!(view.read(cx).bpm(cx), 120.);
    })
    .expect("interact with tempo controls");
}

#[gpui_kit::test]
fn tap_tempo_combines_clicks_shortcuts_and_focused_button_activation(cx: &mut TestAppContext) {
    let (handle, view, device) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("tap-tempo", cx);
        assert_eq!(window.find("tap-status").label(), Some("Tap again · T"));
        assert_eq!(window.find("tempo-value").label(), Some("120 BPM"));
    })
    .expect("start tapping with the button");
    cx.run_until_parked();
    for (delay, key, bpm) in [(400, "t", 150.), (600, "enter", 120.), (500, "space", 120.)] {
        cx.background_executor
            .advance_clock(Duration::from_millis(delay));
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            if key == "enter" {
                for _ in 0..5 {
                    window.press("tab", cx);
                }
                assert_eq!(window.find("tap-tempo").focused(), Some(true));
            }
            window.press(key, cx);
            assert_eq!(view.read(cx).bpm(cx), bpm, "after {key}");
            assert_eq!(window.find("tap-status").label(), Some("Keep tapping · T"));
            assert_eq!(window.find("playback-status").label(), Some("Stopped"));
        })
        .expect("measure taps through each input path");
        cx.run_until_parked();
    }
    assert_eq!(device.lock().expect("device state").opens, 0);
}

#[gpui_kit::test]
fn held_tap_shortcut_counts_once_until_released(cx: &mut TestAppContext) {
    let (handle, _, _) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.key_down("t", false, cx);
    })
    .expect("press tap shortcut");
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.key_down("t", true, cx);
        assert_eq!(window.find("tap-status").label(), Some("Tap again · T"));
        window.key_up("t", cx);
    })
    .expect("ignore key repeat");
    cx.background_executor
        .advance_clock(Duration::from_millis(500));
    cx.update_window(handle, |_, window, cx| {
        window.press("t", cx);
        assert_eq!(window.find("tempo-value").label(), Some("60 BPM"));
    })
    .expect("measure the next physical tap");
}

#[gpui_kit::test]
fn tap_sequence_expires_after_inactivity_and_keeps_the_last_bpm(cx: &mut TestAppContext) {
    let (handle, _, _) = open(cx);
    for delay in [0, 400] {
        cx.background_executor
            .advance_clock(Duration::from_millis(delay));
        cx.update_window(handle, |_, window, cx| window.press("t", cx))
            .expect("tap a tempo");
        cx.run_until_parked();
    }
    // The cancelled first timer must not clear a more recent tap sequence.
    cx.background_executor
        .advance_clock(Duration::from_millis(2999));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("tap-status").label(), Some("Keep tapping · T"));
    })
    .expect("keep taps until their deadline");
    cx.background_executor
        .advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("tap-status").label(),
            Some("Tap quarter notes · T")
        );
        assert_eq!(window.find("tempo-value").label(), Some("150 BPM"));
        window.press("t", cx);
        assert_eq!(window.find("tap-status").label(), Some("Tap again · T"));
        assert_eq!(window.find("tempo-value").label(), Some("150 BPM"));
    })
    .expect("begin a fresh sequence after the pause");
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(750));
    cx.update_window(handle, |_, window, cx| {
        window.press("t", cx);
        assert_eq!(window.find("tempo-value").label(), Some("80 BPM"));
    })
    .expect("measure the fresh sequence");
}

#[gpui_kit::test]
fn manual_tempo_edits_reset_taps_and_menus_do_not_receive_the_shortcut(cx: &mut TestAppContext) {
    let (handle, view, _) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.press("t", cx);
        window.click("increase-tempo", cx);
        assert_eq!(
            window.find("tap-status").label(),
            Some("Tap quarter notes · T")
        );
        window.press("t", cx);
        let slider = ("slider", view.read(cx).tempo.entity_id());
        let bounds = window.find(slider).bounds();
        window.click_at(
            slider,
            point(bounds.size.width / 2., bounds.size.height / 2.),
            cx,
        );
    })
    .expect("edit tempo manually");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("tap-status").label(),
            Some("Tap quarter notes · T")
        );
        window.click("beat-unit", cx);
    })
    .expect("open the note unit menu");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.press("t", cx);
        assert_eq!(view.read(cx).tap_tempo.taps(), 0);
        assert!(window.try_find("popup-menu").is_some());
        window.press("escape", cx);
    })
    .expect("preserve menu keyboard handling");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.press("t", cx);
        assert_eq!(window.find("tap-status").label(), Some("Tap again · T"));
    })
    .expect("restore the shortcut after closing the menu");
}

#[gpui_kit::test]
fn tap_tempo_updates_running_audio_in_quarter_notes_without_restarting_the_bar(
    cx: &mut TestAppContext,
) {
    let (handle, view, device) = open(cx);
    choose_menu_item(cx, handle, "beat-unit", "1/8 · Eighth note");
    start(cx, handle);
    {
        let mut state = device.lock().expect("device state");
        state.beat = Some(2);
        state.pulse = true;
    }
    refresh_audio(cx);
    cx.update_window(handle, |_, window, cx| {
        window.click("tap-tempo", cx);
    })
    .expect("start tapping during playback");
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(600));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.press("t", cx);
        assert_eq!(window.find("tempo-value").label(), Some("100 BPM"));
        assert_eq!(window.find("current-beat").label(), Some("Beat 3 of 4"));
        assert_eq!(view.read(cx).tempo.read(cx).value().end(), 100.);
        assert_eq!(window.find("playback-status").label(), Some("Running"));
    })
    .expect("apply measured tempo without moving the playhead");
    let state = device.lock().expect("device state");
    assert_eq!(state.bpm, 100);
    assert_eq!(state.meter.unit(), super::BeatUnit::Eighth);
    assert_eq!(state.beat, Some(2));
    assert_eq!(state.volume, 60);
    assert_eq!(state.opens, 1);
    assert!(state.playing);
}

#[gpui_kit::test]
fn slider_updates_display_and_enforces_tempo_limits(cx: &mut TestAppContext) {
    let (handle, view, _) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let slider_id = ("slider", view.read(cx).tempo.entity_id());
        let bounds = window.find(slider_id).bounds();
        window.click_at(
            slider_id,
            point(bounds.size.width - px(0.1), bounds.size.height / 2.),
            cx,
        );
    })
    .expect("set maximum tempo");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("tempo-value").label(), Some("240 BPM"));
        assert_eq!(window.find("increase-tempo").disabled(), Some(true));
        window.click("increase-tempo", cx);
        window.press("up", cx);
        assert_eq!(view.read(cx).bpm(cx), 240.);
        let slider_id = ("slider", view.read(cx).tempo.entity_id());
        let bounds = window.find(slider_id).bounds();
        window.click_at(slider_id, point(px(0.1), bounds.size.height / 2.), cx);
    })
    .expect("check maximum and set minimum tempo");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("tempo-value").label(), Some("40 BPM"));
        assert_eq!(window.find("decrease-tempo").disabled(), Some(true));
        window.click("decrease-tempo", cx);
        window.press("down", cx);
        assert_eq!(view.read(cx).bpm(cx), 40.);
    })
    .expect("check minimum tempo");
}

#[gpui_kit::test]
fn transport_waits_for_audio_before_lighting_the_indicator(cx: &mut TestAppContext) {
    let (handle, view, device) = open(cx);
    start(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("playback-status").label(), Some("Running"));
        assert_eq!(window.find("toggle-playback").label(), Some("Stop"));
        assert!(!view.read(cx).position.is_pulse());
    })
    .expect("inspect running state");
    {
        let mut state = device.lock().expect("device state");
        state.pulse = true;
        state.beat = Some(0);
    }
    refresh_audio(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("beat-indicator").label(), Some("Beat"));
        window.press("space", cx);
        assert_eq!(window.find("playback-status").label(), Some("Stopped"));
        assert!(!view.read(cx).position.is_pulse());
        window.press("enter", cx);
        assert_eq!(window.find("playback-status").label(), Some("Running"));
        window.click("toggle-playback", cx);
        assert_eq!(window.find("toggle-playback").label(), Some("Start"));
    })
    .expect("stop and restart through keyboard and pointer");
    assert!(!device.lock().expect("device state").playing);
}

#[gpui_kit::test]
fn bpm_and_volume_updates_reach_the_running_output_without_reopening_it(cx: &mut TestAppContext) {
    let (handle, view, device) = open(cx);
    start(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("increase-tempo", cx);
        let slider = ("slider", view.read(cx).volume.entity_id());
        let bounds = window.find(slider).bounds();
        window.click_at(
            slider,
            point(bounds.size.width - px(0.1), bounds.size.height / 2.),
            cx,
        );
    })
    .expect("change tempo and volume");
    cx.run_until_parked();
    {
        let state = device.lock().expect("device state");
        assert_eq!(
            (state.bpm, state.volume, state.playing, state.opens),
            (121, 100, true, 1)
        );
    }
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("down", cx);
        assert_eq!(window.find("volume-value").label(), Some("95%"));
        assert_eq!(window.find("tempo-value").label(), Some("121 BPM"));
        let slider = ("slider", view.read(cx).volume.entity_id());
        let bounds = window.find(slider).bounds();
        window.click_at(slider, point(px(0.1), bounds.size.height / 2.), cx);
    })
    .expect("adjust volume with keyboard and mute");
    cx.run_until_parked();
    let state = device.lock().expect("device state");
    assert_eq!(state.volume, 0);
    assert!(state.playing);
}

#[gpui_kit::test]
fn disconnected_output_stops_playback_and_can_be_retried(cx: &mut TestAppContext) {
    let (handle, view, device) = open(cx);
    start(cx, handle);
    device.lock().expect("device state").failed = true;
    refresh_audio(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("playback-status").label(), Some("Unavailable"));
        assert_eq!(window.find("toggle-playback").label(), Some("Start"));
        assert!(view.read(cx).audio.is_none());
        assert!(!view.read(cx).position.is_pulse());
    })
    .expect("inspect disconnected state");
    start(cx, handle);
    cx.update(|cx| assert!(view.read(cx).playback == PlaybackState::Running));
    assert_eq!(device.lock().expect("device state").opens, 2);
}

#[gpui_kit::test]
fn stopping_during_output_initialization_does_not_start_playback_later(cx: &mut TestAppContext) {
    let (handle, view, _) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("toggle-playback", cx);
        assert_eq!(window.find("playback-status").label(), Some("Starting"));
        window.click("toggle-playback", cx);
    })
    .expect("cancel pending startup");
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(view.read(cx).playback == PlaybackState::Stopped);
        assert!(!view.read(cx).position.is_pulse());
    });
}

#[gpui_kit::test]
fn tab_and_arrow_keys_keep_volume_and_playback_reachable(cx: &mut TestAppContext) {
    let (handle, _, _) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("tab", cx);
        assert_eq!(window.find("open-presets").focused(), Some(true));
        window.press("tab", cx);
        assert_eq!(window.find("restore-defaults").focused(), Some(true));
        window.press("tab", cx);
        assert_eq!(window.find("decrease-tempo").focused(), Some(true));
        window.press("enter", cx);
        assert_eq!(window.find("tempo-value").label(), Some("119 BPM"));
        window.press("tab", cx);
        assert_eq!(window.find("increase-tempo").focused(), Some(true));
        window.press("tab", cx);
        assert_eq!(window.find("tap-tempo").focused(), Some(true));
        for _ in 0..7 {
            window.press("tab", cx);
        }
        assert_eq!(window.find("volume-controls").focused(), Some(true));
        window.press("right", cx);
        assert_eq!(window.find("volume-value").label(), Some("65%"));
        assert_eq!(window.find("tempo-value").label(), Some("119 BPM"));
        window.press("tab", cx);
        assert_eq!(window.find("toggle-playback").focused(), Some(true));
        window.press("enter", cx);
    })
    .expect("operate volume and playback through keyboard");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("playback-status").label(), Some("Running"));
    })
    .expect("inspect started output");
}

#[gpui_kit::test]
fn unavailable_audio_and_larger_text_fit_the_minimum_window(cx: &mut TestAppContext) {
    use gpui_kit::component::{Theme, ThemeMode};
    let (handle, _) = open_with_factory(cx, Arc::new(|| Err(AudioError::NoDevice)));
    start(cx, handle);
    cx.simulate_window_resize(handle, size(px(480.), px(720.)));
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        for font_size in [16., 20.] {
            cx.update(|cx| {
                Theme::change(mode, None, cx);
                Theme::update(cx, |theme| theme.font_size = px(font_size));
            });
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                let minus = window.find("decrease-tempo").bounds();
                let plus = window.find("increase-tempo").bounds();
                let value = window.find("tempo-value").bounds();
                let range = window.find("tempo-range").bounds();
                let volume = window.find("volume-controls").bounds();
                let toggle = window.find("toggle-playback").bounds();
                let message = window.find("audio-message").bounds();
                assert_eq!(window.find("playback-status").label(), Some("Unavailable"));
                assert!(minus.left() >= px(0.) && plus.right() <= px(480.));
                assert!((f32::from(minus.center().y - value.center().y)).abs() <= 1.);
                assert!(range.bottom() <= volume.top());
                assert!(volume.bottom() <= toggle.top());
                assert!(message.bottom() <= px(720.), "message: {message:?}");
                assert!(
                    window
                        .find("audio-message")
                        .label()
                        .is_some_and(|message| message.contains("No audio output"))
                );
            })
            .expect("inspect error layout at minimum size");
        }
    }
}

#[gpui_kit::test]
fn clicking_the_volume_thumb_transfers_keyboard_focus(cx: &mut TestAppContext) {
    let (handle, view, _) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let slider = ("slider", view.read(cx).volume.entity_id());
        window.within(slider).click(("slider-thumb", 0_u32), cx);
        assert_eq!(window.find("volume-controls").focused(), Some(true));
        window.press("left", cx);
        assert_eq!(window.find("volume-value").label(), Some("55%"));
        assert_eq!(window.find("tempo-value").label(), Some("120 BPM"));
    })
    .expect("focus volume with its thumb");
}

fn choose_menu_item(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    trigger: impl Into<gpui_kit::ElementId>,
    label: &str,
) {
    let trigger = trigger.into();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(trigger, cx);
    })
    .expect("open control menu");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let target = window.find_by_label(label);
        let id = target.path().last().expect("menu item identity").clone();
        window.within("popup-menu").click(id, cx);
    })
    .expect("choose menu item");
    cx.run_until_parked();
}

#[gpui_kit::test]
fn meter_and_each_beat_sound_can_be_changed_through_native_menus(cx: &mut TestAppContext) {
    use super::super::meter::{BeatSound, BeatUnit};
    let (handle, view, device) = open(cx);
    start(cx, handle);
    choose_menu_item(cx, handle, "beats-per-bar", "3 beats");
    choose_menu_item(cx, handle, "beat-unit", "1/8 · Eighth note");
    choose_menu_item(cx, handle, ("beat-sound", 1_u32), "Low");
    choose_menu_item(cx, handle, ("beat-sound", 2_u32), "Muted");
    choose_menu_item(cx, handle, ("beat-sound", 3_u32), "High");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("current-beat").label(), Some("3 / 8"));
        assert_eq!(
            window.find(("beat-sound", 1_u32)).label(),
            Some("Beat 1: Low")
        );
        assert_eq!(
            window.find(("beat-sound", 2_u32)).label(),
            Some("Beat 2: Muted")
        );
        assert!(window.try_find(("beat-sound", 4_u32)).is_none());
        assert_eq!(view.read(cx).meter.unit(), BeatUnit::Eighth);
        assert!(view.read(cx).playback == PlaybackState::Running);
    })
    .expect("inspect meter and sound choices");
    let state = device.lock().expect("device state");
    assert_eq!(state.meter.beats(), 3);
    assert_eq!(state.meter.sound(0), BeatSound::Low);
    assert_eq!(state.meter.sound(1), BeatSound::Muted);
    assert_eq!(state.meter.sound(2), BeatSound::High);
    assert_eq!(state.opens, 1);
}

#[gpui_kit::test]
fn playhead_tracks_muted_beats_between_pulses_and_resets_on_stop(cx: &mut TestAppContext) {
    let (handle, _, device) = open(cx);
    choose_menu_item(cx, handle, ("beat-sound", 2_u32), "Muted");
    start(cx, handle);
    {
        let mut state = device.lock().expect("device state");
        state.beat = Some(1);
        state.pulse = true;
    }
    refresh_audio(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("current-beat").label(), Some("Beat 2 of 4"));
        assert_eq!(
            window.find(("beat-sound", 2_u32)).label(),
            Some("Beat 2: Muted, playing")
        );
        assert_eq!(
            window.find(("beat-sound", 1_u32)).label(),
            Some("Beat 1: High")
        );
    })
    .expect("inspect silent beat");
    device.lock().expect("device state").pulse = false;
    refresh_audio(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("current-beat").label(), Some("Beat 2 of 4"));
        assert_eq!(
            window.find(("beat-sound", 2_u32)).label(),
            Some("Beat 2: Muted, playing")
        );
        window.click("toggle-playback", cx);
        assert_eq!(window.find("current-beat").label(), Some("4 / 4"));
        assert_eq!(
            window.find(("beat-sound", 2_u32)).label(),
            Some("Beat 2: Muted")
        );
    })
    .expect("retain playhead until the next beat and clear on stop");
}

#[gpui_kit::test]
fn beat_sound_menus_support_keyboard_selection_and_escape_returns_focus(cx: &mut TestAppContext) {
    let (handle, _, _) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(("beat-sound", 1_u32), cx);
    })
    .expect("open sound menu");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find_by_label("High").checked().unwrap_or(false));
        window.press("escape", cx);
    })
    .expect("dismiss sound menu");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find(("beat-sound", 1_u32)).focused(), Some(true));
        window.press("enter", cx);
    })
    .expect("reopen menu with keyboard");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("down", cx);
        window.press("down", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .expect("select low pitch with keyboard");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find(("beat-sound", 1_u32)).label(),
            Some("Beat 1: Low")
        );
        assert_eq!(window.find("playback-status").label(), Some("Stopped"));
    })
    .expect("inspect keyboard sound selection");
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
#[ignore = "Requires a real GPU renderer and exports interface previews into the temporary directory."]
fn render_meter_previews() {
    use super::super::meter::BeatSound;
    use gpui_kit::{
        HeadlessAppContext,
        assets::Assets,
        component::{Theme, ThemeMode},
    };
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        super::init(cx);
    });
    for (name, mode, width, height, font_size) in [
        ("light", ThemeMode::Light, 600., 780., 16.),
        ("dark", ThemeMode::Dark, 600., 780., 16.),
        ("zoom", ThemeMode::Dark, 480., 720., 20.),
    ] {
        let state = Arc::new(Mutex::new(DeviceState::default()));
        let device = state.clone();
        let factory: AudioFactory = Arc::new(move || Ok(Box::new(TestPlayback(device.clone()))));
        let (handle, _) = cx.update(|cx| {
            Theme::change(mode, None, cx);
            Theme::update(cx, |theme| theme.font_size = px(font_size));
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        Default::default(),
                        size(px(width), px(height)),
                    ))),
                    show: false,
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        let mut view = Metronome::with_audio_factory(window, cx, factory);
                        view.meter = view.meter.with_beats(12);
                        for beat in 0..12 {
                            view.meter = view
                                .meter
                                .with_sound(beat, BeatSound::ALL[usize::from(beat % 4)]);
                        }
                        view
                    })
                },
            )
            .expect("open preview window")
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click("toggle-playback", cx);
        })
        .expect("start preview session");
        cx.run_until_parked();
        {
            let mut device = state.lock().expect("preview device");
            device.beat = Some(2);
            device.pulse = true;
        }
        cx.advance_clock(Duration::from_millis(16));
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("current-beat").label(), Some("Beat 3 of 12"));
            let grid = window.find("beat-grid").bounds();
            let button = window.find("toggle-playback").bounds();
            assert!(grid.left() >= px(0.) && grid.right() <= px(width));
            assert!(button.bottom() < px(height));
        })
        .expect("inspect rendered meter");
        cx.capture_screenshot(handle)
            .expect("render preview")
            .save(std::env::temp_dir().join(format!("metronomo-meter-{name}.png")))
            .expect("save preview");
        if name == "dark" {
            cx.update_window(handle, |_, window, cx| window.click("tap-tempo", cx))
                .expect("start preview tap sequence");
            cx.run_until_parked();
            cx.advance_clock(Duration::from_millis(375));
            cx.update_window(handle, |_, window, cx| {
                window.click("tap-tempo", cx);
                assert_eq!(window.find("tempo-value").label(), Some("160 BPM"));
                assert_eq!(window.find("tap-status").label(), Some("Keep tapping · T"));
            })
            .expect("measure preview tempo");
            cx.capture_screenshot(handle)
                .expect("render active tap tempo")
                .save(std::env::temp_dir().join("metronomo-tap-active.png"))
                .expect("save tap tempo preview");
            cx.update_window(handle, |_, window, cx| {
                window.click(("beat-sound", 3_u32), cx)
            })
            .expect("open sound picker");
            cx.run_until_parked();
            cx.capture_screenshot(handle)
                .expect("render sound menu")
                .save(std::env::temp_dir().join("metronomo-meter-menu.png"))
                .expect("save sound menu");
        }
        cx.update_window(handle, |_, window, _| window.remove_window())
            .expect("close preview");
    }
}

#[gpui_kit::test]
fn all_twelve_beat_controls_remain_reachable_in_a_small_zoomed_window(cx: &mut TestAppContext) {
    use gpui_kit::component::Theme;
    let (handle, view, _) = open(cx);
    cx.update_window(handle, |_, window, _| window.activate_window())
        .expect("activate test window");
    choose_menu_item(cx, handle, "beats-per-bar", "12 beats");
    cx.simulate_window_resize(handle, size(px(480.), px(720.)));
    cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(20.)));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        for _ in 0..13 {
            window.press("tab", cx);
        }
        assert_eq!(window.find(("beat-sound", 12_u32)).focused(), Some(true));
    })
    .expect("navigate to the last beat");
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(16));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        assert!(
            window.find(("beat-sound", 12_u32)).visible(),
            "offset={:?}, viewport={:?}, target={:?}",
            view.read(cx).practice_scroll.offset(),
            view.read(cx).practice_scroll.bounds(),
            window.find(("beat-sound", 12_u32)).bounds()
        );
    })
    .expect("scroll the last beat into view");
    cx.update_window(handle, |_, window, cx| {
        for _ in 0..14 {
            window.press("shift-tab", cx);
        }
    })
    .expect("navigate back to tempo");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        assert_eq!(window.find("tap-tempo").focused(), Some(true));
        assert!(window.find("tap-tempo").visible());
        window.press("shift-tab", cx);
        assert_eq!(window.find("increase-tempo").focused(), Some(true));
        assert!(window.find("increase-tempo").visible());
    })
    .expect("reveal the tempo controls when navigating back");
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "Opens a real desktop window to minimize, restore, and close it through the title bar."]
fn native_window_controls_smoke() {
    use gpui_kit::{WindowDecorations, component::TitleBar};
    use std::{cell::Cell, rc::Rc};
    let closed = Rc::new(Cell::new(false));
    let minimized = Rc::new(Cell::new(false));
    let did_close = closed.clone();
    let did_minimize = minimized.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            super::init(cx);
            let (handle, _) = gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(600.), px(780.)),
                        cx,
                    ))),
                    window_decorations: Some(WindowDecorations::Client),
                    ..TitleBar::window_options()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        Metronome::with_audio_factory(
                            window,
                            cx,
                            Arc::new(|| Err(AudioError::NoDevice)),
                        )
                    })
                },
            )
            .expect("open native control test");
            cx.on_window_closed(move |cx, _| {
                did_close.set(true);
                cx.quit();
            })
            .detach();
            cx.activate(true);
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                cx.update_window(handle, |_, window, cx| {
                    window.render_frame(cx);
                    assert!(window.find("minimize").visible());
                    assert!(window.find("close").visible());
                    window.click("minimize", cx);
                })
                .expect("minimize through the title bar");
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                cx.update_window(handle, |_, window, _| {
                    assert!(
                        !window.is_window_active(),
                        "minimized window must lose activation"
                    );
                    did_minimize.set(true);
                    window.activate_window();
                })
                .expect("restore the minimized window");
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                cx.update_window(handle, |_, window, cx| window.click("close", cx))
                    .expect("close through the title bar");
            })
            .detach();
            cx.spawn(async move |cx| {
                cx.background_executor().timer(Duration::from_secs(8)).await;
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    assert!(minimized.get(), "native minimize flow did not finish");
    assert!(closed.get(), "native close flow did not finish");
}
