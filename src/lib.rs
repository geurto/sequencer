//! The desktop sequencer application: GUI, keyboard input, MIDI ports and the
//! async plumbing between them.
//!
//! Everything musical — pattern types, the Euclidean generator, the mixer and
//! the playback transport — lives in the platform-independent [`seq_core`]
//! crate; this one only drives it.

pub mod gui;
pub mod mixer;
pub mod playback;
pub mod sequencers;

pub use gui::Gui;
pub use mixer::{Mixer, state::MixerState};
pub use playback::{
    PlaybackHandler,
    engine::PlaybackEngine,
    midi::{MidiCommand, MidirSink, midi_utils},
    state::{PlaybackCommand, PlaybackStatus, SharedState},
};
pub use sequencers::euclidean::{
    EuclideanSequencer, state::EuclideanSequencerState,
};
