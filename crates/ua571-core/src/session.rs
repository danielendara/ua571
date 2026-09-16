//! Saved console session for the native frontends (#109).
//!
//! The web frontend already remembers how you left the console — theme, sound,
//! skip-boot, and weapon / IFF / system mode — in localStorage. TUI and pixel
//! read a config file at startup and then never wrote anything back, so every
//! `T` theme cycle and Options change was thrown away on exit.
//!
//! This is that memory, for native: one small file under the platform config
//! dir, written atomically on a clean exit, separate from the config file the
//! operator hand-writes. Saved state never rewrites their config.
//!
//! Precedence, extending #17's rule: **CLI flag → saved session → config file →
//! built-in defaults.**

use crate::config::Theme;
use crate::options::{IffStatus, SystemMode, WeaponStatus};

/// What a console remembers between runs. Deliberately small: look and switch
/// positions, never simulation state (rounds, damage, demo progress).
///
/// Every field is optional because a field the session does not carry must fall
/// through to the config file rather than stamping a default over it. A session
/// this program wrote is always complete; a hand-mangled one may not be.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionPrefs {
    pub theme: Option<Theme>,
    pub sound: Option<bool>,
    pub system_mode: Option<SystemMode>,
    pub weapon_status: Option<WeaponStatus>,
    pub iff_status: Option<IffStatus>,
}

impl SessionPrefs {
    /// Serializes to TOML by hand: the wire names have to match what the web
    /// frontend stores, and hand-rolling is cheaper than teaching serde three
    /// remote-enum shims for five fields.
    pub fn to_toml(&self) -> String {
        let mut out =
            String::from("# Written by ua571 on exit. Hand edits belong in config.toml.\n");
        if let Some(theme) = self.theme {
            out.push_str(&format!("theme = \"{}\"\n", theme.as_str()));
        }
        if let Some(sound) = self.sound {
            out.push_str(&format!("sound = {sound}\n"));
        }
        if let Some(mode) = self.system_mode {
            out.push_str(&format!("system_mode = \"{}\"\n", mode.wire_name()));
        }
        if let Some(weapon) = self.weapon_status {
            out.push_str(&format!("weapon_status = \"{}\"\n", weapon.wire_name()));
        }
        if let Some(iff) = self.iff_status {
            out.push_str(&format!("iff_status = \"{}\"\n", iff.wire_name()));
        }
        out
    }

    /// Parses a saved session. Any field that is missing, misspelled, or the
    /// wrong type falls back to its default — a partially-broken file degrades
    /// field by field instead of taking the console down with it.
    pub fn from_toml(text: &str) -> Option<Self> {
        let value: toml::Value = toml::from_str(text).ok()?;
        let table = value.as_table()?;

        let string_field = |key: &str| table.get(key).and_then(|v| v.as_str());

        Some(Self {
            // An unknown theme name is dropped rather than defaulted, so the
            // config file still gets its say.
            theme: string_field("theme").and_then(Theme::parse),
            sound: table.get("sound").and_then(|v| v.as_bool()),
            system_mode: string_field("system_mode").map(SystemMode::parse_wire_name),
            weapon_status: string_field("weapon_status").map(WeaponStatus::parse_wire_name),
            iff_status: string_field("iff_status").map(IffStatus::parse_wire_name),
        })
    }
}

/// Everything a native frontend needs at startup: the resolved config, the
/// Options-panel positions to restore, where to write on exit, and one line to
/// say what happened.
#[derive(Debug, Clone)]
pub struct NativeStartup {
    pub config: crate::config::Config,
    pub system_mode: Option<SystemMode>,
    pub weapon_status: Option<WeaponStatus>,
    pub iff_status: Option<IffStatus>,
    /// `None` when saving is disabled (`--no-save-session`) or no config dir exists.
    pub session_path: Option<std::path::PathBuf>,
    /// Status-line / log line, in the voice of the existing theme-cycle logging.
    pub notice: Option<String>,
}

/// Resolves startup state with the full precedence chain (#109):
/// **CLI flag → saved session → config file → built-in defaults.**
///
/// A missing session file is silent; an unreadable or unparsable one is ignored
/// with a notice, never a failed boot.
pub fn load_native_startup(
    cli: &crate::config::NativeCli,
) -> Result<NativeStartup, crate::config::ConfigLoadError> {
    let session_path = if cli.no_save_session {
        None
    } else {
        default_session_path()
    };

    let (session, notice) = match &session_path {
        Some(path) if path.exists() => match load_session(path) {
            Some(prefs) => {
                let note = format!("SESSION RESTORED FROM {}", path.display());
                (Some(prefs), Some(note))
            }
            None => (
                None,
                Some(format!(
                    "SESSION FILE IGNORED (UNREADABLE): {}",
                    path.display()
                )),
            ),
        },
        _ => (None, None),
    };

    let config = compose_config(crate::config::load_config_file(cli)?, session.as_ref(), cli)?;

    Ok(NativeStartup {
        config,
        system_mode: session.as_ref().and_then(|p| p.system_mode),
        weapon_status: session.as_ref().and_then(|p| p.weapon_status),
        iff_status: session.as_ref().and_then(|p| p.iff_status),
        session_path,
        notice,
    })
}

/// The precedence chain itself, free of the filesystem so each rung is testable:
/// config file (already loaded) → saved session → CLI flags → clamp.
pub fn compose_config(
    mut config: crate::config::Config,
    session: Option<&SessionPrefs>,
    cli: &crate::config::NativeCli,
) -> Result<crate::config::Config, crate::config::ConfigLoadError> {
    if let Some(prefs) = session {
        if let Some(theme) = prefs.theme {
            config.theme = theme;
        }
        if let Some(sound) = prefs.sound {
            config.sound = sound;
        }
    }
    // The CLI always wins, and only for flags actually passed (#17).
    crate::config::apply_cli_overrides(&mut config, cli)?;
    Ok(config.validate())
}

impl NativeStartup {
    /// Applies the restored Options-panel positions to every sentry and logs the
    /// notice once, in the voice of the existing theme-cycle logging (#98).
    pub fn apply(&self, state: &mut crate::state::AppState) {
        for sentry in state.bank.iter_mut() {
            if let Some(mode) = self.system_mode {
                sentry.options.system_mode = mode;
            }
            if let Some(weapon) = self.weapon_status {
                sentry.options.weapon_status = weapon;
            }
            if let Some(iff) = self.iff_status {
                sentry.options.iff_status = iff;
            }
        }
        if let Some(notice) = &self.notice {
            state.log.push_info(notice.clone());
        }
    }

    /// Writes the console's current look and switch positions, if saving is on.
    /// A failure here is a no-op with a log line, never a crash on the way out.
    pub fn save_on_exit(&self, state: &crate::state::AppState) {
        let Some(path) = &self.session_path else {
            return;
        };
        let options = state.active_sentry().options;
        let prefs = SessionPrefs {
            theme: Some(state.config.theme),
            sound: Some(state.config.sound),
            system_mode: Some(options.system_mode),
            weapon_status: Some(options.weapon_status),
            iff_status: Some(options.iff_status),
        };
        if let Err(err) = save_session(path, &prefs) {
            eprintln!("ua571: could not save session to {}: {err}", path.display());
        }
    }
}

/// `~/.config/ua571/session.toml` (or the OS equivalent). Deliberately *not*
/// `config.toml`: the operator owns that file.
pub fn default_session_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("ua571").join("session.toml"))
}

/// Reads a saved session. An unreadable or unparsable file reads as "no saved
/// session" — the console still boots, on defaults.
pub fn load_session(path: &std::path::Path) -> Option<SessionPrefs> {
    let text = std::fs::read_to_string(path).ok()?;
    SessionPrefs::from_toml(&text)
}

/// Writes the session atomically (temp file + rename), so a kill mid-write
/// cannot leave a truncated file that breaks the next launch.
pub fn save_session(path: &std::path::Path, prefs: &SessionPrefs) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("toml.tmp");
    std::fs::write(&temp, prefs.to_toml())?;
    match std::fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(err) => {
            // Don't leave the temp file behind if the rename failed.
            let _ = std::fs::remove_file(&temp);
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SessionPrefs {
        SessionPrefs {
            theme: Some(Theme::Amber),
            sound: Some(true),
            system_mode: Some(SystemMode::SemiAuto),
            weapon_status: Some(WeaponStatus::Armed),
            iff_status: Some(IffStatus::Engaged),
        }
    }

    #[test]
    fn round_trips_through_toml() {
        let prefs = sample();
        assert_eq!(SessionPrefs::from_toml(&prefs.to_toml()), Some(prefs));
    }

    #[test]
    fn wire_names_match_what_the_web_frontend_stores() {
        // ua571-web writes `format!("{:?}", value)` into localStorage; native
        // must read and write the very same strings.
        for mode in SystemMode::ALL {
            assert_eq!(mode.wire_name(), format!("{mode:?}"));
            assert_eq!(SystemMode::parse_wire_name(mode.wire_name()), mode);
        }
        for weapon in WeaponStatus::ALL {
            assert_eq!(weapon.wire_name(), format!("{weapon:?}"));
            assert_eq!(WeaponStatus::parse_wire_name(weapon.wire_name()), weapon);
        }
        for iff in IffStatus::ALL {
            assert_eq!(iff.wire_name(), format!("{iff:?}"));
            assert_eq!(IffStatus::parse_wire_name(iff.wire_name()), iff);
        }
    }

    #[test]
    fn unknown_names_fall_back_to_defaults_like_the_web_does() {
        assert_eq!(
            WeaponStatus::parse_wire_name("armed"),
            WeaponStatus::default()
        );
        assert_eq!(WeaponStatus::parse_wire_name(""), WeaponStatus::default());
        assert_eq!(IffStatus::parse_wire_name("nonsense"), IffStatus::default());
    }

    #[test]
    fn a_corrupt_file_reads_as_no_session() {
        assert_eq!(SessionPrefs::from_toml("this is not toml {{{"), None);
    }

    #[test]
    fn a_partial_file_degrades_field_by_field() {
        let prefs = SessionPrefs::from_toml("theme = \"amber\"\nsound = \"yes please\"\n").unwrap();
        assert_eq!(prefs.theme, Some(Theme::Amber));
        assert_eq!(
            prefs.sound, None,
            "a wrong-typed field falls through to the config file, it doesn't fail the read"
        );
        assert_eq!(prefs.weapon_status, None);
    }

    #[test]
    fn an_unknown_theme_falls_back_rather_than_failing_the_read() {
        let prefs = SessionPrefs::from_toml("theme = \"ultraviolet\"\n").unwrap();
        assert_eq!(
            prefs.theme, None,
            "an unknown theme lets the config file decide"
        );
    }

    #[test]
    fn saving_is_atomic_and_leaves_no_temp_file_behind() {
        let dir = std::env::temp_dir().join(format!("ua571-session-test-{}", std::process::id()));
        let path = dir.join("session.toml");
        save_session(&path, &sample()).unwrap();

        assert_eq!(load_session(&path), Some(sample()));
        assert!(!path.with_extension("toml.tmp").exists());

        // An interrupted write leaves a temp file; the real file is untouched
        // until the rename, so the next launch still reads the old session.
        std::fs::write(path.with_extension("toml.tmp"), "half-written").unwrap();
        assert_eq!(load_session(&path), Some(sample()));

        std::fs::remove_dir_all(&dir).ok();
    }

    fn cli() -> crate::config::NativeCli {
        crate::config::NativeCli::default()
    }

    #[test]
    fn cli_beats_session_beats_config_file_beats_default() {
        let default_config = crate::config::Config::default();
        assert_eq!(default_config.theme, Theme::Yellow);

        // Config file rung.
        let from_file = crate::config::Config {
            theme: Theme::Phosphor,
            ..Default::default()
        };
        assert_eq!(
            compose_config(from_file.clone(), None, &cli())
                .unwrap()
                .theme,
            Theme::Phosphor
        );

        // Session rung beats the file.
        let session = SessionPrefs {
            theme: Some(Theme::Amber),
            sound: Some(true),
            ..Default::default()
        };
        let composed = compose_config(from_file.clone(), Some(&session), &cli()).unwrap();
        assert_eq!(composed.theme, Theme::Amber);
        assert!(composed.sound);

        // CLI beats the session.
        let with_flag = crate::config::NativeCli {
            theme: Some("mono".into()),
            mute: true,
            ..cli()
        };
        let composed = compose_config(from_file, Some(&session), &with_flag).unwrap();
        assert_eq!(composed.theme, Theme::Mono);
        assert!(!composed.sound, "--mute still wins over a saved sound pref");
    }

    #[test]
    fn a_session_field_that_is_absent_leaves_the_config_file_alone() {
        let from_file = crate::config::Config {
            theme: Theme::Phosphor,
            sound: true,
            ..Default::default()
        };
        let session = SessionPrefs::default();
        let composed = compose_config(from_file, Some(&session), &cli()).unwrap();
        assert_eq!(composed.theme, Theme::Phosphor);
        assert!(composed.sound);
    }

    #[test]
    fn restored_values_are_still_clamped() {
        let from_file = crate::config::Config {
            tick_ms: 1,
            starting_rounds: 5000,
            log_capacity: 2,
            ..Default::default()
        };
        let composed = compose_config(from_file, None, &cli()).unwrap();
        assert_eq!(composed.tick_ms, 16);
        assert_eq!(composed.starting_rounds, 999);
        assert_eq!(composed.log_capacity, 8);
    }

    #[test]
    fn no_save_session_means_no_path_to_write() {
        let startup = load_native_startup(&crate::config::NativeCli {
            no_save_session: true,
            ..cli()
        })
        .unwrap();
        assert!(startup.session_path.is_none());
        assert!(startup.notice.is_none());
        assert!(startup.weapon_status.is_none());
    }

    #[test]
    fn a_missing_file_is_not_an_error() {
        assert_eq!(
            load_session(std::path::Path::new("/nonexistent/ua571/session.toml")),
            None
        );
    }
}
