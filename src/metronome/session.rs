//! Feature commands and authoritative domain state, independent of any window or control.

use super::{
    audio::AudioFactory,
    configuration::Configuration,
    meter::{BeatSound, Meter},
    playback::PlaybackController,
    presets::{Preset, PresetError, PresetLibrary},
    settings::{Preferences, PreferencesWriter, Settings},
    tap_tempo::{RESET_AFTER, TapTempoState},
};
use gpui_kit::{App, AppContext as _, Context, Entity, EventEmitter, Task};

#[derive(Clone, Copy)]
pub(super) enum SessionEvent {
    Configuration,
    Presets,
    Taps,
}

/// Owns persistent musical settings and transient tap history. Views issue commands;
/// playback and the writer own their separate asynchronous lifecycles.
pub(super) struct MetronomeSession {
    configuration: Configuration,
    presets: PresetLibrary,
    taps: TapTempoState,
    tap_reset: Option<Task<()>>,
    playback: Entity<PlaybackController>,
    writer: Entity<PreferencesWriter>,
    closing: bool,
}
impl EventEmitter<SessionEvent> for MetronomeSession {}

impl MetronomeSession {
    pub(super) fn new(
        preferences: Preferences,
        factory: AudioFactory,
        cx: &mut Context<Self>,
    ) -> Self {
        let (settings, store, error) = preferences.into_parts();
        let configuration = Configuration::new(settings.bpm(), settings.volume(), settings.meter());
        let playback = cx.new(|_| PlaybackController::new(configuration, factory));
        let writer = cx.new(|cx| PreferencesWriter::new(store, error, cx));
        Self {
            configuration,
            presets: settings.presets().clone(),
            taps: TapTempoState::default(),
            tap_reset: None,
            playback,
            writer,
            closing: false,
        }
    }
    pub(super) fn configuration(&self) -> Configuration {
        self.configuration
    }
    pub(super) fn presets(&self) -> &PresetLibrary {
        &self.presets
    }
    pub(super) fn taps(&self) -> usize {
        self.taps.taps()
    }
    pub(super) fn playback(&self) -> &Entity<PlaybackController> {
        &self.playback
    }
    pub(super) fn writer(&self) -> &Entity<PreferencesWriter> {
        &self.writer
    }
    pub(super) fn settings(&self) -> Settings {
        Settings::new(
            self.configuration.bpm(),
            self.configuration.volume(),
            self.configuration.meter(),
        )
        .with_presets(self.presets.clone())
    }
    fn save(&self, cx: &mut Context<Self>) {
        let settings = self.settings();
        self.writer
            .update(cx, |writer, cx| writer.enqueue(settings, cx));
    }
    fn change_configuration(&mut self, configuration: Configuration, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.configuration = configuration;
        self.playback
            .update(cx, |playback, cx| playback.configure(configuration, cx));
        self.save(cx);
        cx.emit(SessionEvent::Configuration);
    }
    fn reset_taps(&mut self, cx: &mut Context<Self>) {
        self.tap_reset = None;
        self.taps = TapTempoState::default();
        cx.emit(SessionEvent::Taps);
    }
    pub(super) fn set_bpm(&mut self, bpm: u16, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.reset_taps(cx);
        let next = self.configuration.with_bpm(bpm);
        if next != self.configuration {
            self.change_configuration(next, cx);
        }
    }
    pub(super) fn set_volume(&mut self, volume: u8, cx: &mut Context<Self>) {
        self.change_configuration(self.configuration.with_volume(volume), cx);
    }
    pub(super) fn set_meter(&mut self, meter: Meter, cx: &mut Context<Self>) {
        self.change_configuration(self.configuration.with_meter(meter), cx);
    }
    pub(super) fn set_sound(&mut self, beat: u8, sound: BeatSound, cx: &mut Context<Self>) {
        self.set_meter(self.configuration.meter().with_sound(beat, sound), cx);
    }
    pub(super) fn apply_configuration(
        &mut self,
        configuration: Configuration,
        cx: &mut Context<Self>,
    ) {
        if self.closing {
            return;
        }
        self.reset_taps(cx);
        self.change_configuration(configuration, cx);
    }
    pub(super) fn restore_defaults(&mut self, cx: &mut Context<Self>) {
        self.apply_configuration(Configuration::default(), cx);
    }
    pub(super) fn tap(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        if let Some(bpm) = self.taps.tap(cx.background_executor().now()) {
            let next = self.configuration.with_bpm(bpm);
            if next != self.configuration {
                self.change_configuration(next, cx);
            }
        }
        self.tap_reset = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(RESET_AFTER).await;
            let _ = this.update(cx, |this, cx| this.reset_taps(cx));
        }));
        cx.emit(SessionEvent::Taps);
    }
    pub(super) fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        if !self.closing {
            self.playback
                .update(cx, |playback, cx| playback.toggle_playback(cx));
        }
    }
    fn presets_changed(&self, cx: &mut Context<Self>) {
        self.save(cx);
        cx.emit(SessionEvent::Presets);
    }
    pub(super) fn save_preset(
        &mut self,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Result<u32, PresetError> {
        if self.closing {
            return Err(PresetError::NotFound);
        }
        let id = self.presets.add(name, self.configuration)?;
        self.presets_changed(cx);
        Ok(id)
    }
    pub(super) fn rename_preset(
        &mut self,
        id: u32,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), PresetError> {
        if self.closing {
            return Err(PresetError::NotFound);
        }
        self.presets.rename(id, name)?;
        self.presets_changed(cx);
        Ok(())
    }
    pub(super) fn delete_preset(&mut self, id: u32, cx: &mut Context<Self>) -> Option<Preset> {
        if self.closing {
            return None;
        }
        let preset = self.presets.remove(id)?;
        self.presets_changed(cx);
        Some(preset)
    }
    pub(super) fn restore_preset(
        &mut self,
        preset: Preset,
        cx: &mut Context<Self>,
    ) -> Result<(), PresetError> {
        if self.closing {
            return Err(PresetError::NotFound);
        }
        self.presets.restore(preset)?;
        self.presets_changed(cx);
        Ok(())
    }
    pub(super) fn load_preset(
        &mut self,
        id: u32,
        cx: &mut Context<Self>,
    ) -> Result<(), PresetError> {
        if self.closing {
            return Err(PresetError::NotFound);
        }
        let configuration = self
            .presets
            .get(id)
            .ok_or(PresetError::NotFound)?
            .configuration();
        self.apply_configuration(configuration, cx);
        Ok(())
    }
    pub(super) fn close(&mut self, cx: &mut Context<Self>) -> Task<()> {
        self.closing = true;
        self.tap_reset = None;
        self.playback.update(cx, |playback, cx| playback.stop(cx));
        self.writer.update(cx, |writer, cx| writer.finish(cx))
    }
    pub(super) fn settings_error(&self, cx: &App) -> Option<&'static str> {
        self.writer.read(cx).error()
    }
}

#[cfg(test)]
mod tests;
