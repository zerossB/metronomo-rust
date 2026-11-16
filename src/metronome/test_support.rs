//! Device-free audio shared by feature and window integration tests.
#![expect(clippy::expect_used, reason = "A poisoned fixture must fail the test.")]
use super::{
    audio::{AudioError, AudioPlayback, PlaybackPosition},
    meter::Meter,
};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Default)]
pub(super) struct DeviceState {
    pub(super) configures: usize,
    pub(super) bpm: u16,
    pub(super) volume: u8,
    pub(super) playing: bool,
    pub(super) pulse: bool,
    pub(super) failed: bool,
    pub(super) opens: usize,
    pub(super) meter: Meter,
    pub(super) beat: Option<u8>,
}

pub(super) struct TestPlayback(pub(super) Arc<Mutex<DeviceState>>);
impl AudioPlayback for TestPlayback {
    fn configure(&self, bpm: u16, volume: u8, playing: bool, meter: Meter) {
        let mut state = self.0.lock().expect("test device state");
        if playing != state.playing
            || meter.beats() != state.meter.beats()
            || meter.unit() != state.meter.unit()
        {
            state.beat = None;
            state.pulse = false;
        }
        state.configures += 1;
        state.meter = meter;
        state.bpm = bpm;
        state.volume = volume;
        state.playing = playing;
    }
    fn poll(&mut self, _: Instant) -> Result<PlaybackPosition, AudioError> {
        let state = self.0.lock().expect("test device state");
        if state.failed {
            Err(AudioError::StreamFailed(
                cpal::ErrorKind::DeviceNotAvailable,
            ))
        } else {
            Ok(if state.playing {
                state.beat.map_or_else(PlaybackPosition::idle, |beat| {
                    PlaybackPosition::at_beat(beat, state.pulse)
                })
            } else {
                PlaybackPosition::idle()
            })
        }
    }
}
