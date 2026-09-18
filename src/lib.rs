//! The desktop sequencer application: window, keyboard, MIDI ports and the
//! task plumbing between them.
//!
//! Everything musical — pattern types, the Euclidean generator, the mixer and
//! the playback transport — lives in the platform-independent [`seq_core`]
//! crate, and the interface seam ([`seq_ui::UiSnapshot`],
//! [`seq_ui::ControlEvent`]) lives in `seq-ui`. This crate only drives them.

pub mod gui;
pub mod playback;
pub mod sequencer;
pub mod state;

pub use gui::Gui;
pub use playback::{
    engine::PlaybackEngine,
    midi::{MidiCommand, MidirSink, midi_utils},
    state::PlaybackCommand,
};
pub use sequencer::Sequencer;
pub use state::{SequencerState, SlotState};
