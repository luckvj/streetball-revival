//! The crowds: `/XF/<COURT>C.VIV` and the seating plan `<court>w.bin` in
//! `/XF/XF000003`. The game's loader is 0x30b9b8 and the models' class
//! 0x308830 (vtable 0x4991d0).
//!
//! A spectator is a body (`<court>B.e3b`, textures `<court>b.ssh`) and a
//! head of one of four kinds (`M`, `C`, `F`, `P` models with `m`, `c`, `f`,
//! `p` sheets), seated by `<court>w.bin` and dressed by `<court>t.bin`.
//!
//! ## `.e3b` models
//!
//! Little-endian. Header: `01 04 04 20`, u32 triangles, u32 vertices, u32,
//! u32 parts, u32 animations, u32, six f32 bounds, a 16-byte name (`spec`,
//! `head`). Then a 0x20-byte record per part: u32 offset of the part's VU
//! program (a DMA packet of `MPG` uploads), u32 offset of its VU data image
//! (a DMA tag whose u16 count + 1 quadwords follow 0x20 bytes later), u32
//! first input quadword (12), u32 input vertices, u32 first vertex of the
//! model's vertex array, ... Then a 0x40-byte record per animation: u32 rate
//! (60), u32 frames, u32 key offset, u32 frames before this one, u32 delta
//! offset, three f32 delta scales, a 32-byte name (`animation`).
//!
//! An animation's frame `f` puts vertex `v` at `key[v] + delta[f][v] *
//! scale`: `key` is one f32 xyz per vertex (the animation's lowest corner),
//! `delta` three **unsigned** bytes per vertex per frame (offsets from the
//! key, not from the frame before). The class builds a table of every
//! frame's (record, key, deltas) (`0x308830`); `FUN_00309ff0` decodes one
//! with `lbu` loops (`0x3242b0` and its siblings for 139, 61, 60, 49 and
//! 151 vertices, else the C loop over `byte *`). Read as signed, every
//! delta past 127 lands 256 steps the wrong way: the cheering animations,
//! which move furthest, came apart into long zigzags.
//!
//! The mesh is not stored as triangles. Each part's VU program is unrolled:
//! it loads every input vertex (`LQ` from quadword 12 up), transforms it,
//! and stores the screen position (`SQ`) into each output slot of a GS
//! triangle strip that uses it. The data image holds the strip: a GIF tag
//! (`nloop` vertices, registers UV and XYZ2) then per vertex a UV quadword
//! (integer texels × 16) and an XYZ2 template whose bit 15 of w is the ADC
//! (no-draw) flag that starts a new strip. [`decode`] runs the program as a
//! data flow (which input each stored register came from) to find each
//! output slot's vertex; every model on the disc comes out with exactly the
//! triangle count its header gives.

use std::io::{Read, Seek};

use crate::{Error, Result, big, bytes, iso::Iso};

/// One corner of a crowd triangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Corner {
    /// Index into the model's vertex array.
    pub vertex: u16,
    /// Texel coordinates (divide by the image size).
    pub texel: [f32; 2],
}

#[derive(Clone, Debug)]
pub struct Animation {
    /// 60 in every record; the game's playback does not read it (it shows
    /// a frame every four ticks, `street_render::crowd`).
    pub rate: u32,
    pub key: Vec<[f32; 3]>,
    /// `frames * vertices * 3` unsigned deltas.
    pub deltas: Vec<u8>,
    pub scale: [f32; 3],
    pub frames: usize,
}

impl Animation {
    /// Vertex positions at `frame` (wrapping); the key for a record
    /// without frames.
    pub fn pose(&self, frame: usize, out: &mut Vec<[f32; 3]>) {
        out.clear();
        let n = self.key.len();
        if self.frames == 0 {
            out.extend_from_slice(&self.key);
            return;
        }
        let f = frame % self.frames;
        for (v, k) in self.key.iter().enumerate() {
            let d = &self.deltas[(f * n + v) * 3..(f * n + v) * 3 + 3];
            out.push([
                k[0] + f32::from(d[0]) * self.scale[0],
                k[1] + f32::from(d[1]) * self.scale[1],
                k[2] + f32::from(d[2]) * self.scale[2],
            ]);
        }
    }
}

#[derive(Clone, Debug)]
pub struct Model {
    pub name: String,
    pub vertices: usize,
    /// The rest positions (from the parts' data images).
    pub rest: Vec<[f32; 3]>,
    pub triangles: Vec<[Corner; 3]>,
    /// Every animation record, in order (the game picks them by index).
    pub animations: Vec<Animation>,
}

fn u32_at(d: &[u8], at: usize) -> Result<usize> {
    Ok(bytes::u32_le(d, at)? as usize)
}

fn f32_at(d: &[u8], at: usize) -> Result<f32> {
    bytes::f32_le(d, at)
}

/// Gathers the microprogram a part uploads (its `MPG` VIF codes).
fn program(d: &[u8], tag: usize) -> Result<Vec<(u32, u32)>> {
    let qwc = bytes::u16_le(d, tag)? as usize;
    let end = (tag + 16 + qwc * 16).min(d.len());
    let mut pos = tag + 8;
    let mut code: Vec<(u32, u32)> = Vec::new();
    while pos + 4 <= end {
        let w = bytes::u32_le(d, pos)?;
        let (cmd, num, imm) = ((w >> 24) & 0x7f, (w >> 16) & 0xff, (w & 0xffff) as usize);
        pos += 4;
        match cmd {
            0x4a => {
                let n = if num == 0 { 256 } else { num as usize };
                if code.len() < imm + n {
                    code.resize(imm + n, (0, 0x2ff));
                }
                for i in 0..n {
                    code[imm + i] = (
                        bytes::u32_le(d, pos + 8 * i)?,
                        bytes::u32_le(d, pos + 8 * i + 4)?,
                    );
                }
                pos += 8 * n;
            }
            0x20 => pos += 4,
            0x30 | 0x31 => pos += 16,
            _ => {}
        }
    }
    Ok(code)
}

/// The data flow of an unrolled crowd program: for each quadword the
/// program stores, the input quadword its value was computed from.
/// VU1 interlocks on register hazards, so registers act in program order.
fn data_flow(code: &[(u32, u32)], inputs: std::ops::RangeInclusive<i32>) -> Vec<(i32, i32)> {
    let mut prov: [Option<i32>; 32] = [None; 32];
    let mut acc: Option<i32> = None;
    let mut stores = Vec::new();
    let vertex = |p: Option<i32>| p.filter(|a| inputs.contains(a));
    for &(lo, up) in code {
        let (ft, fs, fd) = (
            ((up >> 16) & 31) as usize,
            ((up >> 11) & 31) as usize,
            ((up >> 6) & 31) as usize,
        );
        let op = up & 0x3f;
        let either = |prov: &[Option<i32>; 32]| vertex(prov[fs]).or(vertex(prov[ft]));
        match op {
            0x00..=0x1b => {
                let v = either(&prov);
                prov[fd] = match op >> 2 {
                    2 | 3 => acc.or(v), // MADDbc, MSUBbc
                    _ => v,
                };
            }
            0x1c..=0x1f => prov[fd] = prov[fs],
            0x20..=0x27 => {
                prov[fd] = if matches!(op, 0x21 | 0x23 | 0x25 | 0x27) {
                    acc.or(vertex(prov[fs]))
                } else {
                    prov[fs]
                };
            }
            0x28..=0x2f => {
                let v = either(&prov);
                prov[fd] = if matches!(op, 0x29 | 0x2d | 0x2e) {
                    acc.or(v)
                } else {
                    v
                };
            }
            0x3c..=0x3f => {
                let (k, bc) = ((up >> 6) & 31, up & 3);
                match k {
                    0 | 1 | 6 => acc = either(&prov),     // ADDA, SUBA, MULA (bc)
                    2 | 3 => acc = acc.or(either(&prov)), // MADDA, MSUBA (bc)
                    4 | 5 => prov[ft] = prov[fs],         // ITOF, FTOI
                    _ => match (bc, k) {
                        (1, 7) => prov[ft] = prov[fs], // ABS
                        (0 | 2, 7..=9) => acc = vertex(prov[fs]),
                        (0, 10 | 11) | (2, 10 | 11) => acc = either(&prov),
                        (1 | 3, 8..=11) => acc = acc.or(either(&prov)),
                        _ => {}
                    },
                }
            }
            _ => {}
        }
        // The I bit makes the lower word a float constant.
        if up >> 31 & 1 == 1 {
            continue;
        }
        let imm = {
            let v = (lo & 0x7ff) as i32;
            if v & 0x400 != 0 { v - 0x800 } else { v }
        };
        match lo >> 25 {
            0x00 => prov[((lo >> 16) & 31) as usize] = Some(imm), // LQ
            0x01 => stores.push((imm, prov[((lo >> 11) & 31) as usize].unwrap_or(-1))), // SQ
            _ => {}
        }
    }
    stores
}

/// Decodes an `.e3b` crowd model.
pub fn decode(d: &[u8]) -> Result<Model> {
    if d.get(..4) != Some(&[1, 4, 4, 0x20]) {
        return Err(Error::Format("not a crowd model (.e3b)".into()));
    }
    let triangle_count = u32_at(d, 4)?;
    let vertices = u32_at(d, 8)?;
    let parts = u32_at(d, 0x10)?;
    let animation_count = u32_at(d, 0x14)?;
    let name = bytes::cstr(d, 0x34)?;
    let mut rest = vec![[0f32; 3]; vertices];
    let mut triangles = Vec::new();
    for p in 0..parts {
        let rec = 0x44 + 0x20 * p;
        let (tag, data, first, count, start) = (
            u32_at(d, rec)?,
            u32_at(d, rec + 4)?,
            u32_at(d, rec + 8)?,
            u32_at(d, rec + 12)?,
            u32_at(d, rec + 16)?,
        );
        let qwords = bytes::u16_le(d, data)? as usize + 1;
        let image = data + 0x20;
        let q = |i: usize| image + 16 * i;
        for k in 0..count {
            if let Some(slot) = rest.get_mut(start + k) {
                *slot = [
                    f32_at(d, q(first + k))?,
                    f32_at(d, q(first + k) + 4)?,
                    f32_at(d, q(first + k) + 8)?,
                ];
            }
        }
        let code = program(d, tag)?;
        let inputs = first as i32..=(first + count) as i32 - 1;
        let flow: std::collections::HashMap<i32, i32> = {
            let mut m = std::collections::HashMap::new();
            for (slot, src) in data_flow(&code, inputs) {
                m.entry(slot).or_insert(src);
            }
            m
        };
        // The strips: GIF tags with registers UV, XYZ2.
        let mut i = 0;
        while i < qwords {
            let lo = bytes::u32_le(d, q(i))? as u64 | (bytes::u32_le(d, q(i) + 4)? as u64) << 32;
            let regs = bytes::u32_le(d, q(i) + 8)?;
            let nloop = (lo & 0x7fff) as usize;
            let nreg = (lo >> 60) & 15;
            if nreg == 2 && regs == 0x43 && nloop > 0 && i + 2 * nloop < qwords {
                let mut strip: Vec<Option<(Corner, bool)>> = Vec::with_capacity(nloop);
                for k in 0..nloop {
                    let (uvq, xyzq) = (q(i + 1 + 2 * k), q(i + 2 + 2 * k));
                    let texel = [
                        bytes::u32_le(d, uvq)? as i32 as f32 / 16.0,
                        bytes::u32_le(d, uvq + 4)? as i32 as f32 / 16.0,
                    ];
                    let adc = bytes::u32_le(d, xyzq + 12)? & 0x8000 != 0;
                    let src = flow.get(&((i + 2 + 2 * k) as i32)).copied().unwrap_or(-1);
                    strip.push((src >= first as i32).then(|| {
                        let vertex = (start + (src as usize - first)) as u16;
                        (Corner { vertex, texel }, adc)
                    }));
                }
                for k in 2..strip.len() {
                    let (Some(a), Some(b), Some(c)) = (strip[k - 2], strip[k - 1], strip[k]) else {
                        continue;
                    };
                    if c.1 {
                        continue;
                    }
                    // Strips alternate winding.
                    triangles.push(if k % 2 == 0 {
                        [a.0, b.0, c.0]
                    } else {
                        [b.0, a.0, c.0]
                    });
                }
                i += 1 + 2 * nloop;
            } else {
                i += 1;
            }
        }
    }
    if triangles.len() != triangle_count {
        return Err(Error::Format(format!(
            "crowd model {name}: {} triangles decoded, header says {triangle_count}",
            triangles.len()
        )));
    }
    let mut animations = Vec::new();
    let base = 0x44 + 0x20 * parts;
    for a in 0..animation_count {
        let rec = base + 0x40 * a;
        let (rate, frames, key_at, delta_at) = (
            u32_at(d, rec)?,
            u32_at(d, rec + 4)?,
            u32_at(d, rec + 8)?,
            u32_at(d, rec + 16)?,
        );
        // Every record keeps its place: the game picks animations by
        // record index.
        if frames == 0 {
            animations.push(Animation {
                rate: rate as u32,
                key: rest.clone(),
                deltas: Vec::new(),
                scale: [0.0; 3],
                frames: 0,
            });
            continue;
        }
        if delta_at + frames * vertices * 3 > d.len() {
            return Err(Error::Format(format!(
                "crowd model {name}: animation {a}'s deltas run past the file"
            )));
        }
        let scale = [
            f32_at(d, rec + 20)?,
            f32_at(d, rec + 24)?,
            f32_at(d, rec + 28)?,
        ];
        let key = (0..vertices)
            .map(|v| {
                Ok([
                    f32_at(d, key_at + 12 * v)?,
                    f32_at(d, key_at + 12 * v + 4)?,
                    f32_at(d, key_at + 12 * v + 8)?,
                ])
            })
            .collect::<Result<Vec<_>>>()?;
        let deltas = d[delta_at..delta_at + frames * vertices * 3].to_vec();
        animations.push(Animation {
            rate: rate as u32,
            key,
            deltas,
            scale,
            frames,
        });
    }
    Ok(Model {
        name,
        vertices,
        rest,
        triangles,
        animations,
    })
}

/// One seat of the plan (`<court>w.bin`).
#[derive(Clone, Debug, PartialEq)]
pub struct Seat {
    pub name: String,
    /// Head kind: 0 `M`, 1 `C`, 2 `F`, 3 `P`: the plan's column 8. The
    /// loader counts seats per head kind by it, loads those heads and
    /// gives the seat that head (`0x30b9b8`, seat record +0x1c).
    pub head: usize,
    /// The seat's role in the crowd's animation graphs, from the name's
    /// letters 2..4 (`FUN_0030b7e0`: `ST` 0, `SD` 1, `C` 2, `W` 4 a
    /// walker; anything else `DAT_00403030` = 1).
    pub role: usize,
    pub rotation_degrees: [f32; 3],
    pub position: [f32; 3],
    /// Which of the crowd's animation groups the seat follows.
    pub group: usize,
    /// The plan's column 10 (`strtoul`, seat record +0x20) as its four
    /// bytes, most significant first. Not RGBA as written: see
    /// [`Seat::vertex_colour`].
    pub colour: [u8; 4],
    /// From `<court>t.bin`: body and head texture choices.
    pub textures: [usize; 2],
}

impl Seat {
    /// The vertex colour (R, G, B, A, PS2 scale: 0x80 = 1.0) the game
    /// draws both the seat's body and its head with. `FUN_0030e250` hands
    /// the plan's word to each model's vtable `+0x3c` (`0x30a450`), which
    /// halves every byte and swaps the middle two; the draw (`0x3090d0`,
    /// `0x309964`) unpacks the result most significant byte first into the
    /// instance's qword 10, which every part's VU program (bodies and
    /// heads alike) stores unchanged as the strips' RGBAQ: the GIF tag
    /// after the input vertices names RGBAQ and TEX0 (`ruckerB` part 0:
    /// tag at 98, `SQ vf06, 99`), and the texture modulates
    /// (`FUN_00368ed8` clears TFX, sets TCC). So `0xRRBBGGAA`
    /// as written, at half scale: `0xb65a9bff` is a khaki (0x5b, 0x4d,
    /// 0x2d), not a pink.
    pub fn vertex_colour(&self) -> [u8; 4] {
        let [b3, b2, b1, b0] = self.colour;
        [b3 >> 1, b1 >> 1, b2 >> 1, b0 >> 1]
    }
}

/// `FUN_0030b7e0`: a seat's role from its name.
pub fn role(name: &str) -> usize {
    let b = name.as_bytes();
    match (b.get(2), b.get(3)) {
        (Some(b'S'), Some(b'T')) => 0,
        (Some(b'S'), Some(b'D')) => 1,
        (Some(b'C'), _) => 2,
        (Some(b'W'), _) => 4,
        _ => 1,
    }
}

fn number(word: &str) -> f32 {
    word.trim_end_matches(['F', 'f']).parse().unwrap_or(0.0)
}

/// Parses the seating plan and its texture table.
pub fn seats(plan: &str, textures: &str) -> Vec<Seat> {
    let mut lines = plan.lines().map(str::trim).filter(|l| !l.is_empty());
    let count: usize = lines.next().and_then(|l| l.parse().ok()).unwrap_or(0);
    let _groups = lines.next();
    let mut picks = textures.lines().filter_map(|l| {
        let mut w = l.split_whitespace().filter_map(|x| x.parse::<usize>().ok());
        Some([w.next()?, w.next().unwrap_or(0)])
    });
    let mut out = Vec::with_capacity(count);
    for line in lines.take(count) {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.len() < 11 {
            continue;
        }
        let colour = u32::from_str_radix(w[10].trim_start_matches("0x"), 16).unwrap_or(0x808080ff);
        out.push(Seat {
            name: w[0].to_owned(),
            head: w[8].parse().unwrap_or(0),
            role: role(w[0]),
            rotation_degrees: [number(w[1]), number(w[2]), number(w[3])],
            position: [number(w[4]), number(w[5]), number(w[6])],
            group: w[7].parse().unwrap_or(0),
            colour: colour.to_be_bytes(),
            textures: picks.next().unwrap_or([0, 0]),
        });
    }
    out
}

/// A court's crowd as the game loads it.
pub struct Crowd {
    pub seats: Vec<Seat>,
    pub body: Model,
    /// Head models by kind (`M`, `C`, `F`, `P`), when the court has them.
    pub heads: [Option<Model>; 4],
    /// Texture sheets: body, then heads by kind.
    pub body_sheet: Vec<(String, crate::ssh::Texture)>,
    pub head_sheets: [Vec<(String, crate::ssh::Texture)>; 4],
}

const KINDS: [char; 4] = ['M', 'C', 'F', 'P'];

/// Reads the crowd of the court whose archive is `/XF/<id>.VIV`.
pub fn load<R: Read + Seek>(iso: &mut Iso<R>, id: &str) -> Result<Crowd> {
    let id = id.to_ascii_lowercase();
    let archive = format!("/XF/{}C.VIV", id.to_ascii_uppercase());
    let entries = big::read_index(iso, &archive)?;
    let get = |iso: &mut Iso<R>, name: &str| -> Result<Vec<u8>> {
        let e = entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| Error::NotFound(format!("{archive}: {name}")))?;
        big::read_entry(iso, &archive, e)
    };
    let shared = big::read_index(iso, "/XF/XF000003")?;
    let plan_entry = shared
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(&format!("{id}w.bin")))
        .ok_or_else(|| Error::NotFound(format!("{id}w.bin")))?
        .clone();
    let plan = big::read_entry(iso, "/XF/XF000003", &plan_entry)?;
    let picks = get(iso, &format!("{id}t.bin")).unwrap_or_default();
    let seats = seats(
        &String::from_utf8_lossy(&plan),
        &String::from_utf8_lossy(&picks),
    );
    let sheet = |iso: &mut Iso<R>, name: &str| -> Vec<(String, crate::ssh::Texture)> {
        get(iso, name)
            .ok()
            .and_then(|b| crate::ssh::parse(&b).ok())
            .map(|s| {
                s.textures
                    .into_iter()
                    .map(|t| (t.name.clone(), t))
                    .collect()
            })
            .unwrap_or_default()
    };
    let body = decode(&get(iso, &format!("{id}B.e3b"))?)?;
    let body_sheet = sheet(iso, &format!("{id}b.ssh"));
    let mut heads: [Option<Model>; 4] = Default::default();
    let mut head_sheets: [Vec<(String, crate::ssh::Texture)>; 4] = Default::default();
    for (k, kind) in KINDS.iter().enumerate() {
        if let Ok(bytes) = get(iso, &format!("{id}{kind}.e3b")) {
            heads[k] = Some(decode(&bytes)?);
            head_sheets[k] = sheet(iso, &format!("{id}{}.ssh", kind.to_ascii_lowercase()));
        }
    }
    Ok(Crowd {
        seats,
        body,
        heads,
        body_sheet,
        head_sheets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_seats_and_their_textures() {
        let plan = "2\r\n7\r\nA_ST231 0F 270F 0F 56.2021F 1.79492F -19.5017F 2 0 3 0x273527ff\r\n\
                    C_SD195 0F 90F 0F -56.2F 1.5F 10.7F 5 2 4 0x874c76ff\r\n";
        let s = seats(plan, "4 1\r\n7 3\r\n");
        assert_eq!(s.len(), 2);
        // The head is column 8, the role comes from the name.
        assert_eq!((s[0].head, s[0].role), (0, 0));
        assert_eq!((s[1].head, s[1].role), (2, 1));
        assert_eq!(s[0].rotation_degrees, [0.0, 270.0, 0.0]);
        assert_eq!(s[0].position, [56.2021, 1.79492, -19.5017]);
        assert_eq!((s[0].group, s[0].colour), (2, [0x27, 0x35, 0x27, 0xff]));
        assert_eq!(s[1].textures, [7, 3]);
    }

    /// `0x30a450` halves each byte and swaps the middle two; the draw
    /// sends them most significant first as R, G, B, A.
    #[test]
    fn the_seat_colour_is_rrbbggaa_at_half_scale() {
        let plan = "1\r\n1\r\nB_ST258 0F 270F 0F 65.2F 5.4F -9.5F 7 0 3 0xb65a9bff\r\n";
        let s = seats(plan, "9 1\r\n");
        assert_eq!(s[0].colour, [0xb6, 0x5a, 0x9b, 0xff]);
        assert_eq!(s[0].vertex_colour(), [0x5b, 0x4d, 0x2d, 0x7f]);
    }

    #[test]
    fn roles_follow_fun_0030b7e0() {
        assert_eq!(
            ["A_ST1", "B_SD2", "C_C3", "G_WK1", "B_LN3", "X"].map(role),
            [0, 1, 2, 4, 1, 1]
        );
    }

    /// Deltas are unsigned bytes (`0x3242b0`'s `lbu`): 200 is 200 steps up
    /// from the key, not 56 down.
    #[test]
    fn deltas_are_unsigned() {
        let a = Animation {
            rate: 60,
            key: vec![[1.0, 2.0, 3.0]],
            deltas: vec![0, 128, 200, 255, 1, 2],
            scale: [0.5, 0.25, 0.1],
            frames: 2,
        };
        let mut pose = Vec::new();
        a.pose(0, &mut pose);
        assert_eq!(pose, [[1.0, 2.0 + 32.0, 3.0 + 200.0 * 0.1]]);
        a.pose(1, &mut pose);
        assert_eq!(pose, [[1.0 + 127.5, 2.25, 3.0 + 2.0 * 0.1]]);
    }
}
