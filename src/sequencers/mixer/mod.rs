pub mod gui;
pub mod state;

use crate::{note::Sequence, SharedState};
use log::{debug, error};
use state::MixerState;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

pub struct Mixer {
    cached_state: MixerState,
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
            cached_state: MixerState::default(),
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
                self.cached_state = state.clone();
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
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

    pub async fn mix(&mut self) {
        // TODO: implement normally
        let mixed_sequence = self.sequences.0.clone();

        if let Err(e) = self.tx_mixed_sequence.send(mixed_sequence).await {
            error!("Error sending mixed sequence: {}", e);
        }
    }
}
