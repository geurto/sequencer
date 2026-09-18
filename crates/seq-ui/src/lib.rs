//! The user-interface seam.
//!
//! Two types define the boundary between the sequencer and any interface to
//! it:
//!
//! - [`UiSnapshot`] — everything a renderer needs to draw one frame, and
//!   nothing else. Flat, `Copy`, integer-only. It is the payload of the
//!   desktop's `watch` channel and of the MCU's `embassy_sync::Signal`.
//! - [`ControlEvent`] — everything an input surface can ask for. Produced by
//!   desktop keys, simulator keys, GPIO encoders or MIDI CC; the sequencer
//!   never learns which.
//!
//! The renderer itself (`embedded-graphics`) arrives in a later step; this
//! crate deliberately holds the *types only*, so the concurrency model and the
//! interfaces on both sides of it can settle first.

#![cfg_attr(not(test), no_std)]

pub mod control;
pub mod snapshot;

pub use control::ControlEvent;
pub use snapshot::{Slot, SlotSnapshot, UiSnapshot};
