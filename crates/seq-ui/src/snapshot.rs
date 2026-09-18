//! What a renderer needs to draw one frame, and nothing else.

use seq_core::{MAX_STEPS, Pattern};

/// Which of the two sequencer slots.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Slot {
    #[default]
    Left,
    Right,
}

impl Slot {
    #[must_use]
    pub fn other(self) -> Self {
        match self {
            Slot::Left => Slot::Right,
            Slot::Right => Slot::Left,
        }
    }

    /// Index into [`UiSnapshot::slots`].
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Slot::Left => 0,
            Slot::Right => 1,
        }
    }
}

/// One slot's parameters and the pattern they currently produce.
///
/// The struck steps are carried as data (`hits`) rather than recomputed by the
/// renderer, so the display keeps agreeing with what plays when a slot's
/// pattern stops being Euclidean (Part C's external sources).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlotSnapshot {
    pub steps: u8,
    pub pulses: u8,
    pub phase: u8,
    pub pitch: u8,
    /// Bit `i` is set when step `i` strikes at least one voice.
    pub hits: u64,
}

impl SlotSnapshot {
    /// Whether step `index` strikes. `false` past the end of the pattern.
    #[must_use]
    pub fn hit(&self, index: usize) -> bool {
        index < usize::from(self.steps)
            && index < u64::BITS as usize
            && self.hits & (1 << index) != 0
    }
}

impl Default for SlotSnapshot {
    fn default() -> Self {
        Self {
            steps: 16,
            pulses: 0,
            phase: 0,
            pitch: 60,
            hits: 0,
        }
    }
}

// MAX_STEPS is 64, so every step of a pattern has a bit in the mask.
const _: () = assert!(MAX_STEPS <= u64::BITS as usize);

/// The hit bitmask of a pattern, for [`SlotSnapshot::hits`].
#[must_use]
pub fn hits(pattern: &Pattern) -> u64 {
    let mut mask = 0u64;
    for index in 0..pattern.len() {
        if pattern.hit(index) {
            mask |= 1 << index;
        }
    }
    mask
}

/// Everything a renderer needs to draw one frame.
///
/// Flat, `Copy` and integer-only — no desktop types, no `Arc`, no `Vec`, no
/// floats (the intended MCU has no FPU). This is the payload of the state
/// `watch` channel on the desktop and of the `Signal` on the MCU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiSnapshot {
    pub playing: bool,
    /// Tempo in thousandths of a beat per minute (120 BPM = `120_000`).
    pub bpm_milli: u32,
    /// 0-based; displays conventionally show it 1-based.
    pub midi_channel: u8,
    pub active: Slot,
    /// The play head's step within the mixed sequence. Per-slot displays
    /// reduce it modulo their own step count.
    pub step_index: u8,
    pub slots: [SlotSnapshot; 2],
    /// Crossfade position, `0` fully left to `255` fully right.
    pub mix: u8,
}

impl UiSnapshot {
    #[must_use]
    pub fn active_slot(&self) -> &SlotSnapshot {
        &self.slots[self.active.index()]
    }
}

impl Default for UiSnapshot {
    fn default() -> Self {
        Self {
            playing: false,
            bpm_milli: 120_000,
            midi_channel: 0,
            active: Slot::Left,
            step_index: 0,
            slots: [SlotSnapshot::default(); 2],
            mix: 128,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seq_core::euclid;

    #[test]
    fn test_hits_match_the_pattern() {
        let pattern = euclid::pattern(8, 3, 0, 60, 100);
        let mask = hits(&pattern);

        for index in 0..8 {
            assert_eq!(
                mask & (1 << index) != 0,
                pattern.hit(index),
                "bit {index} disagrees with the pattern"
            );
        }
        assert_eq!(mask >> 8, 0, "bits past the pattern length must be clear");
    }

    #[test]
    fn test_slot_hit_is_bounded() {
        let snapshot = SlotSnapshot {
            steps: 4,
            hits: u64::MAX,
            ..SlotSnapshot::default()
        };

        assert!(snapshot.hit(0));
        assert!(snapshot.hit(3));
        assert!(!snapshot.hit(4), "past the step count must read as a rest");
        assert!(!snapshot.hit(usize::MAX));
    }

    #[test]
    fn test_slot_other_and_index() {
        assert_eq!(Slot::Left.other(), Slot::Right);
        assert_eq!(Slot::Right.other(), Slot::Left);
        assert_eq!(Slot::Left.index(), 0);
        assert_eq!(Slot::Right.index(), 1);
    }

    #[test]
    fn test_default_snapshot_is_sane() {
        let snapshot = UiSnapshot::default();

        assert!(!snapshot.playing);
        assert_eq!(snapshot.bpm_milli, 120_000);
        assert_eq!(snapshot.active_slot().steps, 16);
    }
}
