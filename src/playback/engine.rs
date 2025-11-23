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
    TimedEvent, TICKS_PER_QUARTER_NOTE,
};

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
                        info!(
                            "Engine received new sequence of length {}",
                            seq.events.len()
                        );

                        // Update current_tick, next_event_index, next_note_tick
                        // based on new sequence length
                        let length_ratio =
                            seq.total_ticks / self.sequence.total_ticks;
                        self.current_tick *= length_ratio as f64;
                        self.next_event_index *= length_ratio as usize;
                        self.next_note_tick *= length_ratio as f64;

                        self.sequence = seq;
                    }
                    PlaybackCommand::SetMidiChannel(channel) => {
                        self.midi_channel = channel
                    }
                    PlaybackCommand::SetBPM(bpm) => self.bpm = bpm,
                    PlaybackCommand::SetOutputConnection(conn) => {
                        self.midi_conn = conn
                    }
                }
            }

            // Handle changes in input
            let keys: HashSet<Keycode> =
                device_state.get_keys().into_iter().collect();

            if keys != last_keys {
                let diff: Vec<_> =
                    keys.difference(&last_keys).cloned().collect();

                // Handle playback changes here to save time (vs Engine->StateHandler->Engine)
                for key in diff.clone() {
                    match key {
                        Keycode::Space => self.is_playing = !self.is_playing,
                        Keycode::C => {
                            self.midi_channel = (self.midi_channel + 1) % 16
                        }

                        _ => {}
                    };
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
                    (self.bpm / 60.0) * TICKS_PER_QUARTER_NOTE as f64;
                self.current_tick +=
                    delta_time.as_secs_f64() * ticks_per_second;

                // Send NotePlayed to update GUI
                if self.current_tick > self.next_note_tick {
                    if let Err(e) =
                        self.tx_status.send(PlaybackStatus::NotePlayed(
                            self.current_tick as usize
                                / (TICKS_PER_QUARTER_NOTE as usize / 4),
                        ))
                    {
                        error!("Error sending PlaybackStatus: {e}");
                    }
                    self.next_note_tick += TICKS_PER_QUARTER_NOTE as f64 / 4.;
                }

                // Loop sequence
                if self.current_tick >= self.sequence.total_ticks as f64 {
                    self.current_tick -= self.sequence.total_ticks as f64;
                    self.next_event_index = 0;
                    self.next_note_tick -= self.sequence.total_ticks as f64;
                }

                if !self.sequence.events.is_empty() {
                    // Process sequence events
                    while let Some(&event) =
                        self.sequence.events.get(self.next_event_index)
                    {
                        if event.tick as f64 <= self.current_tick {
                            self.process_event(&event);
                            self.next_event_index += 1;
                        } else {
                            break;
                        }
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn process_event(&mut self, timed_event: &TimedEvent) {
        const NOTE_ON: u8 = 0x90;
        const NOTE_OFF: u8 = 0x80;

        let message = match timed_event.event {
            MidiEventType::NoteOn { pitch, velocity } => {
                [NOTE_ON | self.midi_channel, pitch, velocity]
            }
            MidiEventType::NoteOff { pitch } => {
                [NOTE_OFF | self.midi_channel, pitch, 0]
            }
        };

        self.midi_conn
            .send(&message)
            .unwrap_or_else(|e| log::error!("MIDI send error: {}", e));
    }
}
