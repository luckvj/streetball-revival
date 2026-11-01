//! Steam from a court's vents (`PS_STEAM x y z dx dy dz`): the vent's
//! particle system (`FUN_0020a4d0`, 40 particles of the engine's 0x70-byte
//! kind) and its update (`FUN_0020a5c8`, once a frame), ported from the
//! executable. Its tuning is the shared constructor's (`FUN_0020aba0`,
//! `0x20ad68..0x20af3c`), at the game's 60 frames a second: a puff lives
//! 270 frames, coming in over the first 90 and going over the last 180 to
//! an alpha of 191.25; it starts within 4 of the vent's mouth (half its
//! way on from the vent) at 2 mph up plus the vent's way, is nudged up to
//! 0.21 mph across every 1..20 frames, and while going out leans into the
//! court's wind (`WIND`, `0x3f4fec`/`0x3f4ff0`) at a random share up to
//! 0.8 a second. A vent lets out one puff a frame at odds of 40 in 270.
//!
//! The draws are the game's shared generator's ([`crate::draws`]).

use crate::draws::Draws;

/// The frames a second the game's tuning is made for.
const FPS: f32 = 60.0;
/// `0x4ca7c8`: a puff's life, 90 + 180 frames.
const LIFE: i32 = 270;
/// `0x4ca7b8`: below this life a puff is going out.
const GOING: i32 = 180;
/// `0x4ca7c0`: the longest wait between nudges.
const NUDGE_WAIT: u32 = 20;
/// `0x4ca7c4`: a puff's full alpha (PS2 vertex alpha, 128 is 1).
pub const ALPHA: f32 = 191.25;
/// A puff's size (+0x40 at its start).
pub const SIZE: f32 = 3.0;
/// The particles a vent has.
pub const PUFFS: usize = 40;

fn mph(n: f32) -> f32 {
    5280.0 / (FPS * 3600.0) * n
}

/// `0x4ca7f0`, `0x4ca7f4`: a nudge's reach across.
fn nudge() -> f32 {
    mph(1.0) * f32::from_bits(0x3ed7_0a3d)
}

/// `0x4ca7bc`: the share of the way to the wind a frame at most.
fn lean() -> f32 {
    f32::from_bits(0x3f4c_cccd) / FPS
}

/// `0x4ca7d4`: the odds of a puff a frame (40 over 270; its whole part,
/// `0x4ca7d0`, is 0).
fn odds() -> f32 {
    let r = 40.0 / LIFE as f32;
    r - r as i32 as f32
}

/// The court's wind as `WIND speed x z` sets it (`0x2f3f90..0x2f4040`):
/// mph in feet a frame, along (x, 0, z) made unit.
pub fn court_wind(speed: f32, x: f32, z: f32) -> [f32; 3] {
    let s = speed * (5280.0 / (FPS * 3600.0));
    let l = (x * x + z * z).sqrt();
    if l == 0.0 {
        return [0.0; 3];
    }
    [x / l * s, 0.0, z / l * s]
}

/// One puff (the engine's particle: +0 place, +0x10 velocity, +0x2c
/// alpha, +0x44 frames to its next nudge, +0x48 life left).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Puff {
    pub position: [f32; 3],
    velocity: [f32; 3],
    pub alpha: f32,
    nudge: u32,
    pub life: i32,
}

/// A vent: where it is (+0x20), the way it blows (+0x60) and its puffs.
#[derive(Clone, Debug)]
pub struct Vent {
    at: [f32; 3],
    way: [f32; 3],
    pub puffs: [Puff; PUFFS],
}

impl Vent {
    pub fn new(at: [f32; 3], way: [f32; 3]) -> Self {
        Vent {
            at,
            way,
            puffs: [Puff::default(); PUFFS],
        }
    }

    /// One frame of `FUN_0020a5c8` in `wind` ([`court_wind`]).
    pub fn step(&mut self, wind: [f32; 3], d: &mut impl Draws) {
        let mut due = u32::from(d.unit() < odds());
        for i in 0..PUFFS {
            let mut p = self.puffs[i];
            if p.life > 0 {
                p.life -= 1;
                if p.life > 0 {
                    p.position = add(p.position, p.velocity);
                    p.nudge = p.nudge.saturating_sub(1);
                    if p.nudge == 0 {
                        let x = (d.unit() - 0.5) * nudge();
                        let z = (d.unit() - 0.5) * nudge();
                        p.velocity[0] += x;
                        p.velocity[2] += z;
                        p.nudge = d.shared() % NUDGE_WAIT + 1;
                    }
                    let share = if GOING < p.life {
                        (LIFE - p.life) as f32 * (1.0 / 90.0)
                    } else {
                        // Going out it leans into the wind, across only.
                        let y = p.velocity[1];
                        let k = d.unit() * lean();
                        for (v, w) in p.velocity.iter_mut().zip(wind) {
                            *v += (w - *v) * k;
                        }
                        p.velocity[1] = y;
                        p.life as f32 * (1.0 / GOING as f32)
                    };
                    p.alpha = share.clamp(0.0, 1.0) * ALPHA;
                }
            }
            // `0x20a94c..0x20ab38`: a free particle takes a puff due.
            if due > 0 && p.life == 0 {
                due -= 1;
                p.life = LIFE;
                let mouth = add(self.way.map(|c| c * 0.5), self.at);
                let x = (d.unit() - 0.5) * 8.0;
                let _ = d.unit();
                let z = (d.unit() - 0.5) * 8.0;
                p.position = add(mouth, [x, 0.0, z]);
                p.velocity = add([0.0, mph(1.0) + mph(1.0), 0.0], self.way);
                p.alpha = 0.0;
                p.nudge = d.shared() % NUDGE_WAIT + 1;
            }
            self.puffs[i] = p;
        }
    }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tuning_is_the_constructors() {
        // The bss words save state 9 holds (`0x4ca7b0..0x4ca800`).
        assert!((odds() - 0.148_15).abs() < 1e-5);
        assert!((nudge() - 0.010_27).abs() < 1e-5);
        assert!((lean() - 0.013_33).abs() < 1e-5);
        assert!((mph(2.0) - 0.048_89).abs() < 1e-5);
        let w = court_wind(15.0, -1.0, 1.0);
        assert!((w[0] + 0.259_27).abs() < 1e-4 && w[1] == 0.0);
    }

    #[test]
    fn puffs_rise_come_in_and_go_out_over_270_frames() {
        let mut v = Vent::new([67.98, 1.0, 31.82], [0.0; 3]);
        let mut d = crate::draws::Own(5);
        let mut most_alpha: f32 = 0.0;
        let mut alive = 0;
        for frame in 0..2000 {
            v.step(court_wind(15.0, -1.0, 1.0), &mut d);
            for p in &v.puffs {
                if p.life > 0 {
                    most_alpha = most_alpha.max(p.alpha);
                    assert!(p.life <= LIFE);
                    // Up at 2 mph: never below the vent's mouth.
                    assert!(p.position[1] >= 1.0);
                }
            }
            if frame > 1500 {
                alive += v.puffs.iter().filter(|p| p.life > 0).count();
            }
        }
        assert!((most_alpha - ALPHA).abs() < 3.0, "{most_alpha}");
        // 40 / 270 a frame for 270 frames: about 40 alive, never more.
        let mean = alive as f32 / 499.0;
        assert!((25.0..=40.0).contains(&mean), "{mean}");
    }

    /// Counts the draws by the order they come in a frame.
    struct Counted {
        own: crate::draws::Own,
        draws: u64,
    }

    impl Draws for Counted {
        fn shared(&mut self) -> u32 {
            self.draws += 1;
            self.own.shared()
        }

        fn rand(&mut self) -> i32 {
            self.own.rand()
        }
    }

    #[test]
    fn a_vent_draws_as_often_as_the_originals() {
        // The original's two vents on Broad Street, logged live over 22,856
        // updates (docs/TRACE.md "The game's construction, draw by draw"):
        // per vent and frame 1 odds draw, 0.133 new puffs (four draws
        // each), 3.362 nudges (three) and 23.92 leans (one), 35.5 in all.
        let mut v = Vent::new([0.0; 3], [0.0; 3]);
        let mut d = Counted {
            own: crate::draws::Own(17),
            draws: 0,
        };
        for _ in 0..2000 {
            v.step(court_wind(15.0, -1.0, 1.0), &mut d);
        }
        d.draws = 0;
        let frames = 40_000;
        for _ in 0..frames {
            v.step(court_wind(15.0, -1.0, 1.0), &mut d);
        }
        let per_frame = d.draws as f64 / f64::from(frames);
        let original = 1.0 + 0.133 * 4.0 + 3.362 * 3.0 + 23.92;
        assert!(
            (per_frame - original).abs() < original * 0.03,
            "{per_frame} vs {original}"
        );
    }
}
