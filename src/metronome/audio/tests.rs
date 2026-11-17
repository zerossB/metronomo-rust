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
        meter: Meter::default(),
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
            |offset, _| beats.push(position + offset),
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
    renderer.render(&mut samples, 2, playing(120, 100), |_, _| {});
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
    full.render(&mut loud, 1, playing(120, 100), |_, _| {});
    half.render(&mut quiet, 1, playing(120, 50), |_, _| {});
    assert!(
        loud.iter()
            .zip(&quiet)
            .all(|(loud, quiet)| (loud * 0.5 - quiet).abs() < 0.00001)
    );

    let mut muted = ClickRenderer::new(48_000);
    let mut beats = Vec::new();
    muted.render(&mut quiet, 1, playing(120, 0), |offset, _| {
        beats.push(offset)
    });
    assert!(quiet.iter().all(|sample| *sample == 0.));
    assert_eq!(beats.len(), 2);
}

#[test]
fn live_tempo_changes_preserve_progress_toward_the_next_beat() {
    let mut renderer = ClickRenderer::new(48_000);
    let mut samples = vec![0_f32; 10_000];
    renderer.render(&mut samples, 1, playing(120, 60), |_, _| {});
    let mut beats = Vec::new();
    renderer.render(&mut samples, 1, playing(240, 60), |offset, _| {
        beats.push(offset)
    });
    assert_eq!(beats.len(), 1);
    assert!(beats[0].abs_diff(7000) <= 1);
}

#[test]
fn stop_fades_the_click_and_restart_begins_on_a_fresh_beat() {
    let mut renderer = ClickRenderer::new(48_000);
    let mut samples = vec![0_f32; 256];
    renderer.render(&mut samples, 1, playing(120, 60), |_, _| {});
    let mut stopped = playing(120, 60);
    stopped.playing = false;
    stopped.generation = 2;
    renderer.render(&mut samples, 1, stopped, |_, _| {
        panic!("stopped output emitted a beat")
    });
    assert!(samples[128..].iter().all(|sample| sample.abs() < 0.000001));
    let mut restarted = playing(120, 60);
    restarted.generation = 3;
    let mut beats = Vec::new();
    renderer.render(&mut samples, 1, restarted, |offset, _| beats.push(offset));
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
        |_, _| {},
    );
    assert_eq!(samples, [32768; 64]);
}

#[test]
fn visual_pulse_waits_for_playback_latency_and_expires_at_the_audio_deadline() {
    let now = Instant::now();
    let (mut producer, consumer) = RingBuffer::new(4);
    producer
        .push(Beat {
            interval: Duration::from_millis(500),
            at: now + Duration::from_millis(250),
            generation: 1,
            beat: 0,
        })
        .expect("queue beat");
    let mut timeline = Timeline {
        beats: consumer,
        pulse_until: None,
        active_beat: None,
        generation: 0,
    };
    assert!(!timeline.poll(now, playing(120, 60)).is_pulse());
    assert!(
        !timeline
            .poll(now + Duration::from_millis(249), playing(120, 60))
            .is_pulse()
    );
    assert!(
        timeline
            .poll(now + Duration::from_millis(250), playing(120, 60))
            .is_pulse()
    );
    assert!(
        !timeline
            .poll(now + Duration::from_millis(350), playing(120, 60))
            .is_pulse()
    );
}

#[test]
fn stale_visual_events_are_discarded_after_restart_or_a_ui_stall() {
    let now = Instant::now();
    let (mut producer, consumer) = RingBuffer::new(4);
    producer
        .push(Beat {
            interval: Duration::from_millis(500),
            at: now + Duration::from_secs(1),
            generation: 1,
            beat: 0,
        })
        .expect("queue old beat");
    producer
        .push(Beat {
            interval: Duration::from_millis(500),
            at: now + Duration::from_millis(50),
            generation: 3,
            beat: 0,
        })
        .expect("queue new beat");
    let mut timeline = Timeline {
        beats: consumer,
        pulse_until: Some(now + Duration::from_secs(1)),
        active_beat: Some(0),
        generation: 1,
    };
    let settings = Settings {
        generation: 3,
        ..playing(120, 60)
    };
    assert!(!timeline.poll(now, settings).is_pulse());
    assert!(
        timeline
            .poll(now + Duration::from_millis(50), settings)
            .is_pulse()
    );
    assert!(
        !timeline
            .poll(now + Duration::from_secs(5), settings)
            .is_pulse()
    );
}

#[test]
fn control_snapshots_clamp_values_and_only_restart_on_transport_changes() {
    let controls = Controls::new();
    controls.configure(500, 200, true, Meter::default());
    let first = controls.load();
    assert_eq!((first.bpm, first.volume, first.playing), (240, 100, true));
    controls.configure(137, 0, true, Meter::default());
    assert_eq!(controls.load().generation, first.generation);
    controls.configure(137, 0, false, Meter::default());
    assert_ne!(controls.load().generation, first.generation);
}

#[test]
fn advisory_notifications_are_counted_without_failing_playback() {
    let diagnostics = Diagnostics::default();
    let (mut producer, mut consumer) = RingBuffer::new(1);
    for kind in [
        cpal::ErrorKind::Xrun,
        cpal::ErrorKind::DeviceChanged,
        cpal::ErrorKind::RealtimeDenied,
    ] {
        report_stream_error(kind, &diagnostics, &mut producer);
    }
    assert!(consumer.pop().is_err());
    let snapshot = diagnostics.snapshot();
    assert_eq!(
        (
            snapshot.xruns,
            snapshot.device_changes,
            snapshot.realtime_denied
        ),
        (1, 1, 1)
    );
    assert_eq!(snapshot.stream_errors, 0);
    assert!(snapshot.notices_changed(DiagnosticSnapshot::default()));
}

#[test]
fn fatal_errors_are_counted_even_when_the_error_queue_is_full() {
    let diagnostics = Diagnostics::default();
    let (mut producer, mut consumer) = RingBuffer::new(1);
    for _ in 0..3 {
        report_stream_error(
            cpal::ErrorKind::DeviceNotAvailable,
            &diagnostics,
            &mut producer,
        );
    }
    let snapshot = diagnostics.snapshot();
    assert_eq!(snapshot.stream_errors, 3);
    assert_eq!(snapshot.dropped_error_events, 2);
    assert_eq!(consumer.pop(), Ok(cpal::ErrorKind::DeviceNotAvailable));
}

#[test]
fn full_visual_queue_never_interrupts_sample_rendering() {
    let controls = Controls::new();
    let (mut producer, _consumer) = RingBuffer::new(1);
    let mut renderer = ClickRenderer::new(48_000);
    let mut samples = vec![0_f32; 48_000];
    renderer.render(&mut samples, 1, playing(240, 60), |offset, beat| {
        controls.diagnostics.beat();
        if producer
            .push(Beat {
                at: Instant::now() + Duration::from_secs_f64(offset as f64 / 48_000.),
                interval: Duration::from_millis(250),
                beat,
                generation: 1,
            })
            .is_err()
        {
            controls.diagnostics.dropped_visual();
        }
    });
    let snapshot = controls.diagnostics.snapshot();
    assert_eq!((snapshot.beats, snapshot.dropped_visual_events), (4, 3));
    for start in [0, 12_001, 24_001, 36_001] {
        assert!(
            samples[start..start + 1200]
                .iter()
                .any(|sample| sample.abs() > 0.01)
        );
    }
}

#[test]
#[ignore = "Plays a short click through the real default output; requires an audio session."]
fn default_output_audio_smoke() {
    let mut output = open_default().expect("open real default output");
    let meter = Meter::default()
        .with_sound(2, super::super::meter::BeatSound::Low)
        .with_sound(3, super::super::meter::BeatSound::Muted);
    output.configure(240, 20, true, meter);
    let until = Instant::now() + Duration::from_millis(1600);
    let mut pulses = 0;
    let mut previous = false;
    let mut seen = [false; 4];
    while Instant::now() < until {
        let position = output
            .poll(Instant::now())
            .expect("audio output remains available");
        if let Some(beat) = position.beat() {
            seen[usize::from(beat)] = true;
        }
        let pulse = position.is_pulse();
        if pulse && !previous {
            pulses += 1;
        }
        previous = pulse;
        std::thread::sleep(Duration::from_millis(4));
    }
    output.configure(240, 20, false, meter);
    assert!(
        seen.into_iter().all(|seen| seen),
        "The device did not advance through all four beats"
    );
    assert!(
        pulses >= 2,
        "No playback-timestamp events arrived from the device"
    );
}

#[test]
fn each_beat_uses_its_assigned_pitch_and_muted_beats_still_advance() {
    use super::super::meter::BeatSound;
    let meter = Meter::default()
        .with_sound(0, BeatSound::High)
        .with_sound(1, BeatSound::Medium)
        .with_sound(2, BeatSound::Low)
        .with_sound(3, BeatSound::Muted);
    let mut renderer = ClickRenderer::new(48_000);
    let mut samples = vec![0_f32; 48_000];
    let mut beats = Vec::new();
    renderer.render(
        &mut samples,
        1,
        Settings {
            meter,
            ..playing(240, 100)
        },
        |offset, beat| beats.push((offset, beat)),
    );
    assert_eq!(
        beats.iter().map(|(_, beat)| *beat).collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    let crossings = |start: usize| {
        samples[start..start + 1000]
            .windows(2)
            .filter(|pair| pair[0] * pair[1] < 0.)
            .count()
    };
    assert!(crossings(beats[0].0) > crossings(beats[1].0) * 3 / 2);
    assert!(crossings(beats[1].0) > crossings(beats[2].0) * 3 / 2);
    assert!(samples[beats[3].0..].iter().all(|sample| *sample == 0.));
}

#[test]
fn first_beat_is_louder_even_when_every_beat_uses_the_same_timbre() {
    use super::super::meter::BeatSound;
    let meter = Meter::default().with_sound(0, BeatSound::Medium);
    let mut renderer = ClickRenderer::new(48_000);
    let mut samples = vec![0_f32; 24_000];
    renderer.render(
        &mut samples,
        1,
        Settings {
            meter,
            ..playing(240, 100)
        },
        |_, _| {},
    );
    let peak = |slice: &[f32]| slice.iter().copied().map(f32::abs).fold(0., f32::max);
    let first = peak(&samples[..1200]);
    let second = peak(&samples[12_002..13_200]);
    assert!((first / second - 1.25).abs() < 0.001);
    assert!(first < 1.);
}

#[test]
fn three_beat_meter_wraps_and_resizing_starts_from_the_first_beat() {
    let mut renderer = ClickRenderer::new(48_000);
    let mut samples = vec![0_f32; 60_002];
    let mut beats = Vec::new();
    renderer.render(
        &mut samples,
        1,
        Settings {
            meter: Meter::default().with_beats(3),
            ..playing(240, 60)
        },
        |_, beat| beats.push(beat),
    );
    assert_eq!(beats, vec![0, 1, 2, 0, 1, 2]);
    beats.clear();
    renderer.render(
        &mut samples[..128],
        1,
        Settings {
            generation: 2,
            meter: Meter::default().with_beats(6),
            ..playing(240, 60)
        },
        |_, beat| beats.push(beat),
    );
    assert_eq!(beats, vec![0]);
}

#[test]
fn playhead_retains_the_current_beat_between_pulses_and_includes_silent_beats() {
    let now = Instant::now();
    let (mut producer, consumer) = RingBuffer::new(4);
    producer
        .push(Beat {
            interval: Duration::from_millis(500),
            at: now,
            beat: 2,
            generation: 1,
        })
        .expect("queue silent beat");
    let mut timeline = Timeline {
        beats: consumer,
        pulse_until: None,
        active_beat: None,
        generation: 0,
    };
    let settings = Settings {
        meter: Meter::default().with_sound(2, super::super::meter::BeatSound::Muted),
        ..playing(120, 60)
    };
    assert_eq!(
        timeline.poll(now, settings),
        PlaybackPosition::at_beat(2, true)
    );
    assert_eq!(
        timeline.poll(now + Duration::from_millis(200), settings),
        PlaybackPosition::at_beat(2, false)
    );
    assert_eq!(
        timeline.poll(
            now,
            Settings {
                playing: false,
                generation: 2,
                ..settings
            }
        ),
        PlaybackPosition::idle()
    );
}

#[test]
fn note_units_scale_click_intervals_relative_to_quarter_note_bpm() {
    use super::super::meter::BeatUnit;
    for (unit, interval) in [
        (BeatUnit::Half, 48_000),
        (BeatUnit::Quarter, 24_000),
        (BeatUnit::Eighth, 12_000),
        (BeatUnit::Sixteenth, 6_000),
    ] {
        let mut renderer = ClickRenderer::new(48_000);
        let settings = Settings {
            meter: Meter::default().with_unit(unit),
            ..playing(120, 60)
        };
        let total_frames = interval * 4 + 2;
        let mut samples = [0_f32; 254];
        let mut beats = Vec::new();
        let mut position = 0;
        while position < total_frames {
            let frames = 127.min(total_frames - position);
            renderer.render(&mut samples[..frames * 2], 2, settings, |offset, beat| {
                beats.push((position + offset, beat))
            });
            position += frames;
        }
        assert_eq!(beats.len(), 5, "wrong click rate for {unit:?}");
        for (ix, (frame, beat)) in beats.into_iter().enumerate() {
            assert!(
                frame.abs_diff(ix * interval) <= 1,
                "wrong interval for {unit:?}: frame {frame}, expected {}",
                ix * interval
            );
            assert_eq!(usize::from(beat), ix % 4);
        }
    }
}

#[test]
fn eighth_and_sixteenth_notes_preserve_fractional_timing_over_a_minute() {
    use super::super::meter::BeatUnit;
    for (unit, clicks_per_minute) in [(BeatUnit::Eighth, 274), (BeatUnit::Sixteenth, 548)] {
        let mut renderer = ClickRenderer::new(44_100);
        let settings = Settings {
            meter: Meter::default().with_unit(unit),
            ..playing(137, 0)
        };
        let mut samples = [0_f32; 1024];
        let mut beats = Vec::new();
        let mut position = 0;
        let total_frames = 44_100 * 60;
        while position < total_frames {
            let frames = 1024.min(total_frames - position);
            renderer.render(&mut samples[..frames], 1, settings, |offset, _| {
                beats.push(position + offset)
            });
            position += frames;
        }
        assert_eq!(beats.len(), clicks_per_minute);
        for (ix, frame) in beats.into_iter().enumerate() {
            let expected = ix as f64 * total_frames as f64 / clicks_per_minute as f64;
            assert!(
                (frame as f64 - expected).abs() <= 1.01,
                "{unit:?} drifted at click {ix}"
            );
        }
        assert!(samples.iter().all(|sample| *sample == 0.));
    }
}

#[test]
fn fast_note_units_keep_visual_pulses_separate_without_losing_the_playhead() {
    let now = Instant::now();
    let interval = Duration::from_micros(62_500); // 1/16 notes at 240 quarter-note BPM.
    let (mut producer, consumer) = RingBuffer::new(4);
    producer
        .push(Beat {
            at: now,
            interval,
            beat: 0,
            generation: 1,
        })
        .expect("queue first beat");
    producer
        .push(Beat {
            at: now + interval,
            interval,
            beat: 1,
            generation: 1,
        })
        .expect("queue next beat");
    let mut timeline = Timeline {
        beats: consumer,
        pulse_until: None,
        active_beat: None,
        generation: 0,
    };
    let settings = Settings {
        meter: Meter::default().with_unit(super::super::meter::BeatUnit::Sixteenth),
        ..playing(240, 60)
    };
    assert_eq!(
        timeline.poll(now, settings),
        PlaybackPosition::at_beat(0, true)
    );
    assert_eq!(
        timeline.poll(now + Duration::from_micros(31_250), settings),
        PlaybackPosition::at_beat(0, false)
    );
    assert_eq!(
        timeline.poll(now + interval, settings),
        PlaybackPosition::at_beat(1, true)
    );
}
