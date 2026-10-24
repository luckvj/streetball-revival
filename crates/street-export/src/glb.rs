//! A small glTF 2.0 binary (`.glb`) writer for [`Character`]s and animation
//! [`Library`]s: one buffer, PNG images embedded, one skin.
//!
//! Layout of a character: node 0 is the armature (the character's uniform
//! scale to metres, and a half turn when the game's model faces -z), its
//! children are the root joint and the mesh node; the joints follow in the
//! skeleton's order (node `1 + joint`), named with the game's bone names. A
//! library has the same armature and joints and no mesh, so a clip's
//! channels name joints that every character of that skeleton shares.

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use crate::model::{Character, Clip, Image, Joint, Library};
use crate::picture;

const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;
const FLOAT: u32 = 5126;
const UNSIGNED_SHORT: u32 = 5123;
const UNSIGNED_INT: u32 = 5125;

#[derive(Default)]
struct Builder {
    bin: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Builder {
    fn view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let mut view = json!({
            "buffer": 0,
            "byteOffset": self.bin.len(),
            "byteLength": bytes.len(),
        });
        if let Some(t) = target {
            view["target"] = t.into();
        }
        self.bin.extend_from_slice(bytes);
        self.views.push(view);
        self.views.len() - 1
    }

    /// Floats as `kind` (`SCALAR`, `VEC2`, ...), `width` components each.
    fn floats(
        &mut self,
        data: &[f32],
        width: usize,
        kind: &str,
        bounds: bool,
        target: Option<u32>,
    ) -> usize {
        let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();
        let view = self.view(&bytes, target);
        let count = data.len() / width;
        let mut accessor = json!({
            "bufferView": view,
            "componentType": FLOAT,
            "count": count,
            "type": kind,
        });
        if bounds && count > 0 {
            let mut lo = vec![f32::INFINITY; width];
            let mut hi = vec![f32::NEG_INFINITY; width];
            for v in data.chunks_exact(width) {
                for k in 0..width {
                    lo[k] = lo[k].min(v[k]);
                    hi[k] = hi[k].max(v[k]);
                }
            }
            accessor["min"] = json!(lo);
            accessor["max"] = json!(hi);
        }
        self.accessors.push(accessor);
        self.accessors.len() - 1
    }

    fn joints(&mut self, data: &[[u16; 4]]) -> usize {
        let bytes: Vec<u8> = data
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let view = self.view(&bytes, Some(ARRAY_BUFFER));
        self.accessors.push(json!({
            "bufferView": view,
            "componentType": UNSIGNED_SHORT,
            "count": data.len(),
            "type": "VEC4",
        }));
        self.accessors.len() - 1
    }

    fn indices(&mut self, data: &[u32]) -> usize {
        let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();
        let view = self.view(&bytes, Some(ELEMENT_ARRAY_BUFFER));
        self.accessors.push(json!({
            "bufferView": view,
            "componentType": UNSIGNED_INT,
            "count": data.len(),
            "type": "SCALAR",
        }));
        self.accessors.len() - 1
    }

    fn image(&mut self, image: &Image) -> Value {
        let bytes = picture::encode(image.width, image.height, &image.rgba);
        let view = self.view(&bytes, None);
        json!({ "name": image.name, "bufferView": view, "mimeType": "image/png" })
    }

    /// The armature and joint nodes; returns the nodes (armature first).
    fn armature(&mut self, name: &str, joints: &[Joint], scale: f32, turn: bool) -> Vec<Value> {
        let mut nodes = Vec::with_capacity(joints.len() + 1);
        let mut armature = json!({ "name": name, "scale": [scale, scale, scale] });
        if turn {
            armature["rotation"] = json!([0.0, 1.0, 0.0, 0.0]);
        }
        let roots: Vec<usize> = joints
            .iter()
            .enumerate()
            .filter(|(_, j)| j.parent.is_none())
            .map(|(i, _)| 1 + i)
            .collect();
        armature["children"] = json!(roots);
        nodes.push(armature);
        for (i, joint) in joints.iter().enumerate() {
            let children: Vec<usize> = joints
                .iter()
                .enumerate()
                .filter(|(_, j)| j.parent == Some(i))
                .map(|(c, _)| 1 + c)
                .collect();
            let mut node = json!({
                "name": joint.name,
                "translation": joint.translation,
                "rotation": normalised(joint.rotation),
                "scale": joint.scale,
            });
            if !children.is_empty() {
                node["children"] = json!(children);
            }
            nodes.push(node);
        }
        nodes
    }

    fn finish(self, mut root: Value) -> Vec<u8> {
        let mut bin = self.bin;
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        root["asset"] = json!({ "version": "2.0", "generator": "street-export" });
        root["buffers"] = json!([{ "byteLength": bin.len() }]);
        root["bufferViews"] = Value::Array(self.views);
        root["accessors"] = Value::Array(self.accessors);
        let mut text = serde_json::to_vec(&root).expect("glTF JSON");
        while !text.len().is_multiple_of(4) {
            text.push(b' ');
        }
        let total = 12 + 8 + text.len() + 8 + bin.len();
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(text.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&text);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
        out
    }
}

fn normalised(q: [f32; 4]) -> [f32; 4] {
    let n = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    if n > 1e-6 && n.is_finite() {
        q.map(|v| v / n)
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

/// Weights summing to 1 (glTF requires it), unused joints at 0.
fn weights(w: [f32; 4], j: [u16; 4]) -> ([f32; 4], [u16; 4]) {
    let mut w = w.map(|v| if v.is_finite() && v > 0.0 { v } else { 0.0 });
    let mut j = j;
    let sum: f32 = w.iter().sum();
    if sum <= 1e-6 {
        return ([1.0, 0.0, 0.0, 0.0], [j[0], 0, 0, 0]);
    }
    for k in 0..4 {
        w[k] /= sum;
        if w[k] == 0.0 {
            j[k] = 0;
        }
    }
    (w, j)
}

/// A character as a `.glb`.
pub fn character(c: &Character) -> Vec<u8> {
    let mut b = Builder::default();
    let mut nodes = b.armature(&c.name, &c.joints, c.scale, c.turn);
    let mesh_node = nodes.len();
    nodes[0]["children"]
        .as_array_mut()
        .expect("armature children")
        .push(mesh_node.into());

    let images: Vec<Value> = c.images.iter().map(|i| b.image(i)).collect();
    let textures: Vec<Value> = (0..images.len())
        .map(|i| json!({ "sampler": 0, "source": i }))
        .collect();
    let materials: Vec<Value> = c
        .materials
        .iter()
        .map(|m| {
            let mut pbr = json!({
                "baseColorFactor": m.color,
                "metallicFactor": 0.0,
                "roughnessFactor": 1.0,
            });
            if let Some(i) = m.image {
                pbr["baseColorTexture"] = json!({ "index": i });
            }
            let mut mat = json!({
                "name": m.name,
                "pbrMetallicRoughness": pbr,
                "doubleSided": true,
            });
            if m.alpha_mask {
                mat["alphaMode"] = "MASK".into();
                mat["alphaCutoff"] = 0.5.into();
            }
            mat
        })
        .collect();

    let mut primitives = Vec::new();
    for p in &c.primitives {
        if p.positions.is_empty() || p.indices.is_empty() {
            continue;
        }
        let n = p.positions.len();
        let flat: Vec<f32> = p.positions.iter().flatten().copied().collect();
        let mut attributes = Map::new();
        attributes.insert(
            "POSITION".into(),
            b.floats(&flat, 3, "VEC3", true, Some(ARRAY_BUFFER)).into(),
        );
        if p.normals.len() == n {
            let flat: Vec<f32> = p
                .normals
                .iter()
                .flat_map(|v| {
                    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                    if len > 1e-6 && len.is_finite() {
                        v.map(|x| x / len)
                    } else {
                        [0.0, 1.0, 0.0]
                    }
                })
                .collect();
            attributes.insert(
                "NORMAL".into(),
                b.floats(&flat, 3, "VEC3", false, Some(ARRAY_BUFFER)).into(),
            );
        }
        if p.uvs.len() == n {
            let flat: Vec<f32> = p.uvs.iter().flatten().copied().collect();
            attributes.insert(
                "TEXCOORD_0".into(),
                b.floats(&flat, 2, "VEC2", false, Some(ARRAY_BUFFER)).into(),
            );
        }
        if p.colors.len() == n {
            let flat: Vec<f32> = p
                .colors
                .iter()
                .flatten()
                .map(|v| v.clamp(0.0, 1.0))
                .collect();
            attributes.insert(
                "COLOR_0".into(),
                b.floats(&flat, 4, "VEC4", false, Some(ARRAY_BUFFER)).into(),
            );
        }
        // Every primitive of a skinned node needs a skin; unskinned parts
        // ride the root joint.
        let (w, j): (Vec<[f32; 4]>, Vec<[u16; 4]>) = if p.joints.len() == n && p.weights.len() == n
        {
            p.weights
                .iter()
                .zip(&p.joints)
                .map(|(&w, &j)| weights(w, j))
                .unzip()
        } else {
            (vec![[1.0, 0.0, 0.0, 0.0]; n], vec![[0; 4]; n])
        };
        attributes.insert("JOINTS_0".into(), b.joints(&j).into());
        let flat: Vec<f32> = w.iter().flatten().copied().collect();
        attributes.insert(
            "WEIGHTS_0".into(),
            b.floats(&flat, 4, "VEC4", false, Some(ARRAY_BUFFER)).into(),
        );
        let indices = b.indices(&p.indices);
        primitives.push(json!({
            "attributes": attributes,
            "indices": indices,
            "material": p.material,
            "extras": { "name": p.name },
        }));
    }

    let ibm: Vec<f32> = c.joints.iter().flat_map(|j| j.inverse_bind).collect();
    let ibm = b.floats(&ibm, 16, "MAT4", false, None);
    nodes.push(json!({ "name": format!("{} mesh", c.name), "mesh": 0, "skin": 0 }));

    let mut extras = Map::new();
    extras.insert("id".into(), c.id.clone().into());
    extras.insert(
        "game".into(),
        c.id.split(':').next().unwrap_or_default().into(),
    );
    extras.insert("skeleton".into(), c.skeleton.clone().into());
    for (k, v) in &c.extras {
        extras.insert(k.clone(), v.clone());
    }

    let mut root = json!({
        "scene": 0,
        "scenes": [{ "name": c.name, "nodes": [0], "extras": extras }],
        "nodes": nodes,
        "meshes": [{ "name": c.name, "primitives": primitives }],
        "skins": [{
            "name": c.skeleton,
            "inverseBindMatrices": ibm,
            "joints": (1..=c.joints.len()).collect::<Vec<_>>(),
            "skeleton": 0,
        }],
        "materials": materials,
    });
    if !images.is_empty() {
        root["images"] = Value::Array(images);
        root["textures"] = Value::Array(textures);
        root["samplers"] = json!([{
            "magFilter": 9729, "minFilter": 9987, "wrapS": 10497, "wrapT": 10497,
        }]);
    }
    b.finish(root)
}

/// An animation library as a `.glb`: the skeleton and every clip.
pub fn library(l: &Library) -> Vec<u8> {
    let mut b = Builder::default();
    let nodes = b.armature(&l.name, &l.joints, l.scale, l.turn);
    let mut times: HashMap<(usize, u32), usize> = HashMap::new();
    let mut animations = Vec::new();
    for clip in &l.clips {
        animations.push(animation(&mut b, &mut times, clip));
    }
    let root = json!({
        "scene": 0,
        "scenes": [{
            "name": l.name,
            "nodes": [0],
            "extras": { "library": l.name, "skeleton": l.skeleton },
        }],
        "nodes": nodes,
        "animations": animations,
    });
    b.finish(root)
}

fn animation(b: &mut Builder, times: &mut HashMap<(usize, u32), usize>, clip: &Clip) -> Value {
    let frames = clip.frames.max(1);
    let input = *times
        .entry((frames, clip.fps.to_bits()))
        .or_insert_with(|| {
            let t: Vec<f32> = (0..frames).map(|f| f as f32 / clip.fps).collect();
            b.floats(&t, 1, "SCALAR", true, None)
        });
    let mut samplers = Vec::new();
    let mut channels = Vec::new();
    for (joint, keys) in &clip.rotations {
        let mut flat = Vec::with_capacity(frames * 4);
        let mut previous: Option<[f32; 4]> = None;
        for f in 0..frames {
            let mut q = normalised(keys[f.min(keys.len() - 1)]);
            // Keep neighbouring keys in one hemisphere so linear blends
            // take the short way.
            if let Some(p) = previous
                && p.iter().zip(&q).map(|(a, b)| a * b).sum::<f32>() < 0.0
            {
                q = q.map(|v| -v);
            }
            previous = Some(q);
            flat.extend_from_slice(&q);
        }
        let output = b.floats(&flat, 4, "VEC4", false, None);
        samplers.push(json!({ "input": input, "output": output, "interpolation": "LINEAR" }));
        channels.push(json!({
            "sampler": samplers.len() - 1,
            "target": { "node": 1 + joint, "path": "rotation" },
        }));
    }
    for (joint, keys) in &clip.translations {
        let flat: Vec<f32> = (0..frames)
            .flat_map(|f| keys[f.min(keys.len() - 1)])
            .collect();
        let output = b.floats(&flat, 3, "VEC3", false, None);
        samplers.push(json!({ "input": input, "output": output, "interpolation": "LINEAR" }));
        channels.push(json!({
            "sampler": samplers.len() - 1,
            "target": { "node": 1 + joint, "path": "translation" },
        }));
    }
    json!({
        "name": clip.name,
        "samplers": samplers,
        "channels": channels,
        "extras": { "frames": clip.frames, "fps": clip.fps },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Material, Primitive};

    fn joint(name: &str, parent: Option<usize>) -> Joint {
        let mut inverse_bind = [0.0; 16];
        for k in 0..4 {
            inverse_bind[k * 5] = 1.0;
        }
        Joint {
            name: name.into(),
            parent,
            translation: [0.0, 1.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
            inverse_bind,
        }
    }

    #[test]
    fn writes_a_valid_skinned_character_and_library() {
        let character = Character {
            id: "vol2:0".into(),
            name: "Test".into(),
            skeleton: "vol2".into(),
            joints: vec![joint("Root", None), joint("Head", Some(0))],
            primitives: vec![Primitive {
                name: "body".into(),
                positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
                colors: vec![[1.0; 4]; 3],
                joints: vec![[0, 1, 0, 0]; 3],
                weights: vec![[0.25, 0.25, 0.0, 0.0]; 3],
                indices: vec![0, 1, 2],
                material: 0,
            }],
            materials: vec![Material {
                name: "jers".into(),
                image: Some(0),
                color: [1.0; 4],
                alpha_mask: true,
            }],
            images: vec![Image {
                name: "jers".into(),
                width: 2,
                height: 2,
                rgba: vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 0, 0, 0],
            }],
            scale: 0.25,
            turn: true,
            extras: Map::new(),
        };
        let bytes = super::character(&character);
        let gltf = gltf::Gltf::from_slice(&bytes).unwrap();
        assert_eq!(gltf.skins().count(), 1);
        assert_eq!(gltf.nodes().nth(2).unwrap().name(), Some("Head"));
        let (doc, buffers, images) = gltf::import_slice(&bytes).unwrap();
        assert_eq!(images.len(), 1);
        let mesh = doc.meshes().next().unwrap();
        let prim = mesh.primitives().next().unwrap();
        let reader = prim.reader(|b| Some(&buffers[b.index()]));
        let w: Vec<[f32; 4]> = reader.read_weights(0).unwrap().into_f32().collect();
        assert_eq!(w[0], [0.5, 0.5, 0.0, 0.0]);

        let library = Library {
            name: "base".into(),
            skeleton: "vol2".into(),
            joints: character.joints.clone(),
            scale: 0.25,
            turn: true,
            clips: vec![Clip {
                name: "stand".into(),
                fps: 30.0,
                frames: 3,
                rotations: vec![(1, vec![[0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, -1.0]])],
                translations: vec![(0, vec![[0.0; 3]; 3])],
            }],
        };
        let bytes = super::library(&library);
        let (doc, _, _) = gltf::import_slice(&bytes).unwrap();
        let anim = doc.animations().next().unwrap();
        assert_eq!(anim.name(), Some("stand"));
        assert_eq!(anim.channels().count(), 2);
    }
}
