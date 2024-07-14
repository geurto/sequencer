/// Creating a Sequence object from a Markov chain.
/// Use scale degrees to start with: tonic, dominant, etc. (1-7)
/// I: tonic, II: supertonic, III: mediant, IV, subdominant, V: dominant, VI: submediant, VII: leading tone/subtonic
use anyhow::Error;
use log::debug;
use markov::Chain;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::note::{Note, NoteDuration, Sequence};
use crate::sequencers::traits::Sequencer;
use crate::sequencers::markov::config::MarkovSequencerConfig;
use crate::sequencers::mixer::config::MixerConfig;
use crate::state::SharedState;

pub struct MarkovSequencer {
    chain: Chain<u8>,
    config: MarkovSequencerConfig,
    config_rx: mpsc::Receiver<MarkovSequencerConfig>,
    mixer_update_tx: mpsc::Sender<()>,
    shared_state: Arc<Mutex<SharedState>>,
}

impl MarkovSequencer {
    pub fn new(config_rx: mpsc::Receiver<MarkovSequencerConfig>,
               mixer_update_tx: mpsc::Sender<()>,
               shared_state: Arc<Mutex<SharedState>>) -> Self {
        let config = MarkovSequencerConfig::new();
        let mut chain = Chain::new();

        // from https://topmusicarts.com/blogs/news/5-most-used-chord-progressions-in-edm
        chain.feed(&[6, 4, 5, 1]);
        chain.feed(&[6, 5, 1, 4]);
        chain.feed(&[6, 4, 1, 5]);
        chain.feed(&[6, 4, 1, 5]);
        chain.feed(&[1, 5, 6, 4]);

        // from https://blog.landr.com/edm-chord-progression/
        chain.feed(&[5, 1]);
        chain.feed(&[6, 7, 1, 6]);
        chain.feed(&[1, 3, 5, 4]);
        chain.feed(&[3, 4, 5, 6, 7, 1]);
        chain.feed(&[6, 1, 7, 5]);
        chain.feed(&[5, 4, 6, 1, 2, 1, 4]);

        Self { chain, config, config_rx, mixer_update_tx, shared_state }
    }
}

impl Sequencer for MarkovSequencer {
    async fn generate_sequence(&self) -> Sequence {
        let state = self.shared_state.lock().await;
        let mut sequence = Sequence::default();
        let degrees = self.chain.generate();
        let num_notes = degrees.len();
        for i in 0..self.config.steps {
            let note = Note::new(degrees[i % num_notes], 100, NoteDuration::Quarter, state.bpm);
            sequence.notes.push(note);
        }
        sequence
    }

    async fn run(&mut self, sequencer_slot: usize) -> Result<(), Error> {
        loop {
            if let Some(config) = self.config_rx.recv().await {
                debug!("Markov sequencer received config: {:?}", config);
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
                self.mixer_update_tx.send(()).await?;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }
}
