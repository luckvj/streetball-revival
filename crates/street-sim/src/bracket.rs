//! Be A Legend's tournament brackets, as the game keeps them (story block
//! +0x4c, 0x104 bytes a tournament; screens `singleko.txt`,
//! `doubleko.txt`, `bestof3.txt`).
//!
//! A bracket is 16 rows of one match each: two entrants, two 32-bit crew
//! numbers, and a bit per game won by each side. The event's type byte
//! (`+0x1b`) picks one of five shapes (the table at `0x460a10`): per round
//! the games in a series and the rows that play (`0x460bb8`..`0x460c18`):
//!
//! | Type | What | Rounds (rows) |
//! |---|---|---|
//! | 0 | single elimination, 8 crews | 0..3, 8..9, 12 |
//! | 1 | single elimination, 16 crews | 0..7, 8..11, 12..13, 14 |
//! | 2 | double elimination, 4 crews | 0..1, then 8 (winners) and 2 (losers), 9, 12 |
//! | 3 | best of three, one series | 0 |
//! | 4 | best of three, semis and a final | 0..1, 8 |
//!
//! A row's winner moves to the slot `0x460a38` names for it, and in double
//! elimination the loser to the one `0x460a58` names (`0x1ea528`). A series
//! is won on more than half its games (`0x1e90a8`).
//!
//! Seeding (`0x1e9888`): you in the first match of round one, the event's
//! boss crew in the last (generic crews instead once the tournament has
//! been won), generic crews everywhere else, each with its own number
//! counted up from a random base; one-series brackets put you straight
//! against the boss. Matches between the computer's crews are decided by
//! a coin flip a game (`0x3b42a0` when the winner is passed as -1).

/// Who plays in a bracket slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Entrant {
    #[default]
    Empty,
    /// The player's crew (team 0x25).
    You,
    /// The event's boss crew (an XDBTEAM record).
    Boss(u16),
    /// A generic crew (team 0x24) by its number.
    Crew(u32),
}

/// One row: two entrants and the games each has won (one bit a game).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Match {
    pub sides: [Entrant; 2],
    pub wins: [u8; 2],
}

pub const ROWS: usize = 16;

/// The five shapes: per round, the games in a series and the rows.
const SHAPES: [&[(u8, &[u8])]; 5] = [
    &[(1, &[0, 1, 2, 3]), (1, &[8, 9]), (1, &[12])],
    &[
        (1, &[0, 1, 2, 3, 4, 5, 6, 7]),
        (1, &[8, 9, 10, 11]),
        (1, &[12, 13]),
        (1, &[14]),
    ],
    &[(1, &[0, 1]), (1, &[8, 2]), (1, &[9]), (1, &[12])],
    &[(3, &[0])],
    &[(3, &[0, 1]), (3, &[8])],
];

/// Where a row's winner goes: (row, side) (`0x460a38`).
const WINNER: [Option<(u8, u8)>; ROWS] = [
    Some((8, 0)),
    Some((8, 1)),
    Some((9, 0)),
    Some((9, 1)),
    Some((10, 0)),
    Some((10, 1)),
    Some((11, 0)),
    Some((11, 1)),
    Some((12, 0)),
    Some((12, 1)),
    Some((13, 0)),
    Some((13, 1)),
    Some((14, 0)),
    Some((14, 1)),
    Some((15, 0)),
    None,
];

/// Where a row's loser goes in double elimination (`0x460a58`).
const LOSER: [Option<(u8, u8)>; ROWS] = [
    Some((2, 0)),
    Some((2, 1)),
    None,
    None,
    None,
    None,
    None,
    None,
    Some((9, 1)),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
];

/// The rounds' names on the bracket screens.
pub fn round_name(kind: u8, round: usize) -> &'static str {
    match (kind, round) {
        (0, 0) | (1, 1) => "Quarters",
        (0, 1) | (1, 2) => "Semis",
        (1, 0) | (2, 0) => "Round 1",
        (0, _) | (1, _) | (2, 3) => "Finals",
        (2, 1) => "Winner's and Loser's Brackets",
        (2, _) => "Loser's Bracket Final",
        (3, _) => "Final Series",
        (4, 0) => "Series 1",
        (4, _) => "Final Series",
        _ => "",
    }
}

/// A tournament in progress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bracket {
    /// The shape, 0..4.
    pub kind: u8,
    pub rows: [Match; ROWS],
    /// For the computer's coin flips.
    pub rng: u32,
}

impl Bracket {
    /// A fresh bracket of shape `kind` (0..4) with `boss` (None once the
    /// tournament is won), crews numbered from `seed`.
    pub fn new(kind: u8, boss: Option<u16>, seed: u32) -> Self {
        let kind = kind.min(SHAPES.len() as u8 - 1);
        let mut b = Bracket {
            kind,
            rows: [Match::default(); ROWS],
            rng: seed,
        };
        let first = SHAPES[usize::from(kind)][0].1;
        let mut n = seed;
        let mut crew = || {
            n = n.wrapping_add(1);
            Entrant::Crew(n)
        };
        let rival = boss.map_or_else(&mut crew, Entrant::Boss);
        if first.len() < 2 {
            b.rows[usize::from(first[0])].sides = [Entrant::You, rival];
            return b;
        }
        let last = first.len() - 1;
        for (k, &row) in first.iter().enumerate() {
            let left = if k == 0 {
                Entrant::You
            } else if k == last {
                rival
            } else {
                crew()
            };
            b.rows[usize::from(row)].sides = [left, crew()];
        }
        b
    }

    fn shape(&self) -> &'static [(u8, &'static [u8])] {
        SHAPES[usize::from(self.kind)]
    }

    pub fn rounds(&self) -> usize {
        self.shape().len()
    }

    /// The rows playing in `round`.
    pub fn round_rows(&self, round: usize) -> &'static [u8] {
        self.shape().get(round).map_or(&[], |r| r.1)
    }

    /// Games in a series of `round`.
    pub fn games(&self, round: usize) -> u8 {
        self.shape().get(round).map_or(1, |r| r.0)
    }

    fn round_of(&self, row: usize) -> Option<usize> {
        self.shape().iter().position(|r| r.1.contains(&(row as u8)))
    }

    /// The winning side of a row, once its series is decided.
    pub fn decided(&self, row: usize) -> Option<usize> {
        let round = self.round_of(row)?;
        let need = u32::from(self.games(round) / 2 + 1);
        let m = &self.rows[row];
        (0..2).find(|&s| m.wins[s].count_ones() >= need)
    }

    /// Your next match: (round, row, your side). `None` once you are out
    /// or have won.
    pub fn current(&self) -> Option<(usize, usize, usize)> {
        for (round, &(_, rows)) in self.shape().iter().enumerate() {
            for &row in rows {
                let row = usize::from(row);
                let m = &self.rows[row];
                if let Some(side) = m.sides.iter().position(|&e| e == Entrant::You)
                    && self.decided(row).is_none()
                {
                    // Both sides must be known to play.
                    if m.sides[1 - side] == Entrant::Empty {
                        return None;
                    }
                    return Some((round, row, side));
                }
            }
        }
        None
    }

    /// The champion's slot: where the final's winner goes.
    pub fn champion(&self) -> Entrant {
        let final_row = self.shape().last().map_or(0, |r| usize::from(r.1[0]));
        match WINNER[final_row] {
            Some((row, side)) => self.rows[usize::from(row)].sides[usize::from(side)],
            None => Entrant::Empty,
        }
    }

    pub fn won(&self) -> bool {
        self.champion() == Entrant::You
    }

    /// You lost your way out (or the bracket is over without you).
    pub fn out(&self) -> bool {
        !self.won() && self.current().is_none()
    }

    /// A game of `row` won by `side`: its bit set, and a decided series
    /// moves the winner (and in double elimination the loser) on.
    pub fn record(&mut self, row: usize, side: usize) {
        if self.decided(row).is_some() {
            return;
        }
        let m = &mut self.rows[row];
        let played = (m.wins[0] | m.wins[1]).count_ones();
        m.wins[side] |= 1 << played;
        let sides = m.sides;
        if self.decided(row) == Some(side) {
            let (winner, loser) = (sides[side], sides[1 - side]);
            if let Some((r, s)) = WINNER[row] {
                self.rows[usize::from(r)].sides[usize::from(s)] = winner;
            }
            if self.kind == 2
                && let Some((r, s)) = LOSER[row]
            {
                self.rows[usize::from(r)].sides[usize::from(s)] = loser;
            }
        }
    }

    /// Native C-library LCG `rand()` at `0x003b42a0` and computer match resolution
    /// at `FUN_001ea528`:
    /// `seed = seed.wrapping_mul(0x41c64e6d).wrapping_add(0x3039)`
    /// `winner = ((rand_val ^ 1) & 1)`
    pub(crate) fn flip(&mut self) -> usize {
        self.rng = self.rng.wrapping_mul(0x41c64e6d).wrapping_add(0x3039);
        let rand_val = self.rng & 0x7fffffff;
        ((rand_val ^ 1) & 1) as usize
    }

    /// The computer's matches up to `round`, in order: every row both of
    /// whose sides are known and neither is you, played out game by game.
    pub fn play_out(&mut self, round: usize) {
        for r in 0..=round.min(self.rounds().saturating_sub(1)) {
            for &row in self.round_rows(r) {
                let row = usize::from(row);
                loop {
                    let m = self.rows[row];
                    if self.decided(row).is_some()
                        || m.sides.contains(&Entrant::You)
                        || m.sides.contains(&Entrant::Empty)
                    {
                        break;
                    }
                    let side = self.flip();
                    self.record(row, side);
                }
            }
        }
    }

    /// A game of yours: recorded, then the computer's games that are due.
    pub fn play(&mut self, won: bool) {
        if let Some((_, row, side)) = self.current() {
            self.record(row, if won { side } else { 1 - side });
            // Every match of the computer's that can be played is (they
            // never wait on yours): in double elimination the losers'
            // final must be played before you have a grand final.
            self.play_out(self.rounds());
        }
    }

    /// The series score of a row: games won by each side.
    pub fn score(&self, row: usize) -> [u32; 2] {
        let m = &self.rows[row];
        [m.wins[0].count_ones(), m.wins[1].count_ones()]
    }

    // -- persistence -------------------------------------------------------

    pub fn encode(&self, out: &mut Vec<u8>) {
        out.push(self.kind);
        out.extend_from_slice(&self.rng.to_le_bytes());
        for m in &self.rows {
            for e in m.sides {
                let (tag, v) = match e {
                    Entrant::Empty => (0u8, 0u32),
                    Entrant::You => (1, 0),
                    Entrant::Boss(t) => (2, u32::from(t)),
                    Entrant::Crew(n) => (3, n),
                };
                out.push(tag);
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.extend_from_slice(&m.wins);
        }
    }

    /// Reads [`Self::encode`] from the front of `b`; the bytes used.
    pub fn decode(b: &[u8]) -> Option<(Self, usize)> {
        let mut at = 0usize;
        let mut take = |n: usize| -> Option<&[u8]> {
            let s = b.get(at..at + n)?;
            at += n;
            Some(s)
        };
        let kind = take(1)?[0];
        if usize::from(kind) >= SHAPES.len() {
            return None;
        }
        let rng = u32::from_le_bytes(take(4)?.try_into().ok()?);
        let mut rows = [Match::default(); ROWS];
        for m in &mut rows {
            for side in &mut m.sides {
                let tag = take(1)?[0];
                let v = u32::from_le_bytes(take(4)?.try_into().ok()?);
                *side = match tag {
                    0 => Entrant::Empty,
                    1 => Entrant::You,
                    2 => Entrant::Boss(v as u16),
                    3 => Entrant::Crew(v),
                    _ => return None,
                };
            }
            let w = take(2)?;
            m.wins = [w[0], w[1]];
        }
        Some((Bracket { kind, rows, rng }, at))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_elimination_is_three_wins_to_the_title() {
        let mut b = Bracket::new(0, Some(40), 100);
        assert_eq!(b.rows[0].sides[0], Entrant::You);
        assert_eq!(b.rows[3].sides[0], Entrant::Boss(40));
        assert_eq!(b.current(), Some((0, 0, 0)));
        b.play(true);
        // The other quarters are played; you're in the first semi.
        assert_eq!(b.current().map(|c| (c.0, c.1)), Some((1, 8)));
        b.play(true);
        assert_eq!(b.current().map(|c| (c.0, c.1)), Some((2, 12)));
        assert!(!b.won());
        b.play(true);
        assert!(b.won());
        assert!(b.current().is_none());
    }

    #[test]
    fn a_loss_knocks_you_out_of_single_elimination() {
        let mut b = Bracket::new(0, Some(40), 7);
        b.play(false);
        assert!(b.out());
    }

    #[test]
    fn double_elimination_gives_a_second_chance() {
        let mut b = Bracket::new(2, Some(44), 1);
        // Lose round one: into the losers' bracket (row 2).
        b.play(false);
        assert_eq!(b.current().map(|c| c.1), Some(2));
        b.play(true);
        // The losers' final (row 9) against the winners' final's loser.
        assert_eq!(b.current().map(|c| c.1), Some(9));
        b.play(true);
        assert_eq!(b.current().map(|c| c.1), Some(12));
        b.play(true);
        assert!(b.won());
        // Two losses and you're out.
        let mut b = Bracket::new(2, Some(44), 1);
        b.play(false);
        b.play(false);
        assert!(b.out());
    }

    #[test]
    fn best_of_three_takes_two_wins() {
        let mut b = Bracket::new(4, Some(43), 9);
        assert_eq!(b.games(0), 3);
        b.play(true);
        assert_eq!(b.current().map(|c| c.1), Some(0));
        b.play(false);
        b.play(true);
        assert_eq!(b.current().map(|c| c.1), Some(8));
        b.play(true);
        b.play(true);
        assert!(b.won());
        let one = Bracket::new(3, Some(43), 9);
        assert_eq!(one.rows[0].sides, [Entrant::You, Entrant::Boss(43)]);
    }

    #[test]
    fn a_bracket_survives_a_save() {
        let mut b = Bracket::new(4, None, 5);
        b.play(true);
        let mut bytes = Vec::new();
        b.encode(&mut bytes);
        let (back, used) = Bracket::decode(&bytes).unwrap();
        assert_eq!(back, b);
        assert_eq!(used, bytes.len());
    }

    #[test]
    fn bracket_computer_match_flip_matches_native_c_library_rand() {
        let mut b = Bracket::new(0, None, 0);
        // Step 1: seed becomes 12345 (odd), winner = (12345 ^ 1) & 1 = 0
        assert_eq!(b.flip(), 0);
        assert_eq!(b.rng, 12345);
        // Step 2: seed becomes 3554416254 (even), winner = (1406932606 ^ 1) & 1 = 1
        assert_eq!(b.flip(), 1);
        assert_eq!(b.rng, 3554416254);
        // Step 3: seed becomes 2802067423 (odd), winner = 0
        assert_eq!(b.flip(), 0);
        assert_eq!(b.rng, 2802067423);
        // Step 4: seed becomes 3596950572 (even), winner = 1
        assert_eq!(b.flip(), 1);
        assert_eq!(b.rng, 3596950572);
    }

    #[test]
    fn shapes_and_destinations_are_the_executable_tables() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let elf = street_assets::iso::Iso::open(path)
            .unwrap()
            .read("/SLUS_206.51")
            .unwrap();
        let bytes = |a: u32, n: usize| {
            street_assets::scripts::executable_words(&elf, a & !3, (n + 3 + (a & 3) as usize) / 4)
                .unwrap()
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .skip((a & 3) as usize)
                .take(n)
                .collect::<Vec<u8>>()
        };
        let w = |a: u32| street_assets::scripts::executable_words(&elf, a, 1).unwrap()[0];
        // 0x460a10: {round count, rounds}; a round is {u8 games, u8 rows,
        // pad, row list}.
        for (kind, shape) in SHAPES.iter().enumerate() {
            let (count, rounds) = (w(0x460a10 + 8 * kind as u32), w(0x460a14 + 8 * kind as u32));
            assert_eq!(shape.len(), count as usize, "type {kind}");
            for (r, &(games, rows)) in shape.iter().enumerate() {
                let head = bytes(rounds + 8 * r as u32, 2);
                let list = bytes(w(rounds + 8 * r as u32 + 4), usize::from(head[1]));
                assert_eq!(
                    (games, rows.to_vec()),
                    (head[0], list),
                    "type {kind} round {r}"
                );
            }
        }
        // 0x460a38 / 0x460a58: a {row, side} byte pair per row, side 0xff
        // for none.
        for (table, at) in [(&WINNER, 0x460a38u32), (&LOSER, 0x460a58)] {
            let pairs = bytes(at, 2 * ROWS);
            for row in 0..ROWS {
                let (to, side) = (pairs[2 * row], pairs[2 * row + 1]);
                assert_eq!(
                    table[row],
                    (side != 0xff).then_some((to, side)),
                    "{at:#x} {row}"
                );
            }
        }
    }

    #[test]
    fn flip_is_the_executable_lcg_and_winner_bit() {
        // STREET_ISO: the constants come from the instructions themselves.
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let elf = street_assets::iso::Iso::open(path)
            .unwrap()
            .read("/SLUS_206.51")
            .unwrap();
        let w = |a: u32| street_assets::scripts::executable_words(&elf, a, 1).unwrap()[0];
        // FUN_003b42a0: lui a0, 0x41c6 / ori a0, a0, 0x4e6d; mult; addiu
        // v1, v1, 0x3039; lui/ori 0x7fffffff (the returned mask).
        assert_eq!((w(0x3b42a4), w(0x3b42ac)), (0x3c04_41c6, 0x3484_4e6d));
        assert_eq!(w(0x3b42c0), 0x2463_3039);
        let multiplier = (w(0x3b42a4) & 0xffff) << 16 | (w(0x3b42ac) & 0xffff);
        let increment = w(0x3b42c0) & 0xffff;
        // FUN_001ea528 (0x1ea5c0..0x1ea5cc): jal rand; xori s1, v0, 1;
        // andi s1, s1, 1.
        assert_eq!(
            [w(0x1ea5c0), w(0x1ea5c8), w(0x1ea5cc)],
            [0x0c0e_d0a8, 0x3851_0001, 0x3231_0001]
        );
        let mut b = Bracket::new(0, None, 0);
        let mut seed = 0u32;
        for _ in 0..64 {
            seed = seed.wrapping_mul(multiplier).wrapping_add(increment);
            assert_eq!(b.flip(), (((seed & 0x7fff_ffff) ^ 1) & 1) as usize);
            assert_eq!(b.rng, seed);
        }
    }
}
