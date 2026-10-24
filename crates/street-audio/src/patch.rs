//! "PT" patch headers: the per-sound description in SCHl streams and BNKl
//! banks.
//!
//! After `PT`, a platform byte (5 = PS2) and a zero, the header is a list of
//! tags: `0xfc`, `0xfd` and `0xfe` mark sections and `0xff` ends the list; any
//! other byte is a tag followed by a length byte and that many bytes of
//! big-endian value. Tags read here:
//!
//! | Tag | Meaning |
//! |---|---|
//! | 0x80 | header version (2 on this disc) |
//! | 0x82 | channels (default 1) |
//! | 0x84 | sample rate (default 22050 on PS2) |
//! | 0x85 | sample count |
//! | 0x86, 0x87 | loop start and end, in samples |
//! | 0x88, 0x89 | (banks) offset of channel 0 and 1's data from the bank start |
//! | 0xa0 | codec: 0x0a XA; absent means PS-ADPCM on PS2 |
//!
//! Others seen and not needed: 0x06 priority, 0x0b, 0x13 volume (0x7f),
//! 0x8a, 0x8c, 0x9c and 0x9d (stream sync), 0x14 (a `SYNC` record).

use std::collections::BTreeMap;

use street_assets::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    EaXa,
    Vag,
}

#[derive(Clone, Debug)]
pub struct Patch {
    pub channels: u16,
    pub rate: u32,
    pub samples: u32,
    pub codec: Codec,
    /// Loop start and end in samples, when the sound loops.
    pub looping: Option<(u32, u32)>,
    /// Every integer tag, for anything not decoded above.
    pub tags: BTreeMap<u8, u32>,
}

pub const DEFAULT_RATE: u32 = 22050;

impl Patch {
    /// Parses the header at `data[0]`, which must start with `PT`.
    /// Returns the patch and the offset just past the end tag.
    pub fn parse(data: &[u8]) -> Result<(Self, usize)> {
        if data.get(..2) != Some(b"PT") {
            return Err(Error::Format("no PT patch header".into()));
        }
        let mut tags = BTreeMap::new();
        let mut pos = 4;
        loop {
            let tag = *data
                .get(pos)
                .ok_or_else(|| Error::Format("PT header runs past its data".into()))?;
            pos += 1;
            match tag {
                0xff => break,
                0xfc..=0xfe => continue,
                _ => {
                    let len = *data
                        .get(pos)
                        .ok_or_else(|| Error::Format("PT tag without a length".into()))?
                        as usize;
                    let v = data
                        .get(pos + 1..pos + 1 + len)
                        .ok_or_else(|| Error::Format("PT tag value past the end".into()))?;
                    if len <= 4 {
                        tags.insert(tag, v.iter().fold(0u32, |a, &b| a << 8 | b as u32));
                    }
                    pos += 1 + len;
                }
            }
        }
        let samples = *tags
            .get(&0x85)
            .ok_or_else(|| Error::Format("PT header has no sample count".into()))?;
        let looping = match (tags.get(&0x86), tags.get(&0x87)) {
            (Some(&s), Some(&e)) if e > s => Some((s, e)),
            (None, Some(&e)) if e > 0 => Some((0, e)),
            _ => None,
        };
        let patch = Patch {
            channels: tags.get(&0x82).copied().unwrap_or(1).max(1) as u16,
            rate: tags.get(&0x84).copied().unwrap_or(DEFAULT_RATE),
            samples,
            codec: match tags.get(&0xa0) {
                Some(0x0a) => Codec::EaXa,
                None => Codec::Vag,
                Some(c) => return Err(Error::Format(format!("unsupported codec {c:#x}"))),
            },
            looping,
            tags,
        };
        Ok((patch, pos))
    }

    /// The offset of channel `c`'s data in a bank (tags 0x88 and up).
    pub fn data_offset(&self, c: u16) -> Option<u32> {
        self.tags.get(&(0x88 + c as u8)).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_bank_sound() {
        // From a loading-loop bank: stereo... trimmed to the tags that matter.
        let h = [
            b'P', b'T', 5, 0, 0x06, 1, 0x64, 0xfd, 0x80, 1, 2, 0x85, 3, 0x06, 0xcd, 0x16, 0x82, 1,
            2, 0xa0, 1, 0x0a, 0x84, 3, 0x00, 0xac, 0x44, 0x86, 0, 0x87, 3, 0x06, 0xcd, 0x15, 0x88,
            4, 0, 0, 0, 0x64, 0xff,
        ];
        let (p, end) = Patch::parse(&h).unwrap();
        assert_eq!(end, h.len());
        assert_eq!((p.channels, p.rate, p.samples), (2, 44100, 0x06cd16));
        assert_eq!(p.codec, Codec::EaXa);
        assert_eq!(p.looping, Some((0, 0x06cd15)));
        assert_eq!(p.data_offset(0), Some(0x64));
    }

    #[test]
    fn missing_codec_means_ps_adpcm() {
        let h = [b'P', b'T', 5, 0, 0x85, 1, 0x54, 0xff];
        let (p, _) = Patch::parse(&h).unwrap();
        assert_eq!((p.codec, p.rate, p.channels), (Codec::Vag, DEFAULT_RATE, 1));
    }
}
