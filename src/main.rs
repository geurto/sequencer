mod playback;
mod sequencers;
mod common;
mod midi;
mod input;

use tokio::sync::{mpsc, Mutex};
use std::{error::Error, sync::Arc, time::Duration};
use crate::common::{Input, Sequence, SharedState};
use crate::midi::MidiHandler;
use crate::sequencers::markov::MarkovSequencer;
use crate::sequencers::euclidean::EuclideanSequencer;
use crate::sequencers::Sequencer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let midi_handler = MidiHandler::new()?;
    let (tx, rx) = mpsc::channel(32);

    let shared_state = Arc::new(Mutex::new(SharedState {
        bpm: 120.0,
        sequence: Sequence::default(),
    }));

    playback::start_playback_loop(midi_handler, tx.clone(), rx, shared_state.clone()).await?;

    let _sequencer = MarkovSequencer::new(shared_state.clone());
    let sequencer = EuclideanSequencer::new(16, 7, 60, shared_state.clone());

    let tx_clone = tx.clone();
    tokio::spawn(async move {
        input::handle_user_input(tx_clone).await;
    });

    loop {
        let new_sequence = sequencer.generate_sequence(16).await;
        tx.send(Input::Sequence(new_sequence)).await.unwrap();
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}