//! The heads turned to their looks on screen ([`street_sim::head`]): each
//! update every player's head (bone 21) takes, after his clip's pose, the
//! turn toward what his look aims at ([`street_sim::look::Aim`]) in his
//! neck's frame (bone 20), blending at each new target. A replay shows the
//! joints it recorded, turns included.

use bevy::prelude::*;
use street_render::animation::Rig;
use street_sim::{head::Head, look::Aim};

use crate::{game::Sim, tape::Tape, view::SimPlayer};

/// The head bone and its parent, the neck.
const HEAD: usize = 21;
const NECK: usize = 20;

/// One player's head between updates.
#[derive(Default)]
pub struct Turn {
    head: Head,
    picks: u16,
    tick: u64,
    /// The rotation the last update gave the head while it turns.
    shown: Option<Quat>,
}

/// The world transform of `entity` through its parents.
pub(crate) fn world_of(
    entity: Entity,
    nodes: &Query<(&mut Transform, Option<&ChildOf>)>,
) -> Option<Mat4> {
    let mut m = Mat4::IDENTITY;
    let mut at = entity;
    for _ in 0..64 {
        let (t, parent) = nodes.get(at).ok()?;
        m = t.to_matrix() * m;
        match parent {
            Some(p) => at = p.parent(),
            None => return Some(m),
        }
    }
    Some(m)
}

pub fn turn_heads(
    sim: Res<Sim>,
    tape: Option<Res<Tape>>,
    players: Query<(&SimPlayer, &Rig)>,
    mut nodes: Query<(&mut Transform, Option<&ChildOf>)>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut turns: Local<Vec<Turn>>,
) {
    if tape.as_deref().is_some_and(|t| t.replaying()) {
        return;
    }
    let w = &sim.world;
    let rigs: Vec<(usize, Vec<Entity>)> = players
        .iter()
        .map(|(p, r)| (p.0, r.joints.clone()))
        .collect();
    let head_of = |j: usize| -> Option<Vec3> {
        let (_, joints) = rigs.iter().find(|(k, _)| *k == j)?;
        Some(world_of(*joints.get(HEAD)?, &nodes)?.w_axis.truncate())
    };
    let camera = cameras.iter().next().map(GlobalTransform::translation);
    let ball = Vec3::from_array(w.ball.position);
    let mut updates = Vec::new();
    for (i, joints) in &rigs {
        let i = *i;
        let (Some(&head_joint), Some(&neck_joint)) = (joints.get(HEAD), joints.get(NECK)) else {
            continue;
        };
        let Some(player) = w.players.get(i) else {
            continue;
        };
        if turns.len() <= i {
            turns.resize_with(i + 1, Turn::default);
        }
        let look = player.look;
        let t = &mut turns[i];
        if t.picks != look.picks {
            t.picks = look.picks;
            if look.aim == Aim::Clear {
                t.head.release();
            } else {
                t.head.aim();
            }
        }
        let holder = w.handler();
        let target_actor = match look.aim {
            Aim::Actor(j) => Some(usize::from(j)),
            Aim::Ball { holder: true } => holder,
            _ => None,
        };
        // Himself the target: let go (`0x15476c..0x154788`).
        if t.head.active && target_actor == Some(i) {
            t.head.release();
        }
        let steps = w.tick.saturating_sub(t.tick).min(10);
        t.tick = w.tick;
        if steps > 0 {
            let (Some(head_m), Some(neck_m)) =
                (world_of(head_joint, &nodes), world_of(neck_joint, &nodes))
            else {
                continue;
            };
            let head_at = head_m.w_axis.truncate();
            let (_, neck, _) = neck_m.to_scale_rotation_translation();
            let target = match look.aim {
                Aim::Clear => None,
                Aim::Actor(j) => head_of(usize::from(j)),
                Aim::Ball { holder: true } => holder.and_then(head_of).or(Some(ball)),
                Aim::Ball { holder: false } => Some(ball),
                Aim::Point(p) => Some(Vec3::from_array(p)),
                Aim::Guard => w.guard_of(i).and_then(head_of).map(|g| {
                    let mid = ball * 0.5 + g * 0.5;
                    Vec3::new(mid.x, g.y, mid.z)
                }),
                Aim::Camera => camera,
            };
            let to = target.map(|p| street_sim::head::direction(head_at.to_array(), p.to_array()));
            let clip = nodes
                .get(head_joint)
                .map(|(t, _)| t.rotation)
                .unwrap_or_default();
            let mut q = clip.to_array();
            for _ in 0..steps {
                q = t.head.step(clip.to_array(), neck.to_array(), to);
            }
            t.shown = (t.head.active || t.head.weight > 0.0 || q != clip.to_array())
                .then(|| Quat::from_array(q));
        }
        if let Some(q) = t.shown {
            updates.push((head_joint, q));
        }
    }
    for (joint, q) in updates {
        if let Ok((mut transform, _)) = nodes.get_mut(joint) {
            transform.rotation = q;
        }
    }
}
