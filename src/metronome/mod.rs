//! Sample-clock audio playback, tempo controls, and synchronized visual feedback.

mod audio;
mod meter;
mod settings;
mod tap_tempo;
mod view;

pub(crate) use settings::Preferences;
pub(crate) use view::{Metronome, init};
