//! Sound banks: `BNKl` banks and the AEMS `.ABK` files that carry one.
//!
//! `BNKl`, u16 version (5), u16 sound count, u32 bank size, 8 bytes, then one
//! u32 per sound: the offset of its `PT` header from that u32 (0 = empty
//! slot). A sound's data offsets (tags 0x88, 0x89) count from the `BNKl`
//! tag. Channels are stored one after the other, each a complete mono stream
//! (XA frames without the history that SCHl blocks carry, or PS-ADPCM).
//!
//! An `.ABK` (`AB`, version 9) is an AEMS module bank: AEMS scripts (read
//! by [`crate::aems`]) and a `BNKl`, whose offset is the u32 at 0x1c. Sounds the scripts stream from the matching `.AST` are not in it;
//! `SPCH_PLY.ABK` streams everything and has no `BNKl` at all.

use street_assets::{Error, Result};

use crate::{
    Clip, adpcm,
    patch::{Codec, Patch},
};

pub struct Bank {
    data: Vec<u8>,
    /// Offset of the `BNKl` tag in `data`.
    base: usize,
    /// One entry per slot; `None` for an empty slot.
    pub sounds: Vec<Option<Patch>>,
}

impl Bank {
    /// Parses a bare `BNKl` bank or an `.ABK` file.
    pub fn parse(data: Vec<u8>) -> Result<Self> {
        let base = if data.starts_with(b"BNKl") {
            0
        } else if data.starts_with(b"AB") {
            let at = data
                .get(0x1c..0x20)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
                .filter(|&o| data.get(o..o + 4) == Some(b"BNKl"));
            match at.or_else(|| find(&data, b"BNKl")) {
                Some(o) => o,
                // Every sound streams from the matching .AST (the player
                // speech bank, `SPCH_PLY.ABK`): an empty bank.
                None => {
                    return Ok(Bank {
                        data,
                        base: 0,
                        sounds: Vec::new(),
                    });
                }
            }
        } else {
            return Err(Error::Format("not a BNKl or ABK sound bank".into()));
        };
        let count = data
            .get(base + 6..base + 8)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
            .ok_or_else(|| Error::Format("BNKl header cut short".into()))?;
        let mut sounds = Vec::with_capacity(count);
        for i in 0..count {
            let at = base + 0x14 + 4 * i;
            let rel = data
                .get(at..at + 4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
                .ok_or_else(|| Error::Format("BNKl offset table cut short".into()))?;
            sounds.push(if rel == 0 {
                None
            } else {
                Some(Patch::parse(&data[(at + rel).min(data.len())..])?.0)
            });
        }
        Ok(Bank { data, base, sounds })
    }

    pub fn len(&self) -> usize {
        self.sounds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sounds.is_empty()
    }

    /// Decodes sound `i`, interleaving its channels.
    pub fn clip(&self, i: usize) -> Result<Clip> {
        let p = self
            .sounds
            .get(i)
            .and_then(|s| s.as_ref())
            .ok_or_else(|| Error::NotFound(format!("bank sound {i}")))?;
        let n = p.samples as usize;
        let mut chans = Vec::with_capacity(p.channels as usize);
        for c in 0..p.channels {
            let off = p.data_offset(c).ok_or_else(|| {
                Error::Format(format!("bank sound {i} has no data for channel {c}"))
            })?;
            let data = self
                .data
                .get(self.base + off as usize..)
                .ok_or_else(|| Error::Format(format!("bank sound {i} data past the end")))?;
            let mut s = match p.codec {
                Codec::EaXa => adpcm::ea_xa_bank(data, n),
                Codec::Vag => adpcm::vag(data, n),
            };
            // The last frame can run past the end of the file; pad it.
            s.resize(n, 0);
            chans.push(s);
        }
        let mut samples = Vec::with_capacity(n * chans.len());
        for i in 0..n {
            for c in &chans {
                samples.push(c[i]);
            }
        }
        Ok(Clip {
            rate: p.rate,
            channels: p.channels,
            samples,
            looping: p.looping,
        })
    }
}

fn find(data: &[u8], tag: &[u8]) -> Option<usize> {
    data.windows(tag.len()).position(|w| w == tag)
}
