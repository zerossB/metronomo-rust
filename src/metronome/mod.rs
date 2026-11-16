//! Sample-clock audio playback, tempo controls, and synchronized visual feedback.

mod audio;
mod configuration;
mod meter;
mod playback;
mod presets;
mod session;
mod settings;
mod tap_tempo;
mod view;

pub(crate) use settings::Preferences;
pub(crate) use view::{Metronome, init};

#[cfg(test)]
mod test_support;
