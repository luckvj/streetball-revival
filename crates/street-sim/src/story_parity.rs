//! Mode tables and their consumers held against the shipped program
//! (STREET_ISO=/path/to/the disc). Every expected value is read from
//! `SLUS_206.51`; without the disc the tests return early.
use super::*;
use street_assets::scripts::{executable_constant, executable_words};

fn executable() -> Option<Vec<u8>> {
    let path = std::env::var_os("STREET_ISO")?;
    Some(
        street_assets::iso::Iso::open(path)
            .unwrap()
            .read("/SLUS_206.51")
            .unwrap(),
    )
}

fn words(elf: &[u8], address: u32, count: usize) -> Vec<u32> {
    executable_words(elf, address, count).unwrap()
}

/// Little-endian i16s from a word run.
fn halves(elf: &[u8], address: u32, count: usize) -> Vec<i16> {
    words(elf, address, count.div_ceil(2))
        .into_iter()
        .flat_map(|w| [w as i16, (w >> 16) as i16])
        .take(count)
        .collect()
}

fn outcome(won: bool) -> MatchOutcome {
    MatchOutcome {
        won,
        trick_points: 0,
        level1_gamebreakers: 0,
        level2_gamebreakers: 0,
        difficulty: 0,
    }
}

#[test]
fn event_rows_are_the_0x460790_words() {
    let Some(elf) = executable() else {
        return;
    };
    // 17 rows of 0x1c bytes: +4 court, +8 tier, +0xc visible, +0x10 open,
    // +0x14 target.
    for (row, event) in EVENTS.iter().enumerate() {
        let w = words(&elf, 0x460790 + 0x1c * row as u32, 7);
        assert_eq!(w[1], u32::from(event.court), "row {row} court");
        assert_eq!(w[2], u32::from(event.level), "row {row} tier");
        assert_eq!(w[3], event.visible.word(), "row {row} visible");
        assert_eq!(w[4], event.open.word(), "row {row} open");
        assert_eq!(w[5] as i32, i32::from(event.target), "row {row} target");
    }
}

#[test]
fn boss_moves_and_street_cards_are_the_executables() {
    let Some(elf) = executable() else {
        return;
    };
    for (row, event) in EVENTS.iter().enumerate() {
        let w = words(&elf, 0x460790 + 0x1c * row as u32 + 0x18, 1)[0];
        // FUN_001eac00: the move for kinds 1 and 2, else -1.
        let expect = match event.kind {
            EventKind::Pickup => -1,
            _ => w as i16,
        };
        assert_eq!(BOSS_MOVES[row], expect, "row {row}");
    }
    assert_eq!(
        halves(&elf, 0x45e150, 9),
        STREET_CARD_RECORDS.map(|r| r as i16)
    );
}

#[test]
fn clearing_a_street_challenge_grants_its_jersey_and_a_tournament_its_boss_card() {
    let mut s = Story::default();
    let mut notes = Vec::new();
    s.clear_event(11, &mut notes);
    assert_eq!(notes[0], Notice::BossMove(42));
    assert!(notes.contains(&Notice::Jersey(0x41)));
    assert!(s.owned.contains(&0x41));
    // Cleared again: nothing new.
    let mut again = Vec::new();
    s.clear_event(11, &mut again);
    assert!(
        !again
            .iter()
            .any(|n| matches!(n, Notice::BossMove(_) | Notice::Jersey(_)))
    );
    // Stretch's tournament: his move, then him and his card (3).
    let mut notes = Vec::new();
    s.clear_event(2, &mut notes);
    assert_eq!(notes[0], Notice::BossMove(45));
    assert!(notes.contains(&Notice::Player(145)));
    assert!(s.owned.contains(&3));
    // A save from before the cards: they come back on loading it.
    let mut old = s.clone();
    old.owned.clear();
    let back = Story::read(&old.write()).unwrap();
    assert!(back.owned.contains(&0x41) && back.owned.contains(&3));
}

#[test]
fn street_challenge_rules_are_0x460970_by_the_rows_byte() {
    let Some(elf) = executable() else {
        return;
    };
    // +0x18 (a byte), +0x1a the boss, +0x1b the row: a tournament's format
    // (`0x460a10`), a street challenge's rules.
    for (row, event) in EVENTS.iter().enumerate() {
        let w = words(&elf, 0x460790 + 0x1c * row as u32, 7);
        assert_eq!((w[6] >> 24) as u8, event.row, "row {row} +0x1b");
    }
    for (k, rules) in STREET_RULES.iter().enumerate() {
        let w = words(&elf, 0x460970 + 16 * k as u32, 4);
        assert_eq!(
            [rules.game_type, rules.clock, rules.spot, rules.value],
            [w[0], w[1], w[2], w[3]],
            "rules row {k}"
        );
    }
}

#[test]
fn league_regions_and_reward_points_are_the_executable_values() {
    let Some(elf) = executable() else {
        return;
    };
    // 0x3ec9e8: five {u32 count, u8 list pointer, u16 legends team, u16
    // court} rows.
    for region in 0..5 {
        let row = words(&elf, 0x3ec9e8 + 12 * region as u32, 3);
        let list = executable_words(&elf, row[1], 2).unwrap();
        let teams = list
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .take(row[0] as usize)
            .map(u16::from)
            .collect::<Vec<_>>();
        assert_eq!(REGION_MATCHES[region], teams, "region {region}");
        assert_eq!(
            REGION_REWARDS[region],
            (row[2] as u16, (row[2] >> 16) as u16)
        );
    }
    // FUN_001ded40's per-win payouts by difficulty 0/1/2 (league at
    // 0x1df2c4/0x1df2b0/0x1df298, Pick Up at 0x1df570/0x1df55c/0x1df544)
    // and FUN_001deac8's completion (0x1deb30/0x1deb20/0x1deb0c).
    let c = |a| executable_constant(&elf, a).unwrap();
    let league = [c(0x1df2c4), c(0x1df2b0), c(0x1df298)];
    let pickup = [c(0x1df570), c(0x1df55c), c(0x1df544)];
    let done = [c(0x1deb30), c(0x1deb20), c(0x1deb0c)];
    assert_eq!(
        (LEAGUE_RP, PICKUP_RP, LEAGUE_DONE_RP),
        (league, pickup, done)
    );
    for difficulty in 0..3u8 {
        let mut s = Story::default();
        assert_eq!(s.win_pickup(difficulty), pickup[usize::from(difficulty)]);
        let before = s.reward_points;
        s.play_league(
            0,
            0,
            &MatchOutcome {
                difficulty,
                ..outcome(true)
            },
        );
        assert_eq!(s.reward_points - before, league[usize::from(difficulty)]);
    }
}

#[test]
fn reputation_follows_fun_001e70e0_and_its_tables() {
    let Some(elf) = executable() else {
        return;
    };
    // [event tier][reputation level], six signed u16s a row.
    let win = halves(&elf, 0x460b28, 30);
    let loss = halves(&elf, 0x460b70, 30);
    for tier in 0..5 {
        assert_eq!(REP_WIN[tier].to_vec(), win[tier * 6..tier * 6 + 6]);
        assert_eq!(REP_LOSS[tier].to_vec(), loss[tier * 6..tier * 6 + 6]);
    }
    // The level divisor (FUN_001d83c8, 0x1d8410) and the per-level overall
    // points at 0x460a78.
    let size = executable_constant(&elf, 0x1d8410).unwrap() as i16;
    assert_eq!(REP_LEVEL_SIZE, size);
    let pps = halves(&elf, 0x460a78, 6);
    assert_eq!(pps[0], 0);
    assert!(pps[1..].iter().all(|&p| p as u32 == REP_LEVEL_PPS));
    for tier in 0..5u8 {
        let ev = EVENTS.iter().position(|e| e.level == tier).unwrap();
        for won in [false, true] {
            for start in 0..=size * 5 {
                let mut s = Story {
                    reputation: start,
                    ..Story::default()
                };
                s.nickname = true;
                s.play(ev, &outcome(won));
                // FUN_001e70e0: below level 5, the base keeps a loss from
                // demoting and a win stops at the next level's first point.
                let level = start / size;
                let expected = if level < 5 {
                    let table = if won { &win } else { &loss };
                    let delta = table[usize::from(tier) * 6 + level as usize];
                    let sum = start - level * size + delta;
                    if sum < 0 {
                        level * size
                    } else if sum < 12 {
                        start + delta
                    } else {
                        level * size + size
                    }
                } else {
                    start
                };
                assert_eq!(s.reputation, expected, "tier {tier} won {won} from {start}");
            }
        }
    }
    // The nickname when the new level is 3 and none is set (FUN_001e70e0's
    // `+0x734 < 0` branch).
    let mut s = Story {
        reputation: 3 * size - 1,
        ..Story::default()
    };
    let notes = s.play(
        EVENTS.iter().position(|e| e.level == 4).unwrap(),
        &outcome(true),
    );
    assert!(s.nickname && notes.contains(&Notice::Nickname));
}

#[test]
fn shoes_are_priced_by_menu_row_at_0x4604c8() {
    let Some(elf) = executable() else {
        return;
    };
    // Entries 12 and 13 of the create tables at 0x4602a0: {count, pointer}.
    assert_eq!(words(&elf, 0x460300, 4), [20, 0x4604c8, 20, 0x460518]);
    for base in [0x4604c8, 0x460518] {
        let rows = words(&elf, base, 20);
        for (row, &(price, id)) in crate::cab::SHOES.iter().enumerate() {
            assert_eq!(rows[row] & 0xffff, price, "row {row}");
            assert_eq!(rows[row] >> 16, u32::from(id), "row {row}");
        }
    }
    // FUN_001e4e30 prices the row FUN_001e2020 finds for the id: shoe 2 is
    // row 10 at 75, nothing once owned (FUN_001e7758).
    assert_eq!(crate::cab::shoe_row(2), 10);
    assert_eq!(crate::cab::shoe_price(2), 75);
    let mut owned = crate::cab::Wardrobe::default();
    assert_eq!(crate::cab::shoe_cost(2, &owned), 75);
    owned.buy_shoes(10);
    assert_eq!(crate::cab::shoe_cost(2, &owned), 0);
    // The point-buy ladder, entries 14 and 15 at 0x460568 / 0x460578.
    assert_eq!(words(&elf, 0x460310, 4), [7, 0x460568, 7, 0x460578]);
    let ladder = halves(&elf, 0x460568, 7);
    assert_eq!(ladder[0], 0);
    assert_eq!(
        ladder[1..].iter().map(|&p| p as u32).collect::<Vec<_>>(),
        crate::cab::SKILL_STEP_PRICES
    );
}

fn breakdown(notes: &[Notice]) -> DevPoints {
    notes
        .iter()
        .find_map(|n| match n {
            Notice::DevBreakdown(d) => Some(*d),
            _ => None,
        })
        .unwrap_or_default()
}

fn first_of(kind: EventKind) -> usize {
    EVENTS.iter().position(|e| e.kind == kind).unwrap()
}

#[test]
fn development_charts_follow_fun_001e9b90() {
    let Some(elf) = executable() else {
        return;
    };
    let c = |a| executable_constant(&elf, a).unwrap();
    // 0x460a88: fourteen {u32 threshold, u16 points}, the first reached
    // pays (0x1ea294..0x1ea2dc, stopping once line 0 is set).
    let rows = words(&elf, 0x460a88, 28);
    let tricks: Vec<(u32, u32)> = (0..14)
        .map(|k| (rows[2 * k], rows[2 * k + 1] & 0xffff))
        .collect();
    assert_eq!(TRICK_DP.to_vec(), tricks);
    // {u32 cap, u16 step} by difficulty at 0x460af8 (Level 1) and 0x460b10
    // (Level 2), summed a step a gamebreaker until the cap (0x1ea30c..).
    let gb = |base: u32| -> Vec<(u32, u32)> {
        let w = words(&elf, base, 6);
        (0..3).map(|d| (w[2 * d], w[2 * d + 1] & 0xffff)).collect()
    };
    assert_eq!(GB1_DP.to_vec(), gb(0x460af8));
    assert_eq!(GB2_DP.to_vec(), gb(0x460b10));
    // FUN_001ea900's win line: 100, or 150/200 by difficulty (0x1ea9c0,
    // 0x1eaa00, 0x1ea9f4); the court line's 100 (0x1ea104, 0x1ea1a4,
    // 0x1ea264); the total goes to the DP unscaled (0x1eaa18..0x1eaa48).
    assert_eq!(WIN_DP, [c(0x1ea9c0), c(0x1eaa00), c(0x1ea9f4)]);
    for a in [0x1ea104, 0x1ea1a4, 0x1ea264] {
        assert_eq!(c(a), COURT_DP);
    }
    // The raw gamebreaker walk: add a step, stop at the cap, count games.
    let walk = |(cap, step): (u32, u32), n: u8| {
        if n == 0 {
            return 0;
        }
        let (mut sum, mut k) = (0u32, 0u8);
        loop {
            sum = (sum + step) & 0xffff;
            if sum >= cap {
                break;
            }
            k += 1;
            if k >= n {
                break;
            }
        }
        sum
    };
    let pickup = first_of(EventKind::Pickup);
    for difficulty in 0..3u8 {
        for &(points, l1, l2, won) in &[
            (0u32, 0u8, 0u8, false),
            (1, 1, 0, true),
            (250_000, 3, 1, false),
            (1_000_000, 0, 9, true),
            (3_000_001, 12, 12, true),
        ] {
            let mut s = Story::default();
            let before = s.star_points();
            let notes = s.play(
                pickup,
                &MatchOutcome {
                    won,
                    trick_points: points,
                    level1_gamebreakers: l1,
                    level2_gamebreakers: l2,
                    difficulty,
                },
            );
            let d = usize::from(difficulty);
            let lines = breakdown(&notes);
            let expected_tricks = tricks
                .iter()
                .find(|&&(t, _)| points >= t)
                .map_or(0, |&(_, dp)| dp);
            assert_eq!(lines.tricks, expected_tricks);
            assert_eq!(lines.level1, walk(GB1_DP[d], l1));
            assert_eq!(lines.level2, walk(GB2_DP[d], l2));
            assert_eq!(lines.win_loss, if won { WIN_DP[d] } else { 0 });
            // A loss is not halved (+0xcf0 is stored, never applied).
            assert_eq!(s.star_points() - before, lines.total());
        }
    }
}

#[test]
fn development_points_come_only_from_the_story() {
    let Some(elf) = executable() else {
        return;
    };
    // Every `jal FUN_001e9b90` in the program: the two in FUN_001ded40's
    // human-side loops, each behind the mode-5 test (0x1dee40, 0x1df7ec).
    let start = 0x100000u32;
    let code = words(&elf, start, 0x2c0000 / 4);
    let jal = 0x0c00_0000 | 0x1e9b90 >> 2;
    let sites: Vec<u32> = code
        .iter()
        .enumerate()
        .filter(|&(_, &w)| w == jal)
        .map(|(k, _)| start + 4 * k as u32)
        .collect();
    assert_eq!(sites, [0x1deec0, 0x1df85c]);
    assert_eq!(executable_constant(&elf, 0x1dee40).unwrap(), 5);
    assert_eq!(executable_constant(&elf, 0x1df7ec).unwrap(), 5);
    // Pick Up and the League Challenge pay Reward Points and no development
    // points; a story game pays its charts.
    let mut s = Story::default();
    s.win_pickup(2);
    s.play_league(0, 0, &outcome(true));
    s.play_league(0, 1, &outcome(false));
    assert_eq!(s.star_points(), 0);
    s.play(first_of(EventKind::Pickup), &outcome(true));
    assert!(s.star_points() > 0);
}

#[test]
fn calendar_counts_follow_fun_001e9b90() {
    let Some(elf) = executable() else {
        return;
    };
    let c = |a| executable_constant(&elf, a).unwrap();
    // FUN_001e70a8(blk, 0, n): a tournament's end (0x1e9f8c eliminated,
    // 0x1ea114 champion) 6, a pick-up's game (0x1ea1b4) and a street
    // challenge's (0x1ea274) 2; wins (0x1ea3fc) and losses (0x1ea478) 1.
    let (pickup_games, challenge_games) = (c(0x1ea1b4), c(0x1ea274));
    let (out_games, title_games) = (c(0x1e9f8c), c(0x1ea114));
    assert_eq!((c(0x1ea3fc), c(0x1ea478)), (1, 1));
    // "Week: %d" is games / 6 + 1 (0x2cf984, 0x2cf9b0).
    assert_eq!(c(0x2cf984), 6);
    assert_eq!(words(&elf, 0x2cf9b0, 1)[0] & 0xffff, 1);
    let mut s = Story::default();
    let pickup = first_of(EventKind::Pickup);
    let challenge = first_of(EventKind::StreetChallenge);
    let tournament = first_of(EventKind::Tournament);
    s.play(pickup, &outcome(false));
    assert_eq!((s.games, s.wins, s.losses), (pickup_games, 0, 1));
    s.play(challenge, &outcome(true));
    assert_eq!(s.games, pickup_games + challenge_games);
    assert_eq!(s.week(), s.games / 6 + 1);
    // A tournament counts nothing until it ends, then 6.
    let start = s.games;
    let mut rounds = 0;
    loop {
        let cleared = s.cleared(tournament);
        s.play(tournament, &outcome(true));
        rounds += 1;
        if s.cleared(tournament) && !cleared {
            break;
        }
        assert_eq!(s.games, start, "after {rounds} round(s)");
        assert!(rounds < 8);
    }
    assert_eq!(s.games, start + title_games);
    // Knocked out of a fresh one: 6 as well.
    let mut s = Story::default();
    let mut rounds = 0;
    while s.games == 0 {
        let notes = s.play(tournament, &outcome(false));
        rounds += 1;
        assert_eq!(s.games != 0, notes.contains(&Notice::Eliminated));
        assert!(rounds < 4);
    }
    assert_eq!(s.games, out_games);
    assert_eq!(s.week(), out_games / 6 + 1);
}

#[test]
fn update_team_follows_fun_0029d5a8_and_fun_001e7db8() {
    let Some(elf) = executable() else {
        return;
    };
    let c = |a| executable_constant(&elf, a).unwrap();
    // FUN_0029d5a8, case 5: the beaten three are looked up in the team
    // (game+0x1bb0); one missing on a won game (story+0xcf4, 0x29d72c)
    // that is not a tournament goes to Update Team, screen 0x60
    // (0x29d758). Tournaments offer it from the bracket screen when the
    // final's winner is the player's team 0x25 (`FUN_00266e70`).
    assert_eq!(c(0x29d758), 0x60);
    assert_eq!(words(&elf, 0x29d72c, 1)[0] & 0xffff, 0xcf4);
    let pickup = first_of(EventKind::Pickup);
    let tournament = first_of(EventKind::Tournament);
    let mut s = Story {
        crew: vec![200, 10, 11, 12, 13],
        ..Story::default()
    };
    assert!(!s.offers_update(pickup, false, &[20, 21, 22]));
    assert!(!s.offers_update(pickup, true, &[10, 11, 12]));
    assert!(s.offers_update(pickup, true, &[10, 21, 12]));
    assert!(!s.offers_update(tournament, true, &[20, 21, 22]));
    // FUN_001e7db8: a new player into team slots 1..4 (`param_2 - 1 < 4`,
    // 0x1e7e18); the star at 0 and a player already on the team never
    // change.
    assert_eq!(words(&elf, 0x1e7e18, 1)[0] & 0xffff, TEAM_SIZE as u32 - 1);
    assert!(!s.update_team(0, 30));
    assert!(!s.update_team(5, 30));
    assert!(!s.update_team(2, 13));
    assert!(s.update_team(4, 30));
    assert_eq!(s.crew, vec![200, 10, 11, 12, 30]);
}
