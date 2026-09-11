//! Shared Esc vs O panel semantics for pixel, web, and TUI.
//!
//! Frontends must call [`apply_panel_key`] instead of inlining `set_screen` /
//! `toggle_fire_panel` so the three UIs cannot drift.

use crate::state::{AppState, Screen};

/// Operator keys that switch between Options and the firing panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKey {
    /// Letter O — always open Options (stays on Options if already there).
    OpenOptions,
    /// Escape — toggle Fire ↔ Options via [`AppState::toggle_fire_panel`].
    ToggleFirePanel,
}

/// Stop demo and apply Esc vs O. Callers should skip POST before this.
pub fn apply_panel_key(state: &mut AppState, key: PanelKey) {
    state.stop_demo();
    match key {
        PanelKey::OpenOptions => state.set_screen(Screen::Options),
        PanelKey::ToggleFirePanel => state.toggle_fire_panel(),
    }
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
}
