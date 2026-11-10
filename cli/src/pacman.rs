//! What the island says during a pacman transaction (also yay, which hands
//! installing to pacman). Pure functions; main.rs runs the hook.
//!
//! The hook runs as root. It never talks to D-Bus as root: it starts a copy
//! of this program as each logged-in user (std's `Command::uid`, which also
//! drops root's groups) to deliver one event on that user's session bus.

use std::collections::HashSet;

pub const EVENT_ID: &str = "pacman:transaction";
pub const INTEGRATION: &str = "pacman";
pub const APP: &str = "Packages";
pub const LOCK_FILE: &str = "/var/lib/pacman/db.lck";
pub const LOCAL_DB: &str = "/var/lib/pacman/local";
/// Users get the event if their uid is at least this (no system users).
pub const FIRST_USER_UID: u32 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// Install or upgrade.
    Install,
    Remove,
}

/// Package names from pacman's local database folder entries
/// ("firefox-133.0-1" → "firefox": the last two dash parts are version and
/// release).
pub fn installed_names<'a>(entries: impl IntoIterator<Item = &'a str>) -> HashSet<String> {
    entries
        .into_iter()
        .filter_map(|entry| {
            let mut parts = entry.rsplitn(3, '-');
            let _release = parts.next()?;
            let _version = parts.next()?;
            parts.next().map(String::from)
        })
        .collect()
}

/// Title and detail when the transaction starts. `installed` is None when
/// the local database could not be read.
pub fn summary(
    operation: Operation,
    targets: &[String],
    installed: Option<&HashSet<String>>,
) -> (String, Option<String>) {
    let count = targets.len();
    let title = match operation {
        Operation::Remove if count == 1 => format!("Removing {}", targets[0]),
        Operation::Remove => format!("Removing {count} packages"),
        Operation::Install => {
            let all_new = installed.is_some_and(|names| targets.iter().all(|t| !names.contains(t)));
            match (all_new, count) {
                (true, 1) => format!("Installing {}", targets[0]),
                (true, _) => format!("Installing {count} packages"),
                (false, 1) => format!("Updating {}", targets[0]),
                (false, _) => format!("Updating {count} packages"),
            }
        }
    };
    let detail = if count > 1 {
        let mut names: Vec<&str> = targets.iter().take(3).map(String::as_str).collect();
        if count > 3 {
            names.push("…");
        }
        Some(names.join(", "))
    } else {
        None
    };
    (title, detail)
}

/// "Updating 23 packages" → "Updated 23 packages" when it finished.
pub fn finished_title(title: &str) -> String {
    for (now, done) in [
        ("Updating ", "Updated "),
        ("Installing ", "Installed "),
        ("Removing ", "Removed "),
    ] {
        if let Some(rest) = title.strip_prefix(now) {
            return format!("{done}{rest}");
        }
    }
    title.to_string()
}

/// Valid package names only (pacman's rules: letters, digits and
/// `@ . _ + -`), so nothing odd reaches the island.
pub fn clean_targets(stdin: &str) -> Vec<String> {
    stdin
        .lines()
        .map(str::trim)
        .filter(|name| {
            !name.is_empty()
                && name.len() <= 128
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "@._+-".contains(c))
        })
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn local_database_names() {
        let set = installed_names([
            "firefox-133.0-1",
            "lib32-mesa-24.2.1-2",
            "python-3.13.1-1",
            "ALPM_DB_VERSION",
        ]);
        assert!(set.contains("firefox"));
        assert!(set.contains("lib32-mesa"));
        assert!(set.contains("python"));
        assert!(!set.contains("ALPM_DB_VERSION"));
    }

    #[test]
    fn titles() {
        let installed = installed_names(["firefox-133.0-1", "mesa-24.2.1-2"]);
        let some = Some(&installed);
        assert_eq!(
            summary(Operation::Install, &names(&["spotify"]), some).0,
            "Installing spotify"
        );
        assert_eq!(
            summary(Operation::Install, &names(&["firefox"]), some).0,
            "Updating firefox"
        );
        let update = summary(
            Operation::Install,
            &names(&["firefox", "mesa", "spotify", "linux"]),
            some,
        );
        assert_eq!(update.0, "Updating 4 packages");
        assert_eq!(update.1.as_deref(), Some("firefox, mesa, spotify, …"));
        assert_eq!(
            summary(
                Operation::Install,
                &names(&["a", "b"]),
                Some(&HashSet::new())
            )
            .0,
            "Installing 2 packages"
        );
        assert_eq!(
            summary(Operation::Install, &names(&["spotify"]), None).0,
            "Updating spotify",
            "unknown database: no claim it is new"
        );
        assert_eq!(
            summary(Operation::Remove, &names(&["spotify"]), some).0,
            "Removing spotify"
        );
        assert_eq!(
            summary(Operation::Remove, &names(&["a", "b"]), some).0,
            "Removing 2 packages"
        );
        assert_eq!(finished_title("Updating 4 packages"), "Updated 4 packages");
        assert_eq!(finished_title("Installing spotify"), "Installed spotify");
        assert_eq!(finished_title("Removing a"), "Removed a");
    }

    #[test]
    fn only_package_names_get_through() {
        assert_eq!(
            clean_targets("firefox\nlib32-mesa\n\ngtk4+x\n$(rm -rf)\n"),
            names(&["firefox", "lib32-mesa", "gtk4+x"])
        );
    }
}
