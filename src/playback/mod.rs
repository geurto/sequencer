pub mod engine;
pub mod midi;
pub mod state;

use anyhow::Result;
use device_query::Keycode;
use log::{error, info, warn};
use state::{PlaybackCommand, PlaybackStatus, PolyphonicSequence};
use std::sync::{mpsc::Sender as SyncSender, Arc, Mutex as SyncMutex};
use tokio::sync::{mpsc, RwLock};

use crate::{
    gui::{Event, Message},
    midi_utils, MidiCommand, SharedState,
};

pub struct PlaybackHandler {
    rx_midi: mpsc::Receiver<MidiCommand>,
    rx_sequence: mpsc::Receiver<PolyphonicSequence>,
    rx_engine_status: mpsc::UnboundedReceiver<PlaybackStatus>,
    tx_engine: SyncSender<PlaybackCommand>,
    tx_gui:
        Arc<SyncMutex<Option<iced::futures::channel::mpsc::Sender<Message>>>>,
    shared_state: Arc<RwLock<SharedState>>,
}

impl PlaybackHandler {
    pub fn new(
        rx_midi: mpsc::Receiver<MidiCommand>,
        rx_sequence: mpsc::Receiver<PolyphonicSequence>,
        rx_engine_status: mpsc::UnboundedReceiver<PlaybackStatus>,
        tx_engine: SyncSender<PlaybackCommand>,
        tx_gui: Arc<
            SyncMutex<Option<iced::futures::channel::mpsc::Sender<Message>>>,
        >,
        shared_state: Arc<RwLock<SharedState>>,
    ) -> Self {
        Self {
            rx_midi,
            rx_sequence,
            rx_engine_status,
            tx_engine,
            tx_gui,
            shared_state,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            // get synchronous engine status
            while let Ok(status) = self.rx_engine_status.try_recv() {
                match status {
                    PlaybackStatus::NotePlayed(i) => {
                        let mut w_state = self.shared_state.write().await;
                        w_state.current_note_index = i;
                        drop(w_state);
                        self.update_gui().await;
                    }
                    PlaybackStatus::InputChanged(input) => {
                        self.handle_input_change(input).await
                    }
                };
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

    pub async fn handle_input_change(&mut self, diff: Vec<Keycode>) {
        let mut w_state = self.shared_state.write().await;
        for key in diff {
            match key {
                Keycode::Space => {
                    w_state.playing = !w_state.playing;

                    match w_state.playing {
                        true => info!("Resumed playback!"),
                        false => info!("Paused playback!"),
                    }
                }
                Keycode::C => {
                    w_state.change_midi_channel();
                    info!(
                        "Changing MIDI channel to {}",
                        w_state.midi_channel + 1
                    )
                }
                Keycode::R => w_state.mixer_state.increase_ratio(),
                Keycode::F => w_state.mixer_state.decrease_ratio(),
                Keycode::Up => w_state.increase_steps(),
                Keycode::Down => w_state.decrease_steps(),
                Keycode::Right => w_state.increase_pulses(),
                Keycode::Left => w_state.decrease_pulses(),
                Keycode::W => w_state.change_pitch(1),
                Keycode::S => w_state.change_pitch(-1),
                Keycode::D => w_state.change_pitch(12),
                Keycode::A => w_state.change_pitch(-12),
                Keycode::Tab => w_state.switch_active_sequencer(),
                _ => {}
            };
        }

        drop(w_state);
        self.update_gui().await;
    }

    pub async fn update_gui(&self) {
        let r_state = self.shared_state.read().await;
        if let Some(mut tx) = self.tx_gui.lock().unwrap().clone() {
            if let Err(e) = tx.try_send(Message::ReceivedEvent(
                Event::StateChanged(r_state.clone()),
            )) {
                error!("Error sending Message::ReceivedEvent to GUI: {:?}", e);
            }
        }
    }
}
