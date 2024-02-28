use midir::{MidiOutput, MidiOutputConnection};
use std::{error::Error, thread, time::Duration};

fn main() -> Result<(), Box<dyn Error>> {
    let midi_out = MidiOutput::new("My MIDI Output")?;
    let out_ports = midi_out.ports();

    let out_port = out_ports.get(0).ok_or("No MIDI output ports available.")?;

    println!("Connecting to {}", midi_out.port_name(out_port)?);
    let mut conn_out: MidiOutputConnection = midi_out.connect(out_port, "midir-test")?;

    println!("Sending MIDI messages...");
    // Example: sending a middle C note on (channel 1, note 60, velocity 100)
    // Note on: 0x90, Note off: 0x80, Middle C: 60, Velocity: 100
    conn_out.send(&[0x90, 60, 100])?;
    thread::sleep(Duration::from_millis(500)); // Hold note for 500ms
    conn_out.send(&[0x80, 60, 0])?; // Note off

    println!("Messages sent.");

    Ok(())
}
