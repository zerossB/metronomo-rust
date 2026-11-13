use std::{sync::Arc, time::Duration};

use super::audio::{self, AudioFactory, AudioPlayback, DEFAULT_VOLUME};

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    slider::{Slider, SliderEvent, SliderState},
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyBinding, MouseButton, ParentElement as _, Render, Role, StatefulInteractiveElement as _,
    Styled as _, Subscription, Task, TestSupportExt as _, Window, div,
};

const MIN_BPM: f32 = audio::MIN_BPM as f32;
const MAX_BPM: f32 = audio::MAX_BPM as f32;
const DEFAULT_BPM: f32 = 120.;
const KEY_CONTEXT: &str = "Metronome";
const VISUAL_REFRESH: Duration = Duration::from_millis(16);

gpui_kit::actions!(
    metronome,
    [
        TogglePlayback,
        IncreaseTempo,
        DecreaseTempo,
        IncreaseVolume,
        DecreaseVolume
    ]
);

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(
            "space",
            TogglePlayback,
            Some("Metronome && !TempoControls && !VolumeControls"),
        ),
        KeyBinding::new(
            "enter",
            TogglePlayback,
            Some("Metronome && !TempoControls && !VolumeControls"),
        ),
        KeyBinding::new("up", IncreaseVolume, Some("Metronome > VolumeControls")),
        KeyBinding::new("down", DecreaseVolume, Some("Metronome > VolumeControls")),
        KeyBinding::new("left", DecreaseVolume, Some("Metronome > VolumeControls")),
        KeyBinding::new("right", IncreaseVolume, Some("Metronome > VolumeControls")),
        KeyBinding::new("up", IncreaseTempo, Some("Metronome && !VolumeControls")),
        KeyBinding::new("down", DecreaseTempo, Some("Metronome && !VolumeControls")),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlaybackState {
    Stopped,
    Starting,
    Running,
}

pub(crate) struct Metronome {
    tempo: Entity<SliderState>,
    volume: Entity<SliderState>,
    focus: FocusHandle,
    volume_focus: FocusHandle,
    pulse_on: bool,
    playback: PlaybackState,
    audio: Option<Box<dyn AudioPlayback>>,
    audio_factory: AudioFactory,
    audio_error: Option<String>,
    request: u64,
    _startup: Option<Task<()>>,
    _monitor: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl Metronome {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_audio_factory(window, cx, Arc::new(audio::open_default))
    }

    fn with_audio_factory(
        window: &mut Window,
        cx: &mut Context<Self>,
        audio_factory: AudioFactory,
    ) -> Self {
        let tempo = cx.new(|_| {
            SliderState::new()
                .min(MIN_BPM)
                .max(MAX_BPM)
                .step(1.)
                .default_value(DEFAULT_BPM)
        });
        let volume = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .step(1.)
                .default_value(f32::from(DEFAULT_VOLUME))
        });
        let tempo_subscription = cx.subscribe(&tempo, |this, _, event, cx| {
            if matches!(event, SliderEvent::Change(_)) {
                this.configure_audio(cx);
                cx.notify();
            }
        });
        let volume_subscription = cx.subscribe(&volume, |this, _, event, cx| {
            if matches!(event, SliderEvent::Change(_)) {
                this.configure_audio(cx);
                cx.notify();
            }
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            tempo,
            volume,
            focus,
            volume_focus: cx.focus_handle().tab_stop(true),
            pulse_on: false,
            playback: PlaybackState::Stopped,
            audio: None,
            audio_factory,
            audio_error: None,
            request: 0,
            _startup: None,
            _monitor: None,
            _subscriptions: vec![tempo_subscription, volume_subscription],
        }
    }

    fn bpm(&self, cx: &App) -> f32 {
        self.tempo.read(cx).value().end()
    }

    fn volume(&self, cx: &App) -> f32 {
        self.volume.read(cx).value().end()
    }

    fn adjust_tempo(&mut self, delta: f32, window: &mut Window, cx: &mut Context<Self>) {
        let previous = self.bpm(cx);
        let next = (previous + delta).clamp(MIN_BPM, MAX_BPM);
        if previous != next {
            self.tempo
                .update(cx, |tempo, cx| tempo.set_value(next, window, cx));
            self.configure_audio(cx);
            cx.notify();
        }
    }

    fn adjust_volume(&mut self, delta: f32, window: &mut Window, cx: &mut Context<Self>) {
        let next = (self.volume(cx) + delta).clamp(0., 100.);
        self.volume
            .update(cx, |volume, cx| volume.set_value(next, window, cx));
        self.configure_audio(cx);
        cx.notify();
    }

    fn configure_audio(&self, cx: &App) {
        if let Some(audio) = &self.audio {
            audio.configure(
                self.bpm(cx) as u16,
                self.volume(cx) as u8,
                self.playback == PlaybackState::Running,
            );
        }
    }

    fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        if self.playback != PlaybackState::Stopped {
            self.request = self.request.wrapping_add(1);
            self._startup = None;
            self._monitor = None;
            self.playback = PlaybackState::Stopped;
            self.pulse_on = false;
            self.configure_audio(cx);
        } else if self.audio.is_some() {
            self.begin_playback(cx);
        } else {
            self.audio_error = None;
            self.playback = PlaybackState::Starting;
            self.request = self.request.wrapping_add(1);
            let request = self.request;
            let factory = self.audio_factory.clone();
            let open = cx.background_spawn(async move { factory() });
            self._startup = Some(cx.spawn(async move |this, cx| {
                let result = open.await;
                let _ = this.update(cx, |this, cx| {
                    if this.request != request || this.playback != PlaybackState::Starting {
                        return;
                    }
                    match result {
                        Ok(audio) => {
                            this.audio = Some(audio);
                            this.begin_playback(cx);
                        }
                        Err(error) => {
                            eprintln!("Could not initialize audio: {error:?}");
                            this.audio_error = Some(error.to_string());
                            this.playback = PlaybackState::Stopped;
                            cx.notify();
                        }
                    }
                });
            }));
        }
        cx.notify();
    }

    fn begin_playback(&mut self, cx: &mut Context<Self>) {
        self.playback = PlaybackState::Running;
        self.pulse_on = false;
        self.configure_audio(cx);
        let executor = cx.background_executor().clone();
        self._monitor = Some(cx.spawn(async move |this, cx| {
            loop {
                let keep_polling = this.update(cx, |this, cx| {
                    let Some(audio) = this.audio.as_mut() else {
                        return false;
                    };
                    match audio.poll(executor.now()) {
                        Ok(pulse) => {
                            if pulse != this.pulse_on {
                                this.pulse_on = pulse;
                                cx.notify();
                            }
                            true
                        }
                        Err(error) => {
                            eprintln!("Could not initialize audio: {error:?}");
                            this.audio_error = Some(error.to_string());
                            this.audio = None;
                            this.pulse_on = false;
                            this.playback = PlaybackState::Stopped;
                            cx.notify();
                            false
                        }
                    }
                });
                if !matches!(keep_polling, Ok(true)) {
                    break;
                }
                executor.timer(VISUAL_REFRESH).await;
            }
        }));
        cx.notify();
    }

    fn render_header(&self, cx: &Context<Self>) -> impl IntoElement {
        let status = if self.audio_error.is_some() {
            "Unavailable"
        } else {
            match self.playback {
                PlaybackState::Stopped => "Stopped",
                PlaybackState::Starting => "Starting",
                PlaybackState::Running => "Running",
            }
        };

        div()
            .flex()
            .items_center()
            .justify_between()
            .child(div().text_lg().child("Metronomo"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("beat-indicator")
                            .test_support()
                            .aria_label(if self.playback == PlaybackState::Stopped {
                                "Stopped"
                            } else if self.pulse_on {
                                "Beat"
                            } else {
                                "Between beats"
                            })
                            .size_3()
                            .rounded(cx.theme().radius_full())
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(if self.pulse_on {
                                cx.theme().primary
                            } else {
                                cx.theme().muted
                            }),
                    )
                    .child(
                        div()
                            .id("playback-status")
                            .role(Role::Status)
                            .test_support()
                            .aria_label(status)
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(status),
                    ),
            )
    }

    fn render_tempo_controls(&self, cx: &Context<Self>) -> impl IntoElement {
        let bpm = self.bpm(cx);
        let decrease = Button::new("decrease-tempo")
            .icon(IconName::Minus)
            .accessibility_label("Decrease tempo")
            .tooltip("Decrease tempo (↓)")
            .outline()
            .large()
            .disabled(bpm <= MIN_BPM)
            .on_click(cx.listener(|this, _, window, cx| {
                this.adjust_tempo(-1., window, cx);
            }));
        let increase = Button::new("increase-tempo")
            .icon(IconName::Plus)
            .accessibility_label("Increase tempo")
            .tooltip("Increase tempo (↑)")
            .outline()
            .large()
            .disabled(bpm >= MAX_BPM)
            .on_click(cx.listener(|this, _, window, cx| {
                this.adjust_tempo(1., window, cx);
            }));

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .justify_center()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .key_context("TempoControls")
                            .gap_4()
                            .child(decrease)
                            .child(
                                div()
                                    .id("tempo-value")
                                    .test_support()
                                    .aria_label(format!("{bpm:.0} BPM"))
                                    .w_32()
                                    .text_center()
                                    .text_size(gpui_kit::rems(3.))
                                    .line_height(gpui_kit::relative(1.))
                                    .child(format!("{bpm:.0}")),
                            )
                            .child(increase),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("BPM"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Slider::new(&self.tempo).accessibility_label("Tempo in BPM"))
                    .child(
                        div()
                            .id("tempo-range")
                            .test_support()
                            .flex()
                            .justify_between()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{MIN_BPM:.0}"))
                            .child(format!("{MAX_BPM:.0}")),
                    ),
            )
    }

    fn render_volume(&self, cx: &Context<Self>) -> impl IntoElement {
        let volume = self.volume(cx);
        div()
            .id("volume-controls")
            .test_support()
            .track_focus(&self.volume_focus)
            .key_context("VolumeControls")
            .role(Role::Group)
            .aria_label("Volume. Use arrow keys to adjust.")
            .capture_any_mouse_down(cx.listener(
                |this, event: &gpui_kit::MouseDownEvent, window, cx| {
                    if event.button == MouseButton::Left {
                        window.focus(&this.volume_focus, cx);
                    }
                },
            ))
            .on_action(cx.listener(|this, _: &IncreaseVolume, window, cx| {
                this.adjust_volume(5., window, cx)
            }))
            .on_action(cx.listener(|this, _: &DecreaseVolume, window, cx| {
                this.adjust_volume(-5., window, cx)
            }))
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .border_1()
            .border_color(cx.theme().transparent)
            .rounded(cx.theme().radius)
            .focus_visible(|style| style.border_color(cx.theme().ring))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_sm()
                    .child("Volume")
                    .child(
                        div()
                            .id("volume-value")
                            .test_support()
                            .aria_label(format!("{volume:.0}%"))
                            .text_color(cx.theme().muted_foreground)
                            .child(if volume == 0. {
                                "Muted".into()
                            } else {
                                format!("{volume:.0}%")
                            }),
                    ),
            )
            .child(Slider::new(&self.volume).accessibility_label("Volume percentage"))
    }

    fn render_transport(&self, cx: &Context<Self>) -> impl IntoElement {
        let running = self.playback != PlaybackState::Stopped;
        let toggle_label = if running { "Stop" } else { "Start" };
        let message = self
            .audio_error
            .as_deref()
            .unwrap_or("Default audio output")
            .to_owned();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                Button::new("toggle-playback")
                    .label(toggle_label)
                    .icon(if running {
                        IconName::Square
                    } else {
                        IconName::Play
                    })
                    .primary()
                    .large()
                    .w_full()
                    .tooltip("Start or stop (Space)")
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_playback(cx))),
            )
            .child(
                div()
                    .text_center()
                    .text_xs()
                    .id("audio-message")
                    .test_support()
                    .aria_label(message.clone())
                    .text_color(if self.audio_error.is_some() {
                        cx.theme().danger
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(message),
            )
    }
}

impl Render for Metronome {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("metronome")
            .test_support()
            .track_focus(&self.focus)
            .key_context(KEY_CONTEXT)
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
            .p_4()
            .gap_4()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_header(cx))
            .child(self.render_tempo_controls(cx))
            .child(self.render_volume(cx))
            .child(self.render_transport(cx))
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
