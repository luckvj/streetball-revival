//! Effects that follow the ball: the gamebreaker ball's sparks and
//! streamer, and the motion trail behind a special pass, with `spark.ssh`,
//! `flag.ssh` and `motion.ssh` from `/XF/XF000003`.
//!
//! **Sparks** (the manager of type 0xc, 0x1ef2d0; its update 0x1ef3e8, the
//! constants set by 0x1efaa8 from a 60 Hz frame rate). They show while the
//! ball's state at +0x1a0 is set (0x14a968): 1 while the side with the ball
//! holds a full gamebreaker meter, 2 while a gamebreaker is under way. Each
//! frame two sparks start, from a pool of 30, somewhere in the ball and
//! along the path it covered that frame; each drifts out slowly (up to half
//! a mile an hour) and back along the ball's way (a mile an hour), lives 15
//! frames, shrinks from 0.15 by 0.125 over its life, and blinks out for a
//! frame every 5 to 10 frames. Light blue (136, 160, 190) with a full
//! meter, white in a gamebreaker.
//!
//! **Motion trail** (type 0x12, 0x1eff40, updated by 0x1f00f0): a ribbon of
//! the last 12 ball positions, from the top of the ball to the bottom, in
//! blue (100, 169, 245, 100), while the ball is a pass (state 4, set by
//! 0x14bff8) of a special kind: the game tests a play field (±1) or the
//! receiver's move bit 15; played here as a passout special (off the
//! heezay, back 2 papa, the kick pass) or an alley-oop pass. Gone at once
//! when the pass ends.
//!
//! **Streamer** (type 0xd, 0x1f8320, updated by 0x1f8540, drawn by the
//! ribbon class's 0x3079e0): the last 40 positions of the ball, each with
//! its heading, drawn while the ball's state is 2 (a gamebreaker) as a
//! ribbon faced to the camera: over its first 6.25 feet it widens from 0.25
//! to 1.5, waves across by `0.2 (1 - t) sin(3 pi t)`, and fades from half
//! opacity to nothing; `flag.ssh` runs along it, scrolling one repeat every
//! 0.6 seconds. Normal alpha blending (GS ALPHA 0x44).
//!
//! None of these shows in replays.

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use street_sim::{Action, BallState, PassStyle, ball::RADIUS};

use crate::{
    Screen,
    game::{Paused, Replay, Sim},
    setup::Scene,
    view::{BallView, SidelineCamera},
};

/// Frames a second: the game's constants are per frame.
const RATE: f32 = 60.0;
const POOL: usize = 30;
const PER_FRAME: usize = 2;
const LIFE: u32 = 15;
const START_SIZE: f32 = 0.15;
const SHRINK: f32 = 0.125 / LIFE as f32;
/// A mile an hour, in feet a frame (5280 / 3600 / 60).
const MPH: f32 = 5280.0 / 3600.0 / RATE;
const METER_FULL_COLOUR: [f32; 3] = [136.0, 160.0, 190.0];
const GAMEBREAKER_COLOUR: [f32; 3] = [255.0, 255.0, 255.0];
const TRAIL_LENGTH: usize = 12;
const TRAIL_COLOUR: [f32; 4] = [100.0, 169.0, 245.0, 100.0];
const STREAMER_POINTS: usize = 40;
const STREAMER_LENGTH: f32 = 6.25;
const STREAMER_WIDTH: (f32, f32) = (0.25, 1.5);
const STREAMER_ALPHA: f32 = 0.5;
const STREAMER_SCROLL: f32 = 0.6;

/// What the ball is doing, for the sparks (the ball's +0x1a0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glow {
    None,
    /// The side with the ball has a full meter.
    MeterFull,
    /// A gamebreaker shot is under way.
    Gamebreaker,
}

pub fn glow(w: &street_sim::World) -> Glow {
    // 0x14a968 sets 2 while the game's gamebreaker word (+0xf4) is on,
    // which holds through a Level 2's staged script too: the School's
    // capture shows the white sparks and the streamer on its lob.
    let breaking = w.sequence.is_some()
        || w.staged_start.is_some()
        || w.ball.shot.is_some_and(|s| s.gamebreaker > 0)
        || w.players.iter().any(|p| {
            matches!(
                p.action,
                Action::Shooting { gamebreaker, .. } if gamebreaker > 0
            )
        });
    if breaking {
        Glow::Gamebreaker
    } else if w.teams.get(w.possession).is_some_and(|t| t.meter_full())
        && !matches!(w.phase, street_sim::Phase::Over { .. })
    {
        Glow::MeterFull
    } else {
        Glow::None
    }
}

/// Whether the ball is a pass that leaves a trail.
pub fn trailing(w: &street_sim::World, pass_style: &[PassStyle]) -> bool {
    if w.ball.state != BallState::Pass {
        return false;
    }
    let Some(pass) = w.ball.pass else {
        return false;
    };
    let special = matches!(
        pass_style.get(pass.from),
        Some(PassStyle::Heezay | PassStyle::Glass | PassStyle::Kick)
    );
    let oop = w.players.get(pass.to).is_some_and(|p| p.leaping());
    special || oop
}

/// One spark of the pool.
#[derive(Component)]
struct Spark {
    /// Frames left; 0 is free.
    life: u32,
    /// Frames to the next blink.
    blink: u32,
    size: f32,
    velocity: Vec3,
}

#[derive(Component)]
struct Trail {
    past: Vec<Vec3>,
}

/// The gamebreaker ball's streamer: past positions and headings, newest
/// first, and the texture's scroll.
#[derive(Component, Default)]
struct Streamer {
    past: Vec<(Vec3, Vec3)>,
    phase: f32,
}

#[derive(Resource, Default)]
struct BallFx {
    clock: f32,
    last: Option<Vec3>,
    rng: u32,
}

impl BallFx {
    /// 0..1.
    fn roll(&mut self) -> f32 {
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

pub struct BallFxPlugin;

impl Plugin for BallFxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BallFx>()
            .add_systems(
                OnEnter(Screen::Playing),
                spawn.run_if(crate::options::effects_on),
            )
            .add_systems(
                Update,
                step.after(crate::view::sync_ball)
                    .run_if(in_state(Screen::Playing))
                    .run_if(|p: Res<Paused>| !p.on)
                    .run_if(crate::options::effects_on),
            );
    }
}

fn spawn(
    mut commands: Commands,
    scene: Res<Scene>,
    mut fx: ResMut<BallFx>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    *fx = BallFx {
        rng: 0x5eed_0c0c,
        ..default()
    };
    let a = &scene.ambience;
    let image = |t: Option<&street_assets::ssh::Texture>, images: &mut Assets<Image>| {
        t.map(|t| images.add(street_render::convert::image(t)))
    };
    let spark = image(a.spark.as_ref(), &mut images);
    let motion = image(a.motion.as_ref(), &mut images);
    let flag = image(a.flag.as_ref(), &mut images);
    let quad = meshes.add(Rectangle::new(2.0, 2.0));
    for _ in 0..POOL {
        let material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: spark.clone(),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            cull_mode: None,
            ..default()
        });
        commands.spawn((
            Mesh3d(quad.clone()),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::Hidden,
            Spark {
                life: 0,
                blink: 0,
                size: 0.0,
                velocity: Vec3::ZERO,
            },
            DespawnOnExit(Screen::Playing),
        ));
    }
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: motion,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..default()
    });
    let streamer_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: flag,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(streamer(&[], 0.0, Vec3::Z))),
        MeshMaterial3d(streamer_material),
        Transform::default(),
        Visibility::Hidden,
        Streamer::default(),
        // Its bounds would stay those of the empty mesh it starts as.
        NoFrustumCulling,
        DespawnOnExit(Screen::Playing),
    ));
    commands.spawn((
        Mesh3d(meshes.add(ribbon(&[]))),
        MeshMaterial3d(material),
        Transform::default(),
        Visibility::Hidden,
        Trail {
            past: Vec::with_capacity(TRAIL_LENGTH),
        },
        NoFrustumCulling,
        DespawnOnExit(Screen::Playing),
    ));
}

fn step(
    time: Res<Time>,
    mut fx: ResMut<BallFx>,
    sim: Res<Sim>,
    replay: Res<Replay>,
    cheats: Res<crate::setup::MatchCheats>,
    ball: Query<&Transform, (With<BallView>, Without<Spark>)>,
    camera: Query<&GlobalTransform, With<SidelineCamera>>,
    mut sparks: Query<
        (
            &mut Spark,
            &mut Transform,
            &mut Visibility,
            &MeshMaterial3d<StandardMaterial>,
        ),
        Without<Trail>,
    >,
    mut trail: Query<(&mut Trail, &Mesh3d, &mut Visibility), (Without<Spark>, Without<Streamer>)>,
    mut streamers: Query<
        (&mut Streamer, &Mesh3d, &mut Visibility),
        (Without<Spark>, Without<Trail>),
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    fx.clock += time.delta_secs();
    let mut frames = 0;
    while fx.clock >= 1.0 / RATE {
        fx.clock -= 1.0 / RATE;
        frames += 1;
    }
    let Ok(ball) = ball.single() else {
        return;
    };
    let at = ball.translation;
    let w = &sim.world;
    let replaying = replay.playing.is_some();
    let glow = if replaying { Glow::None } else { glow(w) };
    let (facing, eye) = camera
        .single()
        .map(|c| (c.rotation(), c.translation()))
        .unwrap_or_default();
    for _ in 0..frames.min(4) {
        // The ball's way this frame.
        let moved = fx.last.map_or(Vec3::ZERO, |l| at - l);
        fx.last = Some(at);
        let mut starts = if glow == Glow::None { 0 } else { PER_FRAME };
        for (mut s, mut t, mut vis, material) in &mut sparks {
            if s.life > 0 {
                s.life -= 1;
                t.translation += s.velocity;
                s.size -= SHRINK;
                s.blink = s.blink.saturating_sub(1);
                let shown = s.life > 0 && s.blink > 0;
                if s.blink == 0 {
                    s.blink = 5 + (fx.roll() * 6.0) as u32;
                }
                *vis = if shown {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            } else if starts > 0 {
                starts -= 1;
                // A direction out of the ball, somewhere inside it, along
                // this frame's way.
                let d = Vec3::new(fx.roll() - 0.5, fx.roll() - 0.5, fx.roll() - 0.5)
                    .try_normalize()
                    .unwrap_or(Vec3::Y);
                let inside = d * fx.roll() * (RADIUS - START_SIZE).max(0.0);
                t.translation = at - moved * fx.roll() + inside;
                let mut v = d * 0.5 * MPH * fx.roll();
                if let Some(way) = moved.try_normalize() {
                    v -= way * MPH;
                }
                s.velocity = v;
                s.life = LIFE;
                s.blink = 1;
                s.size = START_SIZE;
                let [r, g, b] = if glow == Glow::Gamebreaker {
                    GAMEBREAKER_COLOUR
                } else {
                    METER_FULL_COLOUR
                };
                if let Some(mut m) = materials.get_mut(&material.0) {
                    // 255 is twice the PS2's full intensity, added over the
                    // scene.
                    m.base_color = Color::linear_rgb(r / 128.0, g / 128.0, b / 128.0);
                }
                *vis = Visibility::Hidden;
            }
            t.rotation = facing;
            t.scale = Vec3::splat(s.size.max(0.0));
        }
        // The streamer keeps the ball's way whether it shows or not.
        for (mut s, _, _) in &mut streamers {
            if moved.length() > 1e-4 {
                s.past.insert(0, (at, moved.normalize()));
                s.past.truncate(STREAMER_POINTS);
            }
            s.phase = (s.phase + 1.0 / RATE / STREAMER_SCROLL).fract();
        }
        // The trail: a point a frame while a special pass flies.
        let on = !replaying && trailing(w, &sim.pass_style);
        for (mut trail, _, _) in &mut trail {
            if on {
                trail.past.insert(0, at);
                trail.past.truncate(TRAIL_LENGTH);
            } else {
                trail.past.clear();
            }
        }
    }
    for (s, mesh, mut vis) in &mut streamers {
        let shown = !replaying
            && (glow == Glow::Gamebreaker || cheats.0.gb2_trails_always_on())
            && s.past.len() >= 2;
        let want = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        if shown && let Some(mut m) = meshes.get_mut(&mesh.0) {
            *m = streamer(&s.past, s.phase, eye);
        }
    }
    for (trail, mesh, mut vis) in &mut trail {
        let shown = trail.past.len() >= 2;
        let want = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        if shown && let Some(mut m) = meshes.get_mut(&mesh.0) {
            *m = ribbon(&trail.past);
        }
    }
}

/// The streamer: a strip across each past heading, faced to the eye at
/// `eye`, widening, waving and fading over its first [`STREAMER_LENGTH`].
fn streamer(past: &[(Vec3, Vec3)], phase: f32, eye: Vec3) -> Mesh {
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut colours = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut s = 0.0f32;
    for (k, &(p, heading)) in past.iter().enumerate() {
        if k > 0 {
            s += p.distance(past[k - 1].0);
        }
        let t = (s / STREAMER_LENGTH).min(1.0);
        let wave = (1.0 - t) * 0.2 * (3.0 * std::f32::consts::PI * t).sin();
        let half = (STREAMER_WIDTH.0 + (STREAMER_WIDTH.1 - STREAMER_WIDTH.0) * t) * 0.5;
        let side = heading.cross(p - eye).try_normalize().unwrap_or(Vec3::Y);
        positions.push((p + side * (half + wave)).to_array());
        positions.push((p + side * (-half + wave)).to_array());
        let u = phase + s / STREAMER_LENGTH;
        uvs.push([u, 0.0]);
        uvs.push([u, 1.0]);
        let c = [1.0, 1.0, 1.0, STREAMER_ALPHA * (1.0 - t)];
        colours.push(c);
        colours.push(c);
        if k > 0 {
            let i = (2 * k) as u32;
            indices.extend([i - 2, i - 1, i, i - 1, i + 1, i]);
        }
        if t >= 1.0 {
            break;
        }
    }
    strip(positions, uvs, colours, indices)
}

/// A triangle list, or one empty triangle for the renderer to hold.
fn strip(
    mut positions: Vec<[f32; 3]>,
    mut uvs: Vec<[f32; 2]>,
    mut colours: Vec<[f32; 4]>,
    mut indices: Vec<u32>,
) -> Mesh {
    if indices.is_empty() {
        positions = vec![[0.0; 3]; 3];
        uvs = vec![[0.0; 2]; 3];
        colours = vec![[0.0; 4]; 3];
        indices = vec![0, 1, 2];
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_indices(Indices::U32(indices))
}

/// The trail as a strip from the top of the ball to the bottom at each
/// past position, the texture running along it, fading at its old end.
fn ribbon(past: &[Vec3]) -> Mesh {
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut colours = Vec::new();
    let mut indices = Vec::new();
    let [r, g, b, a] = TRAIL_COLOUR;
    let n = past.len();
    for (k, &p) in past.iter().enumerate() {
        let u = if n > 1 {
            k as f32 / (n - 1) as f32
        } else {
            0.0
        };
        let c = [r / 255.0, g / 255.0, b / 255.0, a / 128.0 * (1.0 - u)];
        positions.push((p + Vec3::Y * RADIUS).to_array());
        positions.push((p - Vec3::Y * RADIUS).to_array());
        uvs.push([u, 0.0]);
        uvs.push([u, 1.0]);
        colours.push(c);
        colours.push(c);
        if k > 0 {
            let i = (2 * k) as u32;
            indices.extend([i - 2, i - 1, i, i - 1, i + 1, i]);
        }
    }
    strip(positions, uvs, colours, indices)
}

#[cfg(test)]
mod tests {
    use street_sim::{Hoop, Player, Rules, World};

    use super::*;

    fn world() -> World {
        let players = (0..6)
            .map(|k| Player::new(k / 3, [0.0, k as f32], 0.0))
            .collect();
        let hoop = |x: f32| Hoop {
            rim: [x, 11.75, 0.0],
            rim_radius: 0.75,
        };
        World::new([hoop(37.0), hoop(-37.0)], players, Rules::default(), 1)
    }

    #[test]
    fn a_full_meter_lights_the_ball() {
        let mut w = world();
        assert_eq!(glow(&w), Glow::None);
        let team = w.possession;
        w.teams[team].meter = street_sim::street::METER_FULL;
        assert_eq!(glow(&w), Glow::MeterFull);
        w.players[0].action = Action::Shooting {
            hoop: 0,
            elapsed: 0.0,
            gamebreaker: 1,
            fade: false,
            tick: 0,
            script: 0,
            release: 30,
            check: 0,
            end: 40,
            gate: 0,
            latest: 0,
            stall: 0,
        };
        assert_eq!(glow(&w), Glow::Gamebreaker);
    }

    #[test]
    fn the_ribbon_spans_the_ball() {
        let past = [Vec3::ZERO, Vec3::X, Vec3::X * 2.0];
        let m = ribbon(&past);
        assert_eq!(m.count_vertices(), 6);
        assert_eq!(m.indices().map(|i| i.len()), Some(12));
        assert_eq!(ribbon(&[]).count_vertices(), 3);
    }

    /// The streamer widens along its length and ends at 6.25 feet.
    #[test]
    fn the_streamer_widens_and_stops() {
        let past: Vec<(Vec3, Vec3)> = (0..40)
            .map(|k| (Vec3::new(-0.5 * k as f32, 5.0, 0.0), Vec3::X))
            .collect();
        let m = streamer(&past, 0.0, Vec3::new(0.0, 5.0, 30.0));
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) =
            m.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        // Half a foot a point: points 0..=13 reach 6.5 feet, past 6.25.
        assert_eq!(p.len(), 2 * 14);
        let width = |k: usize| Vec3::from(p[2 * k]).distance(Vec3::from(p[2 * k + 1]));
        assert!((width(0) - 0.25).abs() < 1e-4);
        assert!((width(13) - 1.5).abs() < 1e-4);
    }
}
