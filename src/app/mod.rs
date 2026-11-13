//! Application bootstrap and window lifecycle.

use gpui_kit::component::TitleBar;
use gpui_kit::{
    AppContext as _, KeyBinding, WindowBounds, WindowDecorations, WindowOptions, px, size,
};

use crate::metronome::{Metronome, Preferences};

gpui_kit::actions!(metronomo, [Quit]);

pub(crate) fn run() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            crate::metronome::init(cx);

            cx.bind_keys([
                KeyBinding::new("ctrl-q", Quit, None),
                KeyBinding::new("cmd-q", Quit, None),
            ]);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let load = cx.background_spawn(async { Preferences::load() });
            cx.spawn(async move |cx| {
                let preferences = load.await;
                cx.update(move |cx| {
                    // Native window bounds are a platform boundary, expressed in logical pixels.
                    let options =
                        WindowOptions {
                            window_bounds: Some(WindowBounds::Windowed(
                                gpui_kit::Bounds::centered(None, size(px(600.), px(780.)), cx),
                            )),
                            window_min_size: Some(size(px(480.), px(720.))),
                            app_id: Some("com.haynestec.metronomo".into()),
                            window_decorations: Some(WindowDecorations::Client),
                            ..TitleBar::window_options()
                        };

                    match gpui_kit::open_window(options, cx, |window, cx| {
                        cx.new(|cx| Metronome::new(preferences, window, cx))
                    }) {
                        Ok((handle, view)) => {
                            let view = view.downgrade();
                            cx.on_action(move |_: &Quit, cx| {
                                let _ = cx.update_window(handle, |_, window, cx| {
                                    let _ =
                                        view.update(cx, |this, cx| this.request_close(window, cx));
                                });
                            });
                            cx.activate(true);
                        }
                        Err(error) => {
                            eprintln!("Could not open Metronomo: {error:#}");
                            std::process::exit(1);
                        }
                    }
                });
            })
            .detach();
        });
}
