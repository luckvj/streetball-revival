//! The MPEG-2 video tables (ISO/IEC 13818-2 annex B and section 7), as
//! `(code, length)` pairs and the values they stand for.

/// Table B-1, macroblock_address_increment: (code, length, increment);
/// increment 0 is the escape (+33).
pub const MB_ADDRESS: &[(u16, u8, u8)] = &[
    (0b1, 1, 1),
    (0b011, 3, 2),
    (0b010, 3, 3),
    (0b0011, 4, 4),
    (0b0010, 4, 5),
    (0b00011, 5, 6),
    (0b00010, 5, 7),
    (0b0000111, 7, 8),
    (0b0000110, 7, 9),
    (0b00001011, 8, 10),
    (0b00001010, 8, 11),
    (0b00001001, 8, 12),
    (0b00001000, 8, 13),
    (0b00000111, 8, 14),
    (0b00000110, 8, 15),
    (0b0000010111, 10, 16),
    (0b0000010110, 10, 17),
    (0b0000010101, 10, 18),
    (0b0000010100, 10, 19),
    (0b0000010011, 10, 20),
    (0b0000010010, 10, 21),
    (0b00000100011, 11, 22),
    (0b00000100010, 11, 23),
    (0b00000100001, 11, 24),
    (0b00000100000, 11, 25),
    (0b00000011111, 11, 26),
    (0b00000011110, 11, 27),
    (0b00000011101, 11, 28),
    (0b00000011100, 11, 29),
    (0b00000011011, 11, 30),
    (0b00000011010, 11, 31),
    (0b00000011001, 11, 32),
    (0b00000011000, 11, 33),
    (0b00000001000, 11, 0),
];

/// Macroblock type flags.
pub const QUANT: u8 = 1;
pub const FORWARD: u8 = 2;
pub const BACKWARD: u8 = 4;
pub const PATTERN: u8 = 8;
pub const INTRA: u8 = 16;

/// Table B-2, macroblock_type in I pictures.
pub const MB_TYPE_I: &[(u16, u8, u8)] = &[(0b1, 1, INTRA), (0b01, 2, INTRA | QUANT)];

/// Table B-3, macroblock_type in P pictures.
pub const MB_TYPE_P: &[(u16, u8, u8)] = &[
    (0b1, 1, FORWARD | PATTERN),
    (0b01, 2, PATTERN),
    (0b001, 3, FORWARD),
    (0b00011, 5, INTRA),
    (0b00010, 5, QUANT | FORWARD | PATTERN),
    (0b00001, 5, QUANT | PATTERN),
    (0b000001, 6, QUANT | INTRA),
];

/// Table B-4, macroblock_type in B pictures.
pub const MB_TYPE_B: &[(u16, u8, u8)] = &[
    (0b10, 2, FORWARD | BACKWARD),
    (0b11, 2, FORWARD | BACKWARD | PATTERN),
    (0b010, 3, BACKWARD),
    (0b011, 3, BACKWARD | PATTERN),
    (0b0010, 4, FORWARD),
    (0b0011, 4, FORWARD | PATTERN),
    (0b00011, 5, INTRA),
    (0b00010, 5, QUANT | FORWARD | BACKWARD | PATTERN),
    (0b000011, 6, QUANT | FORWARD | PATTERN),
    (0b000010, 6, QUANT | BACKWARD | PATTERN),
    (0b000001, 6, QUANT | INTRA),
];

/// Table B-9, coded_block_pattern (4:2:0).
pub const CBP: &[(u16, u8, u8)] = &[
    (0b111, 3, 60),
    (0b1101, 4, 4),
    (0b1100, 4, 8),
    (0b1011, 4, 16),
    (0b1010, 4, 32),
    (0b10011, 5, 12),
    (0b10010, 5, 48),
    (0b10001, 5, 20),
    (0b10000, 5, 40),
    (0b01111, 5, 28),
    (0b01110, 5, 44),
    (0b01101, 5, 52),
    (0b01100, 5, 56),
    (0b01011, 5, 1),
    (0b01010, 5, 61),
    (0b01001, 5, 2),
    (0b01000, 5, 62),
    (0b001111, 6, 24),
    (0b001110, 6, 36),
    (0b001101, 6, 3),
    (0b001100, 6, 63),
    (0b0010111, 7, 5),
    (0b0010110, 7, 9),
    (0b0010101, 7, 17),
    (0b0010100, 7, 33),
    (0b0010011, 7, 6),
    (0b0010010, 7, 10),
    (0b0010001, 7, 18),
    (0b0010000, 7, 34),
    (0b00011111, 8, 7),
    (0b00011110, 8, 11),
    (0b00011101, 8, 19),
    (0b00011100, 8, 35),
    (0b00011011, 8, 13),
    (0b00011010, 8, 49),
    (0b00011001, 8, 21),
    (0b00011000, 8, 41),
    (0b00010111, 8, 14),
    (0b00010110, 8, 50),
    (0b00010101, 8, 22),
    (0b00010100, 8, 42),
    (0b00010011, 8, 15),
    (0b00010010, 8, 51),
    (0b00010001, 8, 23),
    (0b00010000, 8, 43),
    (0b00001111, 8, 25),
    (0b00001110, 8, 37),
    (0b00001101, 8, 26),
    (0b00001100, 8, 38),
    (0b00001011, 8, 29),
    (0b00001010, 8, 45),
    (0b00001001, 8, 53),
    (0b00001000, 8, 57),
    (0b00000111, 8, 30),
    (0b00000110, 8, 46),
    (0b00000101, 8, 54),
    (0b00000100, 8, 58),
    (0b000000111, 9, 31),
    (0b000000110, 9, 47),
    (0b000000101, 9, 55),
    (0b000000100, 9, 59),
    (0b000000011, 9, 27),
    (0b000000010, 9, 39),
    (0b000000001, 9, 0),
];

/// Table B-10, motion_code magnitudes (a sign bit follows all but 0).
pub const MOTION: &[(u16, u8, u8)] = &[
    (0b1, 1, 0),
    (0b01, 2, 1),
    (0b001, 3, 2),
    (0b0001, 4, 3),
    (0b000011, 6, 4),
    (0b0000101, 7, 5),
    (0b0000100, 7, 6),
    (0b0000011, 7, 7),
    (0b000001011, 9, 8),
    (0b000001010, 9, 9),
    (0b000001001, 9, 10),
    (0b0000010001, 10, 11),
    (0b0000010000, 10, 12),
    (0b0000001111, 10, 13),
    (0b0000001110, 10, 14),
    (0b0000001101, 10, 15),
    (0b0000001100, 10, 16),
];

/// Table B-12, dct_dc_size_luminance.
pub const DC_LUMA: &[(u16, u8, u8)] = &[
    (0b100, 3, 0),
    (0b00, 2, 1),
    (0b01, 2, 2),
    (0b101, 3, 3),
    (0b110, 3, 4),
    (0b1110, 4, 5),
    (0b11110, 5, 6),
    (0b111110, 6, 7),
    (0b1111110, 7, 8),
    (0b11111110, 8, 9),
    (0b111111110, 9, 10),
    (0b111111111, 9, 11),
];

/// Table B-13, dct_dc_size_chrominance.
pub const DC_CHROMA: &[(u16, u8, u8)] = &[
    (0b00, 2, 0),
    (0b01, 2, 1),
    (0b10, 2, 2),
    (0b110, 3, 3),
    (0b1110, 4, 4),
    (0b11110, 5, 5),
    (0b111110, 6, 6),
    (0b1111110, 7, 7),
    (0b11111110, 8, 8),
    (0b111111110, 9, 9),
    (0b1111111110, 10, 10),
    (0b1111111111, 10, 11),
];

/// Run and level of each entry of [`DCT_ZERO`] and [`DCT_ONE`].
pub const RUN: [u8; 111] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3,
    3, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14,
    15, 15, 16, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
];
pub const LEVEL: [u8; 111] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15, 16, 17, 18, 1, 2, 3, 4, 5, 1, 2, 3, 4, 1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 1, 2, 1, 2,
    1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];

/// Table B-14, DCT coefficients table zero: the codes of [`RUN`]/[`LEVEL`]
/// in order, then the escape and the end of block. A sign bit follows each
/// run/level code. (The first coefficient of a non-intra block is `1s`
/// for run 0, level 1.)
pub const DCT_ZERO: [(u16, u8); 113] = [
    (0x3, 2),
    (0x4, 4),
    (0x5, 5),
    (0x6, 7),
    (0x26, 8),
    (0x21, 8),
    (0xa, 10),
    (0x1d, 12),
    (0x18, 12),
    (0x13, 12),
    (0x10, 12),
    (0x1a, 13),
    (0x19, 13),
    (0x18, 13),
    (0x17, 13),
    (0x1f, 14),
    (0x1e, 14),
    (0x1d, 14),
    (0x1c, 14),
    (0x1b, 14),
    (0x1a, 14),
    (0x19, 14),
    (0x18, 14),
    (0x17, 14),
    (0x16, 14),
    (0x15, 14),
    (0x14, 14),
    (0x13, 14),
    (0x12, 14),
    (0x11, 14),
    (0x10, 14),
    (0x18, 15),
    (0x17, 15),
    (0x16, 15),
    (0x15, 15),
    (0x14, 15),
    (0x13, 15),
    (0x12, 15),
    (0x11, 15),
    (0x10, 15),
    (0x3, 3),
    (0x6, 6),
    (0x25, 8),
    (0xc, 10),
    (0x1b, 12),
    (0x16, 13),
    (0x15, 13),
    (0x1f, 15),
    (0x1e, 15),
    (0x1d, 15),
    (0x1c, 15),
    (0x1b, 15),
    (0x1a, 15),
    (0x19, 15),
    (0x13, 16),
    (0x12, 16),
    (0x11, 16),
    (0x10, 16),
    (0x5, 4),
    (0x4, 7),
    (0xb, 10),
    (0x14, 12),
    (0x14, 13),
    (0x7, 5),
    (0x24, 8),
    (0x1c, 12),
    (0x13, 13),
    (0x6, 5),
    (0xf, 10),
    (0x12, 12),
    (0x7, 6),
    (0x9, 10),
    (0x12, 13),
    (0x5, 6),
    (0x1e, 12),
    (0x14, 16),
    (0x4, 6),
    (0x15, 12),
    (0x7, 7),
    (0x11, 12),
    (0x5, 7),
    (0x11, 13),
    (0x27, 8),
    (0x10, 13),
    (0x23, 8),
    (0x1a, 16),
    (0x22, 8),
    (0x19, 16),
    (0x20, 8),
    (0x18, 16),
    (0xe, 10),
    (0x17, 16),
    (0xd, 10),
    (0x16, 16),
    (0x8, 10),
    (0x15, 16),
    (0x1f, 12),
    (0x1a, 12),
    (0x19, 12),
    (0x17, 12),
    (0x16, 12),
    (0x1f, 13),
    (0x1e, 13),
    (0x1d, 13),
    (0x1c, 13),
    (0x1b, 13),
    (0x1f, 16),
    (0x1e, 16),
    (0x1d, 16),
    (0x1c, 16),
    (0x1b, 16),
    (0x1, 6),
    (0x2, 2),
];

/// Table B-15, DCT coefficients table one (intra blocks with
/// intra_vlc_format 1), laid out as [`DCT_ZERO`].
pub const DCT_ONE: [(u16, u8); 113] = [
    (0x02, 2),
    (0x06, 3),
    (0x07, 4),
    (0x1c, 5),
    (0x1d, 5),
    (0x05, 6),
    (0x04, 6),
    (0x7b, 7),
    (0x7c, 7),
    (0x23, 8),
    (0x22, 8),
    (0xfa, 8),
    (0xfb, 8),
    (0xfe, 8),
    (0xff, 8),
    (0x1f, 14),
    (0x1e, 14),
    (0x1d, 14),
    (0x1c, 14),
    (0x1b, 14),
    (0x1a, 14),
    (0x19, 14),
    (0x18, 14),
    (0x17, 14),
    (0x16, 14),
    (0x15, 14),
    (0x14, 14),
    (0x13, 14),
    (0x12, 14),
    (0x11, 14),
    (0x10, 14),
    (0x18, 15),
    (0x17, 15),
    (0x16, 15),
    (0x15, 15),
    (0x14, 15),
    (0x13, 15),
    (0x12, 15),
    (0x11, 15),
    (0x10, 15),
    (0x02, 3),
    (0x06, 5),
    (0x79, 7),
    (0x27, 8),
    (0x20, 8),
    (0x16, 13),
    (0x15, 13),
    (0x1f, 15),
    (0x1e, 15),
    (0x1d, 15),
    (0x1c, 15),
    (0x1b, 15),
    (0x1a, 15),
    (0x19, 15),
    (0x13, 16),
    (0x12, 16),
    (0x11, 16),
    (0x10, 16),
    (0x05, 5),
    (0x07, 7),
    (0xfc, 8),
    (0x0c, 10),
    (0x14, 13),
    (0x07, 5),
    (0x26, 8),
    (0x1c, 12),
    (0x13, 13),
    (0x06, 6),
    (0xfd, 8),
    (0x12, 12),
    (0x07, 6),
    (0x04, 9),
    (0x12, 13),
    (0x06, 7),
    (0x1e, 12),
    (0x14, 16),
    (0x04, 7),
    (0x15, 12),
    (0x05, 7),
    (0x11, 12),
    (0x78, 7),
    (0x11, 13),
    (0x7a, 7),
    (0x10, 13),
    (0x21, 8),
    (0x1a, 16),
    (0x25, 8),
    (0x19, 16),
    (0x24, 8),
    (0x18, 16),
    (0x05, 9),
    (0x17, 16),
    (0x07, 9),
    (0x16, 16),
    (0x0d, 10),
    (0x15, 16),
    (0x1f, 12),
    (0x1a, 12),
    (0x19, 12),
    (0x17, 12),
    (0x16, 12),
    (0x1f, 13),
    (0x1e, 13),
    (0x1d, 13),
    (0x1c, 13),
    (0x1b, 13),
    (0x1f, 16),
    (0x1e, 16),
    (0x1d, 16),
    (0x1c, 16),
    (0x1b, 16),
    (0x01, 6),
    (0x06, 4),
];

/// The index of the escape and the end of block in the DCT tables.
pub const DCT_ESCAPE: u16 = 111;
pub const DCT_EOB: u16 = 112;

/// The zigzag scan: coefficient `k` in scan order is at `ZIGZAG[k]`.
pub const ZIGZAG: [u8; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// The alternate scan.
pub const ALTERNATE: [u8; 64] = [
    0, 8, 16, 24, 1, 9, 2, 10, 17, 25, 32, 40, 48, 56, 57, 49, 41, 33, 26, 18, 3, 11, 4, 12, 19,
    27, 34, 42, 50, 58, 35, 43, 51, 59, 20, 28, 5, 13, 6, 14, 21, 29, 36, 44, 52, 60, 37, 45, 53,
    61, 22, 30, 7, 15, 23, 31, 38, 46, 54, 62, 39, 47, 55, 63,
];

/// The default intra quantiser matrix, in raster order.
pub const DEFAULT_INTRA: [u8; 64] = [
    8, 16, 19, 22, 26, 27, 29, 34, 16, 16, 22, 24, 27, 29, 34, 37, 19, 22, 26, 27, 29, 34, 34, 38,
    22, 22, 26, 27, 29, 34, 37, 40, 22, 26, 27, 29, 32, 35, 40, 48, 26, 27, 29, 32, 35, 40, 48, 58,
    26, 27, 29, 34, 38, 46, 56, 69, 27, 29, 35, 38, 46, 56, 69, 83,
];

/// quantiser_scale for q_scale_type 1 (table 7-6).
pub const NONLINEAR_SCALE: [u8; 32] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 16, 18, 20, 22, 24, 28, 32, 36, 40, 44, 48, 52, 56, 64,
    72, 80, 88, 96, 104, 112,
];

/// A lookup over a prefix-free code: every `bits`-bit value maps to its
/// code's value and length (0 for none).
pub struct Lookup {
    pub bits: u8,
    pub table: Vec<(u16, u8)>,
}

impl Lookup {
    pub fn new(codes: impl IntoIterator<Item = (u16, u8, u16)>) -> Self {
        let codes: Vec<_> = codes.into_iter().collect();
        let bits = codes.iter().map(|c| c.1).max().unwrap_or(1);
        let mut table = vec![(0, 0); 1 << bits];
        for (code, len, value) in codes {
            let shift = bits - len;
            let start = usize::from(code) << shift;
            for e in &mut table[start..start + (1 << shift)] {
                debug_assert_eq!(e.1, 0, "codes overlap");
                *e = (value, len);
            }
        }
        Lookup { bits, table }
    }

    pub fn small(codes: &[(u16, u8, u8)]) -> Self {
        Self::new(codes.iter().map(|&(c, l, v)| (c, l, u16::from(v))))
    }

    pub fn dct(codes: &[(u16, u8); 113]) -> Self {
        Self::new(
            codes
                .iter()
                .enumerate()
                .map(|(k, &(c, l))| (c, l, k as u16)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each table is a prefix code: no code starts another.
    #[test]
    fn the_codes_are_prefix_free() {
        let check = |codes: Vec<(u16, u8)>| {
            for (i, &(a, la)) in codes.iter().enumerate() {
                for &(b, lb) in &codes[i + 1..] {
                    let l = la.min(lb);
                    assert_ne!(a >> (la - l), b >> (lb - l), "{a:b}/{la} and {b:b}/{lb}");
                }
            }
        };
        for t in [
            MB_ADDRESS, MB_TYPE_I, MB_TYPE_P, MB_TYPE_B, CBP, MOTION, DC_LUMA, DC_CHROMA,
        ] {
            check(t.iter().map(|&(c, l, _)| (c, l)).collect());
        }
        check(DCT_ZERO.to_vec());
        check(DCT_ONE.to_vec());
    }

    #[test]
    fn the_scans_are_permutations() {
        for scan in [ZIGZAG, ALTERNATE] {
            let mut seen = [false; 64];
            for k in scan {
                seen[usize::from(k)] = true;
            }
            assert!(seen.iter().all(|&s| s));
        }
    }
}
