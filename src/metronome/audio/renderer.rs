//! Sample-clock scheduling and precomputed percussion, independent of the device and GPUI.

use super::{super::meter::BeatSound, Settings};
use cpal::{FromSample, Sample};

const GAIN_RAMP_FRAMES: u32 = 128;

pub(super) struct ClickRenderer {
    sample_rate: f64,
    clicks: [Box<[f32]>; 3],
    voice: BeatSound,
    accent: f32,
    next_beat: u8,
    click_position: usize,
    phase: f64,
    generation: u32,
    gain: f32,
    target_gain: f32,
    gain_ramp: u32,
}

impl ClickRenderer {
    pub(super) fn new(sample_rate: u32) -> Self {
        let rate = f64::from(sample_rate);
        let length = (rate * 0.025) as usize;
        let clicks = [2200., 1200., 650.].map(|frequency| {
            (0..length)
                .map(|ix| {
                    let time = ix as f64 / rate;
                    let attack = (time / 0.0005).min(1.);
                    let release = 1. - ix as f64 / length as f64;
                    let envelope = attack * (-time * 220.).exp() * release * release;
                    (0.55 * (std::f64::consts::TAU * frequency * time).sin() * envelope) as f32
                })
                .collect::<Vec<_>>()
                .into_boxed_slice()
        });
        Self {
            sample_rate: rate,
            click_position: length,
            clicks,
            voice: BeatSound::Muted,
            accent: 1.,
            next_beat: 0,
            phase: 1.,
            generation: 0,
            gain: 0.,
            target_gain: 0.,
            gain_ramp: 0,
        }
    }

    pub(super) fn render<T: Sample + FromSample<f32>>(
        &mut self,
        output: &mut [T],
        channels: usize,
        settings: Settings,
        mut on_beat: impl FnMut(usize, u8),
    ) {
        let restarting = settings.generation != self.generation;
        if restarting {
            self.generation = settings.generation;
            self.phase = 1.;
            self.next_beat = 0;
        }
        let gain = if settings.playing {
            f32::from(settings.volume) / 100.
        } else {
            0.
        };
        if gain != self.target_gain {
            self.target_gain = gain;
            if restarting && settings.playing && self.gain == 0. {
                // The synthesized attack already fades in; keep the first beat's accent intact.
                self.gain = gain;
                self.gain_ramp = 0;
            } else {
                self.gain_ramp = GAIN_RAMP_FRAMES;
            }
        }
        let increment = f64::from(settings.bpm)
            / (60. * self.sample_rate * settings.meter.unit().quarter_notes());
        for (offset, frame) in output.chunks_mut(channels).enumerate() {
            if settings.playing {
                if self.phase >= 1. {
                    self.phase -= 1.;
                    let beat = self.next_beat;
                    self.voice = settings.meter.sound(beat);
                    self.accent = if beat == 0 { 1.25 } else { 1. };
                    self.click_position = 0;
                    self.next_beat = (beat + 1) % settings.meter.beats();
                    on_beat(offset, beat);
                }
                // Preserve fractional frames, including across callback boundaries and BPM edits.
                self.phase += increment;
            }
            if self.gain_ramp > 0 {
                self.gain += (self.target_gain - self.gain) / self.gain_ramp as f32;
                self.gain_ramp -= 1;
            }
            let voice = match self.voice {
                BeatSound::High => Some(&self.clicks[0]),
                BeatSound::Medium => Some(&self.clicks[1]),
                BeatSound::Low => Some(&self.clicks[2]),
                BeatSound::Muted => None,
            };
            let sample = voice
                .and_then(|click| click.get(self.click_position))
                .copied()
                .unwrap_or(0.)
                * self.gain
                * self.accent;
            if self.click_position < self.clicks[0].len() {
                self.click_position += 1;
            }
            frame.fill(T::from_sample(sample));
        }
    }
}
