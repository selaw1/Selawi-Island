//! Lifecycle rules: which state changes a source may report.

use std::fmt;

use crate::events::LifecycleState;

/// What applying a reported state means for the stored event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    /// No stored event: create one.
    Create,
    /// A finished event with the same id starts again as a new instance.
    Restart,
    /// An active event changes.
    Update,
    /// An active event (or a new one-shot event) reaches a terminal state.
    Finish,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleError {
    /// Updates for an event that already ended are stale and ignored.
    AlreadyFinished,
    /// `expired` can only be applied to an event that exists and is active.
    ExpireUnknown,
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyFinished => f.write_str("event has already finished"),
            Self::ExpireUnknown => f.write_str("cannot expire an event that is not active"),
        }
    }
}

impl std::error::Error for LifecycleError {}

/// `current` is the stored state, or `None` when no event has this id.
///
/// An `updated` for an unknown id is accepted as a creation so a source
/// can resynchronize after the daemon restarts.
pub fn transition(
    current: Option<LifecycleState>,
    next: LifecycleState,
) -> Result<Transition, LifecycleError> {
    use LifecycleState::*;
    match (current, next) {
        (None, Started | Updated) => Ok(Transition::Create),
        (None, Ended | Failed) => Ok(Transition::Finish),
        (None, Expired) => Err(LifecycleError::ExpireUnknown),
        (Some(c), Started) if c.is_terminal() => Ok(Transition::Restart),
        (Some(c), _) if c.is_terminal() => Err(LifecycleError::AlreadyFinished),
        (Some(_), Started | Updated) => Ok(Transition::Update),
        (Some(_), Ended | Failed | Expired) => Ok(Transition::Finish),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use LifecycleState::*;

    #[test]
    fn normal_download_lifecycle() {
        assert_eq!(transition(None, Started), Ok(Transition::Create));
        assert_eq!(transition(Some(Started), Updated), Ok(Transition::Update));
        assert_eq!(transition(Some(Updated), Updated), Ok(Transition::Update));
        assert_eq!(transition(Some(Updated), Ended), Ok(Transition::Finish));
    }

    #[test]
    fn duplicate_start_is_an_update() {
        assert_eq!(transition(Some(Started), Started), Ok(Transition::Update));
    }

    #[test]
    fn stale_messages_after_end_are_rejected() {
        assert_eq!(
            transition(Some(Ended), Updated),
            Err(LifecycleError::AlreadyFinished)
        );
        assert_eq!(
            transition(Some(Failed), Ended),
            Err(LifecycleError::AlreadyFinished)
        );
    }

    #[test]
    fn finished_event_can_restart() {
        assert_eq!(transition(Some(Ended), Started), Ok(Transition::Restart));
    }

    #[test]
    fn update_for_unknown_event_resynchronizes() {
        assert_eq!(transition(None, Updated), Ok(Transition::Create));
        assert_eq!(
            transition(None, Expired),
            Err(LifecycleError::ExpireUnknown)
        );
    }
}
