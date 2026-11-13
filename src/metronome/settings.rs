//! Versioned user preferences and atomic file replacement. Call file operations off the UI thread.

use std::{
    fs::{self, File},
    io::{self, Read as _, Write as _},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    audio::{DEFAULT_VOLUME, MAX_BPM, MIN_BPM},
    meter::{BeatSound, BeatUnit, MAX_BEATS, Meter},
};

const VERSION: u32 = 1;
const MAX_FILE_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Settings {
    version: u32,
    bpm: u16,
    volume: u8,
    beats_per_bar: u8,
    beat_unit: u8,
    sounds: [BeatSound; MAX_BEATS as usize],
}

impl Default for Settings {
    fn default() -> Self {
        Self::new(120, DEFAULT_VOLUME, Meter::default())
    }
}

impl Settings {
    pub(super) fn new(bpm: u16, volume: u8, meter: Meter) -> Self {
        Self {
            version: VERSION,
            bpm,
            volume,
            beats_per_bar: meter.beats(),
            beat_unit: meter.unit().denominator(),
            sounds: std::array::from_fn(|ix| meter.sound(ix as u8)),
        }
    }

    pub(super) fn bpm(&self) -> u16 {
        self.bpm
    }

    pub(super) fn volume(&self) -> u8 {
        self.volume
    }

    pub(super) fn meter(&self) -> Meter {
        let unit = BeatUnit::ALL
            .into_iter()
            .find(|unit| unit.denominator() == self.beat_unit)
            .unwrap_or(BeatUnit::Quarter);
        self.sounds.iter().enumerate().fold(
            Meter::default()
                .with_beats(self.beats_per_bar)
                .with_unit(unit),
            |meter, (ix, sound)| meter.with_sound(ix as u8, *sound),
        )
    }

    fn validate(&self) -> Result<(), SettingsError> {
        if self.version != VERSION {
            return Err(SettingsError::UnsupportedVersion(self.version));
        }
        if !(MIN_BPM..=MAX_BPM).contains(&self.bpm)
            || self.volume > 100
            || !(1..=MAX_BEATS).contains(&self.beats_per_bar)
            || !BeatUnit::ALL
                .iter()
                .any(|unit| unit.denominator() == self.beat_unit)
        {
            return Err(SettingsError::InvalidValues);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(super) struct SettingsStore {
    path: PathBuf,
    writer: Arc<Writer>,
}

#[derive(Default)]
struct Writer {
    lock: Mutex<()>,
    shutting_down: AtomicBool,
}

impl SettingsStore {
    fn for_user() -> Result<Self, SettingsError> {
        let folder = dirs::config_dir().ok_or(SettingsError::NoConfigDirectory)?;
        Ok(Self::new(folder.join("metronomo").join("settings.json")))
    }

    pub(super) fn new(path: PathBuf) -> Self {
        Self {
            path,
            writer: Arc::new(Writer::default()),
        }
    }

    pub(super) fn load(&self) -> Result<Settings, SettingsError> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Settings::default()),
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(SettingsError::TooLarge);
        }
        let settings: Settings = serde_json::from_slice(&bytes)?;
        settings.validate()?;
        Ok(settings)
    }

    pub(super) fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        self.write(settings, false)
    }

    pub(super) fn begin_shutdown(&self) {
        self.writer.shutting_down.store(true, Ordering::Release);
    }

    pub(super) fn save_on_shutdown(&self, settings: &Settings) -> Result<(), SettingsError> {
        self.write(settings, true)
    }

    fn write(&self, settings: &Settings, shutdown: bool) -> Result<(), SettingsError> {
        // Only worker threads acquire this lock. A shutdown snapshot supersedes
        // any queued save, including a cancelled task already executing file I/O.
        let _writer = self
            .writer
            .lock
            .lock()
            .map_err(|_| io::Error::other("Settings writer lock was poisoned"))?;
        if !shutdown && self.writer.shutting_down.load(Ordering::Acquire) {
            return Ok(());
        }
        settings.validate()?;
        let parent = self.path.parent().ok_or(SettingsError::NoConfigDirectory)?;
        fs::create_dir_all(parent)?;
        let bytes = serde_json::to_vec_pretty(settings)?;
        // A same-directory temporary file keeps interrupted writes from truncating the last save.
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        file.persist(&self.path).map_err(|error| error.error)?;
        Ok(())
    }
}

/// The application loads this on a worker before constructing the first window.
pub(crate) struct Preferences {
    settings: Settings,
    store: Option<SettingsStore>,
    error: Option<&'static str>,
}

impl Preferences {
    pub(crate) fn load() -> Self {
        match SettingsStore::for_user() {
            Ok(store) => Self::from_store(store),
            Err(error) => {
                eprintln!("Could not locate Metronomo settings: {error}");
                Self {
                    settings: Settings::default(),
                    store: None,
                    error: Some("Settings cannot be saved. No configuration folder is available."),
                }
            }
        }
    }

    pub(super) fn from_store(store: SettingsStore) -> Self {
        let (settings, error) = match store.load() {
            Ok(settings) => (settings, None),
            Err(error) => {
                eprintln!(
                    "Could not load Metronomo settings at {}: {error}",
                    store.path.display()
                );
                (
                    Settings::default(),
                    Some("Couldn’t load saved settings. Using defaults."),
                )
            }
        };
        Self {
            settings,
            store: Some(store),
            error,
        }
    }

    pub(super) fn into_parts(self) -> (Settings, Option<SettingsStore>, Option<&'static str>) {
        (self.settings, self.store, self.error)
    }

    #[cfg(test)]
    pub(super) fn in_memory() -> Self {
        Self {
            settings: Settings::default(),
            store: None,
            error: None,
        }
    }
}

#[derive(Debug, Error)]
pub(super) enum SettingsError {
    #[error("No user configuration directory is available")]
    NoConfigDirectory,
    #[error("Unsupported settings version {0}")]
    UnsupportedVersion(u32),
    #[error("Settings contain values outside the supported ranges")]
    InvalidValues,
    #[error("Settings exceed the maximum file size")]
    TooLarge,
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests;
