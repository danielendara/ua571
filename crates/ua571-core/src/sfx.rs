//! Procedural fire SFX (no sample assets, no film audio).
//!
//! Character goals (fan recreation, inspired by public descriptions of the
//! prop gun / MG42 proxy used on set — not a copy of the soundtrack):
//! - High cyclic rate (~1200 rpm MG42 family) → ~20 Hz mechanical pulse
//! - High-mid “buzzsaw” pulse rather than a soft bass boom
//! - Rhythmic metal thwack + quiet ratchet grit
//! - Brief electronic chirp edge (targeting console flavor)

/// Sample rate used by the native frontend (web uses the AudioContext rate).
pub const FIRE_SFX_SAMPLE_RATE: u32 = 22_050;

/// Bundled MG42-style burst (trimmed Freesound 387508, mono 22.05 kHz).
const FIRE_BURST_WAV: &[u8] = include_bytes!("../assets/fire_burst.wav");

/// One expended-round pulse length (ms). Short enough to chain into rapid fire.
pub const FIRE_SFX_MS: u32 = 55;

/// MG42-ish cyclic rate (shots per second). Used for multi-fire stagger.
pub const FIRE_CYCLIC_HZ: f32 = 20.0;

/// Decode the bundled burst as `(sample_rate, mono f32 in [-1, 1])`.
pub fn fire_burst_pcm() -> (u32, Vec<f32>) {
    decode_wav_pcm16(FIRE_BURST_WAV).expect("bundled fire_burst.wav is PCM16")
}

/// Duration of the bundled burst (used to gate retriggers).
pub fn fire_burst_duration_secs() -> f32 {
    let (sr, samples) = fire_burst_pcm();
    samples.len() as f32 / sr as f32
}

fn decode_wav_pcm16(bytes: &[u8]) -> Result<(u32, Vec<f32>), &'static str> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAVE file");
    }
    let mut pos = 12;
    let mut sr = 0u32;
    let mut ch = 0u16;
    let mut bits = 0u16;
    let mut data: &[u8] = &[];
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let start = pos + 8;
        let end = start.saturating_add(size);
        if end > bytes.len() {
            return Err("truncated chunk");
        }
        if id == b"fmt " && size >= 16 {
            ch = u16::from_le_bytes(bytes[start + 2..start + 4].try_into().unwrap());
            sr = u32::from_le_bytes(bytes[start + 4..start + 8].try_into().unwrap());
            bits = u16::from_le_bytes(bytes[start + 14..start + 16].try_into().unwrap());
        } else if id == b"data" {
            data = &bytes[start..end];
        }
        pos = end + size % 2;
    }
    if sr == 0 || ch == 0 || bits != 16 || data.is_empty() {
        return Err("unsupported wav");
    }
    let step = ch as usize;
    let mut out = Vec::with_capacity(data.len() / 2 / step.max(1));
    let mut i = 0;
    while i + 2 * step <= data.len() {
        let mut acc = 0.0f32;
        for c in 0..step {
            let o = i + c * 2;
            let s = i16::from_le_bytes([data[o], data[o + 1]]);
            acc += f32::from(s) / 32768.0;
        }
        out.push(acc / step as f32);
        i += 2 * step;
    }
    Ok((sr, out))
}

/// Synthesize one fire pulse as mono `f32` samples in roughly `[-1, 1]`.
///
/// `seed` varies the noise / micro-timing so stacked shots do not phase-lock
/// into a pure tone.
pub fn synthesize_fire_burst(sample_rate: u32, duration_ms: u32, seed: u32) -> Vec<f32> {
    let sample_rate = sample_rate.max(8_000);
    let n = (u64::from(sample_rate) * u64::from(duration_ms) / 1000) as usize;
    let n = n.max(1);
    let mut out = Vec::with_capacity(n);
    let mut rng = seed | 1;

    // Slight rate / pitch jitter per seed (still MG42 ballpark).
    let cyclic_hz = FIRE_CYCLIC_HZ + ((seed % 9) as f32 - 4.0) * 0.12;
    let pitch = 1.0 + ((seed % 11) as f32 - 5.0) * 0.012;
    let period = 1.0 / cyclic_hz;

    for i in 0..n {
        let t = i as f32 / sample_rate as f32;
        // One primary thwack; residual energy into the next half-cycle for grit.
        let phase = t % period;

        // Sharp mechanical envelope (attack + metal decay).
        let pulse = if phase < 0.0025 {
            // Near-instant attack.
            (phase / 0.0025).clamp(0.0, 1.0)
        } else if phase < 0.018 {
            let u = (phase - 0.0025) / 0.0155;
            (1.0 - u).powf(1.6)
        } else if phase < 0.032 {
            // Soft second “chamber” click (ratchet / bolt).
            let u = (phase - 0.018) / 0.014;
            0.22 * (1.0 - u).powf(1.2)
        } else {
            0.0
        };

        // Overall burst tail so the buffer does not click off.
        let tail = (-t * 22.0).exp();

        let noise = xorshift_noise(&mut rng);

        // High-mid grit (high-pitched pulse character) — cheap HP-ish emphasis.
        let noise2 = xorshift_noise(&mut rng);
        let high_grit = (noise - noise2 * 0.55) * 0.55;

        // Mechanical thwack body (mid, not deep sub).
        let thwack = (phase * 310.0 * pitch * std::f32::consts::TAU).sin() * (-phase * 95.0).exp()
            + (phase * 140.0 * pitch * std::f32::consts::TAU).sin() * (-phase * 55.0).exp() * 0.55;

        // Metallic ring (high-pitched “brap”).
        let ring = (t * 2100.0 * pitch * std::f32::consts::TAU).sin()
            * (-phase * 75.0).exp()
            * 0.28
            + (t * 3750.0 * pitch * std::f32::consts::TAU).sin() * (-phase * 130.0).exp() * 0.14
            + (t * 5200.0 * pitch * std::f32::consts::TAU).sin() * (-phase * 180.0).exp() * 0.08;

        // Brief electronic chirp (targeting console edge) on the attack.
        let chirp = if phase < 0.0035 {
            let f = 2800.0 * pitch + phase * 220_000.0;
            (phase * f * std::f32::consts::TAU).sin() * (1.0 - phase / 0.0035) * 0.22
        } else {
            0.0
        };

        // Quiet ratchet (panning-gear grit) between main thwacks.
        let ratchet_phase = (t * cyclic_hz * 2.0) % 1.0;
        let ratchet = if ratchet_phase < 0.02 {
            xorshift_noise(&mut rng) * (1.0 - ratchet_phase / 0.02) * 0.10
        } else {
            0.0
        };

        let sample = (high_grit * pulse * 0.50
            + thwack * pulse * 0.90
            + ring * pulse
            + chirp
            + ratchet * tail)
            * tail
            * 0.72;

        // Soft clip keeps stacked rapid-fire from hard-clipping.
        out.push(sample.tanh());
    }
    out
}

/// Default pulse at the native sample rate (seeded).
pub fn synthesize_default_fire(seed: u32) -> Vec<f32> {
    synthesize_fire_burst(FIRE_SFX_SAMPLE_RATE, FIRE_SFX_MS, seed)
}

#[inline]
fn xorshift_noise(rng: &mut u32) -> f32 {
    let mut x = *rng;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *rng = x;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthesizes_non_empty_burst() {
        let s = synthesize_fire_burst(22_050, 55, 0xC0FFEE);
        assert!(s.len() > 100);
        assert!(s.iter().any(|v| v.abs() > 0.05));
    }

    #[test]
    fn seeds_differ() {
        let a = synthesize_fire_burst(22_050, 55, 1);
        let b = synthesize_fire_burst(22_050, 55, 99);
        assert_ne!(a, b);
    }

    #[test]
    fn length_and_amplitude_bounds() {
        let s = synthesize_fire_burst(FIRE_SFX_SAMPLE_RATE, FIRE_SFX_MS, 7);
        let expected = (u64::from(FIRE_SFX_SAMPLE_RATE) * u64::from(FIRE_SFX_MS) / 1000) as usize;
        assert_eq!(s.len(), expected);
        assert!(s.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
    }

    #[test]
    fn bundled_burst_decodes() {
        let (sr, s) = fire_burst_pcm();
        assert_eq!(sr, 22_050);
        assert!(s.len() > 1000);
        assert!(s.iter().any(|v| v.abs() > 0.1));
        assert!(s.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
        let d = fire_burst_duration_secs();
        assert!((0.07..=0.12).contains(&d), "burst len {d}");
    }
}
