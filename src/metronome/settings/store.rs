//! Atomic, bounded settings I/O. Call file operations on a worker thread.
use super::{MAX_FILE_BYTES, Settings, SettingsError};
use std::{
    fs::{self, File},
    io::{self, Read as _, Write as _},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone)]
pub(in crate::metronome) struct SettingsStore {
    path: PathBuf,
    writer: Arc<Writer>,
}

#[derive(Default)]
struct Writer {
    lock: Mutex<()>,
    shutting_down: AtomicBool,
}

impl SettingsStore {
    pub(super) fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub(super) fn for_user() -> Result<Self, SettingsError> {
        let folder = dirs::config_dir().ok_or(SettingsError::NoConfigDirectory)?;
        Ok(Self::new(folder.join("metronomo").join("settings.json")))
    }

    pub(in crate::metronome) fn new(path: PathBuf) -> Self {
        Self {
            path,
            writer: Arc::new(Writer::default()),
        }
    }

    pub(in crate::metronome) fn load(&self) -> Result<Settings, SettingsError> {
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

    pub(in crate::metronome) fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        self.write(settings, false)
    }

    pub(in crate::metronome) fn begin_shutdown(&self) {
        self.writer.shutting_down.store(true, Ordering::Release);
    }

    pub(in crate::metronome) fn save_on_shutdown(
        &self,
        settings: &Settings,
    ) -> Result<(), SettingsError> {
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
