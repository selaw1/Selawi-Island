//! Selawi Island daemon internals, as a library so the integrations can be
//! tested against fake D-Bus services. The binary is `src/main.rs`.

pub mod config;
pub mod dbus;
pub mod engine;
pub mod hub;
pub mod integrations;
