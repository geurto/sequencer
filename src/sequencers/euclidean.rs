use super::Sequencer;
use crate::common::{Note, NoteDuration, Sequence, SharedState};

use log::{info, warn};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct EuclideanSequencerConfig {
    pub steps: usize,
    pub pulses: usize,
    pub pitch: u8,
}

impl EuclideanSequencerConfig {
    pub fn new() -> Self {
        EuclideanSequencerConfig {
            steps: 16,
            pulses: 0,
            pitch: 60,
        }
    }
}
pub struct EuclideanSequencer {
    config: EuclideanSequencerConfig,
    config_rx: mpsc::Receiver<EuclideanSequencerConfig>,
    shared_state: Arc<Mutex<SharedState>>,
}

impl EuclideanSequencer {
    pub fn new(config_rx: mpsc::Receiver<EuclideanSequencerConfig>, shared_state: Arc<Mutex<SharedState>>) -> Self {
        let mut config = EuclideanSequencerConfig::new();
        EuclideanSequencer { config, config_rx, shared_state }
    }

    pub async fn increase_steps(&mut self) {
        if self.config.steps < 16 {
            self.config.steps += 1;
        }
        info!("Steps: {}", self.config.steps);
    }

    pub async fn decrease_steps(&mut self) {
        if self.config.steps > 1 {
            self.config.steps -= 1;
        }
        info!("Steps: {}", self.config.steps);
    }

    pub async fn increase_pulses(&mut self) {
        if self.config.pulses < 16 {
            self.config.pulses += 1;
        }
        info!("Pulses: {}", self.config.pulses);
    }

    pub async fn decrease_pulses(&mut self) {
        if self.config.pulses > 1 {
            self.config.pulses -= 1;
        }
        info!("Pulses: {}", self.config.pulses);
    }
}

impl Sequencer for EuclideanSequencer {
    async fn generate_sequence(&self, length: usize) -> Sequence {
        let mut sequence = Sequence::empty();

        if self.config.pulses == 0 {
            // Handle zero pulses case
            let note = Note::new(0, 100, NoteDuration::Sixteenth, self.shared_state.lock().await.bpm);
            sequence.notes.push(note);
            return sequence;
        }

        // Bresenham line algorithm cus it looks easier
        let beat_locations = (0..self.config.pulses)
            .map(|i| (i * self.config.steps) / self.config.pulses)
            .collect::<Vec<_>>();

        for i in 0..length {
            let note = if beat_locations.contains(&(i % self.config.steps)) {
                Note::new(self.config.pitch, 100, NoteDuration::Sixteenth, self.shared_state.lock().await.bpm)
            } else {
                Note::new(0, 100, NoteDuration::Sixteenth, self.shared_state.lock().await.bpm)
            };
            sequence.notes.push(note);
        }
        sequence
    }
}

pub enum EuclideanSequencerInput {
    IncreaseSteps,
    DecreaseSteps,
    IncreasePulses,
    DecreasePulses,
    IncreasePitch,
    DecreasePitch,
    IncreaseOctave,
    DecreaseOctave,
}