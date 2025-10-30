use tokio::sync::oneshot;

pub enum MidiCommand {
    GetPorts {
        responder: oneshot::Sender<Vec<String>>,
    },
    SetPort {
        out_port: String,
    },
}
