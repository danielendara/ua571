//! Shared fire-deny classification for pixel, web, and TUI.
//!
//! `AppState::fire()` logs OFFLINE / LINK DOWN / SAFE / DRUM EMPTY / CRITICAL
//! into the event log. Frontends that show a one-shot status line (web `#status`,
//! pixel window title) should use [`fire_deny_reason`] so every console shares the
//! same reason classes and copy.

use crate::state::AppState;

/// Reason a fire attempt produced no round, or the classification to show
/// alongside a fire that just crossed into CRITICAL.
///
/// `None` covers both "not armed" (SAFE — standing chrome on every frontend) and
/// a normal, non-critical successful fire: neither needs a called-out status line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FireDenyReason {
    /// Selected sentry is powered down.
    Offline,
    /// Selected sentry is online but its datalink is faulted.
    LinkDown,
    /// Drum is dry — `fire()` returns `false` without consuming the request.
    Empty,
    /// Fire succeeded but rounds remaining are below `CRITICAL_THRESHOLD`.
    Critical,
}

impl FireDenyReason {
    /// Stable status/log copy, shared across every frontend.
    pub const fn message(self) -> &'static str {
        match self {
            FireDenyReason::Offline => "OFFLINE",
            FireDenyReason::LinkDown => "LINK DOWN",
            FireDenyReason::Empty => "EMPTY",
            FireDenyReason::Critical => "CRITICAL",
        }
    }
}

/// Classify the active sentry's current fire status for a one-shot status line.
///
/// Call once *before* [`AppState::fire`] for the deny reason when it returns
/// `false`, and again *after* a successful fire to detect a fresh CRITICAL crossing.
pub fn fire_deny_reason(state: &AppState) -> Option<FireDenyReason> {
    let s = state.active_sentry();
    if !s.online {
        Some(FireDenyReason::Offline)
    } else if !s.link_ok {
        Some(FireDenyReason::LinkDown)
    } else if !s.is_armed() {
        None
    } else if s.fire.rounds == 0 {
        Some(FireDenyReason::Empty)
    } else if s.fire.critical {
        Some(FireDenyReason::Critical)
    } else {
        None
    }
}

/// Fire once and return `(fired, status)` for a one-shot status line.
///
/// On denial, `status` is the pre-fire reason (state unchanged). On success,
/// `status` is post-fire classification — currently CRITICAL when the shot crossed
/// the threshold — or `None` for an ordinary fire.
pub fn fire_with_status(state: &mut AppState) -> (bool, Option<FireDenyReason>) {
    let before = fire_deny_reason(state);
    let fired = state.fire();
    let status = if fired {
        fire_deny_reason(state)
    } else {
        before
    };
    (fired, status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn fresh() -> AppState {
        AppState::new(Config {
            show_boot: false,
            ..Config::default()
        })
    }

    #[test]
    fn message_mapping_is_stable() {
        assert_eq!(FireDenyReason::Offline.message(), "OFFLINE");
        assert_eq!(FireDenyReason::LinkDown.message(), "LINK DOWN");
        assert_eq!(FireDenyReason::Empty.message(), "EMPTY");
        assert_eq!(FireDenyReason::Critical.message(), "CRITICAL");
    }

    #[test]
    fn armed_ready_fire_has_no_reason() {
        let mut app = fresh();
        app.toggle_arm();
        assert_eq!(fire_deny_reason(&app), None);
    }

    #[test]
    fn unarmed_is_none_not_a_reason() {
        // SAFE is standing chrome; not a "reason".
        let app = fresh();
        assert!(!app.active_sentry().is_armed());
        assert_eq!(fire_deny_reason(&app), None);
    }

    #[test]
    fn offline_takes_priority() {
        let mut app = fresh();
        app.toggle_arm();
        app.active_sentry_mut().unwrap().online = false;
        app.active_sentry_mut().unwrap().link_ok = false;
        assert_eq!(fire_deny_reason(&app), Some(FireDenyReason::Offline));
    }

    #[test]
    fn link_down_when_online_but_faulted() {
        let mut app = fresh();
        app.toggle_arm();
        app.active_sentry_mut().unwrap().link_ok = false;
        assert_eq!(fire_deny_reason(&app), Some(FireDenyReason::LinkDown));
    }

    #[test]
    fn empty_when_armed_and_linked_but_dry() {
        let mut app = fresh();
        app.toggle_arm();
        app.active_sentry_mut().unwrap().fire.rounds = 0;
        assert_eq!(fire_deny_reason(&app), Some(FireDenyReason::Empty));
    }

    #[test]
    fn critical_when_low_rounds() {
        let mut app = fresh();
        app.toggle_arm();
        app.active_sentry_mut()
            .unwrap()
            .fire
            .reset(crate::fire::CRITICAL_THRESHOLD - 1);
        assert_eq!(fire_deny_reason(&app), Some(FireDenyReason::Critical));
    }

    #[test]
    fn matches_app_state_fire_outcomes() {
        // Same scenarios as `AppState::fire` tests so the classifier cannot drift.
        let mut app = fresh();
        app.toggle_arm();
        app.active_sentry_mut().unwrap().online = false;
        assert_eq!(fire_deny_reason(&app), Some(FireDenyReason::Offline));
        assert!(!app.fire());

        app.active_sentry_mut().unwrap().online = true;
        app.active_sentry_mut().unwrap().fire.rounds = 0;
        assert_eq!(fire_deny_reason(&app), Some(FireDenyReason::Empty));
        assert!(!app.fire());
    }

    #[test]
    fn fire_with_status_reports_deny_reason_and_leaves_ammo_untouched() {
        let mut app = fresh();
        app.toggle_arm();
        app.active_sentry_mut().unwrap().link_ok = false;
        let rounds_before = app.fire_telemetry().rounds;

        let (fired, status) = fire_with_status(&mut app);

        assert!(!fired);
        assert_eq!(status, Some(FireDenyReason::LinkDown));
        assert_eq!(
            app.fire_telemetry().rounds,
            rounds_before,
            "denied fire must not spend ammo"
        );
    }

    #[test]
    fn fire_with_status_is_silent_on_a_normal_successful_fire() {
        let mut app = fresh();
        app.toggle_arm();

        let (fired, status) = fire_with_status(&mut app);

        assert!(fired);
        assert_eq!(
            status, None,
            "armed/successful fire should not add status noise"
        );
    }

    #[test]
    fn fire_with_status_reports_critical_on_the_crossing_shot() {
        let mut app = fresh();
        app.toggle_arm();
        app.active_sentry_mut()
            .unwrap()
            .fire
            .reset(crate::fire::CRITICAL_THRESHOLD);

        let (fired, status) = fire_with_status(&mut app);

        assert!(fired);
        assert_eq!(status, Some(FireDenyReason::Critical));
    }

    #[test]
    fn fire_with_status_reports_empty_without_double_counting() {
        let mut app = fresh();
        app.toggle_arm();
        app.active_sentry_mut().unwrap().fire.rounds = 0;

        let (fired, status) = fire_with_status(&mut app);

        assert!(!fired);
        assert_eq!(status, Some(FireDenyReason::Empty));
    }
}
