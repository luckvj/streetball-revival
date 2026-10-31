//! The sound driver's rate converter: every stream and bank sound to the
//! mixer's 48 kHz, the way `SNDDRV.IRX` does it on the I/O chip.
//!
//! `MIX_setpitch` builds each voice's converter with `SFILTER_rsinit`, which
//! picks `SFILTERI_rs16linear` when the driver's resample quality (`0x11dde`,
//! set from the game's init command through `MIX_setresamplequality`) is 25
//! or more, else `SFILTERI_rs16nearest`. Every save state holds 25, so the
//! game plays straight lines between samples: a 16.16 position stepping by
//! the voice's pitch, each output `s[i] + ((s[i + 1] - s[i]) * frac >> 16)`.
//! Measured against the original's Lords in PCSX2, this matches its top
//! octaves within 0.7 dB, where a band-limited converter played the hi-hats
//! 2 to 3 dB brighter than the console.

/// The mixer's output rate.
pub const MIXER_RATE: u32 = 48_000;

/// One converter between two rates, for frames of up to two channels.
pub struct Resampler {
    /// Input frames per output frame, 16.16.
    step: u32,
    /// The point read's fraction past `s0`, 16.16.
    pos: u32,
    s0: [f32; 2],
    s1: [f32; 2],
    started: bool,
    ended: bool,
}

impl Resampler {
    pub fn new(from: u32, to: u32) -> Self {
        Self {
            step: ((u64::from(from) << 16) / u64::from(to.max(1))) as u32,
            pos: 0,
            s0: [0.0; 2],
            s1: [0.0; 2],
            started: false,
            ended: false,
        }
    }

    /// The next output frame, pulling input frames from `input` as needed;
    /// `None` once the input has ended.
    pub fn next(&mut self, mut input: impl FnMut() -> Option<[f32; 2]>) -> Option<[f32; 2]> {
        if !self.started {
            self.started = true;
            self.s0 = input()?;
            match input() {
                Some(f) => self.s1 = f,
                None => {
                    self.s1 = self.s0;
                    self.ended = true;
                }
            }
        }
        while self.pos >= 1 << 16 {
            if self.ended {
                return None;
            }
            self.pos -= 1 << 16;
            self.s0 = self.s1;
            match input() {
                Some(f) => self.s1 = f,
                None => self.ended = true,
            }
        }
        if self.ended && self.pos > 0 {
            return None;
        }
        let frac = self.pos as f32 / 65536.0;
        self.pos += self.step;
        Some([
            self.s0[0] + (self.s1[0] - self.s0[0]) * frac,
            self.s0[1] + (self.s1[1] - self.s0[1]) * frac,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, hz: f64, frames: usize) -> Vec<[f32; 2]> {
        (0..frames)
            .map(|k| {
                let v = (2.0 * std::f64::consts::PI * hz * k as f64 / f64::from(rate)).sin() as f32;
                [v * 0.5, v * 0.5]
            })
            .collect()
    }

    fn convert(input: &[[f32; 2]], from: u32, to: u32) -> Vec<f32> {
        let mut r = Resampler::new(from, to);
        let mut it = input.iter().copied();
        std::iter::from_fn(|| r.next(|| it.next()))
            .map(|f| f[0])
            .collect()
    }

    #[test]
    fn a_tone_keeps_its_pitch_and_level_and_its_length() {
        let input = tone(44_100, 1000.0, 44_100);
        let out = convert(&input, 44_100, MIXER_RATE);
        assert!((out.len() as i64 - 48_000).abs() <= 2, "{}", out.len());
        let peak = out[4800..43_200]
            .iter()
            .fold(0.0f32, |m, &v| m.max(v.abs()));
        assert!((peak - 0.5).abs() < 0.005, "level {peak}");
    }

    #[test]
    fn straight_lines_on_the_drivers_step() {
        // 44.1 kHz to 48 kHz: the step is 60211 / 65536 of an input frame.
        let input: Vec<[f32; 2]> = (0..8).map(|k| [k as f32 * 1000.0; 2]).collect();
        let out = convert(&input, 44_100, MIXER_RATE);
        for (n, v) in out.iter().enumerate() {
            let pos = n as u64 * 60_211;
            let want = (pos >> 16) as f32 * 1000.0 + (pos & 0xffff) as f32 / 65536.0 * 1000.0;
            assert!((v - want).abs() < 0.01, "{n}: {v} vs {want}");
        }
    }

    #[test]
    fn the_same_rate_passes_through() {
        let input = tone(MIXER_RATE, 440.0, 1000);
        let out = convert(&input, MIXER_RATE, MIXER_RATE);
        assert_eq!(out.len(), 1000);
        assert!(out.iter().zip(&input).all(|(a, b)| *a == b[0]));
    }
}
