//! Morph targets (`__Morph:::morph.modelname`): per-vertex offsets for a
//! skinned model's batches. league players share one body per build and get their
//! face from one of these (`/XF/XF000002` entries `5i..5i+2`, one per body
//! variant); the create-a-player kit uses them for faces, beards and shoes.
//!
//! The symbol points at a header: u32 target count, a pointer to the targets'
//! names (`foo`), a pointer to their weights, u32 0, and u32 attribute count.
//! Each attribute follows: a pointer to its name (`Coordinates` or `UV`), u32
//! target count, u32 0 and u32 chunk count, then its chunks. A chunk is
//! `(0xe0000000 or 0xa0000000) | byte size`, the vertex count, the target, the
//! batch number in the model (counting every microprogram run in the model's
//! chain order), then one i16 per component per input vertex of that batch,
//! padded to four bytes.
//!
//! The offsets are in the batch's stored units: positions in the quantised
//! u16 steps of the model's half extent (`raw / 32768 * half_extent`), UVs in
//! 4.12 fixed point. Every input vertex of a batch has an entry.

use std::collections::BTreeMap;

use crate::{Error, Result, bytes, eagl};

/// One target's offsets by batch number.
#[derive(Clone, Debug, Default)]
pub struct Morph {
    /// Position offsets in quantised units, one per input vertex.
    pub positions: BTreeMap<u32, Vec<[i16; 3]>>,
    /// UV offsets in 4.12 fixed point, one per input vertex.
    pub uvs: BTreeMap<u32, Vec<[i16; 2]>>,
}

pub fn parse(bytes: &[u8]) -> Result<Morph> {
    let obj = eagl::Object::read(bytes)?;
    let data = obj.data;
    let at = obj
        .symbols
        .iter()
        .find(|s| s.section == obj.data_index && s.name.starts_with("__Morph:::"))
        .ok_or_else(|| Error::format("no __Morph symbol"))?
        .value as usize;
    let word = |o: usize| bytes::u32_le(data, o);
    if word(at)? != 1 {
        return Err(Error::format("morph with more than one target"));
    }
    let attributes = word(at + 16)?;
    let mut morph = Morph::default();
    let mut at = at + 20;
    for _ in 0..attributes {
        let name_at = *obj
            .pointers
            .get(&(at as u32))
            .ok_or_else(|| Error::format("morph attribute without a name"))?;
        let name = bytes::cstr(data, name_at as usize)?;
        let chunks = word(at + 12)?;
        at += 16;
        for _ in 0..chunks {
            let mark = word(at)?;
            let size = (mark & 0x00ff_ffff) as usize;
            let count = word(at + 4)? as usize;
            let batch = word(at + 12)?;
            let values = bytes::range(data, at + 16, size)?;
            let values: Vec<i16> = values
                .as_chunks::<2>()
                .0
                .iter()
                .map(|h| i16::from_le_bytes([h[0], h[1]]))
                .collect();
            match (name.as_str(), mark >> 24) {
                ("Coordinates", 0xe0) if size == 6 * count => {
                    morph.positions.insert(
                        batch,
                        values
                            .as_chunks::<3>()
                            .0
                            .iter()
                            .map(|v| [v[0], v[1], v[2]])
                            .collect(),
                    );
                }
                ("UV", 0xa0) if size == 4 * count => {
                    morph.uvs.insert(
                        batch,
                        values
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|v| [v[0], v[1]])
                            .collect(),
                    );
                }
                _ => {
                    return Err(Error::format(format!(
                        "morph chunk {mark:#x} with {count} vertices in attribute {name}"
                    )));
                }
            }
            at = (at + 16 + size + 3) & !3;
        }
    }
    Ok(morph)
}

impl Morph {
    /// Add the offsets to `model`, scaled by `weight` (the game's players use
    /// 1). Fails when a batch's vertex count differs from the target's, which
    /// means the morph belongs to another model.
    pub fn apply(&self, model: &mut eagl::Model, weight: f32) -> Result<()> {
        let half = model
            .half_extent
            .ok_or_else(|| Error::format(format!("{} is not quantised", model.name)))?;
        for (batch, offsets) in &self.positions {
            self.check(model, *batch, offsets.len())?;
        }
        for (batch, offsets) in &self.uvs {
            self.check(model, *batch, offsets.len())?;
        }
        for mesh in &mut model.meshes {
            for (v, &(batch, input)) in mesh.sources.iter().enumerate() {
                if let Some(d) = self.positions.get(&batch).map(|o| o[input as usize]) {
                    for (k, &dk) in d.iter().enumerate() {
                        mesh.positions[v][k] += weight * f32::from(dk) / 32768.0 * half[k];
                    }
                }
                if let Some(d) = self.uvs.get(&batch).map(|o| o[input as usize]) {
                    for (k, &dk) in d.iter().enumerate() {
                        mesh.uvs[v][k] += weight * f32::from(dk) / 4096.0;
                    }
                }
            }
        }
        Ok(())
    }

    fn check(&self, model: &eagl::Model, batch: u32, count: usize) -> Result<()> {
        match model.batch_sizes.get(batch as usize) {
            Some(&n) if n as usize == count => Ok(()),
            n => Err(Error::format(format!(
                "morph offsets {count} vertices of batch {batch}, {} has {n:?}",
                model.name
            ))),
        }
    }
}
