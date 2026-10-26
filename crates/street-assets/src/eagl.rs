//! EAGL models (`.o`): PS2 ELF relocatable objects whose `.data` section holds
//! DMA chains of VIF packets, plus named symbols the runtime links against.
//!
//! What the decoder relies on (worked out from Vol. 2's court models, and for
//! the vertex layouts from the VU1 microprograms in `/XF/XF000007`):
//!
//! - `__Model:::<name>` names the model; `__EAGL::TAR:::tar_<tex>_<n>` symbols
//!   are texture slots, where `<tex>` is the 4-character name of an image in the
//!   court's `.ssh` sheets.
//! - Every vertex chain is drawn through a DMA CALL tag whose address word is
//!   relocated to the chain's start. Those CALL tags are found by scanning the
//!   section at 16-byte alignment, keeping the ones a render method uses
//!   (chains may themselves CALL shared sub-chains).
//! - A texture change is a relocated pointer to a TAR slot preceded by the word
//!   `0x0000000e`. It appears after the CALL it applies to, and only when the
//!   texture changes, so a chain without one reuses the previous texture.
//! - Each batch is one microprogram run that writes a GS triangle strip with
//!   three quadwords per output vertex. The three V4-8 quadwords just before the
//!   positions hold 12 "strip marks": each mark `m` sets the GS no-draw bit on
//!   output vertices `m / 3` and `m / 3 + 1`, which starts a new strip.
//! - Static geometry (`ClipTextureCompress`, `Export_Texture`): V3-32 positions,
//!   V2-32 UVs and V4-8 colours, one output vertex per input vertex.
//! - Skinned geometry (`Texture_Skin`, `Street2_ScrollTexture_Skin`): see
//!   [`skinned_batch`]. Input vertices are scattered into the output strip, so
//!   one vertex can appear in it twice.
//! - Vertex colours use the PS2 scale, where 0x80 means 1.0.
//! - Render methods are listed in named draw groups (see [`draw_groups`]),
//!   which the game shows and hides as a whole.
//! - Batches in any other layout (the `ArbitraryShadow` projected shadows) are
//!   counted in [`ModelStats::skipped_batches`].

use std::collections::HashMap;

use crate::{
    Error, Result, bytes,
    vif::{self, Event, Unpack},
};

const SHT_PROGBITS: u32 = 1;
const SHT_SYMTAB: u32 = 2;
const SHT_REL: u32 = 9;
const DMA_CALL: u32 = 5;
const TEXTURE_REF_MARKER: u32 = 0x0000_000e;

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    /// The draw group the mesh belongs to, e.g. `opaque_riml`, when the model lists groups.
    pub group: Option<String>,
    /// Name of the image in the court's `.ssh` sheets, when the mesh is textured.
    pub texture: Option<String>,
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// PS2 scale: 0x80 is full intensity.
    pub colors: Vec<[u8; 4]>,
    /// Unit normals, when the model has them (player models do; courts and
    /// hoops carry baked lighting instead). Empty or one per position.
    pub normals: Vec<[f32; 3]>,
    /// Skin: up to four skeleton bone indices per vertex and their weights
    /// (unused entries have weight 0). Empty for static geometry, or one per
    /// position.
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    /// Where each vertex came from: (batch number in the model, input vertex
    /// within that batch). Skinned batches write an input vertex to one or
    /// two strip slots, so a source can appear twice. Morph targets address
    /// vertices this way.
    pub sources: Vec<(u32, u16)>,
    /// Triangle list into the vertex arrays.
    pub indices: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModelStats {
    pub chains: usize,
    pub batches: usize,
    /// Batches whose vertex layout this decoder does not handle yet.
    pub skipped_batches: usize,
    pub triangles: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Model {
    pub name: String,
    /// One mesh per draw group and texture, in first-use order.
    pub meshes: Vec<Mesh>,
    pub stats: ModelStats,
    /// The bounds stored in the file (`__BBOX`): min xyz, max xyz.
    pub bbox: Option<([f32; 3], [f32; 3])>,
    /// VU1 microprograms the model is drawn with, e.g. `ClipTextureCompress`.
    pub microcode: Vec<String>,
    /// Input vertices per batch, by batch number (0 for skipped batches).
    pub batch_sizes: Vec<u16>,
    /// Skinned models: the half extents their positions are quantised to.
    pub half_extent: Option<[f32; 3]>,
}

impl Model {
    /// Axis-aligned bounds of every decoded vertex, or `None` when there are none.
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut it = self.meshes.iter().flat_map(|m| m.positions.iter());
        let first = *it.next()?;
        Some(it.fold((first, first), |(lo, hi), p| {
            (
                std::array::from_fn(|i| lo[i].min(p[i])),
                std::array::from_fn(|i| hi[i].max(p[i])),
            )
        }))
    }
}

pub fn parse(bytes: &[u8]) -> Result<Model> {
    let obj = Object::read(bytes)?;
    let data = obj.data;

    let symbol = |prefix: &str| {
        obj.symbols
            .iter()
            .filter(|s| s.section == obj.data_index)
            .find_map(|s| Some((s.name.strip_prefix(prefix)?, s.value as usize)))
    };
    let vec3 = |off: usize| -> Option<[f32; 3]> {
        Some([
            bytes::f32_le(data, off).ok()?,
            bytes::f32_le(data, off + 4).ok()?,
            bytes::f32_le(data, off + 8).ok()?,
        ])
    };
    let name = symbol("__Model:::").map_or("", |(n, _)| n).to_owned();
    let group_of = symbol("__Model:::")
        .and_then(|(_, h)| draw_groups(&obj, h))
        .unwrap_or_default();
    // Skinned positions are quantised to +-half_extent, stored at word 23 of the header.
    let half_extent = symbol("__Model:::").and_then(|(_, h)| vec3(h + 23 * 4));
    let bbox = symbol("__BBOX:::").and_then(|(_, b)| Some((vec3(b)?, vec3(b + 12)?)));
    let mut microcode: Vec<String> = obj
        .symbols
        .iter()
        .filter_map(|s| s.name.strip_suffix("0__EAGLMicroCode"))
        .map(str::to_owned)
        .collect();
    microcode.sort_unstable();
    microcode.dedup();
    let slots: HashMap<u32, String> = obj
        .symbols
        .iter()
        .filter(|s| s.section == obj.data_index)
        .filter_map(|s| {
            Some((
                s.value,
                texture_name(s.name.strip_prefix("__EAGL::TAR:::")?),
            ))
        })
        .collect();

    let mut texture_refs: Vec<(u32, &str)> = obj
        .pointers
        .iter()
        .filter_map(|(&off, target)| {
            let tex = slots.get(target)?;
            let marker = bytes::u32_le(data, (off as usize).checked_sub(4)?).ok()?;
            (marker == TEXTURE_REF_MARKER).then_some((off, tex.as_str()))
        })
        .collect();
    texture_refs.sort_unstable();

    // Chains can CALL shared sub-chains (player bodies do), so not every CALL
    // tag starts a draw. When the model lists draw groups, only the CALLs its
    // render methods use are drawn.
    let calls: Vec<(u32, u32)> = (0..data.len().saturating_sub(15))
        .step_by(16)
        .filter_map(|off| {
            let lo = bytes::u32_le(data, off).ok()?;
            let is_call = (lo >> 28) & 7 == DMA_CALL && lo & 0x0fff_0000 == 0;
            is_call.then(|| {
                obj.pointers
                    .get(&(off as u32 + 4))
                    .map(|&chain| (off as u32, chain))
            })?
        })
        .filter(|(off, _)| group_of.is_empty() || group_of.contains_key(off))
        .collect();

    let mut model = Model {
        name,
        bbox,
        microcode,
        half_extent,
        ..Default::default()
    };
    let mut mesh_of: HashMap<(Option<&str>, Option<String>), usize> = HashMap::new();
    let mut texture: Option<String> = None;
    // The VU keeps the uploaded bones and blend rows until the next upload, and
    // render methods only upload them when they change, so both carry over.
    let mut blend_rows = Vec::new();
    let mut bones = Vec::new();
    for (i, &(call, chain)) in calls.iter().enumerate() {
        let next = calls.get(i + 1).map_or(u32::MAX, |c| c.0);
        if let Some(list) = bone_list(&obj, call, next) {
            bones = list;
        }
        if let Some((_, t)) = texture_refs.iter().find(|(o, _)| *o > call && *o < next) {
            texture = Some((*t).to_owned());
        }
        let group = group_of.get(&call).map(String::as_str);
        let idx = *mesh_of.entry((group, texture.clone())).or_insert_with(|| {
            model.meshes.push(Mesh {
                group: group.map(str::to_owned),
                texture: texture.clone(),
                ..Default::default()
            });
            model.meshes.len() - 1
        });
        model.stats.chains += 1;
        decode_chain(
            &obj,
            chain as usize,
            half_extent,
            Skin {
                rows: &mut blend_rows,
                bones: &bones,
            },
            &mut model.meshes[idx],
            &mut model.stats,
            &mut model.batch_sizes,
        )?;
    }
    model.meshes.retain(|m| !m.indices.is_empty());
    Ok(model)
}

/// The skeleton bones the render method after the CALL tag at `call` uploads
/// for skinning, in VU slot order. The method holds the words `0x0e000000,
/// 0x090000nn, EAGLAnimationBuffer, <pointer>, count, <pointer to count u32
/// bone indices>` (the players') or `0x0e000000, 0x0b0000nn,
/// EAGLAnimationBuffer, count, <pointer to the indices>` (the hoops')
/// somewhere before the next CALL at `next`.
fn bone_list(obj: &Object, call: u32, next: u32) -> Option<Vec<u16>> {
    let word = |off: u32| bytes::u32_le(obj.data, off as usize).ok();
    let end = next.min(call.saturating_add(0x200));
    let (at, form) = (call + 16..end).step_by(4).find_map(|o| {
        let form = word(o + 4)? >> 24;
        (word(o) == Some(0x0e00_0000) && (form == 0x09 || form == 0x0b)).then_some((o, form))
    })?;
    let (count, list) = if form == 0x09 {
        (word(at + 16)?, *obj.pointers.get(&(at + 20))?)
    } else {
        (word(at + 12)?, *obj.pointers.get(&(at + 16))?)
    };
    if count > 64 {
        return None;
    }
    (0..count)
        .map(|k| word(list + 4 * k).map(|b| b as u16))
        .collect()
}

/// The model's draw groups, as a map from each CALL tag's offset to the name
/// of the group that draws it. `__Model` word 39 is the group count, word 40
/// points at the list of name pointers and word 51 at the draw list:
///
/// - a 12-byte header `(0xa0000000, 0, total words)`;
/// - per group, in name order, `(0xa0000001, 0, 2 * n)` and `n` entries of
///   `(0xa000ffff, pointer)`. The pointer leads to a pointer to a record whose
///   first word points at the render packet, 16 bytes after its CALL tag.
///
/// `None` when the header does not have this shape.
fn draw_groups(obj: &Object, header: usize) -> Option<HashMap<u32, String>> {
    let data = obj.data;
    let word = |off: usize| bytes::u32_le(data, off).ok();
    let pointer = |off: usize| obj.pointers.get(&(off as u32)).map(|&p| p as usize);
    let count = word(header + 39 * 4)? as usize;
    if count > 1024 {
        return None;
    }
    let names_at = pointer(header + 40 * 4)?;
    let mut at = pointer(header + 51 * 4)?;
    if word(at)? != 0xa000_0000 {
        return None;
    }
    at += 12;
    let mut group_of = HashMap::new();
    for g in 0..count {
        let name = bytes::cstr(data, pointer(names_at + 4 * g)?).ok()?;
        let entries = word(at + 8)? as usize / 2;
        if word(at)? != 0xa000_0001 || entries > data.len() / 8 {
            return None;
        }
        at += 12;
        for _ in 0..entries {
            if word(at)? != 0xa000_ffff {
                return None;
            }
            let packet = pointer(pointer(pointer(at + 4)?)?)?;
            group_of.insert(packet.checked_sub(16)? as u32, name.clone());
            at += 8;
        }
    }
    Some(group_of)
}

/// One bone of a skeleton, in the file's (parents-first) order.
#[derive(Clone, Debug)]
pub struct Bone {
    /// The part after `Root.` in the `__Bone:::Root.<name>` symbol.
    pub name: String,
    /// Index of the parent in [`Skeleton::bones`]; `None` for the root.
    pub parent: Option<usize>,
    pub scale: [f32; 3],
    /// Local rotation, unit quaternion (x, y, z, w).
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
    /// The id stored with the bone (the fourth word of its translation).
    pub id: u32,
    /// Model space to bone space in the rest pose, column-major (the file's
    /// rows are the columns).
    pub inverse_bind: [f32; 16],
}

#[derive(Clone, Debug, Default)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
}

/// A skeleton file (`<court>bs.o`, the player skeleton in `XF000002`):
/// `__Skeleton:::Root` points at `(hash, 0, bone count, 0)`, followed by 112
/// bytes per bone: scale xyz and parent index, rotation quaternion,
/// translation xyz and an id, then the 4×4 inverse bind matrix with the
/// translation in its last row. `__Bone:::Root.<name>` symbols point at
/// records whose first word is the bone's position in that list.
pub fn parse_skeleton(bytes: &[u8]) -> Result<Skeleton> {
    let obj = Object::read(bytes)?;
    let data = obj.data;
    let at = obj
        .symbols
        .iter()
        .find(|s| s.name.starts_with("__Skeleton:::"))
        .ok_or_else(|| Error::format("no __Skeleton symbol"))?
        .value as usize;
    let count = bytes::u32_le(data, at + 8)? as usize;
    if count > 256 {
        return Err(Error::format(format!("skeleton with {count} bones")));
    }
    let mut names = vec![String::new(); count];
    for s in &obj.symbols {
        if let Some(name) = s.name.strip_prefix("__Bone:::") {
            let i = bytes::u32_le(data, s.value as usize)? as usize;
            if let Some(slot) = names.get_mut(i) {
                *slot = name.strip_prefix("Root.").unwrap_or(name).to_owned();
            }
        }
    }
    let f = |off: usize| bytes::f32_le(data, off);
    let mut bones = Vec::with_capacity(count);
    for (i, name) in names.into_iter().enumerate() {
        let o = at + 16 + 112 * i;
        let parent = bytes::u32_le(data, o + 12)? as i32;
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
            id: bytes::u32_le(data, o + 44)?,
            inverse_bind,
        });
    }
    Ok(Skeleton { bones })
}

/// `tar_crck_14055` -> `crck`.
fn texture_name(slot: &str) -> String {
    let s = slot.strip_prefix("tar_").unwrap_or(slot);
    match s.rsplit_once('_') {
        Some((head, tail)) if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) => {
            head.to_owned()
        }
        _ => s.to_owned(),
    }
}

/// Skinning state for one chain.
struct Skin<'a> {
    /// The blend rows in VU memory: one per blended matrix, each up to three
    /// (bone slot, weight) pairs. They live at absolute VU address 4, so they
    /// persist across batches and chains until the next upload.
    rows: &'a mut Vec<[(u8, f32); 3]>,
    /// Skeleton bone for each VU bone slot, from the chain's render method.
    bones: &'a [u16],
}

/// Blend rows as uploaded: V4-32 floats whose low mantissa byte holds the bone
/// slot times 4 (`Texture_Skin` masks it off with an add/subtract of 768.0).
fn blend_rows(u: &Unpack) -> Vec<[(u8, f32); 3]> {
    u.data
        .as_chunks::<16>()
        .0
        .iter()
        .map(|row| {
            std::array::from_fn(|k| {
                let bits = u32::from_le_bytes(row[4 * k..4 * k + 4].try_into().unwrap());
                ((bits & 0xff) as u8 / 4, f32::from_bits(bits & !0xff))
            })
        })
        .collect()
}

fn decode_chain(
    obj: &Object,
    chain: usize,
    half_extent: Option<[f32; 3]>,
    skin: Skin,
    mesh: &mut Mesh,
    stats: &mut ModelStats,
    batch_sizes: &mut Vec<u16>,
) -> Result<()> {
    let mut vu: HashMap<u16, Unpack> = HashMap::new();
    vif::walk_chain(obj.data, chain, &obj.pointers, &mut |event| match event {
        Event::Unpack(u) if !u.relative => {
            if u.addr == 4 && u.components == 4 && u.bits == 32 {
                *skin.rows = blend_rows(&u);
            }
        }
        Event::Unpack(u) => {
            vu.insert(u.addr, u);
        }
        Event::Execute => {
            let number = stats.batches as u32;
            stats.batches += 1;
            let batch =
                static_batch(&vu).or_else(|| half_extent.and_then(|h| skinned_batch(&vu, h)));
            match batch {
                Some(batch) => {
                    batch_sizes.push(batch.input_count as u16);
                    stats.triangles += append(mesh, &batch, number, skin.rows, skin.bones);
                }
                None => {
                    batch_sizes.push(0);
                    stats.skipped_batches += 1;
                }
            }
            vu.clear();
        }
    })
}

/// One microprogram run, as the strip the GS draws.
struct Batch {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[u8; 4]>,
    normals: Option<Vec<[f32; 3]>>,
    /// Skinned geometry: the blended matrix (blend row) each vertex uses.
    blend: Option<Vec<u8>>,
    /// The GS no-draw (ADC) bit per vertex: no triangle ends on a vertex that has it.
    restart: Vec<bool>,
    /// The input vertex each output vertex was written from.
    inputs: Vec<u16>,
    input_count: usize,
}

/// No-draw bits for an `n`-vertex strip from the 12 strip marks: the three
/// single-quadword V4-8 unpacks below `positions_at` (VU 0x14..=0x16 for static
/// geometry, 0x09..=0x0b or 0x0a..=0x0c for hoops, 0x01..=0x03 for players).
/// `None` when they are not exactly three or a mark is not a whole vertex.
fn restart_bits(vu: &HashMap<u16, Unpack>, positions_at: u16, n: usize) -> Option<Vec<bool>> {
    let mut restart = vec![false; n];
    // Unused marks are 0, which lands on the first two vertices; those never draw anyway.
    restart.iter_mut().take(2).for_each(|r| *r = true);
    let marks: Vec<&Unpack> = vu
        .iter()
        .filter(|&(&a, u)| a < positions_at && u.components == 4 && u.bits == 8 && u.count == 1)
        .map(|(_, u)| u)
        .collect();
    if marks.len() != 3 {
        return None;
    }
    for u in marks {
        for &m in u.data.iter().take(4) {
            if m % 3 != 0 {
                return None;
            }
            restart
                .iter_mut()
                .skip(m as usize / 3)
                .take(2)
                .for_each(|r| *r = true);
        }
    }
    Some(restart)
}

/// Static geometry: V3-32 positions, V2-32 UVs and V4-8 colours, one output
/// vertex per input vertex. `ClipTextureCompress` stores them at VU 0x17, 0x5f
/// and 0xa7, `Export_Texture` at 0x17, 0x67 and 0xb7.
fn static_batch(vu: &HashMap<u16, Unpack>) -> Option<Batch> {
    let (&at, pos) = vu.iter().find(|(_, u)| u.components == 3 && u.bits == 32)?;
    let n = pos.count;
    let uv = vu
        .values()
        .find(|u| u.components == 2 && u.bits == 32 && u.count == n)?;
    let col = vu
        .iter()
        .find(|&(&a, u)| a > at && u.components == 4 && u.bits == 8 && u.unsigned && u.count == n)?
        .1;
    // Offset mode adds STROW to the raw words; the decoder only knows the zero-row case.
    if pos.offset_row.is_some_and(|r| r[..3] != [0; 3]) {
        return None;
    }
    let f = |d: &[u8], i: usize| bytes::f32_le(d, i * 4).ok();
    let positions = (0..n)
        .map(|i| {
            Some([
                f(pos.data, 3 * i)?,
                f(pos.data, 3 * i + 1)?,
                f(pos.data, 3 * i + 2)?,
            ])
        })
        .collect::<Option<Vec<_>>>()?;
    let uvs = (0..n)
        .map(|i| Some([f(uv.data, 2 * i)?, f(uv.data, 2 * i + 1)?]))
        .collect::<Option<Vec<_>>>()?;
    let colors = col
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .take(n)
        .map(|c| [c[0], c[1], c[2], c[3]])
        .collect::<Vec<_>>();
    Some(Batch {
        positions,
        uvs,
        colors,
        normals: None,
        blend: None,
        restart: restart_bits(vu, at, n)?,
        inputs: (0..n as u16).collect(),
        input_count: n,
    })
}

/// Skinned geometry, as the `Texture_Skin`, `Street2_ScrollTexture_Skin`
/// (hoops) and `Texture_ToonShade_Skin`/`Texture_ToonShade2_Skin` (players)
/// microprograms read it. The arrays sit one stride apart (60 quadwords for the
/// hoops, 36 for players). With the positions at VU address P:
///
/// - P: V4-16 unsigned in offset mode (STROW = 1.0). xyz is quantised to
///   `(raw - 32768) / 32768 * half_extent`; w selects a blended bone matrix
///   (0, 7, 14 or 21; 0xffff keeps the previous one).
/// - P + stride: V4-16 signed. xy is the UV in 4.12 fixed point; z and w are
///   the output slots (in quadwords, three per vertex) the vertex is written to.
///   Equal values write it once.
/// - Hoops: P + 2 strides holds V4-8 unsigned colours. Players have V4-32
///   normals there instead (float xyz, a bone selector in w; their colour
///   comes from toon lighting) and get neutral 0x80 colours here.
/// - VU 0: V4-32 header whose x is the number of output vertices.
///
/// Without bone matrices the vertices are in the model's rest pose.
fn skinned_batch(vu: &HashMap<u16, Unpack>, half_extent: [f32; 3]) -> Option<Batch> {
    // STROW is set once, in a model's first chain, and the VIF keeps it for the
    // rest; the value does not matter here since the raw words are dequantised directly.
    let (&at, pos) = vu
        .iter()
        .find(|(_, u)| u.components == 4 && u.bits == 16 && u.unsigned && u.offset_row.is_some())?;
    let n = pos.count;
    let after = |components: u8, bits: u8| {
        vu.iter()
            .find(|&(&a, u)| a > at && u.components == components && u.bits == bits && u.count == n)
            .map(|(_, u)| u)
    };
    let uv = after(4, 16).filter(|u| !u.unsigned)?;
    let col = after(4, 8);
    let nrm = after(4, 32);
    let out_n = vu
        .get(&0)
        .filter(|u| u.components == 4 && u.bits == 32)
        .and_then(|u| bytes::u32_le(u.data, 0).ok())? as usize;
    if out_n > 256 {
        return None;
    }

    let half = |d: &[u8], i: usize| bytes::u16_le(d, i * 2).ok();
    let mut slots: Vec<Option<usize>> = vec![None; out_n];
    for i in 0..n {
        for k in [2, 3] {
            let q = half(uv.data, 4 * i + k)? as usize;
            if !q.is_multiple_of(3) || q / 3 >= out_n {
                return None;
            }
            slots[q / 3] = Some(i);
        }
    }
    let slots = slots.into_iter().collect::<Option<Vec<_>>>()?;
    // Position w picks the blended matrix by its quadword offset (7 per matrix);
    // 0xffff keeps the one in use, in input-vertex order.
    let mut selector = Vec::with_capacity(n);
    let mut current = 0u8;
    for i in 0..n {
        let w = half(pos.data, 4 * i + 3)?;
        if w != 0xffff {
            current = (w / 7) as u8;
        }
        selector.push(current);
    }

    let mut positions = Vec::with_capacity(out_n);
    let mut uvs = Vec::with_capacity(out_n);
    let mut colors = Vec::with_capacity(out_n);
    let mut normals = Vec::with_capacity(if nrm.is_some() { out_n } else { 0 });
    let blend = slots.iter().map(|&i| selector[i]).collect();
    let inputs = slots.iter().map(|&i| i as u16).collect();
    for i in slots {
        let mut p = [0.0; 3];
        for (k, v) in p.iter_mut().enumerate() {
            let raw = half(pos.data, 4 * i + k)? as f32;
            *v = (raw - 32768.0) / 32768.0 * half_extent[k];
        }
        positions.push(p);
        let st = |k| half(uv.data, 4 * i + k).map(|v| v as i16 as f32 / 4096.0);
        uvs.push([st(0)?, st(1)?]);
        colors.push(match col {
            Some(col) => {
                let c = bytes::range(col.data, 4 * i, 4).ok()?;
                [c[0], c[1], c[2], c[3]]
            }
            None => [0x80; 4],
        });
        if let Some(nrm) = nrm {
            let f = |k| bytes::f32_le(nrm.data, (4 * i + k) * 4).ok();
            normals.push([f(0)?, f(1)?, f(2)?]);
        }
    }
    Some(Batch {
        positions,
        uvs,
        colors,
        normals: nrm.is_some().then_some(normals),
        blend: Some(blend),
        restart: restart_bits(vu, at, out_n)?,
        inputs,
        input_count: n,
    })
}

/// How far triangle `a, b, c` (counter-clockwise) faces along its vertex
/// normals: positive when they agree.
fn facing(a: [f32; 3], b: [f32; 3], c: [f32; 3], normals: [[f32; 3]; 3]) -> f32 {
    let u = std::array::from_fn::<f32, 3, _>(|k| b[k] - a[k]);
    let v = std::array::from_fn::<f32, 3, _>(|k| c[k] - a[k]);
    let face = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    (0..3)
        .map(|k| face[k] * (normals[0][k] + normals[1][k] + normals[2][k]))
        .sum()
}

/// Append a batch's vertices and its strips as a triangle list; returns triangles added.
fn append(
    mesh: &mut Mesh,
    b: &Batch,
    number: u32,
    rows: &[[(u8, f32); 3]],
    bones: &[u16],
) -> usize {
    let base = mesh.positions.len() as u32;
    mesh.sources.extend(b.inputs.iter().map(|&i| (number, i)));
    if let (Some(blend), false) = (&b.blend, bones.is_empty()) {
        mesh.joints.resize(base as usize, [0; 4]);
        mesh.weights.resize(base as usize, [1.0, 0.0, 0.0, 0.0]);
        for &j in blend {
            let mut joints = [0u16; 4];
            let mut weights = [0f32; 4];
            for (k, &(slot, weight)) in rows.get(j as usize).into_iter().flatten().enumerate() {
                if weight > 0.0 {
                    joints[k] = bones.get(slot as usize).copied().unwrap_or(0);
                    weights[k] = weight;
                }
            }
            let total: f32 = weights.iter().sum();
            if total > 0.0 {
                weights.iter_mut().for_each(|w| *w /= total);
            } else {
                weights[0] = 1.0;
            }
            mesh.joints.push(joints);
            mesh.weights.push(weights);
        }
    } else if !mesh.joints.is_empty() {
        mesh.joints
            .resize(base as usize + b.positions.len(), [0; 4]);
        mesh.weights
            .resize(base as usize + b.positions.len(), [1.0, 0.0, 0.0, 0.0]);
    }
    mesh.positions.extend(&b.positions);
    mesh.uvs.extend(&b.uvs);
    mesh.colors.extend(&b.colors);
    if let Some(normals) = &b.normals {
        // Keep one normal per position if earlier batches in the mesh had none.
        mesh.normals.resize(base as usize, [0.0, 1.0, 0.0]);
        mesh.normals.extend(normals);
    } else if !mesh.normals.is_empty() {
        mesh.normals.resize(mesh.positions.len(), [0.0, 1.0, 0.0]);
    }
    let p = &b.positions;
    let mut added = 0;
    // The vertex after the last no-draw one starts the current strip's first triangle.
    let mut last_restart = 0;
    for k in 0..p.len() {
        if b.restart[k] {
            last_restart = k;
            continue;
        }
        if k < 2 {
            continue;
        }
        // Strips alternate winding; swap the first two on odd triangles to keep it consistent.
        let (a, c) = if (k - last_restart - 1) % 2 == 0 {
            (k - 2, k - 1)
        } else {
            (k - 1, k - 2)
        };
        if p[a] == p[c] || p[c] == p[k] || p[a] == p[k] {
            continue; // degenerate, used to stitch strips together
        }
        // The GS does not cull, so strips start with either winding. When the
        // vertices have normals, wind each triangle counter-clockwise about them.
        let (a, c) = match &b.normals {
            Some(n) if facing(p[a], p[c], p[k], [n[a], n[c], n[k]]) < 0.0 => (c, a),
            _ => (a, c),
        };
        mesh.indices
            .extend([base + a as u32, base + c as u32, base + k as u32]);
        added += 1;
    }
    added
}

pub(crate) struct Symbol {
    pub name: String,
    pub value: u32,
    pub section: u16,
}

/// The parts of an ELF32 little-endian relocatable object EAGL uses.
pub(crate) struct Object<'a> {
    pub data: &'a [u8],
    pub data_index: u16,
    pub symbols: Vec<Symbol>,
    /// Relocated words inside `.data` that point back into `.data`: offset -> target.
    pub pointers: HashMap<u32, u32>,
    /// Relocated words inside `.data` that name a symbol defined elsewhere
    /// (a texture slot, a microprogram, the animation buffer): offset ->
    /// index into `symbols`.
    pub externals: HashMap<u32, usize>,
}

impl<'a> Object<'a> {
    pub(crate) fn read(b: &'a [u8]) -> Result<Self> {
        if bytes::range(b, 0, 6)? != b"\x7fELF\x01\x01" {
            return Err(Error::format("not a 32-bit little-endian ELF object"));
        }
        let shoff = bytes::u32_le(b, 0x20)? as usize;
        let shentsize = bytes::u16_le(b, 0x2e)? as usize;
        let shnum = bytes::u16_le(b, 0x30)? as usize;
        let shstrndx = bytes::u16_le(b, 0x32)? as usize;
        if shentsize != 40 || shnum > 256 || shstrndx >= shnum {
            return Err(Error::format("unexpected ELF section header layout"));
        }
        struct Sh {
            name: u32,
            kind: u32,
            offset: usize,
            size: usize,
            link: usize,
            info: u32,
        }
        let sh = (0..shnum)
            .map(|i| {
                let h = shoff + i * 40;
                Ok(Sh {
                    name: bytes::u32_le(b, h)?,
                    kind: bytes::u32_le(b, h + 4)?,
                    offset: bytes::u32_le(b, h + 16)? as usize,
                    size: bytes::u32_le(b, h + 20)? as usize,
                    link: bytes::u32_le(b, h + 24)? as usize,
                    info: bytes::u32_le(b, h + 28)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let names = sh[shstrndx].offset;
        let data_index = sh
            .iter()
            .position(|s| {
                s.kind == SHT_PROGBITS
                    && bytes::cstr(b, names + s.name as usize).is_ok_and(|n| n == ".data")
            })
            .ok_or_else(|| Error::format("ELF object has no .data section"))?;
        let data = bytes::range(b, sh[data_index].offset, sh[data_index].size)?;

        let mut symbols = Vec::new();
        for s in sh.iter().filter(|s| s.kind == SHT_SYMTAB) {
            let strtab = sh
                .get(s.link)
                .ok_or_else(|| Error::format("symbol table without strings"))?
                .offset;
            for i in 0..s.size / 16 {
                let e = s.offset + i * 16;
                symbols.push(Symbol {
                    name: bytes::cstr(b, strtab + bytes::u32_le(b, e)? as usize)?,
                    value: bytes::u32_le(b, e + 4)?,
                    section: bytes::u16_le(b, e + 14)?,
                });
            }
        }

        let mut pointers = HashMap::new();
        let mut externals = HashMap::new();
        for s in sh
            .iter()
            .filter(|s| s.kind == SHT_REL && s.info as usize == data_index)
        {
            for i in 0..s.size / 8 {
                let off = bytes::u32_le(b, s.offset + i * 8)?;
                let sym = (bytes::u32_le(b, s.offset + i * 8 + 4)? >> 8) as usize;
                let target = symbols
                    .get(sym)
                    .ok_or_else(|| Error::format("relocation names a missing symbol"))?;
                if target.section as usize == data_index {
                    let addend = bytes::u32_le(data, off as usize)?;
                    pointers.insert(off, target.value.wrapping_add(addend));
                } else if target.section == 0 {
                    externals.insert(off, sym);
                }
            }
        }
        Ok(Self {
            data,
            data_index: data_index as u16,
            symbols,
            pointers,
            externals,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_names_drop_prefix_and_serial() {
        assert_eq!(texture_name("tar_crck_14055"), "crck");
        assert_eq!(texture_name("tar_gras_3248"), "gras");
        assert_eq!(texture_name("tar_odd_name"), "odd_name");
    }

    #[test]
    fn strips_become_consistently_wound_triangles() {
        let p = |x: f32, z: f32| [x, 0.0, z];
        let batch = Batch {
            // two strips of 4 vertices: a quad each
            positions: vec![
                p(0., 0.),
                p(1., 0.),
                p(0., 1.),
                p(1., 1.),
                p(5., 5.),
                p(6., 5.),
                p(5., 6.),
                p(6., 6.),
            ],
            uvs: vec![[0.0; 2]; 8],
            colors: vec![[0x80; 4]; 8],
            normals: None,
            blend: None,
            restart: [true, true, false, false].repeat(2),
            inputs: (0..8).collect(),
            input_count: 8,
        };
        let mut mesh = Mesh::default();
        assert_eq!(append(&mut mesh, &batch, 0, &[], &[]), 4);
        // no triangle bridges the two strips
        assert!(
            mesh.indices
                .chunks(3)
                .all(|t| t.iter().all(|&i| i < 4) || t.iter().all(|&i| i >= 4))
        );
        assert_eq!(&mesh.indices[..6], &[0, 1, 2, 2, 1, 3]);
    }

    fn unpack(addr: u16, components: u8, bits: u8, unsigned: bool, data: &[u8]) -> Unpack<'_> {
        let vector_bytes = components as usize * bits as usize / 8;
        Unpack {
            addr,
            components,
            bits,
            count: data.len() / vector_bytes,
            unsigned,
            relative: true,
            offset_row: None,
            data,
        }
    }

    fn halves(v: &[u16]) -> Vec<u8> {
        v.iter().flat_map(|h| h.to_le_bytes()).collect()
    }

    #[test]
    fn skinned_vertices_are_scattered_into_output_slots() {
        // Four vertices; vertex 0 is also written to output slot 4 (quadword 12).
        let pos = halves(&[
            32768, 32768, 32768, 0, //
            65535, 32768, 32768, 0xffff, //
            32768, 49152, 32768, 0xffff, //
            65535, 49152, 32768, 0xffff,
        ]);
        let uv = halves(&[0, 0, 0, 12, 4096, 0, 3, 3, 0, 4096, 6, 6, 4096, 4096, 9, 9]);
        let col = [0x80u8; 16];
        let header: Vec<u8> = [5u32, 4, 1, 1]
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect();
        let marks = [0u8; 4];
        let mut vu = HashMap::new();
        vu.insert(0, unpack(0, 4, 32, true, &header));
        for a in 9..=11 {
            vu.insert(a, unpack(a, 4, 8, true, &marks));
        }
        let mut p = unpack(0x0c, 4, 16, true, &pos);
        p.offset_row = Some([1f32.to_bits(); 4]);
        vu.insert(0x0c, p);
        vu.insert(0x48, unpack(0x48, 4, 16, false, &uv));
        vu.insert(0x84, unpack(0x84, 4, 8, true, &col));

        let b = skinned_batch(&vu, [2.0, 4.0, 1.0]).expect("skinned layout");
        assert_eq!(b.positions.len(), 5);
        assert_eq!(b.positions[0], [0.0, 0.0, 0.0]);
        assert!((b.positions[1][0] - 2.0).abs() < 1e-3); // raw 65535 is just under +half_extent
        assert_eq!(b.positions[2], [0.0, 2.0, 0.0]);
        assert_eq!(b.positions[4], b.positions[0]);
        assert_eq!(b.uvs[1], [1.0, 0.0]);
        assert_eq!(b.restart, [true, true, false, false, false]);

        // A slot no vertex writes leaves the batch undecodable.
        let header: Vec<u8> = [6u32, 4, 1, 1]
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect();
        vu.insert(0, unpack(0, 4, 32, true, &header));
        assert!(skinned_batch(&vu, [1.0; 3]).is_none());
    }

    #[test]
    fn strip_marks_restart_two_vertices_each() {
        let marks = [6u8, 15, 0, 0];
        let zero = [0u8; 4];
        let mut vu = HashMap::new();
        vu.insert(0x14, unpack(0x14, 4, 8, true, &marks));
        vu.insert(0x15, unpack(0x15, 4, 8, true, &zero));
        vu.insert(0x16, unpack(0x16, 4, 8, true, &zero));
        let r = restart_bits(&vu, 0x17, 8).unwrap();
        assert_eq!(r, [true, true, true, true, false, true, true, false]);
        // A mark that is not a whole vertex (multiple of 3 quadwords) is rejected.
        let bad = [7u8, 0, 0, 0];
        vu.insert(0x14, unpack(0x14, 4, 8, true, &bad));
        assert!(restart_bits(&vu, 0x17, 8).is_none());
    }
}
