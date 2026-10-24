//! The movies' container (`/MOVIES/*.NFX`): chunks, each a four-byte
//! tag and a little-endian u32 size that counts the eight header bytes.
//!
//! - `MPCh`: one MPEG-2 video picture's worth of the elementary stream (the
//!   first carries the sequence header); joined, they are the whole stream.
//! - `SCHl`, `SCCl`, `SCDl`, `SCEl`: the soundtrack, a SCHl stream cut
//!   up between the pictures; joined, `street-audio`'s SCHl reader plays
//!   it. The loading loops (`XLP*`) and most clips have no sound.

use street_assets::{Error, Result};

pub struct Movie {
    /// The MPEG-2 video elementary stream.
    pub video: Vec<u8>,
    /// The SCHl soundtrack, when there is one.
    pub audio: Option<Vec<u8>>,
}

impl Movie {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut video = Vec::with_capacity(data.len());
        let mut audio = Vec::new();
        let mut pos = 0;
        while pos + 8 <= data.len() {
            let tag = &data[pos..pos + 4];
            let size =
                u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
                    as usize;
            if size < 8 || pos + size > data.len() {
                return Err(Error::Format(format!(
                    "NFX chunk at {pos:#x} has size {size}"
                )));
            }
            match tag {
                b"MPCh" => video.extend_from_slice(&data[pos + 8..pos + size]),
                b"SCHl" | b"SCCl" | b"SCDl" | b"SCLl" | b"SCEl" => {
                    audio.extend_from_slice(&data[pos..pos + size])
                }
                _ => {}
            }
            pos += size;
        }
        if video.is_empty() {
            return Err(Error::Format("NFX without video".into()));
        }
        Ok(Movie {
            video,
            audio: (!audio.is_empty()).then_some(audio),
        })
    }
}
