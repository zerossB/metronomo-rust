//! Opt-in measurements. Backend timestamps are predictions, never acoustic evidence.
#![expect(
    clippy::expect_used,
    reason = "A missing device or report must fail validation."
)]

use super::*;
use crate::metronome::meter::BeatUnit;
use serde_json::{Value, json};
use std::{fs, path::PathBuf, process::Command};

fn environment() -> Value {
    json!({
        "os": std::env::consts::OS,
        "architecture": std::env::consts::ARCH,
        "source_sha256": Command::new("python3").arg("scripts/release/source_fingerprint.py")
            .output().ok().filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned()),
        "os_release": fs::read_to_string("/etc/os-release").ok(),
        "session": std::env::var("XDG_SESSION_TYPE").ok(),
        "wayland_display": std::env::var("WAYLAND_DISPLAY").ok(),
        "x11_display": std::env::var("DISPLAY").ok(),
        "commit": Command::new("git").args(["rev-parse", "HEAD"]).output().ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned()),
        "unix_time_seconds": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).expect("system clock").as_secs(),
    })
}

fn write_report(name: &str, report: Value) {
    let directory = std::env::var_os("METRONOMO_VALIDATION_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("metronomo-validation"));
    fs::create_dir_all(&directory).expect("create report directory");
    let path = directory.join(format!("{name}.json"));
    fs::write(
        &path,
        serde_json::to_vec_pretty(&report).expect("serialize report"),
    )
    .expect("write validation report");
    eprintln!("Validation report: {}", path.display());
}

fn measure_renderer(
    rate: u32,
    block: usize,
    bpm: u16,
    unit: BeatUnit,
    seconds: usize,
    channels: usize,
) -> Value {
    let mut renderer = ClickRenderer::new(rate);
    let settings = Settings {
        bpm,
        volume: 20,
        playing: true,
        generation: 1,
        meter: Meter::default().with_beats(12).with_unit(unit),
    };
    let total_frames = rate as usize * seconds;
    let interval = f64::from(rate) * 60. * unit.quarter_notes() / f64::from(bpm);
    let expected_count = (((total_frames - 1) as f64 / interval).floor() as usize) + 1;
    let mut samples = vec![0_f32; block * channels];
    let mut position = 0;
    let mut count = 0;
    let mut max_error = 0_f64;
    let mut correct_order = true;
    let mut finite_samples = true;
    while position < total_frames {
        let frames = block.min(total_frames - position);
        renderer.render(
            &mut samples[..frames * channels],
            channels,
            settings,
            |offset, beat| {
                let actual = (position + offset) as f64;
                max_error = max_error.max((actual - count as f64 * interval).abs());
                correct_order &= usize::from(beat) == count % 12;
                count += 1;
            },
        );
        finite_samples &= samples[..frames * channels]
            .iter()
            .all(|sample| sample.is_finite() && sample.abs() < 1.);
        position += frames;
    }
    json!({
        "sample_rate_hz": rate, "buffer_frames": block, "bpm": bpm,
        "beat_unit": unit.denominator(), "channels": channels, "virtual_duration_seconds": seconds,
        "expected_beats": expected_count, "rendered_beats": count,
        "maximum_error_frames": max_error, "correct_bar_order": correct_order,
        "finite_bounded_samples": finite_samples,
        "passed": count == expected_count && max_error <= 1.01 && correct_order && finite_samples,
    })
}

#[test]
#[ignore = "Release-mode renderer matrix; writes a measurement report, no audio device needed."]
fn renderer_timing_matrix_validation() {
    let mut cases = Vec::new();
    for rate in [44_100, 48_000, 96_000] {
        for block in [64, 127, 256, 511, 1024] {
            for bpm in [40, 120, 137, 240] {
                for unit in BeatUnit::ALL {
                    for channels in [1, 2] {
                        cases.push(measure_renderer(rate, block, bpm, unit, 8, channels));
                    }
                }
            }
        }
    }
    let passed = cases.iter().all(|case| case["passed"] == true);
    write_report(
        "renderer-matrix",
        json!({
            "schema_version": 1, "environment": environment(),
            "measurement": "offline sample-frame scheduling; no device or wall-clock timing measured",
            "passed": passed, "cases": cases,
        }),
    );
    assert!(passed, "renderer matrix failed; inspect the report");
}

#[test]
#[ignore = "Simulates 30 minutes per case; run with --release. No audio device needed."]
fn renderer_thirty_minute_validation() {
    let cases = vec![
        measure_renderer(44_100, 511, 137, BeatUnit::Quarter, 1800, 2),
        measure_renderer(48_000, 127, 137, BeatUnit::Sixteenth, 1800, 1),
        measure_renderer(96_000, 1024, 240, BeatUnit::Sixteenth, 1800, 2),
    ];
    let passed = cases.iter().all(|case| case["passed"] == true);
    write_report(
        "renderer-thirty-minutes",
        json!({
            "schema_version": 1, "environment": environment(),
            "measurement": "offline sample-frame scheduling; 30 virtual minutes per case",
            "passed": passed, "cases": cases,
        }),
    );
    assert!(passed, "long renderer run failed; inspect the report");
}

#[test]
#[ignore = "Plays quiet clicks for ten minutes on the REAL default output and writes diagnostics."]
fn default_output_ten_minute_validation() {
    let started = Instant::now();
    let mut output = match open_default_output() {
        Ok(output) => output,
        Err(error) => {
            write_report(
                "default-output-ten-minutes",
                json!({
                    "schema_version": 1, "environment": environment(), "passed": false,
                    "error": error.to_string(), "measurement": "device could not be opened",
                }),
            );
            panic!("real output validation could not start: {error}");
        }
    };
    let mut meter = Meter::default();
    let mut bpm = 137;
    let mut volume = 15;
    let mut playing = true;
    let playback_started = Instant::now();
    output.configure(bpm, volume, playing, meter);
    let mut changes = Vec::new();
    let mut next_change = 1;
    let mut maximum_poll_gap = Duration::ZERO;
    let mut last_poll = Instant::now();
    let mut observed_beats = 0_u64;
    let mut previous = PlaybackPosition::idle();
    let mut failure = None;
    while playback_started.elapsed() < Duration::from_secs(600) {
        let now = Instant::now();
        maximum_poll_gap = maximum_poll_gap.max(now.duration_since(last_poll));
        last_poll = now;
        let elapsed = now.duration_since(playback_started).as_secs();
        if elapsed >= next_change * 60 && next_change <= 9 {
            match next_change {
                1 => bpm = 40,
                2 => bpm = 240,
                3 => meter = meter.with_beats(12).with_unit(BeatUnit::Sixteenth),
                4 => volume = 0,
                5 => {
                    volume = 15;
                    meter = meter.with_sound(2, super::super::meter::BeatSound::Muted);
                }
                6 => playing = false,
                7 => playing = true,
                8 => {
                    bpm = 137;
                    meter = Meter::default().with_beats(3);
                }
                9 => {
                    bpm = 120;
                    meter = Meter::default();
                }
                _ => unreachable!(),
            }
            output.configure(bpm, volume, playing, meter);
            changes.push(json!({"at_seconds": elapsed, "bpm": bpm, "volume": volume,
                "playing": playing, "beats": meter.beats(), "beat_unit": meter.unit().denominator()}));
            next_change += 1;
        }
        match output.poll(now) {
            Ok(position) => {
                if position.beat().is_some() && position != previous && position.is_pulse() {
                    observed_beats += 1;
                }
                previous = position;
            }
            Err(error) => {
                failure = Some(error.to_string());
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(4));
    }
    output.configure(bpm, volume, false, meter);
    let snapshot = output.controls.diagnostics.snapshot();
    let duration = playback_started.elapsed().as_secs_f64();
    let passed = failure.is_none()
        && duration >= 600.
        && observed_beats > 500
        && snapshot.callbacks > 0
        && snapshot.frames > 0
        && snapshot.beats > 500
        && snapshot.xruns == 0
        && snapshot.stream_errors == 0
        && snapshot.device_changes == 0
        && snapshot.dropped_visual_events == 0
        && snapshot.dropped_error_events == 0
        && maximum_poll_gap <= Duration::from_millis(100);
    write_report(
        "default-output-ten-minutes",
        json!({
            "schema_version": 1, "environment": environment(), "device": output.device_info,
            "measurement": "CPAL callback counters and predicted playback timeline; no acoustic or rendered-frame measurement",
            "passed": passed, "duration_seconds": duration, "open_duration_ms": (playback_started - started).as_secs_f64() * 1000.,
            "maximum_consumer_poll_gap_ms": maximum_poll_gap.as_secs_f64() * 1000.,
            "observed_timeline_pulses": observed_beats, "diagnostics": snapshot,
            "initial_configuration": {"bpm": 137, "volume": 15, "beats": 4, "beat_unit": 4},
            "changes": changes, "error": failure,
            "acoustic_validation": "not measured", "desktop_responsiveness": "not measured",
            "audio_visual_alignment": "not measured", "backend_xrun_reporting": "absence of reported xruns does not prove glitch-free physical output",
        }),
    );
    assert!(passed, "real output validation failed; inspect the report");
}
