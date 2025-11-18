use core::fmt;
use device_query::Keycode;
use log::{error, info};

use std::sync::{Arc, Mutex as SyncMutex};
use tokio::sync::mpsc;

use crate::{
    gui::{
        sequencers::euclidean::Message as EuclideanGuiMessage,
        Message as GuiMessage,
    },
    EuclideanSequencerState, MixerState, PlaybackStatus,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub enum SequencerSlot {
    #[default]
    Left,
    Right,
}

#[derive(Clone, Default)]
pub struct SharedState {
    pub active_sequencer: SequencerSlot,
    pub current_note_index: usize,
    pub clock_ticks: u32,
    pub quarter_notes: u32,
}

impl SharedState {
    pub fn new() -> Self {
        SharedState {
            active_sequencer: SequencerSlot::Left,
            current_note_index: 0,
            clock_ticks: 0,
            quarter_notes: 0,
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
            .field("active sequencer", &self.active_sequencer)
            .field("clock ticks", &self.clock_ticks)
            .field("quarter notes", &self.quarter_notes)
            .finish()
    }
}

pub struct StateHandler {
    is_playing: bool,
    midi_channel: u8,
    bpm: f32,
    current_note_index: usize,
    active_sequencer: SequencerSlot,

    left_sequencer: EuclideanSequencerState,
    right_sequencer: EuclideanSequencerState,
    mixer: MixerState,

    rx_engine_status: mpsc::UnboundedReceiver<PlaybackStatus>,
    tx_gui: Arc<
        SyncMutex<Option<iced::futures::channel::mpsc::Sender<GuiMessage>>>,
    >,
}

impl StateHandler {
    pub fn new(
        rx_engine_status: mpsc::UnboundedReceiver<PlaybackStatus>,
        tx_gui: Arc<
            SyncMutex<Option<iced::futures::channel::mpsc::Sender<GuiMessage>>>,
        >,
    ) -> Self {
        Self {
            is_playing: false,
            midi_channel: 0,
            bpm: 120.0,
            current_note_index: 0,
            active_sequencer: SequencerSlot::Left,

            left_sequencer: EuclideanSequencerState::default(),
            right_sequencer: EuclideanSequencerState::default(),
            mixer: MixerState::default(),

            rx_engine_status,
            tx_gui,
        }
    }

    pub async fn run(&mut self) {
        // get synchronous engine status
        while let Ok(status) = self.rx_engine_status.try_recv() {
            match status {
                PlaybackStatus::NotePlayed(i) => {
                    self.current_note_index = i;
                    self.update_gui(GuiMessage::NotePlayed(i)).await;
                }
                PlaybackStatus::InputChanged(input) => {
                    self.handle_input_change(input).await
                }
            };
        }
    }

    pub async fn handle_input_change(&mut self, diff: Vec<Keycode>) {
        let cached_state_left_sequencer = self.left_sequencer.clone();
        let cached_state_right_sequencer = self.right_sequencer.clone();
        let cached_state_mixer = self.mixer.clone();

        for key in diff {
            match key {
                Keycode::Space => {
                    match self.is_playing {
                        true => info!("Paused playback!"),
                        false => info!("Resumed playback!"),
                    };
                    self.is_playing = !self.is_playing
                }
                Keycode::C => {
                    self.midi_channel = (self.midi_channel + 1) % 16;
                    info!("Changing MIDI channel to {}", self.midi_channel + 1);
                }
                Keycode::R => {
                    self.mixer.increase_ratio();
                }
                Keycode::F => {
                    self.mixer.decrease_ratio();
                }
                Keycode::Up => match self.active_sequencer {
                    SequencerSlot::Left => {
                        self.left_sequencer.increase_steps();
                    }
                    SequencerSlot::Right => {
                        self.right_sequencer.increase_steps()
                    }
                },
                Keycode::Down => match self.active_sequencer {
                    SequencerSlot::Left => self.left_sequencer.decrease_steps(),
                    SequencerSlot::Right => {
                        self.right_sequencer.decrease_steps()
                    }
                },
                Keycode::Right => match self.active_sequencer {
                    SequencerSlot::Left => {
                        self.left_sequencer.increase_pulses()
                    }
                    SequencerSlot::Right => {
                        self.right_sequencer.increase_pulses()
                    }
                },
                Keycode::Left => match self.active_sequencer {
                    SequencerSlot::Left => {
                        self.left_sequencer.decrease_pulses()
                    }
                    SequencerSlot::Right => {
                        self.right_sequencer.decrease_pulses()
                    }
                },
                Keycode::W => match self.active_sequencer {
                    SequencerSlot::Left => self.left_sequencer.change_pitch(1),
                    SequencerSlot::Right => {
                        self.right_sequencer.change_pitch(1)
                    }
                },
                Keycode::S => match self.active_sequencer {
                    SequencerSlot::Left => self.left_sequencer.change_pitch(-1),
                    SequencerSlot::Right => {
                        self.right_sequencer.change_pitch(-1)
                    }
                },
                Keycode::D => match self.active_sequencer {
                    SequencerSlot::Left => self.left_sequencer.change_pitch(12),
                    SequencerSlot::Right => {
                        self.right_sequencer.change_pitch(12)
                    }
                },
                Keycode::A => match self.active_sequencer {
                    SequencerSlot::Left => {
                        self.left_sequencer.change_pitch(-12)
                    }
                    SequencerSlot::Right => {
                        self.right_sequencer.change_pitch(-12)
                    }
                },
                Keycode::Tab => match self.active_sequencer {
                    SequencerSlot::Left => {
                        self.active_sequencer = SequencerSlot::Right
                    }
                    SequencerSlot::Right => {
                        self.active_sequencer = SequencerSlot::Left
                    }
                },
                _ => {}
            };
        }

        if self.left_sequencer != cached_state_left_sequencer {
            self.update_gui(GuiMessage::LeftSequencer(
                EuclideanGuiMessage::UpdateState(self.left_sequencer),
            ))
            .await;
        }

        if self.right_sequencer != cached_state_right_sequencer {
            self.update_gui(GuiMessage::LeftSequencer(
                EuclideanGuiMessage::UpdateState(self.right_sequencer),
            ))
            .await;
        }

        if self.mixer != cached_state_mixer {
            self.update_gui(GuiMessage::MixerRatioChanged(self.mixer.ratio))
                .await;
        }
    }

    pub async fn update_gui(&self, message: GuiMessage) {
        if let Some(mut tx) = self.tx_gui.lock().unwrap().clone() {
            if let Err(e) = tx.try_send(message) {
                error!("Error sending Message to GUI: {:?}", e);
            }
        }
    }
}
