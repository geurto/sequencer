pub mod engine;
pub mod midi;
pub mod state;

use anyhow::Result;
use device_query::Keycode;
use log::{error, info, warn};
use std::{
    sync::{mpsc::Sender as SyncSender, Arc, Mutex as SyncMutex},
    time::Duration,
};
use tokio::sync::{mpsc, RwLock};

use crate::{
    gui::state::{Event, GuiMessage},
    midi_utils, MidiCommand,
};
use state::{
    PlaybackCommand, PlaybackStatus, PolyphonicSequence, SequencerSlot,
    SharedState,
};

pub struct PlaybackHandler {
    state: Arc<RwLock<SharedState>>,

    rx_midi: mpsc::Receiver<MidiCommand>,
    rx_sequence: mpsc::Receiver<PolyphonicSequence>,
    tx_engine: SyncSender<PlaybackCommand>,
    rx_engine_status: mpsc::UnboundedReceiver<PlaybackStatus>,
    tx_gui: Arc<
        SyncMutex<Option<iced::futures::channel::mpsc::Sender<GuiMessage>>>,
    >,
}

impl PlaybackHandler {
    pub fn new(
        state: Arc<RwLock<SharedState>>,
        rx_midi: mpsc::Receiver<MidiCommand>,
        rx_sequence: mpsc::Receiver<PolyphonicSequence>,
        tx_engine: SyncSender<PlaybackCommand>,
        rx_engine_status: mpsc::UnboundedReceiver<PlaybackStatus>,
        tx_gui: Arc<
            SyncMutex<Option<iced::futures::channel::mpsc::Sender<GuiMessage>>>,
        >,
    ) -> Self {
        Self {
            state,
            rx_midi,
            rx_sequence,
            tx_engine,
            rx_engine_status,
            tx_gui,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            // get synchronous engine status
            while let Ok(status) = self.rx_engine_status.try_recv() {
                match status {
                    PlaybackStatus::NotePlayed(i) => {
                        let mut w_state = self.state.write().await;
                        w_state.current_note_index = i;
                        drop(w_state);
                        self.update_gui().await;
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
        let mut w_state = self.state.write().await;

        for key in diff {
            match key {
                Keycode::Space => {
                    match w_state.is_playing {
                        true => info!("Paused playback!"),
                        false => info!("Resumed playback!"),
                    };
                    w_state.is_playing = !w_state.is_playing
                }
                Keycode::C => {
                    w_state.midi_channel = (w_state.midi_channel + 1) % 16;
                    info!(
                        "Changing MIDI channel to {}",
                        w_state.midi_channel + 1
                    );
                }
                Keycode::R => {
                    w_state.mixer.increase_ratio();
                }
                Keycode::F => {
                    w_state.mixer.decrease_ratio();
                }
                Keycode::Up => match w_state.active_sequencer {
                    SequencerSlot::Left => {
                        w_state.left_sequencer.increase_steps();
                    }
                    SequencerSlot::Right => {
                        w_state.right_sequencer.increase_steps()
                    }
                },
                Keycode::Down => match w_state.active_sequencer {
                    SequencerSlot::Left => {
                        w_state.left_sequencer.decrease_steps()
                    }
                    SequencerSlot::Right => {
                        w_state.right_sequencer.decrease_steps()
                    }
                },
                Keycode::Right => match w_state.active_sequencer {
                    SequencerSlot::Left => {
                        w_state.left_sequencer.increase_pulses()
                    }
                    SequencerSlot::Right => {
                        w_state.right_sequencer.increase_pulses()
                    }
                },
                Keycode::Left => match w_state.active_sequencer {
                    SequencerSlot::Left => {
                        w_state.left_sequencer.decrease_pulses()
                    }
                    SequencerSlot::Right => {
                        w_state.right_sequencer.decrease_pulses()
                    }
                },
                Keycode::RightBracket => match w_state.active_sequencer {
                    SequencerSlot::Left => {
                        w_state.left_sequencer.increase_phase()
                    }
                    SequencerSlot::Right => {
                        w_state.right_sequencer.increase_phase()
                    }
                },
                Keycode::LeftBracket => match w_state.active_sequencer {
                    SequencerSlot::Left => {
                        w_state.left_sequencer.decrease_phase()
                    }
                    SequencerSlot::Right => {
                        w_state.right_sequencer.decrease_phase()
                    }
                },
                Keycode::W => match w_state.active_sequencer {
                    SequencerSlot::Left => {
                        w_state.left_sequencer.change_pitch(1)
                    }
                    SequencerSlot::Right => {
                        w_state.right_sequencer.change_pitch(1)
                    }
                },
                Keycode::S => match w_state.active_sequencer {
                    SequencerSlot::Left => {
                        w_state.left_sequencer.change_pitch(-1)
                    }
                    SequencerSlot::Right => {
                        w_state.right_sequencer.change_pitch(-1)
                    }
                },
                Keycode::D => match w_state.active_sequencer {
                    SequencerSlot::Left => {
                        w_state.left_sequencer.change_pitch(12)
                    }
                    SequencerSlot::Right => {
                        w_state.right_sequencer.change_pitch(12)
                    }
                },
                Keycode::A => match w_state.active_sequencer {
                    SequencerSlot::Left => {
                        w_state.left_sequencer.change_pitch(-12)
                    }
                    SequencerSlot::Right => {
                        w_state.right_sequencer.change_pitch(-12)
                    }
                },
                Keycode::Tab => match w_state.active_sequencer {
                    SequencerSlot::Left => {
                        w_state.active_sequencer = SequencerSlot::Right
                    }
                    SequencerSlot::Right => {
                        w_state.active_sequencer = SequencerSlot::Left
                    }
                },
                _ => {}
            };
        }
        drop(w_state);
        self.update_gui().await;
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    pub async fn update_gui(&self) {
        let state = self.state.read().await;
        if let Some(mut tx) = self.tx_gui.lock().unwrap().clone() {
            if let Err(e) = tx.try_send(GuiMessage::ReceivedEvent(
                Event::StateChanged(state.clone()),
            )) {
                error!("Error sending Message to GUI: {:?}", e);
            }
        }
    }
}
