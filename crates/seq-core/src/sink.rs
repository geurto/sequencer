//! Where MIDI bytes go.
//!
//! The sequencing core writes through [`MidiSink`] rather than to a concrete
//! backend, so that playback can be driven — and asserted on — without any
//! MIDI hardware, and so an embedded build can substitute a UART writer for a
//! desktop MIDI stack.

use core::fmt;

/// Error returned by [`MidiSink::send`].
///
/// Deliberately backend-agnostic and allocation-free: both variants carry
/// `&'static str` rather than a `String`.
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

impl core::error::Error for SendError {}

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

/// Lets an owner hold a sink behind a `Box` without knowing its type — the
/// desktop engine swaps its output at runtime, so it cannot be generic over
/// one.
#[cfg(any(test, feature = "std"))]
impl<T: MidiSink + ?Sized> MidiSink for Box<T> {
    fn send(&mut self, message: &[u8]) -> Result<(), SendError> {
        (**self).send(message)
    }
}

/// A sink that discards everything written to it.
///
/// Lets a sequencer run with no output attached — before a MIDI port is
/// chosen, on a board whose UART is not configured yet — without any of the
/// callers having to special-case a missing device.
#[derive(Clone, Copy, Debug, Default)]
pub struct SilentSink;

impl MidiSink for SilentSink {
    fn send(&mut self, _message: &[u8]) -> Result<(), SendError> {
        Ok(())
    }
}

/// A sink that records what was written to it instead of emitting it.
///
/// Cloning shares the recording, so a test can keep a handle after handing the
/// sink to the code under test:
///
/// ```
/// # // requires the `test-util` feature
/// use seq_core::{MidiSink, RecordingSink};
///
/// let recorder = RecordingSink::new();
/// let mut sink = recorder.clone();
/// sink.send(&[0x90, 60, 100]).unwrap();
///
/// assert_eq!(recorder.messages(), vec![vec![0x90, 60, 100]]);
/// ```
#[cfg(any(test, feature = "test-util"))]
#[derive(Clone, Debug, Default)]
pub struct RecordingSink {
    messages: std::sync::Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
    /// When set, every `send` fails with this error instead of recording.
    failure: Option<SendError>,
}

#[cfg(any(test, feature = "test-util"))]
impl RecordingSink {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A sink whose every write fails, for exercising error paths.
    #[must_use]
    pub fn failing(error: SendError) -> Self {
        Self {
            failure: Some(error),
            ..Self::default()
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

#[cfg(any(test, feature = "test-util"))]
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
