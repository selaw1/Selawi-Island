//! Selawi Island shared core.
//!
//! Platform-independent event model and engine. Nothing in this crate may
//! depend on GNOME, D-Bus, Windows or macOS APIs.

pub mod events;
pub mod lifecycle;
pub mod priority;
pub mod store;

pub use events::{
    Event, EventId, EventInput, LifecycleState, Payload, Timestamp, PROTOCOL_VERSION,
};
pub use priority::Priority;
pub use store::{parse_input, Change, EngineConfig, EngineError, EventStore};
