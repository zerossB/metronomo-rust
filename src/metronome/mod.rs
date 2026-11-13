//! Sample-clock audio playback, tempo controls, and synchronized visual feedback.

mod audio;
mod meter;
mod view;

pub(crate) use view::{Metronome, init};
