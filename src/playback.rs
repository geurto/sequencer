use anyhow::Result;
use log::{debug, info};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Duration};

use crate::midi::MidiHandler;
use crate::note::Sequence;
use crate::state::*;

pub struct PlaybackHandler {
    midi_handler: MidiHandler,
    rx_sequence: mpsc::Receiver<Sequence>,
    shared_state: Arc<RwLock<SharedState>>,
}

impl PlaybackHandler {
    pub fn new(
        midi_handler: MidiHandler,
        rx_sequence: mpsc::Receiver<Sequence>,
        shared_state: Arc<RwLock<SharedState>>,
    ) -> Self {
        Self {
            midi_handler,
            rx_sequence,
            shared_state,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("Starting playback loop");
        let mut current_note_index = 0;
        let mut sequence = Sequence::default();

        loop {
            if let Some(seq) = self.rx_sequence.recv().await {
                sequence = seq;
            }

            let r_state = self.shared_state.read().await;

            if r_state.playing {
                if sequence.notes.is_empty() {
                    drop(r_state);
                    sleep(Duration::from_millis(10)).await;
                    continue;
                }

                let note = &sequence.notes[current_note_index];
                debug!("Playing note: {:?}", note);

                let note_duration = note.duration as u64;
                self.midi_handler
                    .play_note(
                        note.pitch,
                        note_duration,
                        note.velocity,
                        r_state.midi_channel,
                    )
                    .await;

                // Move to the next note
                current_note_index = (current_note_index + 1) % sequence.notes.len();
            }

            // Always drop the lock before sleeping
            drop(r_state);
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }
}
