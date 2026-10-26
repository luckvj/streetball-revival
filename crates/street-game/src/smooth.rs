//! Smooth motion on screens faster than the game's 60 updates a second (the
//! rewrite's own, PC Settings' "Smooth Motion"): the court's moving things
//! are drawn between the last two updates instead of holding each update
//! for several frames. The players (their places and every joint), the
//! ball, the referee and the camera each keep the transform the last two
//! updates gave them; a frame shows the earlier one blended toward the later
//! by the time since the later update came, over an update's length. So the
//! picture runs one update (1/60 s) behind the simulation, which itself is
//! untouched: what is played, sent online and recorded is the same.
//!
//! A jump too big to be motion (a restart's places, a camera cut) is shown
//! at once, and nothing blends while the game holds still (the pause, a
//! replay, which blends its own recorded poses). The blend is only drawn:
//! each frame starts by putting back the transforms the game set
//! ([`restore`]), so what reads them (the replays' recorder, the controls'
//! camera) sees the game's own.

use bevy::prelude::*;
use street_render::animation::Rig;

use crate::{
    game::Sim,
    view::{BallView, SidelineCamera, SimPlayer},
};

/// A moving thing's last two updates' transforms.
#[derive(Component, Default)]
pub struct Smooth {
    from: Transform,
    to: Transform,
    /// The update `to` came from; `None` until the first.
    tick: Option<u64>,
    /// What the game set before this frame's blend, put back next frame.
    raw: Option<Transform>,
}

/// When the simulation's latest update came, in real seconds.
#[derive(Resource, Default)]
pub struct SmoothClock {
    tick: u64,
    at: f64,
}

/// A jump in place this far (court units) between two updates is a cut,
/// not motion: shown at once. A sprinting player covers about 0.35 an
/// update, a passed ball under 1.5.
const CUT: f32 = 4.0;
/// A turn this large between two updates (radians) is a cut too.
const CUT_TURN: f32 = 1.2;

/// Notes when the simulation's update count moves (after the fixed steps).
pub fn clock(sim: Res<Sim>, time: Res<Time<Real>>, mut clock: ResMut<SmoothClock>) {
    if sim.world.tick != clock.tick {
        clock.tick = sim.world.tick;
        clock.at = time.elapsed_secs_f64();
    }
}

/// A new update's transform `now`: the old end is where the blend starts,
/// unless the jump is a cut.
fn advance(smooth: &mut Smooth, now: Transform, tick: u64) {
    smooth.from = smooth.to;
    smooth.to = now;
    smooth.tick = Some(tick);
    let jump = smooth.from.translation.distance(smooth.to.translation);
    let turn = smooth.from.rotation.angle_between(smooth.to.rotation);
    if jump > CUT || turn > CUT_TURN {
        smooth.from = smooth.to;
    }
}

/// Tracks the court's moving things: the players and their joints, the
/// referee and his, the ball and the camera.
#[allow(clippy::type_complexity)]
pub fn track(
    mut commands: Commands,
    movers: Query<
        Entity,
        (
            Or<(
                With<SimPlayer>,
                With<crate::referee::RefereeView>,
                With<BallView>,
                With<SidelineCamera>,
            )>,
            Without<Smooth>,
        ),
    >,
    rigs: Query<&Rig, Or<(With<SimPlayer>, With<crate::referee::RefereeView>)>>,
    tracked: Query<(), With<Smooth>>,
) {
    for e in &movers {
        commands.entity(e).insert(Smooth::default());
    }
    for rig in &rigs {
        for &j in &rig.joints {
            if tracked.get(j).is_err() {
                commands.entity(j).insert(Smooth::default());
            }
        }
    }
}

/// Puts back what the game set before last frame's blend.
pub fn restore(mut tracked: Query<(&mut Transform, &mut Smooth)>) {
    for (mut transform, mut smooth) in &mut tracked {
        if let Some(raw) = smooth.raw.take() {
            *transform = raw;
        }
    }
}

/// Blends each tracked transform between its last two updates. Runs after
/// every system that places, poses or aims them this frame, before the
/// transforms propagate. A held ball rides the drawn hand, so it is left
/// where its system put it.
pub fn blend(
    on: Option<Res<crate::options::Options>>,
    sim: Res<Sim>,
    clock: Res<SmoothClock>,
    time: Res<Time<Real>>,
    mut tracked: Query<(&mut Transform, &mut Smooth, Has<BallView>)>,
) {
    let enabled = on.is_none_or(|o| o.0.smooth);
    let held = matches!(sim.world.ball.state, street_sim::BallState::Held(_));
    let step = 1.0 / f64::from(street_sim::TICK_HZ);
    let t = ((time.elapsed_secs_f64() - clock.at) / step).clamp(0.0, 1.0) as f32;
    for (mut transform, mut smooth, ball) in &mut tracked {
        match smooth.tick {
            Some(tick) if tick == clock.tick => {}
            // A new update (or a rewind, online): what the systems just set
            // is the new end; the old end where the blend starts.
            Some(tick) if tick < clock.tick => advance(&mut smooth, *transform, clock.tick),
            _ => {
                smooth.from = *transform;
                smooth.to = *transform;
                smooth.tick = Some(clock.tick);
            }
        }
        if !enabled || t >= 1.0 || (ball && held) {
            // Nothing to blend: what the systems set this frame stands
            // (the pause, a replay's own blending).
            continue;
        }
        smooth.raw = Some(*transform);
        transform.translation = smooth.from.translation.lerp(smooth.to.translation, t);
        transform.rotation = smooth.from.rotation.slerp(smooth.to.rotation, t);
        transform.scale = smooth.from.scale.lerp(smooth.to.scale, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cut_shows_at_once_and_motion_blends() {
        let at = |x: f32| Transform::from_xyz(x, 0.0, 0.0);
        let mut s = Smooth {
            tick: Some(1),
            ..default()
        };
        // An update 0.3 on blends from the last; a 10-unit jump does not.
        advance(&mut s, at(0.3), 2);
        assert_eq!((s.from.translation.x, s.to.translation.x), (0.0, 0.3));
        advance(&mut s, at(10.3), 3);
        assert_eq!(s.from.translation.x, 10.3, "a cut does not blend");
        // A quarter turn is a cut too.
        let mut turned = at(10.3);
        turned.rotate_y(std::f32::consts::FRAC_PI_2);
        advance(&mut s, turned, 4);
        assert_eq!(s.from.rotation, s.to.rotation);
    }
}
