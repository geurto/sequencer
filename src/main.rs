use anyhow::Result;
use env_logger::Builder;
use std::{
    sync::{mpsc::channel as sync_channel, Arc, Mutex as SyncMutex},
    thread,
};
use tokio::sync::mpsc;
use tokio::{signal, sync::RwLock};

use sequencer::{
    gui::{sequencers::euclidean::Gui as EuclideanGui, Message as GuiMessage},
    midi_utils,
    playback::state::{PolyphonicSequence, SequencerSlot, SharedState},
    EuclideanSequencer, Gui, MidiCommand, Mixer, PlaybackEngine,
    PlaybackHandler, PlaybackStatus, Sequence, Sequencer,
};

#[tokio::main]
async fn main() -> Result<()> {
    Builder::new().filter(None, log::LevelFilter::Info).init();

    // sequences FROM sequencers TO mixer
    let (tx_sequence, rx_sequence) =
        mpsc::channel::<(Option<Sequence>, Option<Sequence>)>(1);

    // mixed sequence FROM mixer TO playback_handler
    let (tx_mixed_sequence, rx_mixed_sequence) =
        mpsc::channel::<PolyphonicSequence>(1);

    // MIDI messages, either GUI or playing a note
    let (tx_midi, rx_midi) = mpsc::channel::<MidiCommand>(1);

    // synchronous playback commands & status
    let (tx_playback_cmd, rx_playback_cmd) = sync_channel();
    let (tx_playback_status, rx_playback_status) =
        mpsc::unbounded_channel::<PlaybackStatus>();

    // state updates to GUI
    let tx_gui: Arc<
        SyncMutex<Option<iced::futures::channel::mpsc::Sender<GuiMessage>>>,
    > = Arc::new(SyncMutex::new(None));

    // shared state which is read by multiple structs
    let shared_state: Arc<RwLock<SharedState>> =
        Arc::new(RwLock::new(SharedState::new(120.)));

    // Sequencers and mixer
    let mut sequencer_a = EuclideanSequencer::new(
        SequencerSlot::Left,
        shared_state.clone(),
        tx_sequence.clone(),
    );
    sequencer_a.generate_sequence().await;
    tokio::spawn(async move {
        sequencer_a.run().await;
    });

    // both Euclidean for now to keep it simple
    let mut sequencer_b = EuclideanSequencer::new(
        SequencerSlot::Right,
        shared_state.clone(),
        tx_sequence.clone(),
    );
    sequencer_b.generate_sequence().await;
    tokio::spawn(async move { sequencer_b.run().await });

    let mut sequence_mixer =
        Mixer::new(shared_state.clone(), rx_sequence, tx_mixed_sequence);
    sequence_mixer.mix().await;
    tokio::spawn(async move { sequence_mixer.run().await });

    // Playback
    let midi_ports = midi_utils::list_ports()?;
    let midi_conn = midi_utils::create_connection(midi_ports[0].clone())?;

    // Link between async GUI and sync playback engine
    let tx_gui_playback = tx_gui.clone();
    let mut playback_handler = PlaybackHandler::new(
        shared_state.clone(),
        rx_midi,
        rx_mixed_sequence,
        tx_playback_cmd,
        rx_playback_status,
        tx_gui_playback,
    );
    tokio::spawn(async move { playback_handler.run().await });

    // Synchronous playback engine that handles MIDI control
    let playback_engine =
        PlaybackEngine::new(rx_playback_cmd, tx_playback_status, midi_conn);
    thread::spawn(move || {
        playback_engine.run();
    });

    // GUI
    let gui_sequencer_left = EuclideanGui::new(SequencerSlot::Left);
    let gui_sequencer_right = EuclideanGui::new(SequencerSlot::Right);

    Gui::run(
        tx_gui.clone(),
        tx_midi,
        gui_sequencer_left,
        gui_sequencer_right,
    )?;

    // Shutdown
    let _ctrl_c_handle = tokio::spawn(async move {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        println!("Ctrl+C received, exiting...");
        std::process::exit(0);
    });

    Ok(())
}
