//! Bounded LZARI decoder for MAX Drive archives.
//!
//! Algorithm: Haruhiko Okumura's LZARI; model ordering follows Ross Ridge's
//! public-domain mymc implementation (https://github.com/ps2dev/mymc).
//! This is a Rust implementation, with explicit input/output limits.
use crate::{Error, Result};

const N: usize = 4096;
const SYMBOLS: usize = 314;
const Q1: u64 = 32768;
const Q2: u64 = Q1 * 2;
const Q3: u64 = Q1 * 3;
const Q4: u64 = Q1 * 4;

struct Decoder<'a> {
    input: &'a [u8],
    bit: usize,
    low: u64,
    high: u64,
    code: u64,
    cumulative: [u64; SYMBOLS + 1],
    frequency: [u64; SYMBOLS + 1],
    characters: [usize; SYMBOLS + 1],
    positions: [u64; N + 1],
}

impl<'a> Decoder<'a> {
    fn new(input: &'a [u8]) -> Result<Self> {
        let mut this = Self {
            input,
            bit: 0,
            low: 0,
            high: Q4,
            code: 0,
            cumulative: std::array::from_fn(|i| i as u64),
            frequency: [1; SYMBOLS + 1],
            characters: std::array::from_fn(|i| i.saturating_sub(1)),
            positions: [0; N + 1],
        };
        this.frequency[0] = 0;
        for i in (1..=N).rev() {
            this.positions[i - 1] = this.positions[i] + 10000 / (200 + i as u64);
        }
        for _ in 0..17 {
            this.code = (this.code << 1) | this.read_bit()?;
        }
        Ok(this)
    }

    fn read_bit(&mut self) -> Result<u64> {
        // Arithmetic streams require up to 32 zero termination bits. Beyond
        // that, report truncation instead of decoding unlimited fabricated data.
        if self.bit >= self.input.len() * 8 + 32 {
            return Err(Error::format("truncated MAX LZARI stream"));
        }
        let value = self
            .input
            .get(self.bit / 8)
            .map_or(0, |b| u64::from((b >> (7 - self.bit % 8)) & 1));
        self.bit += 1;
        Ok(value)
    }

    fn scaled(&self, total: u64) -> Result<u64> {
        if self.high <= self.low || self.code < self.low || self.code >= self.high {
            return Err(Error::format("invalid MAX arithmetic interval"));
        }
        Ok(((self.code - self.low + 1) * total - 1) / (self.high - self.low))
    }

    fn interval(&mut self, lower: u64, upper: u64, total: u64) -> Result<()> {
        let range = self.high - self.low;
        self.high = self.low + range * upper / total;
        self.low += range * lower / total;
        loop {
            let subtract = if self.low >= Q2 {
                Q2
            } else if self.low >= Q1 && self.high <= Q3 {
                Q1
            } else if self.high <= Q2 {
                0
            } else {
                break;
            };
            self.low = (self.low - subtract) * 2;
            self.high = (self.high - subtract) * 2;
            self.code = (self.code - subtract) * 2 + self.read_bit()?;
        }
        Ok(())
    }

    fn character(&mut self) -> Result<usize> {
        let total = self.cumulative[SYMBOLS];
        let scaled = self.scaled(total)?;
        let index = self.cumulative.partition_point(|&n| n <= scaled);
        if index == 0 || index > SYMBOLS {
            return Err(Error::format("invalid MAX LZARI symbol"));
        }
        self.interval(self.cumulative[index - 1], self.cumulative[index], total)?;
        let symbol = SYMBOLS + 1 - index;
        let character = self.characters[symbol];
        self.update(symbol);
        Ok(character)
    }

    fn update(&mut self, symbol: usize) {
        if self.cumulative[SYMBOLS] >= Q1 - 1 {
            let mut total = 0;
            for i in (1..=SYMBOLS).rev() {
                self.cumulative[SYMBOLS - i] = total;
                self.frequency[i] = self.frequency[i].div_ceil(2);
                total += self.frequency[i];
            }
            self.cumulative[SYMBOLS] = total;
        }
        let frequency = self.frequency[symbol];
        let mut promoted = symbol;
        while self.frequency[promoted - 1] == frequency {
            promoted -= 1;
        }
        self.characters.swap(promoted, symbol);
        self.frequency[promoted] += 1;
        for cumulative in &mut self.cumulative[SYMBOLS - promoted + 1..] {
            *cumulative += 1;
        }
    }

    fn position(&mut self) -> Result<usize> {
        let total = self.positions[0];
        let scaled = self.scaled(total)?;
        let index = self.positions.partition_point(|&n| n > scaled);
        if index == 0 || index > N {
            return Err(Error::format("invalid MAX LZARI distance"));
        }
        self.interval(self.positions[index], self.positions[index - 1], total)?;
        Ok(index - 1)
    }
}

pub(crate) fn decode(input: &[u8], length: usize) -> Result<Vec<u8>> {
    if length == 0 {
        return Ok(Vec::new());
    }
    let mut decoder = Decoder::new(input)?;
    let mut history = [b' '; N];
    history[N - 60..].fill(0);
    let mut cursor = N - 60;
    let mut output = Vec::with_capacity(length);
    while output.len() < length {
        let character = decoder.character()?;
        if character < 256 {
            output.push(character as u8);
            history[cursor] = character as u8;
            cursor = (cursor + 1) % N;
        } else {
            let count = character - 256 + 3;
            if count > length - output.len() {
                return Err(Error::format("MAX LZARI match exceeds declared length"));
            }
            let position = decoder.position()?;
            let base = (cursor + N - position - 1) % N;
            for offset in 0..count {
                let value = history[(base + offset) % N];
                output.push(value);
                history[cursor] = value;
                cursor = (cursor + 1) % N;
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_domain_reference_vector_includes_literals_and_overlap_matches() {
        // Independently encoded using public-domain mymc lzari_codec, with
        // synthetic text: no game bytes are embedded in this test vector.
        let stream = [
            0xa1, 0xfc, 0x69, 0xdf, 0x00, 0xe5, 0x1e, 0x9c, 0xdc, 0xb9, 0x78, 0x57, 0x37, 0x97,
            0x20, 0xa8, 0x62, 0xb2, 0x5b, 0xc5, 0xec, 0xaa, 0x1c, 0xfc, 0xb7, 0x0f, 0xda, 0x6b,
            0xb7, 0x66, 0x10,
        ];
        let expected = b"safe Rust saves: ababababababababababab end";
        assert_eq!(decode(&stream, expected.len()).unwrap(), expected);
        assert!(decode(&stream[..10], expected.len()).is_err());
        assert!(decode(&stream, 5000).is_err());
        assert!(decode(&[], 5000).is_err());
    }
}
