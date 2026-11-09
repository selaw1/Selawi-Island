//! Shared protocol cases. The JavaScript validator runs the same file in
//! tests/js/protocol.test.js, so both sides accept and reject the same events.

use std::path::PathBuf;

use selawi_core::parse_input;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Cases {
    valid: Vec<Value>,
    invalid: Vec<Invalid>,
}

#[derive(Deserialize)]
struct Invalid {
    why: String,
    event: Value,
}

#[test]
fn rust_validator_agrees_with_shared_cases() -> Result<(), Box<dyn std::error::Error>> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/protocol-cases.json");
    let cases: Cases = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    for event in &cases.valid {
        if let Err(e) = parse_input(&event.to_string()) {
            panic!("expected valid, got {e}: {event}");
        }
    }
    for case in &cases.invalid {
        assert!(
            parse_input(&case.event.to_string()).is_err(),
            "accepted invalid event: {}",
            case.why
        );
    }
    Ok(())
}
