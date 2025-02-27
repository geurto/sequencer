use crate::note::Sequence;
use anyhow::Error;

pub trait Sequencer {
    fn generate_sequence(&self) -> impl std::future::Future<Output = Sequence> + Send;
    fn run(&mut self) -> impl std::future::Future<Output = Result<(), Error>> + Send;
}
