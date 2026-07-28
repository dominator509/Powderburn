//! Deterministic integer PCM attenuation for the presentation mixer.

/// Dialogue ducks all other cues to thirty percent of their configured gain.
pub const fn effective_gain_percent(configured: u8, dialogue_active: bool) -> u8 {
    if dialogue_active {
        ((configured as u16 * 30) / 100) as u8
    } else {
        configured
    }
}

/// Apply integer gain to a PCM16 little-endian WAV without changing its
/// container, sample rate, or channel layout.
pub fn attenuate_pcm16_wav(input: &[u8], gain_percent: u8) -> Result<Vec<u8>, String> {
    if input.len() < 12 || &input[..4] != b"RIFF" || &input[8..12] != b"WAVE" {
        return Err("E-AUDIO-FORMAT: not a RIFF/WAVE file".to_string());
    }
    let mut cursor = 12usize;
    let mut pcm16 = false;
    let mut data_range = None;
    while cursor.checked_add(8).is_some_and(|end| end <= input.len()) {
        let id = &input[cursor..cursor + 4];
        let size = u32::from_le_bytes(
            input[cursor + 4..cursor + 8]
                .try_into()
                .map_err(|_| "E-AUDIO-FORMAT: invalid chunk size".to_string())?,
        ) as usize;
        let start = cursor + 8;
        let end = start
            .checked_add(size)
            .ok_or_else(|| "E-AUDIO-FORMAT: chunk overflow".to_string())?;
        if end > input.len() {
            return Err("E-AUDIO-FORMAT: truncated chunk".to_string());
        }
        if id == b"fmt " && size >= 16 {
            let format = u16::from_le_bytes([input[start], input[start + 1]]);
            let bits = u16::from_le_bytes([input[start + 14], input[start + 15]]);
            pcm16 = format == 1 && bits == 16;
        } else if id == b"data" {
            data_range = Some(start..end);
        }
        cursor = end + (size & 1);
    }
    if !pcm16 {
        return Err("E-AUDIO-FORMAT: only PCM16 WAV is supported".to_string());
    }
    let range = data_range.ok_or_else(|| "E-AUDIO-FORMAT: missing data chunk".to_string())?;
    let mut output = input.to_vec();
    for sample in output[range].chunks_exact_mut(2) {
        let value = i16::from_le_bytes([sample[0], sample[1]]) as i32;
        let scaled = (value * i32::from(gain_percent) / 100)
            .clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        sample.copy_from_slice(&scaled.to_le_bytes());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialogue_ducks_to_exactly_thirty_percent() {
        assert_eq!(effective_gain_percent(100, true), 30);
        assert_eq!(effective_gain_percent(80, true), 24);
        assert_eq!(effective_gain_percent(80, false), 80);
    }
}
