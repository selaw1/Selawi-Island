//! The event engine: applies source input, deduplicates, orders by
//! priority, expires stale events and keeps finished ones briefly visible.
//!
//! Time is passed in explicitly so the engine is deterministic and the
//! caller can schedule a single timer from [`EventStore::next_deadline`]
//! instead of polling.

use std::cmp::Reverse;
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

use crate::events::{
    Category, Event, EventId, EventInput, LifecycleState, Timestamp, ValidationError,
};
use crate::lifecycle::{transition, LifecycleError, Transition};
use crate::priority::Priority;

const MAX_REMEMBERED_DISMISSALS: usize = 256;

#[derive(Clone, Debug, PartialEq)]
pub struct EngineConfig {
    /// Lifetime of non-persistent events that don't set `ttl_ms`.
    pub default_ttl_ms: u64,
    /// How long ended or failed events stay visible before removal.
    pub finished_linger_ms: u64,
    /// Upper bound on stored events, to survive event floods.
    pub max_events: usize,
    /// User overrides. These win over the priority a source asks for.
    pub category_priority: BTreeMap<Category, Priority>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            default_ttl_ms: 8_000,
            finished_linger_ms: 4_000,
            max_events: 64,
            category_priority: BTreeMap::new(),
        }
    }
}

/// What changed. The UI can simply re-read [`EventStore::ordered`].
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Added(Event),
    Updated(Event),
    Removed(EventId),
}

#[derive(Debug)]
pub enum EngineError {
    Json(serde_json::Error),
    Invalid(ValidationError),
    Lifecycle(LifecycleError),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(e) => write!(f, "malformed event: {e}"),
            Self::Invalid(e) => write!(f, "invalid event: {e}"),
            Self::Lifecycle(e) => write!(f, "rejected state change: {e}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<ValidationError> for EngineError {
    fn from(e: ValidationError) -> Self {
        Self::Invalid(e)
    }
}

impl From<LifecycleError> for EngineError {
    fn from(e: LifecycleError) -> Self {
        Self::Lifecycle(e)
    }
}

/// Parses and validates one event from JSON, as received at an
/// integration boundary.
pub fn parse_input(json: &str) -> Result<EventInput, EngineError> {
    let input: EventInput = serde_json::from_str(json).map_err(EngineError::Json)?;
    input.validate()?;
    Ok(input)
}

#[derive(Clone, Debug)]
struct Stored {
    event: Event,
    /// Set once the event has finished.
    remove_at: Option<Timestamp>,
}

#[derive(Clone, Debug, Default)]
pub struct EventStore {
    config: EngineConfig,
    events: BTreeMap<EventId, Stored>,
    /// Active events the user dismissed. Further updates are ignored
    /// until the source starts the event again.
    dismissed: VecDeque<EventId>,
}

impl EventStore {
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config,
            events: BTreeMap::new(),
            dismissed: VecDeque::new(),
        }
    }

    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn get(&self, id: &EventId) -> Option<&Event> {
        self.events.get(id).map(|s| &s.event)
    }

    /// Applies one validated or unvalidated input. Returns every change it
    /// caused, which is empty when the input changed nothing.
    pub fn apply(&mut self, input: EventInput, now: Timestamp) -> Result<Vec<Change>, EngineError> {
        input.validate()?;

        if let Some(pos) = self.dismissed.iter().position(|id| *id == input.id) {
            if input.state != LifecycleState::Started {
                if input.state.is_terminal() {
                    self.dismissed.remove(pos);
                }
                return Ok(Vec::new());
            }
            self.dismissed.remove(pos);
        }

        let current = self.events.get(&input.id).map(|s| s.event.state);
        let step = transition(current, input.state)?;
        let priority = self.resolve_priority(&input);

        let change = match step {
            Transition::Create | Transition::Restart => {
                let event = self.new_event(input, priority, now);
                let remove_at = self.remove_at_for(&event, now);
                self.events.insert(
                    event.id.clone(),
                    Stored {
                        event: event.clone(),
                        remove_at,
                    },
                );
                Some(Change::Added(event))
            }
            Transition::Finish if current.is_none() => {
                let event = self.new_event(input, priority, now);
                let remove_at = self.remove_at_for(&event, now);
                self.events.insert(
                    event.id.clone(),
                    Stored {
                        event: event.clone(),
                        remove_at,
                    },
                );
                Some(Change::Added(event))
            }
            Transition::Finish if input.state == LifecycleState::Expired => self
                .events
                .remove(&input.id)
                .map(|s| Change::Removed(s.event.id)),
            Transition::Update | Transition::Finish => self.update_existing(input, priority, now),
        };

        let mut changes: Vec<Change> = change.into_iter().collect();
        if let Some(Change::Added(added)) = changes.first() {
            let keep = added.id.clone();
            changes.extend(self.enforce_capacity(&keep));
        }
        Ok(changes)
    }

    /// Removes an event because the user dismissed it.
    pub fn dismiss(&mut self, id: &EventId) -> Option<Change> {
        let stored = self.events.remove(id)?;
        if !stored.event.state.is_terminal() {
            if self.dismissed.len() >= MAX_REMEMBERED_DISMISSALS {
                self.dismissed.pop_front();
            }
            self.dismissed.push_back(id.clone());
        }
        Some(Change::Removed(stored.event.id))
    }

    /// Expires and removes everything whose deadline has passed.
    pub fn tick(&mut self, now: Timestamp) -> Vec<Change> {
        let due: Vec<EventId> = self
            .events
            .values()
            .filter(|s| Self::deadline_of(s).is_some_and(|d| d <= now))
            .map(|s| s.event.id.clone())
            .collect();
        due.into_iter()
            .filter_map(|id| self.events.remove(&id).map(|s| Change::Removed(s.event.id)))
            .collect()
    }

    /// The next time [`tick`](Self::tick) has work to do.
    pub fn next_deadline(&self) -> Option<Timestamp> {
        self.events.values().filter_map(Self::deadline_of).min()
    }

    /// Events in display order: most important first, then newest first.
    pub fn ordered(&self) -> Vec<&Event> {
        let mut list: Vec<&Event> = self.events.values().map(|s| &s.event).collect();
        list.sort_by(|a, b| {
            (a.priority, Reverse(a.created_at), &a.id).cmp(&(
                b.priority,
                Reverse(b.created_at),
                &b.id,
            ))
        });
        list
    }

    /// The event the compact island should show.
    pub fn primary(&self) -> Option<&Event> {
        self.ordered().into_iter().next()
    }

    /// Events grouped by their group key (or their id when ungrouped), in
    /// display order of each group's most important event.
    pub fn groups(&self) -> Vec<(String, Vec<&Event>)> {
        let mut groups: Vec<(String, Vec<&Event>)> = Vec::new();
        for event in self.ordered() {
            let key = event.group.clone().unwrap_or_else(|| event.id.to_string());
            match groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, members)) => members.push(event),
                None => groups.push((key, vec![event])),
            }
        }
        groups
    }

    /// Events worth keeping across a daemon restart: active persistent ones.
    pub fn snapshot(&self) -> Vec<Event> {
        self.events
            .values()
            .filter(|s| s.event.persistent && !s.event.state.is_terminal())
            .map(|s| s.event.clone())
            .collect()
    }

    /// Rebuilds a store from a snapshot. Entries that are invalid, finished
    /// or not persistent are dropped rather than trusted.
    pub fn restore(config: EngineConfig, snapshot: Vec<Event>) -> Self {
        let mut store = Self::new(config);
        for event in snapshot {
            if !event.persistent
                || event.state.is_terminal()
                || to_input(&event).validate().is_err()
            {
                continue;
            }
            if store.events.len() >= store.config.max_events {
                break;
            }
            store.events.insert(
                event.id.clone(),
                Stored {
                    event,
                    remove_at: None,
                },
            );
        }
        store
    }

    fn resolve_priority(&self, input: &EventInput) -> Priority {
        self.config
            .category_priority
            .get(&input.category)
            .copied()
            .or(input.priority)
            .unwrap_or_else(|| Priority::default_for(input.category))
    }

    fn expires_at_for(&self, input: &EventInput, now: Timestamp) -> Option<Timestamp> {
        if input.persistent || input.state.is_terminal() {
            None
        } else {
            Some(now.plus_ms(input.ttl_ms.unwrap_or(self.config.default_ttl_ms)))
        }
    }

    fn remove_at_for(&self, event: &Event, now: Timestamp) -> Option<Timestamp> {
        event
            .state
            .is_terminal()
            .then(|| now.plus_ms(self.config.finished_linger_ms))
    }

    fn new_event(&self, input: EventInput, priority: Priority, now: Timestamp) -> Event {
        let expires_at = self.expires_at_for(&input, now);
        Event {
            id: input.id,
            source: input.source,
            category: input.category,
            state: input.state,
            priority,
            persistent: input.persistent,
            created_at: now,
            updated_at: now,
            expires_at,
            group: input.group,
            payload: input.payload,
            actions: input.actions,
        }
    }

    fn update_existing(
        &mut self,
        input: EventInput,
        priority: Priority,
        now: Timestamp,
    ) -> Option<Change> {
        let expires_at = self.expires_at_for(&input, now);
        let linger = self.config.finished_linger_ms;
        let stored = self.events.get_mut(&input.id)?;
        let event = &mut stored.event;

        // Started and Updated both mean "active": a repeat with identical
        // content is a duplicate and must not bump timestamps.
        let same_phase = event.state == input.state
            || (!event.state.is_terminal() && !input.state.is_terminal());
        let unchanged = same_phase
            && event.source == input.source
            && event.category == input.category
            && event.priority == priority
            && event.persistent == input.persistent
            && event.group == input.group
            && event.payload == input.payload
            && event.actions == input.actions;
        if unchanged {
            if event.expires_at.is_some() {
                event.expires_at = expires_at;
            }
            return None;
        }

        event.state = input.state;
        if input.state.is_terminal() {
            stored.remove_at = Some(now.plus_ms(linger));
        }
        event.source = input.source;
        event.category = input.category;
        event.priority = priority;
        event.persistent = input.persistent;
        event.group = input.group;
        event.payload = input.payload;
        event.actions = input.actions;
        event.updated_at = now;
        event.expires_at = expires_at;
        Some(Change::Updated(event.clone()))
    }

    /// Evicts the least important, oldest non-persistent events first.
    fn enforce_capacity(&mut self, keep: &EventId) -> Vec<Change> {
        let mut removed = Vec::new();
        while self.events.len() > self.config.max_events {
            let victim = self
                .events
                .values()
                .filter(|s| s.event.id != *keep)
                .max_by_key(|s| {
                    (
                        !s.event.persistent,
                        s.event.priority,
                        Reverse(s.event.updated_at),
                    )
                })
                .map(|s| s.event.id.clone());
            match victim.and_then(|id| self.events.remove(&id)) {
                Some(s) => removed.push(Change::Removed(s.event.id)),
                None => break,
            }
        }
        removed
    }

    fn deadline_of(stored: &Stored) -> Option<Timestamp> {
        match (stored.remove_at, stored.event.expires_at) {
            (Some(r), _) => Some(r),
            (None, e) => e,
        }
    }
}

fn to_input(event: &Event) -> EventInput {
    EventInput {
        id: event.id.clone(),
        source: event.source.clone(),
        category: event.category,
        state: event.state,
        priority: Some(event.priority),
        persistent: event.persistent,
        ttl_ms: None,
        group: event.group.clone(),
        payload: event.payload.clone(),
        actions: event.actions.clone(),
    }
}
