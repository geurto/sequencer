pub mod engine;
pub mod state;

use anyhow::Result;
use log::{error, info, warn};
use std::sync::{Arc, Mutex as SyncMutex};
use tokio::sync::mpsc;

use crate::{gui::Message, midi::midi_utils, midi::state::MidiCommand};

pub struct PlaybackHandler {
    rx_midi: mpsc::Receiver<MidiCommand>,
    tx_gui: Arc<SyncMutex<Option<iced::futures::channel::mpsc::Sender<Message>>>>,
}

impl PlaybackHandler {
    pub fn new(
        rx_midi: mpsc::Receiver<MidiCommand>,
        tx_gui: Arc<SyncMutex<Option<iced::futures::channel::mpsc::Sender<Message>>>>,
    ) -> Self {
        Self { rx_midi, tx_gui }
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
                    let conn_out = midi_utils::create_connection(out_port)?;

                    self.tx_conn.send(conn_out);

                    if let Some(mut tx) = self.tx_gui.lock().unwrap().clone() {
                        if let Err(e) = tx.try_send(Message::MidiPortSet()) {
                            error!("Error sending Message::MidiPortSet to GUI: {:?}", e);
                        }
                    }
                }
            };
        }

        Ok(())
    }
}
