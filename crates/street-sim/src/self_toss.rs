//! Recovered callback-53 launch arithmetic. Move selection and rig sampling
//! supply this data; a randomized trick variant is not a native script id.

use crate::{GRAVITY, TICK_HZ};

/// Timeline constants for default script 69 (juke_lob_aroundbody): 6 startup + 14 leadup + 28 flight + 6 followthrough = 54 updates.
pub const SCRIPT_69_STARTUP: u32 = 6;
pub const SCRIPT_69_LEADUP: u32 = 14;
pub const SCRIPT_69_FLIGHT: u32 = 28;
pub const SCRIPT_69_FOLLOWTHROUGH: u32 = 6;
pub const SCRIPT_69_TOTAL_TICKS: u32 =
    SCRIPT_69_STARTUP + SCRIPT_69_LEADUP + SCRIPT_69_FLIGHT + SCRIPT_69_FOLLOWTHROUGH;
pub const SCRIPT_69_LAUNCH_TICK: u32 = SCRIPT_69_STARTUP + SCRIPT_69_LEADUP;
pub const SCRIPT_69_CATCH_TICK: u32 = SCRIPT_69_LAUNCH_TICK + SCRIPT_69_FLIGHT;

/// Check if a move script id corresponds to a native self-toss trick move (callback 53).
pub fn is_self_toss_script(script: u16) -> bool {
    matches!(script, 21 | 23 | 36 | 60 | 61 | 69)
}

/// Retrieve the native callback-53 Cue parameters for a self-toss script id.
pub fn cue_for_script(script: u16) -> Option<Cue> {
    match script {
        21 => Some(Cue::from_callback_args([42, 70, 0])),
        23 => Some(Cue::from_callback_args([54, 30, 1])),
        36 => Some(Cue::from_callback_args([32, 32, 0])),
        60 => Some(Cue::from_callback_args([69, 50, 0])),
        61 => Some(Cue::from_callback_args([38, 34, 0])),
        69 => Some(Cue::from_callback_args([28, 21, 0])),
        _ => None,
    }
}

/// A self-toss script's schedule in native updates, from its bytecode
/// (`street_assets::scripts::self_toss_timing`, checked against the disc):
/// the opening blend, the wait before callback 53, and the closing wait
/// after the catch. Callback 53's second word is the pose frame to sample,
/// not a time, so the launch comes after the blend and the wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schedule {
    pub blend_updates: u32,
    pub pre_callback_wait_updates: u32,
    pub followthrough_updates: u32,
}

impl Schedule {
    /// Updates from the trick's start to callback 53's launch.
    pub const fn launch_tick(self) -> u32 {
        self.blend_updates + self.pre_callback_wait_updates
    }
}

/// [`Schedule`] for a self-toss script: the bytes `8e f8 NN`,
/// `8d ee .. NN` before `83 35`, and the last `8d ee .. NN` of each script
/// (SLUS_206.51 at 0x44de67, 0x44dedc, 0x44d840, 0x44e044, 0x44e0b9,
/// 0x44e365).
pub fn schedule_for_script(script: u16) -> Option<Schedule> {
    let [blend, wait, follow] = match script {
        21 => [10, 44, 2],
        23 => [6, 2, 2],
        36 => [6, 32, 4],
        60 => [6, 6, 3],
        61 => [6, 14, 2],
        69 => [6, 14, 6],
        _ => return None,
    };
    Some(Schedule {
        blend_updates: blend,
        pre_callback_wait_updates: wait,
        followthrough_updates: follow,
    })
}

/// Local displacement for script 69 in player-local space: [right, height, forward].
/// Corresponds to observed EE callback target (error < 2.04e-5 against capture).
pub const SCRIPT_69_LOCAL_TARGET: [f32; 3] = [4.1959, 3.7131, -1.1863];

/// Local displacements for each of the 6 self-toss scripts in player-local space: [right, height, forward].
/// Evaluated via native forward kinematics on authored ANIBIG.BIN tracks at sample frames.
pub fn local_target_for_script(script: u16) -> [f32; 3] {
    match script {
        21 => [0.3797, 1.0795, -12.5996],
        23 => [-1.2093, 3.3443, -8.5755],
        36 => [-1.1607, 2.3298, -9.6614],
        60 => [-6.4804, 3.7405, -11.4403],
        61 => [-0.7037, 2.7448, -1.5511],
        69 => SCRIPT_69_LOCAL_TARGET,
        _ => [1.0, 3.5, 2.0],
    }
}

/// Compute self-toss world target for an actor from position, heading, height, and script id.
pub fn target_for_player(position: [f32; 2], heading: f32, height: f32, script: u16) -> [f32; 3] {
    let [right, up, forward] = local_target_for_script(script);
    let sin_h = heading.sin();
    let cos_h = heading.cos();
    [
        position[0] + cos_h * right + sin_h * forward,
        up + height,
        position[1] - sin_h * right + cos_h * forward,
    ]
}

/// Arguments of `FUN_001741a8`, in the script's stored order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cue {
    pub flight_frames_at_60_hz: i32,
    pub sample_frame: i32,
    pub bounce: bool,
}

impl Cue {
    /// Only third-argument value 1 enables the native bounce path.
    pub const fn from_callback_args(args: [i32; 3]) -> Self {
        Self {
            flight_frames_at_60_hz: args[0],
            sample_frame: args[1],
            bounce: args[2] == 1,
        }
    }

    /// Initial duration at `00174664..00174678`. The target planner may
    /// subsequently replace it; this is the direct launch's duration.
    pub fn flight_updates(self, native_fps: i32) -> i32 {
        self.flight_frames_at_60_hz.wrapping_mul(native_fps) / 60
    }

    /// State-4 timeout initialized by `FUN_0014c0c0` and `00153ec8`.
    pub fn timeout_updates(self, native_fps: i32) -> i32 {
        self.flight_updates(native_fps)
            .wrapping_add(native_fps / 12)
    }

    /// Direct, non-bounce launch in sim units per second. `0014c100`
    /// subtracts the current ball centre, `0014c1c0..0014c1d4` divides by
    /// duration, then `0014c1d8..0014c1fc` adds half the gravity duration.
    /// The destination must already be transformed from the sampled rig.
    pub fn direct_velocity(self, from: [f32; 3], target: [f32; 3]) -> Option<[f32; 3]> {
        let updates = self.flight_updates(TICK_HZ as i32);
        if self.bounce || updates <= 0 {
            return None;
        }
        let seconds = updates as f32 / TICK_HZ as f32;
        Some([
            (target[0] - from[0]) / seconds,
            (target[1] - from[1]) / seconds + GRAVITY * 0.5 * seconds,
            (target[2] - from[2]) / seconds,
        ])
    }

    /// Bounce launch in sim units per second, derived from `FUN_0014c0c0` and `FUN_0014f728`.
    /// Solves for first floor contact time `t1` (in updates), then computes initial vertical
    /// velocity `vy = 0.5 * g * t1 - (from.y - floor) / t1`, and horizontal velocity `(target - from) / T`.
    pub fn bounce_velocity(
        self,
        from: [f32; 3],
        target: [f32; 3],
        floor: f32,
        restitution: f32,
    ) -> Option<[f32; 3]> {
        let updates = self.flight_updates(TICK_HZ as i32);
        if !self.bounce || updates <= 0 {
            return None;
        }
        let dt = 1.0 / TICK_HZ as f32;
        let g_tick = GRAVITY * dt * dt;
        let total_updates = updates as f32;
        let t1_updates = solve_bounce_time(
            from[1],
            floor,
            target[1],
            total_updates,
            g_tick,
            restitution,
        )?;
        if t1_updates <= 0.0 || t1_updates >= total_updates {
            return None;
        }
        let t1_seconds = t1_updates * dt;
        let total_seconds = total_updates * dt;
        let vy = 0.5 * GRAVITY * t1_seconds - (from[1] - floor) / t1_seconds;
        Some([
            (target[0] - from[0]) / total_seconds,
            vy,
            (target[2] - from[2]) / total_seconds,
        ])
    }

    /// Unified launch velocity covering both direct non-bounce and bounce trajectories
    /// for all 6 self-toss scripts (21, 23, 36, 60, 61, 69).
    pub fn launch_velocity(self, from: [f32; 3], target: [f32; 3], floor: f32) -> Option<[f32; 3]> {
        if self.bounce {
            self.bounce_velocity(from, target, floor, DEFAULT_BOUNCE_RESTITUTION)
        } else {
            self.direct_velocity(from, target)
        }
    }
}

/// Default restitution coefficient for self-toss floor bounce (0.8 from native tuning table `0x0043edd8` via `0x0014a564`).
pub const DEFAULT_BOUNCE_RESTITUTION: f32 = 0.8;

/// Solves the native cubic polynomial $c_3 t_1^3 + c_2 t_1^2 + c_1 t_1 + c_0 = 0$
/// from `FUN_0014f728` for the first floor contact time $t_1$ (in update ticks).
pub fn solve_bounce_time(
    y0: f32,
    y_floor: f32,
    y_target: f32,
    total_updates: f32,
    g_tick: f32,
    restitution: f32,
) -> Option<f32> {
    if total_updates <= 1.0 || g_tick <= 0.0 || restitution <= 0.0 {
        return None;
    }
    let h0 = y0 - y_floor;
    let ht = y_target - y_floor;

    let c3 = (restitution + 1.0) * -0.5 * g_tick;
    let c2 = (restitution * 0.5 + 1.0) * total_updates * g_tick;
    let c1 = (g_tick * -0.5 * total_updates * total_updates - restitution * h0) - ht;
    let c0 = restitution * total_updates * h0;

    let poly = |t: f32| c3 * t * t * t + c2 * t * t + c1 * t + c0;

    // Critical points of derivative P'(t) = 3*c3*t^2 + 2*c2*t + c1:
    let disc = 4.0 * c2 * c2 - 12.0 * c3 * c1;
    let mut crits = [0.0f32; 4];
    let mut count = 0;
    crits[count] = 0.0;
    count += 1;

    if disc >= 0.0 {
        let sqrt_disc = disc.sqrt();
        let denom = 6.0 * c3;
        let t_a = (-2.0 * c2 + sqrt_disc) / denom;
        let t_b = (-2.0 * c2 - sqrt_disc) / denom;
        let (first, second) = if t_a <= t_b { (t_a, t_b) } else { (t_b, t_a) };
        if first > 0.0 && first < total_updates {
            crits[count] = first;
            count += 1;
        }
        if second > 0.0 && second < total_updates && (second - first).abs() > 1e-4 {
            crits[count] = second;
            count += 1;
        }
    }
    crits[count] = total_updates;
    count += 1;

    // Scan adjacent intervals to find sign change (FUN_0014f728 search):
    let mut bracket = None;
    for i in 0..(count - 1) {
        let p_a = poly(crits[i]);
        let p_b = poly(crits[i + 1]);
        if p_a == 0.0 {
            return Some(crits[i]);
        }
        if p_b == 0.0 {
            return Some(crits[i + 1]);
        }
        if (p_a > 0.0 && p_b < 0.0) || (p_a < 0.0 && p_b > 0.0) {
            bracket = Some((crits[i], crits[i + 1]));
            break;
        }
    }

    let (mut t_low, mut t_high) = bracket.unwrap_or((0.0, total_updates));

    // Bisection down to interval width <= 1.0 (matching FUN_0014f728):
    while (t_high - t_low) > 1.0 {
        let mid = (t_low + t_high) * 0.5;
        let p_mid = poly(mid);
        if p_mid == 0.0 {
            return Some(mid);
        }
        if (poly(t_low) > 0.0 && p_mid < 0.0) || (poly(t_low) < 0.0 && p_mid > 0.0) {
            t_high = mid;
        } else {
            t_low = mid;
        }
    }

    // Secant interpolation step (matching FUN_0014f728):
    let p_low = poly(t_low);
    let p_high = poly(t_high);
    let diff = p_low - p_high;
    if diff.abs() > 0.0001 {
        let frac = p_low / diff;
        Some((1.0 - frac) * t_low + frac * t_high)
    } else {
        Some(t_high)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_script_keeps_sample_and_flight_separate() {
        let cue = Cue::from_callback_args([28, 21, 0]);
        assert_eq!(cue.sample_frame, 21);
        assert_eq!(cue.flight_updates(60), 28);
        assert_eq!(cue.timeout_updates(60), 33);
        assert_eq!(cue.flight_updates(30), 14);
        assert!(!Cue::from_callback_args([28, 21, 2]).bounce);
    }

    #[test]
    fn direct_launch_reaches_sampled_destination() {
        let cue = Cue::from_callback_args([28, 21, 0]);
        let from = [2.0, 5.0, -3.0];
        let target = [4.0, 7.0, 1.0];
        let velocity = cue.direct_velocity(from, target).unwrap();
        let mut ball = crate::Ball::loose_at(from);
        ball.velocity = velocity;
        // Actual ball updater: xyz += velocity*dt, y -= g*dt²/2,
        // then vy -= g*dt. This matches native FUN_0014d1f8; no
        // half-tick arrival bias remains with this integration order.
        for _ in 0..cue.flight_updates(TICK_HZ as i32) {
            ball.roll();
        }
        for (actual, expected) in ball.position.into_iter().zip(target) {
            assert!((actual - expected).abs() < 0.0002);
        }
        assert!(
            Cue::from_callback_args([28, 21, 1])
                .direct_velocity(from, target)
                .is_none()
        );
    }

    #[test]
    fn self_toss_script_detection_and_cues_match_known_table() {
        for script in [21, 23, 36, 60, 61, 69] {
            assert!(is_self_toss_script(script));
            assert!(cue_for_script(script).is_some());
        }
        assert!(!is_self_toss_script(0));
        assert!(!is_self_toss_script(10));
        assert!(cue_for_script(0).is_none());

        let cue69 = cue_for_script(69).unwrap();
        assert_eq!(cue69.flight_frames_at_60_hz, 28);
        assert_eq!(cue69.sample_frame, 21);
        assert!(!cue69.bounce);

        let cue23 = cue_for_script(23).unwrap();
        assert_eq!(cue23.flight_frames_at_60_hz, 54);
        assert_eq!(cue23.sample_frame, 30);
        assert!(cue23.bounce);
    }

    #[test]
    fn target_for_player_matches_observed_delta_at_zero_heading() {
        let pos = [10.0, 20.0];
        let target = target_for_player(pos, 0.0, 0.0, 69);
        // at heading 0: sin_h = 0, cos_h = 1
        // x = pos.x + cos_h * right + sin_h * forward = 10.0 + 4.1959 = 14.1959
        // y = up = 3.7131
        // z = pos.z - sin_h * right + cos_h * forward = 20.0 - 1.1863 = 18.8137
        assert!((target[0] - 14.1959).abs() < 1e-4);
        assert!((target[1] - 3.7131).abs() < 1e-4);
        assert!((target[2] - 18.8137).abs() < 1e-4);
    }

    #[test]
    fn bounce_solver_and_rebound_reaches_destination_at_exact_duration() {
        let cue = cue_for_script(23).unwrap();
        assert!(cue.bounce);
        let from = [1.0, 3.5, 2.0];
        let target = [3.0, 3.7131, 5.0];
        let floor = 0.0;
        let total_updates = cue.flight_updates(TICK_HZ as i32);
        assert_eq!(total_updates, 54);

        let vel = cue
            .launch_velocity(from, target, floor)
            .expect("should compute launch velocity");
        assert!(
            vel[1] < 0.0,
            "initial vertical velocity must be downward toward floor"
        );

        let dt = 1.0 / TICK_HZ as f32;
        let g_tick = GRAVITY * dt * dt;
        let t1 = solve_bounce_time(
            from[1],
            floor,
            target[1],
            total_updates as f32,
            g_tick,
            DEFAULT_BOUNCE_RESTITUTION,
        )
        .expect("should solve bounce time");

        assert!(t1 > 0.0 && t1 < total_updates as f32);

        // Continuous trajectory evaluation:
        // 1. Flight to floor at t1:
        let t1_sec = t1 * dt;
        let y_impact = from[1] + vel[1] * t1_sec - 0.5 * GRAVITY * t1_sec * t1_sec;
        assert!((y_impact - floor).abs() < 1e-3, "must hit floor at t1");

        let vy_impact = vel[1] - GRAVITY * t1_sec;
        assert!(vy_impact < 0.0, "impact velocity must be downward");
        let vy_rebound = -DEFAULT_BOUNCE_RESTITUTION * vy_impact;
        assert!(vy_rebound > 0.0, "rebound velocity must be upward");

        // 2. Flight from floor to total_updates:
        let t2_sec = (total_updates as f32 - t1) * dt;
        let y_final = floor + vy_rebound * t2_sec - 0.5 * GRAVITY * t2_sec * t2_sec;
        assert!(
            (y_final - target[1]).abs() < 0.01,
            "final y {y_final} must reach target {}",
            target[1]
        );

        let total_sec = total_updates as f32 * dt;
        let x_final = from[0] + vel[0] * total_sec;
        let z_final = from[2] + vel[2] * total_sec;
        assert!((x_final - target[0]).abs() < 1e-4);
        assert!((z_final - target[2]).abs() < 1e-4);
    }

    #[test]
    fn all_six_self_toss_scripts_produce_valid_launch_velocities() {
        let from = [0.0, 3.5, 0.0];
        let target = [2.0, 3.8, 1.0];
        let floor = 0.0;

        for script in [21, 23, 36, 60, 61, 69] {
            let cue = cue_for_script(script).expect("must find cue");
            let vel = cue
                .launch_velocity(from, target, floor)
                .expect("must produce launch velocity");
            for c in vel {
                assert!(c.is_finite());
            }
            if script == 23 {
                assert!(cue.bounce);
                assert!(vel[1] < 0.0);
            } else {
                assert!(!cue.bounce);
                assert!(vel[1] > 0.0);
            }
        }
    }
}
