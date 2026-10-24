//! The game's replays as the game draws them: the original's replay
//! manager, its director, the gamebreaker task's two slowed replays and a
//! staged Level 2 script's own replay (mode 7, on the script's cameras,
//! `view::step_camera`) are the simulation's (`street_sim::replay`, where
//! their behaviour is noted), which decides when a replay plays, holds the
//! gameplay, and which updates it shows; this module records what the game
//! draws at each update and shows those frames.
//!
//! Played alone, [`step`] runs the three after the simulation's step, on the
//! live world, with the people's pads' presses. Online the match runs them
//! on every machine (`street-net`'s `Match`), so every machine holds the
//! same updates while the server's clock runs on; [`step`] mirrors the
//! predicted match's three into the resources here, for the HUD and the
//! camera, and draws.
//!
//! The rewrite records what it draws: the world, each player's place,
//! rotation, animator and joint transforms, and the ball's place
//! ([`Frame`]), keyed by the world's tick. The renderer shows the blend of
//! the frames of the two updates the manager shows ([`Tape::shown`],
//! `view::sync_players`, `view::sync_ball`, [`pose_bones`]). While a replay
//! plays the recorded world is on screen; at its end the live world comes
//! back, with the newest recorded update's players and ball (the recorded
//! objects `FUN_00329718` writes back).

use std::collections::VecDeque;

use bevy::prelude::*;
use street_render::animation::{Animator, Rig};
use street_sim::World as SimWorld;
pub use street_sim::replay::{
    FRAMES, GAMEBREAKER_DELAY, GAMEBREAKER_GAP, GAMEBREAKER_SPAN, GAMEBREAKER_SPEEDS,
    INSTANT_DELAY, INSTANT_SPEED, LEAD_IN, MOMENT_SPEED, OFFER_RECORD, PROMPT_HOLD, REPEAT_SPEED,
    SLOW_MOTION_BELOW, WON_DELAY, mode,
};
use street_sim::replay::{Manager, Press, StagedReplay};

use crate::{
    controls::Control,
    game::{Replay, Sim, TickEvents},
    view::{BallView, SimPlayer},
};

/// One player as drawn.
#[derive(Clone)]
pub struct Pose {
    pub translation: Vec3,
    pub rotation: Quat,
    pub animator: Option<Animator>,
    /// The rig's joints, as posed, by bone.
    pub bones: Vec<Transform>,
}

/// One drawn update.
#[derive(Clone)]
pub struct Frame {
    pub world: SimWorld,
    /// By player index.
    pub poses: Vec<Option<Pose>>,
    pub ball: Option<Vec3>,
}

/// The replay manager (`street_sim::replay::Manager`, which it derefs
/// to) and what the game drew.
#[derive(Resource, Default)]
pub struct Tape {
    /// The manager: played alone the game's; online the predicted match's,
    /// mirrored.
    pub control: Manager,
    /// The script manager replaying a staged script (mode 7): its camera
    /// and frame, the same played alone or mirrored.
    pub staged: Option<StagedReplay>,
    /// What was drawn, oldest first, one frame a tick (the newest at a
    /// tick drawn twice).
    frames: VecDeque<Frame>,
    /// The live world while a replay shows a recorded one.
    live: Option<SimWorld>,
}

impl std::ops::Deref for Tape {
    type Target = Manager;

    fn deref(&self) -> &Manager {
        &self.control
    }
}

impl std::ops::DerefMut for Tape {
    fn deref_mut(&mut self) -> &mut Manager {
        &mut self.control
    }
}

impl Tape {
    /// The gameplay holds this update: a replay plays, or a staged script
    /// ends after its replay (`street_sim::replay::Replays::holds_world`).
    pub fn holds_world(&self) -> bool {
        self.control.busy() || self.staged.is_some()
    }

    /// The live world while a replay shows a recorded one.
    pub fn live(&self) -> Option<&SimWorld> {
        self.live.as_ref()
    }

    /// The frame drawn at `tick`, or failing that the newest before it
    /// (online, a tick the game stepped past without drawing).
    fn frame(&self, tick: u64) -> Option<&Frame> {
        match self.frames.partition_point(|f| f.world.tick <= tick) {
            0 => self.frames.front(),
            at => self.frames.get(at - 1),
        }
    }

    /// The frames on screen: the manager's two shown updates and the
    /// fraction between them (`FUN_00329718`; at the newest, only it).
    pub fn shown(&self) -> Option<(&Frame, &Frame, f32)> {
        let (a, b, t) = self.control.shown()?;
        Some((self.frame(a.tick)?, self.frame(b.tick)?, t))
    }

    /// Keep what was drawn at the frame's tick, as far back as the
    /// manager's ring reaches.
    fn draw(&mut self, frame: Frame) {
        let tick = frame.world.tick;
        // A tick drawn again (online, after a rollback) replaces what was
        // drawn from it on.
        if let Some(i) = self.frames.iter().position(|f| f.world.tick >= tick) {
            self.frames.truncate(i);
        }
        self.frames.push_back(frame);
        let oldest = self.control.slots().next().map_or(tick, |s| s.tick);
        while self.frames.len() > 2 * FRAMES
            || self.frames.len() > 1 && self.frames[1].world.tick <= oldest
        {
            self.frames.pop_front();
        }
    }
}

/// The director (`street_sim::replay::Director`): played alone the game's,
/// online the predicted match's, mirrored.
#[derive(Resource, Default, Deref, DerefMut)]
pub struct Director(pub street_sim::replay::Director);

/// The gamebreaker task (`street_sim::replay::GamebreakerTask`): played
/// alone the game's, online the predicted match's, mirrored.
#[derive(Resource, Default, Deref, DerefMut)]
pub struct GamebreakerTask(pub street_sim::replay::GamebreakerTask);

/// One update of the replays after the simulation's step: played alone,
/// the director, manager and gamebreaker task on the live world (with the
/// people's presses); online, the predicted match's mirrored. Then what the
/// game draws: this update's frame recorded, and while a replay plays the
/// frame it shows put on screen.
#[allow(clippy::too_many_arguments)]
pub fn step(
    mut sim: ResMut<Sim>,
    (mut tape, mut task, mut director, mut replay): (
        ResMut<Tape>,
        ResMut<GamebreakerTask>,
        ResMut<Director>,
        ResMut<Replay>,
    ),
    mut tick_events: ResMut<TickEvents>,
    (setup, options, control, running, online): (
        Option<Res<crate::setup::MatchSetup>>,
        Option<Res<crate::options::Options>>,
        Option<Res<Control>>,
        Option<Res<crate::story::Running>>,
        Option<Res<crate::online::Online>>,
    ),
    (mut last_tick, mut held): (Local<Option<u64>>, Local<Vec<u16>>),
    players: Query<(&SimPlayer, &Transform, Option<&Animator>, Option<&Rig>)>,
    joints: Query<&Transform, Without<SimPlayer>>,
    balls: Query<&Transform, (With<BallView>, Without<SimPlayer>)>,
) {
    let draws = |world: &SimWorld| Frame {
        world: world.clone(),
        poses: {
            let mut poses = vec![None; world.players.len()];
            for (index, transform, animator, rig) in &players {
                if let Some(slot) = poses.get_mut(index.0) {
                    let bones = rig.map_or_else(Vec::new, |rig| {
                        rig.joints
                            .iter()
                            .map(|&j| joints.get(j).copied().unwrap_or_default())
                            .collect()
                    });
                    *slot = Some(Pose {
                        translation: transform.translation,
                        rotation: transform.rotation,
                        animator: animator.cloned(),
                        bones,
                    });
                }
            }
            poses
        },
        ball: balls.iter().next().map(|t| t.translation),
    };
    let net = setup.as_ref().is_some_and(|s| s.online);
    if net {
        // Online: the match's own (every machine runs them), as the
        // prediction stands.
        let Some(m) = online
            .as_ref()
            .and_then(|o| o.client())
            .and_then(|c| c.session())
            .map(|s| s.current())
        else {
            return;
        };
        tape.control = m.replays.manager.clone();
        tape.staged = m.replays.staged;
        director.0 = m.replays.director.clone();
        task.0 = m.replays.task;
        if tape.started {
            info!(
                "replay: mode {} at {} from tick {} (match tick {})",
                tape.mode,
                tape.speed(),
                tape.control.shown().map_or(0, |(a, _, _)| a.tick),
                m.tick()
            );
        }
        if tape.busy() {
            if let Some((a, _, _)) = tape.shown() {
                sim.world = a.world.clone();
            }
        } else if *last_tick != Some(sim.world.tick) {
            let frame = draws(&sim.world);
            tape.draw(frame);
            *last_tick = Some(sim.world.tick);
        }
    } else {
        let stepped = last_tick.is_none_or(|t| t != sim.world.tick) && !tape.holds_world();
        let lesson = running
            .as_ref()
            .is_some_and(|r| matches!(r.0, Some(crate::story::Run::School(_))));
        let option = options.as_ref().map_or(1, |o| o.0.replays);
        // The director's basket roll reads the live option inside the
        // simulation's update (Event::ReplayRoll).
        sim.world.replays = option;
        // The people's fresh presses (FUN_0012f2c8).
        let mut presses = Vec::new();
        if let Some(control) = control.as_deref().filter(|c| !c.demo) {
            held.resize(control.seats.len(), 0);
            for (k, seat) in control.seats.iter().enumerate() {
                let pressed = seat.raw.buttons & !held[k];
                held[k] = seat.raw.buttons;
                presses.push(Press {
                    team: seat.team,
                    buttons: pressed,
                    other_human: sim
                        .world
                        .teams
                        .get(1 - seat.team.min(1))
                        .is_some_and(|t| t.human),
                });
            }
        }
        {
            // On the live world: the one on screen, or the one a replay
            // holds aside.
            let tape = &mut *tape;
            let live = match &mut tape.live {
                Some(live) => live,
                None => &mut sim.world,
            };
            street_sim::replay::update(
                &mut tape.control,
                &mut director.0,
                &mut task.0,
                &mut tape.staged,
                live,
                &mut tick_events.events,
                stepped,
                &presses,
                option,
                lesson,
            );
        }
        if tape.recorded_now {
            let frame = draws(&sim.world);
            tape.draw(frame);
        }
        if tape.started {
            info!(
                "replay: mode {} at {} from tick {}",
                tape.mode,
                tape.speed(),
                tape.shown().map_or(0, |(f, _, _)| f.world.tick)
            );
        }
        if tape.busy() {
            // The live game held while the replay plays, the recorded one
            // shown.
            if tape.live.is_none() {
                tape.live = Some(sim.world.clone());
            }
            if let Some((a, _, _)) = tape.shown() {
                sim.world = a.world.clone();
            }
        } else if tape.ended {
            info!("replay over: mode {}", tape.mode);
            // The live world back, its recorded objects restored and a
            // pending restart snapped (`street_sim::replay::update`).
            if let Some(live) = tape.live.take() {
                sim.world = live;
            }
        }
        *last_tick = Some(sim.world.tick);
    }
    // Systems that keep out of replays.
    let playing = tape
        .replaying()
        .then_some(crate::game::Playing { mode: tape.mode });
    if replay.playing != playing {
        replay.playing = playing;
    }
}

/// The recorded joints, blended per bone (`FUN_003275d8`: each of the
/// bones slerped, the packed vectors lerped), over the animator's pose
/// while a replay plays.
pub fn pose_bones(
    tape: Option<Res<Tape>>,
    players: Query<(&SimPlayer, &Rig)>,
    mut joints: Query<&mut Transform, Without<SimPlayer>>,
) {
    let Some((a, b, t)) = tape.as_deref().and_then(Tape::shown) else {
        return;
    };
    for (index, rig) in &players {
        let Some(Some(from)) = a.poses.get(index.0) else {
            continue;
        };
        let to = b
            .poses
            .get(index.0)
            .and_then(Option::as_ref)
            .unwrap_or(from);
        for (bone, &joint) in rig.joints.iter().enumerate() {
            let Some(x) = from.bones.get(bone) else {
                continue;
            };
            let y = to.bones.get(bone).unwrap_or(x);
            if let Ok(mut transform) = joints.get_mut(joint) {
                transform.translation = x.translation.lerp(y.translation, t);
                transform.rotation = x.rotation.slerp(y.rotation, t);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(tick: u64) -> Frame {
        let mut world = SimWorld::new(
            [street_sim::Hoop {
                rim: [-37.0, 11.75, 0.0],
                rim_radius: 0.75,
            }; 2],
            Vec::new(),
            street_sim::Rules::default(),
            1,
        );
        world.tick = tick;
        Frame {
            world,
            poses: Vec::new(),
            ball: None,
        }
    }

    /// Through the match's systems: `step` records each rig's joints, and
    /// while the replay plays `pose_bones` slerps them bone by bone between
    /// the frames either side of the position (`FUN_003275d8`).
    #[test]
    fn recorded_joints_blend_per_bone_like_fun_003275d8() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        let player = street_sim::Player::new(0, [0.0, 0.0], 0.0);
        let mut sim = frame(0).world;
        sim.players = vec![player];
        app.insert_resource(Sim {
            world: sim,
            inputs: Vec::new(),
            call: String::new(),
            pass_style: Vec::new(),
        })
        .init_resource::<Tape>()
        .init_resource::<GamebreakerTask>()
        .init_resource::<Director>()
        .init_resource::<Replay>()
        .init_resource::<TickEvents>();
        let world = app.world_mut();
        let joints: Vec<Entity> = (0..2)
            .map(|_| world.spawn(Transform::default()).id())
            .collect();
        world.spawn((
            SimPlayer(0),
            Transform::default(),
            Rig::new(joints.clone(), &street_assets::eagl::Skeleton::default()),
        ));
        // Each update the elbow turns a further 0.1 about y, and moves on x.
        for k in 0..100 {
            *world.get_mut::<Transform>(joints[1]).unwrap() = Transform {
                translation: Vec3::new(k as f32, 0.0, 0.0),
                rotation: Quat::from_rotation_y(0.1 * k as f32),
                ..default()
            };
            world.resource_mut::<Sim>().world.tick = k + 1;
            world.run_system_once(step).unwrap();
        }
        assert_eq!(world.resource::<Tape>().recorded(), 100);
        world.resource_mut::<Tape>().request(
            0.5,
            20,
            0,
            mode::SHOT,
            street_sim::replay::Gates::default(),
        );
        world.run_system_once(step).unwrap();
        // One more recorded (the elbow still at 99), then 0.5 into the span.
        let (a, b, t) = {
            let tape = world.resource::<Tape>();
            let (a, b, t) = tape.shown().unwrap();
            let bone = |f: &Frame| f.poses[0].as_ref().unwrap().bones[1];
            (bone(a), bone(b), t)
        };
        assert_eq!(t, 0.5);
        assert_eq!(b.translation.x, a.translation.x + 1.0);
        world.run_system_once(pose_bones).unwrap();
        let shown = *world.get::<Transform>(joints[1]).unwrap();
        assert!((shown.translation.x - (a.translation.x + 0.5)).abs() < 1e-5);
        let half = a.rotation.slerp(b.rotation, 0.5);
        assert!(shown.rotation.angle_between(half) < 1e-5);
        assert!(shown.rotation.angle_between(a.rotation) > 0.04);
        assert!(world.resource::<Replay>().playing.is_some());
    }
}
