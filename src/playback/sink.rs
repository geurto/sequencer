//! Where MIDI bytes go.
//!
//! The sequencing core writes through [`MidiSink`] rather than to a concrete
//! backend, so that playback can be driven — and asserted on — without any MIDI
//! hardware, and so that a future embedded build can substitute a UART writer
//! for `midir`.

use std::fmt;
use std::sync::{Arc, Mutex};

use midir::MidiOutputConnection;

/// Error returned by [`MidiSink::send`].
///
/// Mirrors `midir::SendError` but names no backend types, which keeps the trait
/// independent of midir. Both variants carry `&'static str` rather than a
/// `String`, so the type stays allocation-free.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendError {
    InvalidData(&'static str),
    Other(&'static str),
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SendError::InvalidData(msg) => {
                write!(f, "invalid MIDI data: {msg}")
            }
            SendError::Other(msg) => write!(f, "MIDI send failed: {msg}"),
        }
    }
}

impl std::error::Error for SendError {}

impl From<midir::SendError> for SendError {
    fn from(error: midir::SendError) -> Self {
        match error {
            midir::SendError::InvalidData(msg) => SendError::InvalidData(msg),
            midir::SendError::Other(msg) => SendError::Other(msg),
        }
    }
}

/// A destination for raw MIDI messages.
pub trait MidiSink {
    /// Write one complete MIDI message.
    ///
    /// # Errors
    ///
    /// Returns [`SendError`] if the backend rejected the message or the
    /// connection is no longer usable.
    fn send(&mut self, message: &[u8]) -> Result<(), SendError>;
}

impl MidiSink for MidiOutputConnection {
    fn send(&mut self, message: &[u8]) -> Result<(), SendError> {
        // Fully qualified: an inherent `send` of the same name also exists, and
        // `self.send(..)` inside this impl would resolve back to this method.
        MidiOutputConnection::send(self, message).map_err(SendError::from)
    }
}

/// Lets an owner hold a sink behind a `Box` without knowing its type — the
/// engine swaps its output at runtime, so it cannot be generic over one.
impl<T: MidiSink + ?Sized> MidiSink for Box<T> {
    fn send(&mut self, message: &[u8]) -> Result<(), SendError> {
        (**self).send(message)
    }
}

/// A sink that records what was written to it instead of emitting it.
///
/// Cloning shares the recording, so a test can keep a handle after handing the
/// sink to the code under test:
///
/// ```
/// use sequencer::playback::sink::{MidiSink, RecordingSink};
///
/// let recorder = RecordingSink::new();
/// let mut sink = recorder.clone();
/// sink.send(&[0x90, 60, 100]).unwrap();
///
/// assert_eq!(recorder.messages(), vec![vec![0x90, 60, 100]]);
/// ```
#[derive(Clone, Debug, Default)]
pub struct RecordingSink {
    messages: Arc<Mutex<Vec<Vec<u8>>>>,
    /// When set, every `send` fails with this error instead of recording.
    failure: Option<SendError>,
}

impl RecordingSink {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A sink whose every write fails, for exercising error paths.
    #[must_use]
    pub fn failing(error: SendError) -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
            failure: Some(error),
        }
    }

    /// Everything written so far, in order.
    #[must_use]
    pub fn messages(&self) -> Vec<Vec<u8>> {
        self.messages.lock().unwrap().clone()
    }

    /// Drop the recording, keeping the handle valid.
    pub fn clear(&self) {
        self.messages.lock().unwrap().clear();
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.messages.lock().unwrap().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl MidiSink for RecordingSink {
    fn send(&mut self, message: &[u8]) -> Result<(), SendError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        self.messages.lock().unwrap().push(message.to_vec());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clones_share_one_recording() {
        let recorder = RecordingSink::new();
        let mut a = recorder.clone();
        let mut b = recorder.clone();

        a.send(&[0x90, 60, 100]).unwrap();
        b.send(&[0x80, 60, 0]).unwrap();

        assert_eq!(
            recorder.messages(),
            vec![vec![0x90, 60, 100], vec![0x80, 60, 0]]
        );
    }

    #[test]
    fn test_failing_sink_records_nothing() {
        let recorder = RecordingSink::failing(SendError::Other("unplugged"));
        let mut sink = recorder.clone();

        assert_eq!(
            sink.send(&[0x90, 60, 100]),
            Err(SendError::Other("unplugged"))
        );
        assert!(recorder.is_empty());
    }

    #[test]
    fn test_boxed_sink_forwards() {
        let recorder = RecordingSink::new();
        let mut sink: Box<dyn MidiSink> = Box::new(recorder.clone());

        sink.send(&[0x90, 60, 100]).unwrap();

        assert_eq!(recorder.messages(), vec![vec![0x90, 60, 100]]);
    }
}
