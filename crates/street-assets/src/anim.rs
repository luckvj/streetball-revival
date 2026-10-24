//! EAGL animation banks (`/ANIM/*.BIN`): ELF objects with one
//! `__AnimationBank:::<name>` symbol, read from the decoders in `SLUS_206.51`.
//!
//! The bank symbol points at `(0x18, count, 0, <records>, <names>)`: `count`
//! record pointers and as many name pointers. Each animation is stored as
//! three records named `<anim>_X` (state and events, `FnRawStateChan`),
//! `<anim>_r` (rotations) and `<anim>_t` (translations). A record is an
//! `FnCompoundChannel` (type 15) whose fourth word points at its channel.
//! Channel objects start with a u16 type that the game's factory (0x387180)
//! maps to a class: 18 `FnDeltaQFast`, 19 `FnDeltaSingleQ`, 20 `FnDeltaF3`,
//! 21 `FnDeltaF1`, 16 `FnRawStateChan`, 0 `FnRawPoseChannel`, among others.
//!
//! Time is measured in frames.
//!
//! Street V3's GameCube banks (`/xf/anims.viv`) are the same objects
//! with a big-endian payload, a 28-byte bank header (`0x1c, count, 0, 0,
//! <records>, <names>`), per-animation channel flags instead of `0xc016`,
//! and no `_X` state records. [`Bank::parse`] reads both.

use std::collections::BTreeMap;

use crate::{Error, Result, bytes, eagl};

const COMPOUND: u16 = 15;
const DELTA_Q_FAST: u16 = 18;
const DELTA_SINGLE_Q: u16 = 19;
const DELTA_QUAT: u16 = 11;
const RAW_POSE: u16 = 0;
const RAW_STATE: u16 = 16;
/// The `flags` word every delta channel on the disc carries.
const CHANNEL_FLAGS: u16 = 0xc016;
const DELTA_F3: u16 = 20;
const DELTA_F1: u16 = 21;

/// Translation channels write into a small float array; slots 8, 9 and 10
/// are the root's x, y and z (every `_t` track on the disc maps there).
const ROOT_TRANSLATION_SLOT: usize = 8;

/// `FUN_0019bde0` converts the evaluated animation's three root translation
/// components into gameplay units with this exact multiplier.
pub const TRANSLATION_SCALE: f32 = 0.325_732_92;

/// One animation's rotation track: a local rotation per animated bone and frame.
#[derive(Clone, Debug)]
pub struct Rotations {
    /// Skeleton bone (list position) of each channel.
    pub bones: Vec<u8>,
    /// `frames[f][c]`: unit quaternion (x, y, z, w) of channel `c` at frame `f`.
    pub frames: Vec<Vec<[f32; 4]>>,
}

/// Native integer-frame quaternion values before renderer normalization.
/// The game's delta channels deliberately retain small norm deviations;
/// gameplay's q*v*conj(q) transform must preserve them.
#[derive(Clone, Debug)]
pub struct NativeRotations {
    pub bones: Vec<u8>,
    pub frames: Vec<Vec<[f32; 4]>>,
    pub interpolation: NativeInterpolation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeInterpolation {
    Components,
    /// RawPose's fractional reader uses normalized shortest-sign weighted
    /// components (00392ef8 -> 00379b20), rather than QFast's linear reader.
    RawPose,
}

/// One animation's root translation per frame, in animation units (see
/// [`TRANSLATION_SCALE`]), relative to the floor below the player.
#[derive(Clone, Debug)]
pub struct RootMotion {
    pub frames: Vec<[f32; 3]>,
}

pub struct Bank {
    name: String,
    data: Vec<u8>,
    /// GameCube (V3) banks are big-endian.
    big_endian: bool,
    pointers: std::collections::HashMap<u32, u32>,
    /// Record name -> record offset in `.data`.
    records: BTreeMap<String, u32>,
}

impl Bank {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let obj = eagl::Object::read(bytes)?;
        let (name, at) = obj
            .symbols
            .iter()
            .find_map(|s| Some((s.name.strip_prefix("__AnimationBank:::")?, s.value)))
            .ok_or_else(|| Error::format("no __AnimationBank symbol"))?;
        let data = obj.data;
        let pointer = |off: u32| {
            obj.pointers
                .get(&off)
                .copied()
                .ok_or_else(|| Error::format(format!("no pointer at {off:#x}")))
        };
        // The header's first word is its size: 0x18 on the PS2, 0x1c on the
        // GameCube, whose words are big-endian.
        let big_endian = bytes::u32_le(data, at as usize)? != 0x18;
        let d = Data {
            b: data,
            be: big_endian,
        };
        let head = d.u32(at as usize)?;
        if !matches!(head, 0x18 | 0x1c) {
            return Err(Error::format(format!("animation bank header {head:#x}")));
        }
        let count = d.u32(at as usize + 4)?;
        let (list, names) = (pointer(at + head - 12)?, pointer(at + head - 8)?);
        let mut records = BTreeMap::new();
        for k in 0..count {
            let name = bytes::cstr(data, pointer(names + 4 * k)? as usize)?;
            records.insert(name, pointer(list + 4 * k)?);
        }
        Ok(Self {
            name: name.to_owned(),
            data: data.to_vec(),
            big_endian,
            pointers: obj.pointers,
            records,
        })
    }

    /// Whether this is a GameCube (big-endian) bank.
    pub fn is_big_endian(&self) -> bool {
        self.big_endian
    }

    fn d(&self) -> Data<'_> {
        Data {
            b: &self.data,
            be: self.big_endian,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Animation names (the record names without their `_X`/`_r`/`_t` suffix).
    pub fn animations(&self) -> impl Iterator<Item = &str> {
        self.records.keys().filter_map(|n| n.strip_suffix("_r"))
    }

    /// The channel of record `<animation><suffix>` and its type.
    fn channel(&self, animation: &str, suffix: &str) -> Result<Option<(usize, u16)>> {
        let name = format!("{animation}{suffix}");
        let record = *self
            .records
            .get(&name)
            .ok_or_else(|| Error::NotFound(format!("animation record {name}")))?;
        if self.d().u16(record as usize)? != COMPOUND {
            return Ok(None);
        }
        let channel = *self
            .pointers
            .get(&(record + 12))
            .ok_or_else(|| Error::format(format!("{name} has no channel")))?
            as usize;
        Ok(Some((channel, self.d().u16(channel)?)))
    }

    /// The rig an animation is made for: the flags word of its rotation
    /// record, which is the top half of the target skeleton's hash (`0xc016`
    /// on Vol. 2's disc; on V3's, `0xd423` for the Nintendo skeletons, whose
    /// files start with `d4230096`, and other values for the body rigs).
    pub fn rig(&self, animation: &str) -> Result<u16> {
        let name = format!("{animation}_r");
        let record = *self
            .records
            .get(&name)
            .ok_or_else(|| Error::NotFound(format!("animation record {name}")))?;
        self.d().u16(record as usize + 2)
    }

    /// Per-integer-frame runtime flags from `<animation>_X`. The native
    /// raw-state evaluator is FUN_00389310/00389190; FUN_00191c08 packs
    /// its fifteen output words into the flags consumed by gameplay.
    pub fn states(&self, animation: &str) -> Result<Option<Vec<u32>>> {
        if self.big_endian || !self.records.contains_key(&format!("{animation}_X")) {
            return Ok(None);
        }
        let Some((at, kind)) = self.channel(animation, "_X")? else {
            return Ok(None);
        };
        if kind != RAW_STATE {
            return Ok(None);
        }
        raw_states(&self.data, at).map(Some)
    }

    /// The rotation track of `animation`, or `Ok(None)` when it is stored in a
    /// channel type this decoder does not handle yet.
    ///
    /// A rotation record holds one or two channels (the count is the u16 at
    /// record + 8). They are stored back to back: typically a
    /// `FnDeltaSingleQ` for hinge bones (the knees) and an `FnDeltaQFast` for
    /// the rest. Their bones are merged into one track.
    pub fn rotations(&self, animation: &str) -> Result<Option<Rotations>> {
        self.rotations_with_normalization(animation, true)
    }

    /// Original QFast, DeltaQuat and quaternion RawPose integer-frame values for authoritative
    /// rig sampling. Unsupported channel formats return None instead of
    /// substituting normalized renderer data.
    pub fn native_rotations(&self, animation: &str) -> Result<Option<NativeRotations>> {
        let interpolation = match self.channel(animation, "_r")? {
            Some((_, RAW_POSE)) => NativeInterpolation::RawPose,
            _ => NativeInterpolation::Components,
        };
        Ok(self
            .rotations_with_normalization(animation, false)?
            .map(|r| NativeRotations {
                bones: r.bones,
                frames: r.frames,
                interpolation,
            }))
    }

    fn rotations_with_normalization(
        &self,
        animation: &str,
        normalize: bool,
    ) -> Result<Option<Rotations>> {
        let Some((mut at, _)) = self.channel(animation, "_r")? else {
            return Ok(None);
        };
        let record = self.records[&format!("{animation}_r")] as usize;
        let count = self.d().u16(record + 8)?;
        let flags = self.d().u16(at + 2)?;
        let mut merged: Option<Rotations> = None;
        for k in 0..count {
            if k > 0 {
                at = self.next_channel(at, flags)?;
            }
            let (part, end) = match self.d().u16(at)? {
                DELTA_Q_FAST => delta_q_fast(self.d(), &self.pointers, at, normalize)?,
                DELTA_SINGLE_Q if normalize => delta_single_q(self.d(), &self.pointers, at)?,
                DELTA_QUAT => delta_quat(self.d(), &self.pointers, at, normalize)?,
                RAW_POSE => {
                    let Some(part) = raw_pose(self.d(), at, normalize)? else {
                        return Ok(None);
                    };
                    part
                }
                _ => return Ok(None),
            };
            merged = Some(match merged {
                None => part,
                Some(mut m) => {
                    if m.frames.len() != part.frames.len() {
                        return Err(Error::format(format!(
                            "{animation}_r: channels with different lengths"
                        )));
                    }
                    m.bones.extend(part.bones);
                    for (a, b) in m.frames.iter_mut().zip(part.frames) {
                        a.extend(b);
                    }
                    m
                }
            });
            at = end;
        }
        Ok(merged)
    }

    /// The channel following one that ends at `end`: the first 2-aligned
    /// offset after it with a delta-channel header (type 18 or 19, the usual
    /// flags or the first channel's, and a relocated bone-map pointer at
    /// +12). Most start at the next 4-byte boundary, a few later.
    fn next_channel(&self, end: usize, flags: u16) -> Result<usize> {
        let data = self.d();
        (end + (end & 1)..end + 64)
            .step_by(2)
            .find(|&at| {
                matches!(data.u16(at), Ok(DELTA_Q_FAST | DELTA_SINGLE_Q))
                    && data
                        .u16(at + 2)
                        .is_ok_and(|f| f == CHANNEL_FLAGS || f == flags)
                    && self.pointers.contains_key(&(at as u32 + 12))
            })
            .ok_or_else(|| Error::format(format!("no second channel after {end:#x}")))
    }

    /// The root translation track of `animation`, or `Ok(None)` when it is
    /// stored in a channel type this decoder does not handle yet.
    pub fn root_motion(&self, animation: &str) -> Result<Option<RootMotion>> {
        let (at, dims) = match self.channel(animation, "_t")? {
            Some((at, DELTA_F3)) => (at, 3),
            Some((at, DELTA_F1)) => (at, 1),
            _ => return Ok(None),
        };
        let (map, values) = delta_floats(self.d(), &self.pointers, at, dims)?;
        let mut frames = vec![[0f32; 3]; values.len()];
        for (frame, out) in values.iter().zip(&mut frames) {
            for (c, &slot) in map.iter().enumerate() {
                for k in 0..dims {
                    let slot = slot as usize + k;
                    match slot.checked_sub(ROOT_TRANSLATION_SLOT) {
                        Some(t) if t < 3 => out[t] = frame[c * dims + k],
                        _ => {
                            return Err(Error::format(format!(
                                "{animation}_t writes float slot {slot}"
                            )));
                        }
                    }
                }
            }
        }
        Ok(Some(RootMotion { frames }))
    }
}

fn raw_states(data: &[u8], at: usize) -> Result<Vec<u32>> {
    let frames = usize::from(bytes::u16_le(data, at + 4)?);
    let keys = usize::from(bytes::u16_le(data, at + 6)?);
    let header = bytes::range(data, at + 8, 2)?;
    let fields = usize::from(header[0]);
    let stride = usize::from(header[1]);
    if frames == 0 || keys == 0 || fields == 0 || fields > 15 || stride < 4 {
        return Err(Error::format("invalid FnRawStateChan header"));
    }
    let descriptors = (0..fields)
        .map(|index| bytes::u16_le(data, at + 10 + index * 2))
        .collect::<Result<Vec<_>>>()?;
    let key_base = (at + 10 + fields * 2 + 3) & !3;
    bytes::range(data, key_base, keys * stride)?;
    let mut decoded = Vec::with_capacity(keys);
    let mut last_time = f32::NEG_INFINITY;
    for key in 0..keys {
        let start = key_base + key * stride;
        let time = bytes::f32_le(data, start)?;
        if !time.is_finite() || time < last_time {
            return Err(Error::format("FnRawStateChan key times are not ordered"));
        }
        last_time = time;
        let stream = bytes::range(data, start + 4, stride - 4)?;
        let mut position = 0;
        let mut bits = 0;
        let mut output = [0u8; 60];
        for &descriptor in &descriptors {
            let value = match descriptor >> 13 {
                kind @ 0..=2 => {
                    let width = 1 << kind;
                    if bits + width > 8 {
                        return Err(Error::format("FnRawStateChan bit field crosses a byte"));
                    }
                    let byte = *stream
                        .get(position)
                        .ok_or_else(|| Error::format("truncated FnRawStateChan bits"))?;
                    bits += width;
                    let value = u32::from(byte >> (8 - bits)) & ((1 << width) - 1);
                    if bits == 8 {
                        position += 1;
                        bits = 0;
                    }
                    value
                }
                3 => {
                    let value = u32::from(
                        *stream
                            .get(position)
                            .ok_or_else(|| Error::format("truncated FnRawStateChan byte"))?,
                    );
                    position += 1;
                    value
                }
                4 => {
                    let value = u32::from(bytes::u16_le(stream, position)?);
                    position += 2;
                    value
                }
                5 => {
                    let value = bytes::u32_le(stream, position)?;
                    position += 4;
                    value
                }
                _ => return Err(Error::format("unsupported FnRawStateChan source width")),
            };
            let size = usize::from((descriptor >> 11 & 3) + 1);
            if size == 3 {
                return Err(Error::format("invalid FnRawStateChan output width"));
            }
            let destination = usize::from(descriptor & 0xff);
            let target = output
                .get_mut(destination..destination + size)
                .ok_or_else(|| Error::format("FnRawStateChan output outside state record"))?;
            target.copy_from_slice(&value.to_le_bytes()[..size]);
        }
        let words: [u32; 15] = std::array::from_fn(|index| {
            u32::from_le_bytes(output[index * 4..index * 4 + 4].try_into().unwrap())
        });
        let booleans = words[3..]
            .iter()
            .enumerate()
            .fold(0u32, |flags, (index, &value)| {
                flags | (u32::from(value != 0) << index)
            });
        let flags = booleans | (words[1] << 12) | (words[0] << 15) | (words[2] << 18);
        decoded.push((time, flags));
    }
    Ok((0..frames)
        .map(|frame| {
            let key = decoded
                .partition_point(|&(time, _)| time <= frame as f32)
                .saturating_sub(1);
            decoded[key].1
        })
        .collect())
}

/// `FnDeltaQuatChan` (type 11, vtable 0x4af440, evaluate 0x3943e8):
/// header u16 type/flags, relocated delta-stream pointer, u16 frame count,
/// then one u16 destination float slot per quaternion. A transform has 12
/// float slots, with its rotation starting at slot 4.
///
/// The stream decoder (0x395508) reads u16 component count, u16 delta bits,
/// then `(delta minimum, delta scale, initial value)` f32 triples. Frame 0
/// uses the initial values; each later frame adds `minimum + code * scale`.
/// Codes are unsigned 4 (low nibble first), 8, or 16 bits. Unlike QFast,
/// this stream has no periodic key blocks. All four disc tracks use 16 bits.
fn delta_quat<'a>(
    data: impl Into<Data<'a>>,
    pointers: &std::collections::HashMap<u32, u32>,
    at: usize,
    normalize: bool,
) -> Result<(Rotations, usize)> {
    let data: Data = data.into();
    let stream = *pointers
        .get(&(at as u32 + 4))
        .ok_or_else(|| Error::format("FnDeltaQuatChan without a delta stream"))?
        as usize;
    let values = data.u16(stream)? as usize;
    let bits = data.u16(stream + 2)? as usize;
    let frames = data.u16(at + 8)? as usize;
    if values == 0 || values > 1024 || !values.is_multiple_of(4) || frames == 0 {
        return Err(Error::format("implausible FnDeltaQuatChan header"));
    }
    if !matches!(bits, 4 | 8 | 16) {
        return Err(Error::format(format!("FnDeltaQuatChan delta bits {bits}")));
    }
    let channels = values / 4;
    let mut bones = Vec::with_capacity(channels);
    for c in 0..channels {
        let slot = data.u16(at + 10 + 2 * c)? as usize;
        let bone = slot.checked_sub(4).filter(|s| s.is_multiple_of(12));
        bones.push(
            bone.and_then(|s| u8::try_from(s / 12).ok())
                .ok_or_else(|| Error::format(format!("FnDeltaQuatChan rotation slot {slot}")))?,
        );
    }
    let start = stream + 4 + 12 * values;
    let stride = (values * bits).div_ceil(8);
    bytes::range(&data, start, stride * (frames - 1))?;
    let mut table = Vec::with_capacity(values);
    let mut current = Vec::with_capacity(values);
    for v in 0..values {
        let e = stream + 4 + 12 * v;
        let minimum = data.f32(e)?;
        let scale = data.f32(e + 4)?;
        let initial = data.f32(e + 8)?;
        if !minimum.is_finite() || !scale.is_finite() || !initial.is_finite() {
            return Err(Error::format("non-finite FnDeltaQuatChan table"));
        }
        table.push((minimum, scale));
        current.push(initial);
    }
    let mut out = Vec::with_capacity(frames);
    for f in 0..frames {
        if f > 0 {
            let row = start + (f - 1) * stride;
            for (v, (cur, &(minimum, scale))) in current.iter_mut().zip(&table).enumerate() {
                let code = match bits {
                    4 => ((data[row + v / 2] >> (4 * (v & 1))) & 15) as f32,
                    8 => data[row + v] as f32,
                    _ => f32::from(data.u16(row + 2 * v)?),
                };
                // Raw 0039565c..00395664 multiplies, adds the minimum,
                // then adds the accumulated value. Preserve this order.
                *cur += code * scale + minimum;
            }
        }
        out.push(
            current
                .as_chunks::<4>()
                .0
                .iter()
                .copied()
                .map(|q| if normalize { unit_quaternion(q) } else { Ok(q) })
                .collect::<Result<Vec<_>>>()?,
        );
    }
    Ok((Rotations { bones, frames: out }, at + 10 + 2 * channels))
}

/// `FnRawPoseChannel` (type 0, vtable 0x4ae758, evaluate 0x392458):
/// u32 type/flags, signature word count, floats per frame, frame count;
/// two copies of the signature, then consecutive f32 frames. A signature
/// holds a channel count per successive bone followed by channel kinds:
/// 0 = xyzw quaternion, 1 = Euler degrees, 2 = xyz translation. Setup
/// 0x392480 replaces the first copy's kinds with direct readers and the
/// second copy's kinds with interpolators. 0x3928e8 advances one transform
/// (48 bytes) per bone. The two disc tracks contain 22 quaternion bones.
fn raw_pose<'a>(
    data: impl Into<Data<'a>>,
    at: usize,
    normalize: bool,
) -> Result<Option<(Rotations, usize)>> {
    let data: Data = data.into();
    let signature_words = data.u32(at + 4)? as usize;
    let stride = data.u32(at + 8)? as usize;
    let frames = data.u32(at + 12)? as usize;
    if signature_words == 0 || signature_words > 1024 || stride > 2560 || frames == 0 {
        return Err(Error::format("implausible FnRawPoseChannel header"));
    }
    let first = bytes::range(&data, at + 16, 4 * signature_words)?;
    let second = bytes::range(&data, at + 16 + 4 * signature_words, 4 * signature_words)?;
    if first != second {
        return Err(Error::format("FnRawPoseChannel signatures differ"));
    }
    let mut words = 0;
    let mut bone = 0;
    let mut consumed = 0;
    let mut rotations = Vec::new();
    while words < signature_words {
        let count = data.u32(at + 16 + 4 * words)? as usize;
        words += 1;
        if count > signature_words - words || bone > u8::MAX as usize {
            return Err(Error::format("invalid FnRawPoseChannel signature"));
        }
        let mut has_rotation = false;
        for _ in 0..count {
            let kind = data.u32(at + 16 + 4 * words)?;
            words += 1;
            match kind {
                0 | 1 => {
                    if has_rotation {
                        return Err(Error::format("duplicate FnRawPoseChannel bone rotation"));
                    }
                    rotations.push((bone as u8, consumed, kind));
                    has_rotation = true;
                    consumed += if kind == 0 { 4 } else { 3 };
                }
                2 => consumed += 3,
                _ => {
                    return Err(Error::format(format!(
                        "FnRawPoseChannel signature kind {kind}"
                    )));
                }
            }
        }
        bone += 1;
    }
    if consumed != stride {
        return Err(Error::format(
            "FnRawPoseChannel frame stride differs from signature",
        ));
    }
    let start = at + 16 + 8 * signature_words;
    let length = frames
        .checked_mul(stride)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| Error::format("FnRawPoseChannel frame size overflow"))?;
    bytes::range(&data, start, length)?;
    // Original 00392c00 copies four quaternion float words unchanged.
    // Native Euler conversion uses a separate reader at 00392ab8; host
    // trigonometry is only the display approximation, not that reader.
    if !normalize && rotations.iter().any(|&(_, _, kind)| kind == 1) {
        return Ok(None);
    }
    let bones = rotations.iter().map(|&(bone, _, _)| bone).collect();
    let mut out = Vec::with_capacity(frames);
    for f in 0..frames {
        let mut pose = Vec::with_capacity(rotations.len());
        for &(_, slot, kind) in &rotations {
            let p = start + 4 * (f * stride + slot);
            let mut q = [0.0; 4];
            for (k, v) in q.iter_mut().enumerate().take(if kind == 0 { 4 } else { 3 }) {
                *v = data.f32(p + 4 * k)?;
            }
            if kind == 1 {
                q = euler(
                    q[0].to_radians() / 2.0,
                    q[1].to_radians() / 2.0,
                    q[2].to_radians() / 2.0,
                );
            }
            pose.push(if normalize { unit_quaternion(q)? } else { q });
        }
        out.push(pose);
    }
    Ok(Some((Rotations { bones, frames: out }, start + length)))
}

fn unit_quaternion(q: [f32; 4]) -> Result<[f32; 4]> {
    let norm = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    if !norm.is_finite() || norm < 1e-6 {
        return Err(Error::format("invalid animation quaternion"));
    }
    Ok(q.map(|v| v / norm))
}

/// `FnDeltaF3` (type 20: evaluate 0x38ef48, setup 0x3906e8) and `FnDeltaF1`
/// (type 21: evaluate 0x390b30, setup 0x391d68), `dims` = 3 or 1:
///
/// - header: u16 type, u16 flags, u32 pointer to one u16 float slot per
///   channel (the first of its `dims` outputs), u32 key-time table (0), u16
///   frames, u16 channels, u8 block shift, u8, u16;
/// - per channel `dims` f32 key minimums, `dims` f32 key ranges, `dims` u16
///   delta minimums and `dims` u16 delta ranges;
/// - blocks of `2^shift` frames: a key (`dims` u16 per channel, each
///   `min + u * range / 65535`) and `2^shift - 1` delta frames (`dims` bytes
///   per channel, each adding `range * (2 dmin / 65535 - 1)` plus
///   `b * 2 range drange / 65535 / 255`), padded to even.
///
/// Returns the slot map and, per frame, `channels * dims` values.
fn delta_floats<'a>(
    data: impl Into<Data<'a>>,
    pointers: &std::collections::HashMap<u32, u32>,
    at: usize,
    dims: usize,
) -> Result<(Vec<u16>, Vec<Vec<f32>>)> {
    let data: Data = data.into();
    if data.u32(at + 8)? != 0 {
        return Err(Error::format("translation channel with a key-time table"));
    }
    let frames = data.u16(at + 0xc)? as usize;
    let channels = data.u16(at + 0xe)? as usize;
    let shift = *data
        .get(at + 0x10)
        .ok_or_else(|| Error::format("short translation channel"))?;
    if shift > 8 || channels > 64 {
        return Err(Error::format("implausible translation channel header"));
    }
    let map_at = *pointers
        .get(&(at as u32 + 4))
        .ok_or_else(|| Error::format("translation channel without a slot map"))?
        as usize;
    let map = (0..channels)
        .map(|c| data.u16(map_at + 2 * c))
        .collect::<Result<Vec<_>>>()?;

    // (key min, key scale, delta min, delta scale) per value.
    let mut table = Vec::with_capacity(channels * dims);
    let entry = 12 * dims;
    for c in 0..channels {
        let e = at + 0x14 + entry * c;
        for k in 0..dims {
            let kmin = data.f32(e + 4 * k)?;
            let krange = data.f32(e + 4 * (dims + k))?;
            let dmin = f32::from(data.u16(e + 8 * dims + 2 * k)?);
            let drange = f32::from(data.u16(e + 10 * dims + 2 * k)?);
            table.push((
                kmin,
                krange / 65535.0,
                krange * (2.0 * dmin / 65535.0 - 1.0),
                2.0 * krange * drange / 65535.0 / 255.0,
            ));
        }
    }
    let values = channels * dims;
    let start = at + 0x14 + entry * channels;
    let per_block = 1usize << shift;
    let mut block_len = 2 * values + (per_block - 1) * values;
    block_len += block_len & 1;

    let mut out = Vec::with_capacity(frames);
    let mut current = vec![0f32; values];
    for f in 0..frames {
        let (block, within) = (f / per_block, f % per_block);
        let base = start + block * block_len;
        if within == 0 {
            for (v, (cur, t)) in current.iter_mut().zip(&table).enumerate() {
                *cur = t.0 + f32::from(data.u16(base + 2 * v)?) * t.1;
            }
        } else {
            let d = base + 2 * values + (within - 1) * values;
            let deltas = bytes::range(&data, d, values)?;
            for ((cur, t), &b) in current.iter_mut().zip(&table).zip(deltas) {
                *cur += t.2 + f32::from(b) * t.3;
            }
        }
        out.push(current.clone());
    }
    Ok((map, out))
}

/// `FnDeltaQFast` (evaluate 0x38d188, key decode 0x38e558, range setup
/// 0x38e438, size 0x38e600):
///
/// - header: u16 type, u16 flags, u16 frames, u8 channels, u8, u32 key-time
///   table (unused here when 0), u32 pointer to one bone per channel, u8 block
///   shift, u8;
/// - per channel 8 u16: delta minimum xyzw (`u * 2/65535 - 1`) and range
///   xyzw (`u * 2/65535`), with the native float multiplier 0x38000080;
/// - blocks of `2^shift` frames, each a key (6 bytes per channel: three u16
///   whose top 12 bits are x, y, z and whose low nibbles make a 12-bit w; each
///   `v * 2/4095 - 1`) and then `2^shift - 1` delta frames (3 bytes per channel:
///   the top 6 bits of each byte are x, y, z, the low 2 bits together w; each
///   adds `min + range * v / 63` to the running quaternion), padded to even.
fn delta_q_fast<'a>(
    data: impl Into<Data<'a>>,
    pointers: &std::collections::HashMap<u32, u32>,
    at: usize,
    normalize: bool,
) -> Result<(Rotations, usize)> {
    let data: Data = data.into();
    let frames = data.u16(at + 4)? as usize;
    let channels = *data
        .get(at + 6)
        .ok_or_else(|| Error::format("short FnDeltaQFast"))? as usize;
    let times = data.u32(at + 8)?;
    if times != 0 {
        return Err(Error::format("FnDeltaQFast with a key-time table"));
    }
    let map = *pointers
        .get(&(at as u32 + 12))
        .ok_or_else(|| Error::format("FnDeltaQFast without a bone map"))? as usize;
    let bones = bytes::range(&data, map, channels)?.to_vec();
    let shift = *data
        .get(at + 0x10)
        .ok_or_else(|| Error::format("short FnDeltaQFast"))?;
    if shift > 8 {
        return Err(Error::format(format!("FnDeltaQFast block shift {shift}")));
    }

    let mut ranges = Vec::with_capacity(channels);
    for c in 0..channels {
        let u = |k: usize| data.u16(at + 0x12 + 16 * c + 2 * k).map(f32::from);
        let mut min = [0.0; 4];
        let mut range = [0.0; 4];
        for k in 0..4 {
            // Raw 0038e4a0..0038e528 uses 0x38000080 (2/65535),
            // not the nearby 1/32768 approximation.
            min[k] = u(k)? * f32::from_bits(Q_FAST_DELTA_SCALE) - 1.0;
            range[k] = u(4 + k)? * f32::from_bits(Q_FAST_DELTA_SCALE);
        }
        ranges.push((min, range));
    }
    let start = at + 0x12 + 16 * channels;
    let per_block = 1usize << shift;
    let mut block_len = channels * (6 + 3 * (per_block - 1));
    block_len += block_len & 1;

    let mut out = Vec::with_capacity(frames);
    let mut pose = vec![[0f32; 4]; channels];
    for f in 0..frames {
        let (block, within) = (f / per_block, f % per_block);
        let base = start + block * block_len;
        if within == 0 {
            for (c, q) in pose.iter_mut().enumerate() {
                let k = base + 6 * c;
                let h = |i: usize| data.u16(k + 2 * i).map(u32::from);
                let raw = [
                    h(0)? >> 4,
                    h(1)? >> 4,
                    h(2)? >> 4,
                    ((h(0)? & 0xf) << 8) | ((h(1)? & 0xf) << 4) | (h(2)? & 0xf),
                ];
                *q = raw.map(native_q_fast_key);
            }
        } else {
            let d = base + 6 * channels + (within - 1) * 3 * channels;
            for (c, q) in pose.iter_mut().enumerate() {
                let t = bytes::range(&data, d + 3 * c, 3)?;
                let v = [
                    t[0] >> 2,
                    t[1] >> 2,
                    t[2] >> 2,
                    ((t[0] & 3) << 4) | ((t[1] & 3) << 2) | (t[2] & 3),
                ];
                let (min, range) = &ranges[c];
                for k in 0..4 {
                    q[k] += min[k] + range[k] * f32::from(v[k]) * f32::from_bits(0x3c82_0821);
                }
            }
        }
        out.push(
            pose.iter()
                .map(|q| {
                    if !normalize {
                        return *q;
                    }
                    let n = q.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-6);
                    q.map(|v| v / n)
                })
                .collect(),
        );
    }
    let mut end = start + block_len * (frames / per_block);
    if !frames.is_multiple_of(per_block) {
        end += channels * (6 + 3 * (frames % per_block - 1));
    }
    Ok((Rotations { bones, frames: out }, end))
}

/// A bank's bytes and their byte order.
#[derive(Clone, Copy)]
struct Data<'a> {
    b: &'a [u8],
    be: bool,
}

impl<'a> From<&'a [u8]> for Data<'a> {
    fn from(b: &'a [u8]) -> Self {
        Self { b, be: false }
    }
}

impl<'a> From<&'a Vec<u8>> for Data<'a> {
    fn from(b: &'a Vec<u8>) -> Self {
        Self { b, be: false }
    }
}

impl std::ops::Deref for Data<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.b
    }
}

impl Data<'_> {
    fn u16(&self, at: usize) -> Result<u16> {
        if self.be {
            bytes::u16_be(self.b, at)
        } else {
            bytes::u16_le(self.b, at)
        }
    }

    fn u32(&self, at: usize) -> Result<u32> {
        if self.be {
            bytes::u32_be(self.b, at)
        } else {
            bytes::u32_le(self.b, at)
        }
    }

    fn f32(&self, at: usize) -> Result<f32> {
        if self.be {
            bytes::f32_be(self.b, at)
        } else {
            bytes::f32_le(self.b, at)
        }
    }
}

/// The QFast delta minimum/range scale, built by `lui t0, 0x3800; ori t0,
/// t0, 0x80` at `0x38e4a0` (2/65535).
const Q_FAST_DELTA_SCALE: u32 = 0x3800_0080;
/// The QFast key scale, built by `lui at, 0x3a00; ori at, at, 0x801` at
/// `0x38e560` (2/4095).
const Q_FAST_KEY_SCALE: u32 = 0x3a00_0801;

/// Raw 0038e558: multiply by exact float 0x3a000801, then subtract 1.
fn native_q_fast_key(value: u32) -> f32 {
    value as f32 * f32::from_bits(Q_FAST_KEY_SCALE) - 1.0
}

/// Quaternion built from three half angles by the formula in the
/// `FnDeltaSingleQ` setup (0x38aa80), transcribed as written.
fn euler(a: f32, b: f32, c: f32) -> [f32; 4] {
    let (sa, ca) = a.sin_cos();
    let (sb, cb) = b.sin_cos();
    let (sc, cc) = c.sin_cos();
    [
        sb * ca * sc - cb * sa * cc,
        sb * ca * cc + cb * sa * sc,
        sb * sa * cc - cb * ca * sc,
        sb * sa * sc + cb * ca * cc,
    ]
}

/// `FnDeltaSingleQ` (type 19; setup and evaluate 0x38aa80): one rotating
/// axis per bone, for hinges such as the knees.
///
/// - header: u16 type, u16 flags, u16 frames, u8 channels, u8 block shift,
///   u32 key-time table (0), u32 pointer to one bone per channel;
/// - per channel 7 u16: two fixed angles (`u * 2pi / 65536 - pi`), delta
///   minimums for the axis component and w (`u / 32768 - 1`), delta ranges
///   (`u / 32768`), and the rotating axis (0 x, 1 y, 2 z);
/// - blocks of `2^shift` frames: a key (2 bytes per channel, the axis
///   component and w, each `b * 2 / 255 - 1`) and `2^shift - 1` delta rows
///   (a byte per channel: the high nibble adds `min + range * n / 15` to the
///   axis component, the low nibble likewise to w), padded to even.
///
/// The bone's rotation is the normalised (axis, w) quaternion between a fixed
/// pre- and post-rotation built from the two angles: for axis x the post is
/// `E(0, t1/2, t2/2)`, for y the pre is `E(t1/2, 0, 0)` and the post
/// `E(0, 0, t2/2)`, for z the pre is `E(t1/2, t2/2, 0)` (E is [`euler`]). The
/// products below are the game's own, term for term (0x38b8e8..0x38bb20).
fn delta_single_q<'a>(
    data: impl Into<Data<'a>>,
    pointers: &std::collections::HashMap<u32, u32>,
    at: usize,
) -> Result<(Rotations, usize)> {
    let data: Data = data.into();
    let frames = data.u16(at + 4)? as usize;
    let header = bytes::range(&data, at + 6, 2)?;
    let (channels, shift) = (header[0] as usize, header[1]);
    if data.u32(at + 8)? != 0 {
        return Err(Error::format("FnDeltaSingleQ with a key-time table"));
    }
    if shift > 8 {
        return Err(Error::format(format!("FnDeltaSingleQ block shift {shift}")));
    }
    let map = *pointers
        .get(&(at as u32 + 12))
        .ok_or_else(|| Error::format("FnDeltaSingleQ without a bone map"))? as usize;
    let bones = bytes::range(&data, map, channels)?.to_vec();

    struct Channel {
        axis: u16,
        min: [f32; 2],
        range: [f32; 2],
        pre: [f32; 4],
        post: [f32; 4],
    }
    const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
    let mut table = Vec::with_capacity(channels);
    for c in 0..channels {
        let u = |k: usize| data.u16(at + 0x10 + 14 * c + 2 * k).map(f32::from);
        let angle = |k: usize| -> Result<f32> {
            Ok(u(k)? * std::f32::consts::TAU / 65536.0 - std::f32::consts::PI)
        };
        let (t1, t2) = (angle(0)? / 2.0, angle(1)? / 2.0);
        let axis = data.u16(at + 0x10 + 14 * c + 12)?;
        let (pre, post) = match axis {
            0 => (IDENTITY, euler(0.0, t1, t2)),
            1 => (euler(t1, 0.0, 0.0), euler(0.0, 0.0, t2)),
            2 => (euler(t1, t2, 0.0), IDENTITY),
            _ => return Err(Error::format(format!("FnDeltaSingleQ axis {axis}"))),
        };
        table.push(Channel {
            axis,
            min: [u(2)? / 32768.0 - 1.0, u(3)? / 32768.0 - 1.0],
            range: [u(4)? / 32768.0, u(5)? / 32768.0],
            pre,
            post,
        });
    }
    let start = at + 0x10 + 14 * channels;
    let per_block = 1usize << shift;
    let mut block_len = channels * (per_block + 1);
    block_len += block_len & 1;

    let mut out = Vec::with_capacity(frames);
    let mut current = vec![[0f32; 2]; channels];
    for f in 0..frames {
        let (block, within) = (f / per_block, f % per_block);
        let base = start + block * block_len;
        if within == 0 {
            let keys = bytes::range(&data, base, 2 * channels)?;
            for (cur, key) in current.iter_mut().zip(keys.as_chunks::<2>().0) {
                *cur = [key[0], key[1]].map(|b| f32::from(b) * 2.0 / 255.0 - 1.0);
            }
        } else {
            let row = bytes::range(
                &data,
                base + 2 * channels + (within - 1) * channels,
                channels,
            )?;
            for ((cur, t), &b) in current.iter_mut().zip(&table).zip(row) {
                cur[0] += t.min[0] + t.range[0] * f32::from(b >> 4) / 15.0;
                cur[1] += t.min[1] + t.range[1] * f32::from(b & 0xf) / 15.0;
            }
        }
        let pose = current
            .iter()
            .zip(&table)
            .map(|(cur, t)| {
                let n = cur[0].hypot(cur[1]).max(1e-6);
                let (a, w) = (cur[0] / n, cur[1] / n);
                let (p, q) = (t.pre, t.post);
                match t.axis {
                    0 => [
                        a * q[3] + w * q[0],
                        a * q[2] + w * q[1],
                        -a * q[1] + w * q[2],
                        -a * q[0] + w * q[3],
                    ],
                    1 => {
                        let m = [p[0] * w, p[3] * a, -p[0] * a, p[3] * w];
                        [
                            m[0] * q[3] - m[1] * q[2],
                            m[0] * q[2] + m[1] * q[3],
                            m[2] * q[3] + m[3] * q[2],
                            -m[2] * q[2] + m[3] * q[3],
                        ]
                    }
                    _ => [
                        p[0] * w - p[1] * a,
                        p[0] * a + p[1] * w,
                        p[2] * w + p[3] * a,
                        -p[2] * a + p[3] * w,
                    ],
                }
            })
            .collect();
        out.push(pose);
    }
    let mut end = start + block_len * (frames / per_block);
    if !frames.is_multiple_of(per_block) {
        end += channels * (frames % per_block + 1);
    }
    Ok((Rotations { bones, frames: out }, end))
}

/// Bank files on the disc: in-game, gamebreakers, intros, outros, front end, faces.
pub const BANKS: &[&str] = &[
    "/ANIM/ANIBIG.BIN",
    "/ANIM/ANIBGB.BIN",
    "/ANIM/ANIBIN.BIN",
    "/ANIM/ANIBOUT.BIN",
    "/ANIM/ANIBFE.BIN",
    "/ANIM/ANIFIG.BIN",
];

pub fn load<R: std::io::Read + std::io::Seek>(
    iso: &mut crate::iso::Iso<R>,
    path: &str,
) -> Result<Bank> {
    Bank::parse(&iso.read(path)?).map_err(|e| Error::format(format!("{path}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q_fast_scales_are_the_executable_instructions() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let data = crate::iso::Iso::open(path)
            .unwrap()
            .read("/SLUS_206.51")
            .unwrap();
        let constant = |a| crate::scripts::executable_constant(&data, a).unwrap();
        assert_eq!(constant(0x38e4a0), Q_FAST_DELTA_SCALE);
        assert_eq!(constant(0x38e560), Q_FAST_KEY_SCALE);
        for value in [0, 1, 2047, 2048, 4094, 4095] {
            assert_eq!(
                native_q_fast_key(value).to_bits(),
                (value as f32 * f32::from_bits(constant(0x38e560)) - 1.0).to_bits()
            );
        }
    }

    #[test]
    fn native_q_fast_keys_use_shipped_multiplier_at_boundaries() {
        assert_eq!(native_q_fast_key(0), -1.0);
        assert_eq!(native_q_fast_key(4095), 1.0);
        assert_eq!(native_q_fast_key(2047).to_bits(), 0xb980_0000);
        assert_eq!(native_q_fast_key(2048).to_bits(), 0x3980_1000);
        assert_ne!(native_q_fast_key(2047), 0.0);
    }

    #[test]
    fn raw_states_select_time_keys_and_pack_native_hand_and_integer_fields() {
        let mut data = vec![0u8; 20];
        data[..2].copy_from_slice(&RAW_STATE.to_le_bytes());
        data[4..6].copy_from_slice(&5u16.to_le_bytes());
        data[6..8].copy_from_slice(&2u16.to_le_bytes());
        data[8] = 4;
        data[9] = 8;
        for (index, descriptor) in [0x7814u16, 0x7818, 0x7800, 0x7808].into_iter().enumerate() {
            data[10 + index * 2..12 + index * 2].copy_from_slice(&descriptor.to_le_bytes());
        }
        data.extend(0f32.to_le_bytes());
        data.extend([1, 0, 2, 3]);
        data.extend(2.5f32.to_le_bytes());
        data.extend([0, 1, 0, 0]);
        let first = 4 | 2 << 15 | 3 << 18;
        assert_eq!(raw_states(&data, 0).unwrap(), [first, first, first, 8, 8]);
        data.pop();
        assert!(raw_states(&data, 0).is_err());
    }

    #[test]
    fn raw_states_unpack_the_original_two_bit_and_boolean_descriptor_layout() {
        let mut data = vec![0u8; 40];
        data[..2].copy_from_slice(&RAW_STATE.to_le_bytes());
        data[4..6].copy_from_slice(&3u16.to_le_bytes());
        data[6..8].copy_from_slice(&1u16.to_le_bytes());
        data[8] = 15;
        data[9] = 8;
        let offsets = [20, 0, 4, 8, 12, 16, 24, 28, 32, 36, 40, 44, 48, 52, 56];
        for (index, offset) in offsets.into_iter().enumerate() {
            let descriptor = if index == 0 {
                0x3814u16
            } else {
                0x1800 | offset
            };
            data[10 + index * 2..12 + index * 2].copy_from_slice(&descriptor.to_le_bytes());
        }
        data.extend(0f32.to_le_bytes());
        data.extend([0x80, 0, 0, 0]); // output word 5 = 2 => left-hand bit 4.
        assert_eq!(raw_states(&data, 0).unwrap(), [4, 4, 4]);
    }

    #[test]
    fn delta_quat_reads_all_code_widths_and_rejects_truncation() {
        for bits in [4u16, 8, 16] {
            let mut data = vec![0; 12];
            data[8..10].copy_from_slice(&2u16.to_le_bytes());
            data[10..12].copy_from_slice(&28u16.to_le_bytes()); // bone 2
            data.extend(4u16.to_le_bytes());
            data.extend(bits.to_le_bytes());
            for initial in [0.0f32, 0.0, 0.0, 1.0] {
                data.extend((-0.01f32).to_le_bytes());
                data.extend(0.001f32.to_le_bytes());
                data.extend(initial.to_le_bytes());
            }
            match bits {
                4 => data.extend([0x21, 0x43]),
                8 => data.extend([1, 2, 3, 4]),
                _ => {
                    for v in [1u16, 2, 3, 4] {
                        data.extend(v.to_le_bytes());
                    }
                }
            }
            let pointers = std::collections::HashMap::from([(4, 12)]);
            let (r, end) = delta_quat(&data, &pointers, 0, true).unwrap();
            assert_eq!(end, 12);
            assert_eq!(r.bones, [2]);
            assert_eq!(r.frames[0], [[0.0, 0.0, 0.0, 1.0]]);
            let expected = unit_quaternion([-0.009, -0.008, -0.007, 0.994]).unwrap();
            for (actual, expected) in r.frames[1][0].into_iter().zip(expected) {
                assert!((actual - expected).abs() < 1e-7, "{bits}-bit delta");
            }
            // FUN_003943e8 copies the accumulated integer-frame value
            // directly. Renderer normalization must not replace it.
            let (raw, _) = delta_quat(&data, &pointers, 0, false).unwrap();
            for (actual, expected) in raw.frames[1][0]
                .into_iter()
                .zip([-0.009, -0.008, -0.007, 0.994])
            {
                assert!((actual - expected).abs() < 1e-7, "{bits}-bit raw delta");
            }
            assert_ne!(raw.frames[1][0], r.frames[1][0]);
            data.pop();
            assert!(delta_quat(&data, &pointers, 0, true).is_err());
        }
    }

    #[test]
    fn delta_quat_adds_minimum_before_accumulated_value() {
        let mut data = vec![0; 12];
        data[8..10].copy_from_slice(&2u16.to_le_bytes());
        data[10..12].copy_from_slice(&4u16.to_le_bytes());
        data.extend(4u16.to_le_bytes());
        data.extend(16u16.to_le_bytes());
        for component in 0..4 {
            let (minimum, scale, initial) = if component == 3 {
                (-1.0f32, f32::from_bits(0x3380_0000), 1.0f32)
            } else {
                (0.0, 0.0, 0.0)
            };
            for value in [minimum, scale, initial] {
                data.extend(value.to_le_bytes());
            }
        }
        for code in [0u16, 0, 0, 1] {
            data.extend(code.to_le_bytes());
        }
        let pointers = std::collections::HashMap::from([(4, 12)]);
        let (native, _) = delta_quat(&data, &pointers, 0, false).unwrap();
        // product + minimum rounds to -1 + 2^-24; adding current
        // leaves 2^-24. Adding current first would round to zero.
        assert_eq!(native.frames[1][0][3].to_bits(), 0x3380_0000);
    }

    #[test]
    fn raw_pose_uses_signature_bones_and_skips_translation_values() {
        let signature = [0u32, 2, 2, 1, 1, 0];
        let mut data = Vec::new();
        for word in [0u32, signature.len() as u32, 10, 1] {
            data.extend(word.to_le_bytes());
        }
        for _ in 0..2 {
            for word in signature {
                data.extend(word.to_le_bytes());
            }
        }
        for value in [100f32, 200.0, 300.0, 0.0, 0.0, 90.0, 0.0, 0.0, 0.0, 1.0] {
            data.extend(value.to_le_bytes());
        }
        let (r, end) = raw_pose(&data, 0, true).unwrap().unwrap();
        assert_eq!(end, data.len());
        assert_eq!(r.bones, [1, 2]);
        let q = r.frames[0][0];
        assert!((q[2] + std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
        assert!((q[3] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
        assert_eq!(r.frames[0][1], [0.0, 0.0, 0.0, 1.0]);
        assert!(raw_pose(&data, 0, false).unwrap().is_none());
        data.pop();
        assert!(raw_pose(&data, 0, true).is_err());
    }

    #[test]
    fn native_raw_pose_copies_integer_quaternion_words_without_normalization() {
        let mut data = Vec::new();
        for word in [0u32, 2, 4, 1, 1, 0, 1, 0] {
            data.extend(word.to_le_bytes());
        }
        let words = [0x8000_0000u32, 0x3e80_0001, 0xbf00_0001, 0x4000_0000];
        for word in words {
            data.extend(word.to_le_bytes());
        }
        let (native, end) = raw_pose(&data, 0, false).unwrap().unwrap();
        assert_eq!(end, data.len());
        assert_eq!(native.frames[0][0].map(f32::to_bits), words);
        let (display, _) = raw_pose(&data, 0, true).unwrap().unwrap();
        assert_ne!(display.frames[0][0].map(f32::to_bits), words);
    }
    #[test]
    fn native_uncompressed_quaternions_match_every_original_disc_word() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let mut iso = crate::iso::Iso::open(path).unwrap();
        let bytes = iso.read("/ANIM/ANIBIG.BIN").unwrap();
        let object = crate::eagl::Object::read(&bytes).unwrap();
        let bank = Bank::parse(&bytes).unwrap();
        let word = |at: usize| u32::from_le_bytes(object.data[at..at + 4].try_into().unwrap());
        let header = object
            .symbols
            .iter()
            .find(|symbol| symbol.name.starts_with("__AnimationBank:::"))
            .unwrap()
            .value;
        let records = object.pointers[&(header + 12)];
        let names = object.pointers[&(header + 16)];
        let mut checked = 0;
        for index in 0..word(header as usize + 4) {
            let name_at = object.pointers[&(names + index * 4)] as usize;
            let name_bytes = &object.data[name_at..];
            let name_end = name_bytes.iter().position(|&byte| byte == 0).unwrap();
            let name = std::str::from_utf8(&name_bytes[..name_end]).unwrap();
            let Some(animation) = name.strip_suffix("_nocomp_r") else {
                continue;
            };
            let record = object.pointers[&(records + index * 4)];
            let at = object.pointers[&(record + 12)] as usize;
            assert_eq!(word(at) & 0xffff, 0);
            let signature_words = word(at + 4) as usize;
            let stride = word(at + 8) as usize;
            let frame_count = word(at + 12) as usize;
            assert_eq!(signature_words, 44);
            assert_eq!(stride, 88);
            for bone in 0..22 {
                assert_eq!(word(at + 16 + bone * 8), 1);
                assert_eq!(word(at + 20 + bone * 8), 0);
            }
            let native = bank
                .native_rotations(&format!("{animation}_nocomp"))
                .unwrap()
                .unwrap();
            assert_eq!(native.interpolation, NativeInterpolation::RawPose);
            assert_eq!(native.bones, (0..22).collect::<Vec<_>>());
            assert_eq!(native.frames.len(), frame_count);
            let start = at + 16 + 8 * signature_words;
            for (frame, pose) in native.frames.iter().enumerate() {
                for (bone, quaternion) in pose.iter().enumerate() {
                    for (component, value) in quaternion.iter().enumerate() {
                        let expected = word(start + (frame * stride + bone * 4 + component) * 4);
                        assert_eq!(
                            value.to_bits(),
                            expected,
                            "{name}/{frame}/{bone}/{component}"
                        );
                        checked += 1;
                    }
                }
            }
        }
        assert_eq!(checked, (41 + 28) * 22 * 4);
    }
}
