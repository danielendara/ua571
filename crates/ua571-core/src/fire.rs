//! Firing-panel telemetry simulation.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Default drum capacity (film / original recreation).
pub const DEFAULT_ROUNDS: u16 = 500;

/// Rounds remaining at which CRITICAL is asserted.
pub const CRITICAL_THRESHOLD: u16 = 100;

/// Starting "time at 100%" in centiseconds (33.33 s).
pub const DEFAULT_TIME_CENTISECS: u32 = 3333;

/// Temperature gauge: cold at rest, climbs toward max under fire.
pub const TEMP_MAX: u8 = 100;
pub const TEMP_MIN: u8 = 0;

/// R(M) — rounds per minute gauge (relative 0..=100 of cyclic rate).
pub const RM_MAX: u8 = 100;
pub const RM_MIN: u8 = 0;

/// UI ticks after last shot before cool-down / R(M) decay begin.
/// (~4 × 80ms ≈ 320ms hold so continuous fire does not immediately cool.)
const IDLE_BEFORE_COOL_TICKS: u32 = 4;

/// Idle ticks between temperature steps down (thermal mass — cools slowly).
const TEMP_COOL_EVERY_TICKS: u32 = 5;

/// Idle ticks between R(M) steps down (cyclic rate falls off faster).
const RM_DECAY_EVERY_TICKS: u32 = 2;

/// Temperature drop per cool step.
const TEMP_COOL_STEP: u8 = 1;

/// R(M) drop per decay step.
const RM_DECAY_STEP: u8 = 2;

/// Telemetry shown on the firing panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FireTelemetry {
    pub rounds: u16,
    /// Time-at-100% remaining, centiseconds (e.g. 3333 => 33.33).
    pub time_centisecs: u32,
    /// Barrel / breech temperature 0..=100 (0 = cold).
    pub temperature: u8,
    /// Rounds per minute relative rate 0..=100 (0 = idle).
    pub rm: u8,
    pub critical: bool,
    /// Blink phase for CRITICAL invert effect.
    pub critical_blink: bool,
    critical_counter: u8,
    temp_counter: u8,
    rm_counter: u8,
    /// UI ticks since last successful fire (for cool-down).
    ticks_since_fire: u32,
    cool_temp_counter: u32,
    cool_rm_counter: u32,
}

impl Default for FireTelemetry {
    fn default() -> Self {
        Self::new(DEFAULT_ROUNDS)
    }
}

impl FireTelemetry {
    pub fn new(rounds: u16) -> Self {
        Self {
            rounds,
            time_centisecs: DEFAULT_TIME_CENTISECS,
            // Idle: cold barrel, zero fire rate.
            temperature: TEMP_MIN,
            rm: RM_MIN,
            critical: rounds < CRITICAL_THRESHOLD,
            critical_blink: false,
            critical_counter: 0,
            temp_counter: 0,
            rm_counter: 0,
            ticks_since_fire: IDLE_BEFORE_COOL_TICKS, // already "idle"
            cool_temp_counter: 0,
            cool_rm_counter: 0,
        }
    }

    /// Reset gauges and counters but keep (or set) round count.
    pub fn reset(&mut self, rounds: u16) {
        *self = Self::new(rounds);
    }

    /// Integer part of time display (seconds).
    pub fn time_secs(&self) -> u32 {
        self.time_centisecs / 100
    }

    /// Fractional hundredths for time display.
    pub fn time_frac(&self) -> u32 {
        self.time_centisecs % 100
    }

    /// Format as `SS.FF` like the original panel (no leading space).
    pub fn time_display(&self) -> String {
        format!("{}.{:02}", self.time_secs(), self.time_frac())
    }

    /// Fire one burst. Decrements ammo and advances telemetry theater.
    /// Returns `true` if a round was expended.
    pub fn fire(&mut self) -> bool {
        if self.rounds == 0 {
            self.update_critical();
            return false;
        }

        self.rounds -= 1;
        self.ticks_since_fire = 0;
        self.cool_temp_counter = 0;
        self.cool_rm_counter = 0;

        // Time-at-100% drains while firing.
        if self.time_centisecs > 0 {
            self.time_centisecs = self.time_centisecs.saturating_sub(7);
        }
        if self.time_centisecs < 300 {
            self.time_centisecs = 0;
        }

        // R(M) ramps up quickly toward cyclic rate.
        self.rm_counter = (self.rm_counter + 1) % 2;
        if self.rm_counter == 0 && self.rm < RM_MAX {
            self.rm = self.rm.saturating_add(2).min(RM_MAX);
        }

        // Temperature climbs more gradually under sustained fire.
        self.temp_counter = (self.temp_counter + 1) % 3;
        if self.temp_counter == 0 && self.temperature < TEMP_MAX {
            self.temperature = self.temperature.saturating_add(1).min(TEMP_MAX);
        }

        self.update_critical();
        true
    }

    /// Periodic UI tick: CRITICAL blink + natural cool-down / R(M) decay when idle.
    pub fn tick(&mut self) {
        self.tick_blink();
        self.tick_cooldown();
    }

    /// Idle tick for blink animation only (legacy name).
    pub fn tick_blink(&mut self) {
        if !self.critical {
            self.critical_blink = false;
            return;
        }
        self.critical_counter = (self.critical_counter + 1) % 3;
        if self.critical_counter == 0 {
            self.critical_blink = !self.critical_blink;
        }
    }

    /// Cool barrel and spin down R(M) after a short idle hold.
    fn tick_cooldown(&mut self) {
        self.ticks_since_fire = self.ticks_since_fire.saturating_add(1);
        if self.ticks_since_fire < IDLE_BEFORE_COOL_TICKS {
            return;
        }

        // R(M) falls off relatively quickly when not firing.
        if self.rm > RM_MIN {
            self.cool_rm_counter += 1;
            if self.cool_rm_counter >= RM_DECAY_EVERY_TICKS {
                self.cool_rm_counter = 0;
                self.rm = self.rm.saturating_sub(RM_DECAY_STEP);
            }
        }

        // Temperature cools more slowly (thermal mass).
        if self.temperature > TEMP_MIN {
            self.cool_temp_counter += 1;
            if self.cool_temp_counter >= TEMP_COOL_EVERY_TICKS {
                self.cool_temp_counter = 0;
                self.temperature = self.temperature.saturating_sub(TEMP_COOL_STEP);
            }
        }
    }

    fn update_critical(&mut self) {
        let was = self.critical;
        self.critical = self.rounds < CRITICAL_THRESHOLD;
        if self.critical && !was {
            self.critical_blink = true;
            self.critical_counter = 0;
        }
        if !self.critical {
            self.critical_blink = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_cold_and_idle() {
        let f = FireTelemetry::new(500);
        assert_eq!(f.temperature, 0);
        assert_eq!(f.rm, 0);
    }

    #[test]
    fn fire_raises_temp_and_rm() {
        let mut f = FireTelemetry::new(500);
        for _ in 0..12 {
            assert!(f.fire());
        }
        assert!(f.rm > 0, "R(M) should climb under fire");
        assert!(f.temperature > 0, "temperature should climb under fire");
        assert!(f.rm <= RM_MAX);
        assert!(f.temperature <= TEMP_MAX);
    }

    #[test]
    fn idle_cools_temperature() {
        let mut f = FireTelemetry::new(500);
        for _ in 0..30 {
            let _ = f.fire();
        }
        let hot = f.temperature;
        assert!(hot > 0);
        // Hold + cool steps: enough ticks to drop at least a few degrees.
        for _ in 0..50 {
            f.tick();
        }
        assert!(
            f.temperature < hot,
            "temp should cool when idle (was {hot}, now {})",
            f.temperature
        );
    }

    #[test]
    fn idle_decays_rm() {
        let mut f = FireTelemetry::new(500);
        for _ in 0..20 {
            let _ = f.fire();
        }
        let peak = f.rm;
        assert!(peak > 0);
        for _ in 0..30 {
            f.tick();
        }
        assert!(
            f.rm < peak,
            "R(M) should decay when idle (was {peak}, now {})",
            f.rm
        );
    }

    #[test]
    fn fire_resets_cooldown() {
        let mut f = FireTelemetry::new(500);
        for _ in 0..15 {
            let _ = f.fire();
        }
        for _ in 0..20 {
            f.tick();
        }
        let cooled = f.temperature;
        // Resume fire — should heat again, not keep cooling that tick path only
        for _ in 0..9 {
            let _ = f.fire();
        }
        assert!(
            f.temperature >= cooled,
            "firing again should not leave temp colder without heat steps"
        );
    }

    #[test]
    fn fire_decrements_rounds() {
        let mut f = FireTelemetry::new(500);
        assert!(f.fire());
        assert_eq!(f.rounds, 499);
    }

    #[test]
    fn critical_below_threshold() {
        let mut f = FireTelemetry::new(100);
        assert!(!f.critical);
        f.fire();
        assert_eq!(f.rounds, 99);
        assert!(f.critical);
    }

    #[test]
    fn empty_drum_does_not_underflow() {
        let mut f = FireTelemetry::new(0);
        assert!(!f.fire());
        assert_eq!(f.rounds, 0);
        assert_eq!(f.temperature, 0);
        assert_eq!(f.rm, 0);
    }

    #[test]
    fn time_display_format() {
        let f = FireTelemetry::new(500);
        assert_eq!(f.time_display(), "33.33");
        let mut f2 = FireTelemetry::new(500);
        f2.time_centisecs = 505;
        assert_eq!(f2.time_display(), "5.05");
    }

    #[test]
    fn reload_resets_gauges() {
        let mut f = FireTelemetry::new(500);
        for _ in 0..20 {
            let _ = f.fire();
        }
        f.reset(500);
        assert_eq!(f.temperature, 0);
        assert_eq!(f.rm, 0);
        assert_eq!(f.rounds, 500);
    }

    #[test]
    fn fire_drains_time_and_floors_small_values() {
        let mut f = FireTelemetry::new(500);
        let start = f.time_centisecs;
        assert!(f.fire());
        assert!(f.time_centisecs < start);
        f.time_centisecs = 299;
        assert!(f.fire());
        assert_eq!(f.time_centisecs, 0);
    }

    #[test]
    fn critical_blinks_then_stops_when_reloaded() {
        let mut f = FireTelemetry::new(99);
        assert!(f.critical);
        let mut saw_true = false;
        let mut saw_false = false;
        for _ in 0..12 {
            f.tick();
            if f.critical_blink {
                saw_true = true;
            } else {
                saw_false = true;
            }
        }
        assert!(saw_true && saw_false, "CRITICAL should blink");
        f.reset(500);
        f.tick();
        assert!(!f.critical);
        assert!(!f.critical_blink);
    }

    #[test]
    fn gauges_cap_at_max() {
        let mut f = FireTelemetry::new(500);
        f.rm = RM_MAX;
        f.temperature = TEMP_MAX;
        for _ in 0..20 {
            assert!(f.fire());
        }
        assert_eq!(f.rm, RM_MAX);
        assert_eq!(f.temperature, TEMP_MAX);
    }
}
