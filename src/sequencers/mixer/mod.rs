pub mod state;

use crate::note::Sequence;
use log::debug;
use state::MixerState;
use tokio::sync::mpsc::{Receiver, Sender};

pub struct Mixer {
    state: MixerState,
    sequences: (Sequence, Sequence),
    rx_sequence: Receiver<(Option<Sequence>, Option<Sequence>)>,
    tx_sequence: Sender<Sequence>,
    rx_update: Receiver<MixerState>,
}

impl Mixer {
    pub fn new(
        rx_sequence: Receiver<(Option<Sequence>, Option<Sequence>)>,
        tx_sequence: Sender<Sequence>,
        rx_update: Receiver<MixerState>,
    ) -> Self {
        Mixer {
            state: MixerState::default(),
            sequences: (Sequence::default(), Sequence::default()),
            rx_sequence,
            tx_sequence,
            rx_update,
        }
    }

    pub async fn run(&mut self) {
        loop {
            if let Some(state) = self.rx_update.recv().await {
                debug!("Mixer received update request");
                self.state = state;
                self.mix().await;
            }

            if let Some(sequences) = self.rx_sequence.recv().await {
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

        let mut mixed_sequence = Sequence::empty();
        let num_notes = self.sequences.0.notes.len();

        for i in 0..num_notes {
            let note_a = self.sequences.0.notes.get(i);

            if let Some(note_a) = note_a {
                let note = note_a.clone();
                mixed_sequence.notes.push(note);
            }
        }
        self.tx_sequence.send(mixed_sequence);
    }
}
