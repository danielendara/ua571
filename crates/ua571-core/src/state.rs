//! Top-level application state machine.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::demo::DemoPlayer;
use crate::fire::FireTelemetry;
use crate::log::{EventLog, LogKind};
use crate::options::{MenuSection, OptionsState, WeaponStatus};
use crate::sentry::{Sentry, SentryBank, SENTRY_COUNT};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Screen {
    #[default]
    Options,
    Fire,
    Boot,
}

impl Screen {
    pub fn label(self) -> &'static str {
        match self {
            Screen::Options => "OPTIONS",
            Screen::Fire => "FIRING PANEL",
            Screen::Boot => "BOOT",
        }
    }
}

/// Cap queued fire SFX so a lagging UI does not explode with overlapping bursts.
const MAX_PENDING_FIRE_SFX: u32 = 6;

#[derive(Debug)]
pub struct AppState {
    pub screen: Screen,
    pub active_index: usize,
    pub bank: SentryBank,
    pub log: EventLog,
    pub config: Config,
    pub demo: DemoPlayer,
    pub boot_ticks_remaining: u32,
    pub should_quit: bool,
    /// Fire sound requests for the UI to drain (demo + manual fire).
    pending_fire_sfx: u32,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let config = config.validate();
        let mut log = EventLog::new(config.log_capacity);
        log.push(LogKind::Boot);

        let boot_ticks = if config.show_boot { 40 } else { 0 };
        let screen = if config.show_boot {
            Screen::Boot
        } else {
            Screen::Options
        };

        let mut state = Self {
            screen,
            active_index: 0,
            bank: SentryBank::new(config.starting_rounds),
            log,
            config,
            demo: DemoPlayer::default_demo(),
            boot_ticks_remaining: boot_ticks,
            should_quit: false,
            pending_fire_sfx: 0,
        };

        if state.config.demo_on_start && !state.config.show_boot {
            state.start_demo();
        }

        state
    }

    pub fn active_sentry(&self) -> &Sentry {
        self.bank
            .get(self.active_index)
            .expect("active sentry index in range")
    }

    pub fn active_sentry_mut(&mut self) -> Option<&mut Sentry> {
        self.bank.get_mut(self.active_index)
    }

    pub fn select_sentry(&mut self, index: usize) {
        if index < SENTRY_COUNT && index != self.active_index {
            self.active_index = index;
            let id = self.active_sentry().id;
            self.log.push(LogKind::SentrySelect { id });
        }
    }

    pub fn select_sentry_by_id(&mut self, id: u8) {
        if let Some(index) =
            (0..SENTRY_COUNT).find(|&i| self.bank.get(i).map(|s| s.id == id).unwrap_or(false))
        {
            self.select_sentry(index);
        }
    }

    pub fn set_screen(&mut self, screen: Screen) {
        if self.screen == screen {
            return;
        }
        // Leaving boot only via tick/skip.
        if self.screen == Screen::Boot && screen != Screen::Boot {
            self.boot_ticks_remaining = 0;
        }
        self.screen = screen;
        if screen != Screen::Boot {
            self.log.push(LogKind::ScreenChange {
                to: screen.label().to_string(),
            });
        }
    }

    pub fn toggle_fire_panel(&mut self) {
        match self.screen {
            Screen::Options => self.set_screen(Screen::Fire),
            Screen::Fire => self.set_screen(Screen::Options),
            Screen::Boot => self.skip_boot(),
        }
    }

    pub fn skip_boot(&mut self) {
        if self.screen == Screen::Boot {
            self.boot_ticks_remaining = 0;
            self.screen = Screen::Options;
            self.log.push(LogKind::ScreenChange {
                to: Screen::Options.label().to_string(),
            });
            if self.config.demo_on_start {
                self.start_demo();
            }
        }
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    // --- Options navigation (active sentry) ---

    pub fn focus_next_section(&mut self) {
        if let Some(s) = self.active_sentry_mut() {
            s.options.focus_next_section();
        }
    }

    pub fn focus_prev_section(&mut self) {
        if let Some(s) = self.active_sentry_mut() {
            s.options.focus_prev_section();
        }
    }

    pub fn select_up(&mut self) {
        let before = self.active_sentry().options;
        if let Some(s) = self.active_sentry_mut() {
            s.options.select_up();
        }
        self.log_option_delta(before, self.active_sentry().options);
    }

    pub fn select_down(&mut self) {
        let before = self.active_sentry().options;
        if let Some(s) = self.active_sentry_mut() {
            s.options.select_down();
        }
        self.log_option_delta(before, self.active_sentry().options);
    }

    /// Toggle SAFE / ARMED on the active sentry and log via `log_option_delta`.
    pub fn toggle_arm(&mut self) {
        let before = self.active_sentry().options;
        if let Some(s) = self.active_sentry_mut() {
            s.options.weapon_status = match s.options.weapon_status {
                WeaponStatus::Safe => WeaponStatus::Armed,
                WeaponStatus::Armed => WeaponStatus::Safe,
            };
        }
        self.log_option_delta(before, self.active_sentry().options);
    }

    fn log_option_delta(&mut self, before: OptionsState, after: OptionsState) {
        if before.system_mode != after.system_mode {
            self.log.push(LogKind::OptionChange {
                section: MenuSection::SystemMode.label().into(),
                value: after.system_mode.label().into(),
            });
        }
        if before.weapon_status != after.weapon_status {
            let id = self.active_sentry().id;
            match after.weapon_status {
                WeaponStatus::Armed => self.log.push(LogKind::Armed { sentry: id }),
                WeaponStatus::Safe => self.log.push(LogKind::Safe { sentry: id }),
            }
        }
        if before.iff_status != after.iff_status {
            self.log.push(LogKind::OptionChange {
                section: MenuSection::IffStatus.label().into(),
                value: after.iff_status.label().into(),
            });
        }
        if before.test_routine != after.test_routine {
            self.log.push(LogKind::OptionChange {
                section: MenuSection::TestRoutine.label().into(),
                value: after.test_routine.label().into(),
            });
        }
        if before.target_profile != after.target_profile {
            self.log.push(LogKind::OptionChange {
                section: MenuSection::TargetProfile.label().into(),
                value: after.target_profile.label().into(),
            });
        }
        if before.spectral_profile != after.spectral_profile {
            self.log.push(LogKind::OptionChange {
                section: MenuSection::SpectralProfile.label().into(),
                value: after.spectral_profile.label().into(),
            });
        }
        if before.target_select != after.target_select {
            self.log.push(LogKind::OptionChange {
                section: MenuSection::TargetSelect.label().into(),
                value: after.target_select.label().into(),
            });
        }
    }

    // --- Fire ---

    /// Fire the active sentry if armed and online.
    pub fn fire(&mut self) -> bool {
        let id = self.active_sentry().id;
        let armed = self.active_sentry().is_armed();
        let online = self.active_sentry().online;
        let profile = self.active_sentry().options.target_profile;
        let spectral = self.active_sentry().options.spectral_profile;
        let was_critical = self.active_sentry().fire.critical;

        if !online {
            self.log.push_info(format!("SENTRY-{id} OFFLINE"));
            return false;
        }
        if !armed {
            self.log
                .push_info(format!("SENTRY-{id} CANNOT FIRE — SAFE"));
            return false;
        }

        let fired = {
            let s = self.active_sentry_mut().unwrap();
            s.fire.fire()
        };

        let rounds = self.active_sentry().fire.rounds;
        let critical = self.active_sentry().fire.critical;

        if fired {
            self.log.push(LogKind::Fire {
                sentry: id,
                rounds_left: rounds,
                profile: profile.label().into(),
                spectral: spectral.label().into(),
            });
            if critical && !was_critical {
                self.log.push(LogKind::Critical { sentry: id, rounds });
            }
            if self.config.sound {
                self.pending_fire_sfx = (self.pending_fire_sfx + 1).min(MAX_PENDING_FIRE_SFX);
            }
        } else {
            self.log.push(LogKind::Empty { sentry: id });
        }
        fired
    }

    /// Drain queued fire SFX counts for the audio frontend (0 if muted/none).
    pub fn take_fire_sfx(&mut self) -> u32 {
        let n = self.pending_fire_sfx;
        self.pending_fire_sfx = 0;
        n
    }

    /// Toggle sound on/off (clears any queued SFX when muting).
    pub fn toggle_sound(&mut self) {
        self.config.sound = !self.config.sound;
        if !self.config.sound {
            self.pending_fire_sfx = 0;
        }
        self.log.push_info(if self.config.sound {
            "AUDIO ONLINE"
        } else {
            "AUDIO MUTED"
        });
    }

    /// Reload active sentry drum to configured starting rounds.
    pub fn reload(&mut self) {
        let rounds = self.config.starting_rounds;
        if let Some(s) = self.active_sentry_mut() {
            let id = s.id;
            s.fire.reset(rounds);
            self.log
                .push_info(format!("SENTRY-{id} RELOADED — {rounds} RDS"));
        }
    }

    // --- Demo ---

    pub fn start_demo(&mut self) {
        self.demo = DemoPlayer::default_demo();
        self.demo.start();
        self.log.push(LogKind::Demo {
            message: "AUTO-PLAY ENGAGED".into(),
        });
    }

    pub fn stop_demo(&mut self) {
        if self.demo.is_active() {
            self.demo.stop();
            self.log.push(LogKind::Demo {
                message: "AUTO-PLAY ABORTED".into(),
            });
        }
    }

    pub fn toggle_demo(&mut self) {
        if self.demo.is_active() {
            self.stop_demo();
        } else {
            self.start_demo();
        }
    }

    // --- Tick ---

    /// Periodic tick: boot countdown, demo, CRITICAL blink, barrel cool-down / R(M) decay.
    pub fn tick(&mut self) {
        if self.screen == Screen::Boot {
            if self.boot_ticks_remaining > 0 {
                self.boot_ticks_remaining -= 1;
            }
            if self.boot_ticks_remaining == 0 {
                self.screen = Screen::Options;
                self.log.push(LogKind::ScreenChange {
                    to: Screen::Options.label().to_string(),
                });
                if self.config.demo_on_start {
                    self.start_demo();
                }
            }
            return;
        }

        if self.demo.is_active() {
            // Avoid double-borrow of `self` and `self.demo`.
            let mut demo = std::mem::take(&mut self.demo);
            demo.tick(self);
            self.demo = demo;
        }

        for s in self.bank.iter_mut() {
            // Heat/R(M) rise on fire(); cool and spin down while idle.
            s.fire.tick();
        }
    }

    pub fn options(&self) -> &OptionsState {
        &self.active_sentry().options
    }

    pub fn fire_telemetry(&self) -> &FireTelemetry {
        &self.active_sentry().fire
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_requires_armed() {
        let mut app = AppState::new(Config {
            show_boot: false,
            ..Config::default()
        });
        assert!(!app.fire());
        app.active_sentry_mut().unwrap().options.weapon_status = WeaponStatus::Armed;
        assert!(app.fire());
        assert_eq!(app.fire_telemetry().rounds, 499);
    }

    #[test]
    fn screen_toggle() {
        let mut app = AppState::new(Config {
            show_boot: false,
            ..Config::default()
        });
        assert_eq!(app.screen, Screen::Options);
        app.toggle_fire_panel();
        assert_eq!(app.screen, Screen::Fire);
        app.toggle_fire_panel();
        assert_eq!(app.screen, Screen::Options);
    }

    #[test]
    fn fire_queues_sfx_when_sound_enabled() {
        let mut app = AppState::new(Config {
            show_boot: false,
            sound: true,
            ..Config::default()
        });
        app.active_sentry_mut().unwrap().options.weapon_status = WeaponStatus::Armed;
        assert!(app.fire());
        assert_eq!(app.take_fire_sfx(), 1);
        assert_eq!(app.take_fire_sfx(), 0);
    }

    #[test]
    fn fire_skips_sfx_when_muted() {
        let mut app = AppState::new(Config {
            show_boot: false,
            sound: false,
            ..Config::default()
        });
        app.active_sentry_mut().unwrap().options.weapon_status = WeaponStatus::Armed;
        assert!(app.fire());
        assert_eq!(app.take_fire_sfx(), 0);
    }

    #[test]
    fn toggle_arm_logs_and_flips() {
        let mut app = AppState::new(Config {
            show_boot: false,
            ..Config::default()
        });
        assert!(!app.active_sentry().is_armed());
        app.toggle_arm();
        assert!(app.active_sentry().is_armed());
        let last = app.log.recent(1)[0].kind.to_string();
        assert!(last.contains("ARMED"));
        app.toggle_arm();
        assert!(!app.active_sentry().is_armed());
    }

    #[test]
    fn toggle_sound_clears_queue() {
        let mut app = AppState::new(Config {
            show_boot: false,
            sound: true,
            ..Config::default()
        });
        app.active_sentry_mut().unwrap().options.weapon_status = WeaponStatus::Armed;
        assert!(app.fire());
        app.toggle_sound();
        assert!(!app.config.sound);
        assert_eq!(app.take_fire_sfx(), 0);
    }

    fn fresh() -> AppState {
        AppState::new(Config {
            show_boot: false,
            ..Config::default()
        })
    }

    #[test]
    fn boot_then_skip_to_options() {
        let mut app = AppState::new(Config {
            show_boot: true,
            demo_on_start: false,
            ..Config::default()
        });
        assert_eq!(app.screen, Screen::Boot);
        assert!(app.boot_ticks_remaining > 0);
        app.skip_boot();
        assert_eq!(app.screen, Screen::Options);
        assert_eq!(app.boot_ticks_remaining, 0);
    }

    #[test]
    fn boot_ticks_down_to_options() {
        let mut app = AppState::new(Config {
            show_boot: true,
            demo_on_start: false,
            ..Config::default()
        });
        let n = app.boot_ticks_remaining;
        for _ in 0..n {
            app.tick();
        }
        assert_eq!(app.screen, Screen::Options);
        assert!(!app.demo.is_active());
    }

    #[test]
    fn demo_starts_after_boot_when_configured() {
        let mut app = AppState::new(Config {
            show_boot: true,
            demo_on_start: true,
            ..Config::default()
        });
        app.skip_boot();
        assert!(app.demo.is_active());
    }

    #[test]
    fn demo_starts_immediately_without_boot() {
        let app = AppState::new(Config {
            show_boot: false,
            demo_on_start: true,
            ..Config::default()
        });
        assert!(app.demo.is_active());
    }

    #[test]
    fn select_sentry_changes_and_ignores_oob() {
        let mut app = fresh();
        app.select_sentry(2);
        assert_eq!(app.active_sentry().id, 3);
        app.select_sentry(2);
        assert_eq!(app.active_sentry().id, 3);
        app.select_sentry(99);
        assert_eq!(app.active_sentry().id, 3);
        app.select_sentry_by_id(1);
        assert_eq!(app.active_sentry().id, 1);
        app.select_sentry_by_id(9);
        assert_eq!(app.active_sentry().id, 1);
    }

    #[test]
    fn fire_offline_and_empty() {
        let mut app = fresh();
        app.toggle_arm();
        app.active_sentry_mut().unwrap().online = false;
        assert!(!app.fire());
        app.active_sentry_mut().unwrap().online = true;
        app.active_sentry_mut().unwrap().fire.rounds = 0;
        assert!(!app.fire());
        let kinds: Vec<_> = app.log.iter().map(|e| e.kind.to_string()).collect();
        assert!(kinds.iter().any(|k| k.contains("OFFLINE")));
        assert!(kinds.iter().any(|k| k.contains("EMPTY")));
    }

    #[test]
    fn fire_logs_critical_crossing() {
        let mut app = fresh();
        app.toggle_arm();
        app.active_sentry_mut().unwrap().fire.rounds = 100;
        assert!(app.fire());
        assert!(app.fire_telemetry().critical);
        assert!(app
            .log
            .iter()
            .any(|e| e.kind.to_string().contains("CRITICAL")));
    }

    #[test]
    fn pending_sfx_is_capped() {
        let mut app = AppState::new(Config {
            show_boot: false,
            sound: true,
            ..Config::default()
        });
        app.toggle_arm();
        for _ in 0..20 {
            assert!(app.fire());
        }
        assert_eq!(app.take_fire_sfx(), 6);
    }

    #[test]
    fn reload_restores_starting_rounds() {
        let mut app = fresh();
        app.toggle_arm();
        assert!(app.fire());
        app.reload();
        assert_eq!(app.fire_telemetry().rounds, app.config.starting_rounds);
    }

    #[test]
    fn toggle_demo_and_quit() {
        let mut app = fresh();
        assert!(!app.demo.is_active());
        app.toggle_demo();
        assert!(app.demo.is_active());
        app.toggle_demo();
        assert!(!app.demo.is_active());
        app.quit();
        assert!(app.should_quit);
    }

    #[test]
    fn set_screen_logs_once() {
        let mut app = fresh();
        let before = app.log.len();
        app.set_screen(Screen::Options);
        assert_eq!(app.log.len(), before);
        app.set_screen(Screen::Fire);
        assert_eq!(app.screen, Screen::Fire);
        assert!(app
            .log
            .iter()
            .any(|e| e.kind.to_string().contains("FIRING PANEL")));
    }

    #[test]
    fn select_down_logs_option_change() {
        let mut app = fresh();
        app.select_down();
        assert_ne!(
            app.options().system_mode,
            crate::options::SystemMode::AutoRemote
        );
        assert!(app
            .log
            .iter()
            .any(|e| matches!(e.kind, LogKind::OptionChange { .. })));
    }

    #[test]
    fn screen_labels() {
        assert_eq!(Screen::Options.label(), "OPTIONS");
        assert_eq!(Screen::Fire.label(), "FIRING PANEL");
        assert_eq!(Screen::Boot.label(), "BOOT");
    }
}
