//! Shared "paused in the background" policy for windowed/tabbed frontends.
//!
//! Web pauses ticks and SFX while its browser tab is hidden (`set_hidden`,
//! #73); pixel does the same while its desktop window has lost OS focus
//! (#90) — a shared machine shouldn't keep hearing gunfire from a window the
//! operator isn't looking at. Both boil down to the same pure decision: while
//! backgrounded, never tick and never make sound, and when the frontend comes
//! back to the foreground, resume ticking and only resume sound if the
//! operator's own sound preference was already on. Neither frontend flips
//! that preference for this — muting while backgrounded must not look like
//! the user muted, and coming back to the foreground must not unmute sound
//! the operator turned off. [`idle_runtime`] is the single place both
//! frontends compute that, so the tick/mute rule can't drift between them.
//!
//! TUI runs in a terminal with no reliable analog to page-visibility or
//! window-focus, so it does not use this.

/// Whether the simulation should tick and SFX should play right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdleRuntime {
    /// Advance the simulation this frame.
    pub tick: bool,
    /// Emit/allow SFX this frame.
    pub audio: bool,
}

/// Classify whether the frontend is backgrounded (hidden tab / unfocused
/// window) into tick/audio policy.
///
/// `backgrounded` never itself changes `sound_enabled` — the caller's own
/// mute/sound preference is only ever read here, never written.
pub fn idle_runtime(backgrounded: bool, sound_enabled: bool) -> IdleRuntime {
    if backgrounded {
        IdleRuntime {
            tick: false,
            audio: false,
        }
    } else {
        IdleRuntime {
            tick: true,
            audio: sound_enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backgrounded_pauses_ticks_and_mutes_without_flipping_sound_pref() {
        let muted = idle_runtime(true, true);
        assert!(!muted.tick, "backgrounded frontend must not simulate");
        assert!(!muted.audio, "backgrounded frontend must mute output");
        let sound_off = idle_runtime(true, false);
        assert!(!sound_off.tick);
        assert!(!sound_off.audio);

        let foreground_on = idle_runtime(false, true);
        assert!(foreground_on.tick);
        assert!(
            foreground_on.audio,
            "foreground + sound pref on resumes audio"
        );
        let foreground_off = idle_runtime(false, false);
        assert!(foreground_off.tick);
        assert!(
            !foreground_off.audio,
            "coming to the foreground must not enable sound the operator muted"
        );
    }
}
