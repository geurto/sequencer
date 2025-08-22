pub mod state;

use crate::note::Note;
use crate::state::SharedState;

use anyhow::{anyhow, Context, Result};
use log::{error, info, warn};
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use state::MidiCommand;
use std::sync::Arc;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task;
use tokio::time::{sleep, Duration};

pub struct MidiHandler {
    rx: mpsc::Receiver<MidiCommand>,
    conn_out: Arc<Mutex<Option<MidiOutputConnection>>>,
    conn_in: Option<MidiInputConnection<()>>,
}

const NOTE_ON_MSG: u8 = 0x90;
const NOTE_OFF_MSG: u8 = 0x80;

impl MidiHandler {
    pub fn new(rx: mpsc::Receiver<MidiCommand>) -> Result<Self> {
        Ok(Self {
            rx,
            conn_out: Arc::new(Mutex::new(None)),
            conn_in: None,
        })
    }

    pub async fn play_multiple_notes(&mut self, notes: (Option<Note>, Option<Note>), channel: u8) {
        match notes {
            (None, None) => {}
            (Some(note_a), None) => {
                play_note(
                    Arc::clone(&self.conn_out),
                    note_a.pitch,
                    note_a.duration as u64,
                    note_a.velocity,
                    channel,
                )
                .await
            }
            (None, Some(note_b)) => {
                play_note(
                    Arc::clone(&self.conn_out),
                    note_b.pitch,
                    note_b.duration as u64,
                    note_b.velocity,
                    channel,
                )
                .await
            }
            (Some(note_a), Some(note_b)) => {
                let conn_a = Arc::clone(&self.conn_out);
                let conn_b = Arc::clone(&self.conn_out);

                let task_a = task::spawn(async move {
                    play_note(
                        conn_a,
                        note_a.pitch,
                        note_a.duration as u64,
                        note_a.velocity,
                        channel,
                    )
                    .await;
                });

                let task_b = task::spawn(async move {
                    play_note(
                        conn_b,
                        note_b.pitch,
                        note_b.duration as u64,
                        note_b.velocity,
                        channel,
                    )
                    .await;
                });

                let _ = tokio::join!(task_a, task_b);
            }
        }
    }

    pub async fn setup_midi_input(&mut self, shared_state: Arc<RwLock<SharedState>>) -> Result<()> {
        info!("Setting up MIDI input...");
        let mut midi_in = MidiInput::new("MIDI Input").context("Failed to create MIDI input")?;
        midi_in.ignore(midir::Ignore::None);

        let in_ports = midi_in.ports();
        info!("Available input ports:");
        for (i, p) in in_ports.iter().enumerate() {
            info!(
                "{}: {}",
                i,
                midi_in.port_name(p).context("Failed to get port name")?
            );
        }
        let in_ports_ttymidi = in_ports
            .iter()
            .filter(|p| midi_in.port_name(p).unwrap_or_default().contains("ttymidi"))
            .collect::<Vec<_>>();
        let in_port = if in_ports_ttymidi.is_empty() {
            warn!("No ttymidi input ports available.");
            in_ports
                .first()
                .ok_or_else(|| anyhow!("No MIDI input ports available"))?
        } else {
            in_ports_ttymidi
                .first()
                .ok_or_else(|| anyhow!("No ttymidi input ports available"))?
        };

        info!(
            "Connecting to {}",
            midi_in
                .port_name(in_port)
                .context("Failed to get port name")?
        );

        let handle = Handle::current();

        let conn_in = midi_in
            .connect(
                in_port,
                "midir-test",
                move |_stamp, message, _| {
                    let shared_state = shared_state.clone();
                    let message = message.to_vec();
                    let handle = handle.clone();
                    handle.spawn(async move {
                        handle_midi_message(message, &shared_state).await;
                    });
                },
                (),
            )
            .map_err(|e| anyhow!("Failed to connect to MIDI input: {}", e))?;

        self.conn_in = Some(conn_in);
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        while let Some(midi_command) = self.rx.recv().await {
            match midi_command {
                MidiCommand::PlayNotes { notes, channel } => {
                    self.play_multiple_notes(notes, channel).await
                }
                MidiCommand::GetPorts { responder } => {
                    let midi_out = MidiOutput::new("Generative Sequencer MIDI Out")?;
                    let port_names = midi_out.ports().iter().map(|p| p.id()).collect::<Vec<_>>();
                    if responder.send(port_names).is_err() {
                        warn!("Unable to send MIDI output ports.");
                    }
                }
                MidiCommand::SetPort { out_port } => {
                    info!("Received SetPort from GUI");
                    let midi_out = MidiOutput::new("Generative Sequencer MIDI Out")?;
                    match midi_out.find_port_by_id(out_port.clone()) {
                        Some(midi_port) => {
                            let conn_out = midi_out
                                .connect(&midi_port, "gen-seq")
                                .map_err(|e| anyhow!("Failed to connect to MIDI output: {}", e))?;

                            *self.conn_out.lock().await = Some(conn_out);
                            info!("Successfully changed MIDI output port to {out_port}");
                        }
                        None => {
                            error!("Unable to find MIDI output port with name {out_port}")
                        }
                    }
                }
            };
        }

        Ok(())
    }
}

async fn handle_midi_message(message: Vec<u8>, shared_state: &Arc<RwLock<SharedState>>) {
    if message[0] == 0xF8 {
        // MIDI Clock message -- @todo this may need to stay an Arc<Mutex<>>
        let mut state = shared_state.write().await;
        state.clock_ticks += 1;
        if state.clock_ticks >= 24 {
            state.clock_ticks = 0;
            state.quarter_notes += 1;
        }
    }
}

pub async fn play_note(
    conn: Arc<Mutex<Option<MidiOutputConnection>>>,
    note: u8,
    duration: u64,
    velocity: u8,
    channel: u8,
) {
    let mut guard = conn.lock().await;

    if let Some(output) = guard.as_mut() {
        output
            .send(&[NOTE_ON_MSG, note, velocity, channel])
            .expect("Failed to send NOTE_ON_MSG");
    }

    sleep(Duration::from_millis(duration)).await;

    if let Some(output) = guard.as_mut() {
        output
            .send(&[NOTE_OFF_MSG, note, velocity, channel])
            .expect("Failed to send NOTE_OFF_MSG");
    }

    drop(guard);
}
