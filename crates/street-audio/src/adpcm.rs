//! The two ADPCM codecs on the disc.
//!
//! - **XA** (the publisher's codec 0x0a): music, crowds, commentary. In SCHl stream
//!   blocks each channel starts with a 4-byte history (two little-endian
//!   `i16`, newest first); bank sounds have none and start from silence.
//!   Then come 15-byte frames of 28 samples: one byte with the predictor (high nibble) and shift
//!   (low nibble), then 14 bytes of nibbles, high nibble first. A first byte
//!   of `0xee` marks an uncompressed frame (two big-endian history samples and
//!   28 big-endian samples), which later tools wrote; none has been seen on
//!   this disc but the decoder accepts it.
//! - **PS-ADPCM** (Sony's "VAG"): the sound effects. 16-byte frames of 28
//!   samples: shift (low nibble) and predictor (high nibble), a flags byte,
//!   then 14 bytes of nibbles, low nibble first.

/// XA predictor coefficients (×256): `COEF[p]` and `COEF[p + 4]`.
const EA_XA: [i32; 20] = [
    0, 240, 460, 392, 0, 0, -208, -220, 0, 1, 3, 4, 7, 8, 10, 11, 0, -1, -3, -4,
];

/// PS-ADPCM predictor coefficients (×64).
const VAG: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];

pub const FRAME_SAMPLES: usize = 28;
pub const EA_XA_FRAME: usize = 15;
pub const VAG_FRAME: usize = 16;

fn clamp16(v: i32) -> i16 {
    v.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

/// One XA channel's decoder state.
#[derive(Clone, Copy, Debug, Default)]
pub struct EaXa {
    h1: i32,
    h2: i32,
}

impl EaXa {
    /// A channel starting from the 4-byte history at the front of `data`.
    pub fn with_history(data: &[u8]) -> Option<Self> {
        let h = data.get(..4)?;
        Some(Self {
            h1: i16::from_le_bytes([h[0], h[1]]) as i32,
            h2: i16::from_le_bytes([h[2], h[3]]) as i32,
        })
    }

    /// Decodes the frame at the front of `data` into `out`. Returns the bytes
    /// the frame used, or `None` if `data` is too short.
    pub fn frame(&mut self, data: &[u8], out: &mut [i16; FRAME_SAMPLES]) -> Option<usize> {
        let info = *data.first()?;
        if info == 0xee {
            let f = data.get(..61)?;
            self.h1 = i16::from_be_bytes([f[1], f[2]]) as i32;
            self.h2 = i16::from_be_bytes([f[3], f[4]]) as i32;
            for (i, s) in out.iter_mut().enumerate() {
                *s = i16::from_be_bytes([f[5 + 2 * i], f[6 + 2 * i]]);
            }
            return Some(61);
        }
        let f = data.get(..EA_XA_FRAME)?;
        let p = (info >> 4) as usize;
        let (c1, c2) = (EA_XA[p], EA_XA[p + 4]);
        let shift = (info & 0x0f) as u32 + 8;
        for (i, s) in out.iter_mut().enumerate() {
            let byte = f[1 + i / 2];
            let nibble = if i % 2 == 0 { byte >> 4 } else { byte & 0x0f };
            let raw = ((nibble as i32) << 28) >> shift;
            let v = clamp16((raw + c1 * self.h1 + c2 * self.h2) >> 8);
            *s = v;
            self.h2 = self.h1;
            self.h1 = v as i32;
        }
        Some(EA_XA_FRAME)
    }
}

/// Decodes `samples` samples of one XA channel whose history and frames
/// start at `data[0]` (the layout inside SCHl blocks).
pub fn ea_xa(data: &[u8], samples: usize) -> Vec<i16> {
    match EaXa::with_history(data) {
        Some(ch) => ea_xa_frames(ch, data, 4, samples),
        None => Vec::new(),
    }
}

/// Decodes `samples` samples of a bank sound's XA channel: frames from
/// `data[0]`, no history. (Checked over every XA bank sound on the disc:
/// reading the first 4 bytes as history makes them rough and clipped.)
pub fn ea_xa_bank(data: &[u8], samples: usize) -> Vec<i16> {
    ea_xa_frames(EaXa::default(), data, 0, samples)
}

fn ea_xa_frames(mut ch: EaXa, data: &[u8], start: usize, samples: usize) -> Vec<i16> {
    let mut out = Vec::with_capacity(samples);
    let mut pos = start;
    let mut buf = [0i16; FRAME_SAMPLES];
    while out.len() < samples {
        let Some(used) = ch.frame(&data[pos.min(data.len())..], &mut buf) else {
            break;
        };
        pos += used;
        let take = (samples - out.len()).min(FRAME_SAMPLES);
        out.extend_from_slice(&buf[..take]);
    }
    out
}

/// Decodes `samples` samples of one PS-ADPCM channel starting at `data[0]`.
pub fn vag(data: &[u8], samples: usize) -> Vec<i16> {
    let mut out = Vec::with_capacity(samples);
    let (mut h1, mut h2) = (0i32, 0i32);
    for f in data.as_chunks::<VAG_FRAME>().0 {
        if out.len() >= samples {
            break;
        }
        let shift = (f[0] & 0x0f) as u32;
        let (c1, c2) = VAG.get((f[0] >> 4) as usize).copied().unwrap_or((0, 0));
        for i in 0..FRAME_SAMPLES {
            let byte = f[2 + i / 2];
            let nibble = if i % 2 == 0 { byte & 0x0f } else { byte >> 4 };
            let raw = (((nibble as i32) << 28) >> 16) >> shift;
            let v = clamp16(raw + ((h1 * c1 + h2 * c2) >> 6));
            h2 = h1;
            h1 = v as i32;
            if out.len() < samples {
                out.push(v);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_frames_decode_to_silence() {
        let mut data = vec![0u8; 4 + 2 * EA_XA_FRAME];
        data[4] = 0x0c;
        data[4 + EA_XA_FRAME] = 0x0c;
        assert_eq!(ea_xa(&data, 56), vec![0; 56]);
        assert_eq!(vag(&[0u8; 32], 56), vec![0; 56]);
    }

    #[test]
    fn ea_xa_follows_its_predictor() {
        // History 1000, 1000; predictor 1 (240/256 of the last sample), no
        // residual: each sample is 240/256 of the one before.
        let mut data = vec![0u8; 4 + EA_XA_FRAME];
        data[..2].copy_from_slice(&1000i16.to_le_bytes());
        data[2..4].copy_from_slice(&1000i16.to_le_bytes());
        data[4] = 0x1c;
        let out = ea_xa(&data, 3);
        assert_eq!(out, vec![937, 878, 823]);
    }

    #[test]
    fn vag_residual_is_signed_and_shifted() {
        // Shift 12, predictor 0: nibble 0x7 is 7 << 12 >> 12 = 7, 0x8 is -8.
        let mut f = [0u8; 16];
        f[0] = 0x0c;
        f[2] = 0x87;
        assert_eq!(vag(&f, 2), vec![7, -8]);
    }
}
