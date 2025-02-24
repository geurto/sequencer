use anyhow::Result;
use env_logger::Builder;
use sequencer::InputHandler;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};

use sequencer::{
    input::InputHandler, playback::play, sequencers::euclidean::gui::Gui as EuclideanGui,
    EuclideanSequencer, Gui, MidiHandler, Mixer, Sequencer, SharedState,
};

#[tokio::main]
async fn main() -> Result<()> {
    Builder::new().filter(None, log::LevelFilter::Info).init();

    // sequencer / mixer state - EuclideanSequencerState / MixerState
    let (tx_sequencer_left, rx_sequencer_left) = broadcast::channel(2);
    let rx_sequencer_left_gui = tx_sequencer_left.subscribe();

    let (tx_sequencer_right, rx_sequencer_right) = broadcast::channel(2);
    let rx_sequencer_right_gui = tx_sequencer_right.subscribe();

    let (tx_mixer, rx_mixer) = broadcast::channel(2);

    // separate sequences from left/right - (Option<Sequence>, Option<Sequence>)
    let (tx_sequence, rx_sequence) = mpsc::channel(1);

    // final sequence for playback - Sequence
    let (tx_mixed_sequence, rx_mixed_sequence) = mpsc::channel(1);

    let shared_state: RwLock<SharedState> = RwLock::new(SharedState::new(120));

    // MIDI input and output
    let midi_handler = Arc::new(Mutex::new(MidiHandler::new()?));
    midi_handler
        .lock()
        .await
        .setup_midi_input(shared_state)
        .await?;

    let mut handles = vec![];

    // Sequencers and mixer
    let mut sequencer_a =
        EuclideanSequencer::new(rx_sequencer_left, tx_sequence.clone(), shared_state);
    sequencer_a.generate_sequence().await;
    handles.push(tokio::spawn(async move {
        sequencer_a.run(0).await.unwrap();
    }));

    // both Euclidean for now to keep it simple
    let mut sequencer_b =
        EuclideanSequencer::new(rx_sequencer_right, tx_sequence.clone(), shared_state);
    sequencer_b.generate_sequence().await;
    handles.push(tokio::spawn(async move {
        sequencer_b.run(1).await.unwrap();
    }));

    let mut sequence_mixer = Mixer::new(rx_sequence, tx_mixed_sequence, rx_mixer);
    sequence_mixer.mix().await;
    handles.push(tokio::spawn(async move {
        sequence_mixer.run().await;
    }));

    // Input handling
    let input_handler = InputHandler::new(
        shared_state,
        tx_sequencer_left,
        tx_sequencer_right,
        tx_mixer,
    );
    handles.push(tokio::spawn(async move {
        input_handler.run().await;
    }));

    // Playback
    let shared_state_playback = shared_state;
    let midi_handler_clone = midi_handler.clone();
    handles.push(tokio::spawn(async move {
        play(midi_handler_clone, rx_mixed_sequence, shared_state_playback)
            .await
            .unwrap();
    }));

    // Shutdown
    let _ctrl_c_handle = tokio::spawn(async move {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        println!("Ctrl+C received, exiting...");
        std::process::exit(0);
    });

    // GUI
    let gui_sequencer_left = EuclideanGui::new(1, rx_sequencer_left_gui);
    let gui_sequencer_right = EuclideanGui::new(2, rx_sequencer_right_gui);

    Gui::run(rx_gui)?;
    Ok(())
}
