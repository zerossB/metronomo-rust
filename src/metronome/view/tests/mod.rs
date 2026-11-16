#![expect(
    clippy::expect_used,
    reason = "UI test setup and dispatch must succeed."
)]

use super::super::audio::{AudioError, AudioFactory};
use super::{Meter, Metronome, PlaybackState};
use crate::metronome::meter::{BeatSound, BeatUnit};
use gpui_kit::test::{TestEventExt as _, TestQueryExt as _, TestWindowExt as _};
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

mod layout;
mod meter;
mod playback;
mod preferences;
mod presets;
mod previews;
mod tempo;

use crate::metronome::test_support::{DeviceState, TestPlayback};

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
