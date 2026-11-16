//! Owns audio startup, cancellation, output failures, and the audible playhead.

use super::{
    audio::{AudioFactory, AudioPlayback, PlaybackPosition},
    configuration::Configuration,
};
use gpui_kit::{AppContext as _, Context, Task};
use std::time::Duration;

const VISUAL_REFRESH: Duration = Duration::from_millis(16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PlaybackState {
    Stopped,
    Starting,
    Running,
}

pub(super) struct PlaybackController {
    configuration: Configuration,
    playback: PlaybackState,
    position: PlaybackPosition,
    audio: Option<Box<dyn AudioPlayback>>,
    audio_factory: AudioFactory,
    audio_error: Option<String>,
    request: u64,
    _startup: Option<Task<()>>,
    _monitor: Option<Task<()>>,
}

impl PlaybackController {
    pub(super) fn new(configuration: Configuration, audio_factory: AudioFactory) -> Self {
        Self {
            configuration,
            playback: PlaybackState::Stopped,
            position: PlaybackPosition::idle(),
            audio: None,
            audio_factory,
            audio_error: None,
            request: 0,
            _startup: None,
            _monitor: None,
        }
    }
    pub(super) fn state(&self) -> PlaybackState {
        self.playback
    }
    pub(super) fn position(&self) -> PlaybackPosition {
        self.position
    }
    pub(super) fn error(&self) -> Option<&str> {
        self.audio_error.as_deref()
    }
    pub(super) fn configure(&mut self, configuration: Configuration, cx: &mut Context<Self>) {
        let previous = self.configuration.meter();
        let next = configuration.meter();
        if previous.beats() != next.beats() || previous.unit() != next.unit() {
            self.position = PlaybackPosition::idle();
        }
        self.configuration = configuration;
        self.configure_audio();
        cx.notify();
    }
    pub(super) fn stop(&mut self, cx: &mut Context<Self>) {
        self.request = self.request.wrapping_add(1);
        self._startup = None;
        self._monitor = None;
        self.playback = PlaybackState::Stopped;
        self.position = PlaybackPosition::idle();
        self.configure_audio();
        cx.notify();
    }
    fn configure_audio(&self) {
        if let Some(audio) = &self.audio {
            audio.configure(
                self.configuration.bpm(),
                self.configuration.volume(),
                self.playback == PlaybackState::Running,
                self.configuration.meter(),
            );
        }
    }

    pub(super) fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        if self.playback != PlaybackState::Stopped {
            self.stop(cx);
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
        self.configure_audio();
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
}
