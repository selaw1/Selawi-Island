//! `selawi-island run -- COMMAND…`: what the island says about a command.
//! Pure functions; the process handling is in main.rs.

use std::time::Duration;

/// A short title from the command line: the program's name and, when it
/// looks like a subcommand, the next word ("cargo build", "make").
/// Only those two words, so arguments (which may hold secrets) never show.
pub fn command_title(command: &[String]) -> String {
    let program = command
        .first()
        .map(|p| p.rsplit('/').next().unwrap_or(p).to_string())
        .unwrap_or_default();
    let sub = command.get(1).filter(|word| {
        word.len() <= 24
            && word.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && word
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == ':')
    });
    let title = match sub {
        Some(sub) => format!("{program} {sub}"),
        None => program,
    };
    let title: String = title.chars().filter(|c| !c.is_control()).take(64).collect();
    if title.trim().is_empty() {
        "Command".into()
    } else {
        title
    }
}

/// How a command ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Exit code.
    Exited(i32),
    /// Ended by a signal.
    Signal(i32),
}

const SIGINT: i32 = 2;
const SIGKILL: i32 = 9;
const SIGTERM: i32 = 15;

impl Outcome {
    pub fn success(self) -> bool {
        self == Outcome::Exited(0)
    }

    /// Exit code for `selawi-island run` itself: the command's own, or
    /// 128 + signal like a shell.
    pub fn exit_code(self) -> u8 {
        let code = match self {
            Outcome::Exited(code) => code,
            Outcome::Signal(signal) => 128 + signal,
        };
        u8::try_from(code.clamp(0, 255)).unwrap_or(1)
    }

    /// The line shown under the title when it has finished.
    pub fn detail(self, elapsed: Duration) -> String {
        let time = format_duration(elapsed);
        match self {
            Outcome::Exited(0) => format!("Done · {time}"),
            Outcome::Exited(code) => format!("Failed (exit {code}) · {time}"),
            Outcome::Signal(SIGINT | SIGTERM | SIGKILL) => format!("Stopped · {time}"),
            Outcome::Signal(signal) => format!("Failed (signal {signal}) · {time}"),
        }
    }
}

/// "0:07", "2:31", "1:02:05".
pub fn format_duration(elapsed: Duration) -> String {
    let total = elapsed.as_secs();
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        line.split(' ').map(String::from).collect()
    }

    #[test]
    fn titles_never_show_arguments() {
        assert_eq!(
            command_title(&words("cargo build --release")),
            "cargo build"
        );
        assert_eq!(command_title(&words("/usr/bin/make")), "make");
        assert_eq!(command_title(&words("npm run test:unit")), "npm run");
        assert_eq!(
            command_title(&words("curl -u user:secret https://x")),
            "curl",
            "flags and what follows are not shown"
        );
        assert_eq!(
            command_title(&words("mysql hunter2")),
            "mysql hunter2",
            "a plain word is shown as the subcommand"
        );
        assert_eq!(command_title(&words("deploy SecretToken")), "deploy");
        assert_eq!(command_title(&[]), "Command");
    }

    #[test]
    fn outcomes() {
        let t = Duration::from_secs(151);
        assert_eq!(Outcome::Exited(0).detail(t), "Done · 2:31");
        assert_eq!(Outcome::Exited(2).detail(t), "Failed (exit 2) · 2:31");
        assert_eq!(Outcome::Signal(2).detail(t), "Stopped · 2:31");
        assert_eq!(Outcome::Signal(11).detail(t), "Failed (signal 11) · 2:31");
        assert_eq!(Outcome::Exited(3).exit_code(), 3);
        assert_eq!(Outcome::Signal(2).exit_code(), 130);
        assert!(Outcome::Exited(0).success());
        assert_eq!(format_duration(Duration::from_secs(3725)), "1:02:05");
        assert_eq!(format_duration(Duration::from_secs(7)), "0:07");
    }
}
