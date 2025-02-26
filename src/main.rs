use anyhow::Result;
use device_query::Keycode;
use env_logger::Builder;
use std::{collections::HashSet, sync::Arc};
use tokio::signal;
use tokio::sync::{broadcast, mpsc, RwLock};

use sequencer::{
    gui::AppFlags, run_input_handler, sequencers::euclidean::gui::Gui as EuclideanGui,
    start_polling, EuclideanSequencer, EuclideanSequencerState, Gui, MidiHandler, Mixer,
    MixerState, PlaybackHandler, Sequence, Sequencer, SharedState,
};

#[tokio::main]
async fn main() -> Result<()> {
    Builder::new().filter(None, log::LevelFilter::Info).init();

    // key input handling
    let (tx_keys, rx_keys) = mpsc::channel::<HashSet<Keycode>>(100);

    // sequencer / mixer state - EuclideanSequencerState / MixerState
    let (tx_sequencer_left, rx_sequencer_left) = broadcast::channel::<EuclideanSequencerState>(16);

    let (tx_sequencer_right, rx_sequencer_right) =
        broadcast::channel::<EuclideanSequencerState>(16);

    let (tx_mixer, rx_mixer) = broadcast::channel::<MixerState>(16);

    // separate sequences from left/right - (Option<Sequence>, Option<Sequence>)
    let (tx_sequence, rx_sequence) = mpsc::channel::<(Option<Sequence>, Option<Sequence>)>(1);

    // final sequence for playback - Sequence
    let (tx_mixed_sequence, rx_mixed_sequence) = mpsc::channel::<Sequence>(1);

    let shared_state: Arc<RwLock<SharedState>> = Arc::new(RwLock::new(SharedState::new(120.)));

    // Sequencers and mixer
    let mut sequencer_a =
        EuclideanSequencer::new(rx_sequencer_left, tx_sequence.clone(), shared_state.clone());
    sequencer_a.generate_sequence().await;
    tokio::spawn(async move {
        sequencer_a.run(0).await.unwrap();
    });

    // both Euclidean for now to keep it simple
    let mut sequencer_b = EuclideanSequencer::new(
        rx_sequencer_right,
        tx_sequence.clone(),
        shared_state.clone(),
    );
    sequencer_b.generate_sequence().await;
    tokio::spawn(async move { sequencer_b.run(1).await });

    let mut sequence_mixer = Mixer::new(rx_sequence, tx_mixed_sequence, rx_mixer);
    sequence_mixer.mix().await;
    tokio::spawn(async move { sequence_mixer.run().await });

    // Input handling
    start_polling(tx_keys);
    let shared_state_input = shared_state.clone();
    tokio::spawn(async move {
        run_input_handler(
            rx_keys,
            shared_state_input,
            tx_sequencer_left,
            tx_sequencer_right,
            tx_mixer,
        )
        .await
    });

    // Playback

    let midi_handler = MidiHandler::new()?;
    let mut playback_handler =
        PlaybackHandler::new(midi_handler, rx_mixed_sequence, shared_state.clone());
    tokio::spawn(async move { playback_handler.run().await });

    // Shutdown
    let _ctrl_c_handle = tokio::spawn(async move {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        println!("Ctrl+C received, exiting...");
        std::process::exit(0);
    });

    // GUI
    let gui_sequencer_left = EuclideanGui::new(1);
    let gui_sequencer_right = EuclideanGui::new(2);

    let flags = AppFlags {
        rx_state: rx_gui,
        sequencer_left: gui_sequencer_left,
        sequencer_right: gui_sequencer_right,
    };

    Gui::run(flags)?;
    Ok(())
}
