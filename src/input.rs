use anyhow::Result;
use device_query::{DeviceQuery, DeviceState, Keycode};
use log::info;
use std::collections::HashSet;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::sequencers::euclidean::state::{EuclideanSequencerInput, EuclideanSequencerState};
use crate::sequencers::mixer::state::{MixerInput, MixerState};
use crate::state::{ActiveSequencer, SharedState};

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
    state: Arc<RwLock<SharedState>>,
    tx_left: broadcast::Sender<EuclideanSequencerState>,
    tx_right: broadcast::Sender<EuclideanSequencerState>,
    tx_mixer: broadcast::Sender<MixerState>,
) -> Result<()> {
    let mut last_keys = HashSet::new();

    while let Some(keys) = rx.recv().await {
        let diff: Vec<_> = keys.difference(&last_keys).cloned().collect();

        if !diff.is_empty() {
            let mut w_state = state.write().await;
            let mut sequencer_config = match w_state.active_sequencer {
                ActiveSequencer::Left => w_state.left_state.clone(),
                ActiveSequencer::Right => w_state.right_state.clone(),
            };
            for key in diff {
                match key {
                    Keycode::Space => w_state.playing = !w_state.playing,
                    Keycode::C => {
                        w_state.change_midi_channel();
                        info!("Changing MIDI channel to {}", w_state.midi_channel + 1)
                    }
                    Keycode::R => w_state.mixer_state.increase_ratio(),
                    Keycode::F => w_state.mixer_state.decrease_ratio(),
                    Keycode::Up => sequencer_config.increase_steps(),
                    Keycode::Down => sequencer_config.decrease_steps(),
                    Keycode::Right => sequencer_config.increase_pulses(),
                    Keycode::Left => sequencer_config.decrease_pulses(),
                    Keycode::W => sequencer_config.change_pitch(1),
                    Keycode::S => sequencer_config.change_pitch(-1),
                    Keycode::D => sequencer_config.change_pitch(12),
                    Keycode::A => sequencer_config.change_pitch(-12),
                    _ => {}
                };
            }

            tx_left.send(w_state.left_state.clone())?;
            tx_right.send(w_state.right_state.clone())?;
            tx_mixer.send(w_state.mixer_state.clone())?;
        }

        last_keys = keys;
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    Ok(())
}
