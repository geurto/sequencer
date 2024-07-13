/// Creating a Sequence object from a Markov chain.
/// Use scale degrees to start with: tonic, dominant, etc. (1-7)
/// I: tonic, II: supertonic, III: mediant, IV, subdominant, V: dominant, VI: submediant, VII: leading tone/subtonic
use super::Sequencer;
use markov::Chain;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::common::{Note, NoteDuration, Sequence, SharedState};

pub struct MarkovSequencer {
    chain: Chain<u8>,
    shared_state: Arc<Mutex<SharedState>>,
}

impl MarkovSequencer {
    pub fn new(shared_state: Arc<Mutex<SharedState>>) -> Self {
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
        Self { chain, shared_state }
    }
}

impl Sequencer for MarkovSequencer {
    async fn generate_sequence(&self, length: usize) -> Sequence {
        let mut sequence = Sequence::default();
        let degrees = self.chain.generate();
        let num_notes = degrees.len();
        for i in 0..length {
            let note = Note::new(degrees[i % num_notes], 100, NoteDuration::Quarter, self.shared_state.lock().await.bpm); // 120 BPM
            sequence.notes.push(note);
        }
        sequence
    }
}

pub enum MarkovSequencerInput {
    Feed(Vec<u8>),
    Generate,
}
