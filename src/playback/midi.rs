use midir::MidiOutputConnection;
use seq_core::{MidiSink, SendError};
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum MidiCommand {
    GetPorts {
        responder: oneshot::Sender<Vec<String>>,
    },
    SetPort {
        out_port: String,
    },
}

/// A midir-backed [`MidiSink`].
///
/// A newtype rather than `impl MidiSink for MidiOutputConnection`: both the
/// trait and the type are foreign to this crate, so a direct impl would
/// violate the orphan rule.
pub struct MidirSink(pub MidiOutputConnection);

impl std::fmt::Debug for MidirSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MidirSink(..)")
    }
}

impl MidiSink for MidirSink {
    fn send(&mut self, message: &[u8]) -> Result<(), SendError> {
        self.0.send(message).map_err(|e| match e {
            midir::SendError::InvalidData(msg) => SendError::InvalidData(msg),
            midir::SendError::Other(msg) => SendError::Other(msg),
        })
    }
}

pub mod midi_utils {
    use anyhow::{Error, anyhow};
    use midir::{MidiOutput, MidiOutputConnection};

    pub fn create_connection(
        port_name: &str,
    ) -> Result<MidiOutputConnection, Error> {
        let midi_out = MidiOutput::new("Generative Sequencer MIDI Out")?;
        match midi_out.find_port_by_id(port_name) {
            Some(midi_port) => midi_out
                .connect(&midi_port, "gen-seq")
                .map_err(|e| anyhow!("Failed to connect to MIDI output: {e}")),
            None => Err(anyhow!(
                "Unable to find MIDI output port with name {port_name}"
            )),
        }
    }

    pub fn list_ports() -> Result<Vec<String>, Error> {
        let midi_out = MidiOutput::new("MIDI Out")?;
        Ok(midi_out
            .ports()
            .iter()
            .map(midir::MidiOutputPort::id)
            .collect::<Vec<_>>())
    }
}
