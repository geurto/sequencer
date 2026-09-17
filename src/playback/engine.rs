use device_query::{DeviceQuery, DeviceState, Keycode};
use log::{error, info};
use midir::MidiOutputConnection;
use std::collections::HashSet;
use std::{
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::playback::state::{
    MidiEventType, PlaybackCommand, PlaybackStatus, PolyphonicSequence,
    TimedEvent, TICKS_PER_STEP,
};

const NOTE_ON: u8 = 0x90;
const NOTE_OFF: u8 = 0x80;
const CONTROL_CHANGE: u8 = 0xB0;
const CC_ALL_SOUND_OFF: u8 = 120;
const CC_ALL_NOTES_OFF: u8 = 123;

const MIDI_CHANNELS: u8 = 16;
const MAX_MIDI_PITCH: u8 = 127;

pub struct PlaybackEngine {
    rx_command: Receiver<PlaybackCommand>,
    tx_status: UnboundedSender<PlaybackStatus>,
    midi_conn: MidiOutputConnection,

    is_playing: bool,
    sequence: PolyphonicSequence,
    next_event_index: usize,
    current_tick: f64,
    next_note_tick: f64,
    bpm: f64,
    midi_channel: u8,

    /// Bit `c` of `sounding[p]` is set while pitch `p` is held on channel `c`.
    ///
    /// Tracked so that notes can be released *explicitly*: CC 123 is advisory
    /// and some synths ignore it, and a mid-note channel switch would otherwise
    /// orphan whatever is still held on the previous channel.
    sounding: [u16; 128],

    last_update_time: Instant,
}

impl PlaybackEngine {
    pub fn new(
        rx_command: Receiver<PlaybackCommand>,
        tx_status: UnboundedSender<PlaybackStatus>,
        midi_conn: MidiOutputConnection,
    ) -> Self {
        Self {
            rx_command,
            tx_status,
            midi_conn,

            is_playing: false,
            sequence: PolyphonicSequence::default(),
            next_event_index: 0,
            current_tick: 0.,
            next_note_tick: 0.,
            bpm: 120.,
            midi_channel: 0,

            sounding: [0; 128],

            last_update_time: Instant::now(),
        }
    }

    pub fn run(mut self) {
        info!("Starting synchronous playback engine");
        self.last_update_time = Instant::now();

        let device_state = DeviceState::new();
        let mut last_keys = HashSet::new();

        loop {
            // Handle commands
            while let Ok(cmd) = self.rx_command.try_recv() {
                match cmd {
                    PlaybackCommand::LoadSequence(seq) => {
                        self.load_sequence(seq)
                    }
                    PlaybackCommand::SetMidiChannel(channel) => {
                        let channel = channel % MIDI_CHANNELS;
                        if channel != self.midi_channel {
                            // Release on the old channel first, or those notes
                            // can never be addressed again.
                            self.release_all_notes();
                            self.midi_channel = channel;
                        }
                    }
                    PlaybackCommand::SetBPM(bpm) => self.bpm = bpm,
                    PlaybackCommand::SetOutputConnection(conn) => {
                        self.release_all_notes();
                        self.midi_conn = conn;
                    }
                }
            }

            // Handle changes in input
            let keys: HashSet<Keycode> =
                device_state.get_keys().into_iter().collect();

            if keys != last_keys {
                let diff: Vec<_> =
                    keys.difference(&last_keys).copied().collect();

                // Handle playback changes here to save time (vs Engine->StateHandler->Engine)
                for key in &diff {
                    if *key == Keycode::Space {
                        self.is_playing = !self.is_playing;
                        if !self.is_playing {
                            // Otherwise whatever was mid-note stays held for as
                            // long as playback is paused.
                            self.release_all_notes();
                        }
                    }
                }
                if let Err(e) =
                    self.tx_status.send(PlaybackStatus::InputChanged(diff))
                {
                    error!(
                        "Error sending input changes to PlaybackHandler: {e}"
                    );
                }

                last_keys = keys;
            }

            // Advance play position
            let now = Instant::now();
            let delta_time = now.duration_since(self.last_update_time);
            self.last_update_time = now;

            if self.is_playing {
                let ticks_per_second =
                    (self.bpm / 60.0) * f64::from(TICKS_PER_STEP * 4);
                self.current_tick +=
                    delta_time.as_secs_f64() * ticks_per_second;

                // Send NotePlayed to update GUI
                if self.current_tick > self.next_note_tick {
                    let step = (self.current_tick / f64::from(TICKS_PER_STEP))
                        as usize;
                    if let Err(e) =
                        self.tx_status.send(PlaybackStatus::NotePlayed(step))
                    {
                        error!("Error sending PlaybackStatus: {e}");
                    }
                    self.next_note_tick += f64::from(TICKS_PER_STEP);
                }

                // Loop sequence
                let total_ticks = f64::from(self.sequence.total_ticks());
                if self.current_tick >= total_ticks {
                    self.current_tick -= total_ticks;
                    self.next_event_index = 0;
                    self.next_note_tick -= total_ticks;
                }

                // Process sequence events
                while let Some(&event) =
                    self.sequence.events().get(self.next_event_index)
                {
                    if f64::from(event.tick) <= self.current_tick {
                        self.process_event(&event);
                        self.next_event_index += 1;
                    } else {
                        break;
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn load_sequence(&mut self, seq: PolyphonicSequence) {
        info!(
            "Engine received new sequence of length {}",
            seq.events().len()
        );

        self.release_all_notes();

        if self.sequence.total_ticks() > 0 && seq.total_ticks() > 0 {
            let length_ratio = f64::from(seq.total_ticks())
                / f64::from(self.sequence.total_ticks());

            self.current_tick *= length_ratio;
            self.next_event_index = 0;
            while let Some(event) = seq.events().get(self.next_event_index) {
                if f64::from(event.tick) < self.current_tick {
                    self.next_event_index += 1;
                } else {
                    break;
                }
            }
            let current_step =
                (self.current_tick / f64::from(TICKS_PER_STEP)).ceil();
            self.next_note_tick = current_step * f64::from(TICKS_PER_STEP);
        } else {
            self.current_tick = 0.0;
            self.next_event_index = 0;
            self.next_note_tick = 0.0;
        }

        self.sequence = seq;
    }

    fn process_event(&mut self, timed_event: &TimedEvent) {
        let channel = self.midi_channel;
        let message = match timed_event.event {
            MidiEventType::NoteOn { pitch, velocity } => {
                let pitch = pitch.min(MAX_MIDI_PITCH);
                self.sounding[pitch as usize] |= 1 << channel;
                [NOTE_ON | channel, pitch, velocity.min(MAX_MIDI_PITCH)]
            }
            MidiEventType::NoteOff { pitch } => {
                let pitch = pitch.min(MAX_MIDI_PITCH);
                self.sounding[pitch as usize] &= !(1 << channel);
                [NOTE_OFF | channel, pitch, 0]
            }
        };

        self.send(&message);
    }

    /// Explicitly release every note this engine has started and not yet
    /// stopped, on whichever channel it was started on.
    fn release_all_notes(&mut self) {
        for pitch in 0..self.sounding.len() {
            let held = self.sounding[pitch];
            if held == 0 {
                continue;
            }
            for channel in 0..MIDI_CHANNELS {
                if held & (1 << channel) != 0 {
                    self.send(&[NOTE_OFF | channel, pitch as u8, 0]);
                }
            }
            self.sounding[pitch] = 0;
        }

        // Belt and braces for anything the engine did not start itself (e.g. a
        // sequence swapped out underneath a synth that missed a Note-Off).
        let channel = self.midi_channel;
        self.send(&[CONTROL_CHANGE | channel, CC_ALL_NOTES_OFF, 0]);
        self.send(&[CONTROL_CHANGE | channel, CC_ALL_SOUND_OFF, 0]);
    }

    fn send(&mut self, message: &[u8]) {
        if let Err(e) = self.midi_conn.send(message) {
            error!("MIDI send error: {e}");
        }
    }
}
