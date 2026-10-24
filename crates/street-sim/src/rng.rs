//! The original six-word carry-add generator, FUN_003a16b8/003a1790.
//! Identical state and draw order replay identically. Original caller draw
//! order remains a separate obligation from this instruction-level port.
//!
//! The original keeps one instance (`0x4309f0`) for the whole session: the
//! boot seeds it from the clock ([`boot_seed`]), a team set for a side no
//! person holds reseeds it from the 100 Hz timer (`FUN_001dbe30`), and it
//! runs on through the front end and from game to game (docs/TRACE.md "The
//! session's generator"). A game built on it is
//! [`crate::World::from_generator`].

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng([u32; 6]);

#[cfg(test)]
#[path = "rng_capture_parity.rs"]
mod capture_parity;

/// The seed the boot gives both generators (`FUN_002f5578`, raw
/// `0x2f5578..`): the clock's second, minute and hour bytes as
/// `sceCdReadClock` returns them (`FUN_003ad8e8`, binary-coded decimal, not
/// converted), `s + m × 60 + h × 3600 + 1`, into `FUN_003b4290` (the libc
/// LCG) and `FUN_003a1790` (this generator).
pub fn boot_seed(hour: u8, minute: u8, second: u8) -> u32 {
    let bcd = |v: u8| u32::from(v / 10 * 16 + v % 10);
    bcd(second) + bcd(minute) * 60 + bcd(hour) * 3600 + 1
}

impl Rng {
    /// Native seeding uses six successive wrapping additions, without an
    /// extra host-side scramble or a special case for zero.
    pub fn new(seed: u32) -> Self {
        let mut value = seed;
        Self(
            [
                0xf22d0e56, 0x96041893, 0x3df3b646, 0x40dde76d, 0x97327ae1, 0xd1a9fbe7,
            ]
            .map(|increment| {
                value = value.wrapping_add(increment);
                value
            }),
        )
    }

    /// The generator's six words (`0x4309f0..0x430a08`), as a snapshot
    /// ([`crate::snapshot`]) and a capture record them.
    pub fn state(&self) -> [u32; 6] {
        self.0
    }

    /// All six native words, including the valid all-zero carry-wrap state.
    pub fn from_state(state: [u32; 6]) -> Self {
        Self(state)
    }

    /// The seed `FUN_003a1790` set this state from, when it is one: every
    /// word the seed plus its running increment.
    pub fn seeded_by(&self) -> Option<u32> {
        let fresh = Self::new(0).0;
        let seed = self.0[0].wrapping_sub(fresh[0]);
        (1..6)
            .all(|k| self.0[k].wrapping_sub(fresh[k]) == seed)
            .then_some(seed)
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut carry = 0_u64;
        for index in (0..5).rev() {
            let sum = u64::from(self.0[index]) + u64::from(self.0[index + 1]) + carry;
            self.0[index] = sum as u32;
            carry = sum >> 32;
        }
        for index in (0..6).rev() {
            self.0[index] = self.0[index].wrapping_add(1);
            if self.0[index] != 0 {
                break;
            }
        }
        self.0[0]
    }

    /// One draw undone: the last word's count, then the add-with-carry
    /// chain from the bottom word up (for walking a captured state back).
    #[cfg(test)]
    pub(crate) fn previous(&mut self) {
        for word in self.0.iter_mut().rev() {
            let wrapped = *word == 0;
            *word = word.wrapping_sub(1);
            if !wrapped {
                break;
            }
        }
        let after = self.0;
        let mut carry = 0_u64;
        for index in (0..5).rev() {
            let below = u64::from(after[index + 1]);
            let word = u64::from(after[index])
                .wrapping_sub(below)
                .wrapping_sub(carry)
                & 0xffff_ffff;
            self.0[index] = word as u32;
            carry = (word + below + carry) >> 32;
        }
    }

    /// Uniform in 0..1.
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// True with probability `p`.
    pub fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replays_and_spreads() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        let xs: Vec<f32> = (0..1000).map(|_| a.unit()).collect();
        let ys: Vec<f32> = (0..1000).map(|_| b.unit()).collect();
        assert_eq!(xs, ys);
        let mean = xs.iter().sum::<f32>() / xs.len() as f32;
        assert!((mean - 0.5).abs() < 0.05);
        assert!(xs.iter().all(|x| (0.0..1.0).contains(x)));
    }

    #[test]
    fn seed_and_draws_match_original_integer_instructions() {
        // Offline execution of the original ELF's instructions, retained in
        // street-native-rng-instruction-reference.json, not a copied kernel.
        let mut rng = Rng::new(0);
        assert_eq!(
            rng.state(),
            [
                4063039062, 2284922601, 3324304687, 117621916, 2654289789, 1876900708
            ]
        );
        assert_eq!(
            (0..4).map(|_| rng.next_u32()).collect::<Vec<_>>(),
            [1436176877, 659466229, 3894933472, 1991661106]
        );
    }

    #[test]
    fn full_counter_carry_preserves_zero_state() {
        let mut rng = Rng::from_state([0, 0, 0, 0, 0, u32::MAX]);
        assert_eq!(rng.next_u32(), 0);
        assert_eq!(rng.state(), [0; 6]);
        assert_eq!(rng.next_u32(), 0);
        assert_eq!(rng.state(), [0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn a_draw_undone_lands_on_the_state_before_it() {
        let mut rng = Rng::new(18_061);
        let states: Vec<[u32; 6]> = (0..2000)
            .map(|_| {
                let s = rng.state();
                rng.next_u32();
                s
            })
            .collect();
        for s in states.iter().rev() {
            rng.previous();
            assert_eq!(rng.state(), *s);
        }
        assert_eq!(rng.seeded_by(), Some(18_061));
        rng.next_u32();
        assert_eq!(rng.seeded_by(), None);
        // The carry chain's edges: every word at its wrap.
        for state in [[u32::MAX; 6], [0; 6], [0, u32::MAX, 0, u32::MAX, 0, 0]] {
            let mut r = Rng::from_state(state);
            r.next_u32();
            r.previous();
            assert_eq!(r.state(), state);
        }
    }

    #[test]
    fn the_boot_seed_reads_the_clocks_bytes_as_decimal_digits() {
        // 12:34:56 reads 0x12, 0x34, 0x56.
        assert_eq!(boot_seed(12, 34, 56), 0x56 + 0x34 * 60 + 0x12 * 3600 + 1);
        assert_eq!(boot_seed(0, 0, 0), 1);
        // The largest: 23:59:59.
        assert_eq!(boot_seed(23, 59, 59), 0x59 + 0x59 * 60 + 0x23 * 3600 + 1);
    }
}
