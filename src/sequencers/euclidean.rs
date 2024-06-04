use super::Sequencer;
use crate::common::{Note, NoteDuration, Sequence, SharedState};

use log::warn;
use std::sync::Arc;
use tokio::sync::Mutex;


pub struct EuclideanSequencer {
    steps: usize,
    pulses: usize,
    pitch: u8,
    shared_state: Arc<Mutex<SharedState>>,
}

impl EuclideanSequencer {
    pub fn new(steps: usize, pulses: usize, pitch: u8, shared_state: Arc<Mutex<SharedState>>) -> Self {
        if pulses > steps {
            warn!("Pulses cannot be greater than steps. Setting pulses to steps.");
            return EuclideanSequencer {
                steps,
                pulses: steps,
                pitch,
                shared_state
            };
        }

        EuclideanSequencer {
            steps,
            pulses,
            pitch,
            shared_state
        }
    }
}

impl Sequencer for EuclideanSequencer {
    async fn generate_sequence(&self, length: usize) -> Sequence {
        let mut sequence = Sequence::default();

        if self.pulses == 0 {
            // Handle zero pulses case
            let note = Note::new(0, 100, NoteDuration::Sixteenth, self.shared_state.lock().await.bpm);
            sequence.notes.push(note);
            return sequence;
        }

        // Bresenham line algorithm cus it looks easier
        let beat_locations = (0..self.pulses)
            .map(|i| (i * self.steps) / self.pulses)
            .collect::<Vec<_>>();

        for i in 0..length {
            let note_pitch = if beat_locations.contains(&(i % self.steps)) {
                self.pitch
            } else {
                0
            };
            let note = Note::new(note_pitch, 100, NoteDuration::Sixteenth, self.shared_state.lock().await.bpm);
            sequence.notes.push(note);
        }
        sequence
    }
}