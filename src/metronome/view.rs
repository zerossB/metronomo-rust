use std::{sync::Arc, time::Duration};

use super::audio::{self, AudioFactory, AudioPlayback, PlaybackPosition};
use super::meter::{BeatSound, BeatUnit, Meter};
use super::settings::{Preferences, Settings, SettingsStore};
use super::tap_tempo::{RESET_AFTER, TapTempoState};

mod meter_controls;
mod persistence;
use meter_controls::MeterFocus;

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, TitleBar,
    button::{Button, ButtonVariants as _},
    scroll::ScrollableElement as _,
    slider::{Slider, SliderEvent, SliderState},
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyBinding, KeyUpEvent, MouseButton, ParentElement as _, Render, Role,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, TestSupportExt as _, Window,
    div,
};

const MIN_BPM: f32 = audio::MIN_BPM as f32;
const MAX_BPM: f32 = audio::MAX_BPM as f32;
const KEY_CONTEXT: &str = "Metronome";
const VISUAL_REFRESH: Duration = Duration::from_millis(16);

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
    cx.bind_keys([
        KeyBinding::new("t", TapTempo, Some("Metronome && !PopupMenu")),
        KeyBinding::new("ctrl-w", CloseWindow, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-w", CloseWindow, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-m", MinimizeWindow, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-m", MinimizeWindow, Some(KEY_CONTEXT)),
        KeyBinding::new(
            "space",
            TogglePlayback,
            Some("Metronome && !TempoControls && !VolumeControls && !MeterControls"),
        ),
        KeyBinding::new(
            "enter",
            TogglePlayback,
            Some("Metronome && !TempoControls && !VolumeControls && !MeterControls"),
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
    position: PlaybackPosition,
    meter: Meter,
    meter_focus: MeterFocus,
    practice_scroll: gpui_kit::ScrollHandle,
    tempo_focus: [FocusHandle; 3],
    tap_tempo: TapTempoState,
    tap_key_down: bool,
    playback: PlaybackState,
    audio: Option<Box<dyn AudioPlayback>>,
    audio_factory: AudioFactory,
    audio_error: Option<String>,
    settings_store: Option<SettingsStore>,
    settings_error: Option<&'static str>,
    pending_settings: Option<Settings>,
    settings_dirty: bool,
    closing: bool,
    request: u64,
    _startup: Option<Task<()>>,
    _monitor: Option<Task<()>>,
    _tap_reset: Option<Task<()>>,
    save_task: Option<Task<()>>,
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
        let (settings, settings_store, settings_error) = preferences.into_parts();
        let tempo = cx.new(|_| {
            SliderState::new()
                .min(MIN_BPM)
                .max(MAX_BPM)
                .step(1.)
                .default_value(f32::from(settings.bpm()))
        });
        let volume = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .step(1.)
                .default_value(f32::from(settings.volume()))
        });
        let tempo_subscription = cx.subscribe(&tempo, |this, _, event, cx| {
            if matches!(event, SliderEvent::Change(_)) {
                this.reset_taps();
                this.configure_audio(cx);
                this.save_preferences(cx);
                cx.notify();
            }
        });
        let volume_subscription = cx.subscribe(&volume, |this, _, event, cx| {
            if matches!(event, SliderEvent::Change(_)) {
                this.configure_audio(cx);
                this.save_preferences(cx);
                cx.notify();
            }
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let practice_scroll = gpui_kit::ScrollHandle::default();
        let meter_focus = MeterFocus::new(cx, &practice_scroll);
        let tempo_focus = std::array::from_fn(|_| cx.focus_handle().tab_stop(true));
        let mut subscriptions = vec![tempo_subscription, volume_subscription];
        subscriptions.push(cx.on_app_quit(|this, cx| {
            let final_save = this
                .settings_store
                .clone()
                .filter(|_| this.settings_dirty)
                .map(|store| {
                    let settings =
                        Settings::new(this.bpm(cx) as u16, this.volume(cx) as u8, this.meter);
                    store.begin_shutdown();
                    // GPUI destroys windows before polling quit futures. This final
                    // write must not depend on the view or the foreground save loop.
                    this.save_task = None;
                    this.close_task = None;
                    cx.background_spawn(async move {
                        if let Err(error) = store.save_on_shutdown(&settings) {
                            eprintln!("Could not save Metronomo settings during shutdown: {error}");
                        }
                    })
                });
            async move {
                if let Some(save) = final_save {
                    save.await;
                }
            }
        }));
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
        for focus in &tempo_focus {
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
            tempo,
            volume,
            focus,
            volume_focus: cx.focus_handle().tab_stop(true),
            position: PlaybackPosition::idle(),
            meter: settings.meter(),
            meter_focus,
            practice_scroll,
            tempo_focus,
            tap_tempo: TapTempoState::default(),
            tap_key_down: false,
            playback: PlaybackState::Stopped,
            audio: None,
            audio_factory,
            audio_error: None,
            settings_store,
            settings_error,
            pending_settings: None,
            settings_dirty: false,
            closing: false,
            request: 0,
            _startup: None,
            _monitor: None,
            _tap_reset: None,
            save_task: None,
            close_task: None,
            _subscriptions: subscriptions,
        }
    }

    fn bpm(&self, cx: &App) -> f32 {
        self.tempo.read(cx).value().end()
    }

    fn volume(&self, cx: &App) -> f32 {
        self.volume.read(cx).value().end()
    }

    fn adjust_tempo(&mut self, delta: f32, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.reset_taps();
        self.set_tempo(self.bpm(cx) + delta, window, cx);
        cx.notify();
    }

    fn set_tempo(&mut self, bpm: f32, window: &mut Window, cx: &mut Context<Self>) {
        let previous = self.bpm(cx);
        let next = bpm.clamp(MIN_BPM, MAX_BPM);
        if previous != next {
            self.tempo
                .update(cx, |tempo, cx| tempo.set_value(next, window, cx));
            self.configure_audio(cx);
            self.save_preferences(cx);
            cx.notify();
        }
    }

    fn reset_taps(&mut self) {
        self._tap_reset = None;
        self.tap_tempo = TapTempoState::default();
    }

    fn tap_tempo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        if let Some(bpm) = self.tap_tempo.tap(cx.background_executor().now()) {
            self.set_tempo(f32::from(bpm), window, cx);
        }
        self._tap_reset = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(RESET_AFTER).await;
            let _ = this.update(cx, |this, cx| {
                this.tap_tempo = TapTempoState::default();
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn adjust_volume(&mut self, delta: f32, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        let next = (self.volume(cx) + delta).clamp(0., 100.);
        self.volume
            .update(cx, |volume, cx| volume.set_value(next, window, cx));
        self.configure_audio(cx);
        self.save_preferences(cx);
        cx.notify();
    }

    fn configure_audio(&self, cx: &App) {
        if let Some(audio) = &self.audio {
            audio.configure(
                self.bpm(cx) as u16,
                self.volume(cx) as u8,
                self.playback == PlaybackState::Running,
                self.meter,
            );
        }
    }

    fn set_meter(&mut self, meter: Meter, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        if self.meter.beats() != meter.beats() || self.meter.unit() != meter.unit() {
            self.position = PlaybackPosition::idle();
        }
        self.meter = meter;
        self.configure_audio(cx);
        self.save_preferences(cx);
        cx.notify();
    }

    fn set_sound(&mut self, beat: u8, sound: BeatSound, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.meter = self.meter.with_sound(beat, sound);
        self.configure_audio(cx);
        self.save_preferences(cx);
        cx.notify();
    }

    fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        if self.playback != PlaybackState::Stopped {
            self.request = self.request.wrapping_add(1);
            self._startup = None;
            self._monitor = None;
            self.playback = PlaybackState::Stopped;
            self.position = PlaybackPosition::idle();
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
                            eprintln!("Audio output failed: {error:?}");
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
        self.position = PlaybackPosition::idle();
        self.configure_audio(cx);
        let executor = cx.background_executor().clone();
        self._monitor = Some(cx.spawn(async move |this, cx| {
            loop {
                let keep_polling = this.update(cx, |this, cx| {
                    let Some(audio) = this.audio.as_mut() else {
                        return false;
                    };
                    match audio.poll(executor.now()) {
                        Ok(position) => {
                            if position != this.position {
                                this.position = position;
                                cx.notify();
                            }
                            true
                        }
                        Err(error) => {
                            eprintln!("Audio output failed: {error:?}");
                            this.audio_error = Some(error.to_string());
                            this.audio = None;
                            this.position = PlaybackPosition::idle();
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

        TitleBar::new()
            .on_close_window(cx.listener(|this, _, window, cx| this.request_close(window, cx)))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .mr_3()
                    .items_center()
                    .justify_between()
                    .child(div().text_sm().child("Metronomo"))
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
                                    } else if self.position.is_pulse() {
                                        "Beat"
                                    } else {
                                        "Between beats"
                                    })
                                    .size_3()
                                    .rounded(cx.theme().radius_full())
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(if self.position.is_pulse() {
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
                    ),
            )
    }

    fn render_tempo_controls(&self, cx: &Context<Self>) -> impl IntoElement {
        let bpm = self.bpm(cx);
        let tap_hint = match self.tap_tempo.taps() {
            0 => "Tap quarter notes · T",
            1 => "Tap again · T",
            _ => "Keep tapping · T",
        };
        let decrease = Button::new("decrease-tempo")
            .track_focus(&self.tempo_focus[0])
            .icon(IconName::Minus)
            .accessibility_label("Decrease tempo")
            .tooltip("Decrease tempo (↓)")
            .outline()
            .large()
            .disabled(self.closing || bpm <= MIN_BPM)
            .on_click(cx.listener(|this, _, window, cx| {
                this.adjust_tempo(-1., window, cx);
            }));
        let increase = Button::new("increase-tempo")
            .track_focus(&self.tempo_focus[1])
            .icon(IconName::Plus)
            .accessibility_label("Increase tempo")
            .tooltip("Increase tempo (↑)")
            .outline()
            .large()
            .disabled(self.closing || bpm >= MAX_BPM)
            .on_click(cx.listener(|this, _, window, cx| {
                this.adjust_tempo(1., window, cx);
            }));

        div()
            .key_context("TempoControls")
            .flex()
            .flex_col()
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
                            .child("BPM · 1/4 note"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        Slider::new(&self.tempo)
                            .disabled(self.closing)
                            .accessibility_label("Tempo in quarter-note BPM"),
                    )
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
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("tap-tempo")
                            .disabled(self.closing)
                            .track_focus(&self.tempo_focus[2])
                            .label("Tap tempo")
                            .tooltip("Tap quarter notes to set the tempo (T)")
                            .outline()
                            .large()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.tap_tempo(window, cx);
                            })),
                    )
                    .child(
                        div()
                            .id("tap-status")
                            .test_support()
                            .role(Role::Status)
                            .aria_label(tap_hint)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(tap_hint),
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
            .child(
                Slider::new(&self.volume)
                    .disabled(self.closing)
                    .accessibility_label("Volume percentage"),
            )
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
                    .disabled(self.closing)
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
            .children(self.settings_error.map(|message| {
                div()
                    .id("settings-message")
                    .test_support()
                    .role(Role::Status)
                    .aria_label(message)
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(message)
            }))
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
                            .gap_6()
                            .child(self.render_tempo_controls(cx))
                            .child(self.render_meter(cx)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p_4()
                    .gap_4()
                    .child(self.render_volume(cx))
                    .child(self.render_transport(cx)),
            )
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
