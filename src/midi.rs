use midir::{MidiOutput, MidiOutputConnection};
use std::error::Error;

pub struct MidiHandler {
    conn_out: MidiOutputConnection,
}

impl MidiHandler {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let midi_out = MidiOutput::new("My MIDI Output")?;
        let out_ports = midi_out.ports();
        let out_port = out_ports.get(0).ok_or("No MIDI output ports available.")?;
        println!("Connecting to {}", midi_out.port_name(out_port)?);
        let conn_out = midi_out.connect(out_port, "midir-test")?;
        Ok(Self { conn_out })
    }

    pub fn send_note_on(&mut self, note: u8, velocity: u8) -> Result<(), Box<dyn Error>> {
        self.conn_out.send(&[0x90, note, velocity])?;
        Ok(())
    }

    pub fn send_note_off(&mut self, note: u8) -> Result<(), Box<dyn Error>> {
        self.conn_out.send(&[0x80, note, 0])?;
        Ok(())
    }
}