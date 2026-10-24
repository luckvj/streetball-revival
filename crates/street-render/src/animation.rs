//! Playing the game's animations on a skinned player.
//!
//! A [`Clip`] is one decoded animation. An [`Animator`] on a player's root
//! (next to its [`Rig`]) plays a clip and crossfades to the next one;
//! [`pose_skeletons`] writes the result into the joint transforms each frame.
//!
//! Animations face the opposite way from the rest pose (their root rotation is
//! the rest pose's turned half a turn about y), so an animated player faces
//! -z in its own space.

use std::{
    io::{Read, Seek},
    sync::Arc,
};

use bevy::prelude::*;
pub use street_assets::clip::{
    ANIM_FPS, Clip, mirror_layout, mirror_rotation, mirrored_rotation, native_body_width_ratio,
    native_joint_translation,
};
use street_assets::iso::Iso;

/// Decoded clips by name ([`street_assets::clip::Clips`]), as a resource.
#[derive(Resource, Default)]
pub struct Clips(pub street_assets::clip::Clips);

impl Clips {
    /// All clips on the user's disc, including script-selected alternates.
    pub fn load_all<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Self, Box<dyn std::error::Error>> {
        street_assets::clip::Clips::load_all(iso).map(Self)
    }

    /// Decode the named animations from whichever banks hold them.
    pub fn load<R: Read + Seek>(
        iso: &mut Iso<R>,
        names: &[&str],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        street_assets::clip::Clips::load(iso, names).map(Self)
    }
}

impl std::ops::Deref for Clips {
    type Target = street_assets::clip::Clips;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A player's joints, indexed by skeleton bone.
#[derive(Component)]
pub struct Rig {
    pub joints: Vec<Entity>,
    pub mirror_bones: Vec<usize>,
    native_mirror: bool,
}

impl Rig {
    pub fn new(joints: Vec<Entity>, skeleton: &street_assets::eagl::Skeleton) -> Self {
        let (mirror_bones, native_mirror) = mirror_layout(skeleton);
        Self {
            joints,
            mirror_bones,
            native_mirror,
        }
    }
}

/// One clip being played.
#[derive(Clone)]
pub struct Playing {
    pub clip: Arc<Clip>,
    /// Current frame (fractional).
    pub frame: f32,
    /// Playback speed: 1 is the authored rate.
    pub speed: f32,
    pub looping: bool,
    /// Ignore the clip's travel across the floor (the game moves the player).
    pub in_place: bool,
    pub mirrored: bool,
}

/// Quantized heading correction matching native `FUN_00196698`.
/// The native circle uses 1024 units (0x400). If heading delta exceeds 22 units (~7.7 degrees),
/// outgoing root rotation is pre-compensated by rotating around Y by the heading delta.
pub fn quantized_heading_correction(old_heading: f32, new_heading: f32) -> Option<Quat> {
    let two_pi = 2.0 * std::f32::consts::PI;
    let to_quant = |h: f32| -> i32 {
        let norm = (h % two_pi + two_pi) % two_pi;
        ((norm / two_pi) * 1024.0).round() as i32 & 0x3ff
    };
    let old_q = to_quant(old_heading);
    let new_q = to_quant(new_heading);
    let delta = (old_q - new_q) & 0x3ff;
    if delta > 0x16 && delta < 0x3ea {
        let rad = (delta as f32) * (two_pi / 1024.0);
        Some(Quat::from_rotation_y(rad))
    } else {
        None
    }
}

/// Plays clips on a [`Rig`], crossfading from the previous clip.
#[derive(Component, Clone)]
pub struct Animator {
    pub current: Playing,
    /// The clip being faded out, how far the fade has gone (0..1), and optional outgoing-root heading compensation (FUN_00196698).
    previous: Option<(Playing, f32, Option<Quat>)>,
    fade_seconds: f32,
    pub last_heading: f32,
}

impl Animator {
    pub fn new(clip: Arc<Clip>) -> Self {
        Self {
            current: Playing {
                clip,
                frame: 0.0,
                speed: 1.0,
                looping: true,
                in_place: false,
                mirrored: false,
            },
            previous: None,
            fade_seconds: 0.0,
            last_heading: 0.0,
        }
    }

    /// Switch to `clip` over `fade_seconds`, unless it is already playing.
    pub fn play(&mut self, clip: &Arc<Clip>, fade_seconds: f32, looping: bool) {
        self.play_with(clip, fade_seconds, looping, false);
    }

    /// [`Self::play`], keeping the player where the game puts them when
    /// `in_place` (for actions whose clips step or lunge).
    pub fn play_with(
        &mut self,
        clip: &Arc<Clip>,
        fade_seconds: f32,
        looping: bool,
        in_place: bool,
    ) {
        self.play_with_mirror(clip, fade_seconds, looping, in_place, false);
    }

    /// Play the executable animation record's handed variant with optional heading compensation and explicit restart.
    #[allow(clippy::too_many_arguments)]
    pub fn play_with_mirror_and_heading_restart(
        &mut self,
        clip: &Arc<Clip>,
        fade_seconds: f32,
        looping: bool,
        in_place: bool,
        mirrored: bool,
        new_heading: Option<f32>,
        restart: bool,
    ) {
        if !restart && Arc::ptr_eq(&self.current.clip, clip) && self.current.mirrored == mirrored {
            if let Some(h) = new_heading {
                self.last_heading = h;
            }
            return;
        }
        let comp = new_heading.and_then(|h| {
            let res = quantized_heading_correction(self.last_heading, h);
            self.last_heading = h;
            res
        });
        let next = Playing {
            clip: clip.clone(),
            frame: 0.0,
            speed: 1.0,
            looping,
            in_place,
            mirrored,
        };
        let previous = std::mem::replace(&mut self.current, next);
        self.previous = Some((previous, 0.0, comp));
        self.fade_seconds = fade_seconds.max(1e-3);
    }

    /// Play the executable animation record's handed variant with optional heading compensation.
    pub fn play_with_mirror_and_heading(
        &mut self,
        clip: &Arc<Clip>,
        fade_seconds: f32,
        looping: bool,
        in_place: bool,
        mirrored: bool,
        new_heading: Option<f32>,
    ) {
        self.play_with_mirror_and_heading_restart(
            clip,
            fade_seconds,
            looping,
            in_place,
            mirrored,
            new_heading,
            false,
        );
    }

    /// Play the executable animation record's handed variant.
    pub fn play_with_mirror(
        &mut self,
        clip: &Arc<Clip>,
        fade_seconds: f32,
        looping: bool,
        in_place: bool,
        mirrored: bool,
    ) {
        self.play_with_mirror_and_heading(clip, fade_seconds, looping, in_place, mirrored, None);
    }

    /// Stop the clock: the pose stays where it is until something sets
    /// its frame (a replay showing a recorded pose).
    pub fn hold(&mut self) {
        self.current.speed = 0.0;
        if let Some((previous, _, _)) = &mut self.previous {
            previous.speed = 0.0;
        }
    }

    /// A crossfade from the previous clip is still on.
    pub fn fading(&self) -> bool {
        self.previous.is_some()
    }

    /// Put the crossfade at `t` (0: the previous clip, 1: the current),
    /// for a blend whose clock is somebody else's (an actor's animation
    /// channel on the simulation's updates). Nothing without a fade.
    pub fn set_fade_progress(&mut self, t: f32) {
        if let Some((_, progress, _)) = &mut self.previous {
            *progress = t.clamp(0.0, 1.0);
        }
    }

    /// Drop the crossfade: the current clip alone.
    pub fn clear_fade(&mut self) {
        self.previous = None;
    }

    /// The clip fading out, to set what a blend holds.
    pub fn previous_mut(&mut self) -> Option<&mut Playing> {
        self.previous.as_mut().map(|(playing, _, _)| playing)
    }

    fn advance(&mut self, dt: f32) {
        self.current.frame += dt * ANIM_FPS * self.current.speed;
        if let Some((prev, t, _)) = &mut self.previous {
            prev.frame += dt * ANIM_FPS * prev.speed;
            *t += dt / self.fade_seconds;
            if *t >= 1.0 {
                self.previous = None;
            }
        }
    }
}

/// Advance every [`Animator`] and pose its rig.
pub fn pose_skeletons(
    time: Res<Time>,
    mut players: Query<(&mut Animator, &Rig)>,
    mut joints: Query<&mut Transform>,
) {
    for (mut animator, rig) in &mut players {
        animator.advance(time.delta_secs());
        let current = &animator.current;
        for (bone, &joint) in rig.joints.iter().enumerate() {
            let Ok(mut transform) = joints.get_mut(joint) else {
                continue;
            };
            let sample_rotation = |playing: &Playing| {
                let source = if playing.mirrored {
                    rig.mirror_bones.get(bone).copied().unwrap_or(bone)
                } else {
                    bone
                };
                playing
                    .clip
                    .rotation(source, playing.frame, playing.looping)
                    .map(|q| mirrored_rotation(bone, q, playing.mirrored, rig.native_mirror))
            };
            let sample_root = |playing: &Playing| {
                playing
                    .clip
                    .root(playing.frame, playing.looping, playing.in_place)
                    .map(|mut v| {
                        if playing.mirrored {
                            v.x = -v.x;
                        }
                        v
                    })
            };
            let mut rotation = sample_rotation(current);
            let mut translation = if bone == 0 {
                sample_root(current)
            } else {
                None
            };
            if let Some((prev, t, comp)) = &animator.previous {
                let mut from_rot = sample_rotation(prev);
                if let (0, Some(comp_q), Some(r)) = (bone, comp, from_rot) {
                    from_rot = Some(*comp_q * r);
                }
                rotation = match (from_rot, rotation) {
                    (Some(a), Some(b)) => Some(a.slerp(b, *t)),
                    (a, b) => b.or(a),
                };
                if bone == 0 {
                    let from_t = sample_root(prev);
                    translation = match (from_t, translation) {
                        (Some(a), Some(b)) => Some(a.lerp(b, *t)),
                        (a, b) => b.or(a),
                    };
                }
            }
            if let Some(r) = rotation {
                transform.rotation = r;
            }
            if let Some(tr) = translation {
                transform.translation = tr;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use street_assets::{anim, clip::NATIVE_MIRROR_BONES};

    #[test]
    fn fractional_raw_pose_uses_signed_weighted_components_with_raw_lengths() {
        let first = [0.0, 0.0, 0.0, 2.0];
        for second in [[0.0, 1.0, 0.0, 1.0], [0.0, 1.0, 0.0, -1.0]] {
            let clip = Clip {
                name: "raw pose fixture".into(),
                states: vec![0; 2],
                rotations: anim::Rotations {
                    bones: vec![0],
                    frames: vec![vec![first], vec![second]],
                },
                native_interpolation: Some(anim::NativeInterpolation::RawPose),
                root: None,
                channel_of: HashMap::from([(0, 0)]),
            };
            let actual = clip.rotation(0, 0.25, false).unwrap();
            // Original weighted SECOND +/- FIRST. In the negative-dot
            // branch it flips the first endpoint's sign, not the second's.
            let expected = if second[3] < 0.0 {
                Quat::from_xyzw(0.0, 0.25, 0.0, -1.75)
            } else {
                Quat::from_xyzw(0.0, 0.25, 0.0, 1.75)
            }
            .normalize();
            for (a, b) in actual.to_array().into_iter().zip(expected.to_array()) {
                assert!((a - b).abs() < 1e-6);
            }
            let normalized_key_slerp =
                Quat::IDENTITY.slerp(Quat::from_array(second).normalize(), 0.25);
            assert!(actual.angle_between(normalized_key_slerp) > 0.02);
        }
    }

    #[test]
    fn fractional_qfast_preserves_raw_lengths_until_display_conversion() {
        // Different raw lengths distinguish native component interpolation
        // from both SLERP and normalized-key interpolation.
        let clip = Clip {
            name: "raw QFast fixture".into(),
            states: vec![0; 2],
            rotations: anim::Rotations {
                bones: vec![0],
                frames: vec![vec![[0.0, 0.0, 0.0, 2.0]], vec![[0.0, 1.0, 0.0, 0.0]]],
            },
            native_interpolation: Some(anim::NativeInterpolation::Components),
            root: None,
            channel_of: HashMap::from([(0, 0)]),
        };
        let sampled = clip.rotation(0, 0.5, false).unwrap();
        let expected = Quat::from_xyzw(0.0, 0.5, 0.0, 1.0).normalize();
        assert!(
            (sampled
                .to_array()
                .into_iter()
                .zip(expected.to_array())
                .map(|(a, b)| (a - b).abs())
                .sum::<f32>())
                < 1e-6
        );
        assert!(
            sampled
                .angle_between(
                    Quat::IDENTITY.slerp(Quat::from_rotation_y(std::f32::consts::PI), 0.5)
                )
                .abs()
                > 0.1
        );
        assert_eq!(clip.rotation(0, 0.0, false), Some(Quat::IDENTITY));
        assert_eq!(
            clip.rotation(0, 9.0, false),
            Some(Quat::from_xyzw(0.0, 1.0, 0.0, 0.0))
        );
        assert_eq!(clip.rotation(0, 1.5, true), Some(expected));
    }

    #[test]
    fn native_player_layout_from_disc() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = Iso::open(path).unwrap();
        let skeleton = street_assets::player::skeleton(&mut iso).unwrap();
        let (map, native) = mirror_layout(&skeleton);
        assert!(native);
        assert_eq!(&map[..22], &NATIVE_MIRROR_BONES);
        for (index, &source) in map.iter().enumerate().skip(22) {
            assert_eq!(source, index);
        }
    }

    #[test]
    fn native_mirror_uses_root_permutation_and_body_z_reflection() {
        let q = Quat::from_euler(EulerRot::XYZ, 0.3, -0.7, 1.1);
        assert_eq!(
            mirrored_rotation(0, q, true, true).to_array(),
            [q.z, q.w, q.x, q.y]
        );
        let body = mirrored_rotation(15, q, true, true);
        assert_eq!(body.to_array(), [q.x, q.y, -q.z, -q.w]);
        let reflect_z = |v: Vec3| Vec3::new(v.x, v.y, -v.z);
        let point = Vec3::new(2.0, 3.0, -4.0);
        assert!((body * reflect_z(point) - reflect_z(q * point)).length() < 1e-5);
        for bone in [0, 15, 19, 21, 22, 42] {
            let mirrored = mirrored_rotation(bone, q, true, true);
            assert_eq!(mirrored_rotation(bone, mirrored, true, true), q);
        }
        assert_eq!(mirrored_rotation(22, q, true, true), q);
        assert_eq!(mirrored_rotation(15, q, false, true), q);
    }

    #[test]
    fn native_mirrored_state_swaps_exclusive_pairs_and_preserves_other_bits() {
        let clip = |state| Clip {
            name: "state fixture".into(),
            states: vec![state],
            rotations: anim::Rotations {
                bones: vec![],
                frames: vec![],
            },
            native_interpolation: None,
            root: None,
            channel_of: HashMap::new(),
        };
        let unrelated = 0xfffc_f020;
        for (left, right) in [(1, 2), (4, 8), (0x40, 0x200), (0x80, 0x400), (0x100, 0x800)] {
            for (input, expected) in [
                (0, 0),
                (left, right),
                (right, left),
                (left | right, left | right),
            ] {
                assert_eq!(
                    clip(input | unrelated).state_at_mirrored(0, true),
                    expected | unrelated
                );
                assert_eq!(
                    clip(input | unrelated).state_at_mirrored(0, false),
                    input | unrelated
                );
            }
        }
    }

    #[test]
    fn native_mirrored_disc_pose_matches_joint_sampler() {
        use bevy::ecs::system::RunSystemOnce;
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = Iso::open(path).unwrap();
        let skeleton = street_assets::player::skeleton(&mut iso).unwrap();
        let clips = Clips::load(&mut iso, &["DRIBBLE_FREESTYLE_12_2_B_R"]).unwrap();
        let clip = clips.get("DRIBBLE_FREESTYLE_12_2_B_R").unwrap();
        let scripts = street_assets::scripts::MoveScripts::read(&mut iso).unwrap();
        let alternatives = &scripts.tricks[69].blends[0].targets;
        assert_eq!(
            (alternatives[0].record, alternatives[0].mirrored),
            (296, false)
        );
        assert_eq!(
            (alternatives[1].record, alternatives[1].mirrored),
            (297, true)
        );
        assert_eq!(clip.state_at_mirrored(21, false) & 0xc, 8);
        assert_eq!(clip.state_at_mirrored(21, true) & 0xc, 4);
        for mirrored in [false, true] {
            let mut world = World::new();
            world.insert_resource(Time::<()>::default());
            let joints: Vec<Entity> = skeleton
                .bones
                .iter()
                .map(|bone| {
                    world
                        .spawn(
                            Transform::from_scale(Vec3::from_array(bone.scale))
                                .with_rotation(Quat::from_array(bone.rotation))
                                .with_translation(Vec3::from_array(bone.translation)),
                        )
                        .id()
                })
                .collect();
            let mut animator = Animator::new(clip.clone());
            animator.current.frame = 21.0;
            animator.current.in_place = false;
            animator.current.mirrored = mirrored;
            world.spawn((animator, Rig::new(joints.clone(), &skeleton)));
            world.run_system_once(pose_skeletons).unwrap();
            let mut matrices: Vec<Mat4> = Vec::new();
            for (index, bone) in skeleton.bones.iter().enumerate() {
                let posed = world.get::<Transform>(joints[index]).unwrap();
                let source = if mirrored {
                    NATIVE_MIRROR_BONES.get(index).copied().unwrap_or(index)
                } else {
                    index
                };
                if let Some(q) = clip.rotation(source, 21.0, false) {
                    assert_eq!(posed.rotation, mirrored_rotation(index, q, mirrored, true));
                }
                let local = posed.to_matrix();
                matrices.push(bone.parent.map_or(local, |p| matrices[p] * local));
            }
            let root = clip.root(21.0, false, false).unwrap();
            let posed_root = world.get::<Transform>(joints[0]).unwrap().translation;
            assert_eq!(
                posed_root,
                if mirrored {
                    Vec3::new(-root.x, root.y, root.z)
                } else {
                    root
                }
            );
            for hand in [15, 19] {
                let offset = Vec3::new(0.4, 0.0, 0.0);
                let sampled = clip
                    .joint_position_with_offset(&skeleton, hand, 21.0, mirrored, false, offset)
                    .unwrap();
                assert!((sampled - matrices[hand].transform_point3(offset)).length() < 1e-5);
            }
        }
    }

    #[test]
    fn native_body_scaled_disc_hands_match_spawned_rig() {
        use bevy::ecs::system::RunSystemOnce;
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = Iso::open(path).unwrap();
        let skeleton = street_assets::player::skeleton(&mut iso).unwrap();
        let clips = Clips::load(&mut iso, &["DRIBBLE_FREESTYLE_12_2_B_R"]).unwrap();
        let clip = clips.get("DRIBBLE_FREESTYLE_12_2_B_R").unwrap();
        // Head proportions are independently applied at bone21. Neither
        // hand's parent chain contains that bone, so no head input is needed.
        for hand in [15, 19] {
            let mut joint = Some(hand);
            while let Some(index) = joint {
                assert_ne!(index, 21);
                joint = skeleton.bones[index].parent;
            }
        }
        let mut largest_width_change = 0.0f32;
        for body in [
            None,
            Some([1.2; 3]),
            Some([1.2, 1.8, 1.8]),
            Some([1.2, 0.9, 0.9]),
        ] {
            for mirrored in [false, true] {
                for size in [1.0, 0.5] {
                    let assets = crate::player::PlayerAssets {
                        name: "body geometry fixture".into(),
                        models: Vec::new(),
                        textures: Default::default(),
                        skeleton: skeleton.clone(),
                        extra_groups: Vec::new(),
                        scale: 1.2 * size,
                        native_body_scale: body,
                        native_head_scale: Some([2.0, 3.0, 4.0]),
                        head_scale: 2.0,
                    };
                    let mut world = World::new();
                    world.insert_resource(Time::<()>::default());
                    let root = crate::player::spawn(
                        &mut world.commands(),
                        &assets,
                        Transform::IDENTITY,
                        &mut Assets::<Mesh>::default(),
                        &mut Assets::<StandardMaterial>::default(),
                        &mut Assets::<Image>::default(),
                        &mut Assets::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>::default(),
                    );
                    world.flush();
                    let joints = world.get::<Rig>(root).unwrap().joints.clone();
                    let mut animator = Animator::new(clip.clone());
                    animator.current.frame = 21.0;
                    animator.current.mirrored = mirrored;
                    world.entity_mut(root).insert(animator);
                    world.run_system_once(pose_skeletons).unwrap();
                    let outer = world.get::<Transform>(root).unwrap().to_matrix();
                    let mut matrices: Vec<Mat4> = Vec::new();
                    for (index, bone) in skeleton.bones.iter().enumerate() {
                        let local = world.get::<Transform>(joints[index]).unwrap().to_matrix();
                        matrices.push(bone.parent.map_or(outer * local, |p| matrices[p] * local));
                    }
                    for hand in [15, 19] {
                        let sampled = clip
                            .joint_position_with_body_scale(
                                &skeleton, hand, 21.0, mirrored, false, body,
                            )
                            .unwrap();
                        let actual = matrices[hand].transform_point3(Vec3::ZERO);
                        assert!((sampled * assets.scale - actual).length() < 1e-5);
                        let uniform = clip
                            .joint_position(&skeleton, hand, 21.0, mirrored, false)
                            .unwrap();
                        if native_body_width_ratio(body) > 1.0 {
                            largest_width_change =
                                largest_width_change.max((sampled - uniform).length());
                        } else {
                            assert_eq!(sampled, uniform);
                        }
                    }
                }
            }
        }
        assert!(
            largest_width_change > 0.01,
            "fixture must distinguish native width from uniform height"
        );
    }

    #[test]
    fn mirrored_rotation_reflects_motion_and_preserves_handed_switch() {
        let rotation = Quat::from_euler(EulerRot::XYZ, 0.3, -0.7, 1.1);
        let point = Vec3::new(2.0, 3.0, -4.0);
        let reflect = |v: Vec3| Vec3::new(-v.x, v.y, v.z);
        assert!(
            (mirror_rotation(rotation) * reflect(point) - reflect(rotation * point)).length()
                < 1e-5
        );
        let clip = Arc::new(Clip {
            name: "synthetic handed move".into(),
            states: vec![0],
            rotations: anim::Rotations {
                bones: vec![0],
                frames: vec![vec![rotation.to_array()]],
            },
            native_interpolation: None,
            root: None,
            channel_of: HashMap::from([(0, 0)]),
        });
        let mut animator = Animator::new(clip.clone());
        animator.current.frame = 20.0;
        animator.play_with_mirror(&clip, 0.1, false, true, true);
        assert!(animator.current.mirrored);
        assert_eq!(animator.current.frame, 0.0);
        assert!(!animator.previous.as_ref().unwrap().0.mirrored);
    }

    #[test]
    fn joint_position_transforms_bone_local_offsets() {
        let rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let clip = Clip {
            name: "synthetic offset".into(),
            states: vec![0],
            rotations: anim::Rotations {
                bones: vec![0],
                frames: vec![vec![rotation.to_array()]],
            },
            native_interpolation: None,
            root: None,
            channel_of: HashMap::from([(0, 0)]),
        };
        let skeleton = street_assets::eagl::Skeleton {
            bones: vec![street_assets::eagl::Bone {
                name: "RHand".into(),
                parent: None,
                scale: [1.0; 3],
                rotation: Quat::IDENTITY.to_array(),
                translation: [1.0, 2.0, 3.0],
                id: 0,
                inverse_bind: Mat4::IDENTITY.to_cols_array(),
            }],
        };

        let origin = clip.joint_position(&skeleton, 0, 0.0, false, true).unwrap();
        let offset = clip
            .joint_position_with_offset(&skeleton, 0, 0.0, false, true, Vec3::new(0.4, 0.0, 0.0))
            .unwrap();
        assert!((origin - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-5);
        assert!((offset - Vec3::new(1.0, 2.4, 3.0)).length() < 1e-5);
    }

    #[test]
    fn joint_local_offset_follows_parent_chain() {
        let rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let clip = Clip {
            name: "parented offset".into(),
            states: vec![0],
            rotations: anim::Rotations {
                bones: vec![0],
                frames: vec![vec![rotation.to_array()]],
            },
            native_interpolation: None,
            root: None,
            channel_of: HashMap::from([(0, 0)]),
        };
        let skeleton = street_assets::eagl::Skeleton {
            bones: vec![
                street_assets::eagl::Bone {
                    name: "Root".into(),
                    parent: None,
                    scale: [1.0; 3],
                    rotation: Quat::IDENTITY.to_array(),
                    translation: [1.0, 0.0, 0.0],
                    id: 0,
                    inverse_bind: Mat4::IDENTITY.to_cols_array(),
                },
                street_assets::eagl::Bone {
                    name: "RHand".into(),
                    parent: Some(0),
                    scale: [1.0; 3],
                    rotation: Quat::IDENTITY.to_array(),
                    translation: [0.0, 2.0, 0.0],
                    id: 1,
                    inverse_bind: Mat4::IDENTITY.to_cols_array(),
                },
            ],
        };

        let hand = clip
            .joint_position_with_offset(&skeleton, 1, 0.0, false, true, Vec3::new(0.4, 0.0, 0.0))
            .unwrap();
        assert!((hand - Vec3::new(-1.0, 0.4, 0.0)).length() < 1e-5);
    }

    #[test]
    fn quantized_heading_correction_matches_native_1024_circle_and_deadzone() {
        use super::quantized_heading_correction;

        // Heading delta <= 22 units (~7.7 degrees) is within deadzone -> None
        let small_delta = 5.0f32.to_radians();
        assert_eq!(quantized_heading_correction(0.0, small_delta), None);
        assert_eq!(quantized_heading_correction(small_delta, 0.0), None);

        // Heading delta >= 23 units -> Some(Quat)
        let large_delta = 45.0f32.to_radians();
        let comp = quantized_heading_correction(0.0, large_delta);
        assert!(comp.is_some());
        let q = comp.unwrap();
        assert!(q.x.abs() < 1e-5);
        assert!(q.z.abs() < 1e-5);
        assert!(q.y.abs() > 0.01);
    }

    #[test]
    fn play_with_mirror_and_heading_restart_restarts_frame_at_zero() {
        let clip = Arc::new(Clip {
            name: "test".into(),
            states: vec![0; 60],
            rotations: anim::Rotations {
                bones: vec![0],
                frames: vec![vec![Quat::IDENTITY.to_array()]; 60],
            },
            native_interpolation: None,
            root: None,
            channel_of: HashMap::from([(0, 0)]),
        });

        let mut animator = Animator::new(clip.clone());
        // Advance animator 2 seconds (60 frames at 30 fps)
        animator.advance(2.0);
        assert!(animator.current.frame > 30.0);

        // Calling without restart keeps old frame due to pointer equality
        animator.play_with_mirror_and_heading(&clip, 0.1, false, true, false, None);
        assert!(animator.current.frame > 30.0);

        // Calling with restart resets frame to 0.0
        animator.play_with_mirror_and_heading_restart(&clip, 0.1, false, true, false, None, true);
        assert_eq!(animator.current.frame, 0.0);
    }
}
