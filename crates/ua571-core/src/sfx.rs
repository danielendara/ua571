//! Fire SFX: bundled MG42-style burst (no film audio).
//!
//! Native and web frontends retrigger `fire_burst.wav` (Freesound 387508,
//! CC0, trimmed).

/// Bundled MG42-style burst (trimmed Freesound 387508, mono 22.05 kHz).
const FIRE_BURST_WAV: &[u8] = include_bytes!("../assets/fire_burst.wav");

/// MG42-ish cyclic rate (shots per second). Used for multi-fire stagger.
pub const FIRE_CYCLIC_HZ: f32 = 20.0;

/// Cap queued / staggered fire SFX so a lagging UI does not explode with overlapping bursts.
pub const MAX_FIRE_SFX_BURST: usize = 6;

/// Decode the bundled burst as `(sample_rate, mono f32 in [-1, 1])`.
pub fn fire_burst_pcm() -> (u32, Vec<f32>) {
    decode_wav_pcm16(FIRE_BURST_WAV).expect("bundled fire_burst.wav is PCM16")
}

fn read_u32_le(bytes: &[u8], start: usize) -> Result<u32, &'static str> {
    let end = start + 4;
    if end > bytes.len() {
        return Err("truncated chunk");
    }
    let chunk: [u8; 4] = bytes[start..end]
        .try_into()
        .map_err(|_| "truncated chunk")?;
    Ok(u32::from_le_bytes(chunk))
}

fn read_u16_le(bytes: &[u8], start: usize) -> Result<u16, &'static str> {
    let end = start + 2;
    if end > bytes.len() {
        return Err("truncated chunk");
    }
    let chunk: [u8; 2] = bytes[start..end]
        .try_into()
        .map_err(|_| "truncated chunk")?;
    Ok(u16::from_le_bytes(chunk))
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
        let size = usize::try_from(read_u32_le(bytes, pos + 4)?).map_err(|_| "chunk too large")?;
        let start = pos + 8;
        let end = start.saturating_add(size);
        if end > bytes.len() {
            return Err("truncated chunk");
        }
        if id == b"fmt " && size >= 16 {
            ch = read_u16_le(bytes, start + 2)?;
            sr = read_u32_le(bytes, start + 4)?;
            bits = read_u16_le(bytes, start + 14)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncated_wav_chunk_returns_err() {
        // `fmt ` claims 32-byte payload; file is 44 bytes (min WAVE length) so chunk is truncated.
        let mut wav = Vec::from(b"RIFF\x00\x00\x00\x00WAVEfmt \x20\x00\x00\x00");
        wav.resize(44, 0);
        assert_eq!(decode_wav_pcm16(&wav), Err("truncated chunk"));

        // 14-byte buffer: a u16 read at offset 13 needs bytes 13..15 (past the end).
        let fourteen = [0u8; 14];
        assert_eq!(read_u16_le(&fourteen, 13), Err("truncated chunk"));
    }

    #[test]
    fn bundled_burst_decodes() {
        let (sr, s) = fire_burst_pcm();
        assert_eq!(sr, 22_050);
        assert!(s.len() > 1000);
        assert!(s.iter().any(|v| v.abs() > 0.1));
        assert!(s.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
        let d = s.len() as f32 / sr as f32;
        assert!((0.07..=0.12).contains(&d), "burst len {d}");
    }
}
