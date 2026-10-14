//! Shared Esc vs O panel semantics for pixel, web, and TUI.
//!
//! Frontends must call [`apply_command`] instead of inlining `stop_demo` +
//! action pairs so the three UIs cannot drift.

use crate::fire_deny::{fire_with_status, FireDenyReason};
use crate::state::{AppState, Screen};

/// Operator keys that switch between Options and the firing panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKey {
    /// Letter O — always open Options (stays on Options if already there).
    OpenOptions,
    /// Escape — toggle Fire ↔ Options via [`AppState::toggle_fire_panel`].
    ToggleFirePanel,
}

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
        Command::SectionPrev | Command::SectionNext | Command::SelectUp | Command::SelectDown
            if state.screen != Screen::Options =>
        {
            return None;
        }
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

/// Stop demo and apply Esc vs O. Thin wrapper over [`apply_command`].
pub fn apply_panel_key(state: &mut AppState, key: PanelKey) {
    apply_command(
        state,
        match key {
            PanelKey::OpenOptions => Command::OpenOptions,
            PanelKey::ToggleFirePanel => Command::ToggleFirePanel,
        },
    );
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
        apply_panel_key(&mut state, PanelKey::OpenOptions);
        assert_eq!(state.screen, Screen::Options);
        state.set_screen(Screen::Fire);
        apply_panel_key(&mut state, PanelKey::OpenOptions);
        assert_eq!(state.screen, Screen::Options);
        apply_panel_key(&mut state, PanelKey::OpenOptions);
        assert_eq!(state.screen, Screen::Options);
    }

    #[test]
    fn esc_toggles_fire_and_options() {
        let mut state = app();
        assert_eq!(state.screen, Screen::Options);
        apply_panel_key(&mut state, PanelKey::ToggleFirePanel);
        assert_eq!(state.screen, Screen::Fire);
        apply_panel_key(&mut state, PanelKey::ToggleFirePanel);
        assert_eq!(state.screen, Screen::Options);
    }

    #[test]
    fn both_keys_stop_demo() {
        let mut state = app();
        state.toggle_demo();
        assert!(state.demo.is_active());
        apply_panel_key(&mut state, PanelKey::OpenOptions);
        assert!(!state.demo.is_active());

        state.toggle_demo();
        assert!(state.demo.is_active());
        apply_panel_key(&mut state, PanelKey::ToggleFirePanel);
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
}
