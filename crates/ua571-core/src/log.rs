//! Rolling engagement / system event log.

use std::collections::VecDeque;
use std::fmt;
use std::time::Duration;

use web_time::Instant;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub const DEFAULT_LOG_CAPACITY: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum LogKind {
    Boot,
    ScreenChange {
        to: String,
    },
    SentrySelect {
        id: u8,
    },
    OptionChange {
        section: String,
        value: String,
    },
    Armed {
        sentry: u8,
    },
    Safe {
        sentry: u8,
    },
    Fire {
        sentry: u8,
        rounds_left: u16,
        profile: String,
        spectral: String,
    },
    Critical {
        sentry: u8,
        rounds: u16,
    },
    Empty {
        sentry: u8,
    },
    Demo {
        message: String,
    },
    Info {
        message: String,
    },
}

impl fmt::Display for LogKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogKind::Boot => write!(f, "SYSTEM BOOT — UA 571-C CONSOLE ONLINE"),
            LogKind::ScreenChange { to } => write!(f, "DISPLAY → {to}"),
            LogKind::SentrySelect { id } => write!(f, "OPERATOR SELECT SENTRY-{id}"),
            LogKind::OptionChange { section, value } => {
                write!(f, "{section} SET → {value}")
            }
            LogKind::Armed { sentry } => write!(f, "SENTRY-{sentry} WEAPON ARMED"),
            LogKind::Safe { sentry } => write!(f, "SENTRY-{sentry} WEAPON SAFE"),
            LogKind::Fire {
                sentry,
                rounds_left,
                profile,
                spectral,
            } => write!(
                f,
                "SENTRY-{sentry} FIRE — {profile}/{spectral} — {rounds_left} RDS"
            ),
            LogKind::Critical { sentry, rounds } => {
                write!(f, "SENTRY-{sentry} CRITICAL — {rounds} RDS REMAINING")
            }
            LogKind::Empty { sentry } => write!(f, "SENTRY-{sentry} DRUM EMPTY"),
            LogKind::Demo { message } => write!(f, "DEMO: {message}"),
            LogKind::Info { message } => write!(f, "{message}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEvent {
    pub kind: LogKind,
    pub at: Instant,
    /// Monotonic sequence for stable ordering.
    pub seq: u64,
}

impl LogEvent {
    pub fn age(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.at)
    }
}

#[derive(Debug, Clone)]
pub struct EventLog {
    events: VecDeque<LogEvent>,
    capacity: usize,
    next_seq: u64,
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new(DEFAULT_LOG_CAPACITY)
    }
}

impl EventLog {
    pub fn new(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity),
            capacity: capacity.max(1),
            next_seq: 0,
        }
    }

    pub fn push(&mut self, kind: LogKind) {
        if self.events.len() >= self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(LogEvent {
            kind,
            at: Instant::now(),
            seq: self.next_seq,
        });
        self.next_seq += 1;
    }

    pub fn push_info(&mut self, message: impl Into<String>) {
        self.push(LogKind::Info {
            message: message.into(),
        });
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &LogEvent> {
        self.events.iter()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Newest-first slice for UI.
    pub fn recent(&self, n: usize) -> Vec<&LogEvent> {
        self.events.iter().rev().take(n).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_evicts_oldest() {
        let mut log = EventLog::new(3);
        log.push_info("a");
        log.push_info("b");
        log.push_info("c");
        log.push_info("d");
        assert_eq!(log.len(), 3);
        let msgs: Vec<_> = log.iter().map(|e| e.kind.to_string()).collect();
        assert!(msgs[0].contains('b'));
        assert!(msgs[2].contains('d'));
    }
}
