//! The neutral character and animation shapes the glTF writer takes.
//!
//! Every game's reader fills these: Vol. 2's PS2 players ([`crate::vol2`]),
//! and later V3's and Vol. 1's GameCube characters (`street_assets` GC
//! readers): a skeleton in the game's own bone names and units, skinned
//! triangle lists, and their images. Nothing here knows a disc format.

/// One joint, parents first (a joint's parent comes before it).
#[derive(Clone, Debug, PartialEq)]
pub struct Joint {
    /// The game's own bone name (`Root`, `LThigh`, `Head`, ...).
    pub name: String,
    pub parent: Option<usize>,
    /// The joint's local rest transform as the character wears it.
    pub translation: [f32; 3],
    /// Unit quaternion (x, y, z, w).
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    /// Model space to joint space at bind time, column-major.
    pub inverse_bind: [f32; 16],
}

/// RGBA8, straight alpha, top row first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    /// True when any pixel is not fully opaque (drawn with an alpha test).
    pub fn has_alpha(&self) -> bool {
        self.rgba.as_chunks::<4>().0.iter().any(|p| p[3] < 255)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    /// The game's texture slot (`jers`, `head`, `hair`, ...).
    pub name: String,
    /// Index into [`Character::images`].
    pub image: Option<usize>,
    /// Multiplies the image (or stands alone without one), linear RGBA.
    pub color: [f32; 4],
    /// Cut out pixels below half alpha.
    pub alpha_mask: bool,
}

/// One triangle list with one material.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Primitive {
    /// The game's draw group and slot, for the node's name.
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    /// Empty, or one unit normal per position.
    pub normals: Vec<[f32; 3]>,
    /// Empty, or one per position.
    pub uvs: Vec<[f32; 2]>,
    /// Empty, or one linear RGBA multiplier per position (0..1).
    pub colors: Vec<[f32; 4]>,
    /// One per position: up to four joints and weights summing to 1.
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub material: usize,
}

/// A character ready to write: model space is the game's (y up, units of
/// the game's skeleton), and `scale` takes it to metres for this character.
#[derive(Clone, Debug, PartialEq)]
pub struct Character {
    /// `<game>:<key>` (see `docs/PLAYER_EDITOR.md`).
    pub id: String,
    pub name: String,
    /// Which skeleton the joints follow (`vol2`, `v3gc`, ...).
    pub skeleton: String,
    pub joints: Vec<Joint>,
    pub primitives: Vec<Primitive>,
    pub materials: Vec<Material>,
    pub images: Vec<Image>,
    /// Uniform scale of the armature: game units to metres for this player.
    pub scale: f32,
    /// A half turn about y on the armature, so the character faces +z.
    pub turn: bool,
    /// Extra facts for the scene's `extras` (merged after `id`, `game`, ...).
    pub extras: serde_json::Map<String, serde_json::Value>,
}

/// One animation clip on a skeleton, sampled per frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    /// The game's own name.
    pub name: String,
    pub fps: f32,
    pub frames: usize,
    /// `(joint, rotation per frame)`, unit quaternions (x, y, z, w).
    pub rotations: Vec<(usize, Vec<[f32; 4]>)>,
    /// `(joint, translation per frame)` in skeleton units.
    pub translations: Vec<(usize, Vec<[f32; 3]>)>,
}

impl Clip {
    pub fn seconds(&self) -> f32 {
        self.frames.saturating_sub(1) as f32 / self.fps
    }
}

/// A set of clips over one skeleton.
#[derive(Clone, Debug, PartialEq)]
pub struct Library {
    pub name: String,
    pub skeleton: String,
    pub joints: Vec<Joint>,
    /// Armature scale (metres per unit) so clips preview at a sensible size.
    pub scale: f32,
    pub turn: bool,
    pub clips: Vec<Clip>,
}
