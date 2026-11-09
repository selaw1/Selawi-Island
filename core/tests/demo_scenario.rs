//! The GNOME extension's simulator plays `gnome-extension/data/demo-scenario.json`.
//! This test makes sure every event in it is valid under the Rust protocol
//! and that the whole scenario plays through the engine without errors,
//! so the UI is never built against events the daemon would reject.

use std::path::PathBuf;

use selawi_core::{EventInput, EventStore, Timestamp, PROTOCOL_VERSION};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    protocol_version: u32,
    #[serde(rename = "loop")]
    looping: bool,
    steps: Vec<Step>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    delay_ms: u64,
    event: EventInput,
}

#[test]
fn demo_scenario_is_valid_and_plays_cleanly() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../gnome-extension/data/demo-scenario.json");
    let scenario: Scenario = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    assert_eq!(scenario.protocol_version, PROTOCOL_VERSION);
    assert!(scenario.looping);
    assert!(!scenario.steps.is_empty());

    let mut store = EventStore::default();
    let mut now = 0u64;
    // Play twice to prove the loop restarts cleanly.
    for _ in 0..2 {
        for step in &scenario.steps {
            now += step.delay_ms;
            store.tick(Timestamp(now));
            step.event.validate()?;
            store.apply(step.event.clone(), Timestamp(now))?;
        }
    }
    Ok(())
}
