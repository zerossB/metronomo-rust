//! Application bootstrap and window lifecycle.

use gpui_kit::{
    AppContext as _, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};

use crate::metronome::Metronome;

gpui_kit::actions!(metronomo, [Quit]);

pub(crate) fn run() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);

            cx.bind_keys([
                KeyBinding::new("ctrl-q", Quit, None),
                KeyBinding::new("cmd-q", Quit, None),
            ]);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            // Native window bounds are a platform boundary, expressed in logical pixels.
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(gpui_kit::Bounds::centered(
                    None,
                    size(px(480.), px(360.)),
                    cx,
                ))),
                window_min_size: Some(size(px(360.), px(240.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Metronomo".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };

            match gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| Metronome)) {
                Ok(_) => cx.activate(true),
                Err(error) => {
                    eprintln!("Could not open Metronomo: {error:#}");
                    std::process::exit(1);
                }
            }
        });
}
