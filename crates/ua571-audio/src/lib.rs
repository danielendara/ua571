//! Lightweight native fire SFX (procedural burst — no asset files).
//!
//! Web uses Web Audio in `ua571-web` instead.

#![forbid(unsafe_code)]

use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, OutputStreamHandle, Sink};

const SAMPLE_RATE: u32 = 22_050;
const FIRE_MS: u32 = 70;

/// Native audio device handle. Keep alive for the app lifetime.
pub struct FireAudio {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    samples: Vec<f32>,
    pub muted: bool,
}

impl FireAudio {
    /// Open default output. Returns `None` if the device is unavailable.
    pub fn try_new() -> Option<Self> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        let samples = synthesize_fire_burst(SAMPLE_RATE, FIRE_MS);
        Some(Self {
            _stream: stream,
            handle,
            samples,
            muted: false,
        })
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Play one autocannon-style burst (non-blocking).
    pub fn play_fire(&self) {
        if self.muted {
            return;
        }
        let Ok(sink) = Sink::try_new(&self.handle) else {
            return;
        };
        sink.append(SamplesBuffer::new(1, SAMPLE_RATE, self.samples.clone()));
        sink.detach();
    }

    /// Play up to a few queued fires (demo / rapid key repeat).
    pub fn play_fires(&self, count: u32) {
        for _ in 0..count.min(4) {
            self.play_fire();
        }
    }
}

/// Short noise + low thump with exponential decay.
fn synthesize_fire_burst(sample_rate: u32, duration_ms: u32) -> Vec<f32> {
    let n = (sample_rate as u64 * duration_ms as u64 / 1000) as usize;
    let mut out = Vec::with_capacity(n);
    let mut rng = 0xC0FFEE_u32;
    for i in 0..n {
        let t = i as f32 / sample_rate as f32;
        let env = (-t * 38.0).exp();
        // xorshift-ish noise
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        let noise = (rng as f32 / u32::MAX as f32) * 2.0 - 1.0;
        let thump = (t * 90.0 * std::f32::consts::TAU).sin() * (-t * 28.0).exp();
        let crack = (t * 420.0 * std::f32::consts::TAU).sin() * (-t * 55.0).exp() * 0.35;
        out.push((noise * 0.4 + thump * 0.55 + crack) * env * 0.55);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthesizes_non_empty_burst() {
        let s = synthesize_fire_burst(22_050, 70);
        assert!(s.len() > 100);
        assert!(s.iter().any(|v| v.abs() > 0.01));
    }
}
