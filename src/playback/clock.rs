//! Where the time comes from.
//!
//! The playback loop reads the time through [`Clock`] rather than calling
//! `Instant::now` directly, so tests can step time forward deterministically
//! instead of sleeping, and so the eventual embedded build can substitute a
//! hardware timer.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// A monotonic source of elapsed microseconds.
///
/// Only the *differences* between readings are meaningful; implementations are
/// free to pick any origin.
pub trait Clock {
    fn now_us(&self) -> u64;
}

/// Wall-clock time, measured from the moment the clock was created.
#[derive(Clone, Debug)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now_us(&self) -> u64 {
        // u64 microseconds covers ~584,000 years from the origin; saturating
        // rather than casting keeps the conversion total regardless.
        u64::try_from(self.origin.elapsed().as_micros()).unwrap_or(u64::MAX)
    }
}

/// A clock that only moves when told to.
///
/// Cloning shares the underlying time, so a test can keep a handle after
/// handing the clock to the code under test:
///
/// ```
/// use sequencer::playback::clock::{Clock, ManualClock};
///
/// let clock = ManualClock::new();
/// let reader = clock.clone();
///
/// clock.advance_us(250);
/// assert_eq!(reader.now_us(), 250);
/// ```
#[derive(Clone, Debug, Default)]
pub struct ManualClock {
    now_us: Arc<AtomicU64>,
}

impl ManualClock {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Move time forward.
    pub fn advance_us(&self, us: u64) {
        self.now_us.fetch_add(us, Ordering::Relaxed);
    }

    /// Move time forward by whole milliseconds.
    pub fn advance_ms(&self, ms: u64) {
        self.advance_us(ms * 1_000);
    }
}

impl Clock for ManualClock {
    fn now_us(&self) -> u64 {
        self.now_us.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manual_clock_only_moves_when_told() {
        let clock = ManualClock::new();
        assert_eq!(clock.now_us(), 0);

        clock.advance_ms(2);
        assert_eq!(clock.now_us(), 2_000);

        assert_eq!(clock.now_us(), 2_000, "reading must not advance time");
    }

    #[test]
    fn test_clones_share_one_timeline() {
        let clock = ManualClock::new();
        let reader = clock.clone();

        clock.advance_us(125);

        assert_eq!(reader.now_us(), 125);
    }

    #[test]
    fn test_system_clock_is_monotonic() {
        let clock = SystemClock::new();
        let first = clock.now_us();
        let second = clock.now_us();

        assert!(second >= first);
    }
}
