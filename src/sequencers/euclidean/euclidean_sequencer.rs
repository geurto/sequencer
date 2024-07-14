use crate::note::{Note, NoteDuration, Sequence};
use crate::sequencers::euclidean::config::EuclideanSequencerConfig;
use crate::sequencers::mixer::config::MixerConfig;
use crate::sequencers::traits::Sequencer;

use crate::state::SharedState;
use anyhow::Error;
use log::debug;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

pub struct EuclideanSequencer {
    config: EuclideanSequencerConfig,
    config_rx: mpsc::Receiver<EuclideanSequencerConfig>,
    shared_state: Arc<Mutex<SharedState>>,
}

impl EuclideanSequencer {
    pub fn new(config_rx: mpsc::Receiver<EuclideanSequencerConfig>,
               shared_state: Arc<Mutex<SharedState>>) -> Self {
        let mut config = EuclideanSequencerConfig::new();
        EuclideanSequencer { config, config_rx, shared_state }
    }
}

impl Sequencer for EuclideanSequencer {
    async fn generate_sequence(&self) -> Sequence {
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

        for i in 0..self.config.steps {
            let note = if beat_locations.contains(&(i % self.config.steps)) {
                Note::new(self.config.pitch, 100, NoteDuration::Sixteenth, self.shared_state.lock().await.bpm)
            } else {
                Note::new(0, 100, NoteDuration::Sixteenth, self.shared_state.lock().await.bpm)
            };
            sequence.notes.push(note);
        }
        sequence
    }

    async fn run(&mut self, sequencer_slot: usize) -> Result<(), Error> {
        loop {
            if let Some(config) = self.config_rx.recv().await {
                debug!("Euclidean sequencer received config: {:?}", config);
                self.config = config;
                let sequence = self.generate_sequence().await;
                match sequencer_slot {
                    0 => {
                        self.shared_state.lock().await.mixer_config.sequence_a = sequence;
                    }
                    1 => {
                        self.shared_state.lock().await.mixer_config.sequence_b = sequence;
                    }
                    _ => {
                        panic!("Invalid sequencer slot");
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }
}