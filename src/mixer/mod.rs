pub mod state;

use crate::{
    playback::state::{
        MidiEventType, PolyphonicSequence, SharedState, TimedEvent,
        TICKS_PER_QUARTER_NOTE,
    },
    MixerState, Sequence,
};
use log::{debug, error, info};
use num::integer;
use rand::random;
use std::{cmp::max, sync::Arc};
use tokio::sync::{mpsc, RwLock};

pub struct Mixer {
    state: MixerState,
    sequences: (Sequence, Sequence),
    shared_state: Arc<RwLock<SharedState>>,
    rx_sequence: mpsc::Receiver<(Option<Sequence>, Option<Sequence>)>,
    tx_polyphonic_sequence: mpsc::Sender<PolyphonicSequence>,
}

impl Mixer {
    pub fn new(
        shared_state: Arc<RwLock<SharedState>>,
        rx_sequence: mpsc::Receiver<(Option<Sequence>, Option<Sequence>)>,
        tx_polyphonic_sequence: mpsc::Sender<PolyphonicSequence>,
    ) -> Self {
        Mixer {
            state: MixerState::default(),
            sequences: (Sequence::default(), Sequence::default()),
            shared_state,
            rx_sequence,
            tx_polyphonic_sequence,
        }
    }

    pub async fn run(&mut self) {
        loop {
            let r_state = self.shared_state.read().await.mixer;
            if r_state != self.state {
                debug!("Mixer received update request");
                self.state = r_state;
                self.mix().await;
            }

            while let Ok(sequences) = self.rx_sequence.try_recv() {
                debug!("Mixer received sequences.");
                match sequences {
                    (Some(left), Some(right)) => self.sequences = (left, right),
                    (Some(left), None) => {
                        self.sequences = (left, self.sequences.1.clone())
                    }
                    (None, Some(right)) => {
                        self.sequences = (self.sequences.0.clone(), right)
                    }
                    (None, None) => {}
                }
                let mixed_sequence = self.mix().await;
                let polyphonic_sequence =
                    self.make_polyphonic_sequence(mixed_sequence).await;

                if let Err(e) =
                    self.tx_polyphonic_sequence.send(polyphonic_sequence).await
                {
                    error!("Error sending mixed sequence: {}", e);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

    pub async fn mix(&mut self) -> Sequence {
        // Determine resulting sequence length
        let len_a = self.sequences.0.notes.len();
        let len_b = self.sequences.1.notes.len();

        let common_factor = if len_a >= len_b {
            len_a.is_multiple_of(len_b)
        } else {
            len_b.is_multiple_of(len_a)
        };

        let sequence_length = if common_factor {
            max(len_a, len_b)
        } else {
            integer::lcm(len_a, len_b)
        };

        let mut mixed_sequence = Sequence::empty();
        for i in 0..sequence_length {
            let note_a = self.sequences.0.notes[i % len_a];
            let note_b = self.sequences.1.notes[i % len_b];
            let mut mixed_note = note_a;
            mixed_note.pitch = match (note_a.pitch, note_b.pitch) {
                (0, 0) => 0,
                (_, 0) => note_a.pitch,
                (0, _) => note_b.pitch,
                (_, _) => {
                    let mixer_ratio = self.state.ratio;
                    let r = random::<f32>();
                    if r > mixer_ratio {
                        note_b.pitch
                    } else {
                        note_a.pitch
                    }
                }
            };
            mixed_sequence.notes.push(mixed_note);
        }
        info!("Created sequence with {} notes from sequences with length {} and {} (common factor {})", sequence_length, len_a, len_b, common_factor);

        mixed_sequence
    }

    // This currently assumes only 16th notes are played.
    async fn make_polyphonic_sequence(
        &self,
        sequence: Sequence,
    ) -> PolyphonicSequence {
        let mut timed_events: Vec<TimedEvent> = Vec::new();

        for (i, note) in sequence.notes.clone().iter().enumerate() {
            if note.pitch != 0 {
                let tick_position = i as u32 * TICKS_PER_QUARTER_NOTE / 4;

                // ON event
                timed_events.push(TimedEvent {
                    tick: tick_position,
                    event: MidiEventType::NoteOn {
                        pitch: note.pitch,
                        velocity: 100u8,
                    },
                });

                // OFF event
                timed_events.push(TimedEvent {
                    tick: tick_position + TICKS_PER_QUARTER_NOTE / 4 - 1,
                    event: MidiEventType::NoteOn {
                        pitch: note.pitch,
                        velocity: 100u8,
                    },
                });
            }
        }

        PolyphonicSequence {
            events: timed_events,
            total_ticks: sequence.notes.len() as u32
                * TICKS_PER_QUARTER_NOTE
                * 4u32,
        }
    }
}
