//! The sequencer's parameters, and the single place they change.
//!
//! One task owns a [`SequencerState`]; everything else sends it
//! [`ControlEvent`]s and reads the [`UiSnapshot`] it publishes. There is no
//! lock and no second copy — which is what lets every loop in the program be
//! event-driven rather than a poll against shared memory.

use log::{debug, info};
use seq_core::{Pattern, euclid};
use seq_ui::{ControlEvent, Slot, SlotSnapshot, UiSnapshot, snapshot};

pub const MAX_STEPS: usize = 16;
pub const MIN_PITCH: u8 = 20;
pub const MAX_PITCH: u8 = 108;

pub const MIN_BPM: f64 = 20.0;
pub const MAX_BPM: f64 = 300.0;

/// Velocity of a struck pulse. The mixer scales this by the crossfade ratio.
const PULSE_VELOCITY: u8 = 100;

/// One slot's Euclidean parameters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlotState {
    pub steps: usize,
    pub pulses: usize,
    pub phase: usize,
    pub pitch: u8,
}

impl SlotState {
    #[must_use]
    pub fn new() -> Self {
        SlotState {
            steps: MAX_STEPS,
            // Four on the floor, so the sequencer makes a sound on startup
            // rather than waiting for the user to find the pulse keys.
            pulses: 4,
            phase: 0,
            pitch: 60,
        }
    }

    pub fn change_steps(&mut self, delta: i8) {
        self.steps = add_clamped(self.steps, delta, 1, MAX_STEPS);
        info!("Steps: {}", self.steps);
    }

    pub fn change_pulses(&mut self, delta: i8) {
        self.pulses = add_clamped(self.pulses, delta, 0, MAX_STEPS);
        info!("Pulses: {}", self.pulses);
    }

    /// Rotate the pattern; wraps in both directions.
    pub fn change_phase(&mut self, delta: i8) {
        let steps = i32::try_from(self.steps).unwrap_or(i32::MAX).max(1);
        let phase = i32::try_from(self.phase).unwrap_or(0);
        // rem_euclid rather than %, so a negative delta wraps to the far end
        // instead of going negative.
        let wrapped = (phase + i32::from(delta)).rem_euclid(steps);
        self.phase = usize::try_from(wrapped).unwrap_or(0);
        info!("Phase: {}", self.phase);
    }

    pub fn change_pitch(&mut self, delta: i8) {
        // Widen before adding: `self.pitch as i8` wraps for any pitch above
        // 127, and the addition itself can overflow i8.
        let pitch = i16::from(self.pitch) + i16::from(delta);
        let pitch = pitch.clamp(i16::from(MIN_PITCH), i16::from(MAX_PITCH));
        // Infallible: clamped between two u8 bounds on the line above.
        self.pitch = u8::try_from(pitch).unwrap_or(MIN_PITCH);
        info!("Pitch: {}", self.pitch);
    }

    /// The pattern these parameters produce.
    #[must_use]
    pub fn pattern(&self) -> Pattern {
        euclid::pattern(
            self.steps,
            self.pulses,
            self.phase,
            self.pitch,
            PULSE_VELOCITY,
        )
    }
}

impl Default for SlotState {
    fn default() -> Self {
        Self::new()
    }
}

/// `value + delta`, clamped to `min..=max`, without under- or overflow.
fn add_clamped(value: usize, delta: i8, min: usize, max: usize) -> usize {
    let value = i64::try_from(value).unwrap_or(i64::MAX);
    let target = value + i64::from(delta);
    let min = i64::try_from(min).unwrap_or(i64::MAX);
    let max = i64::try_from(max).unwrap_or(i64::MAX);
    usize::try_from(target.clamp(min, max)).unwrap_or(0)
}

/// Everything the sequencer knows, owned by exactly one task.
#[derive(Clone, Debug)]
pub struct SequencerState {
    pub is_playing: bool,
    pub bpm: f64,
    pub midi_channel: u8,
    pub active: Slot,
    pub step_index: usize,
    pub slots: [SlotState; 2],
    /// Crossfade between the slots: `0.0` fully left, `1.0` fully right.
    pub mix_ratio: f32,
}

/// Deriving `Default` would give `bpm: 0.0`, which stalls the playback clock.
impl Default for SequencerState {
    fn default() -> Self {
        Self::new(120.0)
    }
}

impl SequencerState {
    #[must_use]
    pub fn new(bpm: f64) -> Self {
        SequencerState {
            is_playing: false,
            bpm: bpm.clamp(MIN_BPM, MAX_BPM),
            midi_channel: 0,
            active: Slot::Left,
            step_index: 0,
            slots: [SlotState::new(); 2],
            mix_ratio: 0.5,
        }
    }

    /// Apply one user intention. The single mutation entry point: every input
    /// surface — desktop keys today, encoders and MIDI CC later — reduces to
    /// [`ControlEvent`] before touching the state.
    pub fn apply(&mut self, event: ControlEvent) {
        match event {
            ControlEvent::TogglePlay => {
                self.is_playing = !self.is_playing;
                if self.is_playing {
                    info!("Resumed playback!");
                } else {
                    info!("Paused playback!");
                }
            }
            ControlEvent::NextSlot => {
                self.active = self.active.other();
                info!("Switched sequencer to {:?}", self.active);
            }
            ControlEvent::CycleMidiChannel => {
                self.midi_channel = (self.midi_channel + 1) % 16;
                info!("Changing MIDI channel to {}", self.midi_channel + 1);
            }
            ControlEvent::Bpm(delta) => {
                self.bpm =
                    (self.bpm + f64::from(delta)).clamp(MIN_BPM, MAX_BPM);
                info!("BPM: {}", self.bpm);
            }
            ControlEvent::Steps(delta) => self.active_mut().change_steps(delta),
            ControlEvent::Pulses(delta) => {
                self.active_mut().change_pulses(delta);
            }
            ControlEvent::Phase(delta) => self.active_mut().change_phase(delta),
            ControlEvent::Pitch(delta) => self.active_mut().change_pitch(delta),
            ControlEvent::Mix(delta) => {
                self.mix_ratio =
                    (self.mix_ratio + f32::from(delta) * 0.01).clamp(0.0, 1.0);
                debug!("Mixer ratio changed to {}", self.mix_ratio);
            }
        }
    }

    fn active_mut(&mut self) -> &mut SlotState {
        &mut self.slots[self.active.index()]
    }

    /// The frame the UI renders. Everything display-related reduces to this
    /// flat, integer-only value at the seam.
    #[must_use]
    pub fn ui_snapshot(&self) -> UiSnapshot {
        UiSnapshot {
            playing: self.is_playing,
            bpm_milli: bpm_to_milli(self.bpm),
            midi_channel: self.midi_channel,
            active: self.active,
            step_index: u8::try_from(self.step_index).unwrap_or(0),
            slots: [
                slot_snapshot(&self.slots[0]),
                slot_snapshot(&self.slots[1]),
            ],
            mix: ratio_to_mix(self.mix_ratio),
        }
    }
}

/// The transport keeps tempo in integer milli-BPM; this state deals in `f64`.
#[must_use]
pub fn bpm_to_milli(bpm: f64) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to a u32-representable, non-negative range first"
    )]
    {
        (bpm.clamp(0.0, 4_000_000.0) * 1000.0).round() as u32
    }
}

fn ratio_to_mix(ratio: f32) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=1 and scaled to 0..=255 before the cast"
    )]
    {
        (ratio.clamp(0.0, 1.0) * 255.0).round() as u8
    }
}

fn slot_snapshot(state: &SlotState) -> SlotSnapshot {
    SlotSnapshot {
        steps: u8::try_from(state.steps).unwrap_or(u8::MAX),
        pulses: u8::try_from(state.pulses).unwrap_or(u8::MAX),
        phase: u8::try_from(state.phase).unwrap_or(u8::MAX),
        pitch: state.pitch,
        hits: snapshot::hits(&state.pattern()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_steps_and_pulses_are_clamped() {
        let mut state = SlotState::new();

        state.change_steps(i8::MAX);
        assert_eq!(state.steps, MAX_STEPS);
        state.change_steps(i8::MIN);
        assert_eq!(state.steps, 1, "steps must not reach zero");

        state.change_pulses(i8::MAX);
        assert_eq!(state.pulses, MAX_STEPS);
        state.change_pulses(i8::MIN);
        assert_eq!(state.pulses, 0);
    }

    #[test]
    fn test_phase_wraps_in_both_directions() {
        let mut state = SlotState::new();
        state.steps = 8;
        state.phase = 0;

        state.change_phase(-1);
        assert_eq!(state.phase, 7, "decreasing past zero should wrap");

        state.change_phase(1);
        assert_eq!(state.phase, 0, "increasing past the end should wrap");

        state.change_phase(-17);
        assert_eq!(state.phase, 7, "multi-step deltas wrap too");
    }

    #[test]
    fn test_pitch_change_never_wraps() {
        for start in MIN_PITCH..=MAX_PITCH {
            for delta in [i8::MIN, -12, -1, 1, 12, i8::MAX] {
                let mut state = SlotState::new();
                state.pitch = start;
                state.change_pitch(delta);
                assert!(
                    (MIN_PITCH..=MAX_PITCH).contains(&state.pitch),
                    "pitch {start} + {delta} left range at {}",
                    state.pitch
                );
            }
        }
    }

    #[test]
    fn test_default_bpm_does_not_stall_the_clock() {
        assert!(SequencerState::default().bpm >= MIN_BPM);
    }

    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the clamp returns the bound itself, so the comparison is exact"
    )]
    fn test_bpm_is_clamped() {
        let mut state = SequencerState::new(MAX_BPM);
        state.apply(ControlEvent::Bpm(1));
        assert_eq!(state.bpm, MAX_BPM);

        let mut state = SequencerState::new(MIN_BPM);
        state.apply(ControlEvent::Bpm(-1));
        assert_eq!(state.bpm, MIN_BPM);
    }

    #[test]
    fn test_toggle_play_toggles() {
        let mut state = SequencerState::default();
        state.apply(ControlEvent::TogglePlay);
        assert!(state.is_playing);
        state.apply(ControlEvent::TogglePlay);
        assert!(!state.is_playing);
    }

    #[test]
    fn test_slot_events_hit_the_active_slot_only() {
        let mut state = SequencerState::default();
        let right_before = state.slots[1];

        state.apply(ControlEvent::Pitch(12));
        assert_eq!(state.slots[0].pitch, 72);
        assert_eq!(state.slots[1], right_before);

        state.apply(ControlEvent::NextSlot);
        state.apply(ControlEvent::Steps(-1));
        assert_eq!(state.slots[1].steps, 15);
        assert_eq!(state.slots[0].steps, 16);
    }

    #[test]
    fn test_midi_channel_cycles_through_sixteen() {
        let mut state = SequencerState::default();
        for expected in (1..16).chain([0]) {
            state.apply(ControlEvent::CycleMidiChannel);
            assert_eq!(state.midi_channel, expected);
        }
    }

    #[test]
    fn test_mix_deltas_are_hundredths() {
        let mut state = SequencerState::default();
        state.apply(ControlEvent::Mix(5));
        assert!((state.mix_ratio - 0.55).abs() < 1e-6);

        for _ in 0..20 {
            state.apply(ControlEvent::Mix(5));
        }
        assert!((state.mix_ratio - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_snapshot_reflects_the_state() {
        let mut state = SequencerState::default();
        state.apply(ControlEvent::TogglePlay);
        state.apply(ControlEvent::Bpm(5));
        state.step_index = 7;

        let snapshot = state.ui_snapshot();

        assert!(snapshot.playing);
        assert_eq!(snapshot.bpm_milli, 125_000);
        assert_eq!(snapshot.step_index, 7);
        assert_eq!(snapshot.active_slot().steps, 16);
        assert_eq!(snapshot.mix, 128);
    }

    /// The snapshot's hit mask must agree with the generator that feeds the
    /// mixer — the display and the audio must not diverge.
    #[test]
    fn test_snapshot_hits_match_generation() {
        let state = SequencerState::default();
        let snapshot = state.ui_snapshot();
        let pattern = state.slots[0].pattern();

        for index in 0..16 {
            assert_eq!(
                snapshot.slots[0].hit(index),
                pattern.hit(index),
                "step {index} disagrees"
            );
        }
    }

    #[test]
    fn test_bpm_to_milli_conversion() {
        assert_eq!(bpm_to_milli(120.5), 120_500);
        assert_eq!(bpm_to_milli(0.0), 0);
        assert_eq!(bpm_to_milli(-10.0), 0, "negative tempo clamps to zero");
    }
}
