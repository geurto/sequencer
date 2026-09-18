use anyhow::Result;
use env_logger::Builder;
use log::{error, info, warn};
use seq_ui::{ControlEvent, Slot};
use std::{sync::mpsc::channel as sync_channel, thread};
use tokio::signal;
use tokio::sync::{mpsc, watch};

use sequencer::{
    Gui, MidiCommand, MidirSink, PlaybackEngine, Sequencer, SequencerState,
    gui::sequencers::euclidean::Gui as EuclideanGui, midi_utils,
    playback::state::PlaybackCommand,
};

/// Plenty for a human at a keyboard; input is never allowed to block.
const CONTROL_CAPACITY: usize = 64;

#[tokio::main]
async fn main() -> Result<()> {
    Builder::new().filter(None, log::LevelFilter::Info).init();

    // User intentions, from any input surface to the sequencer task.
    let (tx_control, rx_control) =
        mpsc::channel::<ControlEvent>(CONTROL_CAPACITY);

    // MIDI port queries from the window.
    let (tx_midi, rx_midi) = mpsc::channel::<MidiCommand>(8);

    // Commands to the synchronous playback thread.
    let (tx_playback, rx_playback) = sync_channel::<PlaybackCommand>();

    // The play head, engine -> sequencer task. Lossy-latest: a missed step
    // should be skipped, not queued.
    let (tx_step, rx_step) = watch::channel(0usize);

    // The frame the UI renders, sequencer task -> window.
    let state = SequencerState::default();
    let (tx_snapshot, rx_snapshot) = watch::channel(state.ui_snapshot());

    // The one owner of the sequencer's state. Everything else talks to it
    // through the channels above; nothing polls and nothing shares a lock.
    let mut sequencer = Sequencer::new(
        state,
        rx_control,
        rx_midi,
        rx_step,
        tx_playback.clone(),
        tx_snapshot,
    );
    tokio::spawn(async move { sequencer.run().await });

    // Playback. Starting without a port is not fatal — the window can pick one
    // later — so a missing device only costs a warning.
    let sink: sequencer::playback::engine::BoxedSink = match open_first_port() {
        Ok(connection) => Box::new(MidirSink(connection)),
        Err(e) => {
            warn!(
                "Starting without MIDI output: {e}. \
                 Pick a port in the window once one is available."
            );
            Box::new(seq_core::SilentSink)
        }
    };

    let playback_engine = PlaybackEngine::new(rx_playback, tx_step, sink);
    thread::spawn(move || playback_engine.run());

    // Shutdown. Installed before the window takes over the calling thread:
    // spawning it afterwards means it only starts once the window has already
    // closed and main is about to return anyway.
    tokio::spawn(async move {
        if let Err(e) = signal::ctrl_c().await {
            error!("Failed to install Ctrl+C handler: {e}");
            return;
        }
        info!("Ctrl+C received, exiting...");
        std::process::exit(0);
    });

    Gui::run(
        rx_snapshot,
        tx_control,
        tx_midi,
        EuclideanGui::new(Slot::Left),
        EuclideanGui::new(Slot::Right),
    )?;

    Ok(())
}

fn open_first_port() -> Result<midir::MidiOutputConnection> {
    let ports = midi_utils::list_ports()?;
    let port = ports.first().ok_or_else(|| {
        anyhow::anyhow!(
            "no MIDI output ports found (start a synthesiser such as FluidSynth, \
             or connect a device)"
        )
    })?;
    info!("Connecting to MIDI output port {port}");
    midi_utils::create_connection(port)
}
