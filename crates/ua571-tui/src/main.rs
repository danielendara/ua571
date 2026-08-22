//! UA 571-C Remote Sentry Weapon System — terminal operator console.

mod app;
mod theme;
mod views;

use std::path::PathBuf;

use clap::Parser;
use color_eyre::eyre::Result;
use ua571_core::{load_native_config, NativeCli};

use app::App;

#[derive(Debug, Parser)]
#[command(
    name = "ua571",
    version,
    about = "UA 571-C Remote Sentry Weapon System console (unofficial fan recreation)",
    long_about = "Terminal recreation of the Colonial Marines UA 571-C operator console \
from Aliens (1986). Unofficial fan project — not affiliated with franchise rights holders."
)]
struct Cli {
    /// Color theme: yellow | phosphor | amber | mono
    ///
    /// When omitted, uses the config file or the built-in yellow default.
    #[arg(short, long)]
    theme: Option<String>,

    /// Starting rounds per sentry drum
    #[arg(short, long)]
    rounds: Option<u16>,

    /// UI tick interval in milliseconds
    #[arg(long)]
    tick_ms: Option<u64>,

    /// Skip POST / boot splash
    #[arg(long)]
    no_boot: bool,

    /// Start demo auto-play after boot
    #[arg(long)]
    demo: bool,

    /// Mute fire SFX
    #[arg(long)]
    mute: bool,

    /// Path to TOML config file
    #[arg(short, long)]
    config: Option<PathBuf>,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    let config = load_native_config(&NativeCli {
        theme: cli.theme,
        rounds: cli.rounds,
        tick_ms: cli.tick_ms,
        no_boot: cli.no_boot,
        demo: cli.demo,
        mute: cli.mute,
        config: cli.config,
    })?;
    let mut app = App::new(config);
    app.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_flag_is_optional() {
        let cli = Cli::try_parse_from(["ua571"]).unwrap();
        assert!(cli.theme.is_none());
        assert!(!cli.mute);
    }

    #[test]
    fn reports_crate_version() {
        let err = Cli::try_parse_from(["ua571", "--version"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
        assert!(err.to_string().contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn theme_and_mute_parse() {
        let cli = Cli::try_parse_from(["ua571", "-t", "amber", "--mute"]).unwrap();
        assert_eq!(cli.theme.as_deref(), Some("amber"));
        assert!(cli.mute);
    }
}
