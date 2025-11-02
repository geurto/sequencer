pub mod engine;
pub mod midi;
pub mod state;

use anyhow::Result;
use log::{error, info, warn};
use state::{PlaybackCommand, PlaybackStatus, PolyphonicSequence};
use std::sync::{
    mpsc::{Receiver as SyncReceiver, Sender as SyncSender},
    Arc, Mutex as SyncMutex,
};
use tokio::sync::mpsc;

use crate::{gui::Message, midi_utils, MidiCommand};

pub struct PlaybackHandler {
    rx_midi: mpsc::Receiver<MidiCommand>,
    rx_sequence: mpsc::Receiver<PolyphonicSequence>,
    rx_engine_status: SyncReceiver<PlaybackStatus>,
    tx_engine: SyncSender<PlaybackCommand>,
    tx_gui:
        Arc<SyncMutex<Option<iced::futures::channel::mpsc::Sender<Message>>>>,
}

impl PlaybackHandler {
    pub fn new(
        rx_midi: mpsc::Receiver<MidiCommand>,
        rx_sequence: mpsc::Receiver<PolyphonicSequence>,
        rx_engine_status: SyncReceiver<PlaybackStatus>,
        tx_engine: SyncSender<PlaybackCommand>,
        tx_gui: Arc<
            SyncMutex<Option<iced::futures::channel::mpsc::Sender<Message>>>,
        >,
    ) -> Self {
        Self {
            rx_midi,
            rx_sequence,
            rx_engine_status,
            tx_engine,
            tx_gui,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            // get synchronous engine status
            while let Ok(status) = self.rx_engine_status.try_recv() {
                if let Some(tx) = self.tx_gui.lock().await {
                    tx.try_send(Message::LeftSequencer())
                }
            }

            // TODO is it better to send directly from Mixer to PlaybackEngine?
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
                                tx.try_send(Message::MidiPortSet(out_port))
                            {
                                error!("Error sending Message::MidiPortSet to GUI: {:?}", e);
                            }
                        }
                    }
                };
            }
        }
    }
}
