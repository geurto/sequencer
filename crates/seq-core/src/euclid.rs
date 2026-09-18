//! The Euclidean pattern generator, as a pure function.

use crate::event::TICKS_PER_STEP;
use crate::note::{MAX_STEPS, Pattern, Step, Voice};

/// Gate for a generated pulse: one tick short of a full step, so a pulse on
/// the next step is not swallowed by this one's release.
#[expect(
    clippy::cast_possible_truncation,
    reason = "TICKS_PER_STEP is 120, comfortably within u16"
)]
const STEP_GATE_TICKS: u16 = (TICKS_PER_STEP - 1) as u16;

/// Distribute `pulses` onsets as evenly as possible over `steps` steps,
/// rotated by `phase`.
///
/// This is the Bresenham approximation the sequencer has always used, not the
/// canonical Euclidean rhythm: E(3,8) comes out as `x.x..x..` (onsets at
/// {0, 2, 5}) rather than the conventional `x..x..x.`.
///
/// `steps` is clamped to `1..=MAX_STEPS`. `pulses` greater than `steps`
/// degenerates exactly as it always has (colliding onsets merge).
#[must_use]
pub fn pattern(
    steps: usize,
    pulses: usize,
    phase: usize,
    pitch: u8,
    velocity: u8,
) -> Pattern {
    let steps = steps.clamp(1, MAX_STEPS);
    // Reduced up front so the additions below cannot overflow, and because
    // more pulses than steps cannot strike more than every step anyway.
    let pulses = pulses.min(MAX_STEPS);
    let phase = phase % steps;
    let mut result = Pattern::new(steps);

    let mut hits = [false; MAX_STEPS];
    for k in 0..pulses {
        hits[(phase + (k * steps) / pulses) % steps] = true;
    }

    let voice = Voice {
        pitch,
        velocity,
        gate_ticks: STEP_GATE_TICKS,
    };
    for (index, &hit) in hits[..steps].iter().enumerate() {
        if hit {
            result.set_step(index, Step::single(voice));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn as_string(steps: usize, pulses: usize, phase: usize) -> String {
        let generated = pattern(steps, pulses, phase, 60, 100);
        (0..generated.len())
            .map(|i| if generated.hit(i) { 'x' } else { '.' })
            .collect()
    }

    /// Note these are the Bresenham approximation the sequencer actually uses,
    /// not canonical Euclidean rhythms: E(3,8) is conventionally `x..x..x.`,
    /// whereas `(i * steps) / pulses` yields onsets at {0, 2, 5}.
    #[test]
    fn test_euclidean_patterns() {
        assert_eq!(as_string(8, 0, 0), "........");
        assert_eq!(as_string(8, 8, 0), "xxxxxxxx");
        assert_eq!(as_string(8, 4, 0), "x.x.x.x.");
        assert_eq!(as_string(8, 3, 0), "x.x..x..");
        assert_eq!(as_string(16, 4, 0), "x...x...x...x...");
    }

    #[test]
    fn test_phase_rotates_the_pattern() {
        assert_eq!(as_string(8, 3, 1), ".x.x..x.");
        assert_eq!(as_string(8, 3, 2), "..x.x..x");
    }

    #[test]
    fn test_pulse_count_is_honoured() {
        for steps in 1..=16 {
            for pulses in 0..=steps {
                let struck = as_string(steps, pulses, 0).matches('x').count();
                assert_eq!(
                    struck, pulses,
                    "steps={steps} pulses={pulses} produced {struck} onsets"
                );
            }
        }
    }

    #[test]
    fn test_generated_voice_carries_pitch_velocity_and_gate() {
        let generated = pattern(4, 1, 0, 72, 90);
        let voice = generated.steps()[0].voices().next().unwrap();

        assert_eq!(voice.pitch, 72);
        assert_eq!(voice.velocity, 90);
        assert_eq!(u32::from(voice.gate_ticks), TICKS_PER_STEP - 1);
    }

    #[test]
    fn test_degenerate_inputs_do_not_panic() {
        let _ = pattern(0, 0, 0, 60, 100);
        let _ = pattern(1, 100, 0, 60, 100);
        let _ = pattern(usize::MAX, 3, usize::MAX - 7, 60, 100);
    }
}
