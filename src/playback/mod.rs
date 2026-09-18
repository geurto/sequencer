pub mod engine;
pub mod midi;
pub mod state;

use device_query::Keycode;
use log::{error, info, warn};
use seq_core::PolyphonicSequence;
use std::{
    sync::{Arc, Mutex as SyncMutex, mpsc::Sender as SyncSender},
    time::Duration,
};
use tokio::sync::{RwLock, mpsc};

use crate::playback::midi::MidirSink;
use crate::{
    MidiCommand,
    gui::state::{Event, GuiMessage},
    midi_utils,
};
use state::{PlaybackCommand, PlaybackStatus, SharedState};

/// Idle back-off for the handler loop.
///
/// Without this the loop contains no await point when every channel is empty,
/// so the task never yields and permanently occupies a runtime worker. The
/// real fix is to select over the receivers rather than poll them.
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug)]
pub struct PlaybackHandler {
    state: Arc<RwLock<SharedState>>,

    rx_midi: mpsc::Receiver<MidiCommand>,
    rx_sequence: mpsc::Receiver<Box<PolyphonicSequence>>,
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
        rx_sequence: mpsc::Receiver<Box<PolyphonicSequence>>,
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

    pub async fn run(&mut self) {
        // The engine boots with its own hardcoded defaults; make the shared
        // state authoritative before anything plays.
        {
            let state = self.state.read().await;
            self.send_engine(PlaybackCommand::SetBPM(state.bpm));
            self.send_engine(PlaybackCommand::SetMidiChannel(
                state.midi_channel,
            ));
        }

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
                        self.handle_input_change(input).await;
                    }
                }
            }

            while let Ok(sequence) = self.rx_sequence.try_recv() {
                self.send_engine(PlaybackCommand::LoadSequence(sequence));
            }

            // get changes to MIDI
            while let Ok(midi_command) = self.rx_midi.try_recv() {
                self.handle_midi_command(midi_command);
            }

            tokio::time::sleep(IDLE_POLL_INTERVAL).await;
        }
    }

    fn handle_midi_command(&self, midi_command: MidiCommand) {
        match midi_command {
            MidiCommand::GetPorts { responder } => {
                match midi_utils::list_ports() {
                    Ok(port_names) => {
                        if responder.send(port_names).is_err() {
                            warn!("Unable to send MIDI output ports.");
                        }
                    }
                    Err(e) => error!("Unable to list MIDI output ports: {e}"),
                }
            }
            MidiCommand::SetPort { out_port } => {
                info!("Received SetPort from GUI");
                match midi_utils::create_connection(&out_port) {
                    Ok(conn_out) => {
                        self.send_engine(PlaybackCommand::SetOutputConnection(
                            Box::new(MidirSink(conn_out)),
                        ));

                        if let Some(mut tx) =
                            self.tx_gui.lock().unwrap().clone()
                            && let Err(e) =
                                tx.try_send(GuiMessage::MidiPortSet(out_port))
                        {
                            error!(
                                "Error sending Message::MidiPortSet to GUI: {e:?}"
                            );
                        }
                    }
                    Err(e) => {
                        error!(
                            "Unable to connect to MIDI port {out_port}: {e}"
                        );
                    }
                }
            }
        }
    }

    fn send_engine(&self, command: PlaybackCommand) {
        if let Err(e) = self.tx_engine.send(command) {
            error!("Error sending command to PlaybackEngine: {e}");
        }
    }

    pub async fn handle_input_change(&mut self, diff: Vec<Keycode>) {
        let (bpm, midi_channel) = {
            let mut w_state = self.state.write().await;

            for key in diff {
                match key {
                    Keycode::Space => {
                        w_state.is_playing = !w_state.is_playing;
                        if w_state.is_playing {
                            info!("Resumed playback!");
                        } else {
                            info!("Paused playback!");
                        }
                    }
                    Keycode::C => {
                        w_state.change_midi_channel();
                        info!(
                            "Changing MIDI channel to {}",
                            w_state.midi_channel + 1
                        );
                    }
                    Keycode::Equal => w_state.increase_bpm(),
                    Keycode::Minus => w_state.decrease_bpm(),
                    Keycode::R => {
                        w_state.mixer.increase_ratio();
                    }
                    Keycode::F => {
                        w_state.mixer.decrease_ratio();
                    }
                    // These all act on whichever sequencer is active;
                    // `SharedState` owns that dispatch.
                    Keycode::Up => w_state.increase_steps(),
                    Keycode::Down => w_state.decrease_steps(),
                    Keycode::Right => w_state.increase_pulses(),
                    Keycode::Left => w_state.decrease_pulses(),
                    Keycode::RightBracket => w_state.increase_phase(),
                    Keycode::LeftBracket => w_state.decrease_phase(),
                    Keycode::W => w_state.change_pitch(1),
                    Keycode::S => w_state.change_pitch(-1),
                    Keycode::D => w_state.change_pitch(12),
                    Keycode::A => w_state.change_pitch(-12),
                    Keycode::Tab => w_state.switch_active_sequencer(),
                    _ => {}
                }
            }

            (w_state.bpm, w_state.midi_channel)
        };

        // The engine keeps its own copy of both, and previously handled the
        // channel key itself — two counters that only stayed in step by luck.
        self.send_engine(PlaybackCommand::SetBPM(bpm));
        self.send_engine(PlaybackCommand::SetMidiChannel(midi_channel));

        self.update_gui().await;
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    pub async fn update_gui(&self) {
        let state = self.state.read().await;
        if let Some(mut tx) = self.tx_gui.lock().unwrap().clone()
            && let Err(e) = tx.try_send(GuiMessage::ReceivedEvent(
                Event::StateChanged(state.clone()),
            ))
        {
            error!("Error sending Message to GUI: {e:?}");
        }
    }
}
