//! RefPack (QFS) compression. Street V3 stores most archive entries
//! packed (its models, sheets and database tables all start `10 fb`).
//!
//! Header: a flags byte and `0xfb`, an optional packed size (flag 0x01), then
//! the unpacked size, three bytes big-endian (four with flag 0x80). Commands
//! follow, each copying some literal bytes from the input and then some bytes
//! from earlier output:
//!
//! | First byte | Bytes | Literal | Copy length | Copy distance |
//! |---|---|---|---|---|
//! | `0x00..0x7f` | 2 | `b0 & 3` | `(b0 >> 2 & 7) + 3` | `(b0 & 0x60) << 3 + b1 + 1` |
//! | `0x80..0xbf` | 3 | `b1 >> 6` | `(b0 & 0x3f) + 4` | `(b1 & 0x3f) << 8 + b2 + 1` |
//! | `0xc0..0xdf` | 4 | `b0 & 3` | `(b0 & 0x0c) << 6 + b3 + 5` | `(b0 & 0x10) << 12 + b1 << 8 + b2 + 1` |
//! | `0xe0..0xfb` | 1 | `(b0 & 0x1f) * 4 + 4` | none | |
//! | `0xfc..0xff` | 1 | `b0 & 3`, then stop | none | |

use crate::{Error, Result};

/// Whether `b` starts with a RefPack header.
pub fn is_packed(b: &[u8]) -> bool {
    b.len() >= 5 && b[1] == 0xfb && b[0] & 0x3e == 0x10
}

/// Unpacks `b`, or returns a copy of it when it is not RefPack data.
pub fn unpack(b: &[u8]) -> Result<Vec<u8>> {
    if !is_packed(b) {
        return Ok(b.to_vec());
    }
    let flags = b[0];
    let width = if flags & 0x80 != 0 { 4 } else { 3 };
    let mut i = 2 + if flags & 0x01 != 0 { width } else { 0 };
    let size = be(b, i, width)?;
    i += width;
    // A header claiming more than 512 MB is not one of the game's files.
    if size > 512 << 20 {
        return Err(Error::format(format!("RefPack size {size} is implausible")));
    }
    let mut out = Vec::with_capacity(size);
    let at = |k: usize| {
        b.get(k)
            .copied()
            .ok_or_else(|| Error::format("RefPack data ends early"))
    };
    while out.len() < size {
        let b0 = at(i)?;
        let (literal, length, distance, step) = match b0 {
            0x00..=0x7f => {
                let b1 = at(i + 1)? as usize;
                (
                    (b0 & 3) as usize,
                    ((b0 >> 2) & 7) as usize + 3,
                    (((b0 & 0x60) as usize) << 3) + b1 + 1,
                    2,
                )
            }
            0x80..=0xbf => {
                let (b1, b2) = (at(i + 1)? as usize, at(i + 2)? as usize);
                (
                    (b1 >> 6),
                    (b0 & 0x3f) as usize + 4,
                    ((b1 & 0x3f) << 8) + b2 + 1,
                    3,
                )
            }
            0xc0..=0xdf => {
                let (b1, b2, b3) = (
                    at(i + 1)? as usize,
                    at(i + 2)? as usize,
                    at(i + 3)? as usize,
                );
                (
                    (b0 & 3) as usize,
                    (((b0 & 0x0c) as usize) << 6) + b3 + 5,
                    (((b0 & 0x10) as usize) << 12) + (b1 << 8) + b2 + 1,
                    4,
                )
            }
            0xe0..=0xfb => ((((b0 & 0x1f) as usize) << 2) + 4, 0, 0, 1),
            0xfc..=0xff => {
                let n = (b0 & 3) as usize;
                out.extend_from_slice(literal_bytes(b, i + 1, n)?);
                break;
            }
        };
        i += step;
        out.extend_from_slice(literal_bytes(b, i, literal)?);
        i += literal;
        if length > 0 {
            if distance > out.len() {
                return Err(Error::format("RefPack copies from before the start"));
            }
            let start = out.len() - distance;
            // The copy may overlap what it writes, so it goes a byte at a time.
            for k in 0..length {
                let byte = out[start + k];
                out.push(byte);
            }
        }
    }
    if out.len() != size {
        return Err(Error::format(format!(
            "RefPack unpacked {} bytes, header says {size}",
            out.len()
        )));
    }
    Ok(out)
}

fn be(b: &[u8], off: usize, n: usize) -> Result<usize> {
    let s = b
        .get(off..off + n)
        .ok_or_else(|| Error::format("RefPack header ends early"))?;
    Ok(s.iter().fold(0usize, |v, &x| v << 8 | x as usize))
}

fn literal_bytes(b: &[u8], off: usize, n: usize) -> Result<&[u8]> {
    b.get(off..off + n)
        .ok_or_else(|| Error::format("RefPack literal runs past the end"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpacked_data_is_returned_as_is() {
        assert_eq!(unpack(b"BIGF....").unwrap(), b"BIGF....");
    }

    #[test]
    fn literals_and_a_short_back_reference() {
        // 8 bytes: "abcd" as a long literal (0xe0: 4 bytes), then a 2-byte
        // command with no literal copying 4 bytes from distance 4.
        let packed = [
            0x10, 0xfb, 0, 0, 8, 0xe0, b'a', b'b', b'c', b'd', 0x04, 0x03, 0xfc,
        ];
        assert_eq!(unpack(&packed).unwrap(), b"abcdabcd");
    }

    #[test]
    fn overlapping_copy_repeats_bytes() {
        // "a", then copy 5 from distance 1: "aaaaaa".
        let packed = [0x10, 0xfb, 0, 0, 6, 0x09, 0x00, b'a', 0xfc];
        assert_eq!(unpack(&packed).unwrap(), b"aaaaaa");
    }

    #[test]
    fn rejects_a_copy_from_before_the_start() {
        let packed = [0x10, 0xfb, 0, 0, 4, 0x04, 0x05, 0xfc];
        assert!(unpack(&packed).is_err());
    }
}
