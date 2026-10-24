//! EAGL models for the GameCube: Street V3 (`G3VE69`) and Street V1
//! (`GNSE69`). They use the same ELF container as the PS2's [`crate::eagl`]
//! models (relocated pointer words little-endian), but the payload is
//! big-endian and drawn through GX display lists instead of VIF chains.
//!
//! What the decoder relies on (worked out on V3's player models, checked by
//! rendering Luigi, Mario and Peach from the disc):
//!
//! - **Stream tables**: a relocated pointer to the stream's name
//!   (`Coordinates`, `Normals`, `UV`), u32 entry count, then 16-byte entries:
//!   u32 batch, u16 element count, u16 format, u32 byte size, a pointer to the
//!   array (the byte size is padded to four). Coordinates are f32 × 3;
//!   normals i16 × 3 in 8.8 or 2.14 fixed point (normalised here), or f32 × 3;
//!   UVs i16 × 2 / 1024 (or f32 × 2).
//!   Every batch has all three, with equal counts.
//! - **Display lists**: GX draw commands, `0x98` strip, `0x90` triangles,
//!   `0xa0` fan (the low three bits pick a vertex format), a u16 vertex count,
//!   then per vertex a u8 position/normal matrix slot, a u8 texture matrix
//!   slot, and position, normal and UV indices: u8 when the batch has up to 255
//!   vertices, u16 above. A list ends at its first byte that is not a draw
//!   command (zero padding, or the next record straight away).
//! - Each list is named by a relocated pointer in its render record. The next
//!   pointer in that record to a `Coordinates` array gives the batch, and the
//!   next `__EAGL::TAR` symbol it refers to the texture slot (`SHAPENAME=head`).
//! - GX treats clockwise triangles as front-facing; meshes come out
//!   counter-clockwise here, like the PS2 models and glTF.
//! - Positions are in model space (the bind pose). **Skinning**: the render
//!   record holds `1, &EAGLAnimationBuffer, n, &rows`: `n` blend rows of four
//!   big-endian floats each. A float's low byte is a skeleton bone index and
//!   the rest (low byte cleared) its weight, so `3f80000e` is bone 14 with
//!   weight 1. A vertex's position/normal matrix slot (a multiple of three,
//!   as GX counts matrix rows) picks row `slot / 3`. The bones index the
//!   skeleton named by the model's `skeletontype` ([`parse_skeleton`]).

use std::collections::{HashMap, HashSet};

use crate::{
    Error, Result, bytes,
    eagl::{Bone, Mesh, Model, ModelStats, Object, Skeleton},
};

const STRIP: u8 = 0x98;
const TRIANGLES: u8 = 0x90;
const FAN: u8 = 0xa0;
/// How far into a render record to look for its arrays and texture slot.
const RECORD_REACH: u32 = 0x140;
/// More blend rows than GX has matrix slots (10) would be corrupt.
const MAX_ROWS: u32 = 64;

#[derive(Clone, Copy, Debug)]
struct Stream {
    count: usize,
    stride: usize,
    at: usize,
}

fn is_draw(op: u8) -> bool {
    matches!(op & 0xf8, STRIP | TRIANGLES | FAN)
}

pub fn parse(b: &[u8]) -> Result<Model> {
    let obj = Object::read(b)?;
    if !obj.symbols.iter().any(|s| s.name.starts_with("__Model:::"))
        && obj.symbols.iter().any(|s| s.name.starts_with("__Model::"))
    {
        return parse_v1(&obj);
    }
    let data = obj.data;
    let symbol = |prefix: &str| {
        obj.symbols
            .iter()
            .filter(|s| s.section == obj.data_index)
            .find_map(|s| Some((s.name.strip_prefix(prefix)?, s.value as usize)))
    };
    let name = symbol("__Model:::").map_or("", |(n, _)| n).to_owned();
    let bbox = symbol("__BBOX:::").and_then(|(_, at)| {
        let f = |k: usize| bytes::f32_be(data, at + 4 * k).ok();
        Some(([f(0)?, f(1)?, f(2)?], [f(3)?, f(4)?, f(5)?]))
    });

    let streams = stream_tables(&obj);
    let coords = streams
        .get("Coordinates")
        .ok_or_else(|| Error::format("GameCube model has no Coordinates streams"))?;
    let batch_at: HashMap<usize, u32> = coords.iter().map(|(&b, s)| (s.at, b)).collect();
    let batches = coords.keys().max().map_or(0, |&m| m as usize + 1);
    let mut batch_sizes = vec![0u16; batches];
    for (&b, s) in coords {
        batch_sizes[b as usize] = s.count.min(u16::MAX as usize) as u16;
    }

    // Vertex arrays can start with a byte that looks like a draw command
    // (a float's 0xa3), so pointers to them are not display lists.
    let arrays: HashSet<u32> = streams
        .values()
        .flat_map(|m| m.values().map(|s| s.at as u32))
        .collect();
    let mut lists: Vec<(u32, u32)> = obj
        .pointers
        .iter()
        .map(|(&off, &target)| (off, target))
        .filter(|&(_, target)| {
            !arrays.contains(&target) && data.get(target as usize).is_some_and(|&op| is_draw(op))
        })
        .collect();
    lists.sort_unstable();

    let mut meshes = Vec::new();
    let mut seen = HashSet::new();
    let mut stats = ModelStats::default();
    for (off, target) in lists {
        let Some(batch) = (1..RECORD_REACH).find_map(|k| {
            obj.pointers
                .get(&(off + k))
                .and_then(|t| batch_at.get(&(*t as usize)).copied())
        }) else {
            continue;
        };
        if !seen.insert(target) {
            continue;
        }
        let texture = (1..RECORD_REACH).find_map(|k| {
            let sym = obj.externals.get(&(off + k))?;
            texture_slot(&obj.symbols[*sym].name)
        });
        let rows = blend_rows(&obj, off);
        let (Some(pos), Some(nrm), Some(uv)) = (
            coords.get(&batch),
            streams.get("Normals").and_then(|s| s.get(&batch)),
            streams.get("UV").and_then(|s| s.get(&batch)),
        ) else {
            stats.skipped_batches += 1;
            continue;
        };
        match batch_mesh(
            data,
            target as usize,
            batch,
            *pos,
            *nrm,
            *uv,
            rows.as_deref(),
        ) {
            Ok(mut mesh) => {
                mesh.texture = texture;
                stats.batches += 1;
                stats.triangles += mesh.indices.len() / 3;
                meshes.push(mesh);
            }
            Err(_) => stats.skipped_batches += 1,
        }
    }
    stats.chains = stats.batches;
    Ok(Model {
        name,
        meshes,
        stats,
        bbox,
        microcode: Vec::new(),
        batch_sizes,
        half_extent: None,
    })
}

/// Street V1's (`GNSE69`) older EAGL models: `__Model::` symbols with two
/// colons, each texture slot a data symbol `__TAR::tar_<name>_<n>`, the
/// skeleton inside the model (`__Skeleton::Root`, Vol. 2's 43 bones), and
/// render records that describe their own arrays. Around the relocated
/// pointer to its display list at `a`:
///
/// - `a - 28 .. a`: (count, pointer) for the positions (i16 × 3), normals
///   (i8 × 3) and UVs (i16 × 2), then the list's byte size;
/// - `a + 4`: (1, pointer to the `__TAR` slot);
/// - before them, `1, &EAGLAnimationBuffer, n, &rows`: the blend rows, as
///   in V3.
///
/// Display lists hold per vertex the matrix slot and 16-bit indices, with no
/// texture matrix byte. Positions and UVs are 8.8 fixed point (the fraction
/// is checked against `__BBOX::`), normals are scaled to unit length.
fn parse_v1(obj: &Object) -> Result<Model> {
    let data = obj.data;
    let symbol = |prefix: &str| {
        obj.symbols
            .iter()
            .filter(|s| s.section == obj.data_index)
            .find_map(|s| Some((s.name.strip_prefix(prefix)?, s.value as usize)))
    };
    let name = symbol("__Model::").map_or("", |(n, _)| n).to_owned();
    let bbox = symbol("__BBOX::").and_then(|(_, at)| {
        let f = |k: usize| bytes::f32_be(data, at + 4 * k).ok();
        Some(([f(0)?, f(1)?, f(2)?], [f(3)?, f(4)?, f(5)?]))
    });
    let tar_at: HashMap<u32, String> = obj
        .symbols
        .iter()
        .filter(|s| s.section == obj.data_index)
        .filter_map(|s| {
            let slot = s.name.strip_prefix("__TAR::")?;
            let slot = slot.strip_prefix("tar_").unwrap_or(slot);
            let slot = match slot.rsplit_once('_') {
                Some((head, tail)) if tail.bytes().all(|b| b.is_ascii_digit()) => head,
                _ => slot,
            };
            Some((s.value, slot.to_ascii_lowercase()))
        })
        .collect();
    let word = |off: u32| bytes::u32_be(data, off as usize).ok();
    let mut lists: Vec<(u32, u32)> = obj
        .pointers
        .iter()
        .map(|(&off, &target)| (off, target))
        .filter(|&(_, target)| data.get(target as usize).is_some_and(|&op| is_draw(op)))
        .collect();
    lists.sort_unstable();

    struct Batch {
        list: usize,
        arrays: [(usize, usize); 3],
        rows: Option<Vec<BlendRow>>,
        texture: Option<String>,
    }
    let mut batches = Vec::new();
    let mut seen = HashSet::new();
    for (a, list) in lists {
        let (Some(np), Some(nn), Some(nu)) = (word(a - 28), word(a - 20), word(a - 12)) else {
            continue;
        };
        let (Some(&pp), Some(&pn), Some(&pu)) = (
            obj.pointers.get(&(a - 24)),
            obj.pointers.get(&(a - 16)),
            obj.pointers.get(&(a - 8)),
        ) else {
            continue;
        };
        if np == 0 || np != nn || np != nu || np > 0xffff || !seen.insert(list) {
            continue;
        }
        let rows = (a.saturating_sub(0x60)..a)
            .rev()
            .find(|o| {
                obj.externals
                    .get(o)
                    .is_some_and(|&s| obj.symbols[s].name.contains("EAGLAnimationBuffer"))
            })
            .and_then(|at| {
                let count = word(at + 4)?;
                let rows = *obj.pointers.get(&(at + 8))? as usize;
                if count == 0 || count > MAX_ROWS {
                    return None;
                }
                read_rows(data, rows, count as usize)
            });
        let texture = obj
            .pointers
            .get(&(a + 8))
            .and_then(|t| tar_at.get(t).cloned());
        let n = np as usize;
        batches.push(Batch {
            list: list as usize,
            arrays: [(pp as usize, n), (pn as usize, n), (pu as usize, n)],
            rows,
            texture,
        });
    }
    if batches.is_empty() {
        return Err(Error::format("Street V1 model has no render records"));
    }

    // Position fraction bits: the ones that make the vertices fill __BBOX.
    let mut raw_max = 0i32;
    for b in &batches {
        let (at, n) = b.arrays[0];
        for i in 0..n * 3 {
            raw_max = raw_max.max(i32::from(bytes::i16_be(data, at + 2 * i)?).abs());
        }
    }
    let frac = bbox
        .map(|(lo, hi)| {
            let extent = lo.iter().chain(&hi).fold(0.0f32, |m, v| m.max(v.abs()));
            (0..16)
                .min_by(|&x, &y| {
                    let err = |f: i32| (raw_max as f32 / (1 << f) as f32 - extent).abs();
                    err(x).total_cmp(&err(y))
                })
                .unwrap_or(8)
        })
        .unwrap_or(8);
    let pos_scale = 1.0 / (1u32 << frac) as f32;

    let mut meshes = Vec::new();
    let mut stats = ModelStats::default();
    let mut batch_sizes = Vec::new();
    for (number, b) in batches.iter().enumerate() {
        let counts = [b.arrays[0].1, b.arrays[1].1, b.arrays[2].1];
        batch_sizes.push(counts[0].min(u16::MAX as usize) as u16);
        let prims = [[2, 2, 2], [1, 1, 1], [2, 2, 1], [1, 1, 2]]
            .into_iter()
            .find_map(|w| display_list_with(data, b.list, w, counts, false).ok());
        let Some(prims) = prims else {
            stats.skipped_batches += 1;
            continue;
        };
        let mut mesh = Mesh {
            texture: b.texture.clone(),
            ..Mesh::default()
        };
        let mut index: HashMap<[u16; 4], u32> = HashMap::new();
        let mut vertex = |v: [u16; 4], mesh: &mut Mesh| -> Result<u32> {
            if let Some(&i) = index.get(&v) {
                return Ok(i);
            }
            let i = mesh.positions.len() as u32;
            let (pa, na, ua) = (b.arrays[0].0, b.arrays[1].0, b.arrays[2].0);
            let p = |k: usize| bytes::i16_be(data, pa + 6 * v[1] as usize + 2 * k);
            mesh.positions.push([
                f32::from(p(0)?) * pos_scale,
                f32::from(p(1)?) * pos_scale,
                f32::from(p(2)?) * pos_scale,
            ]);
            let n = |k: usize| -> Result<f32> {
                Ok(f32::from(
                    *data
                        .get(na + 3 * v[2] as usize + k)
                        .ok_or_else(|| Error::format("normal past the end"))?
                        as i8,
                ))
            };
            let nv = [n(0)?, n(1)?, n(2)?];
            let len = (nv[0] * nv[0] + nv[1] * nv[1] + nv[2] * nv[2]).sqrt();
            mesh.normals.push(if len > 0.0 {
                nv.map(|c| c / len)
            } else {
                [0.0, 1.0, 0.0]
            });
            let u = |k: usize| bytes::i16_be(data, ua + 4 * v[3] as usize + 2 * k);
            mesh.uvs
                .push([f32::from(u(0)?) / 256.0, f32::from(u(1)?) / 256.0]);
            mesh.colors.push([0x80, 0x80, 0x80, 0x80]);
            mesh.sources.push((number as u32, v[1]));
            if let Some(rows) = &b.rows {
                let row = rows
                    .get(v[0] as usize / 3)
                    .ok_or_else(|| Error::format("GX matrix slot past the blend rows"))?;
                let (j, w) = skin(row);
                mesh.joints.push(j);
                mesh.weights.push(w);
            }
            index.insert(v, i);
            Ok(i)
        };
        let mut ok = true;
        for (op, verts) in prims {
            let Ok(ids) = verts
                .into_iter()
                .map(|v| vertex(v, &mut mesh))
                .collect::<Result<Vec<_>>>()
            else {
                ok = false;
                break;
            };
            push_primitive(&mut mesh, op, &ids);
        }
        if !ok {
            stats.skipped_batches += 1;
            continue;
        }
        stats.batches += 1;
        stats.triangles += mesh.indices.len() / 3;
        meshes.push(mesh);
    }
    stats.chains = stats.batches;
    Ok(Model {
        name,
        meshes,
        stats,
        bbox,
        microcode: Vec::new(),
        batch_sizes,
        half_extent: None,
    })
}

/// `count` blend rows of four big-endian floats at `at`.
fn read_rows(data: &[u8], at: usize, count: usize) -> Option<Vec<BlendRow>> {
    (0..count)
        .map(|r| {
            let mut row = [(0u8, 0f32); 4];
            for (k, slot) in row.iter_mut().enumerate() {
                let bits = bytes::u32_be(data, at + 16 * r + 4 * k).ok()?;
                *slot = ((bits & 0xff) as u8, f32::from_bits(bits & !0xff));
            }
            Some(row)
        })
        .collect()
}

/// One primitive's vertices as counter-clockwise triangles (GX's front
/// faces are clockwise).
fn push_primitive(mesh: &mut Mesh, op: u8, ids: &[u32]) {
    match op & 0xf8 {
        STRIP => {
            for k in 0..ids.len().saturating_sub(2) {
                let (a, b, c) = (ids[k], ids[k + 1], ids[k + 2]);
                if a == b || b == c || a == c {
                    continue;
                }
                let tri = if k % 2 == 0 { [b, a, c] } else { [a, b, c] };
                mesh.indices.extend(tri);
            }
        }
        TRIANGLES => {
            for &[a, b, c] in ids.as_chunks::<3>().0 {
                mesh.indices.extend([a, c, b]);
            }
        }
        _ => {
            for k in 1..ids.len().saturating_sub(1) {
                mesh.indices.extend([ids[0], ids[k + 1], ids[k]]);
            }
        }
    }
}

/// One blend row: up to four (skeleton bone, weight) pairs.
type BlendRow = [(u8, f32); 4];

/// The blend rows of the render record whose display-list pointer is at
/// `off`: after `&EAGLAnimationBuffer` come their count and a pointer.
fn blend_rows(obj: &Object, off: u32) -> Option<Vec<BlendRow>> {
    let at = (off + 1..off + RECORD_REACH).find(|o| {
        obj.externals
            .get(o)
            .is_some_and(|&s| obj.symbols[s].name.contains("EAGLAnimationBuffer"))
    })?;
    let count = bytes::u32_be(obj.data, at as usize + 4).ok()?;
    let rows = *obj.pointers.get(&(at + 8))? as usize;
    if count == 0 || count > MAX_ROWS {
        return None;
    }
    (0..count as usize)
        .map(|r| {
            let mut row = [(0u8, 0f32); 4];
            for (k, slot) in row.iter_mut().enumerate() {
                let bits = bytes::u32_be(obj.data, rows + 16 * r + 4 * k).ok()?;
                *slot = ((bits & 0xff) as u8, f32::from_bits(bits & !0xff));
            }
            Some(row)
        })
        .collect()
}

/// A row as joints and weights (unused entries weigh 0), normalised.
fn skin(row: &BlendRow) -> ([u16; 4], [f32; 4]) {
    let mut joints = [0u16; 4];
    let mut weights = [0f32; 4];
    let mut n = 0;
    for &(bone, w) in row {
        if w > 0.0 {
            joints[n] = bone as u16;
            weights[n] = w;
            n += 1;
        }
    }
    let total: f32 = weights.iter().sum();
    if total > 0.0 {
        weights.iter_mut().for_each(|w| *w /= total);
    } else {
        weights[0] = 1.0;
    }
    (joints, weights)
}

/// A skeleton file (`skeleton.o`, `skel_nintendo_mario.o`, ...): the PS2
/// layout of [`crate::eagl::parse_skeleton`] (112 bytes per bone, parents
/// first) with big-endian numbers. The fourth word of a bone's translation
/// is the bone that mirrors it (`LThigh` <-> `RThigh`).
pub fn parse_skeleton(b: &[u8]) -> Result<Skeleton> {
    let obj = Object::read(b)?;
    let data = obj.data;
    let at = obj
        .symbols
        .iter()
        .find(|s| s.name.starts_with("__Skeleton::"))
        .ok_or_else(|| Error::format("no __Skeleton symbol"))?
        .value as usize;
    let count = bytes::u32_be(data, at + 8)? as usize;
    if count > 256 {
        return Err(Error::format(format!("skeleton with {count} bones")));
    }
    let mut names = vec![String::new(); count];
    for s in &obj.symbols {
        if let Some(name) = s
            .name
            .strip_prefix("__Bone:::")
            .or_else(|| s.name.strip_prefix("__Bone::"))
        {
            let i = bytes::u32_be(data, s.value as usize)? as usize;
            if let Some(slot) = names.get_mut(i) {
                *slot = name.strip_prefix("Root.").unwrap_or(name).to_owned();
            }
        }
    }
    let f = |off: usize| bytes::f32_be(data, off);
    let mut bones = Vec::with_capacity(count);
    for (i, name) in names.into_iter().enumerate() {
        let o = at + 16 + 112 * i;
        let parent = bytes::u32_be(data, o + 12)? as i32;
        let mut inverse_bind = [0.0; 16];
        for (k, v) in inverse_bind.iter_mut().enumerate() {
            *v = f(o + 48 + 4 * k)?;
        }
        bones.push(Bone {
            name,
            parent: usize::try_from(parent).ok().filter(|&p| p < i),
            scale: [f(o)?, f(o + 4)?, f(o + 8)?],
            rotation: [f(o + 16)?, f(o + 20)?, f(o + 24)?, f(o + 28)?],
            translation: [f(o + 32)?, f(o + 36)?, f(o + 40)?],
            id: bytes::u32_be(data, o + 44)?,
            inverse_bind,
        });
    }
    Ok(Skeleton { bones })
}

/// `SHAPENAME=head,1;...` in a texture slot's symbol name -> `head`.
fn texture_slot(symbol: &str) -> Option<String> {
    let rest = symbol.strip_prefix("__EAGL::TAR:::")?;
    let at = rest.find("SHAPENAME=")? + "SHAPENAME=".len();
    let end = rest[at..].find([',', ';']).map_or(rest.len(), |e| at + e);
    Some(rest[at..end].to_ascii_lowercase())
}

/// stream name -> batch -> array.
fn stream_tables(obj: &Object) -> HashMap<String, HashMap<u32, Stream>> {
    let data = obj.data;
    let mut out: HashMap<String, HashMap<u32, Stream>> = HashMap::new();
    for (&off, &target) in &obj.pointers {
        let Ok(name) = bytes::cstr(data, target as usize) else {
            continue;
        };
        if !matches!(name.as_str(), "Coordinates" | "Normals" | "UV") {
            continue;
        }
        let Ok(n) = bytes::u32_be(data, off as usize + 4) else {
            continue;
        };
        if n == 0 || n > 4096 {
            continue;
        }
        let mut entries = HashMap::new();
        for k in 0..n {
            let e = off as usize + 8 + 16 * k as usize;
            let (Ok(batch), Ok(count), Ok(size), Some(&at)) = (
                bytes::u32_be(data, e),
                bytes::u16_be(data, e + 4),
                bytes::u32_be(data, e + 8),
                obj.pointers.get(&(e as u32 + 12)),
            ) else {
                break;
            };
            // Sizes are padded to four bytes (301 normals take 1,808, not
            // 1,806), so the element size is the whole part of size / count.
            let count = count as usize;
            let stride = (size as usize).checked_div(count).unwrap_or(0);
            if !matches!(stride, 4 | 6 | 8 | 12) || size as usize - count * stride >= 32 {
                continue;
            }
            entries.insert(
                batch,
                Stream {
                    count,
                    stride,
                    at: at as usize,
                },
            );
        }
        out.entry(name).or_default().extend(entries);
    }
    out
}

fn batch_mesh(
    data: &[u8],
    list: usize,
    batch: u32,
    pos: Stream,
    nrm: Stream,
    uv: Stream,
    rows: Option<&[BlendRow]>,
) -> Result<Mesh> {
    let counts = [pos.count, nrm.count, uv.count];
    let prims = index_widths(counts)
        .into_iter()
        .find_map(|w| display_list(data, list, w, counts).ok())
        .ok_or_else(|| Error::format("no index widths read the GX display list"))?;

    let position = |i: usize| -> Result<[f32; 3]> {
        let at = pos.at + i * pos.stride;
        Ok([
            bytes::f32_be(data, at)?,
            bytes::f32_be(data, at + 4)?,
            bytes::f32_be(data, at + 8)?,
        ])
    };
    // Fixed-point normals use 8 fraction bits in most batches and 14 in a
    // few (the format code is the same), so each is scaled to unit length.
    let normal = |i: usize| -> Result<[f32; 3]> {
        let at = nrm.at + i * nrm.stride;
        let n = if nrm.stride == 6 {
            let c = |k: usize| bytes::i16_be(data, at + 2 * k).map(f32::from);
            [c(0)?, c(1)?, c(2)?]
        } else {
            [
                bytes::f32_be(data, at)?,
                bytes::f32_be(data, at + 4)?,
                bytes::f32_be(data, at + 8)?,
            ]
        };
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        Ok(if len > 0.0 {
            n.map(|c| c / len)
        } else {
            [0.0, 1.0, 0.0]
        })
    };
    let texcoord = |i: usize| -> Result<[f32; 2]> {
        let at = uv.at + i * uv.stride;
        Ok(if uv.stride == 4 {
            let c = |k: usize| bytes::i16_be(data, at + 2 * k).map(|v| v as f32 / 1024.0);
            [c(0)?, c(1)?]
        } else {
            [bytes::f32_be(data, at)?, bytes::f32_be(data, at + 4)?]
        })
    };

    let mut mesh = Mesh::default();
    let mut index: HashMap<[u16; 4], u32> = HashMap::new();
    let mut vertex = |v: [u16; 4], mesh: &mut Mesh| -> Result<u32> {
        if let Some(&i) = index.get(&v) {
            return Ok(i);
        }
        let i = mesh.positions.len() as u32;
        mesh.positions.push(position(v[1] as usize)?);
        mesh.normals.push(normal(v[2] as usize)?);
        mesh.uvs.push(texcoord(v[3] as usize)?);
        mesh.colors.push([0x80, 0x80, 0x80, 0x80]);
        mesh.sources.push((batch, v[1]));
        if let Some(rows) = rows {
            let row = rows
                .get(v[0] as usize / 3)
                .ok_or_else(|| Error::format("GX matrix slot past the blend rows"))?;
            let (j, w) = skin(row);
            mesh.joints.push(j);
            mesh.weights.push(w);
        }
        index.insert(v, i);
        Ok(i)
    };
    for (op, verts) in prims {
        let ids = verts
            .into_iter()
            .map(|v| vertex(v, &mut mesh))
            .collect::<Result<Vec<_>>>()?;
        match op & 0xf8 {
            STRIP => {
                for k in 0..ids.len().saturating_sub(2) {
                    let (a, b, c) = (ids[k], ids[k + 1], ids[k + 2]);
                    if a == b || b == c || a == c {
                        continue;
                    }
                    // GX's clockwise front faces, turned counter-clockwise.
                    let tri = if k % 2 == 0 { [b, a, c] } else { [a, b, c] };
                    mesh.indices.extend(tri);
                }
            }
            TRIANGLES => {
                for &[a, b, c] in ids.as_chunks::<3>().0 {
                    mesh.indices.extend([a, c, b]);
                }
            }
            _ => {
                for k in 1..ids.len().saturating_sub(1) {
                    mesh.indices.extend([ids[0], ids[k + 1], ids[k]]);
                }
            }
        }
    }
    Ok(mesh)
}

/// The byte widths (1 or 2) to try for the position, normal and UV indices:
/// first one byte per array of up to 255 elements, then every other mix (a
/// batch's descriptor can make an index 16-bit whatever its array's size).
fn index_widths(counts: [usize; 3]) -> Vec<[usize; 3]> {
    let natural = counts.map(|c| if c > 255 { 2 } else { 1 });
    let mut out = vec![natural];
    for bits in 0..8 {
        let w = [0, 1, 2].map(|k| 1 + (bits >> k & 1));
        if w != natural && (0..3).all(|k| counts[k] <= 255 || w[k] == 2) {
            out.push(w);
        }
    }
    out
}

/// The primitives of the display list at `start`: (opcode, per vertex the
/// matrix slot and the position, normal and UV index), with index widths
/// `widths`. Every index must lie inside its array, and each vertex's texture
/// matrix slot is its position matrix slot plus 30 (`GX_TEXMTX0`), which
/// rules out wrong widths.
fn display_list(
    data: &[u8],
    start: usize,
    widths: [usize; 3],
    counts: [usize; 3],
) -> Result<Vec<(u8, Vec<[u16; 4]>)>> {
    display_list_with(data, start, widths, counts, true)
}

/// [`display_list`], with or without the texture matrix byte (Street V1's
/// models have only the position/normal matrix slot).
fn display_list_with(
    data: &[u8],
    start: usize,
    widths: [usize; 3],
    counts: [usize; 3],
    texmtx: bool,
) -> Result<Vec<(u8, Vec<[u16; 4]>)>> {
    let head = 1 + usize::from(texmtx);
    let stride = head + widths.iter().sum::<usize>();
    let offsets = [head, head + widths[0], head + widths[0] + widths[1]];
    let mut i = start;
    let mut prims = Vec::new();
    while let Some(&op) = data.get(i) {
        if !is_draw(op) {
            break;
        }
        let n = bytes::u16_be(data, i + 1)? as usize;
        i += 3;
        let body = bytes::range(data, i, n * stride)?;
        let mut verts = Vec::with_capacity(n);
        for v in body.chunks_exact(stride) {
            let idx = |k: usize| -> u16 {
                let at = offsets[k];
                if widths[k] == 2 {
                    u16::from_be_bytes([v[at], v[at + 1]])
                } else {
                    v[at] as u16
                }
            };
            let tri = [idx(0), idx(1), idx(2)];
            if (0..3).any(|k| tri[k] as usize >= counts[k]) {
                return Err(Error::format("GX vertex index outside its array"));
            }
            if (texmtx && v[1] != v[0].wrapping_add(30)) || v[0] % 3 != 0 {
                return Err(Error::format("GX matrix slots out of step"));
            }
            verts.push([v[0] as u16, tri[0], tri[1], tri[2]]);
        }
        i += n * stride;
        prims.push((op, verts));
    }
    if prims.is_empty() {
        return Err(Error::format("empty GX display list"));
    }
    Ok(prims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_slots_come_from_shapename() {
        let s = "__EAGL::TAR:::RUNTIME_ALLOC::UID=2425;SHAPENAME=Head,1;GCEXTOBJ_SetClampModeU=EAGL::GCCM_CLAMP";
        assert_eq!(texture_slot(s).as_deref(), Some("head"));
        assert_eq!(texture_slot("__Model:::x"), None);
    }

    #[test]
    fn display_lists_end_at_the_first_non_draw_byte() {
        // strip of 3 vertices with u8 indices, then a record byte (0x05).
        let mut d = vec![0x98, 0, 3];
        d.extend([0, 30, 0, 0, 0, 0, 30, 1, 1, 1, 0, 30, 2, 2, 2]);
        d.extend([0x05, 0, 1, 0]);
        let prims = display_list(&d, 0, [1, 1, 1], [3, 3, 3]).unwrap();
        assert_eq!(prims.len(), 1);
        assert_eq!(prims[0].1, vec![[0, 0, 0, 0], [0, 1, 1, 1], [0, 2, 2, 2]]);
        // An index past its array is rejected, so a wrong width can't slip through.
        assert!(display_list(&d, 0, [1, 1, 1], [2, 3, 3]).is_err());
    }

    #[test]
    fn sixteen_bit_indices_for_large_batches() {
        let mut d = vec![0x98, 0, 3];
        for k in 0u16..3 {
            d.extend([9, 39]);
            for _ in 0..3 {
                d.extend((300 + k).to_be_bytes());
            }
        }
        let prims = display_list(&d, 0, [2, 2, 2], [400, 400, 400]).unwrap();
        assert_eq!(prims[0].1[2], [9, 302, 302, 302]);
    }

    #[test]
    fn index_widths_can_differ_per_array() {
        // Mario's eyes: one-byte position and normal indices, two-byte UVs.
        let mut d = vec![0x98, 0, 2];
        d.extend([0, 30, 0, 0, 0, 0, 0, 30, 1, 1, 0, 1]);
        d.push(0x05);
        let counts = [148, 148, 148];
        let prims = index_widths(counts)
            .into_iter()
            .find_map(|w| display_list(&d, 0, w, counts).ok())
            .unwrap();
        assert_eq!(prims[0].1, vec![[0, 0, 0, 0], [0, 1, 1, 1]]);
    }
}
