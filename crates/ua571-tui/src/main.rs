//! UA 571-C Remote Sentry Weapon System — terminal operator console.

mod app;
mod theme;
mod views;

use std::fs;
use std::path::PathBuf;

use clap::Parser;
use color_eyre::eyre::{eyre, Result, WrapErr};
use ua571_core::{Config, Theme};

use app::App;

#[derive(Debug, Parser)]
#[command(
    name = "ua571",
    about = "UA 571-C Remote Sentry Weapon System console (unofficial fan recreation)",
    long_about = "Terminal recreation of the Colonial Marines UA 571-C operator console \
from Aliens (1986). Unofficial fan project — not affiliated with franchise rights holders."
)]
struct Cli {
    /// Color theme: phosphor | amber | mono
    #[arg(short, long, default_value = "phosphor")]
    theme: String,

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

    /// Path to TOML config file
    #[arg(short, long)]
    config: Option<PathBuf>,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    let config = load_config(&cli)?;
    let mut app = App::new(config);
    app.run()
}

fn load_config(cli: &Cli) -> Result<Config> {
    let mut config = if let Some(path) = &cli.config {
        load_toml(path)?
    } else if let Some(path) = default_config_path() {
        if path.exists() {
            load_toml(&path)?
        } else {
            Config::default()
        }
    } else {
        Config::default()
    };

    let theme = Theme::parse(&cli.theme).ok_or_else(|| {
        eyre!(
            "unknown theme '{}': use phosphor, amber, or mono",
            cli.theme
        )
    })?;
    config.theme = theme;

    if let Some(r) = cli.rounds {
        config.starting_rounds = r;
    }
    if let Some(t) = cli.tick_ms {
        config.tick_ms = t;
    }
    if cli.no_boot {
        config.show_boot = false;
    }
    if cli.demo {
        config.demo_on_start = true;
    }

    Ok(config.validate())
}

fn load_toml(path: &PathBuf) -> Result<Config> {
    let text = fs::read_to_string(path)
        .wrap_err_with(|| format!("failed to read config {}", path.display()))?;
    let config: Config = toml::from_str(&text)
        .wrap_err_with(|| format!("failed to parse config {}", path.display()))?;
    Ok(config)
}

fn default_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("ua571").join("config.toml"))
}
