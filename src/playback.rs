use anyhow::Error;
use log::{debug, info};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use crate::midi::MidiHandler;
use crate::state::*;

pub async fn play(
    midi_handler: Arc<Mutex<MidiHandler>>,
    shared_state: Arc<Mutex<SharedState>>,
) -> Result<(), Error> {
    info!("Starting playback loop");
    let mut current_note_index = 0;

    loop {
        let state = shared_state.lock().await;
        if state.playing {
            let sequence = state.mixer_state.mixed_sequence.clone();
            if sequence.notes.is_empty() {
                drop(state);
                sleep(Duration::from_millis(10)).await;
                continue;
            }

            let note = &sequence.notes[current_note_index];
            debug!("Playing note: {:?}", note);

            let mut midi = midi_handler.lock().await;
            let note_duration = note.duration as u64;
            midi.play_note(note.pitch, note_duration, note.velocity, state.midi_channel)
                .await;

            // Move to the next note
            current_note_index = (current_note_index + 1) % sequence.notes.len();
        }

        // Always drop the lock before sleeping
        drop(state);
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

