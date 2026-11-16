use super::*;

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
        assert_eq!(window.find("increase-tempo").focused(), Some(true));
        assert!(window.find("increase-tempo").visible());
    })
    .expect("reveal the tempo controls when navigating back");
}

#[gpui_kit::test]
fn beat_blocks_preserve_reading_order_and_the_fixed_footer_across_sizes_and_themes(
    cx: &mut TestAppContext,
) {
    use gpui_kit::component::{Theme, ThemeMode};
    let (handle, view, _) = open(cx);
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        for (width, height) in [(600., 780.), (480., 720.)] {
            cx.simulate_window_resize(handle, size(px(width), px(height)));
            for font_size in [16., 20.] {
                cx.update(|cx| {
                    Theme::change(mode, None, cx);
                    Theme::update(cx, |theme| theme.font_size = px(font_size));
                });
                for beats in [1, 4, 5, 8, 12] {
                    view.update(cx, |view, cx| {
                        view.set_meter(Meter::default().with_beats(beats), cx)
                    });
                    cx.update_window(handle, |_, window, cx| {
                        window.render_frame(cx);
                        let columns = beats.min(4);
                        let grid = window.find("beat-grid").bounds();
                        let footer = window.find("practice-footer").bounds();
                        assert!(grid.left() >= px(0.) && grid.right() <= px(width));
                        for beat in 1..=beats {
                            let block = window.find(("beat-sound", u32::from(beat))).bounds();
                            assert!(block.left() >= grid.left() && block.right() <= grid.right());
                            if beats <= 4 {
                                assert!(
                                    window.find(("beat-sound", u32::from(beat))).visible(),
                                    "{beats} beats, {width}×{height}, font {font_size}"
                                );
                                assert!(block.bottom() <= footer.top());
                            }
                            if beat > 1 {
                                let previous =
                                    window.find(("beat-sound", u32::from(beat - 1))).bounds();
                                if (beat - 1) % columns == 0 {
                                    assert!(block.top() >= previous.bottom());
                                } else {
                                    assert_eq!(block.top(), previous.top());
                                    assert!(block.left() >= previous.right());
                                }
                            }
                        }
                        for id in [
                            "toggle-playback",
                            "tap-tempo",
                            "tap-status",
                            "volume-controls",
                        ] {
                            let bounds = window.find(id).bounds();
                            assert!(window.find(id).visible(), "{id}");
                            assert!(bounds.top() >= footer.top() && bounds.bottom() <= px(height));
                        }
                        let play = window.find("toggle-playback").bounds();
                        let tap = window.find("tap-tempo").bounds();
                        assert_eq!(play.top(), tap.top());
                        assert_eq!(play.bottom(), tap.bottom());
                    })
                    .expect("inspect beat and footer geometry");
                }
            }
        }
    }
}

#[gpui_kit::test]
fn bpm_value_and_adjustment_buttons_do_not_shift_between_two_and_three_digits(
    cx: &mut TestAppContext,
) {
    let (handle, view, _) = open(cx);
    let mut reference = None;
    for bpm in [40, 99, 100, 240] {
        view.read_with(cx, |view, _| view.session.clone())
            .update(cx, |session, cx| session.set_bpm(bpm, cx));
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let bounds = ["decrease-tempo", "tempo-value", "increase-tempo"]
                .map(|id| window.find(id).bounds());
            if let Some(reference) = reference {
                assert_eq!(bounds, reference);
            } else {
                reference = Some(bounds);
            }
        })
        .expect("inspect stable BPM alignment");
    }
}
