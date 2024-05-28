// src/playback.rs
use tokio::sync::{mpsc, Mutex};
use std::sync::Arc;
use tokio::time::{self, Duration};
use midir::{MidiOutputConnection};
use std::error::Error;

use crate::common::*;
use crate::midi::MidiHandler;

pub async fn start_playback_loop(
    mut midi_handler: MidiHandler,
    _tx: mpsc::Sender<Input>,
    rx: mpsc::Receiver<Input>,
) -> Result<(), Box<dyn Error>> {
    let state = Arc::new(Mutex::new(PlaybackState {
        bpm: 120.0,
        sequence: Sequence {
            pitch: vec![60, 62, 64, 65, 67, 69, 71, 72],
            velocity: vec![100; 8],
            duration: vec![500; 8],
        },
    }));

    let playback_state = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            let state = playback_state.lock().await;
            println!("Playing sequence {:?} at {} BPM", state.sequence, state.bpm);
            for i in 0..state.sequence.pitch.len() {
                let note = state.sequence.pitch[i];
                let duration = Duration::from_millis(state.sequence.duration[i] as u64);
                let velocity = state.sequence.velocity[i];
                println!("Playing note: {}", note);
                midi_handler.send_note_on(note, velocity).expect("Failed to send NOTE ON");  // Note on
                time::sleep(duration).await;
                midi_handler.send_note_off(note).expect("Failed to send NOTE OFF"); // Note off
            }
        }
    });

    let playback_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut rx = rx;
        while let Some(input) = rx.recv().await {
            let mut state = playback_state.lock().await;
            match input {
                Input::Bpm(bpm) => {
                    println!("Changing BPM to {}", bpm);
                    state.bpm = bpm;
                }
                Input::Sequence(sequence) => {
                    println!("Changing sequence");
                    state.sequence = sequence;
                }
            }
        }
    });

    Ok(())
}
