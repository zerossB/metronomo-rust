use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::configuration::Configuration;

pub(super) const MAX_PRESETS: usize = 64;
const MAX_NAME_CHARS: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Preset {
    id: u32,
    name: String,
    configuration: Configuration,
}

impl Preset {
    pub(super) fn id(&self) -> u32 {
        self.id
    }
    pub(super) fn name(&self) -> &str {
        &self.name
    }
    pub(super) fn configuration(&self) -> Configuration {
        self.configuration
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct PresetLibrary {
    next_id: u32,
    items: Vec<Preset>,
}

impl Default for PresetLibrary {
    fn default() -> Self {
        Self {
            next_id: 1,
            items: Vec::new(),
        }
    }
}

impl PresetLibrary {
    pub(super) fn items(&self) -> &[Preset] {
        &self.items
    }
    pub(super) fn get(&self, id: u32) -> Option<&Preset> {
        self.items.iter().find(|preset| preset.id == id)
    }

    pub(super) fn add(
        &mut self,
        name: &str,
        configuration: Configuration,
    ) -> Result<u32, PresetError> {
        let name = self.available_name(name, None)?;
        if self.items.len() >= MAX_PRESETS {
            return Err(PresetError::Full);
        }
        if !configuration.is_valid() {
            return Err(PresetError::InvalidConfiguration);
        }
        let next_id = self.next_id.checked_add(1).ok_or(PresetError::Full)?;
        let id = self.next_id;
        self.items.push(Preset {
            id,
            name,
            configuration,
        });
        self.next_id = next_id;
        Ok(id)
    }

    pub(super) fn rename(&mut self, id: u32, name: &str) -> Result<(), PresetError> {
        let name = self.available_name(name, Some(id))?;
        let preset = self
            .items
            .iter_mut()
            .find(|preset| preset.id == id)
            .ok_or(PresetError::NotFound)?;
        preset.name = name;
        Ok(())
    }

    pub(super) fn remove(&mut self, id: u32) -> Option<Preset> {
        let ix = self.items.iter().position(|preset| preset.id == id)?;
        Some(self.items.remove(ix))
    }

    pub(super) fn restore(&mut self, preset: Preset) -> Result<(), PresetError> {
        self.available_name(&preset.name, None)?;
        if self.items.len() >= MAX_PRESETS {
            return Err(PresetError::Full);
        }
        if self.get(preset.id).is_some() {
            return Err(PresetError::InvalidLibrary);
        }
        self.items.push(preset);
        self.items.sort_by_key(|preset| preset.id);
        Ok(())
    }

    fn available_name(&self, name: &str, except: Option<u32>) -> Result<String, PresetError> {
        let name = name.trim();
        if name.is_empty()
            || name.chars().count() > MAX_NAME_CHARS
            || name.chars().any(char::is_control)
        {
            return Err(PresetError::InvalidName);
        }
        let normalized = name.to_lowercase();
        if self
            .items
            .iter()
            .any(|preset| Some(preset.id) != except && preset.name.to_lowercase() == normalized)
        {
            return Err(PresetError::DuplicateName);
        }
        Ok(name.to_owned())
    }

    pub(super) fn validate(&self) -> Result<(), PresetError> {
        if self.items.len() > MAX_PRESETS || self.next_id == 0 {
            return Err(PresetError::InvalidLibrary);
        }
        for (ix, preset) in self.items.iter().enumerate() {
            self.available_name(&preset.name, Some(preset.id))?;
            if preset.id == 0
                || preset.id >= self.next_id
                || preset.name.trim() != preset.name
                || !preset.configuration.is_valid()
                || self.items[..ix].iter().any(|other| other.id == preset.id)
            {
                return Err(PresetError::InvalidLibrary);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(super) enum PresetError {
    #[error("Use a name with 1–64 characters, without control characters.")]
    InvalidName,
    #[error("A preset with this name already exists. Choose another name.")]
    DuplicateName,
    #[error("You can save up to 64 presets. Delete one to add another.")]
    Full,
    #[error("This preset is no longer available.")]
    NotFound,
    #[error("This preset contains unsupported settings.")]
    InvalidConfiguration,
    #[error("The saved preset library is invalid.")]
    InvalidLibrary,
}

#[cfg(test)]
mod tests;
