//! Crossfade two patterns into one playable event list, as a pure function.

use log::warn;
use rand::Rng;

use crate::event::{
    EventVec, MidiEventType, PolyphonicSequence, TICKS_PER_STEP, TimedEvent,
};
use crate::note::{MAX_VOICES, Pattern, Voice};

/// Random velocity spread applied to every emitted note, so repeated steps do
/// not sound mechanical.
const VELOCITY_JITTER: i16 = 8;

/// Voices that crossfade below this velocity are dropped entirely, so a ratio
/// at either extreme really does silence the opposite sequencer.
const MIN_AUDIBLE_VELOCITY: u8 = 8;

const MAX_VELOCITY: u8 = 127;

/// Longest mixed result, in steps. Two patterns of coprime lengths expand to
/// the lcm of their lengths; anything past this bound is a bug upstream, and
/// building it would fill the event list with a loop nobody asked for.
const MAX_MIXED_STEPS: usize = 4096;

/// Fold two patterns into one polyphonic sequence.
///
/// The ratio is an equal-power crossfade between the two patterns: `0.0` is
/// fully left, `1.0` is fully right, `0.5` sounds both. Patterns of different
/// lengths are cycled against each other over the least common multiple of
/// their lengths, so polyrhythms play out in full.
///
/// The randomness source only humanises velocities; pass a seeded RNG for
/// reproducible output.
#[must_use]
pub fn mix<R: Rng>(
    left: &Pattern,
    right: &Pattern,
    ratio: f32,
    rng: &mut R,
) -> PolyphonicSequence {
    let len_a = left.len();
    let len_b = right.len();

    if len_a == 0 || len_b == 0 {
        warn!(
            "Mixer received an empty pattern (lengths {len_a} and {len_b}); emitting silence"
        );
        return PolyphonicSequence::default();
    }

    let steps = lcm(len_a, len_b);
    let Some(total_ticks) = (steps <= MAX_MIXED_STEPS)
        .then_some(steps)
        .and_then(|steps| u32::try_from(steps).ok())
        .map(|steps| steps * TICKS_PER_STEP)
    else {
        warn!(
            "Patterns of length {len_a} and {len_b} need {steps} steps, past \
             the {MAX_MIXED_STEPS}-step limit; emitting silence"
        );
        return PolyphonicSequence::default();
    };

    let (gain_a, gain_b) = crossfade_gains(ratio);

    let mut events = EventVec::new();
    let mut tick = 0u32;
    for step_index in 0..steps {
        // Fade both sides' voices, collapsing repeated pitches to one note —
        // a synth has one voice per pitch, and stacked Note-Ons would make its
        // release behaviour undefined.
        let mut merged: [Option<(Voice, u8)>; 2 * MAX_VOICES] =
            [None; 2 * MAX_VOICES];

        let step_a = &left.steps()[step_index % len_a];
        let step_b = &right.steps()[step_index % len_b];
        for (voice, gain) in step_a
            .voices()
            .map(|voice| (voice, gain_a))
            .chain(step_b.voices().map(|voice| (voice, gain_b)))
        {
            if let Some(faded) = fade(*voice, gain, rng) {
                merge(&mut merged, faded);
            }
        }

        for &(voice, velocity) in merged.iter().flatten() {
            if push_note(voice, velocity, tick, total_ticks, &mut events)
                .is_err()
            {
                warn!(
                    "Mixed sequence overflowed {} events; emitting silence",
                    events.capacity()
                );
                return PolyphonicSequence::default();
            }
        }

        tick += TICKS_PER_STEP;
    }

    PolyphonicSequence::new(events, total_ticks)
}

fn lcm(a: usize, b: usize) -> usize {
    let mut x = a;
    let mut y = b;
    while y != 0 {
        (x, y) = (y, x % y);
    }
    a / x * b
}

/// Equal-power crossfade gains for `(left, right)`.
///
/// Equal-power rather than linear so that a note does not audibly dip when the
/// ratio is near the centre.
fn crossfade_gains(ratio: f32) -> (f32, f32) {
    let ratio = ratio.clamp(0.0, 1.0);
    (libm::sqrtf(1.0 - ratio), libm::sqrtf(ratio))
}

/// Apply a crossfade gain to a voice, returning `None` for voices that faded
/// below audibility.
fn fade<R: Rng>(voice: Voice, gain: f32, rng: &mut R) -> Option<(Voice, u8)> {
    // Decide audibility from the crossfade alone, before jitter, so that a
    // fully crossfaded-out voice can never be nudged back into audibility.
    let faded = libm::roundf(f32::from(voice.velocity) * gain)
        .clamp(0.0, f32::from(MAX_VELOCITY));
    if faded < f32::from(MIN_AUDIBLE_VELOCITY) {
        return None;
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "rounded and clamped to 0..=127 on the line above, so exact"
    )]
    let base = faded as i16;

    let jitter = rng.random_range(-VELOCITY_JITTER..=VELOCITY_JITTER);
    let velocity = (base + jitter).clamp(1, i16::from(MAX_VELOCITY));

    // Infallible after the clamp; `?` keeps it total without an unwrap.
    Some((voice, u8::try_from(velocity).ok()?))
}

/// Add a faded voice to the step's merge buffer, collapsing onto an existing
/// entry of the same pitch (louder velocity, longer gate win).
fn merge(merged: &mut [Option<(Voice, u8)>], candidate: (Voice, u8)) {
    for slot in merged.iter_mut() {
        match slot {
            Some((existing, velocity))
                if existing.pitch == candidate.0.pitch =>
            {
                existing.gate_ticks =
                    existing.gate_ticks.max(candidate.0.gate_ticks);
                *velocity = (*velocity).max(candidate.1);
                return;
            }
            Some(_) => {}
            None => {
                *slot = Some(candidate);
                return;
            }
        }
    }
    // Both sides full of distinct pitches: the buffer holds 2 × MAX_VOICES
    // entries, so this is unreachable; dropping the voice is still safe.
}

/// Emit the Note-On/Note-Off pair for one voice.
///
/// The release is clamped inside the sequence so it always fires before the
/// loop point (the transport restarts its event cursor when the sequence
/// wraps).
fn push_note(
    voice: Voice,
    velocity: u8,
    tick: u32,
    total_ticks: u32,
    events: &mut EventVec,
) -> Result<(), ()> {
    let off_tick =
        (tick + u32::from(voice.gate_ticks)).min(total_ticks.saturating_sub(1));

    events
        .push(TimedEvent {
            tick,
            event: MidiEventType::NoteOn {
                pitch: voice.pitch,
                velocity,
            },
        })
        .map_err(|_| ())?;
    events
        .push(TimedEvent {
            tick: off_tick,
            event: MidiEventType::NoteOff { pitch: voice.pitch },
        })
        .map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::Step;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;
    use std::collections::HashMap;

    fn rng() -> SmallRng {
        SmallRng::seed_from_u64(0x5EC0)
    }

    /// Build a single-voice pattern from pitches; `0` is a rest.
    pub(super) fn pattern(pitches: &[u8]) -> Pattern {
        let mut result = Pattern::new(pitches.len());
        for (index, &pitch) in pitches.iter().enumerate() {
            if pitch != 0 {
                result.set_step(
                    index,
                    Step::single(Voice {
                        pitch,
                        velocity: 100,
                        gate_ticks: u16::try_from(TICKS_PER_STEP - 1).unwrap(),
                    }),
                );
            }
        }
        result
    }

    fn pitches_in(mixed: &PolyphonicSequence) -> Vec<u8> {
        let mut pitches: Vec<u8> = mixed
            .events()
            .iter()
            .filter_map(|e| match e.event {
                MidiEventType::NoteOn { pitch, .. } => Some(pitch),
                MidiEventType::NoteOff { .. } => None,
            })
            .collect();
        pitches.sort_unstable();
        pitches.dedup();
        pitches
    }

    /// Every Note-On must be released. The gate offset used to be applied twice
    /// — once at the call site and once inside the push helper — which pushed
    /// the final Note-Off past the loop point, where the engine never reached
    /// it.
    #[test]
    fn test_note_ons_and_offs_balance() {
        let mixed = mix(
            &pattern(&[60, 0, 62, 0]),
            &pattern(&[67, 67, 0]),
            0.5,
            &mut rng(),
        );

        let mut held: HashMap<u8, i32> = HashMap::new();
        for event in mixed.events() {
            match event.event {
                MidiEventType::NoteOn { pitch, .. } => {
                    *held.entry(pitch).or_default() += 1;
                }
                MidiEventType::NoteOff { pitch } => {
                    *held.entry(pitch).or_default() -= 1;
                }
            }
        }

        assert!(!mixed.is_empty(), "expected some events");
        assert!(
            held.values().all(|&count| count == 0),
            "unbalanced Note-On/Note-Off counts: {held:?}"
        );
    }

    /// The transport restarts its event cursor when the sequence wraps, so any
    /// event at or past `total_ticks` is unreachable and its note hangs.
    #[test]
    fn test_every_event_falls_inside_the_sequence() {
        let mixed = mix(
            &pattern(&[60, 62, 64, 65]),
            &pattern(&[67, 0, 0]),
            0.5,
            &mut rng(),
        );

        for event in mixed.events() {
            assert!(
                event.tick < mixed.total_ticks(),
                "event at tick {} is outside a {}-tick sequence",
                event.tick,
                mixed.total_ticks()
            );
        }
    }

    /// The transport stops dispatching at the first event in the future, so an
    /// out-of-order list stalls playback rather than merely reordering it.
    #[test]
    fn test_events_are_sorted_by_tick() {
        let mixed = mix(
            &pattern(&[60, 0, 62, 0]),
            &pattern(&[67, 69]),
            0.5,
            &mut rng(),
        );

        assert!(
            mixed.events().windows(2).all(|w| w[0].tick <= w[1].tick),
            "events are not sorted: {:?}",
            mixed.events()
        );
    }

    /// A note must release before the next step begins, or a repeat of the same
    /// pitch is swallowed by its predecessor's release.
    #[test]
    fn test_gate_ends_before_the_next_step() {
        let mixed =
            mix(&pattern(&[60, 60]), &pattern(&[0, 0]), 0.0, &mut rng());

        let ons: Vec<u32> = mixed
            .events()
            .iter()
            .filter(|e| matches!(e.event, MidiEventType::NoteOn { .. }))
            .map(|e| e.tick)
            .collect();
        let offs: Vec<u32> = mixed
            .events()
            .iter()
            .filter(|e| matches!(e.event, MidiEventType::NoteOff { .. }))
            .map(|e| e.tick)
            .collect();

        assert_eq!(ons, vec![0, TICKS_PER_STEP]);
        assert_eq!(offs, vec![TICKS_PER_STEP - 1, 2 * TICKS_PER_STEP - 1]);
    }

    /// `num::abs_sub` was the *positive* difference, not the absolute one, so
    /// ratios at or above 0.7 used to drop coincident notes entirely.
    #[test]
    fn test_no_ratio_produces_silence() {
        for ratio in [0.0, 0.1, 0.25, 0.5, 0.7, 0.75, 0.9, 1.0] {
            let mixed = mix(
                &pattern(&[60, 60]),
                &pattern(&[67, 67]),
                ratio,
                &mut rng(),
            );
            assert!(
                !mixed.is_empty(),
                "mixer ratio {ratio} produced silence where both patterns play"
            );
        }
    }

    /// The ratio is a crossfade: each extreme should isolate one pattern.
    #[test]
    fn test_ratio_extremes_isolate_one_pattern() {
        let left = pattern(&[60, 60]);
        let right = pattern(&[67, 67]);

        assert_eq!(
            pitches_in(&mix(&left, &right, 0.0, &mut rng())),
            vec![60],
            "ratio 0.0 should play only the left pattern"
        );
        assert_eq!(
            pitches_in(&mix(&left, &right, 1.0, &mut rng())),
            vec![67],
            "ratio 1.0 should play only the right pattern"
        );
        assert_eq!(
            pitches_in(&mix(&left, &right, 0.5, &mut rng())),
            vec![60, 67],
            "ratio 0.5 should play both patterns"
        );
    }

    /// Two patterns landing on the same pitch should produce one note, not
    /// two overlapping Note-Ons a synth has to disambiguate.
    #[test]
    fn test_identical_pitches_collapse_to_one_note() {
        let mixed =
            mix(&pattern(&[60, 60]), &pattern(&[60, 60]), 0.5, &mut rng());

        assert_eq!(mixed.events().len(), 4, "expected one note pair per step");
    }

    /// Patterns of coprime lengths must expand to their full polyrhythm.
    #[test]
    fn test_coprime_lengths_expand_to_the_lcm() {
        let mixed =
            mix(&pattern(&[60, 60]), &pattern(&[67; 3]), 0.5, &mut rng());

        assert_eq!(mixed.total_ticks(), 6 * TICKS_PER_STEP);
    }
}

#[cfg(test)]
mod property_tests {
    use super::tests::pattern;
    use super::*;
    use proptest::prelude::*;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;
    use std::collections::BTreeSet;

    /// Patterns of 1..=16 steps; pitch 0 is a rest.
    fn any_pitches() -> impl Strategy<Value = Vec<u8>> {
        proptest::collection::vec(0u8..=127, 1..=16)
    }

    proptest! {
        /// Whatever the two patterns hold and wherever the crossfade sits,
        /// the mixer must not emit a sequence that leaves a note ringing:
        /// every pitch it starts is released before the loop point.
        #[test]
        fn prop_mixed_sequences_release_every_note(
            left in any_pitches(),
            right in any_pitches(),
            ratio in 0.0f32..=1.0,
            seed in any::<u64>(),
        ) {
            let mut rng = SmallRng::seed_from_u64(seed);
            let mixed = mix(&pattern(&left), &pattern(&right), ratio, &mut rng);

            let mut held = BTreeSet::new();
            for event in mixed.events() {
                match event.event {
                    MidiEventType::NoteOn { pitch, .. } => { held.insert(pitch); }
                    MidiEventType::NoteOff { pitch } => { held.remove(&pitch); }
                }
            }

            prop_assert!(held.is_empty(), "pitches left ringing: {:?}", held);
        }

        /// Every event must fall inside the loop. The transport restarts its
        /// cursor on wrap, so anything at or past `total_ticks` is unreachable.
        #[test]
        fn prop_mixed_events_are_sorted_and_in_range(
            left in any_pitches(),
            right in any_pitches(),
            ratio in 0.0f32..=1.0,
            seed in any::<u64>(),
        ) {
            let mut rng = SmallRng::seed_from_u64(seed);
            let mixed = mix(&pattern(&left), &pattern(&right), ratio, &mut rng);

            prop_assert!(
                mixed.events().windows(2).all(|w| w[0].tick <= w[1].tick),
                "events are not sorted by tick"
            );
            for event in mixed.events() {
                prop_assert!(
                    event.tick < mixed.total_ticks().max(1),
                    "event at {} outside a {}-tick sequence",
                    event.tick,
                    mixed.total_ticks()
                );
            }
        }

        /// Velocities must stay inside the MIDI range, and a struck note must
        /// never be emitted silently.
        #[test]
        fn prop_velocities_are_audible_and_in_range(
            left in any_pitches(),
            right in any_pitches(),
            ratio in 0.0f32..=1.0,
            seed in any::<u64>(),
        ) {
            let mut rng = SmallRng::seed_from_u64(seed);
            let mixed = mix(&pattern(&left), &pattern(&right), ratio, &mut rng);

            for event in mixed.events() {
                if let MidiEventType::NoteOn { velocity, .. } = event.event {
                    prop_assert!(
                        (1..=127).contains(&velocity),
                        "velocity {velocity} outside 1..=127"
                    );
                }
            }
        }
    }
}
