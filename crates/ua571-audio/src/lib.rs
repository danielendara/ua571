//! Native fire SFX via rodio (bundled MG42 burst from `ua571_core::sfx`).
//!
//! Web uses the same PCM through Web Audio in `ua571-web`.

#![forbid(unsafe_code)]

use std::cell::Cell;
use std::num::{NonZeroU16, NonZeroU32};
use std::time::{Duration, Instant};

use rodio::buffer::SamplesBuffer;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player};
use ua571_core::sfx::{fire_burst_duration_secs, fire_burst_pcm};

/// Native audio device handle. Keep alive for the app lifetime.
pub struct FireAudio {
    sink: MixerDeviceSink,
    sample_rate: u32,
    samples: Vec<f32>,
    burst_len: Duration,
    last_play: Cell<Option<Instant>>,
    pub muted: bool,
}

impl FireAudio {
    /// Open default output. Returns `None` if the device is unavailable.
    pub fn try_new() -> Option<Self> {
        let mut sink = DeviceSinkBuilder::open_default_sink().ok()?;
        sink.log_on_drop(false);
        let (sample_rate, samples) = fire_burst_pcm();
        let burst_len = Duration::from_secs_f32(fire_burst_duration_secs());
        Some(Self {
            sink,
            sample_rate,
            samples,
            burst_len,
            last_play: Cell::new(None),
            muted: false,
        })
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Play the bundled burst unless one is still ringing (no retrigger).
    pub fn play_fire(&self) {
        if self.muted {
            return;
        }
        if let Some(t) = self.last_play.get() {
            if t.elapsed() < self.burst_len {
                return;
            }
        }
        self.last_play.set(Some(Instant::now()));
        self.play_buffer(self.samples.clone());
    }

    /// Drain queued fires: at most one burst while the previous is still playing.
    pub fn play_fires(&self, count: u32) {
        if count > 0 {
            self.play_fire();
        }
    }

    fn play_buffer(&self, samples: Vec<f32>) {
        let channels = NonZeroU16::new(1).expect("mono");
        let rate = NonZeroU32::new(self.sample_rate).expect("sample rate");
        let player = Player::connect_new(self.sink.mixer());
        player.append(SamplesBuffer::new(channels, rate, samples));
        player.detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_burst_ready() {
        let (sr, v) = fire_burst_pcm();
        assert_eq!(sr, 22_050);
        assert!(v.len() > 100);
    }
}
