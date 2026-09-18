//! What the sequencer accepts from any input surface.

/// One user intention, independent of the surface that produced it.
///
/// Parameter changes are *deltas*, not absolute values, because every intended
/// input device is relative: a keyboard key is ±1, a rotary encoder detent is
/// ±1 (or more with acceleration), a MIDI CC arrives as a change. Absolute
/// input surfaces can synthesise deltas against the current [`UiSnapshot`].
///
/// [`UiSnapshot`]: crate::UiSnapshot
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlEvent {
    TogglePlay,
    /// Make the other slot the active one.
    NextSlot,
    /// Advance to the next of the 16 MIDI channels, wrapping.
    CycleMidiChannel,
    /// Tempo change in whole BPM.
    Bpm(i16),
    /// Step-count change for the active slot.
    Steps(i8),
    /// Pulse-count change for the active slot.
    Pulses(i8),
    /// Pattern rotation for the active slot.
    Phase(i8),
    /// Pitch change in semitones for the active slot (an octave is ±12).
    Pitch(i8),
    /// Crossfade change in hundredths of full scale (+5 is five percent
    /// towards the right slot).
    Mix(i8),
}
