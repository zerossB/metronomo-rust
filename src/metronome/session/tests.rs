#![expect(
    clippy::expect_used,
    reason = "Session fixtures must initialize successfully."
)]

use super::*;
use crate::metronome::{
    meter::{BeatUnit, MAX_BEATS},
    playback::PlaybackState,
    settings::SettingsStore,
    test_support::{DeviceState, TestPlayback},
};
use gpui_kit::{Subscription, TestAppContext};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

fn open(
    cx: &mut TestAppContext,
    preferences: Preferences,
) -> (Entity<MetronomeSession>, Arc<Mutex<DeviceState>>) {
    let device = Arc::new(Mutex::new(DeviceState::default()));
    let output = device.clone();
    let factory: AudioFactory = Arc::new(move || {
        output.lock().expect("device").opens += 1;
        Ok(Box::new(TestPlayback(output.clone())))
    });
    let session = cx.new(|cx| MetronomeSession::new(preferences, factory, cx));
    (session, device)
}

#[gpui_kit::test]
fn configuration_commands_keep_the_model_audio_and_disk_coherent_without_a_window(
    cx: &mut TestAppContext,
) {
    let folder = tempfile::tempdir().expect("settings folder");
    let store = SettingsStore::new(folder.path().join("settings.json"));
    let (session, device) = open(cx, Preferences::from_store(store.clone()));
    session.update(cx, |session, cx| session.toggle_playback(cx));
    cx.run_until_parked();
    let meter = Meter::default()
        .with_beats(3)
        .with_unit(BeatUnit::Eighth)
        .with_sound(11, BeatSound::Low);
    session.update(cx, |session, cx| {
        session.set_bpm(137, cx);
        session.set_volume(25, cx);
        session.set_meter(meter, cx);
    });
    cx.run_until_parked();
    let expected = Configuration::new(137, 25, meter);
    cx.update(|cx| {
        assert_eq!(session.read(cx).configuration(), expected);
        assert_eq!(
            session.read(cx).settings(),
            store.load().expect("saved snapshot")
        );
    });
    let device = device.lock().expect("device");
    assert_eq!(
        (
            device.bpm,
            device.volume,
            device.meter,
            device.configures,
            device.opens,
            device.playing
        ),
        (137, 25, meter, 4, 1, true)
    );
}

#[gpui_kit::test]
fn preset_commands_and_defaults_preserve_playback_and_all_hidden_sound_slots(
    cx: &mut TestAppContext,
) {
    let folder = tempfile::tempdir().expect("settings folder");
    let store = SettingsStore::new(folder.path().join("settings.json"));
    let (session, device) = open(cx, Preferences::from_store(store.clone()));
    let mut meter = Meter::default()
        .with_beats(3)
        .with_unit(BeatUnit::Sixteenth);
    for beat in 0..MAX_BEATS {
        meter = meter.with_sound(beat, BeatSound::ALL[usize::from(beat % 4)]);
    }
    let configuration = Configuration::new(173, 35, meter);
    let id = session.update(cx, |session, cx| {
        session.apply_configuration(configuration, cx);
        session.save_preset("Practice", cx).expect("save preset")
    });
    session.update(cx, |session, cx| session.toggle_playback(cx));
    cx.run_until_parked();
    session.update(cx, |session, cx| {
        session.restore_defaults(cx);
        session.load_preset(id, cx).expect("load preset");
    });
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(session.read(cx).configuration(), configuration);
        assert_eq!(
            session.read(cx).settings(),
            store.load().expect("loaded snapshot")
        );
    });
    session.update(cx, |session, cx| session.restore_defaults(cx));
    cx.run_until_parked();
    cx.update(|cx| {
        let session = session.read(cx);
        assert_eq!(session.configuration(), Configuration::default());
        assert_eq!(
            session
                .presets()
                .get(id)
                .expect("retained preset")
                .configuration(),
            configuration
        );
        assert_eq!(session.playback().read(cx).state(), PlaybackState::Running);
        assert_eq!(session.settings(), store.load().expect("default snapshot"));
    });
    let device = device.lock().expect("device");
    assert_eq!(
        (
            device.bpm,
            device.volume,
            device.meter,
            device.opens,
            device.playing
        ),
        (120, 60, Meter::default(), 1, true)
    );
}

struct Events {
    configuration: usize,
    presets: usize,
    taps: usize,
    _subscription: Subscription,
}

#[gpui_kit::test]
fn audible_positions_do_not_emit_configuration_or_preset_events(cx: &mut TestAppContext) {
    let (session, device) = open(cx, Preferences::in_memory());
    let events = cx.new(|cx| {
        let subscription = cx.subscribe(&session, |this: &mut Events, _, event, _| match event {
            SessionEvent::Configuration => this.configuration += 1,
            SessionEvent::Presets => this.presets += 1,
            SessionEvent::Taps => this.taps += 1,
        });
        Events {
            configuration: 0,
            presets: 0,
            taps: 0,
            _subscription: subscription,
        }
    });
    session.update(cx, |session, cx| {
        session.set_volume(45, cx);
        session.save_preset("Quiet", cx).expect("preset");
        session.toggle_playback(cx);
    });
    cx.run_until_parked();
    {
        let mut device = device.lock().expect("device");
        device.beat = Some(2);
        device.pulse = true;
    }
    cx.background_executor
        .advance_clock(Duration::from_millis(16));
    cx.run_until_parked();
    cx.update(|cx| {
        let events = events.read(cx);
        assert_eq!(
            (events.configuration, events.presets, events.taps),
            (1, 1, 0)
        );
        assert_eq!(
            session.read(cx).playback().read(cx).position().beat(),
            Some(2)
        );
    });
}

#[gpui_kit::test]
async fn close_flushes_the_latest_snapshot_and_rejects_later_commands(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().expect("settings folder");
    let store = SettingsStore::new(folder.path().join("settings.json"));
    let (session, device) = open(cx, Preferences::from_store(store.clone()));
    session.update(cx, |session, cx| session.toggle_playback(cx));
    cx.run_until_parked();
    let close = session.update(cx, |session, cx| {
        session.set_bpm(155, cx);
        session.set_volume(32, cx);
        session.save_preset("Final", cx).expect("final preset");
        let close = session.close(cx);
        session.set_bpm(200, cx);
        session.set_volume(100, cx);
        session.restore_defaults(cx);
        session.tap(cx);
        session.toggle_playback(cx);
        assert!(session.save_preset("Too late", cx).is_err());
        close
    });
    close.await;
    let saved = store.load().expect("final snapshot");
    assert_eq!(
        (saved.bpm(), saved.volume(), saved.presets().items().len()),
        (155, 32, 1)
    );
    assert!(!device.lock().expect("stopped output").playing);
}
