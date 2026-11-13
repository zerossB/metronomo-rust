//! Sample-clock scheduling and precomputed percussion, independent of the device and GPUI.

use super::Settings;
use cpal::{FromSample, Sample};

const GAIN_RAMP_FRAMES: u32 = 128;

pub(super) struct ClickRenderer {
    sample_rate: f64,
    click: Box<[f32]>,
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
        let click = (0..length)
            .map(|ix| {
                let time = ix as f64 / rate;
                let attack = (time / 0.0005).min(1.);
                let release = 1. - ix as f64 / length as f64;
                let envelope = attack * (-time * 220.).exp() * release * release;
                (0.7 * (std::f64::consts::TAU * 1800. * time).sin() * envelope) as f32
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            sample_rate: rate,
            click_position: length,
            click,
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
        mut on_beat: impl FnMut(usize),
    ) {
        if settings.generation != self.generation {
            self.generation = settings.generation;
            self.phase = 1.;
        }
        let gain = if settings.playing {
            f32::from(settings.volume) / 100.
        } else {
            0.
        };
        if gain != self.target_gain {
            self.target_gain = gain;
            self.gain_ramp = GAIN_RAMP_FRAMES;
        }
        let increment = f64::from(settings.bpm) / (60. * self.sample_rate);
        for (offset, frame) in output.chunks_mut(channels).enumerate() {
            if settings.playing {
                if self.phase >= 1. {
                    self.phase -= 1.;
                    self.click_position = 0;
                    on_beat(offset);
                }
                // Preserve fractional frames, including across callback boundaries and BPM edits.
                self.phase += increment;
            }
            if self.gain_ramp > 0 {
                self.gain += (self.target_gain - self.gain) / self.gain_ramp as f32;
                self.gain_ramp -= 1;
            }
            let sample = self.click.get(self.click_position).copied().unwrap_or(0.) * self.gain;
            if self.click_position < self.click.len() {
                self.click_position += 1;
            }
            frame.fill(T::from_sample(sample));
        }
    }
}
