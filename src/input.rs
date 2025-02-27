use anyhow::Result;
use device_query::{DeviceQuery, DeviceState, Keycode};
use log::{error, info};
use std::collections::HashSet;
use std::sync::{Arc, Mutex as SyncMutex};
use std::thread;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};

use crate::gui::{Event, Message};
use crate::sequencers::euclidean::state::EuclideanSequencerInput;
use crate::sequencers::mixer::state::MixerInput;
use crate::state::SharedState;

pub enum Input {
    Bpm(f32),
    TogglePlayback,
    ChangeMidiChannel,
    IncreaseBpm,
    DecreaseBpm,
    Euclidean(EuclideanSequencerInput),
    Mixer(MixerInput),
}

pub fn start_polling(tx: mpsc::Sender<HashSet<Keycode>>) {
    thread::spawn(move || {
        let device_state = DeviceState::new();
        let mut last_keys = HashSet::new();

        loop {
            let keys: HashSet<Keycode> = device_state.get_keys().into_iter().collect();

            if keys != last_keys && tx.blocking_send(keys.clone()).is_err() {
                break;
            }

            last_keys = keys;
            thread::sleep(Duration::from_millis(25));
        }
    });
}

pub async fn run_input_handler(
    mut rx: mpsc::Receiver<HashSet<Keycode>>,
    tx_gui: Arc<SyncMutex<Option<iced::futures::channel::mpsc::Sender<Message>>>>,
    state: Arc<RwLock<SharedState>>,
) -> Result<()> {
    let mut last_keys = HashSet::new();

    while let Some(keys) = rx.recv().await {
        let diff: Vec<_> = keys.difference(&last_keys).cloned().collect();

        if !diff.is_empty() {
            let mut w_state = state.write().await;
            for key in diff {
                match key {
                    Keycode::Space => {
                        w_state.playing = !w_state.playing;

                        match w_state.playing {
                            true => info!("Resumed playback!"),
                            false => info!("Paused playback!"),
                        }
                    }
                    Keycode::C => {
                        w_state.change_midi_channel();
                        info!("Changing MIDI channel to {}", w_state.midi_channel + 1)
                    }
                    Keycode::R => w_state.mixer_state.increase_ratio(),
                    Keycode::F => w_state.mixer_state.decrease_ratio(),
                    Keycode::Up => w_state.increase_steps(),
                    Keycode::Down => w_state.decrease_steps(),
                    Keycode::Right => w_state.increase_pulses(),
                    Keycode::Left => w_state.decrease_pulses(),
                    Keycode::W => w_state.change_pitch(1),
                    Keycode::S => w_state.change_pitch(-1),
                    Keycode::D => w_state.change_pitch(12),
                    Keycode::A => w_state.change_pitch(-12),
                    Keycode::Tab => w_state.switch_active_sequencer(),
                    _ => {}
                };
            }

            drop(w_state);
            let r_state = state.read().await;
            if let Some(mut tx) = tx_gui.lock().unwrap().clone() {
                if let Err(e) =
                    tx.try_send(Message::ReceivedEvent(Event::StateChanged(r_state.clone())))
                {
                    error!("Error sending Message::ReceivedEvent to GUI: {:?}", e);
                }
            }
        }

        last_keys = keys;
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    Ok(())
}
