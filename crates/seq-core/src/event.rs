//! Timed MIDI events and the sorted event list the transport plays.

use core::fmt;

pub const TICKS_PER_QUARTER_NOTE: u32 = 480;

/// One sequencer step is a sixteenth note.
pub const TICKS_PER_STEP: u32 = TICKS_PER_QUARTER_NOTE / 4;

/// Capacity of a [`PolyphonicSequence`], in events.
///
/// Sized for the desktop mixer's worst case with headroom: two 16-step
/// sources expand to at most lcm(16, 15) = 240 steps, and each step emits at
/// most one Note-On/Note-Off pair per merged voice. Embedded builds that need
/// a smaller footprint tune this constant down and recompile.
pub const EVENT_CAPACITY: usize = 4096;

/// The fixed-capacity vector a [`PolyphonicSequence`] is built from.
pub type EventVec = heapless::Vec<TimedEvent, EVENT_CAPACITY>;

// MIDI event to send through a MidiSink
#[derive(Debug, Clone, Copy)]
pub enum MidiEventType {
    NoteOn { pitch: u8, velocity: u8 },
    NoteOff { pitch: u8 },
}

impl MidiEventType {
    /// Tie-break for events landing on the same tick: releases sort before
    /// attacks, so a repeated pitch is not silenced by its own predecessor's
    /// Note-Off.
    pub(crate) fn order(self) -> u8 {
        match self {
            MidiEventType::NoteOff { .. } => 0,
            MidiEventType::NoteOn { .. } => 1,
        }
    }
}

// Link an absolute timestamp to the MIDI event
#[derive(Debug, Clone, Copy)]
pub struct TimedEvent {
    pub tick: u32,
    pub event: MidiEventType,
}

/// A collection of timed events, guaranteed to be sorted by tick.
///
/// The [`Transport`](crate::Transport) dispatches by walking this list and
/// stopping at the first entry in the future, so an unsorted list does not
/// merely reorder notes — it stalls playback behind the out-of-order event.
/// [`PolyphonicSequence::new`] is the only way to build one, and it sorts.
///
/// Storage is inline and fixed-capacity ([`EVENT_CAPACITY`]), so the type
/// works without an allocator. It is deliberately `Clone` but not `Copy`:
/// at ~32 KB, copies should be visible in the code.
#[derive(Clone)]
pub struct PolyphonicSequence {
    events: EventVec,
    total_ticks: u32,
}

impl PolyphonicSequence {
    #[must_use]
    pub fn new(mut events: EventVec, total_ticks: u32) -> Self {
        // Unstable sort: no allocator, and events equal in (tick, order) are
        // interchangeable — they differ only by pitch.
        events.sort_unstable_by_key(|e| (e.tick, e.event.order()));
        Self {
            events,
            total_ticks,
        }
    }

    #[must_use]
    pub fn events(&self) -> &[TimedEvent] {
        &self.events
    }

    #[must_use]
    pub fn total_ticks(&self) -> u32 {
        self.total_ticks
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl Default for PolyphonicSequence {
    fn default() -> Self {
        Self::new(EventVec::new(), 4 * TICKS_PER_QUARTER_NOTE)
    }
}

/// Hand-written so a sequence prints as a summary, not thousands of events.
impl fmt::Debug for PolyphonicSequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolyphonicSequence")
            .field("events", &self.events.len())
            .field("total_ticks", &self.total_ticks)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note_on(tick: u32, pitch: u8) -> TimedEvent {
        TimedEvent {
            tick,
            event: MidiEventType::NoteOn {
                pitch,
                velocity: 100,
            },
        }
    }

    fn note_off(tick: u32, pitch: u8) -> TimedEvent {
        TimedEvent {
            tick,
            event: MidiEventType::NoteOff { pitch },
        }
    }

    fn events(list: &[TimedEvent]) -> EventVec {
        EventVec::from_slice(list).unwrap()
    }

    #[test]
    fn test_new_sorts_events_by_tick() {
        let sequence = PolyphonicSequence::new(
            events(&[
                note_on(240, 64),
                note_on(0, 60),
                note_off(479, 64),
                note_off(119, 60),
            ]),
            480,
        );

        let ticks: Vec<u32> =
            sequence.events().iter().map(|e| e.tick).collect();
        assert_eq!(ticks, vec![0, 119, 240, 479]);
    }

    /// At equal ticks a release must precede an attack, or re-striking the same
    /// pitch is immediately silenced by the previous note's Note-Off.
    #[test]
    fn test_note_off_sorts_before_note_on_at_the_same_tick() {
        let sequence = PolyphonicSequence::new(
            events(&[note_on(120, 60), note_off(120, 60)]),
            480,
        );

        assert!(matches!(
            sequence.events()[0].event,
            MidiEventType::NoteOff { .. }
        ));
        assert!(matches!(
            sequence.events()[1].event,
            MidiEventType::NoteOn { .. }
        ));
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// However events are handed to it, `PolyphonicSequence` must come out
        /// sorted — the transport dispatches by walking the list and stopping
        /// at the first event in the future, so an unsorted list stalls
        /// playback behind the out-of-order entry rather than merely
        /// reordering notes.
        #[test]
        fn prop_new_always_sorts(
            raw in proptest::collection::vec(
                (0u32..1_000, 0u8..=127, any::<bool>()),
                0..50,
            ),
        ) {
            let mut events = EventVec::new();
            for (tick, pitch, is_on) in raw {
                events
                    .push(TimedEvent {
                        tick,
                        event: if is_on {
                            MidiEventType::NoteOn { pitch, velocity: 100 }
                        } else {
                            MidiEventType::NoteOff { pitch }
                        },
                    })
                    .unwrap();
            }
            let count = events.len();

            let sequence = PolyphonicSequence::new(events, 1_000);

            prop_assert_eq!(sequence.events().len(), count, "events were lost");
            prop_assert!(
                sequence.events().windows(2).all(|w| w[0].tick <= w[1].tick),
                "not sorted by tick"
            );
            // At equal ticks, releases must come first.
            prop_assert!(
                sequence.events().windows(2).all(|w| {
                    w[0].tick != w[1].tick
                        || w[0].event.order() <= w[1].event.order()
                }),
                "a Note-On sorted before a Note-Off at the same tick"
            );
        }
    }
}
