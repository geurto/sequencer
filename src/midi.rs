use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use std::error::Error;
use crate::common::SharedState;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct MidiHandler {
    conn_out: MidiOutputConnection,
    conn_in: Option<MidiInputConnection<()>>,
}

impl MidiHandler {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let midi_out = MidiOutput::new("My MIDI Output")?;
        let out_ports = midi_out.ports();
        let out_port = out_ports.get(0).ok_or("No MIDI output ports available.")?;
        println!("Connecting to {}", midi_out.port_name(out_port)?);
        let conn_out = midi_out.connect(out_port, "midir-test")?;
        Ok(Self { conn_out, conn_in: None })
    }

    pub fn send_note_on(&mut self, note: u8, velocity: u8) -> Result<(), Box<dyn Error>> {
        self.conn_out.send(&[0x90, note, velocity])?;
        Ok(())
    }

    pub fn send_note_off(&mut self, note: u8) -> Result<(), Box<dyn Error>> {
        self.conn_out.send(&[0x80, note, 0])?;
        Ok(())
    }

    pub fn setup_midi_input(&mut self, shared_state: Arc<Mutex<SharedState>>) -> Result<(), Box<dyn Error>> {
        let mut midi_in = MidiInput::new("MIDI Input")?;
        midi_in.ignore(midir::Ignore::None);

        let in_ports = midi_in.ports();
        let in_port = in_ports.get(0).ok_or("No MIDI input ports available.")?;
        println!("Connecting to {}", midi_in.port_name(in_port)?);

        let conn_in = midi_in.connect(in_port, "midir-read-input", move |_, message, _| {
            handle_midi_message(message, &shared_state);
        }, ())?;

        self.conn_in = Some(conn_in);
        Ok(())
    }
}

async fn handle_midi_message(message: &[u8], shared_state: &Arc<Mutex<SharedState>>) {
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