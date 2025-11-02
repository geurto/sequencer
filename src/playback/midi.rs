use tokio::sync::oneshot;

pub enum MidiCommand {
    GetPorts {
        responder: oneshot::Sender<Vec<String>>,
    },
    SetPort {
        out_port: String,
    },
}

pub mod midi_utils {
    use anyhow::{anyhow, Error};
    use midir::{MidiOutput, MidiOutputConnection};

    pub fn create_connection(port_name: String) -> Result<MidiOutputConnection, Error> {
        let midi_out = MidiOutput::new("Generative Sequencer MIDI Out")?;
        match midi_out.find_port_by_id(port_name.clone()) {
            Some(midi_port) => midi_out
                .connect(&midi_port, "gen-seq")
                .map_err(|e| anyhow!("Failed to connect to MIDI output: {}", e)),
            None => Err(anyhow!(
                "Unable to find MIDI output port with name {port_name}"
            )),
        }
    }

    pub fn list_ports() -> Result<Vec<String>, Error> {
        let midi_out = MidiOutput::new("MIDI Out")?;
        Ok(midi_out.ports().iter().map(|p| p.id()).collect::<Vec<_>>())
    }
}
