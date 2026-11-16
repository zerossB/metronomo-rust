//! Versioned user preferences and atomic file replacement. Call file operations off the UI thread.

use std::io;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    configuration::Configuration,
    meter::Meter,
    presets::{PresetError, PresetLibrary},
};

mod store;
mod writer;
pub(super) use store::SettingsStore;
pub(super) use writer::PreferencesWriter;

const VERSION: u32 = 1;
const MAX_FILE_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Settings {
    version: u32,
    #[serde(flatten)]
    configuration: Configuration,
    presets: PresetLibrary,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: VERSION,
            configuration: Configuration::default(),
            presets: PresetLibrary::default(),
        }
    }
}

impl Settings {
    pub(super) fn new(bpm: u16, volume: u8, meter: Meter) -> Self {
        Self {
            version: VERSION,
            configuration: Configuration::new(bpm, volume, meter),
            presets: PresetLibrary::default(),
        }
    }

    pub(super) fn bpm(&self) -> u16 {
        self.configuration.bpm()
    }

    pub(super) fn volume(&self) -> u8 {
        self.configuration.volume()
    }

    pub(super) fn meter(&self) -> Meter {
        self.configuration.meter()
    }

    pub(super) fn presets(&self) -> &PresetLibrary {
        &self.presets
    }

    pub(super) fn with_presets(mut self, presets: PresetLibrary) -> Self {
        self.presets = presets;
        self
    }

    fn validate(&self) -> Result<(), SettingsError> {
        if self.version != VERSION {
            return Err(SettingsError::UnsupportedVersion(self.version));
        }
        if !self.configuration.is_valid() {
            return Err(SettingsError::InvalidValues);
        }
        self.presets.validate()?;
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
                    store.path().display()
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
    #[error(transparent)]
    Presets(#[from] PresetError),
    #[error("Settings exceed the maximum file size")]
    TooLarge,
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests;
