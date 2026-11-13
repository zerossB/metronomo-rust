#![expect(
    clippy::expect_used,
    reason = "Failures in audio test setup must fail the test."
)]

use super::*;

fn playing(bpm: u16, volume: u8) -> Settings {
    Settings {
        bpm,
        volume,
        playing: true,
        generation: 1,
    }
}

fn beats_for(bpm: u16, block: usize, channels: usize, total_frames: usize) -> Vec<usize> {
    let mut renderer = ClickRenderer::new(48_000);
    let mut samples = vec![0_f32; block * channels];
    let mut beats = Vec::new();
    let mut position = 0;
    while position < total_frames {
        let frames = block.min(total_frames - position);
        renderer.render(
            &mut samples[..frames * channels],
            channels,
            playing(bpm, 60),
            |offset| beats.push(position + offset),
        );
        position += frames;
    }
    beats
}

#[test]
fn schedules_beats_by_frames_independently_of_buffer_size_and_channels() {
    let mono = beats_for(120, 127, 1, 96_000);
    let stereo = beats_for(120, 1024, 2, 96_000);
    assert_eq!(mono, stereo);
    assert_eq!(mono.len(), 4);
    for (ix, beat) in mono.into_iter().enumerate() {
        assert!(beat.abs_diff(ix * 24_000) <= 1);
    }
}

#[test]
fn preserves_fractional_beat_positions_over_a_minute() {
    let beats = beats_for(137, 511, 2, 48_000 * 60);
    assert_eq!(beats.len(), 137);
    for (ix, beat) in beats.into_iter().enumerate() {
        let expected = ix as f64 * 48_000. * 60. / 137.;
        assert!((beat as f64 - expected).abs() <= 1.01, "beat {ix} drifted");
    }
}

#[test]
fn synthesizes_a_bounded_click_on_all_output_channels() {
    let mut renderer = ClickRenderer::new(48_000);
    let mut samples = vec![0_f32; 2400 * 2];
    renderer.render(&mut samples, 2, playing(120, 100), |_| {});
    let peak = samples.iter().copied().map(f32::abs).fold(0., f32::max);
    assert!(peak > 0.1 && peak < 0.7);
    assert!(samples.iter().all(|sample| sample.is_finite()));
    assert!(
        samples
            .as_chunks::<2>()
            .0
            .iter()
            .all(|frame| frame[0] == frame[1])
    );
    assert_eq!(&samples[4000..], &[0.; 800]);
    assert_eq!(samples[0], 0.);
}

#[test]
fn volume_scales_audio_and_mute_preserves_beat_events() {
    let mut full = ClickRenderer::new(48_000);
    let mut half = ClickRenderer::new(48_000);
    let mut loud = vec![0_f32; 24_100];
    let mut quiet = loud.clone();
    full.render(&mut loud, 1, playing(120, 100), |_| {});
    half.render(&mut quiet, 1, playing(120, 50), |_| {});
    assert!(
        loud.iter()
            .zip(&quiet)
            .all(|(loud, quiet)| (loud * 0.5 - quiet).abs() < 0.00001)
    );

    let mut muted = ClickRenderer::new(48_000);
    let mut beats = Vec::new();
    muted.render(&mut quiet, 1, playing(120, 0), |offset| beats.push(offset));
    assert!(quiet.iter().all(|sample| *sample == 0.));
    assert_eq!(beats.len(), 2);
}

#[test]
fn live_tempo_changes_preserve_progress_toward_the_next_beat() {
    let mut renderer = ClickRenderer::new(48_000);
    let mut samples = vec![0_f32; 10_000];
    renderer.render(&mut samples, 1, playing(120, 60), |_| {});
    let mut beats = Vec::new();
    renderer.render(&mut samples, 1, playing(240, 60), |offset| {
        beats.push(offset)
    });
    assert_eq!(beats.len(), 1);
    assert!(beats[0].abs_diff(7000) <= 1);
}

#[test]
fn stop_fades_the_click_and_restart_begins_on_a_fresh_beat() {
    let mut renderer = ClickRenderer::new(48_000);
    let mut samples = vec![0_f32; 256];
    renderer.render(&mut samples, 1, playing(120, 60), |_| {});
    let mut stopped = playing(120, 60);
    stopped.playing = false;
    stopped.generation = 2;
    renderer.render(&mut samples, 1, stopped, |_| {
        panic!("stopped output emitted a beat")
    });
    assert!(samples[128..].iter().all(|sample| sample.abs() < 0.000001));
    let mut restarted = playing(120, 60);
    restarted.generation = 3;
    let mut beats = Vec::new();
    renderer.render(&mut samples, 1, restarted, |offset| beats.push(offset));
    assert_eq!(beats, vec![0]);
}

#[test]
fn unsigned_output_uses_equilibrium_when_stopped() {
    let mut renderer = ClickRenderer::new(44_100);
    let mut samples = [0_u16; 64];
    renderer.render(
        &mut samples,
        2,
        Settings {
            playing: false,
            ..playing(120, 60)
        },
        |_| {},
    );
    assert_eq!(samples, [32768; 64]);
}

#[test]
fn visual_pulse_waits_for_playback_latency_and_expires_at_the_audio_deadline() {
    let now = Instant::now();
    let (mut producer, consumer) = RingBuffer::new(4);
    producer
        .push(Beat {
            at: now + Duration::from_millis(250),
            generation: 1,
        })
        .expect("queue beat");
    let mut timeline = Timeline {
        beats: consumer,
        pulse_until: None,
        generation: 0,
    };
    assert!(!timeline.poll(now, playing(120, 60)));
    assert!(!timeline.poll(now + Duration::from_millis(249), playing(120, 60)));
    assert!(timeline.poll(now + Duration::from_millis(250), playing(120, 60)));
    assert!(!timeline.poll(now + Duration::from_millis(350), playing(120, 60)));
}

#[test]
fn stale_visual_events_are_discarded_after_restart_or_a_ui_stall() {
    let now = Instant::now();
    let (mut producer, consumer) = RingBuffer::new(4);
    producer
        .push(Beat {
            at: now + Duration::from_secs(1),
            generation: 1,
        })
        .expect("queue old beat");
    producer
        .push(Beat {
            at: now + Duration::from_millis(50),
            generation: 3,
        })
        .expect("queue new beat");
    let mut timeline = Timeline {
        beats: consumer,
        pulse_until: Some(now + Duration::from_secs(1)),
        generation: 1,
    };
    let settings = Settings {
        generation: 3,
        ..playing(120, 60)
    };
    assert!(!timeline.poll(now, settings));
    assert!(timeline.poll(now + Duration::from_millis(50), settings));
    assert!(!timeline.poll(now + Duration::from_secs(5), settings));
}

#[test]
fn control_snapshots_clamp_values_and_only_restart_on_transport_changes() {
    let controls = Controls::new();
    controls.configure(500, 200, true);
    let first = controls.load();
    assert_eq!((first.bpm, first.volume, first.playing), (240, 100, true));
    controls.configure(137, 0, true);
    assert_eq!(controls.load().generation, first.generation);
    controls.configure(137, 0, false);
    assert_ne!(controls.load().generation, first.generation);
}

#[test]
#[ignore = "Plays a short click through the real default output; requires an audio session."]
fn default_output_audio_smoke() {
    let mut output = open_default().expect("open real default output");
    output.configure(120, 20, true);
    let until = Instant::now() + Duration::from_millis(1600);
    let mut pulses = 0;
    let mut previous = false;
    while Instant::now() < until {
        let pulse = output
            .poll(Instant::now())
            .expect("audio output remains available");
        if pulse && !previous {
            pulses += 1;
        }
        previous = pulse;
        std::thread::sleep(Duration::from_millis(4));
    }
    output.configure(120, 20, false);
    assert!(
        pulses >= 2,
        "No playback-timestamp events arrived from the device"
    );
}
