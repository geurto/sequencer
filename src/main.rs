// src/main.rs
mod playback;

use midir::MidiOutput;
use tokio::sync::mpsc;
use std::{error::Error, time::Duration};
use playback::{start_playback_loop, Input};
use crate::playback::Sequence;

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

    // Example of sending inputs
    tx.send(Input::Bpm(140.0)).await.unwrap();
    tokio::time::sleep(Duration::from_secs(5)).await;

    sequence.pitch = vec![72, 71, 69, 67, 65, 64, 62, 60];
    tx.send(Input::Sequence(sequence.clone())).await.unwrap();

    tokio::time::sleep(Duration::from_secs(5)).await;
    sequence.velocity = vec![100, 70, 60, 50, 100, 70, 60, 50];
    tx.send(Input::Sequence(sequence.clone())).await.unwrap();

    tokio::time::sleep(Duration::from_secs(5)).await;
    sequence.duration = vec![500, 250, 250, 125, 500, 500, 250, 125];
    tx.send(Input::Sequence(sequence.clone())).await.unwrap();

    // This loop keeps the main function alive
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}
