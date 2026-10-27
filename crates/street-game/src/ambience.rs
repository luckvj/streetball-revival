//! A court's ambient effects, from its `/COURTFX` script
//! ([`street_assets::courtfx`]): litter blowing across on the wind
//! (`DRIFT`, `WIND`), steam from vents (`PS_STEAM`), and the players'
//! breath in the cold (`COLD`). Also the hoop on fire after a gamebreaker
//! basket (the manager of type 0xf, 0x1f6fc8: its basket handler sets the
//! hoop alight, 0x1f6d98; a burst of up to 120 particles from the rim at
//! (±36.75, 11.96), 0x1f7178; out when the gamebreaker is over, 0x1f6f10),
//! with `fire.ssh` from `/XF/XF000003`.
//!
//! What is the game's: which courts have what, the litter's models, counts,
//! resting heights, sizes and spread (0x1f4248) and its motion (0x1f4950,
//! [`crate::drift`]), the vents' places and the way they blow, 40 particles
//! to a vent (0x20a4d0), six breathers (0x2f3b30) and the particle image
//! (`smoke.ssh` in `/XF/XF000003`). The steam's and the breath's motion is
//! not: their updates run on vector-unit code, so the speeds, lifetimes
//! and sizes here are chosen to look the part.

use std::{collections::BTreeMap, f32::consts::PI};

use bevy::prelude::*;
use street_assets::{
    big::Big,
    courtfx::{CourtFx, Steam, Wind},
    eagl,
    iso::Iso,
    ssh,
};
use street_render::animation::Rig;

use crate::{
    Screen,
    game::Paused,
    setup::{MouthJoint, Scene},
    view::{SidelineCamera, SimPlayer},
};

/// The litter, vents and breath a court has, read from the disc.
#[derive(Default)]
pub struct AmbientAssets {
    /// One kind of litter: its model and images, how many, whether it is
    /// the line's second kind (trash), and drawn size.
    litter: Vec<(eagl::Model, BTreeMap<String, ssh::Texture>, u32, bool, f32)>,
    half: [f32; 2],
    wind: Option<Wind>,
    steam: Vec<Steam>,
    cold: bool,
    /// The particle image (`smoke.ssh`, image `pclo`).
    smoke: Option<ssh::Texture>,
    /// The hoop fire's image (`fire.ssh`, image `ball`).
    fire: Option<ssh::Texture>,
    /// The gamebreaker ball's sparks and the pass trail
    /// ([`crate::ballfx`]).
    pub(crate) spark: Option<ssh::Texture>,
    pub(crate) motion: Option<ssh::Texture>,
    /// The gamebreaker ball's streamer.
    pub(crate) flag: Option<ssh::Texture>,
}

impl AmbientAssets {
    pub fn load<R: std::io::Read + std::io::Seek>(iso: &mut Iso<R>, fx: &CourtFx) -> Self {
        let mut out = AmbientAssets {
            wind: fx.wind(),
            steam: fx.steam(),
            cold: fx.cold(),
            ..default()
        };
        if let Some(drift) = fx.drift() {
            out.half = drift.half;
            match iso.read("/XF/XF000004") {
                Ok(bytes) => {
                    if let Ok(big) = Big::parse(&bytes) {
                        // The first kind is drawn at half size, the second
                        // at full size (0x1f4248).
                        for ((name, count), (second, scale)) in
                            drift.kinds.iter().zip([(false, 0.5), (true, 1.0)])
                        {
                            if *count == 0 {
                                continue;
                            }
                            let entry = |ext: &str| {
                                big.entries()
                                    .iter()
                                    .find(|e| e.name.eq_ignore_ascii_case(&format!("{name}.{ext}")))
                                    .map(|e| big.bytes(e))
                            };
                            let model = entry("o").and_then(|b| eagl::parse(b).ok());
                            let sheet = entry("ssh").and_then(|b| ssh::parse(b).ok());
                            let (Some(model), Some(sheet)) = (model, sheet) else {
                                warn!("litter {name}: not in /XF/XF000004");
                                continue;
                            };
                            let images = sheet
                                .textures
                                .into_iter()
                                .map(|t| (t.name.clone(), t))
                                .collect();
                            out.litter.push((model, images, *count, second, scale));
                        }
                    }
                }
                Err(e) => warn!("/XF/XF000004: {e}"),
            }
        }
        if let Ok(bytes) = iso.read("/XF/XF000003")
            && let Ok(big) = Big::parse(&bytes)
        {
            let image = |name: &str| {
                let e = big
                    .entries()
                    .iter()
                    .find(|e| e.name.eq_ignore_ascii_case(name))?;
                ssh::parse(big.bytes(e)).ok()?.textures.into_iter().next()
            };
            out.smoke = image("smoke.ssh");
            out.fire = image("fire.ssh");
            out.spark = image("spark.ssh");
            out.motion = image("motion.ssh");
            out.flag = image("flag.ssh");
        }
        out
    }
}

pub struct AmbiencePlugin;

impl Plugin for AmbiencePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(Screen::Playing),
            spawn_ambience.run_if(crate::options::effects_on),
        )
        .add_systems(
            FixedUpdate,
            (light_hoop, blow_litter.run_if(|p: Res<Paused>| !p.on))
                .after(crate::game::SimStep)
                .run_if(in_state(Screen::Playing))
                .run_if(crate::options::effects_on),
        )
        .add_systems(
            Update,
            (vent_steam, breathe, burn, fade_puffs)
                .chain()
                .run_if(in_state(Screen::Playing))
                .run_if(|p: Res<Paused>| !p.on)
                .run_if(crate::options::effects_on),
        );
    }
}

/// The court's litter ([`crate::drift`]), stepped once a frame.
#[derive(Resource)]
struct Litter(crate::drift::Drift);

/// A piece of litter on screen: its index in [`Litter`].
#[derive(Component)]
struct LitterPiece(usize);

/// One puff of steam or breath.
#[derive(Component)]
struct Puff {
    age: f32,
    life: f32,
    velocity: Vec3,
    size: (f32, f32),
    opacity: f32,
}

/// A vent, emitting at its own rate.
#[derive(Component)]
struct Vent {
    steam: Steam,
    next: f32,
}

/// The shared look of every puff, and the breathers' timers.
#[derive(Resource)]
struct Puffs {
    quad: Handle<Mesh>,
    image: Option<Handle<Image>>,
    fire: Option<Handle<Image>>,
    /// A hoop alight: its rim, and seconds to the next flame.
    burning: Option<(Vec3, f32)>,
    /// A burst to throw from this rim.
    burst: Option<Vec3>,
    wind: Vec3,
    cold: bool,
    breath: [f32; 6],
    rng: u32,
}

impl Puffs {
    fn roll(&mut self) -> f32 {
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Steam: 40 particles alive to a vent (0x20a4d0), each this long.
const STEAM_LIFE: f32 = 2.6;
const STEAM_PARTICLES: f32 = 40.0;

fn spawn_ambience(
    mut commands: Commands,
    scene: Res<Scene>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let a = &scene.ambience;
    let scoped = DespawnOnExit(Screen::Playing);
    let wind = a.wind.map_or(Vec3::ZERO, |w| {
        Vec3::new(w.direction[0], 0.0, w.direction[1]).normalize_or_zero() * w.speed
    });
    let mut puffs = Puffs {
        quad: meshes.add(Rectangle::new(1.0, 1.0)),
        image: a
            .smoke
            .as_ref()
            .map(|t| images.add(street_render::convert::image(t))),
        fire: a
            .fire
            .as_ref()
            .map(|t| images.add(street_render::convert::image(t))),
        burning: None,
        burst: None,
        wind,
        cold: a.cold,
        breath: [0.0; 6],
        rng: 0x002f_3b30 ^ scene.court.id.len() as u32,
    };
    for b in 0..6 {
        puffs.breath[b] = puffs.roll() * 3.0;
    }
    let count = |second: bool| {
        a.litter
            .iter()
            .filter(|l| l.3 == second)
            .map(|l| l.2)
            .sum::<u32>()
    };
    let drift = crate::drift::Drift::new(count(false), count(true), a.half, puffs.rng);
    for (i, piece) in drift.pieces.iter().enumerate() {
        let Some((model, textures, _, _, scale)) = a.litter.iter().find(|l| l.3 == piece.trash)
        else {
            continue;
        };
        let root = street_render::props::spawn(
            &mut commands,
            model,
            textures,
            Transform::from_translation(Vec3::from_array(piece.position))
                .with_scale(Vec3::splat(*scale)),
            &mut meshes,
            &mut materials,
            &mut images,
        );
        commands
            .entity(root)
            .insert((LitterPiece(i), scoped.clone()));
    }
    commands.insert_resource(Litter(drift));
    for steam in &a.steam {
        commands.spawn((
            Vent {
                steam: *steam,
                next: 0.0,
            },
            Transform::from_translation(Vec3::from_array(steam.at)),
            scoped.clone(),
        ));
    }
    commands.insert_resource(puffs);
}

/// One frame of the litter (`FUN_001f4950`), the court's camera as the
/// one whose view a piece may not come back in.
fn blow_litter(
    litter: Option<ResMut<Litter>>,
    camera: Query<(&GlobalTransform, &Projection), With<SidelineCamera>>,
    mut pieces: Query<(&LitterPiece, &mut Transform)>,
) {
    let Some(mut litter) = litter else { return };
    let camera = camera.single().ok().map(|(t, projection)| {
        let (fov, aspect, near, far) = match projection {
            Projection::Perspective(p) => (p.fov, p.aspect_ratio, p.near, p.far),
            _ => (std::f32::consts::FRAC_PI_4, 4.0 / 3.0, 0.5, 4000.0),
        };
        let tan_up = (fov * 0.5).tan();
        crate::drift::Camera {
            at: t.translation().to_array(),
            forward: t.forward().to_array(),
            right: t.right().to_array(),
            up: t.up().to_array(),
            tan_across: tan_up * aspect,
            tan_up,
            near,
            far,
        }
    });
    litter.0.step(camera.as_ref());
    for (piece, mut t) in &mut pieces {
        if let Some(p) = litter.0.pieces.get(piece.0) {
            t.translation = Vec3::from_array(p.position);
            t.rotation = Quat::from_array(p.rotation);
        }
    }
}

fn puff(
    commands: &mut Commands,
    puffs: &Puffs,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
    p: Puff,
) {
    puff_with(
        commands,
        puffs.quad.clone(),
        puffs.image.clone(),
        materials,
        at,
        p,
    );
}

fn puff_with(
    commands: &mut Commands,
    quad: Handle<Mesh>,
    image: Option<Handle<Image>>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
    p: Puff,
) {
    let material = materials.add(StandardMaterial {
        // The sprite is mid grey; added over the scene, brightened, it
        // reads as white steam on any court.
        base_color: Color::linear_rgba(1.6, 1.6, 1.6, 0.0),
        base_color_texture: image,
        alpha_mode: AlphaMode::Add,
        unlit: true,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        Mesh3d(quad),
        MeshMaterial3d(material),
        Transform::from_translation(at).with_scale(Vec3::splat(p.size.0)),
        p,
        DespawnOnExit(Screen::Playing),
    ));
}

fn vent_steam(
    mut commands: Commands,
    time: Res<Time>,
    puffs: Option<ResMut<Puffs>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut vents: Query<&mut Vent>,
) {
    let Some(mut puffs) = puffs else { return };
    let dt = time.delta_secs();
    for mut vent in &mut vents {
        vent.next -= dt;
        while vent.next <= 0.0 {
            vent.next += STEAM_LIFE / STEAM_PARTICLES;
            let blow = Vec3::from_array(vent.steam.blow);
            let jitter = Vec3::new(puffs.roll() - 0.5, 0.0, puffs.roll() - 0.5) * 0.6;
            let velocity = Vec3::Y * 4.0 + blow * 2.0 + puffs.wind * 0.1 + jitter * 2.0;
            let p = Puff {
                age: 0.0,
                life: STEAM_LIFE * (0.8 + 0.4 * puffs.roll()),
                velocity,
                size: (1.5, 7.0),
                opacity: 0.4,
            };
            let at = Vec3::from_array(vent.steam.at) + jitter;
            puff(&mut commands, &puffs, &mut materials, at, p);
        }
    }
}

/// In the cold every player breathes out now and then, a small cloud from
/// the mouth, the way they face.
fn breathe(
    mut commands: Commands,
    time: Res<Time>,
    puffs: Option<ResMut<Puffs>>,
    mouth: Option<Res<MouthJoint>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    players: Query<(&SimPlayer, &Rig, &GlobalTransform)>,
    joints: Query<&GlobalTransform>,
) {
    let (Some(mut puffs), Some(mouth)) = (puffs, mouth) else {
        return;
    };
    if !puffs.cold {
        return;
    }
    let dt = time.delta_secs();
    for (player, rig, body) in &players {
        let Some(b) = puffs.breath.get(player.0).copied() else {
            continue;
        };
        let left = b - dt;
        if left > 0.0 {
            puffs.breath[player.0] = left;
            continue;
        }
        puffs.breath[player.0] = 1.5 + puffs.roll() * 2.5;
        let Some(at) = rig
            .joints
            .get(mouth.0)
            .and_then(|&j| joints.get(j).ok())
            .map(|g| g.translation())
        else {
            continue;
        };
        let facing = body.rotation() * Vec3::NEG_Z;
        for _ in 0..5 {
            let spread =
                Vec3::new(puffs.roll() - 0.5, puffs.roll() - 0.5, puffs.roll() - 0.5) * 0.4;
            let p = Puff {
                age: 0.0,
                life: 0.7 + 0.4 * puffs.roll(),
                velocity: facing * 1.4 + Vec3::Y * 0.3 + spread,
                size: (0.5, 1.8),
                opacity: 0.85,
            };
            puff(&mut commands, &puffs, &mut materials, at + facing * 0.25, p);
        }
    }
}

/// A gamebreaker or Explosive Rims basket sets its hoop alight; play going live again puts it
/// out (the game's "gamebreaker over", slot 53).
fn light_hoop(
    sim: Res<crate::game::Sim>,
    tick: Res<crate::game::TickEvents>,
    replay: Res<crate::game::Replay>,
    cheats: Res<crate::setup::MatchCheats>,
    puffs: Option<ResMut<Puffs>>,
) {
    let Some(mut puffs) = puffs else { return };
    if tick.replay || replay.playing.is_some() {
        return;
    }
    let w = &sim.world;
    for e in &tick.events {
        if let street_sim::Event::Score { basket, .. } = *e
            && hoop_burst(basket, cheats.0.explosive_rims())
        {
            // The hoop the ball went through: the nearer rim.
            let ball = Vec3::from_array(w.ball.position);
            let rim = w
                .hoops
                .iter()
                .map(|h| Vec3::from_array(h.rim))
                .min_by(|a, b| a.distance(ball).total_cmp(&b.distance(ball)));
            if let Some(rim) = rim {
                puffs.burning = Some((rim, 0.0));
                puffs.burst = Some(rim);
            }
        }
    }
    if w.phase == street_sim::Phase::Live
        || matches!(w.phase, street_sim::Phase::Over { .. }) && puffs.burst.is_none()
    {
        puffs.burning = None;
    }
}

/// `FUN_001f6d98`: a made-basket callback activates the existing fire
/// particle manager when either the gamebreaker is active or selector
/// +0x14 is 1. The cheat does not itself authorize backboard destruction.
pub fn hoop_burst(basket: street_sim::Basket, explosive_rims: bool) -> bool {
    explosive_rims || matches!(basket, street_sim::Basket::Gamebreaker { .. })
}

/// Flames off a burning rim: a burst when it catches, then a steady fire.
fn burn(
    mut commands: Commands,
    time: Res<Time>,
    puffs: Option<ResMut<Puffs>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut puffs) = puffs else { return };
    let Some(image) = puffs.fire.clone() else {
        return;
    };
    let mut flame =
        |puffs: &mut Puffs, rim: Vec3, speed: f32, materials: &mut Assets<StandardMaterial>| {
            let a = puffs.roll() * 2.0 * PI;
            let edge = Vec3::new(a.cos(), 0.0, a.sin()) * crate::setup::RIM_RADIUS;
            let out = Vec3::new(puffs.roll() - 0.5, 0.0, puffs.roll() - 0.5);
            let p = Puff {
                age: 0.0,
                life: 0.5 + 0.5 * puffs.roll(),
                velocity: Vec3::Y * (2.0 + 4.0 * puffs.roll()) * speed + out * 3.0 * speed,
                size: (0.9, 0.3),
                opacity: 0.9,
            };
            puff_with(
                &mut commands,
                puffs.quad.clone(),
                Some(image.clone()),
                materials,
                rim + edge,
                p,
            );
        };
    if let Some(rim) = puffs.burst.take() {
        for _ in 0..100 {
            flame(&mut puffs, rim, 2.0, &mut materials);
        }
    }
    if let Some((rim, next)) = puffs.burning {
        let mut next = next - time.delta_secs();
        while next <= 0.0 {
            next += 1.0 / 40.0;
            flame(&mut puffs, rim, 1.0, &mut materials);
        }
        puffs.burning = Some((rim, next));
    }
}

/// Puffs drift, grow and fade, turned to the camera; gone when spent.
fn fade_puffs(
    mut commands: Commands,
    time: Res<Time>,
    camera: Query<&GlobalTransform, With<SidelineCamera>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut puffs: Query<(
        Entity,
        &mut Puff,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let dt = time.delta_secs();
    let facing = camera.single().map(|c| c.rotation()).unwrap_or_default();
    for (e, mut p, mut t, material) in &mut puffs {
        p.age += dt;
        let k = p.age / p.life;
        if k >= 1.0 {
            materials.remove(&material.0);
            commands.entity(e).despawn();
            continue;
        }
        t.translation += p.velocity * dt;
        p.velocity *= 1.0 - 0.4 * dt;
        t.rotation = facing;
        t.scale = Vec3::splat(p.size.0 + (p.size.1 - p.size.0) * k);
        // In quickly, out slowly.
        let alpha = p.opacity * (k / 0.1).min(1.0) * (1.0 - k);
        if let Some(mut m) = materials.get_mut(&material.0) {
            m.base_color.set_alpha(alpha);
        }
    }
}

#[cfg(test)]
mod explosive_tests {
    use super::*;
    use street_sim::Basket;

    #[test]
    fn explosive_rims_burst_for_every_made_basket_without_granting_gamebreaker_points() {
        for basket in [
            Basket::Jumper { long: false },
            Basket::Jumper { long: true },
            Basket::Dunk { advanced: true },
            Basket::Layup,
            Basket::AlleyOop { advanced: false },
        ] {
            assert!(!hoop_burst(basket, false));
            assert!(hoop_burst(basket, true));
        }
        assert!(hoop_burst(
            Basket::Gamebreaker {
                level: 2,
                long: false,
                dunk: false,
            },
            false
        ));
    }
}
