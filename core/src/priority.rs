//! Priority tiers. Lower tiers are more important and sort first.

use serde::{Deserialize, Serialize};

use crate::events::Category;

/// Ordered from most to least important; `Ord` follows this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    /// Critical system warnings.
    Critical,
    /// Active communication calls.
    Call,
    /// Important device connects and disconnects.
    Device,
    /// Important notifications.
    Notification,
    /// Media playback.
    Media,
    /// Downloads and file operations.
    Transfer,
    /// Timers and reminders.
    Reminder,
    /// Low-priority system metrics.
    Metric,
}

impl Priority {
    /// Default tier for a category when neither the user nor the source set one.
    pub fn default_for(category: Category) -> Priority {
        match category {
            Category::Call => Priority::Call,
            Category::Device => Priority::Device,
            Category::Notification | Category::System | Category::Custom => Priority::Notification,
            Category::Media => Priority::Media,
            Category::Transfer => Priority::Transfer,
            Category::Reminder => Priority::Reminder,
            Category::Metric => Priority::Metric,
        }
    }

    /// Whether an event at `self` may take the island over from `current`.
    /// Equal tiers may replace each other; less important ones never do.
    pub fn may_interrupt(self, current: Priority) -> bool {
        self <= current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_outrank_notifications_and_media() {
        assert!(Priority::Call < Priority::Notification);
        assert!(Priority::Critical < Priority::Call);
        assert!(Priority::Media < Priority::Metric);
    }

    #[test]
    fn low_priority_never_interrupts_a_call() {
        for p in [
            Priority::Notification,
            Priority::Media,
            Priority::Transfer,
            Priority::Metric,
        ] {
            assert!(!p.may_interrupt(Priority::Call));
        }
        assert!(Priority::Critical.may_interrupt(Priority::Call));
    }
}
