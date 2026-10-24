//! The players' bones, posed as the original poses them, for the rules that
//! read them: the swipe's hand (`FUN_0016cf30`), the limbs a ball meets
//! (`FUN_00137db8`) and the block's hands (`FUN_0016d888`).
//!
//! The original keeps every actor's model posed (bone `i` of `actor+0x108`
//! at `*(model+0x54) + i * 0xb0`: +0x0 the world position, +0x20 the world
//! rotation), built by `FUN_0019d4c8` from the playing channels' local
//! rotations: a bone's world position is its parent's plus the parent's
//! rotation applied to its local translation times the model's height
//! scale (`model+0x40`), the thighs' z and the upper arms' x times the
//! width instead when the width is the larger (bones 1, 5 and 13, 17), the
//! head's also times the head scale (`model+8`); its rotation is the
//! parent's times its own. The root (the pelvis) stands at the actor's x
//! and z, at the actor's height plus the clip's root height times the
//! height scale, turned by the facing word (`actor+0xa8`, a turn of 1024).
//! Mirrored records swap the left and right bones (`FUN_001914c8`'s
//! fourteen triples at `0x3e6790`) and turn their rotations (`FUN_0019159c`,
//! `FUN_00191658`).
//!
//! The bones come from the disc's animations (`/ANIM/*.BIN`, the 1144
//! animation records at `0x3de8a0`), so a world has them only when the game
//! (or a test with the disc) gives it a [`Poses`] bank. Checked against the
//! bones of six actors in three PCSX2 save states: every bone 0..21 within
//! 0.07 of the original's (`tests/pose_disc.rs`). The original also blends
//! channels: a script's `8e` blend ([`Poses::blend`]), the gait's change of
//! row from the old row's clip held at its frame (flag 0x200000,
//! [`crate::World::actor_pose`]) and a running catch's row on channel 2
//! over the run ([`Poses::overlay`], the pose step `0x157618..0x157714`).

use crate::math;

/// The bones the rules read: the skeleton's first 22 (`Root` .. `Head`).
pub const BONES: usize = 22;

/// The gait channel's blend at a change of row, in updates
/// ([`crate::ordinary::PlayerGaitChannel::blend_counter`]; in pads-1's
/// first 40,000 samples 1,568 of the 2,765 channel-0 blends last 6
/// updates, 1,231 of them from one gait record to another).
pub const GAIT_BLEND: i32 = 6;
pub const PELVIS: usize = 0;
/// `LHand`, hand 2 (`FUN_0014da20`'s bone `0xf`).
pub const LEFT_HAND: usize = 15;
/// `RHand`, hand 1 (bone `0x13`).
pub const RIGHT_HAND: usize = 19;
/// `Head` (bone `0x15`).
pub const HEAD: usize = 21;

/// `FUN_001914c8`'s left/right pairs over the 22 main bones.
const MIRROR: [usize; BONES] = [
    0, 5, 6, 7, 8, 1, 2, 3, 4, 9, 10, 11, 16, 17, 18, 19, 12, 13, 14, 15, 20, 21,
];

/// A skeleton bone at rest (`XF000002`'s player skeleton).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkeletonBone {
    pub parent: Option<u8>,
    pub translation: [f32; 3],
    /// Unit quaternion (x, y, z, w).
    pub rotation: [f32; 4],
}

/// One animation as the disc stores it.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipData {
    /// An `FnRawPoseChannel` track (its fractional reader blends the signed
    /// nearer endpoints, `0x379b20`); otherwise the delta tracks' readers
    /// interpolate the components (`0x38d834`).
    pub raw_pose: bool,
    /// The skeleton bone of each rotation channel.
    pub bones: Vec<u8>,
    /// `rotations[frame][channel]`, quaternions (x, y, z, w).
    pub rotations: Vec<Vec<[f32; 4]>>,
    /// The root's translation per frame, in court units at scale 1.
    pub root: Vec<[f32; 3]>,
    /// The `_X` state word per frame.
    pub states: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq)]
struct Clip {
    raw_pose: bool,
    /// Each bone's channel, if the clip animates it.
    channel: [Option<u8>; BONES],
    rotations: Vec<Vec<[f32; 4]>>,
    root: Vec<[f32; 3]>,
    states: Vec<u32>,
}

/// The disc's animations as the rules pose them.
#[derive(Clone, Debug, PartialEq)]
pub struct Poses {
    skeleton: [SkeletonBone; BONES],
    clips: Vec<Clip>,
    /// Each animation record's clip and mirror flag.
    records: Vec<Option<(u16, bool)>>,
}

/// The disc's animations as a world holds them: shared, fixed for the
/// match, and not game state. Equality, snapshots and checksums leave it
/// out: a world read from a snapshot (an online match's) has none until its
/// machine installs its own disc's ([`crate::World::install`]), and a world
/// without the disc's animations refuses to step ([`crate::World::step`]),
/// so a match that lost them is an error, never a game on other rules.
#[derive(Clone)]
pub enum Bank {
    /// Not installed: the world can't be stepped.
    Missing,
    /// The disc's animations.
    Disc(std::sync::Arc<Poses>),
    /// The rules' stand-ins for the bones (the swipe tested once with a
    /// stand-in hand, the interception and the catch by distance, the swat
    /// at the release, the trick's root spread evenly), for the tests that
    /// run without a disc. It exists only in test builds (`cfg(test)`, or
    /// street-sim's `stand-ins` feature, which only the workspace's
    /// dev-dependencies turn on), where it is also the default.
    #[cfg(any(test, feature = "stand-ins"))]
    StandIns,
}

impl Default for Bank {
    /// [`Bank::Missing`]; in a test build, [`Bank::StandIns`].
    fn default() -> Self {
        #[cfg(any(test, feature = "stand-ins"))]
        {
            Self::StandIns
        }
        #[cfg(not(any(test, feature = "stand-ins")))]
        {
            Self::Missing
        }
    }
}

impl Bank {
    /// The disc's animations, if installed.
    pub fn get(&self) -> Option<&Poses> {
        match self {
            Self::Disc(poses) => Some(poses),
            _ => None,
        }
    }

    /// The disc's animations, shared, if installed.
    pub fn arc(&self) -> Option<std::sync::Arc<Poses>> {
        match self {
            Self::Disc(poses) => Some(poses.clone()),
            _ => None,
        }
    }

    /// Nothing installed: the world can't be stepped.
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

impl PartialEq for Bank {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for Bank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Missing => "Bank(missing)",
            Self::Disc(_) => "Bank(disc)",
            #[cfg(any(test, feature = "stand-ins"))]
            Self::StandIns => "Bank(stand-ins)",
        })
    }
}

#[cfg(test)]
/// The disc's animations (with `STREET_ISO` set), for the rules that
/// read bones ([`crate::pose`]): the swipe's hand and the limbs a pass
/// meets. Without the disc, the stand-ins.
pub(crate) fn disc_bank() -> Bank {
    static POSES: std::sync::OnceLock<Option<std::sync::Arc<Poses>>> = std::sync::OnceLock::new();
    POSES
        .get_or_init(|| {
            let path = std::env::var_os("STREET_ISO")?;
            let mut iso = street_assets::iso::Iso::open(path).ok()?;
            let data = street_assets::pose::PoseData::read(&mut iso).ok()?;
            Poses::from_data(data).map(std::sync::Arc::new)
        })
        .clone()
        .map_or(Bank::StandIns, Bank::Disc)
}

#[cfg(test)]
impl Poses {
    /// A bank from the disc's pose data (`street-net`'s `disc` builds the
    /// game's).
    fn from_data(data: street_assets::pose::PoseData) -> Option<Self> {
        let skeleton: Vec<SkeletonBone> = data
            .skeleton
            .bones
            .iter()
            .map(|b| SkeletonBone {
                parent: b.parent.map(|p| p as u8),
                translation: b.translation,
                rotation: b.rotation,
            })
            .collect();
        let clips = data
            .clips
            .into_iter()
            .map(|c| ClipData {
                raw_pose: c.raw_pose,
                bones: c.bones,
                rotations: c.rotations,
                root: c.root,
                states: c.states,
            })
            .collect();
        Self::new(&skeleton, clips, data.records)
    }
}

/// A player's model scales (`model+0x40`, `model+8`): the height scale
/// (`height * 1.2 / 78`), the width and the head.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub height: f32,
    pub width: f32,
    pub head: f32,
}

impl Body {
    /// A model as tall as wide, with a plain head.
    pub fn uniform(height: f32) -> Self {
        Self {
            height,
            width: height,
            head: 1.0,
        }
    }
}

/// Where an actor stands: his feet (x, his height above the floor, z) and
/// his facing word (`actor+0xa8`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub position: [f32; 3],
    pub word: i32,
}

/// A bone in the world.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Joint {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

/// The 22 bones of a posed actor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub joints: [Joint; BONES],
}

/// What a clip plays at: a record and its frame (fractional).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub record: u16,
    pub time: f32,
    /// A cycle wraps past its last frame; anything else holds it.
    pub looping: bool,
}

/// Local rotations and the root of one sampled frame.
struct Local {
    rotations: [[f32; 4]; BONES],
    root: [f32; 3],
}

pub(crate) fn qmul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

/// `v` turned by `q` (`q v q*`).
pub(crate) fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let p = qmul(
        qmul(q, [v[0], v[1], v[2], 0.0]),
        [-q[0], -q[1], -q[2], q[3]],
    );
    [p[0], p[1], p[2]]
}

fn normalized(q: [f32; 4]) -> [f32; 4] {
    let l = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if l <= 0.0 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    q.map(|c| c / l)
}

/// `b` and `a` mixed by `t`, the nearer of `±a` taken (the raw-pose reader,
/// `0x379b20`), normalised.
fn nearer_mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let d: f32 = (0..4).map(|k| (b[k] - a[k]) * (b[k] - a[k])).sum();
    let s: f32 = (0..4).map(|k| (b[k] + a[k]) * (b[k] + a[k])).sum();
    let sign = if d > s { -1.0 } else { 1.0 };
    normalized(std::array::from_fn(|k| b[k] * t + sign * a[k] * (1.0 - t)))
}

/// `FUN_001914c8`'s mirror for bone `i`'s rotation: the root's (type 1,
/// `0x19159c`) swaps its halves, every other (type 4, `0x191658`) negates
/// z and w.
fn mirror_rotation(i: usize, q: [f32; 4]) -> [f32; 4] {
    if i == 0 {
        [q[2], q[3], q[0], q[1]]
    } else {
        [q[0], q[1], -q[2], -q[3]]
    }
}

/// The five left/right state pairs a mirrored record swaps
/// (`FUN_00191440`, `0x4546a8`).
pub fn mirror_state(mut state: u32) -> u32 {
    for (left, right) in [(1, 2), (4, 8), (0x40, 0x200), (0x80, 0x400), (0x100, 0x800)] {
        if (state & left != 0) != (state & right != 0) {
            state ^= left | right;
        }
    }
    state
}

impl Poses {
    /// A bank of `clips`, `records[r]` naming record `r`'s clip and mirror
    /// flag. None unless the skeleton has its 22 bones, parents first.
    pub fn new(
        skeleton: &[SkeletonBone],
        clips: Vec<ClipData>,
        records: Vec<Option<(u16, bool)>>,
    ) -> Option<Self> {
        if skeleton.len() < BONES
            || skeleton[..BONES]
                .iter()
                .enumerate()
                .any(|(i, b)| b.parent.is_some_and(|p| usize::from(p) >= i))
        {
            return None;
        }
        let clips = clips
            .into_iter()
            .map(|c| {
                let mut channel = [None; BONES];
                for (k, &b) in c.bones.iter().enumerate() {
                    if let Some(slot) = channel.get_mut(usize::from(b)) {
                        *slot = Some(k as u8);
                    }
                }
                Clip {
                    raw_pose: c.raw_pose,
                    channel,
                    rotations: c.rotations,
                    root: c.root,
                    states: c.states,
                }
            })
            .collect::<Vec<_>>();
        let records = records
            .into_iter()
            .map(|r| {
                r.filter(|&(c, _)| {
                    clips
                        .get(usize::from(c))
                        .is_some_and(|c| !c.rotations.is_empty())
                })
            })
            .collect();
        Some(Self {
            skeleton: std::array::from_fn(|i| skeleton[i]),
            clips,
            records,
        })
    }

    /// The bank as bytes in a fixed order (the skeleton, every clip, the
    /// records), for the digest the machines in a match compare before it
    /// starts (`street-net`'s `disc`).
    pub fn canonical(&self, out: &mut dyn FnMut(&[u8])) {
        let floats = |out: &mut dyn FnMut(&[u8]), v: &[f32]| {
            for f in v {
                out(&f.to_bits().to_le_bytes());
            }
        };
        for b in &self.skeleton {
            out(&[b.parent.unwrap_or(u8::MAX)]);
            floats(out, &b.translation);
            floats(out, &b.rotation);
        }
        out(&(self.clips.len() as u32).to_le_bytes());
        for c in &self.clips {
            out(&[u8::from(c.raw_pose)]);
            out(&c.channel.map(|k| k.unwrap_or(u8::MAX)));
            out(&(c.rotations.len() as u32).to_le_bytes());
            for frame in &c.rotations {
                out(&(frame.len() as u32).to_le_bytes());
                for q in frame {
                    floats(out, q);
                }
            }
            out(&(c.root.len() as u32).to_le_bytes());
            for p in &c.root {
                floats(out, p);
            }
            out(&(c.states.len() as u32).to_le_bytes());
            for w in &c.states {
                out(&w.to_le_bytes());
            }
        }
        out(&(self.records.len() as u32).to_le_bytes());
        for r in &self.records {
            match r {
                Some((c, mirrored)) => {
                    out(&[1]);
                    out(&c.to_le_bytes());
                    out(&[u8::from(*mirrored)]);
                }
                None => out(&[0]),
            }
        }
    }

    fn clip(&self, record: u16) -> Option<(&Clip, bool)> {
        let (c, mirrored) = (*self.records.get(usize::from(record))?)?;
        Some((&self.clips[usize::from(c)], mirrored))
    }

    /// Whether `record` can be posed.
    pub fn has(&self, record: u16) -> bool {
        self.clip(record).is_some()
    }

    /// Frames in `record`'s clip.
    pub fn frames(&self, record: u16) -> Option<usize> {
        Some(self.clip(record)?.0.rotations.len())
    }

    /// `record`'s state word at `frame` (mirrored records swap their
    /// left/right flags).
    pub fn state(&self, record: u16, frame: i32) -> u32 {
        let Some((clip, mirrored)) = self.clip(record) else {
            return 0;
        };
        if clip.states.is_empty() {
            return 0;
        }
        let state = clip.states[frame.rem_euclid(clip.states.len() as i32) as usize];
        if mirrored { mirror_state(state) } else { state }
    }

    fn local(&self, frame: Frame) -> Option<Local> {
        let (clip, mirrored) = self.clip(frame.record)?;
        let n = clip.rotations.len();
        let t = if frame.looping {
            frame.time.rem_euclid(n as f32)
        } else {
            frame.time.clamp(0.0, (n - 1) as f32)
        };
        let f0 = (t as usize).min(n - 1);
        let f1 = if frame.looping {
            (f0 + 1) % n
        } else {
            (f0 + 1).min(n - 1)
        };
        let u = t - f0 as f32;
        let rotations = std::array::from_fn(|i| {
            let source = if mirrored { MIRROR[i] } else { i };
            let q = match clip.channel[source] {
                Some(c) => {
                    let a = clip.rotations[f0][usize::from(c)];
                    let b = clip.rotations[f1][usize::from(c)];
                    if clip.raw_pose {
                        nearer_mix(a, b, u)
                    } else {
                        normalized(std::array::from_fn(|k| a[k] + (b[k] - a[k]) * u))
                    }
                }
                None => self.skeleton[i].rotation,
            };
            if mirrored { mirror_rotation(i, q) } else { q }
        });
        let root = if clip.root.is_empty() {
            self.skeleton[0].translation
        } else {
            let at = |f: usize| clip.root[f.min(clip.root.len() - 1)];
            let (a, b) = (at(f0), at(f1));
            let mut r: [f32; 3] = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * u);
            if mirrored {
                r[0] = -r[0];
            }
            r
        };
        Some(Local { rotations, root })
    }

    /// `frame`'s root translation (the clip's own, at scale 1; mirrored
    /// records mirror its x).
    pub fn root(&self, frame: Frame) -> Option<[f32; 3]> {
        Some(self.local(frame)?.root)
    }

    /// `frame`'s pose for a body standing at `place`.
    pub fn pose(&self, frame: Frame, body: Body, place: Place) -> Option<Pose> {
        Some(self.place(&self.local(frame)?, body, place))
    }

    /// A script's blend (`8e`, the channel step `FUN_0019a658`): `from`
    /// held, `to` mixed in by `weight` (the counter over the length), bone
    /// by bone, then posed.
    pub fn blend(
        &self,
        from: Frame,
        to: Frame,
        weight: f32,
        body: Body,
        place: Place,
    ) -> Option<Pose> {
        let b = self.local(to)?;
        let Some(a) = self.local(from) else {
            return Some(self.place(&b, body, place));
        };
        let w = weight.clamp(0.0, 1.0);
        let local = Local {
            rotations: std::array::from_fn(|i| nearer_mix(a.rotations[i], b.rotations[i], w)),
            root: std::array::from_fn(|k| a.root[k] + (b.root[k] - a.root[k]) * w),
        };
        Some(self.place(&local, body, place))
    }

    /// Two channels on one model: the pose step (`0x157618..0x157714`)
    /// writes each channel's parent-local rotations over its own bone range
    /// (`+0x74` on, `+0x7c` of them; channel 2 the upper body, 9..21, only
    /// with its flag 4), channel 0's bone 0 turned by the facing, so
    /// `lower`'s clip gives the root and the bones below `first`, and
    /// `upper`'s the rest, blended while its blend runs from `from` (the
    /// pose channel 0 had when the overlay began, which `FUN_0019c320`
    /// copies into it) by `weight`.
    pub fn overlay(
        &self,
        lower: Frame,
        upper: Frame,
        from: Option<(Frame, f32)>,
        first: usize,
        body: Body,
        place: Place,
    ) -> Option<Pose> {
        let base = self.local(lower)?;
        let mut top = self.local(upper)?;
        if let Some((from, weight)) = from
            && let Some(a) = self.local(from)
        {
            let w = weight.clamp(0.0, 1.0);
            top.rotations =
                std::array::from_fn(|i| nearer_mix(a.rotations[i], top.rotations[i], w));
        }
        let local = Local {
            rotations: std::array::from_fn(|i| {
                if i >= first {
                    top.rotations[i]
                } else {
                    base.rotations[i]
                }
            }),
            root: base.root,
        };
        Some(self.place(&local, body, place))
    }

    /// Bone `i`'s translation from its parent at `body`'s scales: the
    /// height scale, the width instead for the thighs' z and the upper arms'
    /// x when it is the larger, the head's also by the head scale.
    fn scaled(&self, i: usize, body: Body) -> [f32; 3] {
        let bone = &self.skeleton[i];
        let wide = body.height < body.width;
        let mut t = bone.translation.map(|c| c * body.height);
        if wide && matches!(i, 1 | 5) {
            t[2] = bone.translation[2] * body.width;
        } else if wide && matches!(i, 13 | 17) {
            t[0] = bone.translation[0] * body.width;
        } else if i == 21 {
            t = t.map(|c| c * body.head);
        }
        t
    }

    /// `FUN_00156dd0` (`actor+0x2a8`): one more than the longest of three
    /// chains from the pelvis (`0x3d7068`: to the left toe, the left hand
    /// and the head), each the sum of its bones' lengths at `body`'s
    /// scales. The limbs' test starts only within it of the pelvis.
    pub fn reach(&self, body: Body) -> f32 {
        const CHAINS: [&[usize]; 3] = [
            &[0, 1, 2, 3, 4],
            &[0, 9, 10, 11, 12, 13, 14, 15],
            &[0, 9, 10, 11, 20, 21],
        ];
        let longest = CHAINS
            .iter()
            .map(|c| {
                c[1..]
                    .iter()
                    .map(|&i| length(self.scaled(i, body)))
                    .sum::<f32>()
            })
            .fold(0.0, f32::max);
        longest + 1.0
    }

    /// `FUN_0019d4c8` over the 22 bones, then turned and moved to `place`.
    fn place(&self, local: &Local, body: Body, place: Place) -> Pose {
        let h = body.height;
        let theta = place.word as f32 * std::f32::consts::TAU / 1024.0;
        let yaw = [0.0, math::sin(theta * 0.5), 0.0, math::cos(theta * 0.5)];
        let mut joints = [Joint::default(); BONES];
        for i in 0..BONES {
            let bone = &self.skeleton[i];
            let q = local.rotations[i];
            joints[i] = match bone.parent {
                None => {
                    // In place: the pelvis over the actor's feet, at his
                    // height plus the clip's root height.
                    let y = place.position[1] + local.root[1] * h;
                    Joint {
                        position: [place.position[0], y, place.position[2]],
                        rotation: qmul(yaw, q),
                    }
                }
                Some(p) => {
                    let parent = joints[usize::from(p)];
                    let o = rotate(parent.rotation, self.scaled(i, body));
                    Joint {
                        position: std::array::from_fn(|k| parent.position[k] + o[k]),
                        rotation: qmul(parent.rotation, q),
                    }
                }
            };
        }
        Pose { joints }
    }
}

impl crate::World {
    /// `i`'s bones as the rules see them: his swipe's clip while he swipes,
    /// his trick's or reaction's script ([`crate::trick`]), his catch row's
    /// while a pass flies to him ([`crate::touch::Catching`]), his shot's,
    /// dunk's, lay-up's or pass's script, or his catch row's after the
    /// catch ([`crate::held`]), otherwise his gait channel's clip (the
    /// rebound and block jumps' clips are not posed here). None without the
    /// disc's animations.
    pub fn actor_pose(&self, i: usize) -> Option<Pose> {
        let poses = self.poses.get()?;
        if let Some(pose) = self.scripted_pose(i) {
            return Some(pose);
        }
        if let Some(pose) = self.catching_pose(i) {
            return Some(pose);
        }
        if let Some(pose) = self.move_pose(i) {
            return Some(pose);
        }
        let body = self.players[i].body();
        let place = self.place(i);
        if let crate::Action::Stealing { swipe: Some(s), .. } = self.players[i].action {
            return s.pose(poses, body, place);
        }
        // A change of gait row blends from the old row's clip held at its
        // frame over 6 updates (the channel step's blend, flag 0x200000:
        // the counter +0x70 over the length +0x6c, 1 on its first update).
        let g = &self.players[i].gait_channel;
        if (1..=GAIT_BLEND).contains(&g.blend_counter)
            && let Some((record, time)) = g.blend_from
        {
            let from = Frame {
                record,
                time,
                looping: true,
            };
            let weight = (GAIT_BLEND + 1 - g.blend_counter) as f32 / GAIT_BLEND as f32;
            return poses.blend(from, self.gait_frame(i), weight, body, place);
        }
        poses.pose(self.gait_frame(i), body, place)
    }
}

/// `FUN_0014da20`'s pick of a hand point by the clip and its frame: the
/// bones for hands 1 and 2, the offset (BSS at `0x4b84c0..`, set at boot),
/// the axis the ball's radius moves it along (0 +x, 1 -x, 2 +y, 3 -y, 4
/// +z, 5 -z), the axis hand 2 negates, and whether the bone's rotation
/// turns the offset.
#[derive(Clone, Copy, Debug, PartialEq)]
struct HandCase {
    bones: [usize; 2],
    offset: [f32; 3],
    radius_axis: usize,
    negate: Option<usize>,
    rotated: bool,
}

const HAND_CASES: [HandCase; 9] = [
    // 0: the hands, 0.4 along them.
    HandCase {
        bones: [19, 15],
        offset: [0.4, 0.0, 0.0],
        radius_axis: 4,
        negate: Some(2),
        rotated: true,
    },
    // 1: the forearms' ends.
    HandCase {
        bones: [18, 14],
        offset: [0.0, 0.0, 0.0],
        radius_axis: 3,
        negate: None,
        rotated: false,
    },
    // 2..8: the feet, thighs and shins for the clips that hold the ball there.
    HandCase {
        bones: [8, 4],
        offset: [0.2, 0.0, 0.0],
        radius_axis: 0,
        negate: Some(2),
        rotated: true,
    },
    HandCase {
        bones: [8, 4],
        offset: [0.0, 0.2, 0.0],
        radius_axis: 2,
        negate: None,
        rotated: false,
    },
    HandCase {
        bones: [5, 1],
        offset: [0.0, 0.2, -0.2],
        radius_axis: 2,
        negate: Some(2),
        rotated: true,
    },
    HandCase {
        bones: [8, 4],
        offset: [0.0, 0.0, 0.2],
        radius_axis: 4,
        negate: Some(2),
        rotated: true,
    },
    HandCase {
        bones: [6, 2],
        offset: [0.0, -0.2, 0.0],
        radius_axis: 3,
        negate: Some(2),
        rotated: true,
    },
    HandCase {
        bones: [8, 4],
        offset: [0.0, -0.2, 0.0],
        radius_axis: 3,
        negate: None,
        rotated: false,
    },
    HandCase {
        bones: [6, 2],
        offset: [0.0, 0.0, 0.2],
        radius_axis: 4,
        negate: Some(2),
        rotated: true,
    },
];

/// `FUN_0014da20`'s case for `clip` at `frame` (raw `0x14da2c..0x14dc54`).
fn hand_case(clip: i32, frame: i32) -> usize {
    let within = |first: i32, count: i32| (frame - first) as u32 <= (count - 1) as u32;
    match clip {
        0x84 | 0x85 if within(8, 2) => 1,
        0xc8 | 0xc9 if within(11, 2) || frame == 15 || frame == 16 => 7,
        0xcc | 0xcd if within(20, 2) => 4,
        0xce | 0xcf if within(18, 2) => 5,
        0xd6 | 0xd7 if within(24, 2) => 6,
        0xde | 0xdf if within(21, 2) => 2,
        0x10e | 0x10f if within(23, 2) => 2,
        0x122 | 0x123 if within(2, 63) => 6,
        0x124 | 0x125 if within(8, 2) => 8,
        0x12e | 0x12f if within(10, 2) => 5,
        0x207 | 0x208 if frame >= 10 => 2,
        0x209 | 0x20a if frame >= 0x12 => 6,
        0x20b | 0x20c if frame >= 9 => 5,
        0x2a6 | 0x2a7 if within(14, 2) => 1,
        _ => 0,
    }
}

/// `FUN_0014da20(ball, out, model, clip, frame, hand, flag)`: the point a
/// hand (1 the right, 2 the left) holds, for `clip` at `frame`. With
/// `ball_radius` (flag 0, the held ball's centre) the point moves out by
/// the ball's radius after the offset is scaled by the model (`model+0x40`,
/// height then width twice).
pub fn hand_point(
    pose: &Pose,
    clip: i32,
    frame: i32,
    hand: u8,
    body: Body,
    ball_radius: Option<f32>,
) -> [f32; 3] {
    let case = HAND_CASES[hand_case(clip, frame)];
    let bone = pose.joints[case.bones[usize::from(hand != 1)]];
    let mut offset = [
        case.offset[0] * body.height,
        case.offset[1] * body.width,
        case.offset[2] * body.width,
    ];
    if let Some(r) = ball_radius {
        let k = case.radius_axis / 2;
        offset[k] += if case.radius_axis.is_multiple_of(2) {
            r
        } else {
            -r
        };
    }
    if hand == 2
        && let Some(k) = case.negate
    {
        offset[k] = -offset[k];
    }
    let o = if case.rotated {
        rotate(bone.rotation, offset)
    } else {
        offset
    };
    std::array::from_fn(|k| bone.position[k] + o[k])
}

/// The limbs a ball can meet (`PTR_DAT_003d10a0`: three lists of capsules,
/// bone to bone): the head and body, the legs, the arms. Each is (from,
/// to, radius, how far past `to` it reaches).
pub const CAPSULES: [&[(usize, usize, f32, f32)]; 3] = [
    &[(20, 0, 0.73, 1.0), (20, 21, 0.35, 0.85)],
    &[
        (1, 2, 0.4, 0.0),
        (2, 3, 0.3, 0.0),
        (3, 4, 0.3, 0.5),
        (5, 6, 0.4, 0.0),
        (6, 7, 0.3, 0.0),
        (7, 8, 0.3, 0.5),
    ],
    &[
        (12, 13, 0.3, 0.0),
        (13, 14, 0.3, 0.0),
        (14, 15, 0.3, 1.0),
        (16, 17, 0.3, 0.0),
        (17, 18, 0.3, 0.0),
        (18, 19, 0.3, 1.0),
    ],
];

/// A limb a ball met (`FUN_00137c00`'s record): the capsule's bones, its
/// radius, the ball's centre, the way out (the ball from the nearest
/// point on the limb's segment) and that point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub bones: (usize, usize),
    pub radius: f32,
    pub point: [f32; 3],
    pub normal: [f32; 3],
    pub nearest: [f32; 3],
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn length(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

/// `FUN_00137b48`: `point` within `ball_radius + radius` of the segment
/// from bone `a` to bone `b` pushed `reach` past `b` (`FUN_00137ab0`), its
/// foot on the segment (`FUN_00137918`), the distance `FUN_001379d8`'s.
fn capsule(
    point: [f32; 3],
    a: [f32; 3],
    b: [f32; 3],
    radius: f32,
    reach: f32,
    ball_radius: f32,
) -> Option<([f32; 3], [f32; 3])> {
    let mut b = b;
    if reach != 0.0 {
        let ab = sub(b, a);
        let l = length(ab);
        // The original divides by the length; its limbs are never empty.
        if l <= 0.0 {
            return None;
        }
        let k = (reach + l) / l;
        b = std::array::from_fn(|i| a[i] + ab[i] * k);
    }
    if dot(sub(point, a), sub(b, a)) < 0.0 || dot(sub(point, b), sub(a, b)) < 0.0 {
        return None;
    }
    let ap = sub(point, a);
    let ab = sub(b, a);
    if dot(ab, ab) <= 0.0 {
        return None;
    }
    let k = dot(ap, ab) / dot(ab, ab);
    let d: [f32; 3] = std::array::from_fn(|i| ap[i] - ab[i] * k);
    (length(d) < ball_radius + radius).then(|| (d, sub(point, d)))
}

/// `FUN_00137db8`'s sweep: the ball's last move (from `from` to `to`) in
/// four steps, each above the floor tried against the limbs in order
/// (`FUN_00137c00`). The pelvis must be within `reach` of the ball first
/// (`FUN_00137d20`, `actor+0x2a8`).
pub fn sweep(
    pose: &Pose,
    from: [f32; 3],
    to: [f32; 3],
    ball_radius: f32,
    reach: f32,
) -> Option<Hit> {
    if length(sub(pose.joints[PELVIS].position, to)) > reach {
        return None;
    }
    let step = sub(to, from).map(|c| c * 0.25);
    let mut point = from;
    for _ in 0..4 {
        point = std::array::from_fn(|k| point[k] + step[k]);
        if point[1] <= 0.0 {
            continue;
        }
        for list in CAPSULES {
            for &(a, b, radius, ext) in list {
                let pa = pose.joints[a].position;
                let pb = pose.joints[b].position;
                if let Some((normal, nearest)) = capsule(point, pa, pb, radius, ext, ball_radius) {
                    return Some(Hit {
                        bones: (a, b),
                        radius,
                        point,
                        normal,
                        nearest,
                    });
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bank() -> Poses {
        // A two-bone arm: the root, then a hand one along x, still.
        let mut skeleton = vec![
            SkeletonBone {
                parent: None,
                translation: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
            };
            BONES
        ];
        for (i, b) in skeleton.iter_mut().enumerate().skip(1) {
            b.parent = Some((i - 1) as u8);
            b.translation = [1.0, 0.0, 0.0];
        }
        let clip = ClipData {
            raw_pose: false,
            bones: vec![],
            rotations: vec![vec![]; 2],
            root: vec![[0.0, 3.0, 0.0]; 2],
            states: vec![0x8, 0x4],
        };
        Poses::new(
            &skeleton,
            vec![clip],
            vec![Some((0, false)), Some((0, true))],
        )
        .unwrap()
    }

    #[test]
    fn a_still_chain_stands_at_the_actor_turned_by_his_word() {
        let p = bank();
        let place = Place {
            position: [10.0, 0.5, -2.0],
            word: 256,
        };
        let frame = Frame {
            record: 0,
            time: 0.0,
            looping: false,
        };
        let pose = p.pose(frame, Body::uniform(2.0), place).unwrap();
        // The pelvis over the feet at the actor's height plus twice the root's.
        assert_eq!(pose.joints[0].position, [10.0, 6.5, -2.0]);
        // A quarter turn takes the chain's +x to -z.
        let hand = pose.joints[RIGHT_HAND].position;
        assert!((hand[0] - 10.0).abs() < 1e-4, "{hand:?}");
        assert!((hand[2] - (-2.0 - 2.0 * 19.0)).abs() < 1e-3, "{hand:?}");
    }

    #[test]
    fn mirrored_records_swap_their_hand_flags() {
        let p = bank();
        assert_eq!(p.state(0, 0), 0x8);
        assert_eq!(p.state(1, 0), 0x4);
        assert_eq!(p.state(1, 3), 0x4 ^ 0xc);
    }

    #[test]
    fn the_hand_point_cases_follow_the_clip_and_frame() {
        assert_eq!(hand_case(0x182, 5), 0);
        assert_eq!(hand_case(0x84, 8), 1);
        assert_eq!(hand_case(0x84, 10), 0);
        assert_eq!(hand_case(0xc9, 16), 7);
        assert_eq!(hand_case(0x122, 64), 6);
        assert_eq!(hand_case(0x122, 65), 0);
        assert_eq!(hand_case(0x207, 9), 0);
        assert_eq!(hand_case(0x20b, 9), 5);
    }

    #[test]
    fn a_ball_meets_a_limb_on_the_way_through_it() {
        let p = bank();
        let place = Place {
            position: [0.0, 0.0, 0.0],
            word: 0,
        };
        let frame = Frame {
            record: 0,
            time: 0.0,
            looping: false,
        };
        let pose = p.pose(frame, Body::uniform(1.0), place).unwrap();
        // Bones 1..21 run along +x at y 3: a ball dropping through x 13.5
        // meets the arm capsule 12..13.
        let hit = sweep(&pose, [13.5, 5.0, 0.0], [13.5, 2.0, 0.0], 0.47, 30.0).unwrap();
        assert_eq!(hit.bones.0, 20);
        assert!(sweep(&pose, [13.5, 5.0, 4.0], [13.5, 4.5, 4.0], 0.47, 30.0).is_none());
        // Out of reach of the pelvis: no test at all.
        assert!(sweep(&pose, [13.5, 5.0, 0.0], [13.5, 2.0, 0.0], 0.47, 1.0).is_none());
    }
}
