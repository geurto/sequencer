//! The platform-independent sequencing core.
//!
//! Everything musical lives here: pattern types, the Euclidean generator, the
//! crossfading mixer and the playback [`Transport`]. Nothing in this crate
//! allocates, spawns, sleeps or does I/O — output goes through the [`MidiSink`]
//! trait and time comes in through [`Transport::advance_to`] — so the same
//! code drives the desktop application and a bare-metal microcontroller build.
//!
//! The crate is `no_std` by default. The `std` feature adds conveniences for
//! hosted platforms (a wall-clock [`SystemClock`], a boxed-sink impl), and
//! `test-util` adds the shared test doubles ([`RecordingSink`],
//! [`ManualClock`]).

#![cfg_attr(not(any(test, feature = "std")), no_std)]

pub mod clock;
pub mod euclid;
pub mod event;
pub mod mixer;
pub mod note;
pub mod sink;
pub mod transport;

pub use clock::Clock;
#[cfg(any(test, feature = "test-util"))]
pub use clock::ManualClock;
#[cfg(any(test, feature = "std"))]
pub use clock::SystemClock;
pub use event::{
    EVENT_CAPACITY, EventVec, MidiEventType, PolyphonicSequence,
    TICKS_PER_QUARTER_NOTE, TICKS_PER_STEP, TimedEvent,
};
pub use note::{
    MAX_STEPS, MAX_VOICES, NoteDuration, NoteName, Pattern, Step, Voice,
    note_name,
};
#[cfg(any(test, feature = "test-util"))]
pub use sink::RecordingSink;
pub use sink::{MidiSink, SendError};
pub use transport::Transport;
