//! The glow on moves: ribbons of light swept by a player's forearms and
//! shins while they pull a move with a chain going (the trail manager the
//! game builds for every match, type 0x10 at 0x204d88, drawn each frame by
//! 0x204ff0).
//!
//! Per player there are four ribbons, each between two bones (the table at
//! 0x4641e0): left forearm to hand, right forearm to hand, left shin to
//! foot, right shin to foot. A ribbon keeps the last 12 positions of its two
//! bones. It shows while the player's side has a chain going and the player
//! is in one of six move states (bits 6, 7, 10, 12, 13, 15 of the player's
//! state, played here as a trick, a steal or counter, a block, a dunk, a
//! lay-up or alley-oop, a passout special), in the colour of the chain's
//! level (0x204f90: white at one link, then yellow, orange, deep orange and
//! red at five and more), at a quarter of that colour's alpha. A passout
//! special glows blue (slot 15, 0x204d40). Replays show none.

use std::collections::VecDeque;

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use street_render::animation::Rig;
use street_sim::{Action, PassStyle, player::DunkStyle};

use crate::{
    Screen,
    game::{Paused, Replay, Sim},
    view::SimPlayer,
};

/// Bone pairs a ribbon is swept between (0x4641e0).
const PAIRS: [(usize, usize); 4] = [(14, 15), (18, 19), (2, 3), (6, 7)];
/// Positions a ribbon keeps.
const LENGTH: usize = 12;
/// Samples a second (the game's frame rate).
const RATE: f32 = 60.0;

/// The chain level's colour, RGBA 0..255 (0x2056a8: the table 0x204f90
/// reads).
fn level_colour(level: usize) -> [f32; 4] {
    match level {
        0 | 1 => [255.0, 255.0, 255.0, 100.0],
        2 => [235.0, 210.0, 0.0, 170.0],
        3 => [235.0, 175.0, 0.0, 190.0],
        4 => [235.0, 125.0, 0.0, 220.0],
        5 => [180.0, 15.0, 15.0, 255.0],
        _ => [200.0, 15.0, 15.0, 255.0],
    }
}

/// A passout special's blue (0x204ff0 with the slot-15 flag).
const PASSOUT_BLUE: [f32; 4] = [100.0, 169.0, 220.0, 245.0];

/// One player's limb ribbon: its bones' past positions, newest first.
#[derive(Component)]
struct Ribbon {
    player: usize,
    pair: usize,
    past: VecDeque<(Vec3, Vec3)>,
    colour: [f32; 4],
}

#[derive(Resource, Default)]
struct Clock(f32);

pub struct TrailsPlugin;

impl Plugin for TrailsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Clock>()
            .add_systems(
                OnEnter(Screen::Playing),
                spawn_ribbons.run_if(crate::options::effects_on),
            )
            .add_systems(
                Update,
                sweep
                    .after(crate::view::sync_players)
                    .run_if(in_state(Screen::Playing))
                    .run_if(|p: Res<Paused>| !p.on)
                    .run_if(crate::options::effects_on),
            );
    }
}

fn spawn_ribbons(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..default()
    });
    for player in 0..sim.world.players.len() {
        for pair in 0..PAIRS.len() {
            commands.spawn((
                Mesh3d(meshes.add(empty_mesh())),
                MeshMaterial3d(material.clone()),
                Transform::default(),
                Ribbon {
                    player,
                    pair,
                    past: VecDeque::with_capacity(LENGTH),
                    colour: level_colour(0),
                },
                // Its bounds would stay those of the empty mesh it starts as.
                NoFrustumCulling,
                DespawnOnExit(Screen::Playing),
            ));
        }
    }
}

fn empty_mesh() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    // One invisible triangle: the renderer wants a vertex to hold.
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3])
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3])
    .with_inserted_indices(Indices::U32(vec![0, 1, 2]))
}

/// The colour a player's ribbons glow in now, if they glow.
fn glow(w: &street_sim::World, pass_style: &[PassStyle], i: usize) -> Option<[f32; 4]> {
    let p = w.players.get(i)?;
    let passout = matches!(p.action, Action::Passing { .. })
        && matches!(
            pass_style.get(i),
            Some(PassStyle::Heezay | PassStyle::Glass | PassStyle::Kick)
        );
    if passout {
        return Some(PASSOUT_BLUE);
    }
    let moving = match p.action {
        // Bit 10: a trick move, not the clearout.
        Action::Trick { .. } => p.action.juking(),
        Action::Stealing { .. } | Action::Layup { .. } => true,
        Action::Jumping { .. } => w.ball.state != street_sim::BallState::Loose,
        Action::Dunking { style, .. } => style != DunkStyle::Putback,
        _ => false,
    };
    let level = if w.combo.side == p.team {
        w.combo.ring.depth()
    } else {
        0
    };
    (moving && level >= 1).then(|| level_colour(level))
}

fn sweep(
    time: Res<Time>,
    mut clock: ResMut<Clock>,
    sim: Res<Sim>,
    replay: Res<Replay>,
    players: Query<(&SimPlayer, &Rig)>,
    joints: Query<&GlobalTransform>,
    mut ribbons: Query<(&mut Ribbon, &Mesh3d, &mut Visibility)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    clock.0 += time.delta_secs();
    let mut samples = 0;
    while clock.0 >= 1.0 / RATE {
        clock.0 -= 1.0 / RATE;
        samples += 1;
    }
    if samples == 0 {
        return;
    }
    let w = &sim.world;
    let replaying = replay.playing.is_some();
    for (mut ribbon, mesh, mut visibility) in &mut ribbons {
        let colour = if replaying {
            None
        } else {
            glow(w, &sim.pass_style, ribbon.player)
        };
        let ends = players
            .iter()
            .find(|(p, _)| p.0 == ribbon.player)
            .and_then(|(_, rig)| {
                let (a, b) = PAIRS[ribbon.pair];
                let at = |bone: usize| {
                    rig.joints
                        .get(bone)
                        .and_then(|&j| joints.get(j).ok())
                        .map(|g| g.translation())
                };
                Some((at(a)?, at(b)?))
            });
        for _ in 0..samples.min(LENGTH) {
            match (colour, ends) {
                (Some(c), Some(ends)) => {
                    ribbon.colour = c;
                    ribbon.past.push_front(ends);
                    ribbon.past.truncate(LENGTH);
                }
                // Out of the move: the ribbon shrinks away.
                _ => {
                    ribbon.past.pop_back();
                }
            }
        }
        let shown = ribbon.past.len() >= 2;
        let want = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
        if shown && let Some(mut m) = meshes.get_mut(&mesh.0) {
            build(&mut m, &ribbon);
        }
    }
}

/// The ribbon as a strip of quads, fading to nothing at its old end.
fn build(mesh: &mut Mesh, ribbon: &Ribbon) {
    let n = ribbon.past.len();
    let mut positions = Vec::with_capacity(n * 2);
    let mut colours = Vec::with_capacity(n * 2);
    let mut indices = Vec::with_capacity(n.saturating_sub(1) * 6);
    let [r, g, b, a] = ribbon.colour;
    // PS2 alpha: 0x80 is opaque; the game draws at a quarter of it.
    let alpha = (a * 0.25 / 128.0).min(1.0);
    for (k, &(p, q)) in ribbon.past.iter().enumerate() {
        let fade = 1.0 - k as f32 / LENGTH as f32;
        let c = [r / 255.0, g / 255.0, b / 255.0, alpha * fade];
        positions.push(p.to_array());
        positions.push(q.to_array());
        colours.push(c);
        colours.push(c);
        if k > 0 {
            let i = (2 * k) as u32;
            indices.extend([i - 2, i - 1, i, i - 1, i + 1, i]);
        }
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_indices(Indices::U32(indices));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_colour_climbs_with_the_chain() {
        assert_eq!(level_colour(1), [255.0, 255.0, 255.0, 100.0]);
        assert_eq!(level_colour(2)[1], 210.0);
        assert_eq!(level_colour(5), [180.0, 15.0, 15.0, 255.0]);
        assert_eq!(level_colour(9), level_colour(6));
    }

    #[test]
    fn a_ribbon_is_a_strip_between_its_two_bones() {
        let mut ribbon = Ribbon {
            player: 0,
            pair: 0,
            past: VecDeque::new(),
            colour: level_colour(3),
        };
        for k in 0..4 {
            let x = k as f32;
            ribbon
                .past
                .push_front((Vec3::new(x, 1.0, 0.0), Vec3::new(x, 2.0, 0.0)));
        }
        let mut mesh = empty_mesh();
        build(&mut mesh, &ribbon);
        assert_eq!(mesh.count_vertices(), 8);
        assert_eq!(mesh.indices().map(|i| i.len()), Some(3 * 6));
    }
}
