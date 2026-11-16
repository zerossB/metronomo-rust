use super::*;

#[gpui_kit::test]
fn meter_and_each_beat_sound_can_be_changed_through_native_menus(cx: &mut TestAppContext) {
    use crate::metronome::meter::{BeatSound, BeatUnit};
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
            Some("Beat 1: Low, accented")
        );
        assert_eq!(
            window.find(("beat-sound", 2_u32)).label(),
            Some("Beat 2: Muted")
        );
        assert!(window.try_find(("beat-sound", 4_u32)).is_none());
        assert_eq!(view.read(cx).meter(cx).unit(), BeatUnit::Eighth);
        assert!(view.read(cx).playback(cx) == PlaybackState::Running);
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
            window.find(("beat-playing", 2_u32)).label(),
            Some("Playing")
        );
        assert_eq!(
            window.find(("beat-sound", 2_u32)).label(),
            Some("Beat 2: Muted, playing")
        );
        assert_eq!(
            window.find(("beat-sound", 1_u32)).label(),
            Some("Beat 1: High, accented")
        );
    })
    .expect("inspect silent beat");
    device.lock().expect("device state").pulse = false;
    refresh_audio(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("current-beat").label(), Some("Beat 2 of 4"));
        assert_eq!(
            window.find(("beat-playing", 2_u32)).label(),
            Some("Playing")
        );
        assert_eq!(
            window.find(("beat-sound", 2_u32)).label(),
            Some("Beat 2: Muted, playing")
        );
        window.click("toggle-playback", cx);
        assert_eq!(window.find("current-beat").label(), Some("4 / 4"));
        assert_eq!(
            window.find(("beat-playing", 2_u32)).label(),
            Some("Inactive")
        );
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
            Some("Beat 1: Low, accented")
        );
        assert_eq!(window.find("playback-status").label(), Some("Stopped"));
    })
    .expect("inspect keyboard sound selection");
}
