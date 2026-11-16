use super::*;

#[gpui_kit::test]
fn preferences_survive_closing_and_reopening_including_taps_and_hidden_sounds(
    cx: &mut TestAppContext,
) {
    use crate::metronome::settings::{Preferences, SettingsStore};
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
        assert_eq!(restored.read(cx).meter(cx).sound(11), super::BeatSound::Low);
        assert_eq!(restored.read(cx).meter(cx).sound(3), super::BeatSound::High);
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
    use crate::metronome::settings::{Preferences, SettingsStore};
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
    use crate::metronome::settings::{Preferences, SettingsStore};
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
    use crate::metronome::settings::{Preferences, SettingsStore};
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
    use crate::metronome::settings::{Preferences, SettingsStore};
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
