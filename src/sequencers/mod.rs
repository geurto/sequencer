pub mod markov;
pub(crate) mod euclidean;

use crate::common::Sequence;

pub trait Sequencer {
    fn generate_sequence(&self, length: usize) -> Sequence;
}