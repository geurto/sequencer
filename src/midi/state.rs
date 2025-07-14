use tokio::sync::oneshot;

use crate::note::Note;

// TODO set up a tokio channel that sends MidiCommands to a MidiHandler. Gui and PlaybackHandler
// are TX, MidiHandler is RX.
pub enum MidiCommand {
    PlayNotes {
        notes: (Option<Note>, Option<Note>),
    },
    GetPorts {
        responder: oneshot::Sender<Vec<String>>,
    },
    SetPort {
        port_index: usize,
    },
}
