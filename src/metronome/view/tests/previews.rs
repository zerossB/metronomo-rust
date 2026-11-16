use super::*;

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
#[ignore = "Requires a real GPU renderer and exports interface previews into the temporary directory."]
fn render_meter_previews() {
    use crate::metronome::meter::BeatSound;
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
        crate::theme::init(cx);
        crate::metronome::init(cx);
    });
    for (name, mode, width, height, font_size, beats) in [
        ("light", ThemeMode::Light, 600., 780., 16., 12),
        ("dark", ThemeMode::Dark, 600., 780., 16., 12),
        ("zoom", ThemeMode::Dark, 480., 720., 20., 12),
        ("light-zoom", ThemeMode::Light, 480., 720., 20., 12),
        ("light-small", ThemeMode::Light, 480., 720., 16., 4),
        ("dark-small", ThemeMode::Dark, 480., 720., 16., 4),
        ("light-large-text", ThemeMode::Light, 600., 780., 20., 4),
        ("dark-large-text", ThemeMode::Dark, 600., 780., 20., 4),
        ("light-default", ThemeMode::Light, 600., 780., 16., 4),
        ("dark-default", ThemeMode::Dark, 600., 780., 16., 4),
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
                        let mut meter = Meter::default().with_beats(beats);
                        for beat in 0..12 {
                            meter = meter.with_sound(beat, BeatSound::ALL[usize::from(beat % 4)]);
                        }
                        view.set_meter(meter, cx);
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
            assert_eq!(
                window.find("current-beat").label(),
                Some(format!("Beat 3 of {beats}").as_str())
            );
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
            crate::theme::init(cx);
            crate::metronome::init(cx);
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
