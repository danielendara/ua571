//! # ua571-core
//!
//! Domain logic for an unofficial fan recreation of the UA 571-C Remote Sentry
//! Weapon System operator console from *Aliens* (1986).
//!
//! Pure Rust: no terminal or windowing dependencies. The TUI lives in `ua571-tui`.

#![forbid(unsafe_code)]

pub mod config;
pub mod demo;
pub mod fire;
pub mod log;
pub mod options;
pub mod sentry;
pub mod sfx;
pub mod state;

pub use config::{Config, Theme};
pub use demo::{default_script, DemoPlayer, DemoStep};
pub use fire::{
    FireTelemetry, CRITICAL_THRESHOLD, DEFAULT_ROUNDS, DEFAULT_TIME_CENTISECS, RM_MAX, TEMP_MAX,
};
pub use log::{EventLog, LogEvent, LogKind};
pub use options::{
    IffStatus, MenuSection, OptionsState, SpectralProfile, SystemMode, TargetProfile, TargetSelect,
    TestRoutine, WeaponStatus,
};
pub use sentry::{Sentry, SentryBank, SENTRY_COUNT};
pub use sfx::{synthesize_fire_burst, FIRE_CYCLIC_HZ, FIRE_SFX_MS, FIRE_SFX_SAMPLE_RATE};
pub use state::{AppState, Screen};
