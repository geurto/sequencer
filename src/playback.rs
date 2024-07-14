use anyhow::Error;
use log::{debug, info};
use tokio::sync::{mpsc, Mutex};
use std::sync::Arc;
use tokio::time::{self, Duration, Instant};

use crate::state::*;
use crate::midi::MidiHandler;
use crate::input::Input;
use crate::sequencers::euclidean::config::EuclideanSequencerConfig;
use crate::sequencers::euclidean::input::EuclideanSequencerInput;
use crate::sequencers::markov::config::MarkovSequencerConfig;
use crate::sequencers::mixer::sequence_mixer::Mixer;

pub async fn play(midi_handler: Arc<Mutex<MidiHandler>>, shared_state: Arc<Mutex<SharedState>>) -> Result<(), Error> {
    info!("Starting playback loop");
    let mut current_note_index = 0;
    let mut next_note_time = Instant::now();

    loop {
        let state = shared_state.lock().await;
        if state.playing {
            let sequence = state.mixer_config.mixed_sequence.clone();
            if sequence.notes.is_empty() {
                drop(state);
                tokio::time::sleep(Duration::from_millis(10)).await;
                continue;
            }

            if Instant::now() >= next_note_time {
                let note = &sequence.notes[current_note_index];
                debug!("Playing note: {:?}", note);

                {
                    let mut midi = midi_handler.lock().await;
                    midi.send_note_on(note.pitch, note.velocity).expect("Failed to send NOTE ON");
                }

                // Schedule the note off
                let note_duration = Duration::from_millis(note.duration as u64);
                let note_off_time = Instant::now() + note_duration;
                let pitch = note.pitch;
                let midi_handler_clone = midi_handler.clone();
                tokio::spawn(async move {
                    tokio::time::sleep_until(note_off_time).await;
                    let mut midi = midi_handler_clone.lock().await;
                    midi.send_note_off(pitch).expect("Failed to send NOTE OFF");
                });

                // Move to the next note
                current_note_index = (current_note_index + 1) % sequence.notes.len();
                next_note_time = Instant::now() + note_duration;
            }
        } else {
            current_note_index = 0;
            next_note_time = Instant::now();
        }

        // Always drop the lock before sleeping
        drop(state);
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}