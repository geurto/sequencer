use core::fmt;

use device_query::Keycode;
use log::info;
use seq_core::PolyphonicSequence;

use crate::playback::engine::BoxedSink;
use crate::{EuclideanSequencerState, MixerState};

pub const MIN_BPM: f64 = 20.0;
pub const MAX_BPM: f64 = 300.0;

// Commands FROM the UI/input TO the playback thread
pub enum PlaybackCommand {
    /// Boxed because the sequence is a ~32 KB fixed-capacity value; inline it
    /// would dominate the enum and get memcpy'd through every channel hop.
    LoadSequence(Box<PolyphonicSequence>),
    SetMidiChannel(u8),
    SetBPM(f64),
    SetOutputConnection(BoxedSink),
}

/// Hand-written because `SetOutputConnection` carries a trait object, and
/// because a whole sequence dumped into a log line is noise.
impl fmt::Debug for PlaybackCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoadSequence(sequence) => f
                .debug_tuple("LoadSequence")
                .field(&format_args!("{} events", sequence.events().len()))
                .finish(),
            Self::SetMidiChannel(channel) => {
                f.debug_tuple("SetMidiChannel").field(channel).finish()
            }
            Self::SetBPM(bpm) => f.debug_tuple("SetBPM").field(bpm).finish(),
            Self::SetOutputConnection(_) => {
                f.write_str("SetOutputConnection(..)")
            }
        }
    }
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
    #[must_use]
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

    pub fn increase_phase(&mut self) {
        match self.active_sequencer {
            SequencerSlot::Left => self.left_sequencer.increase_phase(),
            SequencerSlot::Right => self.right_sequencer.increase_phase(),
        }
    }

    pub fn decrease_phase(&mut self) {
        match self.active_sequencer {
            SequencerSlot::Left => self.left_sequencer.decrease_phase(),
            SequencerSlot::Right => self.right_sequencer.decrease_phase(),
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

    #[test]
    fn test_default_bpm_does_not_stall_the_clock() {
        assert!(SharedState::default().bpm >= MIN_BPM);
    }

    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the clamp returns the bound itself, so the comparison is exact"
    )]
    fn test_bpm_is_clamped() {
        let mut state = SharedState::new(MAX_BPM);
        state.increase_bpm();
        assert_eq!(state.bpm, MAX_BPM);

        let mut state = SharedState::new(MIN_BPM);
        state.decrease_bpm();
        assert_eq!(state.bpm, MIN_BPM);
    }
}
