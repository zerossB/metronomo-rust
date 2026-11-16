//! The metronome window owns presentation, focus, and native window commands.

use std::sync::Arc;

use super::audio::{self, AudioFactory, PlaybackPosition};
use super::configuration::Configuration;
use super::meter::{BeatSound, Meter};
use super::playback::PlaybackState;
use super::session::{MetronomeSession, SessionEvent};
use super::settings::Preferences;

mod beat_block;
mod header;
mod meter_controls;
mod preset_controls;
mod presets_dialog;
mod tempo_controls;
mod transport_controls;
mod volume_controls;
use meter_controls::MeterFocus;

use gpui_kit::component::{
    ActiveTheme as _,
    scroll::ScrollableElement as _,
    slider::{SliderEvent, SliderState},
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyBinding, KeyUpEvent, ParentElement as _, Render, StatefulInteractiveElement as _,
    Styled as _, Subscription, Task, TestSupportExt as _, Window, div,
};

const MIN_BPM: f32 = super::configuration::MIN_BPM as f32;
const MAX_BPM: f32 = super::configuration::MAX_BPM as f32;
const KEY_CONTEXT: &str = "Metronome";

gpui_kit::actions!(
    metronome,
    [
        TogglePlayback,
        TapTempo,
        CloseWindow,
        MinimizeWindow,
        IncreaseTempo,
        DecreaseTempo,
        IncreaseVolume,
        DecreaseVolume
    ]
);

pub(crate) fn init(cx: &mut App) {
    preset_controls::init(cx);
    cx.bind_keys([
        KeyBinding::new("t", TapTempo, Some("Metronome && !PopupMenu")),
        KeyBinding::new("ctrl-w", CloseWindow, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-w", CloseWindow, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-m", MinimizeWindow, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-m", MinimizeWindow, Some(KEY_CONTEXT)),
        KeyBinding::new(
            "space",
            TogglePlayback,
            Some("Metronome && !TempoControls && !VolumeControls && !MeterControls && !PresetControls"),
        ),
        KeyBinding::new(
            "enter",
            TogglePlayback,
            Some("Metronome && !TempoControls && !VolumeControls && !MeterControls && !PresetControls"),
        ),
        KeyBinding::new("up", IncreaseVolume, Some("Metronome > VolumeControls")),
        KeyBinding::new("down", DecreaseVolume, Some("Metronome > VolumeControls")),
        KeyBinding::new("left", DecreaseVolume, Some("Metronome > VolumeControls")),
        KeyBinding::new("right", IncreaseVolume, Some("Metronome > VolumeControls")),
        KeyBinding::new(
            "up",
            IncreaseTempo,
            Some("Metronome && !VolumeControls && !MeterControls"),
        ),
        KeyBinding::new(
            "down",
            DecreaseTempo,
            Some("Metronome && !VolumeControls && !MeterControls"),
        ),
    ]);
}

/// Window composition and transient control state; musical state lives in the session.
pub(crate) struct Metronome {
    session: Entity<MetronomeSession>,
    tempo: Entity<SliderState>,
    volume: Entity<SliderState>,
    focus: FocusHandle,
    volume_focus: FocusHandle,
    meter_focus: MeterFocus,
    practice_scroll: gpui_kit::ScrollHandle,
    tempo_focus: [FocusHandle; 3],
    preset_focus: FocusHandle,
    tap_key_down: bool,
    closing: bool,
    open_beat_menu: Option<u8>,
    close_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl Metronome {
    pub(crate) fn new(
        preferences: Preferences,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::with_preferences(window, cx, Arc::new(audio::open_default), preferences)
    }

    #[cfg(test)]
    fn with_audio_factory(
        window: &mut Window,
        cx: &mut Context<Self>,
        audio_factory: AudioFactory,
    ) -> Self {
        Self::with_preferences(window, cx, audio_factory, Preferences::in_memory())
    }

    fn with_preferences(
        window: &mut Window,
        cx: &mut Context<Self>,
        audio_factory: AudioFactory,
        preferences: Preferences,
    ) -> Self {
        window.set_window_title("Metronomo");
        let session = cx.new(|cx| MetronomeSession::new(preferences, audio_factory, cx));
        let configuration = session.read(cx).configuration();
        let tempo = cx.new(|_| {
            SliderState::new()
                .min(MIN_BPM)
                .max(MAX_BPM)
                .step(1.)
                .default_value(f32::from(configuration.bpm()))
        });
        let volume = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .step(1.)
                .default_value(f32::from(configuration.volume()))
        });
        let tempo_subscription = cx.subscribe(&tempo, |this, _, event, cx| {
            if !this.closing
                && let SliderEvent::Change(value) = event
            {
                let bpm = value.end() as u16;
                this.session
                    .update(cx, |session, cx| session.set_bpm(bpm, cx));
            }
        });
        let volume_subscription = cx.subscribe(&volume, |this, _, event, cx| {
            if !this.closing
                && let SliderEvent::Change(value) = event
            {
                let volume = value.end() as u8;
                this.session
                    .update(cx, |session, cx| session.set_volume(volume, cx));
            }
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let practice_scroll = gpui_kit::ScrollHandle::default();
        let meter_focus = MeterFocus::new(cx, &practice_scroll);
        let tempo_focus = std::array::from_fn(|_| cx.focus_handle().tab_stop(true));
        let mut subscriptions = vec![tempo_subscription, volume_subscription];
        subscriptions.push(
            cx.subscribe_in(&session, window, |this, _, event, window, cx| {
                if matches!(event, SessionEvent::Configuration) {
                    this.sync_sliders(window, cx);
                }
                cx.notify();
            }),
        );
        let playback = session.read(cx).playback().clone();
        let writer = session.read(cx).writer().clone();
        subscriptions.push(cx.observe(&playback, |_, _, cx| cx.notify()));
        subscriptions.push(cx.observe(&writer, |_, _, cx| cx.notify()));
        let view = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            view.update(cx, |this, cx| this.request_close(window, cx))
                .is_err()
        });
        // A held shortcut counts once, even when the OS sends key-repeat actions.
        // Clear it if focus leaves the view before the key-up can arrive.
        subscriptions.push(cx.on_focus_out(&focus, window, |this, _, _, _| {
            this.tap_key_down = false;
        }));
        subscriptions.extend(meter_focus.observe(window, cx));
        for focus in &tempo_focus[..2] {
            let scroll = practice_scroll.clone();
            let target = focus.clone();
            subscriptions.push(cx.on_focus(focus, window, move |_, window, _| {
                if window.last_input_was_keyboard() {
                    let scroll = scroll.clone();
                    let target = target.clone();
                    window.on_next_frame(move |window, _| {
                        if target.is_focused(window) {
                            scroll.set_offset(Default::default());
                            window.refresh();
                        }
                    });
                }
            }));
        }
        Self {
            session,
            tempo,
            volume,
            focus,
            volume_focus: cx.focus_handle().tab_stop(true),
            meter_focus,
            practice_scroll,
            tempo_focus,
            preset_focus: cx.focus_handle().tab_stop(true),
            tap_key_down: false,
            closing: false,
            open_beat_menu: None,
            close_task: None,
            _subscriptions: subscriptions,
        }
    }

    fn configuration(&self, cx: &App) -> Configuration {
        self.session.read(cx).configuration()
    }
    fn bpm(&self, cx: &App) -> f32 {
        f32::from(self.configuration(cx).bpm())
    }
    fn volume(&self, cx: &App) -> f32 {
        f32::from(self.configuration(cx).volume())
    }
    fn meter(&self, cx: &App) -> Meter {
        self.configuration(cx).meter()
    }
    fn position(&self, cx: &App) -> PlaybackPosition {
        self.session.read(cx).playback().read(cx).position()
    }
    fn playback(&self, cx: &App) -> PlaybackState {
        self.session.read(cx).playback().read(cx).state()
    }
    fn audio_error<'a>(&'a self, cx: &'a App) -> Option<&'a str> {
        self.session.read(cx).playback().read(cx).error()
    }
    fn settings_error(&self, cx: &App) -> Option<&'static str> {
        self.session.read(cx).settings_error(cx)
    }

    fn sync_sliders(&self, window: &mut Window, cx: &mut Context<Self>) {
        let configuration = self.configuration(cx);
        for (slider, value) in [
            (&self.tempo, configuration.bpm() as f32),
            (&self.volume, configuration.volume() as f32),
        ] {
            if slider.read(cx).value().end() != value {
                // Owner synchronization is silent: set_value does not emit SliderEvent.
                slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
            }
        }
    }
    fn adjust_tempo(&mut self, delta: f32, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        let bpm = (self.bpm(cx) + delta).clamp(MIN_BPM, MAX_BPM) as u16;
        self.session
            .update(cx, |session, cx| session.set_bpm(bpm, cx));
        self.sync_sliders(window, cx);
        cx.notify();
    }
    fn tap_tempo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.closing {
            self.session.update(cx, |session, cx| session.tap(cx));
            self.sync_sliders(window, cx);
            cx.notify();
        }
    }
    fn adjust_volume(&mut self, delta: f32, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        let volume = (self.volume(cx) + delta).clamp(0., 100.) as u8;
        self.session
            .update(cx, |session, cx| session.set_volume(volume, cx));
        self.sync_sliders(window, cx);
        cx.notify();
    }
    fn set_meter(&mut self, meter: Meter, cx: &mut Context<Self>) {
        if !self.closing {
            self.session
                .update(cx, |session, cx| session.set_meter(meter, cx));
        }
    }
    fn set_sound(&mut self, beat: u8, sound: BeatSound, cx: &mut Context<Self>) {
        if !self.closing {
            self.session
                .update(cx, |session, cx| session.set_sound(beat, sound, cx));
        }
    }
    fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        if !self.closing {
            self.session
                .update(cx, |session, cx| session.toggle_playback(cx));
        }
    }
    pub(crate) fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.closing = true;
        let save = self.session.update(cx, |session, cx| session.close(cx));
        self.close_task = Some(cx.spawn_in(window, async move |this, cx| {
            save.await;
            let _ = this.update_in(cx, |_, window, _| window.remove_window());
        }));
        cx.notify();
    }
}

impl Render for Metronome {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("metronome")
            .test_support()
            .track_focus(&self.focus)
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &CloseWindow, window, cx| {
                this.request_close(window, cx);
            }))
            .on_action(cx.listener(|_, _: &MinimizeWindow, window, _| window.minimize_window()))
            .capture_key_up(cx.listener(|this, event: &KeyUpEvent, _, _| {
                if event.keystroke.key == "t" {
                    this.tap_key_down = false;
                }
            }))
            .on_action(cx.listener(|this, _: &TapTempo, window, cx| {
                if !this.tap_key_down {
                    this.tap_key_down = true;
                    this.tap_tempo(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &TogglePlayback, _, cx| this.toggle_playback(cx)))
            .on_action(cx.listener(|this, _: &IncreaseTempo, window, cx| {
                this.adjust_tempo(1., window, cx);
            }))
            .on_action(cx.listener(|this, _: &DecreaseTempo, window, cx| {
                this.adjust_tempo(-1., window, cx);
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_header(cx))
            .child(self.render_preset_controls(cx))
            .child(
                div()
                    .id("practice-scroll")
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.practice_scroll)
                    .vertical_scrollbar(&self.practice_scroll)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .p_4()
                            .gap_4()
                            .child(self.render_tempo_controls(cx))
                            .child(self.render_meter(cx)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .id("practice-footer")
                    .test_support()
                    .flex_shrink_0()
                    .px_4()
                    .py_3()
                    .gap_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(self.render_volume(cx))
                    .child(self.render_transport(cx)),
            )
    }
}

#[cfg(test)]
mod tests;
