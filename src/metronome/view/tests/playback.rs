use super::*;

#[gpui_kit::test]
fn transport_waits_for_audio_before_lighting_the_indicator(cx: &mut TestAppContext) {
    let (handle, view, device) = open(cx);
    start(cx, handle);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("playback-status").label(), Some("Running"));
        assert_eq!(window.find("toggle-playback").label(), Some("Stop"));
        assert!(!view.read(cx).position(cx).is_pulse());
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
        assert!(!view.read(cx).position(cx).is_pulse());
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
            (
                state.bpm,
                state.volume,
                state.playing,
                state.opens,
                state.configures
            ),
            (121, 100, true, 1, 3)
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
        assert!(view.read(cx).audio_error(cx).is_some());
        assert!(!view.read(cx).position(cx).is_pulse());
    })
    .expect("inspect disconnected state");
    start(cx, handle);
    cx.update(|cx| assert!(view.read(cx).playback(cx) == PlaybackState::Running));
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
        assert!(view.read(cx).playback(cx) == PlaybackState::Stopped);
        assert!(!view.read(cx).position(cx).is_pulse());
    });
}
