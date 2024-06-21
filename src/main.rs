mod playback;
mod sequencers;
mod common;
mod midi;
mod input;

use ctrlc;
use env_logger::Builder;
use std::{error::Error, sync::Arc};
use tokio::sync::{mpsc, Mutex};

use crate::common::{Input, SharedState};
use crate::midi::MidiHandler;
use crate::sequencers::markov::MarkovSequencer;
use crate::sequencers::euclidean::EuclideanSequencer;
use crate::sequencers::Sequencer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    Builder::new().filter(None, log::LevelFilter::Debug).init();
    let mut midi_handler = MidiHandler::new()?;
    let (tx, rx) = mpsc::channel(32);

    let shared_state = Arc::new(Mutex::new(SharedState::new(120.0)));
    midi_handler.setup_midi_input(shared_state.clone()).await?;

    playback::start_playback_loop(midi_handler, tx.clone(), rx, shared_state.clone()).await?;

    let _sequencer = MarkovSequencer::new(shared_state.clone());
    let sequencer = EuclideanSequencer::new(16, 7, 60, shared_state.clone());

    let tx_input = tx.clone();
    tokio::spawn(async move {
        input::handle_user_input(tx_input).await;
    });

    let tx_ctrlc = tx.clone();
    ctrlc::set_handler(move || {
        let tx_ctrlc = tx_ctrlc.clone();
        tokio::spawn(async move {
            tx_ctrlc.send(Input::Shutdown).await.unwrap();
        });
    }).expect("Error setting Ctrl-C handler");

    loop {
        let new_sequence = sequencer.generate_sequence(16).await;
        tx.send(Input::Sequence(new_sequence)).await.unwrap();
    }
}