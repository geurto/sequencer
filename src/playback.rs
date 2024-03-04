// src/playback.rs
use tokio::sync::{mpsc, Mutex};
use std::sync::Arc;
use tokio::time::{self, Duration};
use midir::{MidiOutputConnection};
use std::error::Error;

#[derive(Clone, Debug)]
pub struct Sequence {
    pub pitch: Vec<u8>,
    pub velocity: Vec<u8>,
    pub duration: Vec<u16>,
}

pub struct PlaybackState {
    pub bpm: f32,
    pub sequence: Sequence,
}

pub enum Input {
    Bpm(f32),
    Sequence(Sequence),
}

pub async fn start_playback_loop(
    mut conn_out: MidiOutputConnection,
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
                conn_out.send(&[0x90, note, velocity]).expect("Failed to send NOTE ON");  // Note on
                time::sleep(duration).await;
                conn_out.send(&[0x80, note, 0]).expect("Failed to send NOTE OFF"); // Note off
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
