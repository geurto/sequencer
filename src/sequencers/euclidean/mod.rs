pub mod gui;
pub mod state;

use crate::note::{Note, NoteDuration, Sequence};
use crate::sequencers::common::Sequencer;
use crate::sequencers::euclidean::state::EuclideanSequencerState;

use crate::state::SharedState;
use anyhow::Result;
use log::{debug, error};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::RwLock;

pub struct EuclideanSequencer {
    config: EuclideanSequencerState,
    rx_config: mpsc::Receiver<EuclideanSequencerState>,
    tx_gui: mpsc::Sender<EuclideanSequencerState>,
    tx_sequence: mpsc::Sender<(Option<Sequence>, Option<Sequence>)>,
    shared_state: Arc<RwLock<SharedState>>,
}

impl EuclideanSequencer {
    pub fn new(
        rx_config: mpsc::Receiver<EuclideanSequencerState>,
        tx_gui: mpsc::Sender<EuclideanSequencerState>,
        tx_sequence: mpsc::Sender<(Option<Sequence>, Option<Sequence>)>,
        shared_state: Arc<RwLock<SharedState>>,
    ) -> Self {
        let config = EuclideanSequencerState::new();
        EuclideanSequencer {
            config,
            rx_config,
            tx_gui,
            tx_sequence,
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
                self.shared_state.read().await.bpm,
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
                    self.shared_state.read().await.bpm,
                )
            } else {
                Note::new(
                    0,
                    0,
                    NoteDuration::Sixteenth,
                    self.shared_state.read().await.bpm,
                )
            };
            sequence.notes.push(note);
        }
        sequence
    }

    async fn run(&mut self, sequencer_slot: usize) -> Result<()> {
        loop {
            if let Some(config) = self.rx_config.recv().await {
                debug!("Euclidean sequencer received config: {:?}", config);
                self.config = config.clone();
                if let Err(e) = self.tx_gui.send(config).await {
                    error!("Error sending GUI message: {}", e);
                }
                let sequence = self.generate_sequence().await;
                {
                    match sequencer_slot {
                        0 => self.tx_sequence.send((Some(sequence), None)).await?,
                        1 => self.tx_sequence.send((None, Some(sequence))).await?,
                        _ => anyhow::bail!("Invalid sequencer slot"),
                    };
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }
}
