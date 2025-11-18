use device_query::Keycode;
use midir::MidiOutputConnection;

pub const TICKS_PER_QUARTER_NOTE: u32 = 480;

// MIDI event to send over midir
#[derive(Debug, Clone, Copy)]
pub enum MidiEventType {
    NoteOn { pitch: u8, velocity: u8 },
    NoteOff { pitch: u8 },
}

// Link an absolute timestamp to the MIDI event
#[derive(Debug, Clone, Copy)]
pub struct TimedEvent {
    pub tick: u32,
    pub event: MidiEventType,
}

// Sequence is just a collection of timed events
#[derive(Debug, Clone, Default)]
pub struct PolyphonicSequence {
    pub events: Vec<TimedEvent>, // sort this by tick
    pub total_ticks: u32,
}

// Commands FROM the UI/input TO the playback thread
pub enum PlaybackCommand {
    LoadSequence(PolyphonicSequence),
    SetMidiChannel(u8),
    SetBPM(f64),
    SetOutputConnection(MidiOutputConnection),
}

// Data FROM the playback thread TO the UI
#[derive(Debug, Clone)]
pub enum PlaybackStatus {
    NotePlayed(usize),
    InputChanged(Vec<Keycode>),
}
