//! Scripted demo / auto-play engagement sequence.

use crate::log::LogKind;
use crate::options::{
    IffStatus, SpectralProfile, SystemMode, TargetProfile, TargetSelect, WeaponStatus,
};
use crate::state::{AppState, Screen};

/// One step in the demo timeline.
#[derive(Debug, Clone)]
pub enum DemoStep {
    Wait {
        ticks: u32,
    },
    SelectSentry {
        index: usize,
    },
    SetScreen {
        screen: Screen,
    },
    SetMode {
        mode: SystemMode,
    },
    SetWeapon {
        status: WeaponStatus,
    },
    SetIff {
        status: IffStatus,
    },
    SetProfile {
        profile: TargetProfile,
        spectral: SpectralProfile,
        select: TargetSelect,
    },
    Fire {
        times: u16,
    },
    Log {
        message: &'static str,
    },
    Done,
}

/// Built-in perimeter defense demo (inspired by Hadley's Hope corridor scene).
pub fn default_script() -> Vec<DemoStep> {
    vec![
        DemoStep::Log {
            message: "PERIMETER DEFENSE SEQUENCE INIT",
        },
        DemoStep::Wait { ticks: 8 },
        DemoStep::SelectSentry { index: 0 },
        DemoStep::SetScreen {
            screen: Screen::Options,
        },
        DemoStep::SetMode {
            mode: SystemMode::AutoRemote,
        },
        DemoStep::SetProfile {
            profile: TargetProfile::Hard,
            spectral: SpectralProfile::Bio,
            select: TargetSelect::MultiSpec,
        },
        DemoStep::SetIff {
            status: IffStatus::Search,
        },
        DemoStep::SetWeapon {
            status: WeaponStatus::Armed,
        },
        DemoStep::Wait { ticks: 6 },
        DemoStep::SelectSentry { index: 1 },
        DemoStep::SetMode {
            mode: SystemMode::AutoRemote,
        },
        DemoStep::SetWeapon {
            status: WeaponStatus::Armed,
        },
        DemoStep::Wait { ticks: 4 },
        DemoStep::SelectSentry { index: 2 },
        DemoStep::SetWeapon {
            status: WeaponStatus::Armed,
        },
        DemoStep::SelectSentry { index: 3 },
        DemoStep::SetWeapon {
            status: WeaponStatus::Armed,
        },
        DemoStep::Log {
            message: "ALL UNITS ARMED — LINK NOMINAL",
        },
        DemoStep::Wait { ticks: 8 },
        DemoStep::SelectSentry { index: 0 },
        DemoStep::SetIff {
            status: IffStatus::Engaged,
        },
        DemoStep::SetScreen {
            screen: Screen::Fire,
        },
        DemoStep::Log {
            message: "CONTACT BEARING 000 — MULTIPLE",
        },
        DemoStep::Wait { ticks: 4 },
        DemoStep::Fire { times: 12 },
        DemoStep::Wait { ticks: 3 },
        DemoStep::SelectSentry { index: 1 },
        DemoStep::SetIff {
            status: IffStatus::Engaged,
        },
        DemoStep::Fire { times: 18 },
        DemoStep::Wait { ticks: 3 },
        DemoStep::SelectSentry { index: 2 },
        DemoStep::Fire { times: 8 },
        DemoStep::SelectSentry { index: 3 },
        DemoStep::Fire { times: 10 },
        DemoStep::Wait { ticks: 6 },
        DemoStep::SelectSentry { index: 0 },
        DemoStep::Log {
            message: "SECTOR CLEAR — RESUME SEARCH",
        },
        DemoStep::SetIff {
            status: IffStatus::Search,
        },
        DemoStep::SetScreen {
            screen: Screen::Options,
        },
        DemoStep::Wait { ticks: 10 },
        DemoStep::Done,
    ]
}

#[derive(Debug, Clone)]
pub struct DemoPlayer {
    steps: Vec<DemoStep>,
    index: usize,
    wait_remaining: u32,
    fire_remaining: u16,
    active: bool,
}

impl Default for DemoPlayer {
    fn default() -> Self {
        Self::default_demo()
    }
}

impl DemoPlayer {
    pub fn new(steps: Vec<DemoStep>) -> Self {
        Self {
            steps,
            index: 0,
            wait_remaining: 0,
            fire_remaining: 0,
            active: false,
        }
    }

    pub fn default_demo() -> Self {
        Self::new(default_script())
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn start(&mut self) {
        self.index = 0;
        self.wait_remaining = 0;
        self.fire_remaining = 0;
        self.active = true;
    }

    pub fn stop(&mut self) {
        self.active = false;
        self.fire_remaining = 0;
        self.wait_remaining = 0;
    }

    /// Advance one tick. Returns true if still running.
    pub fn tick(&mut self, state: &mut AppState) -> bool {
        if !self.active {
            return false;
        }

        if self.wait_remaining > 0 {
            self.wait_remaining -= 1;
            return true;
        }

        if self.fire_remaining > 0 {
            state.fire();
            self.fire_remaining -= 1;
            return true;
        }

        loop {
            if self.index >= self.steps.len() {
                self.active = false;
                state.log.push(LogKind::Demo {
                    message: "SEQUENCE COMPLETE".into(),
                });
                return false;
            }

            let step = self.steps[self.index].clone();
            self.index += 1;

            match step {
                DemoStep::Wait { ticks } => {
                    self.wait_remaining = ticks.saturating_sub(1);
                    return true;
                }
                DemoStep::SelectSentry { index } => {
                    state.select_sentry(index);
                }
                DemoStep::SetScreen { screen } => {
                    state.set_screen(screen);
                }
                DemoStep::SetMode { mode } => {
                    if let Some(s) = state.active_sentry_mut() {
                        s.options.system_mode = mode;
                    }
                }
                DemoStep::SetWeapon { status } => {
                    if let Some(s) = state.active_sentry_mut() {
                        let id = s.id;
                        s.options.weapon_status = status;
                        match status {
                            WeaponStatus::Armed => state.log.push(LogKind::Armed { sentry: id }),
                            WeaponStatus::Safe => state.log.push(LogKind::Safe { sentry: id }),
                        }
                    }
                }
                DemoStep::SetIff { status } => {
                    if let Some(s) = state.active_sentry_mut() {
                        s.options.iff_status = status;
                    }
                }
                DemoStep::SetProfile {
                    profile,
                    spectral,
                    select,
                } => {
                    if let Some(s) = state.active_sentry_mut() {
                        s.options.target_profile = profile;
                        s.options.spectral_profile = spectral;
                        s.options.target_select = select;
                    }
                }
                DemoStep::Fire { times } => {
                    self.fire_remaining = times.saturating_sub(1);
                    state.fire();
                    return true;
                }
                DemoStep::Log { message } => {
                    state.log.push(LogKind::Demo {
                        message: message.into(),
                    });
                }
                DemoStep::Done => {
                    self.active = false;
                    state.log.push(LogKind::Demo {
                        message: "SEQUENCE COMPLETE".into(),
                    });
                    return false;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn demo_runs_to_completion() {
        let mut state = AppState::new(Config::default());
        let mut demo = DemoPlayer::default_demo();
        demo.start();
        let mut guard = 0;
        while demo.tick(&mut state) {
            guard += 1;
            assert!(guard < 10_000, "demo did not finish");
        }
        assert!(!demo.is_active());
    }

    #[test]
    fn stop_halts_before_completion() {
        let mut state = AppState::new(Config {
            show_boot: false,
            ..Config::default()
        });
        let mut demo = DemoPlayer::default_demo();
        demo.start();
        assert!(demo.tick(&mut state));
        demo.stop();
        assert!(!demo.is_active());
        assert!(!demo.tick(&mut state));
    }

    #[test]
    fn fire_step_expends_rounds() {
        let mut state = AppState::new(Config {
            show_boot: false,
            ..Config::default()
        });
        let mut demo = DemoPlayer::new(vec![
            DemoStep::SetWeapon {
                status: WeaponStatus::Armed,
            },
            DemoStep::Fire { times: 3 },
            DemoStep::Done,
        ]);
        demo.start();
        let start = state.fire_telemetry().rounds;
        while demo.tick(&mut state) {}
        assert_eq!(state.fire_telemetry().rounds, start - 3);
    }
}
