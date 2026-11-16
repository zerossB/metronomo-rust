use serde::{Deserialize, Serialize};

use super::meter::{BeatSound, BeatUnit, MAX_BEATS, Meter};

pub(super) const MIN_BPM: u16 = 40;
pub(super) const MAX_BPM: u16 = 240;
pub(super) const DEFAULT_BPM: u16 = 120;
pub(super) const DEFAULT_VOLUME: u8 = 60;

/// A metronome snapshot, independent of the user's preset library and playback state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Configuration {
    bpm: u16,
    volume: u8,
    beats_per_bar: u8,
    beat_unit: u8,
    sounds: [BeatSound; MAX_BEATS as usize],
}

impl Default for Configuration {
    fn default() -> Self {
        Self::new(DEFAULT_BPM, DEFAULT_VOLUME, Meter::default())
    }
}

impl Configuration {
    pub(super) fn new(bpm: u16, volume: u8, meter: Meter) -> Self {
        Self {
            bpm,
            volume,
            beats_per_bar: meter.beats(),
            beat_unit: meter.unit().denominator(),
            sounds: std::array::from_fn(|ix| meter.sound(ix as u8)),
        }
    }

    pub(super) fn bpm(self) -> u16 {
        self.bpm
    }
    pub(super) fn volume(self) -> u8 {
        self.volume
    }

    pub(super) fn meter(self) -> Meter {
        let unit = BeatUnit::ALL
            .into_iter()
            .find(|unit| unit.denominator() == self.beat_unit)
            .unwrap_or(BeatUnit::Quarter);
        self.sounds.iter().enumerate().fold(
            Meter::default()
                .with_beats(self.beats_per_bar)
                .with_unit(unit),
            |meter, (ix, sound)| meter.with_sound(ix as u8, *sound),
        )
    }

    pub(super) fn with_bpm(mut self, bpm: u16) -> Self {
        self.bpm = bpm.clamp(MIN_BPM, MAX_BPM);
        self
    }

    pub(super) fn with_volume(mut self, volume: u8) -> Self {
        self.volume = volume.min(100);
        self
    }

    pub(super) fn with_meter(self, meter: Meter) -> Self {
        Self::new(self.bpm, self.volume, meter)
    }

    pub(super) fn is_valid(self) -> bool {
        (MIN_BPM..=MAX_BPM).contains(&self.bpm)
            && self.volume <= 100
            && (1..=MAX_BEATS).contains(&self.beats_per_bar)
            && BeatUnit::ALL
                .iter()
                .any(|unit| unit.denominator() == self.beat_unit)
    }
}
