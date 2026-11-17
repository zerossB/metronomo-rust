//! Bounded, allocation-free callback counters. Snapshots are read outside the callback.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

#[derive(Default)]
pub(super) struct Diagnostics {
    callbacks: AtomicU64,
    frames: AtomicU64,
    beats: AtomicU64,
    xruns: AtomicU64,
    stream_errors: AtomicU64,
    device_changes: AtomicU64,
    realtime_denied: AtomicU64,
    dropped_visual_events: AtomicU64,
    dropped_error_events: AtomicU64,
}

/// Counters are individually monotonic; a live snapshot is not a transactional sample.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub(super) struct DiagnosticSnapshot {
    pub(super) callbacks: u64,
    pub(super) frames: u64,
    pub(super) beats: u64,
    pub(super) xruns: u64,
    pub(super) stream_errors: u64,
    pub(super) device_changes: u64,
    pub(super) realtime_denied: u64,
    pub(super) dropped_visual_events: u64,
    pub(super) dropped_error_events: u64,
}

impl Diagnostics {
    pub(super) fn rendered(&self, frames: usize) {
        self.callbacks.fetch_add(1, Relaxed);
        self.frames.fetch_add(frames as u64, Relaxed);
    }

    pub(super) fn beat(&self) {
        self.beats.fetch_add(1, Relaxed);
    }

    pub(super) fn dropped_visual(&self) {
        self.dropped_visual_events.fetch_add(1, Relaxed);
    }

    pub(super) fn dropped_error(&self) {
        self.dropped_error_events.fetch_add(1, Relaxed);
    }

    /// CPAL advisory notifications remain nonfatal but are no longer invisible.
    pub(super) fn stream_error(&self, kind: cpal::ErrorKind) -> bool {
        let counter = match kind {
            cpal::ErrorKind::Xrun => &self.xruns,
            cpal::ErrorKind::DeviceChanged => &self.device_changes,
            cpal::ErrorKind::RealtimeDenied => &self.realtime_denied,
            _ => {
                self.stream_errors.fetch_add(1, Relaxed);
                return true;
            }
        };
        counter.fetch_add(1, Relaxed);
        false
    }

    pub(super) fn snapshot(&self) -> DiagnosticSnapshot {
        DiagnosticSnapshot {
            callbacks: self.callbacks.load(Relaxed),
            frames: self.frames.load(Relaxed),
            beats: self.beats.load(Relaxed),
            xruns: self.xruns.load(Relaxed),
            stream_errors: self.stream_errors.load(Relaxed),
            device_changes: self.device_changes.load(Relaxed),
            realtime_denied: self.realtime_denied.load(Relaxed),
            dropped_visual_events: self.dropped_visual_events.load(Relaxed),
            dropped_error_events: self.dropped_error_events.load(Relaxed),
        }
    }
}

impl DiagnosticSnapshot {
    pub(super) fn notices_changed(self, previous: Self) -> bool {
        self.xruns != previous.xruns
            || self.stream_errors != previous.stream_errors
            || self.device_changes != previous.device_changes
            || self.realtime_denied != previous.realtime_denied
            || self.dropped_visual_events != previous.dropped_visual_events
            || self.dropped_error_events != previous.dropped_error_events
    }
}
