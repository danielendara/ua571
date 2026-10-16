//! Shared Esc vs O panel semantics for pixel, web, and TUI.
//!
//! Frontends must call [`apply_command`] instead of inlining `stop_demo` +
//! action pairs so the three UIs cannot drift.

use crate::fire_deny::{fire_with_status, FireDenyReason};
use crate::state::{AppState, Screen};

/// Frontend-neutral operator action. Each frontend maps its own raw keys to a
/// [`Command`] and calls [`apply_command`]; theme (T) stays frontend-owned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Quit,
    ToggleDemo,
    ToggleSound,
    /// Letter F — show the firing panel.
    OpenFirePanel,
    /// Letter O — always open Options.
    OpenOptions,
    /// Escape — toggle Fire ↔ Options.
    ToggleFirePanel,
    ToggleArm,
    Reload,
    /// Select sentry by zero-based index (out of range is a no-op).
    SelectSentry(usize),
    SectionPrev,
    SectionNext,
    SelectUp,
    SelectDown,
    /// Enter / Space — fire on Fire, go to Fire from Options.
    Confirm,
}

impl Command {
    /// Whether [`apply_command`] ignores this command on `screen`: Options-only
    /// navigation anywhere else. Frontends can skip such a key without
    /// treating it as handled, so a later key in the same frame still acts.
    pub fn is_noop_on(self, screen: Screen) -> bool {
        matches!(
            self,
            Command::SectionPrev | Command::SectionNext | Command::SelectUp | Command::SelectDown
        ) && screen != Screen::Options
    }
}

/// Apply one operator command. Every command except
/// Quit/ToggleDemo/ToggleSound stops demo first; Options-only navigation is a
/// no-op elsewhere. Returns the deny reason when a Confirm on Fire was denied.
/// Callers should skip POST before this.
pub fn apply_command(state: &mut AppState, cmd: Command) -> Option<FireDenyReason> {
    match cmd {
        Command::Quit => {
            state.quit();
            return None;
        }
        Command::ToggleDemo => {
            state.toggle_demo();
            return None;
        }
        Command::ToggleSound => {
            state.toggle_sound();
            return None;
        }
        // Navigation is Options-only and a no-op elsewhere: it must not
        // stop the demo there either.
        _ if cmd.is_noop_on(state.screen) => return None,
        _ => state.stop_demo(),
    }
    match cmd {
        Command::OpenFirePanel => state.set_screen(Screen::Fire),
        Command::OpenOptions => state.set_screen(Screen::Options),
        Command::ToggleFirePanel => state.toggle_fire_panel(),
        Command::ToggleArm => state.toggle_arm(),
        Command::Reload => state.reload(),
        Command::SelectSentry(i) => state.select_sentry(i),
        Command::SectionPrev => state.focus_prev_section(),
        Command::SectionNext => state.focus_next_section(),
        Command::SelectUp => state.select_up(),
        Command::SelectDown => state.select_down(),
        Command::Confirm => match state.screen {
            Screen::Fire => return fire_with_status(state).1,
            Screen::Options => state.set_screen(Screen::Fire),
            Screen::Boot => {}
        },
        _ => {}
    }
    None
}

/// Frontend-neutral physical operator key. Each frontend translates its raw
/// key (crossterm `KeyCode`, DOM `KeyboardEvent.code`, minifb `Key`) into this and asks
/// [`command_for`], so the key → [`Command`] table lives in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorKey {
    /// A letter key; matching is ASCII case-insensitive.
    Letter(char),
    /// A digit key (top row or numpad), `0..=9`.
    Digit(u8),
    Left,
    Right,
    Up,
    Down,
    /// Enter, including the numpad one.
    Enter,
    Space,
    Escape,
}

/// The [`Command`] for an operator key, or `None` when the key is unbound.
/// T (theme) is frontend-owned and returns `None` here.
pub fn command_for(key: OperatorKey) -> Option<Command> {
    Some(match key {
        OperatorKey::Letter(c) => match c.to_ascii_lowercase() {
            'q' => Command::Quit,
            'd' => Command::ToggleDemo,
            'm' => Command::ToggleSound,
            'f' => Command::OpenFirePanel,
            'o' => Command::OpenOptions,
            'a' => Command::ToggleArm,
            'r' => Command::Reload,
            'h' => Command::SectionPrev,
            'l' => Command::SectionNext,
            'k' => Command::SelectUp,
            'j' => Command::SelectDown,
            _ => return None,
        },
        OperatorKey::Digit(d @ 1..=4) => Command::SelectSentry(usize::from(d) - 1),
        OperatorKey::Digit(_) => return None,
        OperatorKey::Left => Command::SectionPrev,
        OperatorKey::Right => Command::SectionNext,
        OperatorKey::Up => Command::SelectUp,
        OperatorKey::Down => Command::SelectDown,
        OperatorKey::Enter | OperatorKey::Space => Command::Confirm,
        OperatorKey::Escape => Command::ToggleFirePanel,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn app() -> AppState {
        AppState::new(Config {
            show_boot: false,
            ..Config::default()
        })
    }

    #[test]
    fn o_opens_options_and_does_not_leave() {
        let mut state = app();
        assert_eq!(state.screen, Screen::Options);
        apply_command(&mut state, Command::OpenOptions);
        assert_eq!(state.screen, Screen::Options);
        state.set_screen(Screen::Fire);
        apply_command(&mut state, Command::OpenOptions);
        assert_eq!(state.screen, Screen::Options);
        apply_command(&mut state, Command::OpenOptions);
        assert_eq!(state.screen, Screen::Options);
    }

    #[test]
    fn esc_toggles_fire_and_options() {
        let mut state = app();
        assert_eq!(state.screen, Screen::Options);
        apply_command(&mut state, Command::ToggleFirePanel);
        assert_eq!(state.screen, Screen::Fire);
        apply_command(&mut state, Command::ToggleFirePanel);
        assert_eq!(state.screen, Screen::Options);
    }

    #[test]
    fn both_keys_stop_demo() {
        let mut state = app();
        state.toggle_demo();
        assert!(state.demo.is_active());
        apply_command(&mut state, Command::OpenOptions);
        assert!(!state.demo.is_active());

        state.toggle_demo();
        assert!(state.demo.is_active());
        apply_command(&mut state, Command::ToggleFirePanel);
        assert!(!state.demo.is_active());
        assert_eq!(state.screen, Screen::Fire);
    }

    fn armed() -> AppState {
        let mut state = app();
        state.toggle_arm();
        state.set_screen(Screen::Fire);
        state
    }

    #[test]
    fn demo_preserving_commands_keep_demo_running() {
        let mut state = app();
        state.toggle_demo();
        apply_command(&mut state, Command::ToggleSound);
        assert!(state.demo.is_active());
        let mut state = app();
        apply_command(&mut state, Command::ToggleDemo);
        assert!(state.demo.is_active());
        apply_command(&mut state, Command::ToggleDemo);
        assert!(!state.demo.is_active());
    }

    #[test]
    fn quit_sets_flag_and_keeps_demo() {
        let mut state = app();
        state.toggle_demo();
        assert_eq!(apply_command(&mut state, Command::Quit), None);
        assert!(state.should_quit);
        assert!(state.demo.is_active());
    }

    #[test]
    fn toggle_sound_flips_config() {
        let mut state = app();
        let before = state.config.sound;
        apply_command(&mut state, Command::ToggleSound);
        assert_eq!(state.config.sound, !before);
    }

    #[test]
    fn demo_stopping_commands_stop_demo() {
        for cmd in [
            Command::OpenFirePanel,
            Command::OpenOptions,
            Command::ToggleFirePanel,
            Command::ToggleArm,
            Command::Reload,
            Command::SelectSentry(1),
            Command::Confirm,
        ] {
            for screen in [Screen::Options, Screen::Fire] {
                let mut state = app();
                state.set_screen(screen);
                state.toggle_demo();
                assert!(state.demo.is_active());
                apply_command(&mut state, cmd);
                assert!(!state.demo.is_active(), "{cmd:?} on {screen:?}");
            }
        }
    }

    #[test]
    fn nav_commands_stop_demo_only_on_options() {
        for cmd in [
            Command::SectionPrev,
            Command::SectionNext,
            Command::SelectUp,
            Command::SelectDown,
        ] {
            let mut state = app();
            state.set_screen(Screen::Options);
            state.toggle_demo();
            apply_command(&mut state, cmd);
            assert!(!state.demo.is_active(), "{cmd:?} on Options");

            let mut state = app();
            state.set_screen(Screen::Fire);
            state.toggle_demo();
            apply_command(&mut state, cmd);
            assert!(state.demo.is_active(), "{cmd:?} on Fire must not stop demo");
        }
    }

    #[test]
    fn open_fire_panel_goes_to_fire() {
        let mut state = app();
        apply_command(&mut state, Command::OpenFirePanel);
        assert_eq!(state.screen, Screen::Fire);
        apply_command(&mut state, Command::OpenFirePanel);
        assert_eq!(state.screen, Screen::Fire);
    }

    #[test]
    fn arm_and_reload_apply() {
        let mut state = app();
        apply_command(&mut state, Command::ToggleArm);
        assert!(state.active_sentry().is_armed());
        apply_command(&mut state, Command::ToggleArm);
        assert!(!state.active_sentry().is_armed());
        state.active_sentry_mut().unwrap().fire.rounds = 3;
        apply_command(&mut state, Command::Reload);
        assert_eq!(state.fire_telemetry().rounds, state.config.starting_rounds);
    }

    #[test]
    fn select_sentry_in_range_and_oob() {
        let mut state = app();
        apply_command(&mut state, Command::SelectSentry(2));
        assert_eq!(state.active_sentry().id, 3);
        apply_command(&mut state, Command::SelectSentry(99));
        assert_eq!(state.active_sentry().id, 3);
    }

    #[test]
    fn nav_commands_move_on_options() {
        for cmd in [
            Command::SectionPrev,
            Command::SectionNext,
            Command::SelectUp,
            Command::SelectDown,
        ] {
            let mut state = app();
            let before = state.active_sentry().options;
            apply_command(&mut state, cmd);
            assert_ne!(state.active_sentry().options, before, "{cmd:?}");
        }
    }

    #[test]
    fn nav_commands_ignored_on_fire() {
        for cmd in [
            Command::SectionPrev,
            Command::SectionNext,
            Command::SelectUp,
            Command::SelectDown,
        ] {
            let mut state = app();
            state.set_screen(Screen::Fire);
            let before = state.active_sentry().options;
            assert_eq!(apply_command(&mut state, cmd), None);
            assert_eq!(state.active_sentry().options, before, "{cmd:?}");
            assert_eq!(state.screen, Screen::Fire);
        }
    }

    #[test]
    fn confirm_on_options_goes_to_fire_without_firing() {
        let mut state = app();
        state.toggle_arm();
        let rounds = state.fire_telemetry().rounds;
        assert_eq!(apply_command(&mut state, Command::Confirm), None);
        assert_eq!(state.screen, Screen::Fire);
        assert_eq!(state.fire_telemetry().rounds, rounds);
    }

    #[test]
    fn confirm_on_fire_matches_fire_with_status() {
        let mut state = armed();
        let rounds = state.fire_telemetry().rounds;
        assert_eq!(apply_command(&mut state, Command::Confirm), None);
        assert_eq!(state.fire_telemetry().rounds, rounds - 1);

        let mut state = armed();
        state.active_sentry_mut().unwrap().link_ok = false;
        assert_eq!(
            apply_command(&mut state, Command::Confirm),
            Some(FireDenyReason::LinkDown)
        );

        let mut state = armed();
        state.active_sentry_mut().unwrap().fire.rounds = 0;
        assert_eq!(
            apply_command(&mut state, Command::Confirm),
            Some(FireDenyReason::Empty)
        );

        let mut state = armed();
        state.active_sentry_mut().unwrap().fire.reset(5);
        assert_eq!(
            apply_command(&mut state, Command::Confirm),
            Some(FireDenyReason::Critical)
        );
    }

    #[test]
    fn navigation_is_noop_off_options_only() {
        let nav = [
            Command::SectionPrev,
            Command::SectionNext,
            Command::SelectUp,
            Command::SelectDown,
        ];
        for cmd in nav {
            assert!(cmd.is_noop_on(Screen::Fire), "{cmd:?} on Fire");
            assert!(cmd.is_noop_on(Screen::Boot), "{cmd:?} on Boot");
            assert!(!cmd.is_noop_on(Screen::Options), "{cmd:?} on Options");
        }
        for cmd in [Command::Confirm, Command::ToggleArm, Command::Quit] {
            assert!(!cmd.is_noop_on(Screen::Fire), "{cmd:?}");
        }
    }

    #[test]
    fn nav_key_before_confirm_does_not_shadow_fire() {
        // Mirrors a frontend frame loop: skip no-op keys, act on the first rest.
        let mut state = app();
        state.set_screen(Screen::Fire);
        let mut handled = None;
        for key in [
            OperatorKey::Down,
            OperatorKey::Letter('h'),
            OperatorKey::Space,
        ] {
            let cmd = command_for(key).unwrap();
            if cmd.is_noop_on(state.screen) {
                continue;
            }
            handled = Some(cmd);
            break;
        }
        assert_eq!(handled, Some(Command::Confirm));
    }

    #[test]
    fn command_for_maps_every_bound_key() {
        use OperatorKey::*;
        let table: &[(OperatorKey, Option<Command>)] = &[
            (Letter('q'), Some(Command::Quit)),
            (Letter('d'), Some(Command::ToggleDemo)),
            (Letter('m'), Some(Command::ToggleSound)),
            (Letter('f'), Some(Command::OpenFirePanel)),
            (Letter('o'), Some(Command::OpenOptions)),
            (Letter('a'), Some(Command::ToggleArm)),
            (Letter('r'), Some(Command::Reload)),
            (Letter('h'), Some(Command::SectionPrev)),
            (Letter('l'), Some(Command::SectionNext)),
            (Letter('k'), Some(Command::SelectUp)),
            (Letter('j'), Some(Command::SelectDown)),
            (Digit(1), Some(Command::SelectSentry(0))),
            (Digit(2), Some(Command::SelectSentry(1))),
            (Digit(3), Some(Command::SelectSentry(2))),
            (Digit(4), Some(Command::SelectSentry(3))),
            (Left, Some(Command::SectionPrev)),
            (Right, Some(Command::SectionNext)),
            (Up, Some(Command::SelectUp)),
            (Down, Some(Command::SelectDown)),
            (Enter, Some(Command::Confirm)),
            (Space, Some(Command::Confirm)),
            (Escape, Some(Command::ToggleFirePanel)),
        ];
        for (key, expected) in table {
            assert_eq!(command_for(*key), *expected, "{key:?}");
            if let Letter(c) = key {
                assert_eq!(
                    command_for(Letter(c.to_ascii_uppercase())),
                    *expected,
                    "uppercase {key:?}"
                );
            }
        }
    }

    #[test]
    fn command_for_leaves_theme_and_unbound_keys_to_the_frontend() {
        use OperatorKey::*;
        // T is frontend-owned; the rest are simply unbound.
        for c in ['t', 'T', 'z', 'Z', 'b', 'é', ' ', '1'] {
            assert_eq!(command_for(Letter(c)), None, "{c:?}");
        }
        for d in [0u8, 5, 6, 7, 8, 9, 10, 255] {
            assert_eq!(command_for(Digit(d)), None, "Digit({d})");
        }
    }
}
