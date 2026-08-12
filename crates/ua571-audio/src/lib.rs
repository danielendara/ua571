//! Native fire SFX via rodio (procedural buffer from `ua571_core::sfx`).
//!
//! Web uses the same synthesis through Web Audio in `ua571-web`.

#![forbid(unsafe_code)]

use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use ua571_core::sfx::{synthesize_fire_burst, FIRE_CYCLIC_HZ, FIRE_SFX_MS, FIRE_SFX_SAMPLE_RATE};

/// How many pre-baked pulse variants to rotate through (less robotic).
const VARIANT_COUNT: usize = 6;

/// Native audio device handle. Keep alive for the app lifetime.
pub struct FireAudio {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    variants: Vec<Vec<f32>>,
    next_variant: std::cell::Cell<usize>,
    pub muted: bool,
}

impl FireAudio {
    /// Open default output. Returns `None` if the device is unavailable.
    pub fn try_new() -> Option<Self> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        let variants = (0..VARIANT_COUNT)
            .map(|i| {
                synthesize_fire_burst(
                    FIRE_SFX_SAMPLE_RATE,
                    FIRE_SFX_MS,
                    0xA57E_u32.wrapping_mul(i as u32 + 1).wrapping_add(0xC0FFEE),
                )
            })
            .collect();
        Some(Self {
            _stream: stream,
            handle,
            variants,
            next_variant: std::cell::Cell::new(0),
            muted: false,
        })
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Play one MG42-style pulse (non-blocking).
    pub fn play_fire(&self) {
        if self.muted {
            return;
        }
        let samples = self.take_variant();
        self.play_buffer(samples);
    }

    /// Play queued fires as a short cyclic brap (MG42-ish spacing).
    pub fn play_fires(&self, count: u32) {
        if self.muted || count == 0 {
            return;
        }
        let n = count.min(6) as usize;
        if n == 1 {
            self.play_fire();
            return;
        }

        let period = ((FIRE_SFX_SAMPLE_RATE as f32) / FIRE_CYCLIC_HZ).round() as usize;
        let pulse_len = self.variants[0].len();
        let total = period * (n - 1) + pulse_len;
        let mut buf = vec![0.0f32; total];
        for k in 0..n {
            let start = k * period;
            let pulse = self.take_variant();
            for (i, s) in pulse.iter().enumerate() {
                let idx = start + i;
                if idx < buf.len() {
                    buf[idx] = (buf[idx] + s).tanh();
                }
            }
        }
        self.play_buffer(buf);
    }

    fn take_variant(&self) -> Vec<f32> {
        let i = self.next_variant.get();
        self.next_variant.set((i + 1) % self.variants.len());
        self.variants[i].clone()
    }

    fn play_buffer(&self, samples: Vec<f32>) {
        let Ok(sink) = Sink::try_new(&self.handle) else {
            return;
        };
        sink.append(SamplesBuffer::new(1, FIRE_SFX_SAMPLE_RATE, samples));
        sink.detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_non_empty() {
        let v = synthesize_fire_burst(FIRE_SFX_SAMPLE_RATE, FIRE_SFX_MS, 1);
        assert!(v.len() > 100);
    }
}
