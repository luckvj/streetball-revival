//! A player's breath in the cold (`COLD`): at the court script's end the
//! game makes six breathers, one a player (`0x2f45dc..0x2f462c`, effect
//! type 3, `FUN_001f06a0`: 10 of the engine's particles each), and steps
//! each once a frame (`FUN_001f07c0`), ported from the executable. Its
//! tuning is the shared constructor's (`FUN_0020aba0`), at the game's 60
//! frames a second: every 120 + `r % 181` frames a player breathes out all
//! ten puffs at once, from 1.25 along his head bone's turned up axis
//! (bone 21's +y), each within 0.3 of it; a puff lives 40 frames, keeping
//! to the mouth as it comes in over 30 to an alpha of 76.5, then drifts
//! free for the last 10, leaning into the court's wind, going out. It
//! rises 0.3 mph, is nudged up to ±0.08 mph across every 1..4 frames, and is
//! 0.35 across.
//!
//! The game breathes only for a player whose +0x68 is set (1 for those on
//! screen in the save states); the rewrite breathes for all. The draws are
//! the rewrite's own generator (the game's shared one).

const FPS: f32 = 60.0;
/// `0x4c7e6c`: a puff's life.
const LIFE: i32 = 40;
/// `0x4c7e58`: below this life a puff drifts free.
const FREE: i32 = 10;
/// `0x4c7e64`: the longest wait between nudges.
const NUDGE_WAIT: u32 = 4;
/// `0x4c7e34`, `0x4c7e60`: a breath's wait, 120 + `r % 181` frames.
const WAIT: i32 = 120;
const WAIT_SPREAD: u32 = 181;
/// `0x4c7e68`: a puff's full alpha (128 is 1).
pub const ALPHA: f32 = 76.5;
/// A puff's size (+0x40 at its start).
pub const SIZE: f32 = f32::from_bits(0x3eb3_3333);
/// The particles a breather has, and puffs a breath (`0x4c7e70`).
pub const PUFFS: usize = 10;
/// `0x4c7e90`, `0x4c7e94`: a nudge's reach across (about 0.17 mph; the
/// word save state 9 holds).
const NUDGE: f32 = f32::from_bits(0x3b84_f71a);
/// The head bone the breath comes from.
pub const HEAD: usize = 21;

fn mph(n: f32) -> f32 {
    5280.0 / (FPS * 3600.0) * n
}

/// One puff: its offset from the mouth while it keeps to it (+0x9c's
/// entry), place, velocity, alpha, frames to a nudge, life left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Puff {
    offset: [f32; 3],
    pub position: [f32; 3],
    velocity: [f32; 3],
    pub alpha: f32,
    nudge: u32,
    pub life: i32,
}

/// One player's breather.
#[derive(Clone, Debug)]
pub struct Breather {
    pub puffs: [Puff; PUFFS],
    /// +0x70: breathing out this frame; +0x90 frames to the next breath.
    out: bool,
    wait: i32,
    rng: u32,
}

impl Breather {
    /// `FUN_001f06a0`: the first breath `WAIT + r % 181` frames off.
    pub fn new(seed: u32) -> Self {
        let mut b = Breather {
            puffs: [Puff::default(); PUFFS],
            out: false,
            wait: 0,
            rng: seed | 1,
        };
        b.wait = WAIT + (b.next() % WAIT_SPREAD) as i32;
        b
    }

    fn next(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    fn unit(&mut self) -> f32 {
        (self.next() & 0x7fff_ffff) as f32 * f32::from_bits(0x3000_0000)
    }

    /// One frame of `FUN_001f07c0`: `head` the head bone's place and its
    /// up axis turned (`FUN_002f7430` of `(0, 1, 0)` by its rotation),
    /// `wind` the court's ([`crate::steam::court_wind`]).
    pub fn step(&mut self, head: [f32; 3], up: [f32; 3], wind: [f32; 3]) {
        let mut due = 0;
        if !self.out && self.wait == 0 {
            self.out = true;
            self.wait = 1;
        }
        if self.out {
            if self.wait == 0 {
                self.out = false;
                self.wait = WAIT + (self.next() % WAIT_SPREAD) as i32;
            } else {
                due = PUFFS;
            }
        }
        let l = (up[0] * up[0] + up[1] * up[1] + up[2] * up[2]).sqrt();
        let k = if l > 0.0 { 1.25 / l } else { 0.0 };
        let mouth = [
            head[0] + up[0] * k,
            head[1] + up[1] * k,
            head[2] + up[2] * k,
        ];
        for i in 0..PUFFS {
            let mut p = self.puffs[i];
            if p.life > 0 {
                p.life -= 1;
                if p.life > 0 {
                    p.nudge = p.nudge.saturating_sub(1);
                    if p.nudge == 0 {
                        let x = (self.unit() - 0.5) * NUDGE;
                        let z = (self.unit() - 0.5) * NUDGE;
                        p.velocity[0] += x;
                        p.velocity[2] += z;
                        p.nudge = self.next() % NUDGE_WAIT + 1;
                    }
                    let share = if FREE < p.life {
                        // Keeping to the mouth.
                        p.offset = add(p.offset, p.velocity);
                        p.position = add(p.offset, mouth);
                        (LIFE - p.life) as f32 * (1.0 / 30.0)
                    } else {
                        p.position = add(p.position, p.velocity);
                        let y = p.velocity[1];
                        let k = self.unit() * f32::from_bits(0x3ba3_d70b);
                        for (v, w) in p.velocity.iter_mut().zip(wind) {
                            *v += (w - *v) * k;
                        }
                        p.velocity[1] = y;
                        p.life as f32 * 0.1
                    };
                    p.alpha = share.clamp(0.0, 1.0) * ALPHA;
                }
            }
            if self.out && due > 0 && p.life == 0 {
                due -= 1;
                p.life = LIFE;
                let s = 0.6;
                p.offset = [
                    (self.unit() - 0.5) * s,
                    (self.unit() - 0.5) * s,
                    (self.unit() - 0.5) * s,
                ];
                p.position = add(mouth, p.offset);
                p.velocity = [0.0, mph(1.0) * f32::from_bits(0x3e99_999a), 0.0];
                p.alpha = 0.0;
                p.nudge = self.next() % NUDGE_WAIT + 1;
            }
            self.puffs[i] = p;
        }
        if self.wait > 0 {
            self.wait -= 1;
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
    fn a_breath_lets_out_ten_puffs_that_keep_to_the_mouth_then_drift() {
        let mut b = Breather::new(9);
        let head = [10.0, 6.0, 0.0];
        let up = [0.0, 0.0, 2.0];
        let mut breaths = 0;
        let mut most_alpha: f32 = 0.0;
        let mut was_out = false;
        for _ in 0..2000 {
            b.step(head, up, [0.0; 3]);
            let alive = b.puffs.iter().filter(|p| p.life > 0).count();
            let fresh = b.puffs.iter().filter(|p| p.life == LIFE).count();
            if fresh > 0 {
                // All ten at once, around 1.25 along the head's up.
                assert_eq!(fresh, PUFFS);
                breaths += 1;
                for p in &b.puffs {
                    assert!((p.position[2] - 1.25).abs() <= 0.3 + 1e-4);
                }
            }
            was_out |= alive > 0;
            for p in &b.puffs {
                most_alpha = most_alpha.max(p.alpha);
            }
        }
        assert!(was_out);
        // 2000 frames at 120..300 between: 6..17 breaths.
        assert!((6..=17).contains(&breaths), "{breaths}");
        assert!((most_alpha - ALPHA).abs() < 3.0, "{most_alpha}");
    }
}
