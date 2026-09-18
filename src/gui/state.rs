use iced::futures::{Stream, stream};
use iced::keyboard::{Key, key::Named};
use seq_ui::{ControlEvent, UiSnapshot};
use tokio::sync::watch;

use crate::gui::sequencers::euclidean::Message as EuclideanGuiMessage;

#[derive(Debug, Clone)]
pub enum GuiMessage {
    /// A fresh frame from the sequencer task.
    SnapshotChanged(UiSnapshot),
    /// A user intention, headed back to the sequencer task.
    Control(ControlEvent),
    LeftSequencer(EuclideanGuiMessage),
    RightSequencer(EuclideanGuiMessage),
    MixerRatioChanged(f32),
    RefreshMidiPorts,
    MidiPortsLoaded(Result<Vec<String>, String>),
    MidiPortSelected(String),
    MidiPortSet(String),
    ErrorOccurred(String),
}

/// Frames from the sequencer task, as a stream iced can subscribe to.
///
/// Ends when the sender goes away, which is how the window learns the
/// sequencer has shut down.
pub fn snapshot_stream(
    rx: watch::Receiver<UiSnapshot>,
) -> impl Stream<Item = UiSnapshot> {
    stream::unfold(rx, |mut rx| async move {
        rx.changed().await.ok()?;
        let snapshot = *rx.borrow_and_update();
        Some((snapshot, rx))
    })
}

/// Translate a key press into a user intention.
///
/// This is the whole of the keyboard's involvement: the rest of the program
/// only ever sees [`ControlEvent`]s, so swapping this surface for encoders,
/// MIDI CC or a simulator window touches nothing else.
#[must_use]
pub fn control_for_key(key: &Key) -> Option<ControlEvent> {
    Some(match key {
        Key::Named(Named::Space) => ControlEvent::TogglePlay,
        Key::Named(Named::Tab) => ControlEvent::NextSlot,
        Key::Named(Named::ArrowUp) => ControlEvent::Steps(1),
        Key::Named(Named::ArrowDown) => ControlEvent::Steps(-1),
        Key::Named(Named::ArrowRight) => ControlEvent::Pulses(1),
        Key::Named(Named::ArrowLeft) => ControlEvent::Pulses(-1),
        // Shifted forms are accepted so the bindings survive a stuck Shift.
        Key::Character(character) => match character.as_str() {
            "c" | "C" => ControlEvent::CycleMidiChannel,
            "=" | "+" => ControlEvent::Bpm(1),
            "-" | "_" => ControlEvent::Bpm(-1),
            "r" | "R" => ControlEvent::Mix(5),
            "f" | "F" => ControlEvent::Mix(-5),
            "]" | "}" => ControlEvent::Phase(1),
            "[" | "{" => ControlEvent::Phase(-1),
            "w" | "W" => ControlEvent::Pitch(1),
            "s" | "S" => ControlEvent::Pitch(-1),
            "d" | "D" => ControlEvent::Pitch(12),
            "a" | "A" => ControlEvent::Pitch(-12),
            _ => return None,
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character(text: &str) -> Key {
        Key::Character(text.into())
    }

    /// Every binding the README documents must translate, and nothing else
    /// should.
    #[test]
    fn test_keymap_covers_the_documented_bindings() {
        let bound = [
            (Key::Named(Named::Space), ControlEvent::TogglePlay),
            (Key::Named(Named::Tab), ControlEvent::NextSlot),
            (Key::Named(Named::ArrowUp), ControlEvent::Steps(1)),
            (Key::Named(Named::ArrowDown), ControlEvent::Steps(-1)),
            (Key::Named(Named::ArrowRight), ControlEvent::Pulses(1)),
            (Key::Named(Named::ArrowLeft), ControlEvent::Pulses(-1)),
            (character("c"), ControlEvent::CycleMidiChannel),
            (character("="), ControlEvent::Bpm(1)),
            (character("-"), ControlEvent::Bpm(-1)),
            (character("r"), ControlEvent::Mix(5)),
            (character("f"), ControlEvent::Mix(-5)),
            (character("]"), ControlEvent::Phase(1)),
            (character("["), ControlEvent::Phase(-1)),
            (character("w"), ControlEvent::Pitch(1)),
            (character("s"), ControlEvent::Pitch(-1)),
            (character("d"), ControlEvent::Pitch(12)),
            (character("a"), ControlEvent::Pitch(-12)),
        ];
        for (key, expected) in bound {
            assert_eq!(control_for_key(&key), Some(expected), "{key:?}");
        }

        assert_eq!(control_for_key(&character("q")), None);
        assert_eq!(control_for_key(&Key::Named(Named::Enter)), None);
    }

    #[test]
    fn test_shifted_letters_keep_their_binding() {
        assert_eq!(
            control_for_key(&character("W")),
            Some(ControlEvent::Pitch(1))
        );
        assert_eq!(
            control_for_key(&character("+")),
            Some(ControlEvent::Bpm(1))
        );
    }
}
