//! Byte-level reading and writing: LEB128 varints, fixed-width integers,
//! length-prefixed byte strings, and pad inputs.

use std::fmt;

use street_sim::Input;

/// Why a datagram could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The first byte is not [`MAGIC`](crate::protocol::MAGIC).
    NotOurs,
    /// Sent with another protocol version.
    Version(u8),
    /// The datagram ended early.
    Short,
    /// A field holds a value it cannot.
    Bad(&'static str),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::NotOurs => write!(f, "not a street-net datagram"),
            DecodeError::Version(v) => write!(f, "protocol version {v}"),
            DecodeError::Short => write!(f, "datagram cut short"),
            DecodeError::Bad(what) => write!(f, "bad {what}"),
        }
    }
}

impl std::error::Error for DecodeError {}

pub type Result<T> = std::result::Result<T, DecodeError>;

/// Appends fields to a buffer.
#[derive(Default)]
pub struct Writer(pub Vec<u8>);

impl Writer {
    pub fn u8(&mut self, v: u8) {
        self.0.push(v);
    }

    pub fn bool(&mut self, v: bool) {
        self.0.push(u8::from(v));
    }

    pub fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    /// An unsigned LEB128 varint.
    pub fn uv(&mut self, mut v: u64) {
        loop {
            let byte = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.0.push(byte);
                return;
            }
            self.0.push(byte | 0x80);
        }
    }

    /// A signed varint (zigzag).
    pub fn iv(&mut self, v: i64) {
        self.uv(((v << 1) ^ (v >> 63)) as u64);
    }

    /// A varint length, then the bytes.
    pub fn bytes(&mut self, b: &[u8]) {
        self.uv(b.len() as u64);
        self.0.extend_from_slice(b);
    }

    pub fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }

    pub fn input(&mut self, i: &Input) {
        self.0.extend_from_slice(&i.to_bytes());
    }
}

/// Reads fields from a datagram.
pub struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Self { b, at: 0 }
    }

    /// True when every byte has been read.
    pub fn done(&self) -> bool {
        self.at == self.b.len()
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&e| e <= self.b.len())
            .ok_or(DecodeError::Short)?;
        let s = &self.b[self.at..end];
        self.at = end;
        Ok(s)
    }

    /// Everything not read yet.
    pub fn rest(&mut self) -> &'a [u8] {
        let s = &self.b[self.at..];
        self.at = self.b.len();
        s
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn bool(&mut self) -> Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(DecodeError::Bad("flag")),
        }
    }

    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().expect("2")))
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("4")))
    }

    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().expect("8")))
    }

    pub fn uv(&mut self) -> Result<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.u8()?;
            v |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(DecodeError::Bad("varint"))
    }

    pub fn iv(&mut self) -> Result<i64> {
        let u = self.uv()?;
        Ok(((u >> 1) as i64) ^ -((u & 1) as i64))
    }

    /// A varint length of at most `max`.
    pub fn len(&mut self, max: usize) -> Result<usize> {
        let n = self.uv()?;
        if n > max as u64 {
            return Err(DecodeError::Bad("length"));
        }
        Ok(n as usize)
    }

    /// Length-prefixed bytes, at most `max` of them.
    pub fn bytes(&mut self, max: usize) -> Result<&'a [u8]> {
        let n = self.len(max)?;
        self.take(n)
    }

    pub fn str(&mut self, max: usize) -> Result<String> {
        let b = self.bytes(max)?;
        String::from_utf8(b.to_vec()).map_err(|_| DecodeError::Bad("text"))
    }

    pub fn input(&mut self) -> Result<Input> {
        let b: [u8; Input::BYTES] = self.take(Input::BYTES)?.try_into().expect("8");
        Ok(Input::from_bytes(b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_round_trip() {
        let values = [0, 1, 127, 128, 300, u32::MAX as u64, u64::MAX];
        let mut w = Writer::default();
        for &v in &values {
            w.uv(v);
        }
        for v in [0i64, -1, 1, -64, 64, i64::MIN, i64::MAX] {
            w.iv(v);
        }
        let mut r = Reader::new(&w.0);
        for &v in &values {
            assert_eq!(r.uv().unwrap(), v);
        }
        for v in [0i64, -1, 1, -64, 64, i64::MIN, i64::MAX] {
            assert_eq!(r.iv().unwrap(), v);
        }
        assert!(r.done());
        assert_eq!(r.u8(), Err(DecodeError::Short));
    }
}
