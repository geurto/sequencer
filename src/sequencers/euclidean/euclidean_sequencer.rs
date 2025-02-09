use crate::note::{Note, NoteDuration, Sequence};
use crate::sequencers::euclidean::config::EuclideanSequencerConfig;
use crate::sequencers::traits::Sequencer;

use crate::state::SharedState;
use anyhow::Result;
use log::{debug, error};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

pub struct EuclideanSequencer {
    config: EuclideanSequencerConfig,
    config_rx: mpsc::Receiver<EuclideanSequencerConfig>,
    gui_tx: mpsc::Sender<EuclideanSequencerConfig>,
    mixer_update_tx: mpsc::Sender<()>,
    shared_state: Arc<Mutex<SharedState>>,
}

impl EuclideanSequencer {
    pub fn new(
        config_rx: mpsc::Receiver<EuclideanSequencerConfig>,
        gui_tx: mpsc::Sender<EuclideanSequencerConfig>,
        mixer_update_tx: mpsc::Sender<()>,
        shared_state: Arc<Mutex<SharedState>>,
    ) -> Self {
        let config = EuclideanSequencerConfig::new();
        EuclideanSequencer {
            config,
            config_rx,
            gui_tx,
            mixer_update_tx,
            shared_state,
        }
    }
}

impl Sequencer for EuclideanSequencer {
    async fn generate_sequence(&self) -> Sequence {
        let mut sequence = Sequence::empty();

        if self.config.pulses == 0 {
            // Handle zero pulses case
            let note = Note::new(
                0,
                0,
                NoteDuration::Sixteenth,
                self.shared_state.lock().await.bpm,
            );
            sequence.notes.push(note);
            return sequence;
        }

        // Bresenham line algorithm cus it looks easier
        let beat_locations = (0..self.config.pulses)
            .map(|i| (i * self.config.steps) / self.config.pulses)
            .collect::<Vec<_>>();

        for i in 0..self.config.steps {
            let note = if beat_locations.contains(&(i % self.config.steps)) {
                Note::new(
                    self.config.pitch,
                    100,
                    NoteDuration::Sixteenth,
                    self.shared_state.lock().await.bpm,
                )
            } else {
                Note::new(
                    0,
                    0,
                    NoteDuration::Sixteenth,
                    self.shared_state.lock().await.bpm,
                )
            };
            sequence.notes.push(note);
        }
        sequence
    }

    async fn run(&mut self, sequencer_slot: usize) -> Result<()> {
        loop {
            if let Some(config) = self.config_rx.recv().await {
                debug!("Euclidean sequencer received config: {:?}", config);
                self.config = config.clone();
                if let Err(e) = self.gui_tx.send(config).await {
                    error!("Error sending GUI message: {}", e);
                }
                let sequence = self.generate_sequence().await;
                {
                    let mut state = self.shared_state.lock().await;
                    match sequencer_slot {
                        0 => state.mixer_config.sequence_a = sequence,
                        1 => state.mixer_config.sequence_b = sequence,
                        _ => anyhow::bail!("Invalid sequencer slot"),
                    }
                }
                self.mixer_update_tx.send(()).await?;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }
}
