// src/main.rs
mod playback;
mod markov_notes;
mod structs;

use midir::MidiOutput;
use tokio::sync::mpsc;
use std::{error::Error, time::Duration};

use playback::start_playback_loop;
use markov_notes::{generate_sequence, initiate_chain};
use crate::structs::{Input, Sequence};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let midi_out = MidiOutput::new("My MIDI Output")?;
    let out_ports = midi_out.ports();
    let out_port = out_ports.get(0).ok_or("No MIDI output ports available.")?;
    println!("Connecting to {}", midi_out.port_name(out_port)?);
    let conn_out = midi_out.connect(out_port, "midir-test")?;

    let (tx, rx) = mpsc::channel(32);

    start_playback_loop(conn_out, tx.clone(), rx).await?;

    let mut sequence: Sequence = Sequence {
        pitch: vec![60, 62, 64, 65, 67, 69, 71, 72],
        velocity: vec![100; 8],
        duration: vec![500; 8],
    };

    let chain = initiate_chain();

    // This loop keeps the main function alive
    loop {
        let new_sequence = generate_sequence(&chain, 8, "major", "C");
        tx.send(Input::Sequence(new_sequence)).await.unwrap();
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
