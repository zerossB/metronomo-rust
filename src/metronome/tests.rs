#![expect(
    clippy::expect_used,
    reason = "Tests fail immediately when window setup or UI dispatch fails."
)]

use super::Metronome;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size,
};
use std::time::Duration;

fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Metronome>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        super::init(cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(480.), px(440.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Metronome::new(window, cx)),
        )
        .expect("open metronome test window")
    })
}

#[gpui_kit::test]
fn adjusts_tempo_with_pointer_and_keyboard(cx: &mut TestAppContext) {
    let (handle, view) = open(cx);
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
fn slider_updates_display_and_enforces_tempo_limits(cx: &mut TestAppContext) {
    let (handle, view) = open(cx);
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
fn toggles_playback_once_per_pointer_or_keyboard_activation(cx: &mut TestAppContext) {
    let (handle, view) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("playback-status").label(), Some("Stopped"));
        window.click("toggle-playback", cx);
        assert_eq!(window.find("toggle-playback").label(), Some("Stop"));
        assert_eq!(window.find("playback-status").label(), Some("Running"));
        window.press("space", cx);
        assert_eq!(window.find("toggle-playback").label(), Some("Start"));
        assert!(view.read(cx).playback.is_none());
        window.press("enter", cx);
        assert_eq!(window.find("playback-status").label(), Some("Running"));
        window.click("toggle-playback", cx);
        assert_eq!(window.find("playback-status").label(), Some("Stopped"));
    })
    .expect("toggle playback through the UI");
}

#[gpui_kit::test]
fn stops_pending_beats_and_restarts_at_the_new_tempo(cx: &mut TestAppContext) {
    let (handle, view) = open(cx);
    let executor = cx.background_executor.clone();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("toggle-playback", cx);
        assert!(view.read(cx).pulse_on);
    })
    .expect("start visual playback");
    cx.run_until_parked();
    executor.advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    cx.update(|cx| assert!(!view.read(cx).pulse_on));
    executor.advance_clock(Duration::from_millis(399));
    cx.run_until_parked();
    cx.update(|cx| assert!(!view.read(cx).pulse_on));
    executor.advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    cx.update(|cx| assert!(view.read(cx).pulse_on));

    cx.update_window(handle, |_, window, cx| {
        window.click("toggle-playback", cx);
    })
    .expect("stop playback");
    cx.run_until_parked();
    executor.advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    cx.update(|cx| assert!(!view.read(cx).pulse_on));

    cx.update_window(handle, |_, window, cx| {
        let slider_id = ("slider", view.read(cx).tempo.entity_id());
        let bounds = window.find(slider_id).bounds();
        window.click_at(
            slider_id,
            point(bounds.size.width - px(0.1), bounds.size.height / 2.),
            cx,
        );
    })
    .expect("change tempo");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.click("toggle-playback", cx);
    })
    .expect("restart playback");
    cx.run_until_parked();
    executor.advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    cx.update(|cx| assert!(!view.read(cx).pulse_on));
    executor.advance_clock(Duration::from_millis(150));
    cx.run_until_parked();
    cx.update(|cx| assert!(view.read(cx).pulse_on));
}

#[gpui_kit::test]
fn changing_tempo_while_running_cancels_the_previous_beat_schedule(cx: &mut TestAppContext) {
    let (handle, view) = open(cx);
    let executor = cx.background_executor.clone();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("toggle-playback", cx);
    })
    .expect("start playback");
    cx.run_until_parked();
    executor.advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        let slider_id = ("slider", view.read(cx).tempo.entity_id());
        let bounds = window.find(slider_id).bounds();
        window.click_at(
            slider_id,
            point(bounds.size.width - px(0.1), bounds.size.height / 2.),
            cx,
        );
    })
    .expect("change tempo while playing");
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(view.read(cx).playback.is_some());
        assert!(view.read(cx).pulse_on);
    });
    executor.advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    executor.advance_clock(Duration::from_millis(149));
    cx.run_until_parked();
    cx.update(|cx| assert!(!view.read(cx).pulse_on));
    executor.advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    cx.update(|cx| assert!(view.read(cx).pulse_on));
    executor.advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    executor.advance_clock(Duration::from_millis(50));
    cx.run_until_parked();
    // This is the old 120 BPM deadline. Its canceled task must not emit a beat.
    cx.update(|cx| assert!(!view.read(cx).pulse_on));
}

#[gpui_kit::test]
fn tab_navigation_keeps_commands_reachable_without_a_pointer(cx: &mut TestAppContext) {
    let (handle, _) = open(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("tab", cx);
        assert_eq!(window.find("decrease-tempo").focused(), Some(true));
        window.press("enter", cx);
        assert_eq!(window.find("tempo-value").label(), Some("119 BPM"));
        assert_eq!(window.find("playback-status").label(), Some("Stopped"));
        window.press("tab", cx);
        assert_eq!(window.find("increase-tempo").focused(), Some(true));
        window.press("tab", cx);
        assert_eq!(window.find("toggle-playback").focused(), Some(true));
        window.press("enter", cx);
        assert_eq!(window.find("playback-status").label(), Some("Running"));
    })
    .expect("navigate and activate controls with the keyboard");
}

#[gpui_kit::test]
fn preserves_layout_at_minimum_size_and_larger_text(cx: &mut TestAppContext) {
    use gpui_kit::component::{Theme, ThemeMode};

    let (handle, _) = open(cx);
    cx.simulate_window_resize(handle, size(px(360.), px(400.)));
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
                let toggle = window.find("toggle-playback").bounds();
                assert!(minus.left() >= px(0.) && plus.right() <= px(360.));
                assert!((f32::from(minus.center().y - value.center().y)).abs() <= 1.);
                assert!((f32::from(plus.center().y - value.center().y)).abs() <= 1.);
                assert!(
                    range.bottom() + px(font_size) <= toggle.top(),
                    "font={font_size}, range={range:?}, toggle={toggle:?}"
                );
                assert!(toggle.bottom() < px(400.));
            })
            .expect("inspect minimum window layout");
        }
    }
}
