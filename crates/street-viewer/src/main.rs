//! Court viewer: renders an original game court read straight from your
//! disc image, optionally with a player playing one of the game's animations.
//!
//!     street-viewer [--iso <path>] [--court RUCKER] [--list] [--view <yaw>,<pitch>,<distance>]
//!                   [--focus <x>,<y>,<z>] [--screenshot <out.png>] [--hide <model>,...]
//!                   [--player <n>] [--body <model>] [--head <model>] [--anim <name>]
//!                   [--hires <Textures/GNZE69>]
//!
//! Mouse: drag to orbit, wheel to zoom. Keys: WASD/arrows move, Q/E down/up, Esc quits.
//! With `--screenshot` the viewer saves one frame and exits (for automated checks).

use std::path::PathBuf;

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit},
    mesh::skinning::SkinnedMeshInverseBindposes,
    prelude::*,
};
use street_assets::{court, iso::Iso, player};
use street_render::{
    StreetRenderPlugin,
    animation::{Animator, Clips},
    app::{self, ScreenshotPlugin},
    player::{Choice, PlayerAssets},
};

struct Args {
    iso: Option<PathBuf>,
    court: String,
    list: bool,
    view: Option<(f32, f32, f32)>,
    focus: Vec3,
    screenshot: Option<PathBuf>,
    hide: Vec<String>,
    player: Option<Choice>,
    anim: Option<String>,
    hires: Option<PathBuf>,
}

/// The player options as given; turned into a [`Choice`] once all are read.
#[derive(Default)]
struct PlayerArgs {
    index: Option<usize>,
    home: bool,
    body: Option<String>,
    head: Option<String>,
    anim: bool,
}

impl PlayerArgs {
    fn choice(self) -> Option<Choice> {
        let side = if self.home {
            player::Side::Home
        } else {
            player::Side::Road
        };
        match (self.body, self.head) {
            (None, None) if self.index.is_some() || self.anim || self.home => {
                Some(Choice::Record {
                    index: self.index.unwrap_or(0),
                    side,
                })
            }
            (None, None) => None,
            (body, head) => Some(Choice::Kit {
                textures: self.index.unwrap_or(0),
                body: body.unwrap_or_else(|| "body0_outfit6".into()),
                head: head.unwrap_or_else(|| "malehair02_head00_body0".into()),
            }),
        }
    }
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        iso: None,
        court: "RUCKER".to_owned(),
        list: false,
        view: None,
        focus: Vec3::ZERO,
        screenshot: None,
        hide: Vec::new(),
        player: None,
        anim: None,
        hires: None,
    };
    let floats = |s: String| -> Result<Vec<f32>, String> {
        s.split(',')
            .map(|v| v.trim().parse::<f32>().map_err(|e| e.to_string()))
            .collect()
    };
    let mut player = PlayerArgs::default();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--iso" => args.iso = Some(value()?.into()),
            "--court" => args.court = value()?.to_ascii_uppercase(),
            "--list" => args.list = true,
            "--screenshot" => args.screenshot = Some(value()?.into()),
            "--hide" => args
                .hide
                .extend(value()?.split(',').map(|m| m.trim().to_ascii_lowercase())),
            "--player" => {
                player.index = Some(value()?.parse().map_err(|e| format!("--player: {e}"))?);
            }
            "--home" => player.home = true,
            "--hires" => args.hires = Some(value()?.into()),
            "--body" => player.body = Some(value()?),
            "--head" => player.head = Some(value()?),
            "--anim" => {
                player.anim = true;
                args.anim = Some(value()?);
            }
            "--view" => {
                let [yaw, pitch, dist] = floats(value()?)?[..] else {
                    return Err("--view takes yaw,pitch,distance".into());
                };
                args.view = Some((yaw, pitch, dist));
            }
            "--focus" => {
                let [x, y, z] = floats(value()?)?[..] else {
                    return Err("--focus takes x,y,z".into());
                };
                args.focus = Vec3::new(x, y, z);
            }
            "-h" | "--help" => return Err(include_str!("usage.txt").into()),
            other => {
                return Err(format!(
                    "unknown argument {other:?}\n{}",
                    include_str!("usage.txt")
                ));
            }
        }
    }
    args.player = player.choice();
    Ok(args)
}

#[derive(Resource)]
struct Scene {
    court: court::Court,
    player: Option<(PlayerAssets, Option<String>)>,
}

#[derive(Resource)]
struct InitialView {
    focus: Vec3,
    yaw_deg: f32,
    pitch_deg: f32,
    distance: f32,
}

#[derive(Component)]
struct Orbit {
    focus: Vec3,
    yaw: f32,
    pitch: f32,
    distance: f32,
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    };
    if let Err(e) = run(args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let iso_path = match args.iso {
        Some(p) => p,
        None => app::find_iso()?,
    };
    let mut iso = Iso::open(&iso_path).map_err(|e| format!("{}: {e}", iso_path.display()))?;
    if args.list {
        for id in court::court_ids(&mut iso)? {
            println!("{id}");
        }
        return Ok(());
    }
    if let Some(dir) = &args.hires {
        let pack = street_render::hires::set_pack(dir)
            .map_err(|e| format!("--hires {}: {e}", dir.display()))?;
        println!(
            "hires: {} of the pack's {} images stand in for PS2 images",
            pack.used().len(),
            pack.len()
        );
    }
    let mut court = court::load(&mut iso, &args.court)?;
    court.models.retain(|m| !args.hide.contains(&m.name));
    report(&court);
    let player = match &args.player {
        Some(choice) => {
            let assets = PlayerAssets::load(&mut iso, choice)?;
            println!(
                "player: {}, scale {:.3}, {} bones",
                assets.name,
                assets.scale,
                assets.skeleton.bones.len()
            );
            Some((assets, args.anim.clone()))
        }
        None => None,
    };
    let clips = match &args.anim {
        Some(name) => Clips::load(&mut iso, &[name])?,
        None => Clips::default(),
    };

    let (yaw, pitch, distance) = args.view.unwrap_or((35.0, 28.0, 140.0));
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(app::window_plugin(format!(
            "the original game - {}",
            court.id
        ))),
        StreetRenderPlugin,
    ))
    .insert_resource(ClearColor(Color::srgb(0.53, 0.70, 0.86)))
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .insert_resource(Scene { court, player })
    .insert_resource(clips)
    .insert_resource(InitialView {
        focus: args.focus,
        yaw_deg: yaw,
        pitch_deg: pitch,
        distance,
    })
    .add_systems(Startup, (spawn_scene, spawn_camera, app::spawn_sun))
    .add_systems(Update, app::quit_on_escape);
    match args.screenshot {
        // A screenshot run ignores the mouse and keyboard so the capture matches --view.
        Some(path) => {
            app.add_plugins(ScreenshotPlugin {
                path,
                delay_secs: 10.0,
            });
        }
        None => {
            app.add_systems(Update, orbit_camera);
        }
    }
    app.run();
    Ok(())
}

fn report(c: &court::Court) {
    let tris: usize = c.models.iter().map(|m| m.stats.triangles).sum();
    let skipped: usize = c.models.iter().map(|m| m.stats.skipped_batches).sum();
    let batches: usize = c.models.iter().map(|m| m.stats.batches).sum();
    println!(
        "{}: {} models, {} triangles, {} textures; {skipped}/{batches} batches in a vertex format not decoded yet",
        c.id,
        c.models.len(),
        tris,
        c.textures.len()
    );
    if !c.skipped_models.is_empty() {
        println!("not drawn yet: {:?} (hoop shadows)", c.skipped_models);
    }
    let missing = c.missing_textures();
    if !missing.is_empty() {
        println!("textures referenced but not found: {missing:?}");
    }
}

fn spawn_scene(
    mut commands: Commands,
    scene: Res<Scene>,
    clips: Res<Clips>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
) {
    street_render::court::spawn(
        &mut commands,
        &scene.court,
        &mut meshes,
        &mut materials,
        &mut images,
    );
    let Some((assets, anim)) = &scene.player else {
        return;
    };
    let clip = anim.as_deref().and_then(|name| clips.get(name));
    // An animation's root motion holds the hips at standing height above an
    // origin on the floor; the rest pose's origin is at the hips.
    let lift = if clip.is_some() {
        0.0
    } else {
        assets.hip_height()
    };
    let player = street_render::player::spawn(
        &mut commands,
        assets,
        Transform::from_xyz(0.0, lift, 0.0),
        &mut meshes,
        &mut materials,
        &mut images,
        &mut bindposes,
    );
    if let Some(clip) = clip {
        commands.entity(player).insert(Animator::new(clip));
    }
}

fn spawn_camera(mut commands: Commands, view: Res<InitialView>) {
    let orbit = Orbit {
        focus: view.focus,
        yaw: view.yaw_deg.to_radians(),
        pitch: view.pitch_deg.to_radians(),
        distance: view.distance,
    };
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 50f32.to_radians(),
            near: 0.5,
            far: 5000.0,
            ..default()
        }),
        orbit_transform(&orbit),
        orbit,
    ));
}

fn orbit_transform(o: &Orbit) -> Transform {
    let offset = Vec3::new(
        o.pitch.cos() * o.yaw.sin(),
        o.pitch.sin(),
        o.pitch.cos() * o.yaw.cos(),
    ) * o.distance;
    Transform::from_translation(o.focus + offset).looking_at(o.focus, Vec3::Y)
}

fn orbit_camera(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    mut cameras: Query<(&mut Orbit, &mut Transform)>,
) {
    for (mut o, mut transform) in &mut cameras {
        if buttons.pressed(MouseButton::Left) {
            o.yaw -= motion.delta.x * 0.005;
            o.pitch = (o.pitch + motion.delta.y * 0.005).clamp(-1.4, 1.5);
        }
        let notches = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
        };
        o.distance = (o.distance * (1.0 - notches * 0.1)).clamp(5.0, 2500.0);

        let forward = Vec3::new(-o.yaw.sin(), 0.0, -o.yaw.cos());
        let right = Vec3::new(o.yaw.cos(), 0.0, -o.yaw.sin());
        let mut step = Vec3::ZERO;
        for (codes, dir) in [
            ([KeyCode::KeyW, KeyCode::ArrowUp], forward),
            ([KeyCode::KeyS, KeyCode::ArrowDown], -forward),
            ([KeyCode::KeyD, KeyCode::ArrowRight], right),
            ([KeyCode::KeyA, KeyCode::ArrowLeft], -right),
            ([KeyCode::KeyE, KeyCode::PageUp], Vec3::Y),
            ([KeyCode::KeyQ, KeyCode::PageDown], -Vec3::Y),
        ] {
            if keys.any_pressed(codes) {
                step += dir;
            }
        }
        let speed = o.distance.max(20.0) * time.delta_secs();
        o.focus += step.normalize_or_zero() * speed;
        *transform = orbit_transform(&o);
    }
}
