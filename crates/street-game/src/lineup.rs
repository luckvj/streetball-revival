//! Who plays for a side no person holds: `FUN_001dbe30(setup, side, seed,
//! team, out)`, which `FUN_001dbb20` calls when a team is set for a side
//! with no controller on it (Choose Team's commit, the attract match's
//! picks, a story game's opponent, `FUN_001e91a0`).
//!
//! It seeds the C library's generator (and the session's) with `seed`, or
//! with the 100 Hz timer's count when none is given, then fills the side's
//! three slots by the game's mode (`setup + 0x78`):
//!
//! - *Pick Up* (modes 0, 2, 3): a team's first three (`FUN_001ebf80`); league
//!   Stars (0x27) draws from every league team's first five with the star flag
//!   (field 28), Old School (0x26) from the legends 0x9e..0xb7 unlocked
//!   (`FUN_001d9ca0`), each list kept apart from the players the other side
//!   holds (`FUN_001db3e0`), which wait in a spare list.
//! - *The League Challenge* (mode 4): the team's first three.
//! - *Be A Legend* (mode 5), by the event (`setup + 0xf28`): a pick-up's or
//!   a tournament's generic crew (0x24) is drawn from five lists by level
//!   (`FUN_001d7e40`), each list shuffled and cut to its share of the
//!   court's tier mix (`FUN_001d3880`); a boss crew is its rival and two
//!   generic ballers of its level (teams 0x28, 0x2a..0x2c) or its fixed
//!   three (0x29, 0x2d); a street challenge draws by its court.
//!
//! The lists are shuffled with `std::random_shuffle` over the C library's
//! `rand` (`j = rand() % (i + 1)` for each `i` from 1), and the slots are
//! taken from a list's end. Generic crews, All-Stars and Old School keep a
//! woman only one draw in three and never two of a play style
//! (`FUN_001d8088`, `FUN_001db438`). A tournament's crews are drawn from the
//! bracket's one seed (your entry's, `FUN_001e8790(.., 0, 0, 0)`), three
//! draws thrown away a round you are in (`+0x94`, `uStack_c4`), so each
//! round meets the next crew of the same stream.

use street_assets::db;
use street_sim::story::{self, EventKind};

use crate::session::Libc;

/// A side's three, `None` where a slot is empty (-1).
pub type Lineup = [Option<usize>; 3];

/// The teams the chooser names (`docs/FORMATS.md`, Teams).
const LEAGUE_TEAMS: usize = 29;
const GENERIC: usize = 0x24;
const CREATED: i16 = 0x25;
const OLD_SCHOOL: usize = 0x26;
const LEAGUE_STARS: usize = 0x27;
/// The legends of Rucker Park '78 (the street challenge on court 10).
const RUCKER_LEGENDS: usize = 0x2e;
/// The records the lists walk: the generic ballers, the streetballers and
/// the legends.
const GENERIC_BALLERS: std::ops::Range<usize> = 0xb8..0xea;
const STREETBALLERS: std::ops::Range<usize> = 0x91..0x9e;
const LEGENDS: std::ops::Range<usize> = 0x9e..0xb8;

/// The rival triples at `0x4609e0`: a boss team and the event that frees
/// its players for the career's opponents (`FUN_001e79a0`).
const RIVALS: [(usize, usize); 7] = [
    (40, 2),
    (42, 3),
    (44, 5),
    (43, 7),
    (41, 8),
    (45, 9),
    (46, 10),
];

/// The level thresholds at `0x45fd78` (`FUN_001d7e40`): a player's level is
/// the first whose mean skill bar it does not pass.
const LEVELS: [f32; 5] = [1.75, 2.49, 3.0, 3.45, 6.0];

/// What the chooser reads of a player record.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Card {
    /// Field 0, the persistent id (a created player's is compared).
    pub id: i16,
    /// Field 3, the player's team.
    pub team: i16,
    /// Field 12.
    pub female: bool,
    /// Field 27, the play style (-1 to work it out from the bars).
    pub style: i16,
    /// Field 28.
    pub star: bool,
    /// The seven skill bars, 0..6 each (`FUN_001d7f48`).
    pub bars: [u8; 7],
}

impl Card {
    pub fn of(p: &db::Player) -> Self {
        use crate::progression_trace::skill_bar;
        Card {
            id: p.id,
            team: p.team,
            female: p.female,
            style: p.style,
            star: p.star,
            bars: db::Skill::ALL.map(|s| skill_bar(s, p.ratings[s.rating()])),
        }
    }

    /// `FUN_001d7e40`: the mean of the seven bars against [`LEVELS`], 0..4.
    pub fn level(&self) -> usize {
        let sum: u32 = self.bars.iter().map(|&b| u32::from(b)).sum();
        let mean = sum as f32 / 7.0;
        LEVELS.iter().position(|&t| mean <= t).unwrap_or(4)
    }

    /// `FUN_001d8088`: field 27 when it is 0..4, 0 past it, and for -1 the
    /// style of the best bar among dunks (1), handles (2), shots (0), blocks
    /// (3) and rebounds (4), the first of equal bars winning.
    pub fn style(&self) -> i16 {
        match self.style {
            0..=4 => self.style,
            s if s > 4 => 0,
            _ => {
                let mut best = (0, -1i16);
                for (style, skill) in [(1, 1), (2, 5), (0, 0), (3, 3), (4, 6)] {
                    let bar = i16::from(self.bars[skill]);
                    if bar > best.1 {
                        best = (style, bar);
                    }
                }
                best.0
            }
        }
    }
}

/// The records, the teams' first five and the courts' tier mixes.
#[derive(Clone, Debug, Default)]
pub struct Book {
    pub cards: Vec<Card>,
    /// Fields 4..8 of each team, `None` for -1.
    pub rosters: Vec<[Option<usize>; 5]>,
    /// Each court's fields 5..9 (`FUN_001d3880`).
    pub tiers: Vec<[f32; 5]>,
}

impl Book {
    pub fn of(disc: &crate::setup::Disc) -> Self {
        Book {
            cards: disc.players.iter().map(Card::of).collect(),
            rosters: disc
                .teams
                .iter()
                .map(|t| std::array::from_fn(|k| t.roster.get(k).copied()))
                .collect(),
            tiers: disc.courts.iter().map(|c| c.tier_mix).collect(),
        }
    }

    pub fn card(&self, record: usize) -> Card {
        self.cards.get(record).copied().unwrap_or_default()
    }

    /// `FUN_001ebf80(team, k)` for `k` < 5.
    pub fn player(&self, team: usize, k: usize) -> Option<usize> {
        self.rosters.get(team).and_then(|r| r[k])
    }

    fn on(&self, team: usize, record: usize) -> bool {
        self.rosters
            .get(team)
            .is_some_and(|r| r.contains(&Some(record)))
    }

    fn tier_mix(&self, court: usize) -> [f32; 5] {
        self.tiers.get(court).copied().unwrap_or_default()
    }
}

/// A Be A Legend career, as the chooser reads it.
pub struct Career<'a> {
    /// The event being played (`setup + 0xf28`).
    pub event: usize,
    /// The career's team (`career + 0xca0`): the crew.
    pub crew: &'a [usize],
    /// The created ballers' records (the list at `+0x33f4`), the one the
    /// setup holds (`+0xd0`) left out.
    pub created: &'a [usize],
    /// `FUN_001d9ca0`.
    pub unlocked: &'a dyn Fn(usize) -> bool,
    /// `FUN_001e7ff8`.
    pub cleared: &'a dyn Fn(usize) -> bool,
    /// The draws of three a generic crew throws away (your round in the
    /// bracket, or in double elimination the opponent's entry past yours,
    /// `FUN_001e86e0`).
    pub round: u32,
}

impl Career<'_> {
    /// `FUN_001e79a0`: whether a player may play against the career: not on
    /// its team and unlocked; a created player not the career's own; a boss
    /// team's player once its event is cleared; no streetballer the story
    /// keeps (0x97, 0x98, 0x9a..0x9d); no generic baller of Bonafide's or
    /// Osmosis's (teams 0x29 and 0x2d, slots 1 and 2).
    pub fn available(&self, book: &Book, record: usize) -> bool {
        if self.crew.contains(&record) || !(self.unlocked)(record) {
            return false;
        }
        let card = book.card(record);
        if card.team == CREATED {
            return !self.crew.iter().any(|&m| {
                let m = book.card(m);
                m.team == CREATED && m.id == card.id
            });
        }
        if RIVALS
            .iter()
            .any(|&(team, ev)| book.on(team, record) && !(self.cleared)(ev))
        {
            return false;
        }
        if card.team != GENERIC as i16 {
            return !matches!(record, 0x97 | 0x98 | 0x9a..=0x9d);
        }
        let fixed = |team: usize| (1..3).any(|k| book.player(team, k) == Some(record));
        !fixed(0x2d) && !fixed(0x29)
    }
}

/// `FUN_001d9ca0` as the rewrite keeps unlocks: a streetballer's or a
/// legend's record (teams 29..35) once the story has unlocked it, every
/// other record always.
pub fn unlocked<'a>(
    disc: &'a crate::setup::Disc,
    story: &'a story::Story,
) -> impl Fn(usize) -> bool + 'a {
    move |record| {
        let team = disc.players.get(record).map_or(0, |p| p.team);
        !(29..=35).contains(&team)
            || u16::try_from(record).is_ok_and(|r| story.unlocked_players.contains(&r))
    }
}

/// The game's mode (`setup + 0x78`).
pub enum Mode<'a> {
    /// Pick Up (and the attract match): `unlocked` is `FUN_001d9ca0`.
    PickUp { unlocked: &'a dyn Fn(usize) -> bool },
    /// The League Challenge.
    League,
    /// Be A Legend.
    Legend(Career<'a>),
}

/// `std::random_shuffle` over the C library's `rand`.
fn shuffle(list: &mut [usize], libc: &mut Libc) {
    for i in 1..list.len() {
        let j = libc.rand() as usize % (i + 1);
        list.swap(i, j);
    }
}

/// `FUN_001dbe30` for `team` on a side whose opponents hold `other`: the
/// three it plays. `seed` is the seed given (a tournament's); without one
/// the caller has seeded `libc` from the timer. A slot no list can fill
/// gets record 0, as the original's does.
pub fn choose(
    book: &Book,
    mode: &Mode,
    team: usize,
    other: Lineup,
    seed: Option<u32>,
    libc: &mut Libc,
) -> [usize; 3] {
    if let Some(seed) = seed {
        *libc = Libc::seeded(seed);
    }
    let mut lineup: Lineup = [None; 3];
    // The lists the slots are taken from (`psStack_320`), the spare one
    // (`puStack_300`), whether the first is shuffled once more
    // (`iStack_c0`), and the draws of three to throw away (`uStack_c4`).
    let mut main: Vec<usize> = Vec::new();
    let mut spare: Vec<usize> = Vec::new();
    let mut shuffle_main = true;
    let mut skip = 0u32;
    let first_three = |lineup: &mut Lineup| {
        for (k, slot) in lineup.iter_mut().enumerate() {
            *slot = book.player(team, k);
        }
    };
    match mode {
        Mode::PickUp { unlocked } => {
            let mut sort = |p: usize| {
                if other.contains(&Some(p)) {
                    spare.push(p);
                } else {
                    main.push(p);
                }
            };
            match team {
                LEAGUE_STARS => {
                    for t in 0..LEAGUE_TEAMS {
                        for k in 0..5 {
                            if let Some(p) = book.player(t, k)
                                && book.card(p).star
                            {
                                sort(p);
                            }
                        }
                    }
                }
                OLD_SCHOOL => LEGENDS.filter(|&p| unlocked(p)).for_each(sort),
                _ => first_three(&mut lineup),
            }
        }
        Mode::League => first_three(&mut lineup),
        Mode::Legend(career) => {
            let ev = &story::EVENTS[career.event.min(story::EVENTS.len() - 1)];
            if ev.kind == EventKind::StreetChallenge {
                street(book, career, &mut lineup, &mut main, libc);
                shuffle_main = usize::from(ev.court) != 10;
            } else {
                match team {
                    0x28 | 0x2a..=0x2c => {
                        // The rival, then generic ballers of the crew's level.
                        lineup[0] = book.player(team, 0);
                        let want = if team >= 0x2b { 2 } else { 1 };
                        main.extend(GENERIC_BALLERS.filter(|&p| {
                            career.available(book, p) && book.card(p).level() == want
                        }));
                    }
                    0x29 | 0x2d => first_three(&mut lineup),
                    _ => {
                        let mut exclude: Vec<usize> = Vec::new();
                        if ev.kind == EventKind::Tournament {
                            let boss = usize::from(ev.boss_team);
                            if !(career.cleared)(career.event) && boss < 0xff && boss != GENERIC {
                                // The boss's own three first (`FUN_001dbe30`
                                // called for the boss team into a list),
                                // then the seed again.
                                exclude = choose(book, mode, boss, [None; 3], seed, libc).to_vec();
                                if let Some(seed) = seed {
                                    *libc = Libc::seeded(seed);
                                }
                            }
                            skip = career.round * 3;
                        }
                        tiers(book, career, &exclude, &mut main, libc);
                    }
                }
            }
        }
    }
    // `LAB_001dde3c`: short of three, the spare list (shuffled) makes up
    // the rest from its end.
    if main.len() < 3 && !spare.is_empty() {
        shuffle(&mut spare, libc);
        while main.len() < 3
            && let Some(p) = spare.pop()
        {
            main.push(p);
        }
    }
    if !main.is_empty() && shuffle_main {
        shuffle(&mut main, libc);
    }
    let filtered = matches!(team, GENERIC | OLD_SCHOOL | LEAGUE_STARS);
    let mut spare_taken = false;
    let mut slot = 0;
    while slot < 3 {
        if lineup[slot].is_some() {
            slot += 1;
            continue;
        }
        if !spare_taken {
            if filtered && !main.is_empty() {
                // A woman one draw in three, and no second player of a style.
                let mut roll = libc.rand();
                while let Some(&p) = main.last() {
                    let card = book.card(p);
                    if !card.female || roll.is_multiple_of(3) {
                        let style = card.style();
                        let taken = lineup
                            .iter()
                            .flatten()
                            .any(|&q| book.card(q).style() == style);
                        if !taken {
                            break;
                        }
                    }
                    spare.push(p);
                    main.pop();
                    roll = libc.rand();
                }
            }
            if main.is_empty() && !spare.is_empty() {
                while let Some(p) = spare.pop() {
                    main.push(p);
                }
                spare_taken = true;
            }
        }
        match main.pop() {
            None => lineup[slot] = Some(0),
            Some(p) => {
                lineup[slot] = Some(p);
                if skip > 0 {
                    skip -= 1;
                    if skip.is_multiple_of(3) {
                        lineup = [None; 3];
                        slot = 0;
                        continue;
                    }
                }
            }
        }
        slot += 1;
    }
    lineup.map(|p| p.unwrap_or(0))
}

/// The league teams' first five, in team order.
fn league(book: &Book) -> impl Iterator<Item = usize> + '_ {
    (0..LEAGUE_TEAMS).flat_map(move |t| (0..5).filter_map(move |k| book.player(t, k)))
}

/// A generic crew's list (the default case of `FUN_001dbe30`'s mode 5):
/// the league's first fives, the generic ballers, the streetballers and the
/// created ballers who may play, in five lists by level, each shuffled and
/// cut to the court's share of its level.
fn tiers(book: &Book, career: &Career, exclude: &[usize], main: &mut Vec<usize>, libc: &mut Libc) {
    let mut lists: [Vec<usize>; 5] = Default::default();
    let candidates = league(book)
        .chain(GENERIC_BALLERS)
        .chain(STREETBALLERS)
        .chain(career.created.iter().copied());
    for p in candidates {
        if career.available(book, p) && !exclude.contains(&p) {
            lists[book.card(p).level()].push(p);
        }
    }
    let court = usize::from(story::EVENTS[career.event.min(story::EVENTS.len() - 1)].court);
    let mix = book.tier_mix(court);
    for (level, list) in lists.iter_mut().enumerate() {
        if list.is_empty() {
            continue;
        }
        shuffle(list, libc);
        // `cvt.w.s`: the EE's conversion cuts toward zero.
        let n = (mix[level] * list.len() as f32) as usize;
        main.extend(list.iter().take(n));
    }
}

/// A street challenge's players by its court (`FUN_001dbe30`'s mode 5,
/// event kind 2, the switch on the court).
fn street(
    book: &Book,
    career: &Career,
    lineup: &mut Lineup,
    main: &mut Vec<usize>,
    libc: &mut Libc,
) {
    let court = usize::from(story::EVENTS[career.event.min(story::EVENTS.len() - 1)].court);
    let may = |p: usize| career.available(book, p);
    let level = |p: usize| book.card(p).level();
    let style = |p: usize| book.card(p).style();
    match court {
        1 => {
            // Broad Street: level-2 slashers and playmakers, one slasher
            // and two playmakers at most, taken from the shuffled list.
            main.extend(
                league(book)
                    .chain(GENERIC_BALLERS)
                    .filter(|&p| may(p) && matches!(style(p), 1 | 2) && level(p) == 2),
            );
            if !main.is_empty() {
                shuffle(main, libc);
                let (mut slashers, mut playmakers, mut slot) = (0, 0, 0);
                while slot < 3
                    && let Some(p) = main.pop()
                {
                    let s = style(p);
                    if s == 1 && slashers < 1 {
                        slashers += 1;
                    } else if s == 2 && playmakers < 2 {
                        playmakers += 1;
                    } else {
                        continue;
                    }
                    lineup[slot] = Some(p);
                    slot += 1;
                }
                main.clear();
            }
        }
        2 => main.extend(league(book).filter(|&p| may(p) && style(p) == 1 && level(p) == 3)),
        3 => main.extend(league(book).filter(|&p| may(p) && level(p) == 2)),
        5 => main.extend(
            league(book)
                .chain(GENERIC_BALLERS)
                .chain(STREETBALLERS)
                .chain(career.created.iter().copied())
                .filter(|&p| may(p) && level(p) == 4),
        ),
        7 => main.extend(
            league(book)
                .chain(GENERIC_BALLERS)
                .filter(|&p| may(p) && style(p) != 2 && level(p) == 3),
        ),
        8 => {
            // Lincoln College: a league team's first three, all of whom may
            // play: team 11 first, then the others from the strongest (a
            // stable sort by their three's levels, `FUN_001dac18`).
            let sum =
                |t: usize| -> usize { (0..3).filter_map(|k| book.player(t, k)).map(level).sum() };
            let mut teams: Vec<usize> = (0..LEAGUE_TEAMS).filter(|&t| t != 11).collect();
            teams.sort_by_key(|&t| sum(t));
            teams.push(11);
            while let Some(t) = teams.pop() {
                main.clear();
                for k in 0..3 {
                    match book.player(t, k) {
                        Some(p) if may(p) => main.push(p),
                        _ => {
                            main.clear();
                            break;
                        }
                    }
                }
                if !main.is_empty() {
                    break;
                }
            }
            if main.len() > 2 {
                for (k, slot) in lineup.iter_mut().enumerate() {
                    *slot = Some(main[k]);
                }
            }
        }
        10 => {
            // Rucker Park '78: the legends not on the career's team, the
            // rest from the streetballers and the level-4 league stars, the
            // stars last (so first taken), neither list shuffled again.
            for (k, slot) in lineup.iter_mut().enumerate() {
                if let Some(p) = book.player(RUCKER_LEGENDS, k)
                    && !career.crew.contains(&p)
                {
                    *slot = Some(p);
                }
            }
            let mut ballers: Vec<usize> = STREETBALLERS.filter(|&p| may(p)).collect();
            shuffle(&mut ballers, libc);
            main.extend(ballers);
            let mut stars: Vec<usize> = league(book)
                .filter(|&p| may(p) && level(p) == 4 && book.card(p).star)
                .collect();
            shuffle(&mut stars, libc);
            main.extend(stars);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A book of `n` records, all men of style 0 at the given level's
    /// bars, with team `t`'s first five `5t..5t+4` for the league's 29.
    fn book(n: usize) -> Book {
        let mut b = Book {
            cards: vec![Card::default(); n],
            rosters: vec![[None; 5]; 0x30],
            tiers: vec![[1.0, 0.0, 0.0, 0.0, 0.0]; 11],
        };
        for t in 0..LEAGUE_TEAMS {
            b.rosters[t] = std::array::from_fn(|k| Some(5 * t + k));
        }
        b
    }

    fn bars(level: usize) -> [u8; 7] {
        // Sums 7, 14, 21, 24, 42: means 1, 2, 3, 3.43, 6.
        match level {
            0 => [1; 7],
            1 => [2; 7],
            2 => [3; 7],
            3 => [4, 4, 4, 3, 3, 3, 3],
            _ => [6; 7],
        }
    }

    #[test]
    fn a_level_is_the_first_threshold_the_mean_bar_does_not_pass() {
        for level in 0..5 {
            let c = Card {
                bars: bars(level),
                ..Default::default()
            };
            assert_eq!(c.level(), level);
        }
        // 12 / 7 = 1.71 is level 0, 13 / 7 = 1.86 level 1; 21 / 7 = 3.0
        // exactly is level 2.
        let sum = |s: u8| Card {
            bars: [s, 0, 0, 0, 0, 0, 0],
            ..Default::default()
        };
        assert_eq!(sum(6).level(), 0);
        let mut c = sum(6);
        c.bars[1] = 6;
        assert_eq!(c.level(), 0);
        c.bars[2] = 1;
        assert_eq!(c.level(), 1);
    }

    #[test]
    fn a_style_left_unset_is_the_best_bar_the_first_of_equals_winning() {
        let mut c = Card {
            style: -1,
            bars: [5, 5, 0, 5, 0, 5, 5],
            ..Default::default()
        };
        // Dunks (1) is looked at first.
        assert_eq!(c.style(), 1);
        c.bars[0] = 6;
        assert_eq!(c.style(), 0);
        c.style = 3;
        assert_eq!(c.style(), 3);
        c.style = 7;
        assert_eq!(c.style(), 0);
    }

    #[test]
    fn a_pick_up_team_plays_its_first_three_without_a_draw() {
        let b = book(240);
        let mut libc = Libc::seeded(5);
        let none = |_: usize| true;
        let got = choose(
            &b,
            &Mode::PickUp { unlocked: &none },
            3,
            [None; 3],
            None,
            &mut libc,
        );
        assert_eq!(got, [15, 16, 17]);
        assert_eq!(libc, Libc::seeded(5));
    }

    #[test]
    fn league_stars_draws_stars_of_three_styles_not_on_the_other_side() {
        let mut b = book(240);
        // Stars: records 0, 5, 10, .. (each team's first), styles by team
        // mod 3; the other side holds 0 and 5.
        for t in 0..LEAGUE_TEAMS {
            b.cards[5 * t].star = true;
            b.cards[5 * t].style = (t % 3) as i16;
        }
        let none = |_: usize| true;
        for seed in 0..50 {
            let mut libc = Libc::seeded(seed);
            let got = choose(
                &b,
                &Mode::PickUp { unlocked: &none },
                LEAGUE_STARS,
                [Some(0), Some(5), None],
                None,
                &mut libc,
            );
            let mut styles: Vec<i16> = got.iter().map(|&p| b.card(p).style()).collect();
            styles.sort_unstable();
            assert_eq!(styles, [0, 1, 2], "seed {seed}: {got:?}");
            assert!(got.iter().all(|&p| p % 5 == 0 && p != 0 && p != 5));
        }
    }

    #[test]
    fn old_school_takes_the_other_sides_legends_only_when_short() {
        let mut b = book(240);
        for p in LEGENDS {
            b.cards[p].style = (p % 5) as i16;
        }
        // Three legends unlocked, two on the other side: the third comes
        // from the spare list.
        let unlocked = |p: usize| matches!(p, 0x9e..=0xa0);
        let mut libc = Libc::seeded(9);
        let mut got = choose(
            &b,
            &Mode::PickUp {
                unlocked: &unlocked,
            },
            OLD_SCHOOL,
            [Some(0x9e), Some(0x9f), None],
            None,
            &mut libc,
        );
        got.sort_unstable();
        assert_eq!(got, [0x9e, 0x9f, 0xa0]);
    }

    fn career<'a>(event: usize, crew: &'a [usize], round: u32) -> Career<'a> {
        Career {
            event,
            crew,
            created: &[],
            unlocked: &|_| true,
            cleared: &|_| false,
            round,
        }
    }

    #[test]
    fn a_pick_up_crew_comes_from_the_courts_tier_and_skips_the_crew() {
        let mut b = book(240);
        b.rosters[GENERIC] = [None; 5];
        for p in GENERIC_BALLERS {
            b.cards[p].team = GENERIC as i16;
            b.cards[p].bars = bars(if p < 0xc0 { 0 } else { 3 });
            b.cards[p].style = (p % 5) as i16;
        }
        for p in league(&b).chain(STREETBALLERS).collect::<Vec<_>>() {
            b.cards[p].bars = bars(4);
        }
        // NYC Rec Center: all level 0, so the eight level-0 generic ballers.
        let crew = [0xb8, 0xb9, 0xba];
        let mut libc = Libc::seeded(77);
        let got = choose(
            &b,
            &Mode::Legend(career(0, &crew, 0)),
            GENERIC,
            [None; 3],
            None,
            &mut libc,
        );
        for p in got {
            assert!((0xbb..0xc0).contains(&p), "{got:?}");
        }
    }

    #[test]
    fn a_tournament_round_throws_three_draws_away_a_round() {
        let mut b = book(240);
        for p in GENERIC_BALLERS {
            b.cards[p].team = GENERIC as i16;
            b.cards[p].style = (p % 5) as i16;
        }
        for p in league(&b).collect::<Vec<_>>() {
            b.cards[p].bars = bars(4);
        }
        // Soul in the Hole (event 2) on court 3, all of tier 0 here; its
        // boss cleared, so no boss list first.
        let crew = [0];
        let draw = |round| {
            let c = Career {
                cleared: &|_| true,
                ..career(2, &crew, round)
            };
            choose(
                &b,
                &Mode::Legend(c),
                GENERIC,
                [None; 3],
                Some(1234),
                &mut Libc::seeded(0),
            )
        };
        let (first, second) = (draw(0), draw(1));
        assert_ne!(first, second);
        // The same seed and round, the same crew.
        assert_eq!(draw(1), second);
    }

    #[test]
    fn a_boss_crew_is_its_rival_and_two_generic_ballers_of_its_level() {
        let mut b = book(240);
        b.rosters[0x28] = [Some(0x91), Some(0xb8), Some(0xb9), None, None];
        for p in GENERIC_BALLERS {
            b.cards[p].team = GENERIC as i16;
            b.cards[p].bars = bars(if p % 2 == 0 { 1 } else { 0 });
        }
        let crew = [0];
        let mut libc = Libc::seeded(0);
        let got = choose(
            &b,
            &Mode::Legend(career(2, &crew, 0)),
            0x28,
            [None; 3],
            Some(99),
            &mut libc,
        );
        assert_eq!(got[0], 0x91);
        assert!(
            got[1..]
                .iter()
                .all(|&p| GENERIC_BALLERS.contains(&p) && p % 2 == 0)
        );
    }

    #[test]
    fn lincoln_college_meets_team_11_unless_one_of_its_three_is_taken() {
        let b = book(240);
        let crew = [0];
        let mut libc = Libc::seeded(0);
        let got = choose(
            &b,
            &Mode::Legend(career(15, &crew, 0)),
            GENERIC,
            [None; 3],
            None,
            &mut libc,
        );
        assert_eq!(got, [55, 56, 57]);
        // With 56 on the career's team, the last of the sorted others (all
        // level 0: the stable sort keeps them in order, so team 28).
        let crew = [56];
        let got = choose(
            &b,
            &Mode::Legend(career(15, &crew, 0)),
            GENERIC,
            [None; 3],
            None,
            &mut libc,
        );
        assert_eq!(got, [140, 141, 142]);
    }

    #[test]
    fn rucker_78_plays_its_legends_and_fills_a_taken_one_with_a_star() {
        let mut b = book(240);
        b.rosters[RUCKER_LEGENDS] = [Some(0xb0), Some(0xb1), Some(0xb2), None, None];
        for p in STREETBALLERS {
            b.cards[p].bars = bars(2);
        }
        b.cards[42].star = true;
        b.cards[42].bars = bars(4);
        b.cards[42].style = 4;
        let crew = [0xb1];
        let mut libc = Libc::seeded(0);
        let got = choose(
            &b,
            &Mode::Legend(career(10, &crew, 0)),
            GENERIC,
            [None; 3],
            None,
            &mut libc,
        );
        assert_eq!(got, [0xb0, 42, 0xb2]);
    }

    /// The disc's records through the chooser: prints what it draws (run
    /// with `--nocapture` to read it) and checks the rules hold.
    #[test]
    fn the_discs_teams_through_the_chooser() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let disc = crate::setup::Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let b = Book::of(&disc);
        let name = |p: usize| disc.players[p].name();
        let story = story::Story::default();
        let unlocked = unlocked(&disc, &story);
        let pick_up = Mode::PickUp {
            unlocked: &unlocked,
        };
        // All-Stars: three stars of three styles.
        for seed in [1, 632, 79638] {
            let got = choose(
                &b,
                &pick_up,
                LEAGUE_STARS,
                [None; 3],
                None,
                &mut Libc::seeded(seed),
            );
            let mut styles: Vec<i16> = got.iter().map(|&p| b.card(p).style()).collect();
            styles.sort_unstable();
            styles.dedup();
            assert_eq!(styles.len(), 3, "{got:?}");
            assert!(got.iter().all(|&p| b.card(p).star));
            println!("All-Stars from {seed}: {:?}", got.map(name));
        }
        // A new career's first pick-up at NYC Rec Center: all level 0.
        let crew = [234, 0xb8, 0xb9];
        let career = |event| Career {
            event,
            crew: &crew,
            created: &[],
            unlocked: &unlocked,
            cleared: &|_| false,
            round: 0,
        };
        let got = choose(
            &b,
            &Mode::Legend(career(0)),
            GENERIC,
            [None; 3],
            None,
            &mut Libc::seeded(632),
        );
        assert!(
            got.iter()
                .all(|&p| b.card(p).level() == 0 && !crew.contains(&p))
        );
        println!("NYC Rec Center: {:?}", got.map(name));
        // Lincoln College: team 11 while its three may play.
        let got = choose(
            &b,
            &Mode::Legend(career(15)),
            GENERIC,
            [None; 3],
            None,
            &mut Libc::seeded(1),
        );
        assert_eq!(got.map(Some), std::array::from_fn(|k| b.player(11, k)));
        println!(
            "Lincoln College: {} {:?}",
            disc.teams[11].full_name(),
            got.map(name)
        );
        // Rucker Park '78: the legends' three.
        let got = choose(
            &b,
            &Mode::Legend(career(10)),
            GENERIC,
            [None; 3],
            None,
            &mut Libc::seeded(1),
        );
        assert_eq!(
            got.map(Some),
            std::array::from_fn(|k| b.player(RUCKER_LEGENDS, k))
        );
        println!("Rucker Park '78: {:?}", got.map(name));
        // Soul in the Hole's boss crew: Stretch and two level-1 generics.
        let got = choose(
            &b,
            &Mode::Legend(career(2)),
            0x28,
            [None; 3],
            Some(5),
            &mut Libc::seeded(0),
        );
        assert_eq!(Some(got[0]), b.player(0x28, 0));
        assert!(
            got[1..]
                .iter()
                .all(|&p| GENERIC_BALLERS.contains(&p) && b.card(p).level() == 1)
        );
        println!("Soul in the Hole's boss: {:?}", got.map(name));
        for court in 0..b.tiers.len() {
            println!(
                "court {court} {}: {:?}",
                disc.courts[court].name, b.tiers[court]
            );
        }
    }

    /// Two of the original's draws read from RAM in the project's PCSX2
    /// (2026-10-19): save state 4's career (the crew 245, 204, 227 and 198,
    /// two wins) starting NYC Rec Center's pick-up, which drew 214, 213 and
    /// 229 and left the C library's word at 0xd4ad32fb (the timer read 120540
    /// before the press and 120919 after; 120569 is the one count in that
    /// window that gives both), and save state 7's Soul in the Hole (its
    /// pick-up cleared), whose bracket's seed is 666 and whose first crew is
    /// 133, 94 and 190.
    #[test]
    fn the_originals_draws_from_two_save_states() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let disc = crate::setup::Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let b = Book::of(&disc);
        let crew = [245, 204, 227, 198];
        let unlocked = |r: usize| !(29..=35).contains(&disc.players.get(r).map_or(0, |p| p.team));
        let pick_up = Career {
            event: 0,
            crew: &crew,
            created: &[],
            unlocked: &unlocked,
            cleared: &|_| false,
            round: 0,
        };
        let mode = Mode::Legend(pick_up);
        let mut libc = Libc::seeded(120_569);
        assert_eq!(
            choose(&b, &mode, GENERIC, [None; 3], None, &mut libc),
            [214, 213, 229]
        );
        assert_eq!(libc.state(), 0xd4ad_32fb);
        let matching: Vec<u32> = (120_540..=120_919)
            .filter(|&s| {
                let mut libc = Libc::seeded(s);
                choose(&b, &mode, GENERIC, [None; 3], None, &mut libc) == [214, 213, 229]
                    && libc.state() == 0xd4ad_32fb
            })
            .collect();
        assert_eq!(matching, [120_569]);
        // Save state 3 (item 7's story capture, 2026-10-21): the career's
        // crew list (game data +0x15ee) holds 245, 204 and 227; the second
        // NYC Rec Center pick-up's opponents were 213, 208 and 224, the
        // shared generator reseeded from the timer's 113,415 (the capture
        // `street-played-legend-1` walks back to it), between the reads
        // around the map's press (113,367 and 113,430).
        let crew3 = [245, 204, 227];
        let again = Career {
            event: 0,
            crew: &crew3,
            created: &[],
            unlocked: &unlocked,
            cleared: &|_| false,
            round: 0,
        };
        let mut libc = Libc::seeded(113_415);
        assert_eq!(
            choose(
                &b,
                &Mode::Legend(again),
                GENERIC,
                [None; 3],
                None,
                &mut libc
            ),
            [213, 208, 224]
        );
        let tournament = Career {
            event: 2,
            crew: &crew,
            created: &[],
            unlocked: &unlocked,
            cleared: &|ev| ev == 0,
            round: 0,
        };
        let got = choose(
            &b,
            &Mode::Legend(tournament),
            GENERIC,
            [None; 3],
            Some(666),
            &mut libc,
        );
        assert_eq!(got, [133, 94, 190]);
    }

    #[test]
    fn the_career_may_not_meet_a_boss_teams_player_before_beating_it() {
        let mut b = book(240);
        b.rosters[40] = [Some(0x91), Some(0xb8), None, None, None];
        b.rosters[0x29] = [Some(0x92), Some(0xc0), Some(0xc1), None, None];
        b.cards[0xc0].team = GENERIC as i16;
        b.cards[0xc2].team = GENERIC as i16;
        b.cards[0x97].team = 29;
        let crew = [7];
        let c = career(0, &crew, 0);
        assert!(!c.available(&b, 7));
        assert!(!c.available(&b, 0x91));
        assert!(!c.available(&b, 0xc0));
        assert!(c.available(&b, 0xc2));
        assert!(!c.available(&b, 0x97));
        let beaten = Career {
            cleared: &|ev| ev == 2,
            ..career(0, &crew, 0)
        };
        assert!(beaten.available(&b, 0x91));
    }
}
