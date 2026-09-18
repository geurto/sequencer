//! Commands from the sequencer task to the playback engine.

use core::fmt;

use seq_core::PolyphonicSequence;

use crate::playback::engine::BoxedSink;

/// Commands FROM the sequencer task TO the playback thread.
pub enum PlaybackCommand {
    /// Boxed because the sequence is a ~32 KB fixed-capacity value; inline it
    /// would dominate the enum and get memcpy'd through every channel hop.
    LoadSequence(Box<PolyphonicSequence>),
    SetPlaying(bool),
    SetMidiChannel(u8),
    SetBPM(f64),
    SetOutputConnection(BoxedSink),
}

/// Hand-written because `SetOutputConnection` carries a trait object, and
/// because a whole sequence dumped into a log line is noise.
impl fmt::Debug for PlaybackCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoadSequence(sequence) => f
                .debug_tuple("LoadSequence")
                .field(&format_args!("{} events", sequence.events().len()))
                .finish(),
            Self::SetPlaying(playing) => {
                f.debug_tuple("SetPlaying").field(playing).finish()
            }
            Self::SetMidiChannel(channel) => {
                f.debug_tuple("SetMidiChannel").field(channel).finish()
            }
            Self::SetBPM(bpm) => f.debug_tuple("SetBPM").field(bpm).finish(),
            Self::SetOutputConnection(_) => {
                f.write_str("SetOutputConnection(..)")
            }
        }
    }
}
