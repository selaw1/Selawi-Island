//! A 25-minute focus timer on the island with a Stop button.
//!   cargo run -p selawi-island-plugin --example countdown

use std::time::{Duration, Instant};

use futures_util::StreamExt;
use selawi_island_plugin::Island;

const MANIFEST: &str = r#"{"manifest_version":1,"id":"countdown","name":"Focus timer",
    "description":"A 25-minute focus timer","categories":["custom"]}"#;
const LENGTH: Duration = Duration::from_secs(25 * 60);

fn event(state: &str, percent: f64, left: Duration) -> String {
    format!(
        r#"{{"id":"focus","category":"custom","state":"{state}",
            "persistent":true,"payload":{{"type":"progress","title":"Focus","detail":"{}:{:02} left","percent":{percent:.1}}},
            "actions":[{{"id":"stop","label":"Stop"}}]}}"#,
        left.as_secs() / 60,
        left.as_secs() % 60
    )
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> zbus::Result<()> {
    let island = Island::register(MANIFEST).await?;
    if !island.allowed() {
        println!("Allow \"Focus timer\" on the island (it is asking), then run this again.");
        return Ok(());
    }
    let mut pressed = island.actions().await?;
    let start = Instant::now();
    loop {
        let elapsed = start.elapsed().min(LENGTH);
        let percent = elapsed.as_secs_f64() / LENGTH.as_secs_f64() * 100.0;
        let state = if elapsed >= LENGTH {
            "ended"
        } else if elapsed.is_zero() {
            "started"
        } else {
            "updated"
        };
        island
            .show(&event(state, percent, LENGTH - elapsed))
            .await?;
        if elapsed >= LENGTH {
            return Ok(());
        }
        tokio::select! {
            () = tokio::time::sleep(Duration::from_secs(10)) => {}
            Some((_, action)) = pressed.next() => if action == "stop" {
                island.show(&event("failed", percent, LENGTH - elapsed)).await?;
                return Ok(());
            },
        }
    }
}
