//! Shared "paused in the background" policy for windowed/tabbed frontends.
//!
//! While a tab is hidden or a desktop window lacks focus, [`idle_runtime`] pauses
//! simulation ticks and SFX so a shared machine does not keep playing gunfire in
//! the background. Returning to the foreground resumes ticks and only resumes
//! audio when the operator's sound preference was already on — background mute
//! must not look like a user mute, and foreground must not unmute sound they
//! turned off.
//!
//! TUI has no reliable page-visibility or window-focus analog, so it does not use this.

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
