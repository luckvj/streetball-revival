//! The original's two generators as its presentation takes them: the
//! shared one (`FUN_003a16b8`) and the C library's `rand()`
//! (`FUN_003b42a0`). The court's effects draw from the game's own after
//! each frame's gameplay (the ball's sparks, the hoop's fire, the litter,
//! the vents, the breath, in the order the court's script builds them), so
//! a match's later gameplay draws follow the original's (docs/TRACE.md
//! "The game's construction, draw by draw"). Online the client's world is
//! the match's copy, so the effects' draws there change nothing the match
//! keeps.

/// The game whose generator the effects draw from: the live one, which a
/// replay keeps aside while it shows a recorded one (`crate::tape`); the
/// original's effects run through its replays on the game's generator.
pub fn live<'a>(
    sim: &'a mut crate::game::Sim,
    tape: Option<&'a mut crate::tape::Tape>,
) -> &'a mut street_sim::World {
    match tape.and_then(|t| t.live_mut()) {
        Some(live) => live,
        None => &mut sim.world,
    }
}

/// Where an effect takes its draws.
pub trait Draws {
    /// One value of the shared generator.
    fn shared(&mut self) -> u32;
    /// One `rand()`.
    fn rand(&mut self) -> i32;
    /// A shared draw as the effects take it: its low 31 bits over 2³¹
    /// (`and 0x7fffffff`, `cvt.s.w`, times `0x30000000`).
    fn unit(&mut self) -> f32 {
        (self.shared() & 0x7fff_ffff) as f32 * f32::from_bits(0x3000_0000)
    }
}

impl Draws for street_sim::World {
    fn shared(&mut self) -> u32 {
        self.shared_draw()
    }

    fn rand(&mut self) -> i32 {
        self.libc_rand()
    }
}

/// A stand-in for the tests: a generator of its own.
#[cfg(test)]
pub struct Own(pub u32);

#[cfg(test)]
impl Draws for Own {
    fn shared(&mut self) -> u32 {
        let mut x = self.0.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    fn rand(&mut self) -> i32 {
        (self.shared() >> 1) as i32
    }
}
