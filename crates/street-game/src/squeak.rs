//! Sneaker squeaks: the original squeaks when a player plants or cuts
//! (its listener code at `0x1a5120`). It counts those moments and fires
//! event `0x3c` on every fifth one, or whenever the player is doing
//! something other than running; every other such squeak also fires the
//! court's own step (`0x46`). The simulation has no plant event, so a cut
//! is detected from the heading and speed instead.

use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

use crate::{
    Screen,
    game::{Sim, TickEvents},
    sfx,
    view::SidelineCamera,
};

/// Samples kept: 0.15 s of ticks, plus the current one.
const WINDOW: usize = (street_sim::TICK_HZ as f32 * 0.15) as usize + 1;
/// Ticks a cut hides later ones for: 0.25 s.
const COOLDOWN: u32 = (street_sim::TICK_HZ as f32 * 0.25) as u32;
/// A turn wider than this is a cut: 50 degrees.
const TURN: f32 = 50.0 * PI / 180.0;

pub struct SqueakPlugin;

impl Plugin for SqueakPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            squeak_system
                .after(crate::game::SimStep)
                .run_if(in_state(Screen::Playing)),
        );
    }
}

/// A player's recent headings and speeds, and ticks until it may cut again.
#[derive(Default)]
struct CutState {
    past: Vec<(f32, f32)>,
    cooldown: u32,
}

/// True when the player just cut: it was moving faster than 6 units/s and
/// turned more than 50 degrees within 0.15 s, or its speed fell from above
/// 8 to below 2 that fast. At most one cut per 0.25 s.
fn detect_cut(state: &mut CutState, heading: f32, speed: f32) -> bool {
    state.past.push((heading, speed));
    if state.past.len() > WINDOW {
        state.past.remove(0);
    }
    if state.cooldown > 0 {
        state.cooldown -= 1;
        return false;
    }
    if state.past.len() < WINDOW {
        return false;
    }
    let (old_heading, old_speed) = state.past[0];
    if !state.past.iter().any(|&(_, s)| s > 6.0) {
        return false;
    }
    let mut turn = (heading - old_heading).abs() % TAU;
    if turn > PI {
        turn = TAU - turn;
    }
    let cut = turn > TURN || (old_speed > 8.0 && speed < 2.0);
    if cut {
        state.cooldown = COOLDOWN;
    }
    cut
}

fn squeak_system(
    fx: Option<ResMut<sfx::Effects>>,
    sim: Res<Sim>,
    tick: Res<TickEvents>,
    camera: Query<&GlobalTransform, With<SidelineCamera>>,
    mut states: Local<Vec<CutState>>,
    mut cuts: Local<u32>,
    mut alternate: Local<bool>,
) {
    let Some(mut fx) = fx else { return };
    if tick.replay {
        return;
    }
    let Ok(cam) = camera.single() else { return };
    if states.len() != sim.world.players.len() {
        states.resize_with(sim.world.players.len(), CutState::default);
    }
    for (i, player) in sim.world.players.iter().enumerate() {
        if !detect_cut(&mut states[i], player.heading, player.speed) {
            continue;
        }
        *cuts += 1;
        if !cuts.is_multiple_of(5) && player.action == street_sim::Action::None {
            continue;
        }
        let at = Vec3::new(player.position[0], player.height + 0.2, player.position[1]);
        let (angle, distance) = sfx::place(cam, at);
        let loud = sfx::intensity(player.speed);
        let shoe = i32::from(fx.voices.get(i).copied().unwrap_or(0) == 1);
        fx.fire(&[sfx::ev::SQUEAK, angle, distance, loud, shoe]);
        *alternate = !*alternate;
        if *alternate {
            fx.fire(&[sfx::ev::COURT_STEP, angle, distance, loud, shoe]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(samples: &[(f32, f32)]) -> Vec<bool> {
        let mut state = CutState::default();
        samples
            .iter()
            .map(|&(h, s)| detect_cut(&mut state, h, s))
            .collect()
    }

    #[test]
    fn a_straight_run_does_not_squeak() {
        let out = run(&[(0.3, 7.0); 30]);
        assert!(out.iter().all(|&c| !c));
    }

    #[test]
    fn a_sharp_cut_squeaks() {
        let mut samples = vec![(0.0, 8.0); 10];
        samples.push((1.2, 8.0));
        let out = run(&samples);
        assert_eq!(out.iter().filter(|&&c| c).count(), 1);
        assert!(out[10]);
    }

    #[test]
    fn a_hard_stop_squeaks() {
        let mut samples = vec![(0.0, 9.0); 10];
        samples.push((0.0, 1.0));
        let out = run(&samples);
        assert!(out[10]);
    }

    #[test]
    fn two_cuts_close_together_squeak_once() {
        let mut samples = vec![(0.0, 8.0); 10];
        samples.push((1.2, 8.0));
        samples.extend([(2.4, 8.0); 5]);
        let out = run(&samples);
        assert_eq!(out.iter().filter(|&&c| c).count(), 1);
    }
}
