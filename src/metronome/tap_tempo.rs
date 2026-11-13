use std::time::{Duration, Instant};

use super::audio::{MAX_BPM, MIN_BPM};

pub(super) const RESET_AFTER: Duration = Duration::from_secs(3);
const MIN_INTERVAL: Duration = Duration::from_millis(100);
const INTERVAL_COUNT: usize = 4;

/// Measures quarter-note tempo from up to four recent intervals.
#[derive(Default)]
pub(super) struct TapTempoState {
    last: Option<Instant>,
    intervals: [Duration; INTERVAL_COUNT],
    next_ix: usize,
    len: usize,
}

impl TapTempoState {
    /// The first tap starts a sequence; very rapid duplicate inputs are ignored.
    pub(super) fn tap(&mut self, now: Instant) -> Option<u16> {
        let Some(last) = self.last else {
            self.last = Some(now);
            return None;
        };
        let interval = now.saturating_duration_since(last);
        if interval >= RESET_AFTER {
            *self = Self::default();
            self.last = Some(now);
            return None;
        }
        if interval < MIN_INTERVAL {
            return None;
        }

        self.last = Some(now);
        self.intervals[self.next_ix] = interval;
        self.next_ix = (self.next_ix + 1) % INTERVAL_COUNT;
        self.len = (self.len + 1).min(INTERVAL_COUNT);
        let seconds: f64 = self.intervals[..self.len]
            .iter()
            .map(Duration::as_secs_f64)
            .sum();
        Some(
            (60. * self.len as f64 / seconds)
                .round()
                .clamp(f64::from(MIN_BPM), f64::from(MAX_BPM)) as u16,
        )
    }

    pub(super) fn taps(&self) -> usize {
        self.last.map_or(0, |_| self.len + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn averages_intervals_and_keeps_only_the_four_most_recent() {
        let mut taps = TapTempoState::default();
        let mut now = Instant::now();
        assert_eq!(taps.tap(now), None);
        for (milliseconds, bpm) in [(400, 150), (600, 120), (500, 120), (500, 120), (1000, 92)] {
            now += Duration::from_millis(milliseconds);
            assert_eq!(taps.tap(now), Some(bpm));
        }
    }

    #[test]
    fn a_pause_starts_a_new_sequence_without_changing_tempo_on_the_first_tap() {
        let mut taps = TapTempoState::default();
        let now = Instant::now();
        taps.tap(now);
        assert_eq!(taps.tap(now + Duration::from_millis(500)), Some(120));
        let restart = now + Duration::from_millis(500) + RESET_AFTER;
        assert_eq!(taps.tap(restart), None);
        assert_eq!(taps.tap(restart + Duration::from_millis(750)), Some(80));
    }

    #[test]
    fn ignores_duplicate_and_out_of_order_taps_without_moving_the_last_valid_tap() {
        let mut taps = TapTempoState::default();
        let now = Instant::now();
        taps.tap(now);
        assert_eq!(taps.tap(now), None);
        assert_eq!(taps.tap(now - Duration::from_millis(10)), None);
        assert_eq!(taps.tap(now + Duration::from_millis(50)), None);
        assert_eq!(taps.tap(now + Duration::from_millis(500)), Some(120));
    }

    #[test]
    fn rounds_estimates_and_respects_the_supported_bpm_range() {
        for (interval, bpm) in [(100, 240), (250, 240), (487, 123), (1500, 40), (2500, 40)] {
            let mut taps = TapTempoState::default();
            let now = Instant::now();
            taps.tap(now);
            assert_eq!(taps.tap(now + Duration::from_millis(interval)), Some(bpm));
        }
    }
}
