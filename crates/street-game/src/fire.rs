//! The hoop's fireworks when a basket wins the game: the effect of type 15
//! (`FUN_001f6fc8`, 120 of the engine's particles, its listener
//! `FUN_001f6cc8`) and its update (`FUN_001f7178`, once a frame), ported
//! from the executable. The basket's listener (slot 26, `FUN_001f6d98`)
//! lights it for a made basket that wins the game (the game's vt+0xbc), or
//! any with the Explosive Rims cheat (+0x14 of the cheats); the next update
//! throws 46 sparks off the rim on the ball's side (±36.75, 11.957) and
//! starts 20 frames of flames. The game's reset (slot 53, `FUN_001f6f10`)
//! and a replay's start (slot 58, `FUN_001f6ec8`) put everything out.
//!
//! The tuning is the shared constructor's (`0x4c84f0..0x4c85c0`,
//! `0x3ee3c8..0x3ee3d4`): a spark lives 60..120 frames at up to 10 mph,
//! three in four of them blue (100, 169, 245) and the rest grey (128),
//! fading toward (36, 50, 128) times 0.75..1 over its life; it falls at
//! full gravity, bounces off the floor and the backboards' faces at half
//! speed, winks out for a frame every 5..10, and over its last 40 shrinks
//! and leans into the court's wind. A flame pair lives 100 frames from the
//! rim, one each way at up to 0.375 a frame, the first white and growing
//! from 0.237 toward 1.586, going out at 2.55 of alpha a frame, the second
//! at 200 going at 2.
//!
//! The draws are the rewrite's own generator (the game's shared one).

const FPS: f32 = 60.0;
/// The particles, and those that are flames (`0x3ee3c8`); the rest are
/// sparks.
pub const PARTICLES: usize = 120;
pub const FLAMES: usize = 74;
/// `0x4c84f8`, `0x4c8558`: a spark's life, 60 + `r % 61`.
const SPARK_LIFE: i32 = 60;
const SPARK_SPREAD: u32 = 61;
/// `0x4c8540`: over its last this many frames a spark shrinks and drifts.
const SPARK_LATE: i32 = 40;
/// `0x4c8548`, `0x4c855c`: a spark's wink, every 5 + `r % 6` frames.
const WINK: i32 = 5;
const WINK_SPREAD: u32 = 6;
/// `0x4c8568`: the frames flames rise; `0x4c856c` a flame's life.
const BURN: i32 = 20;
const FLAME_LIFE: i32 = 100;

fn mph(n: f32) -> f32 {
    5280.0 / (FPS * 3600.0) * n
}

/// One particle: place, velocity, colour (0..255, the engine's), its
/// change a frame, size and its change a frame (+0x1c), frames to a wink,
/// life left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Particle {
    pub position: [f32; 3],
    velocity: [f32; 3],
    pub colour: [f32; 4],
    fade: [f32; 4],
    pub size: f32,
    grow: f32,
    wink: i32,
    pub life: i32,
}

/// The effect: its particles, the rim it burns at (+0x20), the flames'
/// frames left (+0x94), lit by a basket since the last update (the
/// listener's +4).
#[derive(Clone, Debug)]
pub struct Fire {
    pub particles: [Particle; PARTICLES],
    rim: [f32; 3],
    burn: i32,
    lit: Option<f32>,
    rng: u32,
}

impl Fire {
    pub fn new(seed: u32) -> Self {
        Fire {
            particles: [Particle::default(); PARTICLES],
            rim: [0.0; 3],
            burn: 0,
            lit: None,
            rng: seed | 1,
        }
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

    /// Slot 26 (`FUN_001f6d98`): a basket lit it, the ball at `ball_x`.
    pub fn light(&mut self, ball_x: f32) {
        self.lit = Some(ball_x);
    }

    /// Slots 53 and 58: every particle out.
    pub fn out(&mut self) {
        for p in &mut self.particles {
            p.life = 0;
        }
    }

    /// One frame of `FUN_001f7178` in `wind` ([`crate::steam::court_wind`]).
    pub fn step(&mut self, wind: [f32; 3]) {
        if let Some(x) = self.lit.take() {
            self.ignite(x);
        }
        // `0x1f7684..0x1f7980`: a flame pair a frame while it burns.
        if self.burn > 0 {
            self.burn -= 1;
            if self.burn != 0 {
                self.flame();
            }
        }
        // `0x1f798c..0x1f7a20`: the flames.
        for p in self.particles.iter_mut().take(FLAMES) {
            if p.life == 0 {
                continue;
            }
            p.life -= 1;
            if p.life == 0 {
                continue;
            }
            p.position = add(p.position, p.velocity);
            for (c, d) in p.colour.iter_mut().zip(p.fade) {
                *c += d;
            }
            p.size += p.grow;
        }
        // `0x1f7a24..0x1f7cd8`: the sparks.
        let gravity = f32::from_bits(0x4200_9be3) / (FPS * FPS);
        for i in FLAMES..PARTICLES {
            let mut p = self.particles[i];
            if p.life == 0 {
                continue;
            }
            p.life -= 1;
            if p.life == 0 {
                self.particles[i] = p;
                continue;
            }
            p.position = add(p.position, p.velocity);
            p.velocity[1] -= gravity;
            for (c, d) in p.colour.iter_mut().zip(p.fade) {
                *c += d;
            }
            if p.position[1] <= p.size && p.velocity[1] < 0.0 {
                // The floor.
                p.position[1] = p.size;
                p.velocity[1] *= -0.5;
            } else if p.position[0].abs() > 38.0
                && p.position[2].abs() <= 3.0
                && 11.5 <= p.position[1]
            {
                // A backboard's face.
                p.position[0] = if 0.0 < p.position[0] { 38.0 } else { -38.0 };
                p.velocity[0] *= -0.5;
            }
            p.wink -= 1;
            if p.wink == 0 {
                p.colour[3] = 0.0;
                p.wink = WINK + (self.next() % WINK_SPREAD) as i32;
            } else {
                p.colour[3] = 255.0;
            }
            if p.life < SPARK_LATE {
                let y = p.velocity[1];
                let k = self.unit() * f32::from_bits(0x3ba3_d70b);
                for (v, w) in p.velocity.iter_mut().zip(wind) {
                    *v += (w - *v) * k;
                }
                p.velocity[1] = y;
                p.size -= f32::from_bits(0x3c19_999a);
            }
            self.particles[i] = p;
        }
    }

    /// `0x1f71ec..0x1f7680`: the rim on the ball's side, 20 frames of
    /// flames, and the sparks.
    fn ignite(&mut self, ball_x: f32) {
        self.rim = [
            if 0.0 < ball_x { 36.75 } else { -36.75 },
            f32::from_bits(0x413f_4ef3),
            0.0,
        ];
        self.burn = BURN;
        let (mut blue, mut thrown) = (0u32, 0u32);
        for i in FLAMES..PARTICLES {
            let mut p = Particle {
                life: SPARK_LIFE + (self.next() % SPARK_SPREAD) as i32,
                position: self.rim,
                ..Particle::default()
            };
            let mut way = [self.unit() - 0.5, self.unit(), self.unit() - 0.5];
            if len3(way) < f32::from_bits(0x3a83_126f) {
                way[1] = f32::from_bits(0x3dcc_cccd);
            }
            let l = len3(way);
            let speed = self.unit() * mph(10.0);
            p.velocity = way.map(|c| c / l * speed);
            // Three in four blue: the share so far under 0.76 (none yet
            // reads as over it, the EE's 0 / 0).
            let share = if thrown == 0 {
                f32::MAX
            } else {
                blue as f32 / thrown as f32
            };
            p.colour = if share < f32::from_bits(0x3f42_8f5c) {
                blue += 1;
                [100.0, 169.0, 245.0, 255.0]
            } else {
                [128.0, 128.0, 128.0, 255.0]
            };
            let k = self.unit() * 0.25 + 0.75;
            let end = [36.0 * k, 50.0 * k, 128.0 * k, 255.0 * k];
            let n = p.life as f32;
            for ((d, e), c) in p.fade.iter_mut().zip(end).zip(p.colour).take(3) {
                *d = (e - c) / n;
            }
            p.fade[3] = 0.0;
            p.size = f32::from_bits(0x3ecc_cccd);
            p.wink = WINK + (self.next() % WINK_SPREAD) as i32;
            thrown += 1;
            self.particles[i] = p;
        }
    }

    /// `0x1f76e8..0x1f797c`: the first free flame and the one after it.
    fn flame(&mut self) {
        let Some(i) = (0..FLAMES).find(|&i| self.particles[i].life == 0) else {
            return;
        };
        let small = f32::from_bits(0x3e72_4a6b);
        let big = f32::from_bits(0x3fcb_10cb);
        let v = [self.unit() - 0.5, self.unit() - 0.5, self.unit() - 0.5].map(|c| c * 0.75);
        let grow = (big - small) / FLAME_LIFE as f32 * self.unit();
        self.particles[i] = Particle {
            position: self.rim,
            velocity: v,
            colour: [255.0; 4],
            fade: [0.0, 0.0, 0.0, -2.55],
            size: small,
            grow,
            life: FLAME_LIFE,
            ..Particle::default()
        };
        if let Some(q) = self.particles.get_mut(i + 1) {
            let keep = q.grow;
            *q = Particle {
                position: sub(self.rim, v),
                velocity: v,
                colour: [255.0, 255.0, 255.0, 200.0],
                fade: [0.0, 0.0, 0.0, -2.0],
                size: f32::from_bits(0x3e73_adc1),
                grow: keep,
                life: FLAME_LIFE,
                ..Particle::default()
            };
        }
    }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn len3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_winning_basket_throws_46_sparks_and_20_frames_of_flames() {
        let mut f = Fire::new(3);
        f.light(20.0);
        f.step([0.0; 3]);
        let sparks: Vec<&Particle> = f.particles[FLAMES..]
            .iter()
            .filter(|p| p.life > 0)
            .collect();
        assert_eq!(sparks.len(), PARTICLES - FLAMES);
        let blue = sparks.iter().filter(|p| p.colour[2] > 200.0).count();
        assert!((30..=38).contains(&blue), "{blue} blue");
        for p in &sparks {
            assert!((p.position[0] - 36.75).abs() < 0.5, "the ball's side");
        }
        // Flames come in pairs for 19 more frames.
        for _ in 0..30 {
            f.step([0.0; 3]);
        }
        let flames = f.particles[..FLAMES].iter().filter(|p| p.life > 0).count();
        assert!((36..=40).contains(&flames), "{flames}");
        // Sparks never sink through the floor, and all are gone in time.
        for _ in 0..200 {
            f.step([0.0; 3]);
            for p in &f.particles[FLAMES..] {
                if p.life > 0 {
                    assert!(p.position[1] >= p.size - 0.6);
                }
            }
        }
        assert!(f.particles.iter().all(|p| p.life == 0));
    }

    #[test]
    fn the_reset_puts_it_out() {
        let mut f = Fire::new(5);
        f.light(-3.0);
        f.step([0.0; 3]);
        f.out();
        assert!(f.particles.iter().all(|p| p.life == 0));
    }
}
