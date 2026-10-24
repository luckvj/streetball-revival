//! A small MPEG-2 video decoder (ISO/IEC 13818-2) for the game's movies:
//! main profile, 4:2:0, frame pictures with frame prediction (what every
//! movie on the disc uses). Field pictures and field prediction are
//! refused with an error rather than drawn wrong.
//!
//! [`Decoder::next_frame`] gives the pictures in display order.

use std::fmt;

use crate::tables::{self, Lookup};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MPEG-2: {}", self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

fn err<T>(what: impl Into<String>) -> Result<T> {
    Err(Error(what.into()))
}

/// Reads bits, most significant first.
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Bits { data, pos: 0 }
    }

    /// The next 32 bits without moving (zeros past the end).
    fn peek32(&self) -> u32 {
        let byte = self.pos >> 3;
        let mut v: u64 = 0;
        for k in 0..5 {
            v = v << 8 | u64::from(*self.data.get(byte + k).unwrap_or(&0));
        }
        (v << (self.pos & 7) >> 8) as u32
    }

    fn peek(&self, n: u8) -> u32 {
        if n == 0 { 0 } else { self.peek32() >> (32 - n) }
    }

    fn skip(&mut self, n: u8) {
        self.pos += usize::from(n);
    }

    fn read(&mut self, n: u8) -> u32 {
        let v = self.peek(n);
        self.skip(n);
        v
    }

    fn flag(&mut self) -> bool {
        self.read(1) == 1
    }

    fn left(&self) -> isize {
        (self.data.len() * 8) as isize - self.pos as isize
    }

    fn align(&mut self) {
        self.pos = (self.pos + 7) & !7;
    }

    /// Moves to the next start code (`00 00 01`) on a byte boundary and
    /// returns its value, past it.
    fn next_start_code(&mut self) -> Option<u8> {
        self.align();
        let mut i = self.pos >> 3;
        while i + 3 < self.data.len() {
            if self.data[i] == 0 && self.data[i + 1] == 0 && self.data[i + 2] == 1 {
                self.pos = (i + 4) * 8;
                return Some(self.data[i + 3]);
            }
            i += 1;
        }
        self.pos = self.data.len() * 8;
        None
    }

    fn decode(&mut self, l: &Lookup) -> Result<u16> {
        let (v, len) = l.table[self.peek(l.bits) as usize];
        if len == 0 {
            return err(format!("bad variable-length code at bit {}", self.pos));
        }
        self.skip(len);
        Ok(v)
    }
}

/// One decoded picture: 8-bit planes, chroma at half size.
#[derive(Clone)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub y: Vec<u8>,
    pub cb: Vec<u8>,
    pub cr: Vec<u8>,
}

impl Frame {
    fn new(width: usize, height: usize) -> Self {
        Frame {
            width,
            height,
            y: vec![0; width * height],
            cb: vec![128; width * height / 4],
            cr: vec![128; width * height / 4],
        }
    }

    /// RGBA (ITU-R BT.601, video levels).
    pub fn to_rgba(&self, out: &mut Vec<u8>) {
        out.clear();
        out.reserve(self.width * self.height * 4);
        let cw = self.width / 2;
        for row in 0..self.height {
            for col in 0..self.width {
                let y = (f32::from(self.y[row * self.width + col]) - 16.0) * 1.164;
                let c = (row / 2) * cw + col / 2;
                let (cb, cr) = (f32::from(self.cb[c]) - 128.0, f32::from(self.cr[c]) - 128.0);
                let px = |v: f32| v.round().clamp(0.0, 255.0) as u8;
                out.extend_from_slice(&[
                    px(y + 1.596 * cr),
                    px(y - 0.392 * cb - 0.813 * cr),
                    px(y + 2.017 * cb),
                    255,
                ]);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    I = 1,
    P = 2,
    B = 3,
}

struct Picture {
    kind: Kind,
    f_code: [[u8; 2]; 2],
    intra_dc_precision: u8,
    frame_pred_frame_dct: bool,
    concealment: bool,
    q_scale_type: bool,
    intra_vlc_format: bool,
    alternate_scan: bool,
}

struct Tables {
    mb_address: Lookup,
    mb_type: [Lookup; 3],
    cbp: Lookup,
    motion: Lookup,
    dc: [Lookup; 2],
    dct: [Lookup; 2],
}

impl Tables {
    fn new() -> Self {
        Tables {
            mb_address: Lookup::small(tables::MB_ADDRESS),
            mb_type: [
                Lookup::small(tables::MB_TYPE_I),
                Lookup::small(tables::MB_TYPE_P),
                Lookup::small(tables::MB_TYPE_B),
            ],
            cbp: Lookup::small(tables::CBP),
            motion: Lookup::small(tables::MOTION),
            dc: [
                Lookup::small(tables::DC_LUMA),
                Lookup::small(tables::DC_CHROMA),
            ],
            dct: [
                Lookup::dct(&tables::DCT_ZERO),
                Lookup::dct(&tables::DCT_ONE),
            ],
        }
    }
}

/// The state that runs through one slice.
struct Slice {
    qscale: i32,
    dc_pred: [i32; 3],
    /// PMV[r][s][t]: first/second vector, forward/backward, x/y.
    pmv: [[[i32; 2]; 2]; 2],
    /// The last macroblock's prediction directions (for skipped ones in B
    /// pictures).
    last: u8,
}

/// How many pictures a video elementary stream holds: its picture start
/// codes (`00 00 01 00`), each a frame shown (the movies have no repeated
/// fields). A player's clock ends the movie at this many frames, as the
/// original's does (its time against the frame count, `FUN_00346d98`),
/// whatever the decoder's pace.
pub fn pictures(es: &[u8]) -> usize {
    es.windows(4).filter(|w| *w == [0, 0, 1, 0]).count()
}

pub struct Decoder<'a> {
    bits: Bits<'a>,
    tables: Tables,
    pub width: usize,
    pub height: usize,
    /// Frames per second.
    pub rate: f32,
    intra_matrix: [u8; 64],
    non_intra_matrix: [u8; 64],
    /// The reference pictures: the older and the newer.
    refs: [Option<Frame>; 2],
    /// A decoded reference held back until the next one (display order).
    held: Option<Frame>,
    ended: bool,
}

impl<'a> Decoder<'a> {
    /// Starts on a video elementary stream; reads its sequence header.
    pub fn new(es: &'a [u8]) -> Result<Self> {
        let mut d = Decoder {
            bits: Bits::new(es),
            tables: Tables::new(),
            width: 0,
            height: 0,
            rate: 30.0,
            intra_matrix: tables::DEFAULT_INTRA,
            non_intra_matrix: [16; 64],
            refs: [None, None],
            held: None,
            ended: false,
        };
        match d.bits.next_start_code() {
            Some(0xb3) => d.sequence_header()?,
            _ => return err("no sequence header"),
        }
        Ok(d)
    }

    fn sequence_header(&mut self) -> Result<()> {
        let b = &mut self.bits;
        self.width = b.read(12) as usize;
        self.height = b.read(12) as usize;
        b.skip(4);
        self.rate = match b.read(4) {
            1 => 24000.0 / 1001.0,
            2 => 24.0,
            3 => 25.0,
            4 => 30000.0 / 1001.0,
            5 => 30.0,
            6 => 50.0,
            7 => 60000.0 / 1001.0,
            8 => 60.0,
            _ => 30.0,
        };
        b.skip(18 + 1 + 10 + 1);
        let read_matrix = |b: &mut Bits| {
            let mut m = [0u8; 64];
            for k in 0..64 {
                m[usize::from(tables::ZIGZAG[k])] = b.read(8) as u8;
            }
            m
        };
        self.intra_matrix = if b.flag() {
            read_matrix(b)
        } else {
            tables::DEFAULT_INTRA
        };
        self.non_intra_matrix = if b.flag() { read_matrix(b) } else { [16; 64] };
        if self.width == 0
            || self.height == 0
            || !self.width.is_multiple_of(16)
            || !self.height.is_multiple_of(16)
        {
            return err(format!("unsupported size {}x{}", self.width, self.height));
        }
        Ok(())
    }

    /// The next picture in display order, or `None` at the end.
    pub fn next_frame(&mut self) -> Result<Option<Frame>> {
        loop {
            if self.ended {
                return Ok(self.held.take());
            }
            let Some(code) = self.bits.next_start_code() else {
                self.ended = true;
                continue;
            };
            match code {
                0xb3 => self.sequence_header()?,
                0xb7 => self.ended = true,
                0x00 => {
                    let (frame, kind) = self.picture()?;
                    if kind == Kind::B {
                        return Ok(Some(frame));
                    }
                    // A reference: show the one before it now, keep this.
                    self.refs[0] = self.refs[1].take();
                    self.refs[1] = Some(frame.clone());
                    if let Some(prev) = self.held.replace(frame) {
                        return Ok(Some(prev));
                    }
                }
                _ => {}
            }
        }
    }

    fn picture(&mut self) -> Result<(Frame, Kind)> {
        let b = &mut self.bits;
        b.skip(10);
        let kind = match b.read(3) {
            1 => Kind::I,
            2 => Kind::P,
            3 => Kind::B,
            k => return err(format!("picture type {k}")),
        };
        b.skip(16);
        if kind != Kind::I {
            b.skip(4);
        }
        if kind == Kind::B {
            b.skip(4);
        }
        // extra_information_picture
        while b.flag() {
            b.skip(8);
        }
        let mut pic = Picture {
            kind,
            f_code: [[15; 2]; 2],
            intra_dc_precision: 0,
            frame_pred_frame_dct: true,
            concealment: false,
            q_scale_type: false,
            intra_vlc_format: false,
            alternate_scan: false,
        };
        let mut frame = Frame::new(self.width, self.height);
        let mut slices = 0;
        loop {
            // Peek at the next start code without losing it.
            let save = self.bits.pos;
            let Some(code) = self.bits.next_start_code() else {
                break;
            };
            match code {
                0xb5 => {
                    let id = self.bits.read(4);
                    match id {
                        8 => self.picture_coding_extension(&mut pic)?,
                        3 => self.quant_matrix_extension(),
                        _ => {}
                    }
                }
                0xb2 => {}
                0x01..=0xaf => {
                    self.slice(code, &pic, &mut frame)?;
                    slices += 1;
                }
                _ => {
                    self.bits.pos = save;
                    break;
                }
            }
        }
        if slices == 0 {
            return err("a picture without slices");
        }
        Ok((frame, kind))
    }

    fn picture_coding_extension(&mut self, pic: &mut Picture) -> Result<()> {
        let b = &mut self.bits;
        for r in 0..2 {
            for t in 0..2 {
                pic.f_code[r][t] = b.read(4) as u8;
            }
        }
        pic.intra_dc_precision = b.read(2) as u8;
        let structure = b.read(2);
        b.skip(1); // top_field_first
        pic.frame_pred_frame_dct = b.flag();
        pic.concealment = b.flag();
        pic.q_scale_type = b.flag();
        pic.intra_vlc_format = b.flag();
        pic.alternate_scan = b.flag();
        if structure != 3 {
            return err("field pictures are not supported");
        }
        Ok(())
    }

    fn quant_matrix_extension(&mut self) {
        let b = &mut self.bits;
        let read_matrix = |b: &mut Bits, m: &mut [u8; 64]| {
            for k in 0..64 {
                m[usize::from(tables::ZIGZAG[k])] = b.read(8) as u8;
            }
        };
        if b.flag() {
            read_matrix(b, &mut self.intra_matrix);
        }
        if b.flag() {
            read_matrix(b, &mut self.non_intra_matrix);
        }
        // Chroma matrices (4:2:2 and up) are not used by 4:2:0.
    }

    fn qscale(pic: &Picture, code: u32) -> i32 {
        if pic.q_scale_type {
            i32::from(tables::NONLINEAR_SCALE[code as usize])
        } else {
            code as i32 * 2
        }
    }

    fn slice(&mut self, code: u8, pic: &Picture, frame: &mut Frame) -> Result<()> {
        let mb_width = self.width / 16;
        let mb_count = mb_width * (self.height / 16);
        let dc_reset = 1 << (7 + pic.intra_dc_precision);
        let mut s = Slice {
            qscale: Self::qscale(pic, self.bits.read(5)),
            dc_pred: [dc_reset; 3],
            pmv: [[[0; 2]; 2]; 2],
            last: 0,
        };
        if self.bits.flag() {
            // intra_slice_flag, intra_slice, reserved bits
            self.bits.skip(8);
            while self.bits.flag() {
                self.bits.skip(8);
            }
        }
        let mut address: isize = (isize::from(code) - 1) * mb_width as isize - 1;
        let mut first = true;
        loop {
            // macroblock_address_increment, with escapes.
            let mut inc = 0isize;
            loop {
                let v = self.bits.decode(&self.tables.mb_address)?;
                if v == 0 {
                    inc += 33;
                } else {
                    inc += v as isize;
                    break;
                }
            }
            // Skipped macroblocks (not at a slice's start).
            if !first {
                for k in 1..inc {
                    let a = (address + k) as usize;
                    if a >= mb_count {
                        return err("skipped macroblock past the picture");
                    }
                    self.skipped(pic, &mut s, frame, a)?;
                }
            }
            address += inc;
            first = false;
            let a = address as usize;
            if a >= mb_count {
                return err(format!("macroblock {a} past the picture"));
            }
            self.macroblock(pic, &mut s, frame, a)?;
            // The slice ends where the next start code begins (23 zero bits).
            if self.bits.left() < 24 || self.bits.peek(23) == 0 {
                break;
            }
        }
        Ok(())
    }

    fn skipped(&mut self, pic: &Picture, s: &mut Slice, frame: &mut Frame, a: usize) -> Result<()> {
        let dc_reset = 1 << (7 + pic.intra_dc_precision);
        s.dc_pred = [dc_reset; 3];
        match pic.kind {
            Kind::I => err("a skipped macroblock in an I picture"),
            Kind::P => {
                s.pmv = [[[0; 2]; 2]; 2];
                self.predict(pic.kind, frame, a, tables::FORWARD, s)
            }
            Kind::B => {
                let last = s.last;
                self.predict(pic.kind, frame, a, last, s)
            }
        }
    }

    fn macroblock(
        &mut self,
        pic: &Picture,
        s: &mut Slice,
        frame: &mut Frame,
        a: usize,
    ) -> Result<()> {
        let mb_type = self
            .bits
            .decode(&self.tables.mb_type[pic.kind as usize - 1])? as u8;
        let intra = mb_type & tables::INTRA != 0;
        let (fwd, bwd) = (
            mb_type & tables::FORWARD != 0,
            mb_type & tables::BACKWARD != 0,
        );
        if !pic.frame_pred_frame_dct && (fwd || bwd) {
            let motion_type = self.bits.read(2);
            if motion_type != 2 {
                return err(format!(
                    "frame_motion_type {motion_type} (field or dual prime)"
                ));
            }
        }
        let field_dct = !pic.frame_pred_frame_dct
            && (intra || mb_type & tables::PATTERN != 0)
            && self.bits.flag();
        if mb_type & tables::QUANT != 0 {
            s.qscale = Self::qscale(pic, self.bits.read(5));
        }
        let dc_reset = 1 << (7 + pic.intra_dc_precision);
        if fwd || (intra && pic.concealment) {
            self.motion_vector(pic, s, 0)?;
        }
        if bwd {
            self.motion_vector(pic, s, 1)?;
        }
        if intra && pic.concealment {
            self.bits.skip(1);
        }
        let cbp = if intra {
            63
        } else if mb_type & tables::PATTERN != 0 {
            self.bits.decode(&self.tables.cbp)? as u8
        } else {
            0
        };

        if intra {
            if !pic.concealment {
                s.pmv = [[[0; 2]; 2]; 2];
            }
        } else {
            s.dc_pred = [dc_reset; 3];
            if pic.kind == Kind::P && !fwd {
                // No motion compensation: a zero forward vector.
                s.pmv = [[[0; 2]; 2]; 2];
            }
            let dirs = if pic.kind == Kind::P {
                tables::FORWARD
            } else {
                mb_type & (tables::FORWARD | tables::BACKWARD)
            };
            self.predict(pic.kind, frame, a, dirs, s)?;
            s.last = dirs;
        }

        let mb_width = self.width / 16;
        let (mx, my) = (a % mb_width, a / mb_width);
        let mut block = [0i32; 64];
        for k in 0..6 {
            if cbp & (32 >> k) == 0 {
                continue;
            }
            block.fill(0);
            if intra {
                self.intra_block(pic, s, k, &mut block)?;
            } else {
                self.non_intra_block(pic, s, &mut block)?;
            }
            idct(&mut block);
            // Where the block goes.
            let (plane, stride, x, y, step) = match k {
                0..=3 => {
                    let (bx, by) = (k & 1, k >> 1);
                    let (y, step) = if field_dct {
                        (my * 16 + by, 2)
                    } else {
                        (my * 16 + by * 8, 1)
                    };
                    (&mut frame.y, self.width, mx * 16 + bx * 8, y, step)
                }
                4 => (&mut frame.cb, self.width / 2, mx * 8, my * 8, 1),
                _ => (&mut frame.cr, self.width / 2, mx * 8, my * 8, 1),
            };
            for r in 0..8 {
                let row = &mut plane[(y + r * step) * stride + x..][..8];
                for c in 0..8 {
                    let v = block[r * 8 + c];
                    row[c] = if intra {
                        v.clamp(0, 255) as u8
                    } else {
                        (i32::from(row[c]) + v).clamp(0, 255) as u8
                    };
                }
            }
        }
        Ok(())
    }

    fn motion_vector(&mut self, pic: &Picture, s: &mut Slice, dir: usize) -> Result<()> {
        for t in 0..2 {
            let f_code = pic.f_code[dir][t];
            if f_code == 0 || f_code > 9 {
                return err(format!("f_code {f_code}"));
            }
            let r_size = u32::from(f_code - 1);
            let code = i32::from(self.bits.decode(&self.tables.motion)?);
            let delta = if code == 0 {
                0
            } else {
                let negative = self.bits.flag();
                let residual = if r_size > 0 {
                    self.bits.read(r_size as u8) as i32
                } else {
                    0
                };
                let mag = ((code - 1) << r_size) + residual + 1;
                if negative { -mag } else { mag }
            };
            let f = 1 << r_size;
            let (low, high, range) = (-16 * f, 16 * f - 1, 32 * f);
            let mut v = s.pmv[0][dir][t] + delta;
            if v < low {
                v += range;
            } else if v > high {
                v -= range;
            }
            s.pmv[0][dir][t] = v;
            s.pmv[1][dir][t] = v;
        }
        Ok(())
    }

    /// The macroblock's prediction from the reference pictures.
    fn predict(&self, kind: Kind, frame: &mut Frame, a: usize, dirs: u8, s: &Slice) -> Result<()> {
        let mb_width = self.width / 16;
        let (mx, my) = (a % mb_width, a / mb_width);
        // P pictures predict from the newest reference; B pictures forward
        // from the older and backward from the newer.
        let Some(newer) = self.refs[1].as_ref() else {
            return err("a predicted picture without a reference");
        };
        let older = self.refs[0].as_ref().unwrap_or(newer);
        let (fwd_ref, bwd_ref) = if kind == Kind::B {
            (older, newer)
        } else {
            (newer, newer)
        };
        let mut luma = [[0u8; 256]; 2];
        let mut cb = [[0u8; 64]; 2];
        let mut cr = [[0u8; 64]; 2];
        let mut n = 0;
        for (dir, reference) in [(0usize, fwd_ref), (1, bwd_ref)] {
            let flag = if dir == 0 {
                tables::FORWARD
            } else {
                tables::BACKWARD
            };
            if dirs & flag == 0 {
                continue;
            }
            let (vx, vy) = (s.pmv[0][dir][0], s.pmv[0][dir][1]);
            fetch(
                &reference.y,
                self.width,
                self.height,
                mx * 16,
                my * 16,
                vx,
                vy,
                16,
                &mut luma[n],
            );
            let (cx, cy) = (vx / 2, vy / 2);
            let (cw, ch) = (self.width / 2, self.height / 2);
            fetch(&reference.cb, cw, ch, mx * 8, my * 8, cx, cy, 8, &mut cb[n]);
            fetch(&reference.cr, cw, ch, mx * 8, my * 8, cx, cy, 8, &mut cr[n]);
            n += 1;
        }
        if n == 0 {
            return err("a predicted macroblock without a direction");
        }
        let avg = |a: &[u8], b: &[u8], out: &mut [u8]| {
            for i in 0..out.len() {
                out[i] = ((u16::from(a[i]) + u16::from(b[i]) + 1) >> 1) as u8;
            }
        };
        let put = |plane: &mut [u8], stride: usize, x: usize, y: usize, size: usize, src: &[u8]| {
            for r in 0..size {
                plane[(y + r) * stride + x..][..size].copy_from_slice(&src[r * size..][..size]);
            }
        };
        let (mut l, mut b, mut r) = ([0u8; 256], [0u8; 64], [0u8; 64]);
        if n == 2 {
            avg(&luma[0], &luma[1], &mut l);
            avg(&cb[0], &cb[1], &mut b);
            avg(&cr[0], &cr[1], &mut r);
        } else {
            (l, b, r) = (luma[0], cb[0], cr[0]);
        }
        put(&mut frame.y, self.width, mx * 16, my * 16, 16, &l);
        put(&mut frame.cb, self.width / 2, mx * 8, my * 8, 8, &b);
        put(&mut frame.cr, self.width / 2, mx * 8, my * 8, 8, &r);
        Ok(())
    }

    fn intra_block(
        &mut self,
        pic: &Picture,
        s: &mut Slice,
        k: usize,
        block: &mut [i32; 64],
    ) -> Result<()> {
        let comp = if k < 4 { 0 } else { k - 3 };
        let size = self.bits.decode(&self.tables.dc[usize::from(comp > 0)])? as u8;
        let diff = if size == 0 {
            0
        } else {
            let v = self.bits.read(size) as i32;
            if v < 1 << (size - 1) {
                v + 1 - (1 << size)
            } else {
                v
            }
        };
        s.dc_pred[comp] += diff;
        let mult = 8 >> pic.intra_dc_precision;
        block[0] = s.dc_pred[comp] * mult;
        let scan = if pic.alternate_scan {
            &tables::ALTERNATE
        } else {
            &tables::ZIGZAG
        };
        let table = &self.tables.dct[usize::from(pic.intra_vlc_format)];
        let mut i = 0usize;
        while let Some((run, level)) = coefficient(&mut self.bits, table, false)? {
            i += run + 1;
            if i > 63 {
                return err("too many coefficients");
            }
            let at = usize::from(scan[i]);
            let w = i32::from(self.intra_matrix[at]);
            block[at] = (level * w * s.qscale / 16).clamp(-2048, 2047);
        }
        mismatch(block);
        Ok(())
    }

    fn non_intra_block(&mut self, pic: &Picture, s: &Slice, block: &mut [i32; 64]) -> Result<()> {
        let scan = if pic.alternate_scan {
            &tables::ALTERNATE
        } else {
            &tables::ZIGZAG
        };
        let mut i: isize = -1;
        let mut first = true;
        loop {
            let c = if first && self.bits.peek(1) == 1 {
                // `1s`: run 0, level 1, as the first coefficient.
                self.bits.skip(1);
                Some((0, if self.bits.flag() { -1 } else { 1 }))
            } else {
                coefficient(&mut self.bits, &self.tables.dct[0], first)?
            };
            first = false;
            let Some((run, level)) = c else { break };
            i += run as isize + 1;
            if i > 63 {
                return err("too many coefficients");
            }
            let at = usize::from(scan[i as usize]);
            let w = i32::from(self.non_intra_matrix[at]);
            let v = (2 * level + level.signum()) * w * s.qscale / 32;
            block[at] = v.clamp(-2048, 2047);
        }
        mismatch(block);
        Ok(())
    }
}

/// One run/level pair, or `None` at the end of the block.
fn coefficient(bits: &mut Bits, table: &Lookup, first: bool) -> Result<Option<(usize, i32)>> {
    match bits.decode(table)? {
        tables::DCT_EOB if !first => Ok(None),
        tables::DCT_EOB => err("end of block before any coefficient"),
        tables::DCT_ESCAPE => {
            let run = bits.read(6) as usize;
            let level = bits.read(12) as i32;
            let level = if level >= 2048 { level - 4096 } else { level };
            if level == 0 || level == -2048 {
                return err("forbidden escape level");
            }
            Ok(Some((run, level)))
        }
        k => {
            let (run, level) = (
                tables::RUN[usize::from(k)],
                i32::from(tables::LEVEL[usize::from(k)]),
            );
            let level = if bits.flag() { -level } else { level };
            Ok(Some((usize::from(run), level)))
        }
    }
}

/// Mismatch control: an even sum toggles the last coefficient's low bit.
fn mismatch(block: &mut [i32; 64]) {
    let sum: i32 = block.iter().sum();
    if sum & 1 == 0 {
        block[63] ^= 1;
    }
}

/// A `size`x`size` block of `plane` at (`x`, `y`) moved by the half-pel
/// vector (`vx`, `vy`), interpolated as the standard says.
#[allow(clippy::too_many_arguments)]
fn fetch(
    plane: &[u8],
    w: usize,
    h: usize,
    x: usize,
    y: usize,
    vx: i32,
    vy: i32,
    size: usize,
    out: &mut [u8],
) {
    let (ix, iy) = (x as i32 + (vx >> 1), y as i32 + (vy >> 1));
    let (hx, hy) = (vx & 1 != 0, vy & 1 != 0);
    let at = |px: i32, py: i32| {
        u16::from(
            plane[(py.clamp(0, h as i32 - 1) as usize) * w + px.clamp(0, w as i32 - 1) as usize],
        )
    };
    for r in 0..size as i32 {
        for c in 0..size as i32 {
            let (px, py) = (ix + c, iy + r);
            let v = match (hx, hy) {
                (false, false) => at(px, py),
                (true, false) => (at(px, py) + at(px + 1, py) + 1) >> 1,
                (false, true) => (at(px, py) + at(px, py + 1) + 1) >> 1,
                (true, true) => {
                    (at(px, py) + at(px + 1, py) + at(px, py + 1) + at(px + 1, py + 1) + 2) >> 2
                }
            };
            out[(r * size as i32 + c) as usize] = v as u8;
        }
    }
}

/// The 8x8 inverse DCT, separable, in floating point, rounded.
fn idct(block: &mut [i32; 64]) {
    use std::sync::OnceLock;
    static COS: OnceLock<[[f32; 8]; 8]> = OnceLock::new();
    let cos = COS.get_or_init(|| {
        let mut t = [[0f32; 8]; 8];
        for (x, row) in t.iter_mut().enumerate() {
            for (u, v) in row.iter_mut().enumerate() {
                let c = if u == 0 {
                    std::f32::consts::FRAC_1_SQRT_2
                } else {
                    1.0
                };
                *v = c * ((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI / 16.0).cos() / 2.0;
            }
        }
        t
    });
    if block.iter().all(|&v| v == 0) {
        return;
    }
    let mut tmp = [0f32; 64];
    // Rows: tmp[y][x] = sum_u cos[x][u] F[y][u]
    for y in 0..8 {
        for x in 0..8 {
            let mut s = 0.0;
            for u in 0..8 {
                s += cos[x][u] * block[y * 8 + u] as f32;
            }
            tmp[y * 8 + x] = s;
        }
    }
    for x in 0..8 {
        for y in 0..8 {
            let mut s = 0.0;
            for v in 0..8 {
                s += cos[y][v] * tmp[v * 8 + x];
            }
            block[y * 8 + x] = (s.round() as i32).clamp(-256, 255);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dc_only_block_is_flat() {
        let mut b = [0i32; 64];
        b[0] = 1024; // 128 x 8: mid-grey
        idct(&mut b);
        assert!(b.iter().all(|&v| v == 128), "{b:?}");
    }

    #[test]
    fn bits_read_across_bytes() {
        let data = [0b1010_1100, 0b0101_0011, 0xff];
        let mut b = Bits::new(&data);
        assert_eq!(b.read(3), 0b101);
        assert_eq!(b.read(7), 0b0110001);
        assert_eq!(b.read(6), 0b010011);
        assert_eq!(b.peek(8), 0xff);
    }

    #[test]
    fn start_codes_are_found_on_bytes() {
        let data = [0xff, 0, 0, 1, 0xb3, 1, 2, 0, 0, 1, 0];
        let mut b = Bits::new(&data);
        assert_eq!(b.next_start_code(), Some(0xb3));
        assert_eq!(b.read(8), 1);
        assert_eq!(b.next_start_code(), Some(0));
        assert_eq!(b.next_start_code(), None);
    }
}
