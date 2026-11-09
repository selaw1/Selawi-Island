use selawi_core::events::{
    Action, CallPayload, Category, Confidence, EventId, EventInput, LifecycleState, MediaPayload,
    NotificationPayload, Payload, ProgressPayload, Source, Timestamp,
};
use selawi_core::{Change, EngineConfig, EngineError, EventStore, Priority};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn id(s: &str) -> EventId {
    EventId::new(s).unwrap_or_else(|e| panic!("bad test id {s}: {e}"))
}

fn source() -> Source {
    Source {
        app: "Test".into(),
        integration: "test".into(),
    }
}

fn notification(name: &str, title: &str) -> EventInput {
    EventInput {
        id: id(name),
        source: source(),
        category: Category::Notification,
        state: LifecycleState::Started,
        priority: None,
        persistent: false,
        ttl_ms: Some(5_000),
        group: None,
        payload: Payload::Notification(NotificationPayload {
            title: title.into(),
            body: None,
        }),
        actions: vec![],
    }
}

fn call(name: &str) -> EventInput {
    EventInput {
        id: id(name),
        source: Source {
            app: "Slack".into(),
            integration: "test".into(),
        },
        category: Category::Call,
        state: LifecycleState::Started,
        priority: None,
        persistent: true,
        ttl_ms: None,
        group: None,
        payload: Payload::Call(CallPayload {
            title: "Standup".into(),
            confidence: Confidence::Confirmed,
            muted: None,
        }),
        actions: vec![Action {
            id: "end".into(),
            label: "End".into(),
        }],
    }
}

fn download(name: &str, state: LifecycleState, percent: f64) -> EventInput {
    EventInput {
        id: id(name),
        source: source(),
        category: Category::Transfer,
        state,
        priority: None,
        persistent: true,
        ttl_ms: None,
        group: None,
        payload: Payload::Progress(ProgressPayload {
            title: "file.iso".into(),
            detail: None,
            percent: Some(percent),
        }),
        actions: vec![],
    }
}

fn media(name: &str) -> EventInput {
    EventInput {
        id: id(name),
        source: source(),
        category: Category::Media,
        state: LifecycleState::Started,
        priority: None,
        persistent: true,
        ttl_ms: None,
        group: None,
        payload: Payload::Media(MediaPayload {
            title: "Song".into(),
            artist: None,
            album: None,
            playing: true,
            position_ms: None,
            duration_ms: None,
        }),
        actions: vec![],
    }
}

fn ordered_ids(store: &EventStore) -> Vec<String> {
    store.ordered().iter().map(|e| e.id.to_string()).collect()
}

#[test]
fn creates_and_stamps_events() -> TestResult {
    let mut store = EventStore::default();
    let changes = store.apply(notification("n1", "Hi"), Timestamp(1_000))?;
    let [Change::Added(event)] = changes.as_slice() else {
        panic!("expected one Added: {changes:?}")
    };
    assert_eq!(event.created_at, Timestamp(1_000));
    assert_eq!(event.updated_at, Timestamp(1_000));
    assert_eq!(event.expires_at, Some(Timestamp(6_000)));
    assert_eq!(event.priority, Priority::Notification);
    Ok(())
}

#[test]
fn updates_keep_creation_time() -> TestResult {
    let mut store = EventStore::default();
    store.apply(
        download("d", LifecycleState::Started, 0.0),
        Timestamp(1_000),
    )?;
    let changes = store.apply(
        download("d", LifecycleState::Updated, 50.0),
        Timestamp(2_000),
    )?;
    let [Change::Updated(event)] = changes.as_slice() else {
        panic!("expected Updated: {changes:?}")
    };
    assert_eq!(event.created_at, Timestamp(1_000));
    assert_eq!(event.updated_at, Timestamp(2_000));
    assert_eq!(event.state, LifecycleState::Updated);
    assert_eq!(event.expires_at, None, "persistent events never expire");
    Ok(())
}

#[test]
fn duplicate_input_is_deduplicated() -> TestResult {
    let mut store = EventStore::default();
    store.apply(
        download("d", LifecycleState::Started, 10.0),
        Timestamp(1_000),
    )?;
    let changes = store.apply(
        download("d", LifecycleState::Updated, 10.0),
        Timestamp(2_000),
    )?;
    assert!(changes.is_empty());
    assert_eq!(
        store.get(&id("d")).map(|e| e.updated_at),
        Some(Timestamp(1_000))
    );
    assert_eq!(store.len(), 1);
    Ok(())
}

#[test]
fn ended_event_lingers_then_is_removed() -> TestResult {
    let mut store = EventStore::new(EngineConfig {
        finished_linger_ms: 3_000,
        ..EngineConfig::default()
    });
    store.apply(download("d", LifecycleState::Started, 0.0), Timestamp(0))?;
    store.apply(
        download("d", LifecycleState::Ended, 100.0),
        Timestamp(10_000),
    )?;
    assert_eq!(
        store.get(&id("d")).map(|e| e.state),
        Some(LifecycleState::Ended)
    );
    assert_eq!(store.next_deadline(), Some(Timestamp(13_000)));
    assert!(store.tick(Timestamp(12_999)).is_empty());
    assert_eq!(
        store.tick(Timestamp(13_000)),
        vec![Change::Removed(id("d"))]
    );
    assert!(store.is_empty());
    Ok(())
}

#[test]
fn stale_update_after_end_is_rejected() -> TestResult {
    let mut store = EventStore::default();
    store.apply(download("d", LifecycleState::Started, 0.0), Timestamp(0))?;
    store.apply(download("d", LifecycleState::Failed, 20.0), Timestamp(1))?;
    let result = store.apply(download("d", LifecycleState::Updated, 30.0), Timestamp(2));
    assert!(matches!(result, Err(EngineError::Lifecycle(_))));
    Ok(())
}

#[test]
fn non_persistent_events_expire() -> TestResult {
    let mut store = EventStore::default();
    store.apply(notification("n", "Hi"), Timestamp(0))?;
    assert_eq!(store.next_deadline(), Some(Timestamp(5_000)));
    assert_eq!(store.tick(Timestamp(5_000)), vec![Change::Removed(id("n"))]);
    Ok(())
}

#[test]
fn updates_refresh_expiry() -> TestResult {
    let mut store = EventStore::default();
    store.apply(notification("n", "One"), Timestamp(0))?;
    let mut again = notification("n", "Two");
    again.state = LifecycleState::Updated;
    store.apply(again, Timestamp(4_000))?;
    assert!(store.tick(Timestamp(5_000)).is_empty());
    assert_eq!(store.next_deadline(), Some(Timestamp(9_000)));
    Ok(())
}

#[test]
fn expired_state_from_source_removes_event() -> TestResult {
    let mut store = EventStore::default();
    store.apply(notification("n", "Hi"), Timestamp(0))?;
    let mut expire = notification("n", "Hi");
    expire.state = LifecycleState::Expired;
    assert_eq!(
        store.apply(expire, Timestamp(1))?,
        vec![Change::Removed(id("n"))]
    );
    Ok(())
}

#[test]
fn notification_does_not_displace_active_call() -> TestResult {
    let mut store = EventStore::default();
    store.apply(call("call"), Timestamp(0))?;
    store.apply(media("music"), Timestamp(1))?;
    store.apply(notification("n", "New mail"), Timestamp(2))?;
    store.apply(download("d", LifecycleState::Started, 0.0), Timestamp(3))?;
    assert_eq!(ordered_ids(&store), ["call", "n", "music", "d"]);
    assert_eq!(
        store.primary().map(|e| e.id.to_string()).as_deref(),
        Some("call")
    );
    assert_eq!(
        store.len(),
        4,
        "nothing disappears because another event arrived"
    );
    Ok(())
}

#[test]
fn newest_first_within_same_priority() -> TestResult {
    let mut store = EventStore::default();
    store.apply(notification("old", "A"), Timestamp(0))?;
    store.apply(notification("new", "B"), Timestamp(10))?;
    // An update to the old one must not reorder (prevents flicker).
    let mut update = notification("old", "A2");
    update.state = LifecycleState::Updated;
    store.apply(update, Timestamp(20))?;
    assert_eq!(ordered_ids(&store), ["new", "old"]);
    Ok(())
}

#[test]
fn user_priority_override_wins() -> TestResult {
    let mut config = EngineConfig::default();
    config
        .category_priority
        .insert(Category::Media, Priority::Critical);
    let mut store = EventStore::new(config);
    store.apply(call("call"), Timestamp(0))?;
    let mut song = media("music");
    song.priority = Some(Priority::Metric);
    store.apply(song, Timestamp(1))?;
    assert_eq!(ordered_ids(&store), ["music", "call"]);
    Ok(())
}

#[test]
fn dismissed_event_ignores_updates_until_restarted() -> TestResult {
    let mut store = EventStore::default();
    store.apply(download("d", LifecycleState::Started, 0.0), Timestamp(0))?;
    assert_eq!(store.dismiss(&id("d")), Some(Change::Removed(id("d"))));
    assert!(store
        .apply(download("d", LifecycleState::Updated, 50.0), Timestamp(1))?
        .is_empty());
    assert!(store.is_empty());
    let changes = store.apply(download("d", LifecycleState::Started, 0.0), Timestamp(2))?;
    assert!(matches!(changes.as_slice(), [Change::Added(_)]));
    Ok(())
}

#[test]
fn grouping_collects_events_with_same_key() -> TestResult {
    let mut store = EventStore::default();
    for (n, title) in [("m1", "A"), ("m2", "B")] {
        let mut input = notification(n, title);
        input.group = Some("chat".into());
        store.apply(input, Timestamp(0))?;
    }
    store.apply(notification("solo", "C"), Timestamp(0))?;
    let groups = store.groups();
    assert_eq!(groups.len(), 2);
    let chat = groups
        .iter()
        .find(|(k, _)| k == "chat")
        .map(|(_, m)| m.len());
    assert_eq!(chat, Some(2));
    Ok(())
}

#[test]
fn capacity_evicts_least_important_first() -> TestResult {
    let mut store = EventStore::new(EngineConfig {
        max_events: 3,
        ..EngineConfig::default()
    });
    store.apply(call("call"), Timestamp(0))?;
    store.apply(notification("n1", "A"), Timestamp(1))?;
    let mut metric = notification("metric", "CPU");
    metric.category = Category::Metric;
    store.apply(metric, Timestamp(2))?;
    let changes = store.apply(notification("n2", "B"), Timestamp(3))?;
    assert!(changes.contains(&Change::Removed(id("metric"))));
    assert_eq!(store.len(), 3);
    assert!(store.get(&id("call")).is_some());
    Ok(())
}

#[test]
fn high_event_volume_stays_bounded() -> TestResult {
    let mut store = EventStore::default();
    for i in 0..10_000u64 {
        store.apply(notification(&format!("n{i}"), "spam"), Timestamp(i))?;
    }
    assert_eq!(store.len(), store.config().max_events);
    Ok(())
}

#[test]
fn snapshot_restores_only_active_persistent_events() -> TestResult {
    let mut store = EventStore::default();
    store.apply(call("call"), Timestamp(0))?;
    store.apply(notification("n", "temp"), Timestamp(0))?;
    store.apply(download("d", LifecycleState::Started, 0.0), Timestamp(0))?;
    store.apply(download("d", LifecycleState::Ended, 100.0), Timestamp(1))?;

    let json = serde_json::to_string(&store.snapshot())?;
    let restored = EventStore::restore(
        EngineConfig::default(),
        serde_json::from_str(&json)?,
        Timestamp(1_000),
        60_000,
    );
    assert_eq!(ordered_ids(&restored), ["call"]);
    assert_eq!(
        restored.get(&id("call")).map(|e| e.created_at),
        Some(Timestamp(0))
    );
    Ok(())
}

#[test]
fn restored_event_expires_unless_its_source_updates_it() -> TestResult {
    let mut store = EventStore::default();
    store.apply(call("a"), Timestamp(0))?;
    store.apply(call("b"), Timestamp(0))?;
    let mut restored = EventStore::restore(
        EngineConfig::default(),
        store.snapshot(),
        Timestamp(1_000),
        60_000,
    );
    assert_eq!(restored.next_deadline(), Some(Timestamp(61_000)));

    // The source of "a" is still alive and repeats its state.
    let mut again = call("a");
    again.state = LifecycleState::Updated;
    assert!(restored.apply(again, Timestamp(30_000))?.is_empty());

    assert_eq!(
        restored.tick(Timestamp(61_000)),
        vec![Change::Removed(id("b"))]
    );
    assert!(
        restored.get(&id("a")).is_some(),
        "updated event is persistent again"
    );
    assert_eq!(restored.next_deadline(), None);
    Ok(())
}

#[test]
fn parse_input_rejects_invalid_json_events() {
    assert!(selawi_core::parse_input("{not json").is_err());
    let bad_percent = r#"{"id":"d","source":{"app":"A","integration":"b"},"category":"transfer",
        "state":"started","payload":{"type":"progress","title":"x","percent":101}}"#;
    assert!(matches!(
        selawi_core::parse_input(bad_percent),
        Err(EngineError::Invalid(_))
    ));
}
