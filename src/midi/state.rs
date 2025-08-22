use tokio::sync::oneshot;

use crate::note::Note;

pub enum MidiCommand {
    PlayNotes {
        notes: (Option<Note>, Option<Note>),
        channel: u8,
    },
    GetPorts {
        responder: oneshot::Sender<Vec<String>>,
    },
    SetPort {
        out_port: String,
    },
}
