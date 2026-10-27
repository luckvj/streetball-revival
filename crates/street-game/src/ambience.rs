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
//! (`smoke.ssh` in `/XF/XF000003`), the steam's (0x20a5c8,
//! [`crate::steam`]) and the breath's (0x1f07c0, [`crate::breath`]).

use std::collections::BTreeMap;

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
    setup::Scene,
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
            (
                light_hoop,
                (blow_litter, vent_steam, breathe, burn).run_if(|p: Res<Paused>| !p.on),
            )
                .after(crate::game::SimStep)
                .run_if(in_state(Screen::Playing))
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

/// A vent, emitting at its own rate.
#[derive(Component)]
struct Vent(crate::steam::Vent);

/// One of a vent's puffs on screen: its vent and index.
#[derive(Component)]
struct SteamPuff(Entity, usize);

/// One of a player's breath puffs on screen: the player and index.
#[derive(Component)]
struct BreathPuff(usize, usize);

/// One of the hoop fireworks' particles on screen.
#[derive(Component)]
struct FirePiece(usize);

/// The shared look of every puff, and the breathers' timers.
#[derive(Resource)]
struct Puffs {
    quad: Handle<Mesh>,
    image: Option<Handle<Image>>,
    fire: Option<Handle<Image>>,
    /// The hoop's fireworks ([`crate::fire`]).
    flames: crate::fire::Fire,
    /// The court's wind in feet a frame, as the steam reads it.
    court_wind: [f32; 3],
    /// A breather a player (`0x2f45ec..0x2f462c`), in the cold.
    breathers: Vec<crate::breath::Breather>,
    rng: u32,
}

fn spawn_ambience(
    mut commands: Commands,
    scene: Res<Scene>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let a = &scene.ambience;
    let scoped = DespawnOnExit(Screen::Playing);
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
        flames: crate::fire::Fire::new(0x001f_6fc8 ^ scene.court.id.len() as u32),
        court_wind: a.wind.map_or([0.0; 3], |w| {
            crate::steam::court_wind(w.speed, w.direction[0], w.direction[1])
        }),
        breathers: Vec::new(),
        rng: 0x002f_3b30 ^ scene.court.id.len() as u32,
    };
    if a.cold {
        for k in 0..6 {
            let seed = puffs.rng ^ (k as u32 + 7).wrapping_mul(0x85eb_ca6b);
            puffs.breathers.push(crate::breath::Breather::new(seed));
            for i in 0..crate::breath::PUFFS {
                let material = materials.add(StandardMaterial {
                    base_color: Color::linear_rgba(1.0, 1.0, 1.0, 0.0),
                    base_color_texture: puffs.image.clone(),
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    cull_mode: None,
                    ..default()
                });
                commands.spawn((
                    Mesh3d(puffs.quad.clone()),
                    MeshMaterial3d(material),
                    Transform::from_scale(Vec3::splat(crate::breath::SIZE)),
                    Visibility::Hidden,
                    BreathPuff(k, i),
                    scoped.clone(),
                ));
            }
        }
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
    for i in 0..crate::fire::PARTICLES {
        let material = materials.add(StandardMaterial {
            base_color: Color::linear_rgba(1.0, 1.0, 1.0, 0.0),
            base_color_texture: puffs.fire.clone(),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            cull_mode: None,
            ..default()
        });
        commands.spawn((
            Mesh3d(puffs.quad.clone()),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::Hidden,
            FirePiece(i),
            scoped.clone(),
        ));
    }
    for (k, steam) in a.steam.iter().enumerate() {
        let vent = commands
            .spawn((
                Vent(crate::steam::Vent::new(
                    steam.at,
                    steam.blow,
                    puffs.rng ^ (k as u32 + 1).wrapping_mul(0x9e37_79b9),
                )),
                Transform::from_translation(Vec3::from_array(steam.at)),
                scoped.clone(),
            ))
            .id();
        for i in 0..crate::steam::PUFFS {
            let material = materials.add(StandardMaterial {
                base_color: Color::linear_rgba(1.0, 1.0, 1.0, 0.0),
                base_color_texture: puffs.image.clone(),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                ..default()
            });
            commands.spawn((
                Mesh3d(puffs.quad.clone()),
                MeshMaterial3d(material),
                Transform::from_translation(Vec3::from_array(steam.at))
                    .with_scale(Vec3::splat(crate::steam::SIZE)),
                Visibility::Hidden,
                SteamPuff(vent, i),
                scoped.clone(),
            ));
        }
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

/// One frame of each vent (`FUN_0020a5c8`) in the court's wind, and its
/// puffs drawn where it puts them: white, the engine's alpha (128 is 1),
/// turned to the camera.
fn vent_steam(
    puffs: Option<Res<Puffs>>,
    camera: Query<&GlobalTransform, With<SidelineCamera>>,
    mut vents: Query<&mut Vent>,
    mut sprites: Query<(
        &SteamPuff,
        &mut Transform,
        &mut Visibility,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(puffs) = puffs else { return };
    let wind = puffs.court_wind;
    for mut vent in &mut vents {
        vent.0.step(wind);
    }
    let facing = camera.single().map(|c| c.rotation()).unwrap_or_default();
    for (sprite, mut t, mut shown, material) in &mut sprites {
        let Ok(vent) = vents.get(sprite.0) else {
            continue;
        };
        let p = vent.0.puffs[sprite.1];
        if p.life == 0 {
            *shown = Visibility::Hidden;
            continue;
        }
        *shown = Visibility::Inherited;
        t.translation = Vec3::from_array(p.position);
        t.rotation = facing;
        if let Some(mut m) = materials.get_mut(&material.0) {
            m.base_color.set_alpha((p.alpha / 128.0).min(1.0));
        }
    }
}

/// In the cold each player's breather (`FUN_001f07c0`) steps once a frame
/// from his head bone (21): its place and its up axis turned. Its puffs are
/// drawn where it puts them, white, the engine's alpha (128 is 1), turned
/// to the camera.
fn breathe(
    puffs: Option<ResMut<Puffs>>,
    players: Query<(&SimPlayer, &Rig)>,
    joints: Query<&GlobalTransform>,
    camera: Query<&GlobalTransform, With<SidelineCamera>>,
    mut sprites: Query<(
        &BreathPuff,
        &mut Transform,
        &mut Visibility,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut puffs) = puffs else { return };
    if puffs.breathers.is_empty() {
        return;
    }
    let wind = puffs.court_wind;
    for (player, rig) in &players {
        let Some(head) = rig
            .joints
            .get(crate::breath::HEAD)
            .and_then(|&j| joints.get(j).ok())
        else {
            continue;
        };
        let at = head.translation().to_array();
        let up = (head.rotation() * Vec3::Y).to_array();
        if let Some(b) = puffs.breathers.get_mut(player.0) {
            b.step(at, up, wind);
        }
    }
    let facing = camera.single().map(|c| c.rotation()).unwrap_or_default();
    for (sprite, mut t, mut shown, material) in &mut sprites {
        let Some(p) = puffs.breathers.get(sprite.0).map(|b| b.puffs[sprite.1]) else {
            continue;
        };
        if p.life == 0 {
            *shown = Visibility::Hidden;
            continue;
        }
        *shown = Visibility::Inherited;
        t.translation = Vec3::from_array(p.position);
        t.rotation = facing;
        if let Some(mut m) = materials.get_mut(&material.0) {
            m.base_color.set_alpha((p.alpha / 128.0).min(1.0));
        }
    }
}

/// The basket's listener (slot 26, `FUN_001f6d98`) lights the fireworks
/// for a basket that wins the game, or any with Explosive Rims; a replay's
/// start (slot 58, `FUN_001f6ec8`) puts them out (the game's reset, slot
/// 53, is a new match's spawn).
fn light_hoop(
    sim: Res<crate::game::Sim>,
    tick: Res<crate::game::TickEvents>,
    replay: Res<crate::game::Replay>,
    cheats: Res<crate::setup::MatchCheats>,
    puffs: Option<ResMut<Puffs>>,
    mut replaying: Local<bool>,
) {
    let Some(mut puffs) = puffs else { return };
    let now = tick.replay || replay.playing.is_some();
    if now && !*replaying {
        puffs.flames.out();
    }
    *replaying = now;
    if now {
        return;
    }
    let w = &sim.world;
    let won = matches!(w.phase, street_sim::Phase::Over { .. });
    for e in &tick.events {
        if matches!(e, street_sim::Event::Score { .. })
            && hoop_burst(won, cheats.0.explosive_rims())
        {
            puffs.flames.light(w.ball.position[0]);
        }
    }
}

/// `FUN_001f6d98`'s gate: the basket wins the game (the game's vt+0xbc,
/// `0x1f6dd0..0x1f6e08`) or the Explosive Rims cheat is on (+0x14). The
/// cheat does not itself authorize backboard destruction.
pub fn hoop_burst(won: bool, explosive_rims: bool) -> bool {
    won || explosive_rims
}

/// One frame of the fireworks (`FUN_001f7178`) in the court's wind, drawn
/// added: the engine's colour and alpha, 128 is 1.
fn burn(
    puffs: Option<ResMut<Puffs>>,
    camera: Query<&GlobalTransform, With<SidelineCamera>>,
    mut sprites: Query<(
        &FirePiece,
        &mut Transform,
        &mut Visibility,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut puffs) = puffs else { return };
    let wind = puffs.court_wind;
    puffs.flames.step(wind);
    let facing = camera.single().map(|c| c.rotation()).unwrap_or_default();
    for (piece, mut t, mut shown, material) in &mut sprites {
        let p = puffs.flames.particles[piece.0];
        if p.life == 0 || p.size <= 0.0 {
            *shown = Visibility::Hidden;
            continue;
        }
        *shown = Visibility::Inherited;
        t.translation = Vec3::from_array(p.position);
        t.rotation = facing;
        t.scale = Vec3::splat(p.size);
        if let Some(mut m) = materials.get_mut(&material.0) {
            let [r, g, b, a] = p.colour.map(|c| c.max(0.0) / 128.0);
            let a = a.min(1.0);
            m.base_color = Color::linear_rgba(r * a, g * a, b * a, a);
        }
    }
}

#[cfg(test)]
mod explosive_tests {
    use super::*;

    #[test]
    fn the_winning_basket_or_explosive_rims_light_the_hoop() {
        // `FUN_001f6d98`: any basket with the cheat, else only the one
        // that wins the game (a gamebreaker's included, no more).
        assert!(hoop_burst(true, false));
        assert!(hoop_burst(false, true));
        assert!(!hoop_burst(false, false));
    }
}
