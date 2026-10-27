//! The hanging dunker's hands on the rim ([`street_sim::rim::Grip`],
//! `FUN_00157fe0`): each update a player hanging with a hand posed on the
//! rim has that hand (bone 15 or 19) turned, over his clip's pose, to grip
//! the ring where it gives. A replay shows the joints
//! it recorded.

use bevy::prelude::*;
use street_render::animation::Rig;
use street_sim::rim::GRIP_BONES;

use crate::{game::Sim, head_turn::world_of, tape::Tape, view::SimPlayer};

pub fn grip_rims(
    sim: Res<Sim>,
    tape: Option<Res<Tape>>,
    players: Query<(&SimPlayer, &Rig)>,
    mut nodes: Query<(&mut Transform, Option<&ChildOf>)>,
) {
    if tape.as_deref().is_some_and(|t| t.replaying()) {
        return;
    }
    let mut updates = Vec::new();
    for (p, rig) in &players {
        let Some(grip) = sim.world.rim_grip(p.0) else {
            continue;
        };
        for (k, &(bone, parent)) in GRIP_BONES.iter().enumerate() {
            if !grip.hands[k] {
                continue;
            }
            let (Some(&hand), Some(&up)) = (rig.joints.get(bone), rig.joints.get(parent)) else {
                continue;
            };
            let (Some(hand_m), Some(up_m)) = (world_of(hand, &nodes), world_of(up, &nodes)) else {
                continue;
            };
            let (_, up_rot, _) = up_m.to_scale_rotation_translation();
            let q = grip.hand(
                hand_m.w_axis.truncate().to_array(),
                up_rot.to_array(),
                k == 1,
            );
            updates.push((hand, Quat::from_array(q)));
        }
    }
    for (joint, q) in updates {
        if let Ok((mut transform, _)) = nodes.get_mut(joint) {
            transform.rotation = q;
        }
    }
}
