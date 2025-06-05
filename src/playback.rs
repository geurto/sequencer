use anyhow::Result;
use log::{debug, error, info};
use std::sync::{Arc, Mutex as SyncMutex};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Duration};

use crate::gui::{Event, Message};
use crate::midi::MidiHandler;
use crate::note::MixedSequence;
use crate::state::*;

pub struct PlaybackHandler {
    midi_handler: MidiHandler,
    rx_sequence: mpsc::Receiver<MixedSequence>,
    tx_gui: Arc<SyncMutex<Option<iced::futures::channel::mpsc::Sender<Message>>>>,
    shared_state: Arc<RwLock<SharedState>>,
}

impl PlaybackHandler {
    pub fn new(
        midi_handler: MidiHandler,
        rx_sequence: mpsc::Receiver<MixedSequence>,
        tx_gui: Arc<SyncMutex<Option<iced::futures::channel::mpsc::Sender<Message>>>>,
        shared_state: Arc<RwLock<SharedState>>,
    ) -> Self {
        Self {
            midi_handler,
            rx_sequence,
            tx_gui,
            shared_state,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("Starting playback loop");
        let mut current_note_index = 0;
        let mut sequence = MixedSequence::default();

        loop {
            if let Ok(seq) = self.rx_sequence.try_recv() {
                debug!(
                    "Received new sequence: {:?} of length {}",
                    seq,
                    seq.notes.len()
                );
                sequence = seq;
                current_note_index = if sequence.notes.is_empty() {
                    0
                } else {
                    current_note_index % sequence.notes.len()
                };
            }

            let (is_playing, midi_channel_for_note) = {
                let r_state = self.shared_state.read().await;
                (r_state.playing, r_state.midi_channel)
            };

            if is_playing {
                if sequence.notes.is_empty() {
                    sleep(Duration::from_millis(10)).await;
                    continue;
                }

                let note = sequence.notes[current_note_index];
                debug!(
                    "Playing note: {:?} at index {}/{}",
                    note,
                    current_note_index,
                    sequence.notes.len()
                );

                self.midi_handler
                    .play_multiple_notes(note, midi_channel_for_note)
                    .await;

                // Move to the next note
                current_note_index = (current_note_index + 1) % sequence.notes.len();

                // Quickly update current_note_index
                {
                    let mut w_state = self.shared_state.write().await;
                    w_state.current_note_index = current_note_index;
                }

                let r_state = self.shared_state.read().await;
                if let Some(mut tx) = self.tx_gui.lock().unwrap().clone() {
                    if let Err(e) =
                        tx.try_send(Message::ReceivedEvent(Event::StateChanged(r_state.clone())))
                    {
                        error!(
                            "Playback: Error sending Message::ReceivedEvent to GUI: {:?}",
                            e
                        );
                    }
                }
                drop(r_state);
            } else {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }
}
