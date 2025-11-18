pub mod engine;
pub mod midi;
pub mod state;

use anyhow::Result;
use device_query::Keycode;
use log::{error, info, warn};
use state::{PlaybackCommand, PlaybackStatus, PolyphonicSequence};
use std::sync::{mpsc::Sender as SyncSender, Arc, Mutex as SyncMutex};
use tokio::sync::mpsc;

use crate::{
    gui::{
        sequencers::euclidean::Message as EuclideanGuiMessage,
        Message as GuiMessage,
    },
    midi_utils, EuclideanSequencerState, MidiCommand, MixerState,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub enum SequencerSlot {
    #[default]
    Left,
    Right,
}

pub struct PlaybackHandler {
    is_playing: bool,
    midi_channel: u8,
    bpm: f32,
    current_note_index: usize,
    active_sequencer: SequencerSlot,

    left_sequencer: EuclideanSequencerState,
    right_sequencer: EuclideanSequencerState,
    mixer: MixerState,

    rx_midi: mpsc::Receiver<MidiCommand>,
    rx_sequence: mpsc::Receiver<PolyphonicSequence>,
    tx_engine: SyncSender<PlaybackCommand>,
    rx_engine_status: mpsc::UnboundedReceiver<PlaybackStatus>,
    tx_gui: Arc<
        SyncMutex<Option<iced::futures::channel::mpsc::Sender<GuiMessage>>>,
    >,

    tx_sequencer_a_state: mpsc::Sender<EuclideanSequencerState>,
    tx_sequencer_b_state: mpsc::Sender<EuclideanSequencerState>,
    tx_mixer_state: mpsc::Sender<MixerState>,
}

impl PlaybackHandler {
    pub fn new(
        rx_midi: mpsc::Receiver<MidiCommand>,
        rx_sequence: mpsc::Receiver<PolyphonicSequence>,
        tx_engine: SyncSender<PlaybackCommand>,
        rx_engine_status: mpsc::UnboundedReceiver<PlaybackStatus>,
        tx_gui: Arc<
            SyncMutex<Option<iced::futures::channel::mpsc::Sender<GuiMessage>>>,
        >,

        tx_sequencer_a_state: mpsc::Sender<EuclideanSequencerState>,
        tx_sequencer_b_state: mpsc::Sender<EuclideanSequencerState>,
        tx_mixer_state: mpsc::Sender<MixerState>,
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

            rx_midi,
            rx_sequence,
            tx_engine,
            rx_engine_status,
            tx_gui,

            tx_sequencer_a_state,
            tx_sequencer_b_state,
            tx_mixer_state,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
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

            while let Ok(sequence) = self.rx_sequence.try_recv() {
                if let Err(e) =
                    self.tx_engine.send(PlaybackCommand::LoadSequence(sequence))
                {
                    error!(
                    "Error sending PolyphonicSequence to PlaybackEngine: {e}"
                );
                }
            }

            // get changes to MIDI
            while let Ok(midi_command) = self.rx_midi.try_recv() {
                match midi_command {
                    MidiCommand::GetPorts { responder } => {
                        let port_names = midi_utils::list_ports()?;
                        if responder.send(port_names).is_err() {
                            warn!("Unable to send MIDI output ports.");
                        }
                    }
                    MidiCommand::SetPort { out_port } => {
                        info!("Received SetPort from GUI");
                        let conn_out =
                            midi_utils::create_connection(out_port.clone())?;

                        match self.tx_engine.send(
                            PlaybackCommand::SetOutputConnection(conn_out),
                        ) {
                            Ok(_) => {}
                            Err(e) => {
                                error!("Error sending MidiOutputConnection to PlaybackEngine: {e}")
                            }
                        }

                        if let Some(mut tx) =
                            self.tx_gui.lock().unwrap().clone()
                        {
                            if let Err(e) =
                                tx.try_send(GuiMessage::MidiPortSet(out_port))
                            {
                                error!("Error sending Message::MidiPortSet to GUI: {:?}", e);
                            }
                        }
                    }
                };
            }
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
