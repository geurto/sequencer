pub mod euclidean;

pub trait Sequencer {
    #[must_use]
    fn generate_sequence(&self) -> Sequence;
    fn run(&mut self) -> impl std::future::Future<Output = ()> + Send;
}

/// `NoteDuration` is a helper enum to define note durations in musical notation.
/// The discriminants are lengths in sixteenth notes, which is also the
/// sequencer's step resolution.
#[derive(Clone, Copy, Debug)]
pub enum NoteDuration {
    Sixteenth = 1,
    Eighth = 2,
    DottedEight = 3,
    Quarter = 4,
    DottedQuarter = 6,
    Half = 8,
    DottedHalf = 12,
    Whole = 16,
}

impl NoteDuration {
    /// Length of this note in sequencer steps (sixteenth notes).
    #[must_use]
    pub fn steps(self) -> u32 {
        self as u32
    }
}

/// A Note is a MIDI object with pitch, velocity, and duration.
#[derive(Clone, Copy, Debug)]
pub struct Note {
    pub pitch: u8,
    pub velocity: u8,
    pub duration: NoteDuration,
}

impl Note {
    #[must_use]
    pub fn new(pitch: u8, velocity: u8, duration: NoteDuration) -> Self {
        Note {
            pitch,
            velocity,
            duration,
        }
    }

    /// A silent step. Pitch 0 is the rest sentinel throughout the sequencer;
    /// go through this constructor rather than spelling it out, so there is a
    /// single place to change when rests become `Option<Note>`.
    #[must_use]
    pub fn rest() -> Self {
        Note::new(0, 0, NoteDuration::Sixteenth)
    }

    #[must_use]
    pub fn is_rest(&self) -> bool {
        self.pitch == 0
    }
}

/// A Sequence is defined as a vector of Notes, produced by one single Sequencer.
#[derive(Clone, Debug)]
pub struct Sequence {
    pub notes: Vec<Note>,
}

impl Sequence {
    #[must_use]
    pub fn empty() -> Self {
        Sequence { notes: vec![] }
    }

    #[must_use]
    pub fn midi_to_note_name(pitch: u8) -> String {
        const NOTE_NAMES: [&str; 12] = [
            "C.",
            "C#. / Db.",
            "D.",
            "D#. / Eb.",
            "E.",
            "F.",
            "F#. / Gb.",
            "G.",
            "G#. / Ab.",
            "A.",
            "A#. / Bb.",
            "B.",
        ];
        // MIDI note 12 is C0, so note 0 lands in octave -1. Computing this as
        // `pitch - 12` underflows for the rest sentinel and every note below C0.
        let octave = i16::from(pitch) / 12 - 1;
        let note = NOTE_NAMES[(pitch % 12) as usize];

        note.replace('.', &octave.to_string())
    }
}

impl Default for Sequence {
    fn default() -> Self {
        let notes = vec![Note::rest(); 16];
        Sequence { notes }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pitch_to_note() {
        assert_eq!(Sequence::midi_to_note_name(21), "A0");
        assert_eq!(Sequence::midi_to_note_name(26), "D1");
        assert_eq!(Sequence::midi_to_note_name(27), "D#1 / Eb1");
        assert_eq!(Sequence::midi_to_note_name(33), "A1");
        assert_eq!(Sequence::midi_to_note_name(60), "C4");
        assert_eq!(Sequence::midi_to_note_name(69), "A4");
        assert_eq!(Sequence::midi_to_note_name(96), "C7");
        assert_eq!(Sequence::midi_to_note_name(127), "G9");
    }

    /// Notes below C0 used to underflow `pitch - 12` and panic. Pitch 0 is the
    /// rest sentinel, so this was reachable from the GUI.
    #[test]
    fn test_pitch_to_note_below_c0() {
        assert_eq!(Sequence::midi_to_note_name(0), "C-1");
        assert_eq!(Sequence::midi_to_note_name(11), "B-1");
        assert_eq!(Sequence::midi_to_note_name(12), "C0");
    }

    #[test]
    fn test_pitch_to_note_never_panics() {
        for pitch in 0..=u8::MAX {
            let _ = Sequence::midi_to_note_name(pitch);
        }
    }
}
