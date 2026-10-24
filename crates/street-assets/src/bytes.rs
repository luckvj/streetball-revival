//! Bounds-checked little/big-endian reads. Every read past the end of a buffer
//! becomes a `Format` error instead of a panic, since all input is untrusted disc data.

use crate::{Error, Result};

fn slice<const N: usize>(b: &[u8], off: usize) -> Result<[u8; N]> {
    off.checked_add(N)
        .and_then(|end| b.get(off..end))
        .map(|s| s.try_into().expect("slice length is N"))
        .ok_or_else(|| {
            Error::format(format!(
                "read of {N} bytes at {off:#x} past end ({:#x})",
                b.len()
            ))
        })
}

pub(crate) fn u16_le(b: &[u8], off: usize) -> Result<u16> {
    slice(b, off).map(u16::from_le_bytes)
}

pub(crate) fn u32_le(b: &[u8], off: usize) -> Result<u32> {
    slice(b, off).map(u32::from_le_bytes)
}

pub(crate) fn u32_be(b: &[u8], off: usize) -> Result<u32> {
    slice(b, off).map(u32::from_be_bytes)
}

pub(crate) fn u16_be(b: &[u8], off: usize) -> Result<u16> {
    slice(b, off).map(u16::from_be_bytes)
}

pub(crate) fn i16_be(b: &[u8], off: usize) -> Result<i16> {
    slice(b, off).map(i16::from_be_bytes)
}

pub(crate) fn f32_be(b: &[u8], off: usize) -> Result<f32> {
    slice(b, off).map(f32::from_be_bytes)
}

pub(crate) fn f32_le(b: &[u8], off: usize) -> Result<f32> {
    slice(b, off).map(f32::from_le_bytes)
}

/// A NUL-terminated string starting at `off`, decoded as Latin-1.
pub(crate) fn cstr(b: &[u8], off: usize) -> Result<String> {
    let rest = b
        .get(off..)
        .ok_or_else(|| Error::format(format!("string at {off:#x} past end")))?;
    let len = rest
        .iter()
        .position(|&c| c == 0)
        .ok_or_else(|| Error::format(format!("unterminated string at {off:#x}")))?;
    Ok(rest[..len].iter().map(|&c| c as char).collect())
}

/// `b[off..off + len]`, or a `Format` error.
pub(crate) fn range(b: &[u8], off: usize, len: usize) -> Result<&[u8]> {
    off.checked_add(len)
        .and_then(|end| b.get(off..end))
        .ok_or_else(|| Error::format(format!("range {off:#x}+{len:#x} past end ({:#x})", b.len())))
}
