#![expect(
    clippy::expect_used,
    reason = "Temporary settings fixtures must be readable and writable."
)]

use super::*;

#[test]
fn missing_file_uses_defaults_without_creating_a_file() {
    let folder = tempfile::tempdir().expect("temporary directory");
    let path = folder.path().join("metronomo/settings.json");
    let store = SettingsStore::new(path.clone());
    assert_eq!(store.load().expect("default settings"), Settings::default());
    assert!(!path.exists());
}

#[test]
fn saves_and_replaces_all_settings_including_hidden_beats() {
    let folder = tempfile::tempdir().expect("temporary directory");
    let path = folder.path().join("metronomo/settings.json");
    let store = SettingsStore::new(path.clone());
    store.save(&Settings::default()).expect("first save");
    let mut meter = Meter::default().with_beats(3).with_unit(BeatUnit::Eighth);
    for beat in 0..MAX_BEATS {
        meter = meter.with_sound(beat, BeatSound::ALL[usize::from(beat % 4)]);
    }
    let settings = Settings::new(137, 0, meter);
    store.save(&settings).expect("replace settings");
    let loaded = SettingsStore::new(path).load().expect("reload settings");
    assert_eq!(loaded, settings);
    assert_eq!(loaded.meter().with_beats(12).sound(11), BeatSound::Muted);
    assert_eq!(
        fs::read_dir(folder.path().join("metronomo"))
            .expect("config folder")
            .count(),
        1
    );
}

#[test]
fn defaults_missing_fields_and_rejects_invalid_or_future_settings() {
    let folder = tempfile::tempdir().expect("temporary directory");
    let path = folder.path().join("settings.json");
    let store = SettingsStore::new(path.clone());
    fs::write(&path, br#"{"bpm": 140}"#).expect("partial file");
    assert_eq!(store.load().expect("partial settings").bpm(), 140);
    for content in [
        r#"{"bpm": 0}"#,
        r#"{"bpm": 241}"#,
        r#"{"volume": 101}"#,
        r#"{"beats_per_bar": 13}"#,
        r#"{"beats_per_bar": 0}"#,
        r#"{"beat_unit": 3}"#,
        r#"{"version": 999}"#,
        r#"{"sounds": ["unknown"]}"#,
        "{truncated",
    ] {
        fs::write(&path, content).expect("invalid file");
        assert!(store.load().is_err(), "accepted {content}");
    }
    fs::write(&path, vec![b' '; MAX_FILE_BYTES as usize + 1]).expect("large file");
    assert!(matches!(store.load(), Err(SettingsError::TooLarge)));
}

#[test]
fn failed_atomic_replace_leaves_the_existing_destination_untouched() {
    let folder = tempfile::tempdir().expect("temporary directory");
    let path = folder.path().join("settings.json");
    fs::create_dir(&path).expect("non-file destination");
    fs::write(path.join("keep"), b"original").expect("original data");
    let store = SettingsStore::new(path.clone());
    assert!(store.save(&Settings::default()).is_err());
    assert_eq!(
        fs::read(path.join("keep")).expect("preserved data"),
        b"original"
    );
    assert_eq!(
        fs::read_dir(folder.path())
            .expect("temporary directory")
            .count(),
        1
    );
}

#[test]
fn shutdown_snapshot_cannot_be_overwritten_by_an_older_queued_save() {
    let folder = tempfile::tempdir().expect("temporary directory");
    let store = SettingsStore::new(folder.path().join("settings.json"));
    let older_writer = store.clone();
    let latest = Settings::new(172, 25, Meter::default().with_beats(7));
    store.begin_shutdown();
    store.save_on_shutdown(&latest).expect("final snapshot");
    older_writer
        .save(&Settings::default())
        .expect("superseded write");
    assert_eq!(store.load().expect("final saved settings"), latest);
}
