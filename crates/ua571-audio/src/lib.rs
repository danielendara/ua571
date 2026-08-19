//! Native fire SFX via rodio (bundled MG42 burst from `ua571_core::sfx`).
//!
//! Web uses the same PCM through Web Audio in `ua571-web`.

#![forbid(unsafe_code)]

use std::num::{NonZeroU16, NonZeroU32};

use rodio::buffer::SamplesBuffer;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player};
use ua571_core::sfx::{fire_burst_pcm, FIRE_CYCLIC_HZ};

/// Native audio device handle. Keep alive for the app lifetime.
pub struct FireAudio {
    sink: MixerDeviceSink,
    sample_rate: u32,
    samples: Vec<f32>,
    pub muted: bool,
}

impl FireAudio {
    /// Open default output. Returns `None` if the device is unavailable.
    pub fn try_new() -> Option<Self> {
        let mut sink = DeviceSinkBuilder::open_default_sink().ok()?;
        sink.log_on_drop(false);
        let (sample_rate, samples) = fire_burst_pcm();
        Some(Self {
            sink,
            sample_rate,
            samples,
            muted: false,
        })
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Play one burst (non-blocking).
    pub fn play_fire(&self) {
        if self.muted {
            return;
        }
        self.play_buffer(self.samples.clone());
    }

    /// Play queued fires, staggered at cyclic rate so audio keeps up with rounds.
    pub fn play_fires(&self, count: u32) {
        if self.muted || count == 0 {
            return;
        }
        let n = count.min(6) as usize;
        if n == 1 {
            self.play_fire();
            return;
        }
        let period = ((self.sample_rate as f32) / FIRE_CYCLIC_HZ).round() as usize;
        let total = period * (n - 1) + self.samples.len();
        let mut buf = vec![0.0f32; total];
        for k in 0..n {
            let start = k * period;
            for (i, s) in self.samples.iter().enumerate() {
                let idx = start + i;
                if idx < buf.len() {
                    buf[idx] = (buf[idx] + s).tanh();
                }
            }
        }
        self.play_buffer(buf);
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
