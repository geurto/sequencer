//! Pattern generators as async plumbing.
//!
//! The musical generation itself lives in `seq_core` (`euclid`, and eventually
//! other sources); this module holds the tasks that watch the shared state and
//! feed generated patterns to the mixer.

pub mod euclidean;
