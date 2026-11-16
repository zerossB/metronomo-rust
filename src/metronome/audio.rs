//! Default-device output and a UI timeline derived from audio playback timestamps.

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use cpal::{
    FromSample, SampleFormat, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _},
};
use rtrb::{Consumer, Producer, RingBuffer};

use super::{
    configuration::{DEFAULT_BPM, DEFAULT_VOLUME, MAX_BPM, MIN_BPM},
    meter::Meter,
};

mod renderer;
use renderer::ClickRenderer;

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
    StreamFailed(cpal::ErrorKind),
}

/// The view controls playback through this seam; tests can supply a device-free session.
pub(super) trait AudioPlayback: Send {
    fn configure(&self, bpm: u16, volume: u8, playing: bool, meter: Meter);
    fn poll(&mut self, now: Instant) -> Result<PlaybackPosition, AudioError>;
}

pub(super) type AudioFactory =
    Arc<dyn Fn() -> Result<Box<dyn AudioPlayback>, AudioError> + Send + Sync>;

#[derive(Clone, Copy)]
struct Settings {
    bpm: u16,
    volume: u8,
    playing: bool,
    generation: u32,
    meter: Meter,
}

impl Settings {
    fn packed(self) -> u64 {
        u64::from(self.bpm.clamp(MIN_BPM, MAX_BPM))
            | (u64::from(self.volume.min(100)) << 8)
            | (u64::from(self.playing) << 15)
            | (u64::from(self.meter.packed()) << 16)
            | (u64::from(self.generation & GENERATION_MASK) << 46)
    }

    fn unpack(value: u64) -> Self {
        Self {
            bpm: (value & 255) as u16,
            volume: ((value >> 8) & 127) as u8,
            playing: (value >> 15) & 1 != 0,
            meter: Meter::from_packed((value >> 16) as u32 & 0x3fff_ffff),
            generation: (value >> 46) as u32,
        }
    }
}

const GENERATION_MASK: u32 = (1 << 18) - 1;

/// Current audible beat, retained between clicks; a pulse marks its short attack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PlaybackPosition {
    beat: Option<u8>,
    pulse: bool,
}

impl PlaybackPosition {
    pub(super) fn idle() -> Self {
        Self {
            beat: None,
            pulse: false,
        }
    }
    pub(super) fn at_beat(beat: u8, pulse: bool) -> Self {
        Self {
            beat: Some(beat),
            pulse,
        }
    }
    pub(super) fn beat(self) -> Option<u8> {
        self.beat
    }
    pub(super) fn is_pulse(self) -> bool {
        self.pulse
    }
}

struct Controls {
    settings: AtomicU64,
}

impl Controls {
    fn new() -> Self {
        Self {
            settings: AtomicU64::new(
                Settings {
                    bpm: DEFAULT_BPM,
                    volume: DEFAULT_VOLUME,
                    playing: false,
                    generation: 0,
                    meter: Meter::default(),
                }
                .packed(),
            ),
        }
    }

    fn load(&self) -> Settings {
        Settings::unpack(self.settings.load(Ordering::Acquire))
    }

    // Only the UI writes settings. One atomic snapshot keeps start/stop and BPM coherent.
    fn configure(&self, bpm: u16, volume: u8, playing: bool, meter: Meter) {
        let previous = self.load();
        let generation = if previous.playing != playing
            || previous.meter.beats() != meter.beats()
            || previous.meter.unit() != meter.unit()
        {
            previous.generation.wrapping_add(1) & GENERATION_MASK
        } else {
            previous.generation
        };
        self.settings.store(
            Settings {
                bpm,
                volume,
                playing,
                generation,
                meter,
            }
            .packed(),
            Ordering::Release,
        );
    }
}

#[derive(Clone, Copy, Debug)]
struct Beat {
    at: Instant,
    interval: Duration,
    beat: u8,
    generation: u32,
}

struct Timeline {
    beats: Consumer<Beat>,
    pulse_until: Option<Instant>,
    active_beat: Option<u8>,
    generation: u32,
}

impl Timeline {
    fn poll(&mut self, now: Instant, settings: Settings) -> PlaybackPosition {
        if !settings.playing || self.generation != settings.generation {
            self.pulse_until = None;
            self.active_beat = None;
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
            self.active_beat = Some(beat.beat);
            self.pulse_until = Some(beat.at + PULSE_DURATION.min(beat.interval / 2));
            let _ = self.beats.pop();
        }
        if let Some(beat) = self.active_beat {
            PlaybackPosition::at_beat(
                beat,
                settings.playing && self.pulse_until.is_some_and(|until| now < until),
            )
        } else {
            PlaybackPosition::idle()
        }
    }
}

struct AudioOutput {
    _stream: Stream,
    controls: Arc<Controls>,
    timeline: Timeline,
    errors: Consumer<cpal::ErrorKind>,
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
    let (error_producer, errors) = RingBuffer::new(8);
    let renderer = ClickRenderer::new(config.sample_rate);
    let stream = match format {
        SampleFormat::F32 => build::<f32>(
            &device,
            config,
            renderer,
            controls.clone(),
            producer,
            error_producer,
        ),
        SampleFormat::F64 => build::<f64>(
            &device,
            config,
            renderer,
            controls.clone(),
            producer,
            error_producer,
        ),
        SampleFormat::I8 => build::<i8>(
            &device,
            config,
            renderer,
            controls.clone(),
            producer,
            error_producer,
        ),
        SampleFormat::I16 => build::<i16>(
            &device,
            config,
            renderer,
            controls.clone(),
            producer,
            error_producer,
        ),
        SampleFormat::I24 => build::<cpal::I24>(
            &device,
            config,
            renderer,
            controls.clone(),
            producer,
            error_producer,
        ),
        SampleFormat::I32 => build::<i32>(
            &device,
            config,
            renderer,
            controls.clone(),
            producer,
            error_producer,
        ),
        SampleFormat::I64 => build::<i64>(
            &device,
            config,
            renderer,
            controls.clone(),
            producer,
            error_producer,
        ),
        SampleFormat::U8 => build::<u8>(
            &device,
            config,
            renderer,
            controls.clone(),
            producer,
            error_producer,
        ),
        SampleFormat::U16 => build::<u16>(
            &device,
            config,
            renderer,
            controls.clone(),
            producer,
            error_producer,
        ),
        SampleFormat::U32 => build::<u32>(
            &device,
            config,
            renderer,
            controls.clone(),
            producer,
            error_producer,
        ),
        SampleFormat::U64 => build::<u64>(
            &device,
            config,
            renderer,
            controls.clone(),
            producer,
            error_producer,
        ),
        _ => return Err(AudioError::UnsupportedFormat(format)),
    }
    .map_err(AudioError::Device)?;
    stream.play().map_err(AudioError::Device)?;
    Ok(Box::new(AudioOutput {
        _stream: stream,
        controls,
        errors,
        timeline: Timeline {
            beats: consumer,
            pulse_until: None,
            active_beat: None,
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
    mut errors: Producer<cpal::ErrorKind>,
) -> Result<Stream, cpal::Error> {
    let channels = usize::from(config.channels);
    let rate = f64::from(config.sample_rate);
    device.build_output_stream(
        config,
        move |output: &mut [T], info| {
            let now = Instant::now();
            let timestamp = info.timestamp();
            let playback = now + timestamp.playback.duration_since(timestamp.callback);
            let settings = controls.load();
            let interval = Duration::from_secs_f64(
                60. * settings.meter.unit().quarter_notes() / f64::from(settings.bpm),
            );
            renderer.render(output, channels, settings, |offset, beat| {
                let _ = beats.push(Beat {
                    at: playback + Duration::from_secs_f64(offset as f64 / rate),
                    interval,
                    generation: settings.generation,
                    beat,
                });
            });
        },
        move |error| match error.kind() {
            cpal::ErrorKind::DeviceChanged
            | cpal::ErrorKind::Xrun
            | cpal::ErrorKind::RealtimeDenied => {}
            _ => {
                let _ = errors.push(error.kind());
            }
        },
        Some(Duration::from_secs(3)),
    )
}

impl AudioPlayback for AudioOutput {
    fn configure(&self, bpm: u16, volume: u8, playing: bool, meter: Meter) {
        self.controls.configure(bpm, volume, playing, meter);
    }

    fn poll(&mut self, now: Instant) -> Result<PlaybackPosition, AudioError> {
        if let Ok(kind) = self.errors.pop() {
            return Err(AudioError::StreamFailed(kind));
        }
        Ok(self.timeline.poll(now, self.controls.load()))
    }
}

#[cfg(test)]
mod tests;
