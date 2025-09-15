use log::{debug, info};
use midir::MidiOutputConnection;
use std::{
    sync::mpsc::{Receiver, Sender},
    time::{Duration, Instant},
};

use crate::playback::state::MidiEventType;

use super::state::{PlaybackCommand, PlaybackStatus, PolyphonicSequence, TimedEvent};

const TICKS_PER_QUARTER_NOTE: u32 = 480;

pub struct PlaybackEngine {
    rx_command: Receiver<PlaybackCommand>,
    tx_status: Sender<PlaybackStatus>,
    midi_conn: MidiOutputConnection,

    is_playing: bool,
    sequence: PolyphonicSequence,
    next_event_index: usize,
    current_tick: f64,
    bpm: f64,

    last_update_time: Instant,
}

impl PlaybackEngine {
    pub fn new(
        rx_command: Receiver<PlaybackCommand>,
        tx_status: Sender<PlaybackStatus>,
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
            bpm: 0.,

            last_update_time: Instant::now(),
        }
    }

    pub fn run(mut self) {
        info!("Starting synchronous playback engine");
        self.last_update_time = Instant::now();

        loop {
            // Handle commands
            while let Ok(cmd) = self.rx_command.try_recv() {
                match cmd {
                    PlaybackCommand::Play => self.is_playing = true,
                    PlaybackCommand::Stop => self.is_playing = false,
                    PlaybackCommand::LoadSequence(seq) => {
                        debug!(
                            "Engine received new sequence of length {}",
                            seq.events.len()
                        );
                        self.sequence = seq;
                    }
                    PlaybackCommand::SetMidiChannel(_) => {}
                    PlaybackCommand::SetBPM(bpm) => self.bpm = bpm,
                }
            }

            // Advance play position
            let now = Instant::now();
            let delta_time = now.duration_since(self.last_update_time);
            self.last_update_time = now;

            if self.is_playing && !self.sequence.events.is_empty() {
                let ticks_per_second = (self.bpm / 60.0) * TICKS_PER_QUARTER_NOTE as f64;
                self.current_tick += delta_time.as_secs_f64() * ticks_per_second;

                // Loop sequence
                if self.current_tick >= self.sequence.total_ticks as f64 {
                    self.current_tick -= self.sequence.total_ticks as f64;
                    self.next_event_index = 0;
                }

                // Process sequence events
                while let Some(&event) = self.sequence.events.get(self.next_event_index) {
                    if event.tick as f64 <= self.current_tick {
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

    fn process_event(&mut self, timed_event: &TimedEvent) {
        const NOTE_ON: u8 = 0x90;
        const NOTE_OFF: u8 = 0x80;

        let message = match timed_event.event {
            MidiEventType::NoteOn {
                pitch,
                velocity,
                channel,
            } => [NOTE_ON | channel, pitch, velocity],
            MidiEventType::NoteOff { pitch, channel } => [NOTE_OFF | channel, pitch, 0],
        };

        self.midi_conn
            .send(&message)
            .unwrap_or_else(|e| log::error!("MIDI send error: {}", e));
    }
}
