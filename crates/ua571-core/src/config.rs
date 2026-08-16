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
    pub const ALL: [Theme; 4] = [Theme::Yellow, Theme::Phosphor, Theme::Amber, Theme::Mono];

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

    /// Phosphor-on RGB+A for canvas / Web Audio frontends.
    pub fn on_rgba(self) -> [u8; 4] {
        match self {
            Theme::Yellow => [0xff, 0xee, 0x00, 0xff],
            Theme::Phosphor => [0x50, 0xfa, 0x7b, 0xff],
            Theme::Amber => [0xff, 0xb0, 0x00, 0xff],
            Theme::Mono => [0xe0, 0xe0, 0xe0, 0xff],
        }
    }

    pub fn off_rgba(self) -> [u8; 4] {
        [0x00, 0x00, 0x00, 0xff]
    }

    /// 0x00RRGGBB for minifb.
    pub fn on_rgb_u32(self) -> u32 {
        let [r, g, b, _] = self.on_rgba();
        (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
    }

    pub fn off_rgb_u32(self) -> u32 {
        0
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
    /// Play fire SFX when a round is expended (frontends may still mute).
    pub sound: bool,
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
            sound: true,
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

/// CLI overrides for native frontends (TUI / pixel).
///
/// `theme` is `None` unless `-t/--theme` was passed, so a config-file theme is
/// not overwritten by clap's yellow default (#17).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Default)]
pub struct NativeCli {
    pub theme: Option<String>,
    pub rounds: Option<u16>,
    pub tick_ms: Option<u64>,
    pub no_boot: bool,
    pub demo: bool,
    pub mute: bool,
    pub config: Option<std::path::PathBuf>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub enum ConfigLoadError {
    Read {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: std::path::PathBuf,
        source: toml::de::Error,
    },
    UnknownTheme(String),
}

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigLoadError::Read { path, source } => {
                write!(f, "failed to read config {}: {source}", path.display())
            }
            ConfigLoadError::Parse { path, source } => {
                write!(f, "failed to parse config {}: {source}", path.display())
            }
            ConfigLoadError::UnknownTheme(t) => {
                write!(
                    f,
                    "unknown theme '{t}': use yellow, phosphor, amber, or mono"
                )
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl std::error::Error for ConfigLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigLoadError::Read { source, .. } => Some(source),
            ConfigLoadError::Parse { source, .. } => Some(source),
            ConfigLoadError::UnknownTheme(_) => None,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn default_config_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("ua571").join("config.toml"))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_toml_config(path: &std::path::Path) -> Result<Config, ConfigLoadError> {
    let text = std::fs::read_to_string(path).map_err(|source| ConfigLoadError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&text).map_err(|source| ConfigLoadError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

/// Load TOML (explicit path, else `~/.config/ua571/config.toml`), then apply CLI overrides.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_native_config(cli: &NativeCli) -> Result<Config, ConfigLoadError> {
    let mut config = if let Some(path) = &cli.config {
        load_toml_config(path)?
    } else if let Some(path) = default_config_path() {
        if path.exists() {
            load_toml_config(&path)?
        } else {
            Config::default()
        }
    } else {
        Config::default()
    };

    if let Some(theme) = &cli.theme {
        config.theme =
            Theme::parse(theme).ok_or_else(|| ConfigLoadError::UnknownTheme(theme.clone()))?;
    }
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
    if cli.mute {
        config.sound = false;
    }

    Ok(config.validate())
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

    #[test]
    fn validate_clamps_rounds_capacity_and_high_tick() {
        let c = Config {
            starting_rounds: 5000,
            log_capacity: 2,
            tick_ms: 9_000,
            ..Config::default()
        }
        .validate();
        assert_eq!(c.starting_rounds, 999);
        assert_eq!(c.log_capacity, 8);
        assert_eq!(c.tick_ms, 1000);
    }

    #[test]
    fn theme_next_cycles_all() {
        let mut t = Theme::Yellow;
        let mut seen = vec![t];
        for _ in 0..3 {
            t = t.next();
            seen.push(t);
        }
        assert_eq!(
            seen,
            vec![Theme::Yellow, Theme::Phosphor, Theme::Amber, Theme::Mono]
        );
        assert_eq!(t.next(), Theme::Yellow);
        assert_eq!(Theme::Amber.as_str(), "amber");
    }

    #[test]
    fn theme_rgba_matches_pixel_u32() {
        let [r, g, b, a] = Theme::Yellow.on_rgba();
        assert_eq!(a, 0xff);
        assert_eq!(Theme::Yellow.on_rgb_u32(), 0x00_FF_EE_00);
        assert_eq!(
            Theme::Yellow.on_rgb_u32(),
            (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn toml_theme_kept_when_cli_theme_absent() {
        let dir = std::env::temp_dir().join(format!("ua571-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "theme = \"amber\"\n").unwrap();
        let cfg = load_native_config(&NativeCli {
            config: Some(path),
            ..NativeCli::default()
        })
        .unwrap();
        assert_eq!(cfg.theme, Theme::Amber);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn cli_theme_overrides_toml() {
        let dir = std::env::temp_dir().join(format!("ua571-cfg-cli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "theme = \"amber\"\n").unwrap();
        let cfg = load_native_config(&NativeCli {
            config: Some(path),
            theme: Some("mono".into()),
            ..NativeCli::default()
        })
        .unwrap();
        assert_eq!(cfg.theme, Theme::Mono);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn cli_flags_apply_without_config_file() {
        let cfg = load_native_config(&NativeCli {
            rounds: Some(42),
            tick_ms: Some(50),
            no_boot: true,
            demo: true,
            mute: true,
            ..NativeCli::default()
        })
        .unwrap();
        assert_eq!(cfg.starting_rounds, 42);
        assert_eq!(cfg.tick_ms, 50);
        assert!(!cfg.show_boot);
        assert!(cfg.demo_on_start);
        assert!(!cfg.sound);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn unknown_theme_is_error() {
        let err = load_native_config(&NativeCli {
            theme: Some("octarine".into()),
            ..NativeCli::default()
        })
        .unwrap_err();
        assert!(err.to_string().contains("octarine"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn missing_toml_is_read_error() {
        let path = std::env::temp_dir().join("ua571-definitely-missing.toml");
        let _ = std::fs::remove_file(&path);
        let err = load_toml_config(&path).unwrap_err();
        assert!(matches!(err, ConfigLoadError::Read { .. }));
    }
}
