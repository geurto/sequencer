pub mod gui;
pub mod state;

use crate::{note::Sequence, MixerState, SharedState};
use log::{debug, error, info};
use num::integer;
use rand::random;
use std::{cmp::max, sync::Arc};
use tokio::sync::{mpsc, RwLock};

pub struct Mixer {
    shared_state: Arc<RwLock<SharedState>>,
    rx_sequence: mpsc::Receiver<(Option<Sequence>, Option<Sequence>)>,
    sequences: (Sequence, Sequence),
    tx_mixed_sequence: mpsc::Sender<Sequence>,
}

impl Mixer {
    pub fn new(
        shared_state: Arc<RwLock<SharedState>>,
        tx_mixed_sequence: mpsc::Sender<Sequence>,
        rx_sequence: mpsc::Receiver<(Option<Sequence>, Option<Sequence>)>,
    ) -> Self {
        Mixer {
            shared_state,
            rx_sequence,
            sequences: (Sequence::default(), Sequence::default()),
            tx_mixed_sequence,
        }
    }

    pub async fn run(&mut self) {
        let mut previous_state = MixerState::default();

        loop {
            let state = self.shared_state.read().await.mixer_state.clone();

            if state != previous_state {
                debug!("Mixer received update request");
                self.mix().await;
                previous_state = state;
            }

            if let Some(sequences) = self.rx_sequence.recv().await {
                debug!("Mixer received sequences {:?}", sequences);
                match sequences {
                    (Some(left), Some(right)) => self.sequences = (left, right),
                    (Some(left), None) => self.sequences = (left, self.sequences.1.clone()),
                    (None, Some(right)) => self.sequences = (self.sequences.0.clone(), right),
                    (None, None) => {}
                }
                self.mix().await;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

    pub async fn mix(&mut self) {
        // Determine resulting sequence length
        let len_a = self.sequences.0.notes.len();
        let len_b = self.sequences.1.notes.len();

        let common_factor = if len_a >= len_b {
            len_a % len_b == 0
        } else {
            len_b % len_a == 0
        };

        let sequence_length = if common_factor {
            max(len_a, len_b)
        } else {
            integer::lcm(len_a, len_b)
        };

        let mut mixed_sequence = Sequence::empty();
        for i in 0..sequence_length - 1 {
            let note_a = self.sequences.0.notes[i % len_a];
            let note_b = self.sequences.1.notes[i % len_b];
            let mixed_note = match (note_a.pitch, note_b.pitch) {
                (0, 0) => note_a,
                (_, 0) => note_a,
                (0, _) => note_b,
                (_, _) => {
                    let mixer_ratio = self.shared_state.read().await.mixer_state.ratio;
                    let r = random::<f32>();
                    if r >= mixer_ratio {
                        note_b
                    } else {
                        note_a
                    }
                }
            };
            mixed_sequence.notes.push(mixed_note);
        }
        info!("Created sequence with {} notes from sequences with length {} and {} (common factor {})", sequence_length, len_a, len_b, common_factor);

        if let Err(e) = self.tx_mixed_sequence.send(mixed_sequence).await {
            error!("Error sending mixed sequence: {}", e);
        }
    }
}
