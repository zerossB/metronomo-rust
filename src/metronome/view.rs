use std::time::Duration;

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    slider::{Slider, SliderEvent, SliderState},
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyBinding, ParentElement as _, Render, Role, StatefulInteractiveElement as _, Styled as _,
    Subscription, Task, TestSupportExt as _, Window, div,
};

const MIN_BPM: f32 = 40.;
const MAX_BPM: f32 = 240.;
const DEFAULT_BPM: f32 = 120.;
const KEY_CONTEXT: &str = "Metronome";
const PULSE_DURATION: Duration = Duration::from_millis(100);

gpui_kit::actions!(metronome, [TogglePlayback, IncreaseTempo, DecreaseTempo]);

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("space", TogglePlayback, Some("Metronome && !TempoControls")),
        KeyBinding::new("enter", TogglePlayback, Some("Metronome && !TempoControls")),
        KeyBinding::new("up", IncreaseTempo, Some(KEY_CONTEXT)),
        KeyBinding::new("down", DecreaseTempo, Some(KEY_CONTEXT)),
    ]);
}

pub(crate) struct Metronome {
    tempo: Entity<SliderState>,
    focus: FocusHandle,
    pulse_on: bool,
    // The task owns the running state. Dropping it cancels pending beats.
    playback: Option<Task<()>>,
    _tempo_subscription: Subscription,
}

impl Metronome {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let tempo = cx.new(|_| {
            SliderState::new()
                .min(MIN_BPM)
                .max(MAX_BPM)
                .step(1.)
                .default_value(DEFAULT_BPM)
        });
        let subscription = cx.subscribe(&tempo, |this, _, event, cx| {
            if matches!(event, SliderEvent::Change(_)) {
                this.tempo_changed(cx);
            }
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);

        Self {
            tempo,
            focus,
            pulse_on: false,
            playback: None,
            _tempo_subscription: subscription,
        }
    }

    fn bpm(&self, cx: &App) -> f32 {
        self.tempo.read(cx).value().end()
    }

    fn adjust_tempo(&mut self, delta: f32, window: &mut Window, cx: &mut Context<Self>) {
        let previous = self.bpm(cx);
        let next = (previous + delta).clamp(MIN_BPM, MAX_BPM);
        if previous != next {
            self.tempo.update(cx, |tempo, cx| {
                tempo.set_value(next, window, cx);
            });
            self.tempo_changed(cx);
        }
    }

    fn tempo_changed(&mut self, cx: &mut Context<Self>) {
        if self.playback.is_some() {
            self.start_playback(cx);
        }
        cx.notify();
    }

    fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        if self.playback.take().is_some() {
            self.pulse_on = false;
        } else {
            self.start_playback(cx);
        }
        cx.notify();
    }

    fn start_playback(&mut self, cx: &mut Context<Self>) {
        self.playback = None;
        self.pulse_on = true;
        let interval = Duration::from_secs_f64(60. / f64::from(self.bpm(cx)));
        let executor = cx.background_executor().clone();
        let mut next_beat = executor.now() + interval;

        self.playback = Some(cx.spawn(async move |this, cx| {
            loop {
                executor.timer(PULSE_DURATION).await;
                if this
                    .update(cx, |this, cx| {
                        this.pulse_on = false;
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }

                executor
                    .timer(next_beat.saturating_duration_since(executor.now()))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        this.pulse_on = true;
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }

                // Stay on the beat grid, skipping missed beats after a UI stall.
                next_beat += interval;
                if next_beat <= executor.now() {
                    next_beat = executor.now() + interval;
                }
            }
        }));
    }

    fn render_header(&self, cx: &Context<Self>) -> impl IntoElement {
        let status = if self.playback.is_some() {
            "Running"
        } else {
            "Stopped"
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
                            .aria_label(if self.playback.is_none() {
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

    fn render_transport(&self, cx: &Context<Self>) -> impl IntoElement {
        let running = self.playback.is_some();
        let toggle_label = if running { "Stop" } else { "Start" };

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
                    .text_color(cx.theme().muted_foreground)
                    .child("Visual pulse · no sound"),
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
            .child(self.render_transport(cx))
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
