// src/playback.rs
use tokio::sync::{mpsc, Mutex};
use std::sync::Arc;
use tokio::time::{self, Duration};
use std::error::Error;

use crate::common::*;
use crate::midi::MidiHandler;

pub async fn start_playback_loop(
    mut midi_handler: MidiHandler,
    _tx: mpsc::Sender<Input>,
    mut rx: mpsc::Receiver<Input>,
    shared_state: Arc<Mutex<SharedState>>
) -> Result<(), Box<dyn Error>> {
    tokio::spawn(async move {
        loop {
            if let Some(input) = rx.recv().await {
                let mut state = shared_state.lock().await;
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

            let state = shared_state.lock().await;
            println!("Playing sequence {:?} at {} BPM", state.sequence, state.bpm);
            for i in 0..state.sequence.notes.len() {
                let pitch = state.sequence.notes[i].pitch;
                let duration = Duration::from_millis(state.sequence.notes[i].duration as u64);
                let velocity = state.sequence.notes[i].velocity;
                println!("Playing note: {}", pitch);
                midi_handler.send_note_on(pitch, velocity).expect("Failed to send NOTE ON");  // Note on
                time::sleep(duration).await;
                midi_handler.send_note_off(pitch).expect("Failed to send NOTE OFF"); // Note off
            }
        }
    });

    Ok(())
}
