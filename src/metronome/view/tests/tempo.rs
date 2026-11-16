use super::*;

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
        assert_eq!(view.read(cx).session.read(cx).taps(), 0);
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
