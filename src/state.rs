use core::fmt;

use crate::sequencers::euclidean::state::EuclideanSequencerState;
use crate::sequencers::mixer::state::MixerState;

#[derive(Debug, Default)]
pub enum ActiveSequencer {
    #[default]
    Left,
    Right,
}

#[derive(Default)]
pub struct SharedState {
    pub playing: bool,
    pub bpm: f32,
    pub midi_channel: u8,
    pub active_sequencer: ActiveSequencer,
    pub left_state: EuclideanSequencerState,
    pub right_state: EuclideanSequencerState,
    pub mixer_state: MixerState,
    pub clock_ticks: u32,
    pub quarter_notes: u32,
}

impl SharedState {
    pub fn new(bpm: f32) -> Self {
        SharedState {
            playing: false,
            bpm,
            midi_channel: 0,
            active_sequencer: ActiveSequencer::Left,
            left_state: EuclideanSequencerState::new(),
            right_state: EuclideanSequencerState::new(),
            mixer_state: MixerState::new(),
            clock_ticks: 0,
            quarter_notes: 0,
        }
    }

    pub fn increase_bpm(&mut self) {
        self.bpm += 1.0;
    }

    pub fn decrease_bpm(&mut self) {
        self.bpm -= 1.0;
    }

    pub fn change_midi_channel(&mut self) {
        self.midi_channel = (self.midi_channel + 1) % 16;
    }

    pub fn switch_active_sequencer(&mut self) {
        match self.active_sequencer {
            ActiveSequencer::Left => self.active_sequencer = ActiveSequencer::Right,
            ActiveSequencer::Right => self.active_sequencer = ActiveSequencer::Left,
        }
    }
}

impl fmt::Debug for SharedState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared State")
            .field("playing", &self.playing)
            .field("bpm", &self.bpm)
            .field("midi channel", &self.midi_channel)
            .field("active sequencer", &self.active_sequencer)
            .field("left sequencer state", &self.left_state)
            .field("right sequencer state", &self.right_state)
            .field("mixer state", &self.mixer_state)
            .field("clock ticks", &self.clock_ticks)
            .field("quarter notes", &self.quarter_notes)
            .finish()
    }
}
