use crate::state::SharedState;

use anyhow::{anyhow, Context, Error};
use log::{info, warn};
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use std::sync::Arc;
use tokio::runtime::Handle;
use tokio::sync::Mutex;

pub struct MidiHandler {
    conn_out: MidiOutputConnection,
    conn_in: Option<MidiInputConnection<()>>,
}

impl MidiHandler {
    pub fn new() -> Result<Self, Error> {
        let midi_out = MidiOutput::new("My MIDI Output")?;
        let out_ports = midi_out.ports();

        info!("Available output ports:");
        for (i, p) in out_ports.iter().enumerate() {
            info!("{}: {}", i, midi_out.port_name(p)?);
        }

        let out_ports_ttymidi = out_ports.iter().filter(|p| midi_out.port_name(p).unwrap().contains("ttymidi")).collect::<Vec<_>>();
        let out_port;
        if out_ports_ttymidi.is_empty() {
            warn!("No ttymidi output ports available.");
            out_port = out_ports.get(0)
                .ok_or_else(|| anyhow!("No MIDI output ports available"))?;
        } else {
            out_port = out_ports_ttymidi.get(0)
                .ok_or_else(|| anyhow!("No ttymidi output ports available"))?;
        }

        info!("Connecting to {}", midi_out.port_name(out_port)?);
        let conn_out = midi_out.connect(out_port, "gen-seq")
            .map_err(|e| anyhow!("Failed to connect to MIDI output: {}", e))?;
        Ok(Self { conn_out, conn_in: None })
    }

    pub fn send_note_on(&mut self, note: u8, velocity: u8, channel: u8) -> Result<(), Error> {
        self.conn_out.send(&[0x90, note, velocity, channel])?;
        Ok(())
    }

    pub fn send_note_off(&mut self, note: u8, channel: u8) -> Result<(), Error> {
        self.conn_out.send(&[0x80, note, 0, channel])?;
        Ok(())
    }

    pub fn send_all_notes_off(&mut self, channel: u8) -> Result<(), Error> {
        for note in 0..=127 {
            self.send_note_off(note, channel)?;
        }
        Ok(())
    }

    pub async fn setup_midi_input(&mut self, shared_state: Arc<Mutex<SharedState>>) -> Result<(), Error> {
        info!("Setting up MIDI input...");
        let mut midi_in = MidiInput::new("MIDI Input")
            .context("Failed to create MIDI input")?;
        midi_in.ignore(midir::Ignore::None);

        let in_ports = midi_in.ports();
        let in_port;
        info!("Available input ports:");
        for (i, p) in in_ports.iter().enumerate() {
            info!("{}: {}", i, midi_in.port_name(p)
            .context("Failed to get port name")?);
        }
        let in_ports_ttymidi = in_ports.iter()
            .filter(|p| midi_in.port_name(p)
                .unwrap_or_default()
                .contains("ttymidi"))
            .collect::<Vec<_>>();
        if in_ports_ttymidi.is_empty() {
            warn!("No ttymidi input ports available.");
            in_port = in_ports.get(0)
                .ok_or_else(|| anyhow!("No MIDI input ports available"))?;
        } else {
            in_port = in_ports_ttymidi.get(0)
                .ok_or_else(|| anyhow!("No ttymidi input ports available"))?;
        }
        info!("Connecting to {}", midi_in.port_name(in_port)
        .context("Failed to get port name")?);

        let handle = Handle::current();

        let conn_in = midi_in.connect(
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
        ).map_err(|e| anyhow!("Failed to connect to MIDI input: {}", e))?;

        self.conn_in = Some(conn_in);
        Ok(())
    }
}

async fn handle_midi_message(message: Vec<u8>, shared_state: &Arc<Mutex<SharedState>>) {
    if message[0] == 0xF8 {
        // MIDI Clock message
        let mut state = shared_state.lock().await;
        state.clock_ticks += 1;
        if state.clock_ticks >= 24 {
            state.clock_ticks = 0;
            state.quarter_notes += 1;
        }
    }
}