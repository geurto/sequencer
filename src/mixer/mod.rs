//! The channel plumbing around [`seq_core::mixer::mix`].
//!
//! This type owns no musical logic: it holds the two most recent patterns,
//! watches the shared state for ratio changes, and republishes the mixed
//! sequence whenever either input moves.

pub mod state;

use log::{debug, error};
use seq_core::{Pattern, PolyphonicSequence};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

use crate::{MixerState, playback::state::SharedState};

#[derive(Debug)]
pub struct Mixer {
    state: MixerState,
    patterns: (Pattern, Pattern),
    shared_state: Arc<RwLock<SharedState>>,
    rx_pattern: mpsc::Receiver<(Option<Pattern>, Option<Pattern>)>,
    tx_polyphonic_sequence: mpsc::Sender<Box<PolyphonicSequence>>,
}

impl Mixer {
    pub fn new(
        shared_state: Arc<RwLock<SharedState>>,
        rx_pattern: mpsc::Receiver<(Option<Pattern>, Option<Pattern>)>,
        tx_polyphonic_sequence: mpsc::Sender<Box<PolyphonicSequence>>,
    ) -> Self {
        Mixer {
            state: MixerState::default(),
            patterns: (Pattern::default(), Pattern::default()),
            shared_state,
            rx_pattern,
            tx_polyphonic_sequence,
        }
    }

    pub async fn run(&mut self) {
        loop {
            let r_state = self.shared_state.read().await.mixer;
            if r_state != self.state {
                debug!("Mixer received update request");
                self.state = r_state;
                self.publish().await;
            }

            while let Ok(patterns) = self.rx_pattern.try_recv() {
                debug!("Mixer received patterns.");
                if let (None, None) = patterns {
                    continue;
                }
                if let Some(left) = patterns.0 {
                    self.patterns.0 = left;
                }
                if let Some(right) = patterns.1 {
                    self.patterns.1 = right;
                }
                self.publish().await;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

    async fn publish(&mut self) {
        // Boxed before the await: a PolyphonicSequence is ~32 KB inline (it is
        // a no_std fixed-capacity type), and holding it by value would bloat
        // every future and channel between here and the engine.
        let mixed_sequence = Box::new(seq_core::mixer::mix(
            &self.patterns.0,
            &self.patterns.1,
            self.state.ratio,
            &mut rand::rng(),
        ));
        if let Err(e) = self.tx_polyphonic_sequence.send(mixed_sequence).await {
            error!("Error sending mixed sequence: {e}");
        }
    }
}
