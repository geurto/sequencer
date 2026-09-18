//! Where the time comes from.
//!
//! The playback loop reads the time through [`Clock`] rather than calling
//! `Instant::now` directly, so tests can step time forward deterministically
//! instead of sleeping, and so an embedded build can substitute a hardware
//! timer.

/// A monotonic source of elapsed microseconds.
///
/// Only the *differences* between readings are meaningful; implementations are
/// free to pick any origin.
pub trait Clock {
    fn now_us(&self) -> u64;
}

/// Wall-clock time, measured from the moment the clock was created.
#[cfg(any(test, feature = "std"))]
#[derive(Clone, Debug)]
pub struct SystemClock {
    origin: std::time::Instant,
}

#[cfg(any(test, feature = "std"))]
impl SystemClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: std::time::Instant::now(),
        }
    }
}

#[cfg(any(test, feature = "std"))]
impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "std"))]
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
/// # // requires the `test-util` feature
/// use seq_core::{Clock, ManualClock};
///
/// let clock = ManualClock::new();
/// let reader = clock.clone();
///
/// clock.advance_us(250);
/// assert_eq!(reader.now_us(), 250);
/// ```
#[cfg(any(test, feature = "test-util"))]
#[derive(Clone, Debug, Default)]
pub struct ManualClock {
    now_us: std::sync::Arc<core::sync::atomic::AtomicU64>,
}

#[cfg(any(test, feature = "test-util"))]
impl ManualClock {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Move time forward.
    pub fn advance_us(&self, us: u64) {
        self.now_us
            .fetch_add(us, core::sync::atomic::Ordering::Relaxed);
    }

    /// Move time forward by whole milliseconds.
    pub fn advance_ms(&self, ms: u64) {
        self.advance_us(ms * 1_000);
    }
}

#[cfg(any(test, feature = "test-util"))]
impl Clock for ManualClock {
    fn now_us(&self) -> u64 {
        self.now_us.load(core::sync::atomic::Ordering::Relaxed)
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
