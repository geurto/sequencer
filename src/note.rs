use std::fmt::{Debug, Formatter};

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

#[derive(Clone)]
pub struct Sequence {
    pub notes: Vec<Note>,
}

impl Sequence {
    pub fn empty() -> Self {
        Sequence { notes: vec![] }
    }

    fn midi_to_note_name(pitch: u8) -> String {
        let note_names = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        let octave = (pitch / 12) as i8 - 1;
        let note = note_names[(pitch % 12) as usize];
        format!("[{}{}]", note, octave)
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
                Sequence::midi_to_note_name(note.pitch)
            };
            let duration_symbol = Sequence::duration_to_symbol(note.duration, total_duration);
            result.push_str(&format!("{}{}", note_name, duration_symbol));
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
