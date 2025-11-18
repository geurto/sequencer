use std::fmt;

use device_query::Keycode;
use log::info;
use midir::MidiOutputConnection;

use crate::{EuclideanSequencerState, MixerState};

pub const TICKS_PER_QUARTER_NOTE: u32 = 480;

// MIDI event to send over midir
#[derive(Debug, Clone, Copy)]
pub enum MidiEventType {
    NoteOn { pitch: u8, velocity: u8 },
    NoteOff { pitch: u8 },
}

// Link an absolute timestamp to the MIDI event
#[derive(Debug, Clone, Copy)]
pub struct TimedEvent {
    pub tick: u32,
    pub event: MidiEventType,
}

// Sequence is just a collection of timed events
#[derive(Debug, Clone, Default)]
pub struct PolyphonicSequence {
    pub events: Vec<TimedEvent>, // sort this by tick
    pub total_ticks: u32,
}

// Commands FROM the UI/input TO the playback thread
pub enum PlaybackCommand {
    LoadSequence(PolyphonicSequence),
    SetMidiChannel(u8),
    SetBPM(f64),
    SetOutputConnection(MidiOutputConnection),
}

// Data FROM the playback thread TO the UI
#[derive(Debug, Clone)]
pub enum PlaybackStatus {
    NotePlayed(usize),
    InputChanged(Vec<Keycode>),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum SequencerSlot {
    #[default]
    Left,
    Right,
}

#[derive(Clone, Default)]
pub struct SharedState {
    pub is_playing: bool,
    pub bpm: f32,
    pub midi_channel: u8,
    pub active_sequencer: SequencerSlot,
    pub current_note_index: usize,
    pub left_sequencer: EuclideanSequencerState,
    pub right_sequencer: EuclideanSequencerState,
    pub mixer: MixerState,
    pub clock_ticks: u32,
    pub quarter_notes: u32,
}

impl SharedState {
    pub fn new(bpm: f32) -> Self {
        SharedState {
            is_playing: false,
            bpm,
            midi_channel: 0,
            active_sequencer: SequencerSlot::Left,
            current_note_index: 0,
            left_sequencer: EuclideanSequencerState::new(),
            right_sequencer: EuclideanSequencerState::new(),
            mixer: MixerState::new(),
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

impl fmt::Debug for SharedState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared State")
            .field("playing", &self.is_playing)
            .field("bpm", &self.bpm)
            .field("midi channel", &self.midi_channel)
            .field("active sequencer", &self.active_sequencer)
            .field("left sequencer state", &self.left_sequencer)
            .field("right sequencer state", &self.right_sequencer)
            .field("mixer state", &self.mixer)
            .field("clock ticks", &self.clock_ticks)
            .field("quarter notes", &self.quarter_notes)
            .finish()
    }
}
