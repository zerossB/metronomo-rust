//! Meter and per-beat sound choices shared by the interface and audio renderer.

pub(super) const MAX_BEATS: u8 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub(super) enum BeatSound {
    High,
    Medium,
    Low,
    Muted,
}

impl BeatSound {
    pub(super) const ALL: [Self; 4] = [Self::High, Self::Medium, Self::Low, Self::Muted];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
            Self::Muted => "Muted",
        }
    }

    pub(super) fn short_label(self) -> &'static str {
        match self {
            Self::Medium => "Mid",
            Self::Muted => "Mute",
            _ => self.label(),
        }
    }

    fn from_bits(bits: u32) -> Self {
        Self::ALL[(bits & 3) as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum BeatUnit {
    Half,
    Quarter,
    Eighth,
    Sixteenth,
}

impl BeatUnit {
    pub(super) const ALL: [Self; 4] = [Self::Half, Self::Quarter, Self::Eighth, Self::Sixteenth];
    pub(super) fn denominator(self) -> u8 {
        2 << self as u8
    }

    /// Duration relative to the quarter note used as the fixed BPM reference.
    pub(super) fn quarter_notes(self) -> f64 {
        4. / f64::from(self.denominator())
    }
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Half => "Half note",
            Self::Quarter => "Quarter note",
            Self::Eighth => "Eighth note",
            Self::Sixteenth => "Sixteenth note",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Meter {
    beats: u8,
    unit: BeatUnit,
    sounds: [BeatSound; MAX_BEATS as usize],
}

impl Default for Meter {
    fn default() -> Self {
        let mut sounds = [BeatSound::Medium; MAX_BEATS as usize];
        sounds[0] = BeatSound::High;
        Self {
            beats: 4,
            unit: BeatUnit::Quarter,
            sounds,
        }
    }
}

impl Meter {
    pub(super) fn beats(self) -> u8 {
        self.beats
    }
    pub(super) fn unit(self) -> BeatUnit {
        self.unit
    }
    pub(super) fn with_beats(mut self, beats: u8) -> Self {
        self.beats = beats.clamp(1, MAX_BEATS);
        self
    }
    pub(super) fn with_unit(mut self, unit: BeatUnit) -> Self {
        self.unit = unit;
        self
    }
    pub(super) fn sound(self, beat: u8) -> BeatSound {
        self.sounds
            .get(usize::from(beat))
            .copied()
            .unwrap_or(BeatSound::Muted)
    }
    pub(super) fn with_sound(mut self, beat: u8, sound: BeatSound) -> Self {
        if let Some(slot) = self.sounds.get_mut(usize::from(beat)) {
            *slot = sound;
        }
        self
    }

    // Fits all twelve sound slots and the time signature into one atomic control snapshot.
    pub(super) fn packed(self) -> u32 {
        self.sounds.iter().enumerate().fold(
            u32::from(self.beats) | ((self.unit as u32) << 4),
            |bits, (ix, sound)| bits | ((*sound as u32) << (6 + ix * 2)),
        )
    }

    pub(super) fn from_packed(bits: u32) -> Self {
        Self {
            beats: ((bits & 15) as u8).clamp(1, MAX_BEATS),
            unit: BeatUnit::ALL[((bits >> 4) & 3) as usize],
            sounds: std::array::from_fn(|ix| BeatSound::from_bits(bits >> (6 + ix * 2))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_encoding_preserves_every_sound_and_the_time_signature() {
        let mut meter = Meter::default()
            .with_beats(12)
            .with_unit(BeatUnit::Sixteenth);
        for beat in 0..MAX_BEATS {
            meter = meter.with_sound(beat, BeatSound::ALL[usize::from(beat % 4)]);
        }
        assert_eq!(Meter::from_packed(meter.packed()), meter);
    }
    #[test]
    fn resizing_preserves_hidden_beat_choices() {
        let meter = Meter::default().with_beats(8).with_sound(7, BeatSound::Low);
        assert_eq!(meter.with_beats(2).with_beats(8).sound(7), BeatSound::Low);
    }
}
