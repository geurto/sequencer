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
use rand::{random, random_range};
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
                if let Err(e) =
                    self.tx_polyphonic_sequence.send(mixed_sequence).await
                {
                    error!("Error sending mixed sequence: {}", e);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

    // TODO this can actually be polyphonic now
    pub async fn mix(&mut self) -> PolyphonicSequence {
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

        let mut timed_events: Vec<TimedEvent> = Vec::new();
        for i in 0..sequence_length {
            let tick_position = i as u32 * TICKS_PER_QUARTER_NOTE / 4;

            let note_a = self.sequences.0.notes[i % len_a];
            let note_b = self.sequences.1.notes[i % len_b];
            let mut mixed_note = note_a;

            match (note_a.pitch, note_b.pitch) {
                (0, 0) => {}
                (_, 0) => {
                    self.add_note_on(
                        note_a.pitch,
                        100u8,
                        tick_position,
                        &mut timed_events,
                    );
                    self.add_note_off(
                        note_a.pitch,
                        tick_position + TICKS_PER_QUARTER_NOTE / 4 - 1,
                        &mut timed_events,
                    );
                }
                (0, _) => {
                    self.add_note_on(
                        note_a.pitch,
                        100u8,
                        tick_position,
                        &mut timed_events,
                    );
                    self.add_note_off(
                        note_a.pitch,
                        tick_position + TICKS_PER_QUARTER_NOTE / 4 - 1,
                        &mut timed_events,
                    );
                }
                (_, _) => {
                    let mixer_ratio = self.state.ratio;
                    let dominant_note =
                        if mixer_ratio > 0.5 { note_b } else { note_a };
                    let accent_note =
                        if mixer_ratio > 0.5 { note_a } else { note_b };

                    if num::abs_sub(mixer_ratio, 0.5) < 0.2 {
                        let dominant_velocity: u8 = random_range(60..=80);
                        let accent_velocity: u8 = random_range(40..=60);
                        self.add_note_on(
                            dominant_note.pitch,
                            dominant_velocity,
                            tick_position,
                            &mut timed_events,
                        );
                        self.add_note_on(
                            accent_note.pitch,
                            accent_velocity,
                            tick_position,
                            &mut timed_events,
                        );

                        self.add_note_off(
                            dominant_note.pitch,
                            tick_position + TICKS_PER_QUARTER_NOTE / 4 - 1,
                            &mut timed_events,
                        );
                        self.add_note_off(
                            accent_note.pitch,
                            tick_position + TICKS_PER_QUARTER_NOTE / 4 - 1,
                            &mut timed_events,
                        );
                    }
                }
            }
        }
        info!("Created sequence with {} notes from sequences with length {} and {} (common factor {})", sequence_length, len_a, len_b, common_factor);

        PolyphonicSequence {
            events: timed_events,
            total_ticks: sequence_length as u32 * TICKS_PER_QUARTER_NOTE / 4u32,
        }
    }

    fn add_note_on(
        &self,
        pitch: u8,
        velocity: u8,
        tick_position: u32,
        timed_events: &mut Vec<TimedEvent>,
    ) {
        // ON event
        timed_events.push(TimedEvent {
            tick: tick_position,
            event: MidiEventType::NoteOn { pitch, velocity },
        });
    }

    fn add_note_off(
        &self,
        pitch: u8,
        tick_position: u32,
        timed_events: &mut Vec<TimedEvent>,
    ) {
        // OFF event
        timed_events.push(TimedEvent {
            tick: tick_position + TICKS_PER_QUARTER_NOTE / 4 - 1,
            event: MidiEventType::NoteOff { pitch },
        });
    }
}
