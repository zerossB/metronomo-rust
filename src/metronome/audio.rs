//! Default-device output and a UI timeline derived from audio playback timestamps.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use cpal::{
    FromSample, SampleFormat, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _},
};
use rtrb::{Consumer, Producer, RingBuffer};

mod renderer;
use renderer::ClickRenderer;

pub(super) const MIN_BPM: u16 = 40;
pub(super) const MAX_BPM: u16 = 240;
pub(super) const DEFAULT_VOLUME: u8 = 60;
const PULSE_DURATION: Duration = Duration::from_millis(100);

#[derive(Debug, thiserror::Error)]
pub(super) enum AudioError {
    #[error("No audio output is available. Connect a speaker or headphones and try again.")]
    NoDevice,
    #[error("Could not open the audio output. Check your output device and try again.")]
    Device(#[source] cpal::Error),
    #[error("The default output uses an unsupported audio format ({0}).")]
    UnsupportedFormat(SampleFormat),
    #[error("Audio output stopped. Check your output device and press Start to reconnect.")]
    StreamFailed,
}

/// The view controls playback through this seam; tests can supply a device-free session.
pub(super) trait AudioPlayback: Send {
    fn configure(&self, bpm: u16, volume: u8, playing: bool);
    fn poll(&mut self, now: Instant) -> Result<bool, AudioError>;
}

pub(super) type AudioFactory =
    Arc<dyn Fn() -> Result<Box<dyn AudioPlayback>, AudioError> + Send + Sync>;

#[derive(Clone, Copy)]
struct Settings {
    bpm: u16,
    volume: u8,
    playing: bool,
    generation: u32,
}

impl Settings {
    fn packed(self) -> u64 {
        u64::from(self.bpm.clamp(MIN_BPM, MAX_BPM))
            | (u64::from(self.volume.min(100)) << 16)
            | (u64::from(self.playing) << 24)
            | (u64::from(self.generation) << 32)
    }

    fn unpack(value: u64) -> Self {
        Self {
            bpm: value as u16,
            volume: (value >> 16) as u8,
            playing: (value >> 24) & 1 != 0,
            generation: (value >> 32) as u32,
        }
    }
}

struct Controls {
    settings: AtomicU64,
    failed: AtomicBool,
}

impl Controls {
    fn new() -> Self {
        Self {
            settings: AtomicU64::new(
                Settings {
                    bpm: 120,
                    volume: DEFAULT_VOLUME,
                    playing: false,
                    generation: 0,
                }
                .packed(),
            ),
            failed: AtomicBool::new(false),
        }
    }

    fn load(&self) -> Settings {
        Settings::unpack(self.settings.load(Ordering::Acquire))
    }

    // Only the UI writes settings. One atomic snapshot keeps start/stop and BPM coherent.
    fn configure(&self, bpm: u16, volume: u8, playing: bool) {
        let previous = self.load();
        let generation = if previous.playing != playing {
            previous.generation.wrapping_add(1)
        } else {
            previous.generation
        };
        self.settings.store(
            Settings {
                bpm,
                volume,
                playing,
                generation,
            }
            .packed(),
            Ordering::Release,
        );
    }
}

#[derive(Clone, Copy, Debug)]
struct Beat {
    at: Instant,
    generation: u32,
}

struct Timeline {
    beats: Consumer<Beat>,
    pulse_until: Option<Instant>,
    generation: u32,
}

impl Timeline {
    fn poll(&mut self, now: Instant, settings: Settings) -> bool {
        if !settings.playing || self.generation != settings.generation {
            self.pulse_until = None;
            self.generation = settings.generation;
        }
        // A full queue drops visual events only. Audio rendering never waits for the UI.
        while let Ok(beat) = self.beats.peek() {
            if beat.generation != settings.generation || !settings.playing {
                let _ = self.beats.pop();
                continue;
            }
            if beat.at > now {
                break;
            }
            self.pulse_until = Some(beat.at + PULSE_DURATION);
            let _ = self.beats.pop();
        }
        settings.playing && self.pulse_until.is_some_and(|until| now < until)
    }
}

struct AudioOutput {
    _stream: Stream,
    controls: Arc<Controls>,
    timeline: Timeline,
}

pub(super) fn open_default() -> Result<Box<dyn AudioPlayback>, AudioError> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or(AudioError::NoDevice)?;
    let supported = device.default_output_config().map_err(AudioError::Device)?;
    let format = supported.sample_format();
    let config = supported.config();
    let controls = Arc::new(Controls::new());
    let (producer, consumer) = RingBuffer::new(128);
    let renderer = ClickRenderer::new(config.sample_rate);
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, config, renderer, controls.clone(), producer),
        SampleFormat::F64 => build::<f64>(&device, config, renderer, controls.clone(), producer),
        SampleFormat::I8 => build::<i8>(&device, config, renderer, controls.clone(), producer),
        SampleFormat::I16 => build::<i16>(&device, config, renderer, controls.clone(), producer),
        SampleFormat::I24 => {
            build::<cpal::I24>(&device, config, renderer, controls.clone(), producer)
        }
        SampleFormat::I32 => build::<i32>(&device, config, renderer, controls.clone(), producer),
        SampleFormat::I64 => build::<i64>(&device, config, renderer, controls.clone(), producer),
        SampleFormat::U8 => build::<u8>(&device, config, renderer, controls.clone(), producer),
        SampleFormat::U16 => build::<u16>(&device, config, renderer, controls.clone(), producer),
        SampleFormat::U32 => build::<u32>(&device, config, renderer, controls.clone(), producer),
        SampleFormat::U64 => build::<u64>(&device, config, renderer, controls.clone(), producer),
        _ => return Err(AudioError::UnsupportedFormat(format)),
    }
    .map_err(AudioError::Device)?;
    stream.play().map_err(AudioError::Device)?;
    Ok(Box::new(AudioOutput {
        _stream: stream,
        controls,
        timeline: Timeline {
            beats: consumer,
            pulse_until: None,
            generation: 0,
        },
    }))
}

fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: StreamConfig,
    mut renderer: ClickRenderer,
    controls: Arc<Controls>,
    mut beats: Producer<Beat>,
) -> Result<Stream, cpal::Error> {
    let channels = usize::from(config.channels);
    let rate = f64::from(config.sample_rate);
    let failures = controls.clone();
    device.build_output_stream(
        config,
        move |output: &mut [T], info| {
            let now = Instant::now();
            let timestamp = info.timestamp();
            let playback = now + timestamp.playback.duration_since(timestamp.callback);
            let settings = controls.load();
            renderer.render(output, channels, settings, |offset| {
                let _ = beats.push(Beat {
                    at: playback + Duration::from_secs_f64(offset as f64 / rate),
                    generation: settings.generation,
                });
            });
        },
        move |error| match error.kind() {
            cpal::ErrorKind::DeviceChanged
            | cpal::ErrorKind::Xrun
            | cpal::ErrorKind::RealtimeDenied => {}
            _ => failures.failed.store(true, Ordering::Release),
        },
        Some(Duration::from_secs(3)),
    )
}

impl AudioPlayback for AudioOutput {
    fn configure(&self, bpm: u16, volume: u8, playing: bool) {
        self.controls.configure(bpm, volume, playing);
    }

    fn poll(&mut self, now: Instant) -> Result<bool, AudioError> {
        if self.controls.failed.load(Ordering::Acquire) {
            return Err(AudioError::StreamFailed);
        }
        Ok(self.timeline.poll(now, self.controls.load()))
    }
}

#[cfg(test)]
mod tests;
