use device_query::Keycode;
use log::info;

use crate::playback::engine::BoxedSink;
use crate::{EuclideanSequencerState, MixerState};

pub const TICKS_PER_QUARTER_NOTE: u32 = 480;

/// One sequencer step is a sixteenth note.
pub const TICKS_PER_STEP: u32 = TICKS_PER_QUARTER_NOTE / 4;

pub const MIN_BPM: f64 = 20.0;
pub const MAX_BPM: f64 = 300.0;

// MIDI event to send over midir
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
/// [`PlaybackEngine`](crate::PlaybackEngine) dispatches by walking this list and
/// stopping at the first entry in the future, so an unsorted list does not
/// merely reorder notes — it stalls playback behind the out-of-order event.
/// [`PolyphonicSequence::new`] is the only way to build one, and it sorts.
#[derive(Debug, Clone)]
pub struct PolyphonicSequence {
    events: Vec<TimedEvent>,
    total_ticks: u32,
}

impl PolyphonicSequence {
    pub fn new(mut events: Vec<TimedEvent>, total_ticks: u32) -> Self {
        events.sort_by_key(|e| (e.tick, e.event.order()));
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
        Self::new(Vec::new(), 4 * TICKS_PER_QUARTER_NOTE)
    }
}

// Commands FROM the UI/input TO the playback thread
pub enum PlaybackCommand {
    LoadSequence(PolyphonicSequence),
    SetMidiChannel(u8),
    SetBPM(f64),
    SetOutputConnection(BoxedSink),
}

// Data FROM the playback thread TO the UI
#[derive(Debug, Clone)]
pub enum PlaybackStatus {
    NotePlayed(usize),
    InputChanged(Vec<Keycode>),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SequencerSlot {
    #[default]
    Left,
    Right,
}

#[derive(Clone, Debug)]
pub struct SharedState {
    pub is_playing: bool,
    pub bpm: f64,
    pub midi_channel: u8,
    pub active_sequencer: SequencerSlot,
    pub current_note_index: usize,
    pub left_sequencer: EuclideanSequencerState,
    pub right_sequencer: EuclideanSequencerState,
    pub mixer: MixerState,
}

/// Deriving `Default` would give `bpm: 0.0`, which stalls the playback clock.
impl Default for SharedState {
    fn default() -> Self {
        Self::new(120.0)
    }
}

impl SharedState {
    pub fn new(bpm: f64) -> Self {
        SharedState {
            is_playing: false,
            bpm: bpm.clamp(MIN_BPM, MAX_BPM),
            midi_channel: 0,
            active_sequencer: SequencerSlot::Left,
            current_note_index: 0,
            left_sequencer: EuclideanSequencerState::new(),
            right_sequencer: EuclideanSequencerState::new(),
            mixer: MixerState::new(),
        }
    }

    pub fn increase_bpm(&mut self) {
        self.bpm = (self.bpm + 1.0).clamp(MIN_BPM, MAX_BPM);
        info!("BPM: {}", self.bpm);
    }

    pub fn decrease_bpm(&mut self) {
        self.bpm = (self.bpm - 1.0).clamp(MIN_BPM, MAX_BPM);
        info!("BPM: {}", self.bpm);
    }

    pub fn change_midi_channel(&mut self) {
        self.midi_channel = (self.midi_channel + 1) % 16;
    }

    pub fn increase_steps(&mut self) {
        match self.active_sequencer {
            SequencerSlot::Left => self.left_sequencer.increase_steps(),
            SequencerSlot::Right => self.right_sequencer.increase_steps(),
        }
    }

    pub fn decrease_steps(&mut self) {
        match self.active_sequencer {
            SequencerSlot::Left => self.left_sequencer.decrease_steps(),
            SequencerSlot::Right => self.right_sequencer.decrease_steps(),
        }
    }

    pub fn increase_pulses(&mut self) {
        match self.active_sequencer {
            SequencerSlot::Left => self.left_sequencer.increase_pulses(),
            SequencerSlot::Right => self.right_sequencer.increase_pulses(),
        }
    }

    pub fn decrease_pulses(&mut self) {
        match self.active_sequencer {
            SequencerSlot::Left => self.left_sequencer.decrease_pulses(),
            SequencerSlot::Right => self.right_sequencer.decrease_pulses(),
        }
    }

    pub fn change_pitch(&mut self, amount: i8) {
        match self.active_sequencer {
            SequencerSlot::Left => self.left_sequencer.change_pitch(amount),
            SequencerSlot::Right => self.right_sequencer.change_pitch(amount),
        }
    }

    pub fn switch_active_sequencer(&mut self) {
        match self.active_sequencer {
            SequencerSlot::Left => self.active_sequencer = SequencerSlot::Right,
            SequencerSlot::Right => self.active_sequencer = SequencerSlot::Left,
        }
        info!("Switched sequencer to {:?}", self.active_sequencer);
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

    #[test]
    fn test_new_sorts_events_by_tick() {
        let sequence = PolyphonicSequence::new(
            vec![
                note_on(240, 64),
                note_on(0, 60),
                note_off(479, 64),
                note_off(119, 60),
            ],
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
            vec![note_on(120, 60), note_off(120, 60)],
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

    #[test]
    fn test_default_bpm_does_not_stall_the_clock() {
        assert!(SharedState::default().bpm >= MIN_BPM);
    }

    #[test]
    fn test_bpm_is_clamped() {
        let mut state = SharedState::new(MAX_BPM);
        state.increase_bpm();
        assert_eq!(state.bpm, MAX_BPM);

        let mut state = SharedState::new(MIN_BPM);
        state.decrease_bpm();
        assert_eq!(state.bpm, MIN_BPM);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// However events are handed to it, `PolyphonicSequence` must come out
        /// sorted — the engine dispatches by walking the list and stopping at
        /// the first event in the future, so an unsorted list stalls playback
        /// behind the out-of-order entry rather than merely reordering notes.
        #[test]
        fn prop_new_always_sorts(
            events in proptest::collection::vec(
                (0u32..1_000, 0u8..=127, any::<bool>()),
                0..50,
            ),
        ) {
            let events: Vec<TimedEvent> = events
                .into_iter()
                .map(|(tick, pitch, is_on)| TimedEvent {
                    tick,
                    event: if is_on {
                        MidiEventType::NoteOn { pitch, velocity: 100 }
                    } else {
                        MidiEventType::NoteOff { pitch }
                    },
                })
                .collect();
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
