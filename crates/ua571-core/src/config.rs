//! Runtime configuration for the console.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::fire::DEFAULT_ROUNDS;

/// On-screen color scheme.
///
/// `Yellow` matches the monochrome yellow of the original GRiD / film prop
/// recreation and is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Theme {
    /// Film / GRiD prop yellow on black (default).
    #[default]
    Yellow,
    /// Classic green phosphor.
    Phosphor,
    /// Warm amber CRT.
    Amber,
    /// White-on-black mono.
    Mono,
}

impl Theme {
    pub const ALL: [Theme; 4] = [
        Theme::Yellow,
        Theme::Phosphor,
        Theme::Amber,
        Theme::Mono,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Theme::Yellow => "yellow",
            Theme::Phosphor => "phosphor",
            Theme::Amber => "amber",
            Theme::Mono => "mono",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "yellow" | "grid" | "original" | "film" | "prop" => Some(Theme::Yellow),
            "phosphor" | "green" => Some(Theme::Phosphor),
            "amber" | "orange" => Some(Theme::Amber),
            "mono" | "white" => Some(Theme::Mono),
            _ => None,
        }
    }

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&t| t == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct Config {
    pub theme: Theme,
    /// UI tick rate in milliseconds (blink / demo).
    pub tick_ms: u64,
    pub starting_rounds: u16,
    pub show_boot: bool,
    pub demo_on_start: bool,
    pub log_capacity: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: Theme::Yellow,
            tick_ms: 80,
            starting_rounds: DEFAULT_ROUNDS,
            show_boot: true,
            demo_on_start: false,
            log_capacity: 64,
        }
    }
}

impl Config {
    pub fn validate(mut self) -> Self {
        self.tick_ms = self.tick_ms.clamp(16, 1000);
        if self.log_capacity < 8 {
            self.log_capacity = 8;
        }
        if self.starting_rounds > 999 {
            self.starting_rounds = 999;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_clamps_tick() {
        let c = Config {
            tick_ms: 1,
            ..Config::default()
        }
        .validate();
        assert_eq!(c.tick_ms, 16);
    }

    #[test]
    fn theme_parse() {
        assert_eq!(Theme::parse("yellow"), Some(Theme::Yellow));
        assert_eq!(Theme::parse("grid"), Some(Theme::Yellow));
        assert_eq!(Theme::parse("green"), Some(Theme::Phosphor));
        assert_eq!(Theme::parse("amber"), Some(Theme::Amber));
        assert_eq!(Theme::parse("nope"), None);
    }

    #[test]
    fn default_is_yellow() {
        assert_eq!(Config::default().theme, Theme::Yellow);
        assert_eq!(Theme::default(), Theme::Yellow);
    }
}
