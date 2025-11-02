pub mod engine;
pub mod midi;
pub mod state;

use anyhow::Result;
use log::{error, info, warn};
use state::PlaybackCommand;
use std::sync::{mpsc::Sender as SyncSender, Arc, Mutex as SyncMutex};
use tokio::sync::mpsc;

use crate::{gui::Message, midi_utils, note::MixedSequence, MidiCommand};

pub struct PlaybackHandler {
    rx_midi: mpsc::Receiver<MidiCommand>,
    rx_sequence: mpsc::Receiver<MixedSequence>,
    tx_engine: SyncSender<PlaybackCommand>,
    tx_gui: Arc<SyncMutex<Option<iced::futures::channel::mpsc::Sender<Message>>>>,
}

impl PlaybackHandler {
    pub fn new(
        rx_midi: mpsc::Receiver<MidiCommand>,
        rx_sequence: mpsc::Receiver<MixedSequence>,
        tx_engine: SyncSender<PlaybackCommand>,
        tx_gui: Arc<SyncMutex<Option<iced::futures::channel::mpsc::Sender<Message>>>>,
    ) -> Self {
        Self {
            rx_midi,
            rx_sequence,
            tx_engine,
            tx_gui,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        while let Some(midi_command) = self.rx_midi.recv().await {
            match midi_command {
                MidiCommand::GetPorts { responder } => {
                    let port_names = midi_utils::list_ports()?;
                    if responder.send(port_names).is_err() {
                        warn!("Unable to send MIDI output ports.");
                    }
                }
                MidiCommand::SetPort { out_port } => {
                    info!("Received SetPort from GUI");
                    let conn_out = midi_utils::create_connection(out_port.clone())?;

                    match self
                        .tx_engine
                        .send(PlaybackCommand::SetOutputConnection(conn_out))
                    {
                        Ok(_) => {}
                        Err(e) => {
                            error!("Error sending MidiOutputConnection to PlaybackEngine: {e}")
                        }
                    }

                    if let Some(mut tx) = self.tx_gui.lock().unwrap().clone() {
                        if let Err(e) = tx.try_send(Message::MidiPortSet(out_port)) {
                            error!("Error sending Message::MidiPortSet to GUI: {:?}", e);
                        }
                    }
                }
            };
        }

        Ok(())
    }
}
