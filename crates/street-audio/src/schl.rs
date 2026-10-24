//! SCHl streams: the front-end songs (`.ASF`), the in-game music segments
//! (`.MUS`), the crowd beds and calls (`.AST`) and the commentary clips
//! (`SPCHDATS.BIG`).
//!
//! A stream is a run of little-endian blocks, each `tag`, u32 size (header
//! included), payload:
//!
//! - `SCHl`: `PT` patch header (see [`crate::patch`]);
//! - `SCCl`: u32 count of data blocks;
//! - `SCDl`: u32 sample count, u32 offset per channel (from the end of the
//!   offsets), then each channel's XA history and frames;
//! - `SCLl`: loop marker (skipped);
//! - `SCEl`: end of stream.
//!
//! Files hold several streams back to back, each padded to 0x80 bytes.

use std::sync::Arc;

use street_assets::{Error, Result};

use crate::{
    Clip,
    adpcm::{EaXa, FRAME_SAMPLES},
    patch::{Codec, Patch},
};

/// One stream inside a file.
#[derive(Clone, Debug)]
pub struct Stream {
    pub patch: Patch,
    /// Offset of the `SCHl` block.
    pub start: usize,
    /// Offset of the first block after `SCHl`.
    pub blocks: usize,
    /// Offset just past `SCEl` (or the end of the data).
    pub end: usize,
}

fn u32_at(data: &[u8], pos: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(pos..pos + 4)?.try_into().ok()?))
}

impl Stream {
    /// The stream whose `SCHl` block is at `start`.
    pub fn at(data: &[u8], start: usize) -> Result<Self> {
        if data.get(start..start + 4) != Some(b"SCHl") {
            return Err(Error::Format(format!("no SCHl block at {start:#x}")));
        }
        let size = u32_at(data, start + 4).unwrap_or(0) as usize;
        let (patch, _) = Patch::parse(&data[start + 8..])?;
        if patch.codec != Codec::EaXa {
            return Err(Error::Format("SCHl stream is not XA".into()));
        }
        let blocks = start + size.max(8);
        let mut pos = blocks;
        while let (Some(tag), Some(len)) = (data.get(pos..pos + 4), u32_at(data, pos + 4)) {
            if len < 8 {
                break;
            }
            pos += len as usize;
            if tag == b"SCEl" {
                break;
            }
        }
        Ok(Stream {
            patch,
            start,
            blocks,
            end: pos.min(data.len()),
        })
    }

    /// Decodes the whole stream.
    pub fn decode(&self, data: &[u8]) -> Clip {
        let mut reader = Reader::new(
            Arc::from(&data[self.start..self.end]),
            self.clone_at(0),
            false,
        );
        let mut samples =
            Vec::with_capacity(self.patch.samples as usize * self.patch.channels as usize);
        samples.extend(&mut reader);
        Clip {
            rate: self.patch.rate,
            channels: self.patch.channels,
            samples,
            looping: self.patch.looping,
        }
    }

    /// This stream described relative to a copy of its bytes that starts at
    /// `start` (for [`Reader`] over a slice of the file).
    pub fn clone_at(&self, start: usize) -> Stream {
        Stream {
            patch: self.patch.clone(),
            start,
            blocks: self.blocks - self.start + start,
            end: self.end - self.start + start,
        }
    }
}

/// Every stream in a file, in order.
pub fn streams(data: &[u8]) -> Result<Vec<Stream>> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos + 8 <= data.len() {
        if &data[pos..pos + 4] == b"SCHl" {
            let s = Stream::at(data, pos)?;
            pos = s.end.max(pos + 8);
            out.push(s);
        } else {
            // Streams start on 16-byte boundaries after their padding.
            pos = (pos + 16) & !15;
        }
    }
    Ok(out)
}

/// Decodes a stream block by block as samples are read: interleaved `i16`,
/// optionally starting over at the end (for crowd beds and music).
pub struct Reader {
    data: Arc<[u8]>,
    stream: Stream,
    pos: usize,
    block: Vec<i16>,
    next: usize,
    looping: bool,
}

impl Reader {
    /// `data` holds the stream; `stream` must be relative to it.
    pub fn new(data: Arc<[u8]>, stream: Stream, looping: bool) -> Self {
        Reader {
            pos: stream.blocks,
            data,
            stream,
            block: Vec::new(),
            next: 0,
            looping,
        }
    }

    pub fn channels(&self) -> u16 {
        self.stream.patch.channels
    }

    pub fn rate(&self) -> u32 {
        self.stream.patch.rate
    }

    /// Decodes the next data block into `self.block`. False at the end.
    fn fill(&mut self) -> bool {
        let ch = self.stream.patch.channels as usize;
        let data = &self.data;
        while self.pos + 8 <= self.stream.end {
            let tag = &data[self.pos..self.pos + 4];
            let len = u32_at(data, self.pos + 4).unwrap_or(0) as usize;
            if len < 8 {
                return false;
            }
            let here = self.pos;
            self.pos += len;
            if tag == b"SCEl" {
                break;
            }
            if tag != b"SCDl" {
                continue;
            }
            let Some(n) = u32_at(data, here + 8) else {
                return false;
            };
            let n = n as usize;
            if n == 0 {
                continue;
            }
            let base = here + 12 + 4 * ch;
            let end = (here + len).min(data.len());
            self.block.clear();
            self.block.resize(n * ch, 0);
            for c in 0..ch {
                let Some(off) = u32_at(data, here + 12 + 4 * c) else {
                    return false;
                };
                let start = base + off as usize;
                let Some(chunk) = data.get(start..end) else {
                    continue;
                };
                let Some(mut dec) = EaXa::with_history(chunk) else {
                    continue;
                };
                let mut p = 4;
                let mut buf = [0i16; FRAME_SAMPLES];
                let mut i = 0;
                while i < n {
                    let Some(used) = dec.frame(&chunk[p.min(chunk.len())..], &mut buf) else {
                        break;
                    };
                    p += used;
                    for &s in buf.iter().take(n - i) {
                        self.block[i * ch + c] = s;
                        i += 1;
                    }
                }
            }
            self.next = 0;
            return true;
        }
        if self.looping {
            self.pos = self.stream.blocks;
            // Guard against a stream with no data blocks at all.
            return self.fill_once();
        }
        false
    }

    fn fill_once(&mut self) -> bool {
        self.looping = false;
        let ok = self.fill();
        self.looping = true;
        ok
    }
}

impl Iterator for Reader {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        while self.next >= self.block.len() {
            if !self.fill() {
                return None;
            }
        }
        let s = self.block[self.next];
        self.next += 1;
        Some(s)
    }
}
