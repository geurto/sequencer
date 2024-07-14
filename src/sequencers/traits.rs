use anyhow::Error;
use crate::note::Sequence;

pub trait Sequencer {
    async fn generate_sequence(&self) -> Sequence;
    async fn run(&mut self, sequencer_slot: usize) -> Result<(), Error>;
}