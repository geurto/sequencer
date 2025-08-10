use std::fmt::{Debug, Formatter};

/// NoteDuration is a helper enum to define note durations in musical notation. These durations are
/// then converted to seconds in playback.
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

/// A Note is a MIDI object with pitch, velocity, duration, and a channel. Duration here is in seconds.
#[derive(Clone, Copy, Debug)]
pub struct Note {
    pub pitch: u8,
    pub velocity: u8,
    pub duration: f32,
}

impl Note {
    pub fn new(pitch: u8, velocity: u8, note_duration: NoteDuration, bpm: f32) -> Self {
        let duration = match note_duration {
            NoteDuration::Sixteenth => 60000.0 / bpm / 4.0,
            NoteDuration::Eighth => 60000.0 / bpm / 2.0,
            NoteDuration::DottedEight => 60000.0 / bpm / 2.0 * 1.5,
            NoteDuration::Quarter => 60000.0 / bpm,
            NoteDuration::DottedQuarter => 60000.0 / bpm * 1.5,
            NoteDuration::Half => 60000.0 / bpm * 2.0,
            NoteDuration::DottedHalf => 60000.0 / bpm * 2.0 * 1.5,
            NoteDuration::Whole => 60000.0 / bpm * 4.0,
        };

        Note {
            pitch,
            velocity,
            duration,
        }
    }
}

/// A Sequence is defined as a vector of Notes, produced by one single Sequencer.
#[derive(Clone)]
pub struct Sequence {
    pub notes: Vec<Note>,
}

impl Sequence {
    pub fn empty() -> Self {
        Sequence { notes: vec![] }
    }

    pub fn midi_to_note_name(pitch: u8) -> String {
        let note_names = [
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
        let octave = ((pitch - 12) as f32 / 12.).floor();
        let note = note_names[((pitch - 12) % 12) as usize];

        note.replace(".", &format!("{octave}"))
    }

    fn duration_to_symbol(duration: f32, total_duration: f32) -> String {
        let total_dashes = 40;
        let num_dashes = (duration / total_duration * total_dashes as f32).round() as usize;
        "-".repeat(num_dashes)
    }
}

impl Debug for Sequence {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut result = String::new();
        result.push_str("Sequence: ");
        let total_duration = self.notes.iter().map(|note| note.duration).sum::<f32>();

        for note in &self.notes {
            let note_name = if note.pitch == 0 {
                "[r]".to_string()
            } else {
                format!("[{}]", Sequence::midi_to_note_name(note.pitch))
            };
            let duration_symbol = Sequence::duration_to_symbol(note.duration, total_duration);
            result.push_str(&format!("{note_name}{duration_symbol}"));
        }

        result.fmt(f)
    }
}

impl Default for Sequence {
    fn default() -> Self {
        let notes = vec![Note::new(0, 0, NoteDuration::Sixteenth, 120.0); 16];
        Sequence { notes }
    }
}

/// A MixedSequence is the result of mixing two Sequences in the Mixer.
#[derive(Debug)]
pub struct MixedSequence {
    pub notes: Vec<(Option<Note>, Option<Note>)>,
}

impl MixedSequence {
    pub fn new() -> Self {
        Self {
            notes: Vec::default(),
        }
    }

    pub fn push(&mut self, note: (Option<Note>, Option<Note>)) {
        self.notes.push(note);
    }
}

impl Default for MixedSequence {
    fn default() -> Self {
        let notes = vec![
            (
                Some(Note::new(0, 0, NoteDuration::Sixteenth, 120.0)),
                Some(Note::new(0, 0, NoteDuration::Sixteenth, 120.0))
            );
            16
        ];
        MixedSequence { notes }
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
}
