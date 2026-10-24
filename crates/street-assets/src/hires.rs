//! Dolphin custom-texture packs made for the GameCube Vol. 2 (`GNZE69`),
//! such as the "Legendary Edition" HD faces, jerseys, shoes, logos and
//! loading screens, shown on the PS2 data street-rs runs on.
//!
//! Dolphin names a replacement after the GameCube texture it replaces
//! ([`Key`]): its size, an XXH64 of its encoded bytes, a second XXH64 of the
//! palette range it uses (paletted formats only) and its GX format. The
//! GameCube disc keeps the PS2's archive layout (same files, same entry and
//! image order), so each GameCube image pairs with the PS2 image at the same
//! place; [`build_table`] does that pairing and checks it by comparing the
//! two pictures. The table shipped in `data/hires_gnze69.tsv` (hashes and
//! indices only, made by the `hires_map` example) lets a pack be used
//! without the GameCube disc.
//!
//! At run time a [`Pack`] finds the replacement for a decoded PS2 image by
//! the XXH64 of its RGBA pixels, so callers need not know where an image
//! came from. Replacements are larger than originals; texture
//! coordinates are normalised, so they need no change. The loading screens
//! are cut differently on the two consoles (three strips making 320x256 on
//! the GameCube, ten 128x256 tiles making 640x512 on the PS2) and come back
//! whole from [`Pack::picture`].

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::{Read, Seek},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use twox_hash::XxHash64;

use crate::{Error, Result, bytes, gcdisc::GcDisc, gsh, iso::Iso, ssh};

/// The table of the Legendary Edition pack's images, made by the
/// `hires_map` example from the user's two discs.
pub const GNZE69_TABLE: &str = include_str!("../data/hires_gnze69.tsv");

/// The loading screens' archive. Its sheets are cut differently on the two
/// consoles (see the module docs).
const PICTURES: &str = "/fe/xafeload.bin";

/// Thumbnails of a paired GameCube and PS2 image must differ by less than
/// this (mean absolute difference of 16x16 RGBA thumbnails, 0..1). The
/// Legendary Edition's pairs score at most 0.004; the closest wrong pair
/// seen (a different picture at the same place) scores 0.064.
const MAX_SCORE: f32 = 0.05;

/// The GameCube (GX) texture formats, by the number Dolphin writes in a
/// custom texture's name. An SHPG pixel block's type is the format + 0x10.
pub mod gx {
    pub const I4: u8 = 0;
    pub const I8: u8 = 1;
    pub const IA4: u8 = 2;
    pub const IA8: u8 = 3;
    pub const RGB565: u8 = 4;
    pub const RGB5A3: u8 = 5;
    pub const RGBA8: u8 = 6;
    pub const C4: u8 = 8;
    pub const C8: u8 = 9;
    pub const C14X2: u8 = 10;
    pub const CMPR: u8 = 14;
}

/// Bytes of a `width` × `height` texture in GX format `format` (whole tiles),
/// as Dolphin's `TexDecoder_GetTextureSizeInBytes`.
pub fn gx_size(width: u32, height: u32, format: u8) -> Option<usize> {
    // (tile width, tile height, bytes per tile)
    let (tw, th, tb) = match format {
        gx::I4 | gx::C4 | gx::CMPR => (8, 8, 32),
        gx::I8 | gx::IA4 | gx::C8 => (8, 4, 32),
        gx::IA8 | gx::RGB565 | gx::RGB5A3 | gx::C14X2 => (4, 4, 32),
        gx::RGBA8 => (4, 4, 64),
        _ => return None,
    };
    Some(width.div_ceil(tw) as usize * height.div_ceil(th) as usize * tb)
}

/// The name Dolphin gives a custom texture, without the `tex1_` prefix and
/// the extension: `<w>x<h>_<texhash>[_<tluthash>]_<format>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key {
    pub width: u32,
    pub height: u32,
    pub format: u8,
    pub texture: u64,
    pub tlut: Option<u64>,
}

impl Key {
    /// Hashes a texture the way Dolphin names it (`TextureInfo::CalculateTextureName`):
    /// XXH64 (seed 0) of the base level's encoded bytes, and for C4/C8/C14X2
    /// XXH64 of the palette entries from the lowest index the texture uses
    /// to the highest (two bytes each, as stored). `None` when `data` or
    /// `palette` is too short or the format is not a GX texture format.
    pub fn of(width: u32, height: u32, format: u8, data: &[u8], palette: &[u8]) -> Option<Self> {
        let size = gx_size(width, height, format)?;
        let data = data.get(..size)?;
        let tlut = match format {
            gx::C4 | gx::C8 | gx::C14X2 => {
                let (mut lo, mut hi) = (u32::MAX, 0u32);
                let mut see = |i: u32| {
                    lo = lo.min(i);
                    hi = hi.max(i);
                };
                match format {
                    gx::C4 => data.iter().for_each(|&b| {
                        see(b as u32 & 15);
                        see(b as u32 >> 4)
                    }),
                    gx::C8 => data.iter().for_each(|&b| see(b as u32)),
                    _ => data
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .for_each(|p| see(u16::from_be_bytes(*p) as u32 & 0x3fff)),
                }
                let range = palette.get(2 * lo as usize..2 * (hi as usize + 1))?;
                Some(XxHash64::oneshot(0, range))
            }
            _ => None,
        };
        Some(Self {
            width,
            height,
            format,
            texture: XxHash64::oneshot(0, data),
            tlut,
        })
    }

    /// Reads a pack file's name: `tex1_256x128_1e7df664c2a5adba_14.png` or
    /// `tex1_128x128_06cabd78676a49c6_64b17738ed8469f9_9.png`. Names with a
    /// mipmap mark (`_m`), a wildcard (`$`) or a mip level (`_mip1`) are not
    /// keys (this pack has none).
    pub fn parse(name: &str) -> Option<Self> {
        let stem = name.strip_prefix("tex1_")?;
        let stem = stem.rsplit_once('.').map_or(stem, |(s, _)| s);
        let parts: Vec<&str> = stem.split('_').collect();
        let (size, texture, tlut, format) = match parts.as_slice() {
            [size, tex, format] => (size, tex, None, format),
            [size, tex, tlut, format] => (size, tex, Some(tlut), format),
            _ => return None,
        };
        let (w, h) = size.split_once('x')?;
        let hex = |s: &str| (s.len() == 16).then(|| u64::from_str_radix(s, 16).ok())?;
        Some(Self {
            width: w.parse().ok()?,
            height: h.parse().ok()?,
            format: format.parse().ok()?,
            texture: hex(texture)?,
            tlut: match tlut {
                Some(t) => Some(hex(t)?),
                None => None,
            },
        })
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}_{:016x}", self.width, self.height, self.texture)?;
        if let Some(t) = self.tlut {
            write!(f, "_{t:016x}")?;
        }
        write!(f, "_{}", self.format)
    }
}

/// One image of an SHPG sheet as the GameCube draws it.
#[derive(Clone, Debug)]
pub struct GcImage {
    /// Position in the sheet's directory.
    pub index: usize,
    /// Position among the images [`gsh::parse`] decodes, if it decodes this one.
    pub decoded: Option<usize>,
    pub name: String,
    /// The SHPG pixel block type (GX format + 0x10).
    pub kind: u8,
    pub key: Option<Key>,
}

/// Every image of an SHPG sheet with its Dolphin key (`None` for a block
/// type that is not a GX texture format).
pub fn sheet_keys(buf: &[u8]) -> Result<Vec<GcImage>> {
    if bytes::range(buf, 0, 4)? != b"SHPG" {
        return Err(Error::format("not an SHPG sheet"));
    }
    let count = bytes::u32_be(buf, 8)? as usize;
    if count > buf.len() / 8 {
        return Err(Error::format(format!("SHPG image count {count} too large")));
    }
    let mut out = Vec::with_capacity(count);
    let mut decoded = 0;
    for index in 0..count {
        let name: String = bytes::range(buf, 16 + index * 8, 4)?
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as char)
            .collect();
        let first = bytes::u32_be(buf, 20 + index * 8)? as usize;
        let mut blocks = Vec::new();
        let mut pos = first;
        loop {
            let head = bytes::range(buf, pos, 16)?;
            let next = u32::from_be_bytes([0, head[1], head[2], head[3]]) as usize;
            blocks.push((head[0], pos));
            if next == 0 || blocks.len() > 64 {
                break;
            }
            pos = pos
                .checked_add(next)
                .ok_or_else(|| Error::format("SHPG block chain overflows"))?;
        }
        let (kind, at) = blocks[0];
        let width = bytes::u16_be(buf, at + 4)? as u32;
        let height = bytes::u16_be(buf, at + 6)? as u32;
        let data = buf.get(at + 16..).unwrap_or_default();
        let palette = blocks
            .iter()
            .find(|(k, _)| *k & 0xf0 == 0x30)
            .and_then(|&(_, p)| buf.get(p + 16..))
            .unwrap_or_default();
        let key = kind
            .checked_sub(0x10)
            .filter(|f| *f <= gx::CMPR)
            .and_then(|f| Key::of(width, height, f, data, palette));
        let decodes = gsh::DECODED.contains(&kind);
        out.push(GcImage {
            index,
            decoded: decodes.then_some(decoded),
            name,
            kind,
            key,
        });
        decoded += decodes as usize;
    }
    Ok(out)
}

/// The texture sheets in one disc file: the file itself when it is a sheet
/// (`SHPS`/`SHPG`, entry `None`), else each entry of a BIG or `c0 fb`
/// archive that is one (RefPack-packed entries are unpacked), by entry index.
pub fn sheets_in(data: &[u8]) -> Vec<SheetAt> {
    let is_sheet = |d: &[u8]| matches!(d.get(..4), Some(b"SHPS" | b"SHPG"));
    if is_sheet(data) {
        return vec![SheetAt {
            entry: None,
            name: String::new(),
            data: data.to_vec(),
        }];
    }
    let entries: Vec<(String, usize, usize)> =
        if let Ok(e) = crate::big::parse_table(data, data.len()) {
            e.into_iter()
                .map(|e| (e.name, e.offset as usize, e.size as usize))
                .collect()
        } else if let Ok(e) = crate::locale::c0fb(data) {
            e.into_iter()
                .map(|e| (e.name, e.offset as usize, e.size as usize))
                .collect()
        } else {
            return Vec::new();
        };
    entries
        .into_iter()
        .enumerate()
        .filter_map(|(i, (name, at, len))| {
            let raw = data.get(at..at + len)?;
            let d = crate::refpack::unpack(raw).ok()?;
            is_sheet(&d).then_some(SheetAt {
                entry: Some(i),
                name,
                data: d,
            })
        })
        .collect()
}

/// A texture sheet found by [`sheets_in`].
pub struct SheetAt {
    /// The archive entry (`None` when the file is the sheet).
    pub entry: Option<usize>,
    /// The entry's name, if the archive names its entries (`seattlo.gsh`).
    pub name: String,
    pub data: Vec<u8>,
}

/// An archive entry's name with the console's sheet extension dropped
/// (`seattlo.gsh` and `seattlo.ssh` are the same sheet; the archives sort
/// their entries by name, so the two discs' entry numbers can differ).
fn stem(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    match lower.rsplit_once('.') {
        Some((s, "gsh" | "ssh")) => s.to_owned(),
        _ => lower,
    }
}

/// The hash a [`Pack`] finds a PS2 image by: XXH64 (seed 0) of its decoded
/// RGBA pixels (as [`ssh::parse`] gives them).
pub fn rgba_hash(t: &ssh::Texture) -> u64 {
    XxHash64::oneshot(0, &t.rgba)
}

/// An image's place on a disc: a file (lower case), the archive entry
/// holding the sheet (`None` when the file is the sheet) and the image's
/// position among the sheet's decoded images.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Place {
    pub file: String,
    pub entry: Option<usize>,
    pub image: usize,
}

impl std::fmt::Display for Place {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.entry {
            Some(e) => write!(f, "{}\t{e}\t{}", self.file, self.image),
            None => write!(f, "{}\t-\t{}", self.file, self.image),
        }
    }
}

/// How a GameCube image stands in for PS2 data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    /// In place of one PS2 image.
    Image,
    /// As strip `image` (left to right) of a loading screen; the PS2 sheet
    /// at the row's file and entry is the whole picture.
    Strip,
}

/// One pairing of a GameCube image (by its Dolphin key) with PS2 data.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub key: Key,
    pub use_: Use,
    pub ps2: Place,
    /// The PS2 image's name in its sheet ("" for strips).
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// [`rgba_hash`] of the PS2 image (0 for strips).
    pub hash: u64,
    /// Where the GameCube image is.
    pub gc: Place,
    /// How alike the two pictures are (see [`MAX_SCORE`]); 0 for strips.
    pub score: f32,
}

const HEADER: &str =
    "# key\tuse\tps2 file\tentry\timage\tname\tsize\tps2 rgba xxh64\tgc file\tentry\timage\tscore";

/// Writes rows as the table format: tab-separated, one row a line, `#`
/// comments.
pub fn write_table(rows: &[Row]) -> String {
    let mut out = String::from(HEADER);
    out.push('\n');
    for r in rows {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}x{}\t{:016x}\t{}\t{:.3}\n",
            r.key,
            match r.use_ {
                Use::Image => "image",
                Use::Strip => "strip",
            },
            r.ps2,
            if r.name.is_empty() { "-" } else { &r.name },
            r.width,
            r.height,
            r.hash,
            r.gc,
            r.score
        ));
    }
    out
}

/// Reads a table written by [`write_table`].
pub fn parse_table(text: &str) -> Result<Vec<Row>> {
    let bad = |n: usize| Error::format(format!("hires table line {}", n + 1));
    let mut rows = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        let [key, use_, pf, pe, pi, name, size, hash, gf, ge, gi, score] = f[..] else {
            return Err(bad(n));
        };
        let place = |file: &str, entry: &str, image: &str| -> Option<Place> {
            Some(Place {
                file: file.to_owned(),
                entry: if entry == "-" {
                    None
                } else {
                    Some(entry.parse().ok()?)
                },
                image: image.parse().ok()?,
            })
        };
        let (w, h) = size.split_once('x').ok_or_else(|| bad(n))?;
        rows.push(Row {
            key: Key::parse(&format!("tex1_{key}")).ok_or_else(|| bad(n))?,
            use_: match use_ {
                "image" => Use::Image,
                "strip" => Use::Strip,
                _ => return Err(bad(n)),
            },
            ps2: place(pf, pe, pi).ok_or_else(|| bad(n))?,
            name: if name == "-" { "" } else { name }.to_owned(),
            width: w.parse().map_err(|_| bad(n))?,
            height: h.parse().map_err(|_| bad(n))?,
            hash: u64::from_str_radix(hash, 16).map_err(|_| bad(n))?,
            gc: place(gf, ge, gi).ok_or_else(|| bad(n))?,
            score: score.parse().map_err(|_| bad(n))?,
        });
    }
    Ok(rows)
}

/// A 16x16 RGBA thumbnail, each channel 0..1.
fn thumb(t: &ssh::Texture) -> Vec<f32> {
    const N: usize = 16;
    let (w, h) = (t.width as usize, t.height as usize);
    let mut sum = vec![0f32; N * N * 4];
    let mut count = vec![0f32; N * N];
    for y in 0..h {
        for x in 0..w {
            let c = (y * N / h) * N + x * N / w;
            let o = (y * w + x) * 4;
            for k in 0..4 {
                sum[c * 4 + k] += t.rgba[o + k] as f32;
            }
            count[c] += 1.0;
        }
    }
    for (c, n) in count.iter().enumerate() {
        for k in 0..4 {
            sum[c * 4 + k] /= n.max(1.0) * 255.0;
        }
    }
    sum
}

/// How unlike two pictures are: 0 the same, 1 opposite.
fn score(a: &ssh::Texture, b: &ssh::Texture) -> f32 {
    let (a, b) = (thumb(a), thumb(b));
    a.iter().zip(&b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

/// What [`build_table`] could not pair, for its report.
#[derive(Debug, Default)]
pub struct Unpaired {
    /// (key, GameCube place, why).
    pub images: Vec<(Key, Place, String)>,
}

/// Pairs GameCube images with PS2 data. `wanted` limits it to those keys
/// (a pack's), else every GameCube image is paired. A pair is kept when the
/// PS2 sheet at the same file and entry has an image at the same position,
/// of the same proportions, that looks like the GameCube one.
pub fn build_table<R: Read + Seek>(
    gc: &mut GcDisc,
    ps2: &mut Iso<R>,
    wanted: Option<&HashSet<Key>>,
) -> Result<(Vec<Row>, Unpaired)> {
    let mut rows = Vec::new();
    let mut unpaired = Unpaired::default();
    let ps2_files: HashMap<String, String> = ps2
        .files()
        .map(|(p, _)| (p.to_ascii_lowercase(), p.to_owned()))
        .collect();
    let files: Vec<String> = gc
        .files()
        .filter(|(p, s)| *s >= 16 && !p.ends_with(".nfx") && !p.starts_with("/audio"))
        .map(|(p, _)| p.to_owned())
        .collect();
    for file in files {
        let head = gc.read_range(&file, 0, 16)?;
        if !matches!(&head[..4], b"BIGF" | b"BIG4" | b"SHPG") && head[..2] != [0xc0, 0xfb] {
            continue;
        }
        let gsheets = sheets_in(&gc.read(&file)?);
        // (entry, sheet bytes, images that are wanted)
        let mut todo = Vec::new();
        for g in &gsheets {
            let images: Vec<GcImage> = sheet_keys(&g.data)
                .unwrap_or_default()
                .into_iter()
                .filter(|i| i.key.is_some_and(|k| wanted.is_none_or(|w| w.contains(&k))))
                .collect();
            if !images.is_empty() {
                todo.push((g.entry, stem(&g.name), &g.data, images));
            }
        }
        if todo.is_empty() {
            continue;
        }
        let gc_place = |entry: Option<usize>, image: usize| Place {
            file: file.clone(),
            entry,
            image,
        };
        let Some(ps2_path) = ps2_files.get(&file) else {
            for (entry, _, _, images) in &todo {
                for i in images {
                    unpaired.images.push((
                        i.key.unwrap(),
                        gc_place(*entry, i.index),
                        "no PS2 file".into(),
                    ));
                }
            }
            continue;
        };
        // PS2 sheets by entry, and by name where the archive names them.
        let mut psheets: BTreeMap<Option<usize>, ssh::Sheet> = BTreeMap::new();
        let mut named: HashMap<String, Option<usize>> = HashMap::new();
        for p in sheets_in(&ps2.read(ps2_path)?) {
            if let Ok(sheet) = ssh::parse(&p.data) {
                if !p.name.is_empty() {
                    named.insert(stem(&p.name), p.entry);
                }
                psheets.insert(p.entry, sheet);
            }
        }
        for (entry, name, d, images) in todo {
            let gsheet = gsh::parse(d)?;
            let ps2_entry = if name.is_empty() {
                entry
            } else {
                named.get(&name).copied().unwrap_or(None)
            };
            let ps2_place = |image: usize| Place {
                file: file.clone(),
                entry: ps2_entry,
                image,
            };
            for i in images {
                let key = i.key.unwrap();
                let mut miss = |why: String| {
                    unpaired.images.push((key, gc_place(entry, i.index), why));
                };
                let Some(g) = i.decoded.and_then(|k| gsheet.textures.get(k)) else {
                    miss("GameCube image not decoded".into());
                    continue;
                };
                let Some(psheet) = psheets
                    .get(&ps2_entry)
                    .filter(|_| ps2_entry.is_some() || entry.is_none())
                else {
                    miss("no PS2 sheet there".into());
                    continue;
                };
                if file == PICTURES {
                    rows.push(Row {
                        key,
                        use_: Use::Strip,
                        ps2: ps2_place(i.decoded.unwrap()),
                        name: String::new(),
                        width: g.width,
                        height: g.height,
                        hash: 0,
                        gc: gc_place(entry, i.index),
                        score: 0.0,
                    });
                    continue;
                }
                let k = i.decoded.unwrap();
                let Some(p) = psheet.textures.get(k) else {
                    miss("no PS2 image there".into());
                    continue;
                };
                if p.width * g.height != p.height * g.width {
                    miss(format!(
                        "PS2 image {} is {}x{}, GameCube {}x{}",
                        p.name, p.width, p.height, g.width, g.height
                    ));
                    continue;
                }
                let s = score(g, p);
                if s >= MAX_SCORE {
                    miss(format!("PS2 image {} looks different ({s:.3})", p.name));
                    continue;
                }
                rows.push(Row {
                    key,
                    use_: Use::Image,
                    ps2: ps2_place(k),
                    name: p.name.clone(),
                    width: p.width,
                    height: p.height,
                    hash: rgba_hash(p),
                    gc: gc_place(entry, i.index),
                    score: s,
                });
            }
        }
    }
    Ok((rows, unpaired))
}

/// Reads a PNG as straight RGBA8.
pub fn read_png(path: &Path) -> Result<ssh::Texture> {
    let fail = |e: &dyn std::fmt::Display| Error::format(format!("{}: {e}", path.display()));
    let mut decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| fail(&e))?;
    let mut buf = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or_else(|| fail(&"too large"))?
    ];
    let info = reader.next_frame(&mut buf).map_err(|e| fail(&e))?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err(fail(&"palette not expanded")),
    };
    Ok(ssh::Texture {
        name: String::new(),
        width: info.width,
        height: info.height,
        rgba,
        indices: None,
    })
}

/// A texture pack folder, ready to stand in for PS2 images.
pub struct Pack {
    /// Every PNG of the pack whose name is a [`Key`] (searched recursively).
    files: HashMap<Key, PathBuf>,
    /// (width, height, [`rgba_hash`]) of a PS2 image -> its replacement.
    images: HashMap<(u32, u32, u64), Key>,
    /// (PS2 file, entry) of a loading screen -> its strips, left to right.
    pictures: HashMap<(String, Option<usize>), Vec<Key>>,
    decoded: Mutex<HashMap<Key, Option<Arc<ssh::Texture>>>>,
}

impl Pack {
    /// Opens a pack folder (e.g. `Load/Textures/GNZE69` or the Legendary
    /// Edition's `User/Textures/GNZE69`) with the built-in table.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        Self::with_table(dir, &parse_table(GNZE69_TABLE)?)
    }

    /// Opens a pack folder with a table of one's own (e.g. one
    /// [`build_table`] made from the discs for another pack).
    pub fn with_table(dir: impl AsRef<Path>, rows: &[Row]) -> Result<Self> {
        let mut files = HashMap::new();
        walk(dir.as_ref(), &mut files)?;
        let mut images = HashMap::new();
        let mut strips: BTreeMap<(String, Option<usize>), BTreeMap<usize, Key>> = BTreeMap::new();
        for r in rows.iter().filter(|r| files.contains_key(&r.key)) {
            match r.use_ {
                Use::Image => {
                    images.entry((r.width, r.height, r.hash)).or_insert(r.key);
                }
                Use::Strip => {
                    strips
                        .entry((r.ps2.file.clone(), r.ps2.entry))
                        .or_default()
                        .insert(r.ps2.image, r.key);
                }
            }
        }
        // A picture needs every strip.
        let mut pictures = HashMap::new();
        for (at, s) in strips {
            if s.keys().copied().eq(0..s.len()) {
                pictures.insert(at, s.into_values().collect());
            }
        }
        Ok(Self {
            files,
            images,
            pictures,
            decoded: Mutex::new(HashMap::new()),
        })
    }

    /// Opens a pack folder, pairing its images from the two discs instead of
    /// the built-in table (for packs other than the Legendary Edition's;
    /// it reads the archives that hold sheets, about 15 seconds).
    pub fn from_discs<R: Read + Seek>(
        dir: impl AsRef<Path>,
        gc: &mut GcDisc,
        ps2: &mut Iso<R>,
    ) -> Result<Self> {
        let mut files = HashMap::new();
        walk(dir.as_ref(), &mut files)?;
        let wanted: HashSet<Key> = files.keys().copied().collect();
        let (rows, _) = build_table(gc, ps2, Some(&wanted))?;
        Self::with_table(dir, &rows)
    }

    /// The pack's images that have a name Dolphin would look up.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The pack's images that stand in for some PS2 image or picture.
    pub fn used(&self) -> HashSet<Key> {
        self.images
            .values()
            .chain(self.pictures.values().flatten())
            .copied()
            .collect()
    }

    /// The pack's images that do not (no PS2 equivalent, or not in the table).
    pub fn unused(&self) -> Vec<(Key, &Path)> {
        let used = self.used();
        let mut out: Vec<_> = self
            .files
            .iter()
            .filter(|(k, _)| !used.contains(k))
            .map(|(k, p)| (*k, p.as_path()))
            .collect();
        out.sort();
        out
    }

    fn load(&self, key: Key) -> Option<Arc<ssh::Texture>> {
        let mut cache = self.decoded.lock().unwrap_or_else(|e| e.into_inner());
        cache
            .entry(key)
            .or_insert_with(|| {
                let path = self.files.get(&key)?;
                read_png(path)
                    .inspect_err(|e| eprintln!("hires: {e}"))
                    .ok()
                    .map(Arc::new)
            })
            .clone()
    }

    /// The pack's image for a decoded PS2 image, if it has one. It keeps
    /// the original's name; its size is the pack's.
    pub fn replace(&self, t: &ssh::Texture) -> Option<ssh::Texture> {
        let key = *self.images.get(&(t.width, t.height, rgba_hash(t)))?;
        let hd = self.load(key)?;
        Some(ssh::Texture {
            name: t.name.clone(),
            ..(*hd).clone()
        })
    }

    /// [`Pack::picture`], unless it is smaller than `original` (the PS2's
    /// whole picture) and shows the same thing: several of the Legendary
    /// Edition's loading screens are the GameCube's half-size pictures with
    /// small touches, which would only blur the PS2's.
    pub fn picture_over(&self, entry: usize, original: &ssh::Texture) -> Option<ssh::Texture> {
        let hd = self.picture(entry)?;
        (hd.width >= original.width || score(&hd, original) >= MAX_SCORE).then_some(hd)
    }

    /// A whole loading screen (`/FE/XAFELOAD.BIN` entry `entry`) from the
    /// pack's strips, if it has them all: the PS2's 640x512 picture at the
    /// pack's resolution.
    pub fn picture(&self, entry: usize) -> Option<ssh::Texture> {
        let strips = self.pictures.get(&(PICTURES.to_owned(), Some(entry)))?;
        let strips: Vec<Arc<ssh::Texture>> = strips
            .iter()
            .map(|&k| self.load(k))
            .collect::<Option<_>>()?;
        // GameCube strips are 128, 128 and 64 texels wide; scale each pack
        // image to the tallest one's texel size.
        let height = strips.iter().map(|s| s.height).max()?;
        let width: u32 = strips
            .iter()
            .map(|s| s.width * height / s.height.max(1))
            .sum();
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        let mut x0 = 0;
        for s in &strips {
            let w = s.width * height / s.height.max(1);
            for y in 0..height {
                let sy = (y * s.height / height) as usize;
                for x in 0..w {
                    let sx = (x * s.width / w) as usize;
                    let from = (sy * s.width as usize + sx) * 4;
                    let to = ((y * width + x0 + x) * 4) as usize;
                    rgba[to..to + 4].copy_from_slice(&s.rgba[from..from + 4]);
                }
            }
            x0 += w;
        }
        Some(ssh::Texture {
            name: String::new(),
            width,
            height,
            rgba,
            indices: None,
        })
    }
}

fn walk(dir: &Path, out: &mut HashMap<Key, PathBuf>) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .collect();
    // Sorted, so the same name in two folders always picks the same file.
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out)?;
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png"))
            && let Some(key) = p.file_name().and_then(|n| Key::parse(&n.to_string_lossy()))
        {
            out.entry(key).or_insert(p);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for n in [
            "tex1_256x128_1e7df664c2a5adba_14.png",
            "tex1_128x128_06cabd78676a49c6_64b17738ed8469f9_9.png",
        ] {
            let k = Key::parse(n).unwrap();
            assert_eq!(format!("tex1_{k}.png"), n);
        }
        assert!(Key::parse("tex1_64x64_m_0123456789abcdef_14.png").is_none());
        assert!(Key::parse("tex1_64x64_0123456789abcdef_$_9.png").is_none());
        assert!(Key::parse("tex1_64x64_0123456789abcdef_14_mip1.png").is_none());
    }

    #[test]
    fn xxh64_matches_the_reference() {
        // XXH64 test vectors (seed 0).
        assert_eq!(XxHash64::oneshot(0, b""), 0xef46db3751d8e999);
        assert_eq!(XxHash64::oneshot(0, b"a"), 0xd24ec4f1a98c6e5b);
    }

    #[test]
    fn gx_sizes_round_up_to_whole_tiles() {
        assert_eq!(gx_size(256, 128, gx::CMPR), Some(256 * 128 / 2));
        assert_eq!(gx_size(4, 4, gx::CMPR), Some(32)); // one 8x8 tile
        assert_eq!(gx_size(64, 64, gx::C8), Some(64 * 64));
        assert_eq!(gx_size(6, 4, gx::C8), Some(32));
        assert_eq!(gx_size(128, 256, gx::RGBA8), Some(128 * 256 * 4));
        assert_eq!(gx_size(4, 4, 7), None);
    }

    #[test]
    fn c8_palette_hash_covers_only_the_indices_used() {
        // An 8x4 C8 texture using indices 2..=5 of a 256-entry palette.
        let mut data = vec![2u8; 32];
        data[7] = 5;
        data[9] = 3;
        let palette: Vec<u8> = (0..512).map(|i| (i * 7) as u8).collect();
        let k = Key::of(8, 4, gx::C8, &data, &palette).unwrap();
        assert_eq!(k.texture, XxHash64::oneshot(0, &data));
        assert_eq!(k.tlut, Some(XxHash64::oneshot(0, &palette[4..12])));
        // Entries outside the range do not matter.
        let mut other = palette.clone();
        other[0] ^= 0xff;
        other[100] ^= 0xff;
        assert_eq!(Key::of(8, 4, gx::C8, &data, &other), Some(k));
        // C4: both nibbles count.
        let c4 = [0x31u8; 32];
        let k = Key::of(8, 8, gx::C4, &c4, &palette).unwrap();
        assert_eq!(k.tlut, Some(XxHash64::oneshot(0, &palette[2..8])));
        // Non-paletted formats have no second hash.
        assert_eq!(Key::of(8, 8, gx::CMPR, &[0; 32], &[]).unwrap().tlut, None);
    }

    #[test]
    fn table_round_trips() {
        let rows = vec![
            Row {
                key: Key::parse("tex1_256x128_1e7df664c2a5adba_14.png").unwrap(),
                use_: Use::Image,
                ps2: Place {
                    file: "/xf/xf000002".into(),
                    entry: Some(663),
                    image: 1,
                },
                name: "head".into(),
                width: 128,
                height: 64,
                hash: 0x0123456789abcdef,
                gc: Place {
                    file: "/xf/xf000002".into(),
                    entry: Some(663),
                    image: 1,
                },
                score: 0.012,
            },
            Row {
                key: Key::parse("tex1_64x64_0123456789abcdef_fedcba9876543210_9.png").unwrap(),
                use_: Use::Strip,
                ps2: Place {
                    file: "/xldscrn.bin".into(),
                    entry: None,
                    image: 2,
                },
                name: String::new(),
                width: 64,
                height: 256,
                hash: 0,
                gc: Place {
                    file: "/xldscrn.bin".into(),
                    entry: None,
                    image: 2,
                },
                score: 0.0,
            },
        ];
        assert_eq!(parse_table(&write_table(&rows)).unwrap(), rows);
    }

    #[test]
    fn the_built_in_table_parses() {
        let rows = parse_table(GNZE69_TABLE).unwrap();
        assert!(rows.iter().all(|r| r.score < MAX_SCORE));
    }

    #[test]
    fn a_pack_replaces_by_pixels_and_joins_strips() {
        let dir = std::env::temp_dir().join(format!("street-hires-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let write = |name: &str, w: u32, h: u32, px: [u8; 4]| {
            let f = std::fs::File::create(dir.join("sub").join(name)).unwrap();
            let mut e = png::Encoder::new(f, w, h);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            let rgba: Vec<u8> = (0..w * h).flat_map(|_| px).collect();
            e.write_header().unwrap().write_image_data(&rgba).unwrap();
        };
        let face = "tex1_4x4_00000000000000aa_14.png";
        write(face, 8, 8, [1, 2, 3, 255]);
        write("tex1_8x8_00000000000000b0_6.png", 4, 8, [10, 0, 0, 255]);
        write("tex1_8x8_00000000000000b1_6.png", 2, 4, [20, 0, 0, 255]);
        let ps2 = ssh::Texture {
            name: "head".into(),
            width: 2,
            height: 2,
            rgba: vec![9; 16],
            indices: None,
        };
        let place = |file: &str, entry, image| Place {
            file: file.into(),
            entry,
            image,
        };
        let row = |key: &str, use_, ps2: Place, hash| Row {
            key: Key::parse(key).unwrap(),
            use_,
            ps2: ps2.clone(),
            name: String::new(),
            width: 2,
            height: 2,
            hash,
            gc: ps2,
            score: 0.0,
        };
        let rows = vec![
            row(
                face,
                Use::Image,
                place("/xf/xf000002", Some(3), 1),
                rgba_hash(&ps2),
            ),
            row(
                "tex1_8x8_00000000000000b0_6.png",
                Use::Strip,
                place(PICTURES, Some(7), 0),
                0,
            ),
            row(
                "tex1_8x8_00000000000000b1_6.png",
                Use::Strip,
                place(PICTURES, Some(7), 1),
                0,
            ),
        ];
        let pack = Pack::with_table(&dir, &rows).unwrap();
        assert_eq!(pack.len(), 3);
        assert!(pack.unused().is_empty());
        let hd = pack.replace(&ps2).unwrap();
        assert_eq!((hd.name.as_str(), hd.width, hd.height), ("head", 8, 8));
        assert_eq!(&hd.rgba[..4], &[1, 2, 3, 255]);
        assert!(
            pack.replace(&ssh::Texture {
                rgba: vec![8; 16],
                ..ps2.clone()
            })
            .is_none()
        );
        // The second strip is drawn at the first's texel size: 4x8 + 4x8.
        let pic = pack.picture(7).unwrap();
        assert_eq!((pic.width, pic.height), (8, 8));
        assert_eq!(pic.rgba[3 * 4], 10);
        assert_eq!(pic.rgba[4 * 4], 20);
        assert!(pack.picture(8).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
