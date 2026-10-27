//! High Scores and user names, as the game's screens show them
//! (`score.txt`, `entername.txt`; the data is `street_sim::records`).
//!
//! - The High Scores page lists the twelve tables by `score.txt`'s names;
//!   each table shows its rows with the help line `score.txt` gives it.
//! - Choose Sides goes on to a name page: each person picks a saved name
//!   or enters a new one ("Choose sides and enter a name to track
//!   records."), or plays as a guest.
//! - At the buzzer [`record_match`] counts the game: each named person's
//!   win or loss and career totals (by the side they played), then, as
//!   `FUN_001d4438` does for a named user, the user's record offered to Win
//!   Percentage and the side's game totals offered to the single-game
//!   tables under the user's name. Demo games and Street School lessons
//!   count for nothing; a Be A Legend game has no named users (its finish
//!   goes to Street Legends instead).

use street_sim::{
    World,
    records::{CAREER, GameRecord, NameError, Row},
    story::Story,
};

use crate::{game::Stats, setup::MatchSetup};

/// The name page's rows: each seated controller with its side and the
/// name it plays as (a guest when none) and that name's record; then
/// Continue.
pub fn user_rows(story: &Story, setup: &MatchSetup) -> Vec<String> {
    let mut out: Vec<String> = setup
        .seats
        .iter()
        .enumerate()
        .map(|(k, &(_, side))| {
            let side = if side == 0 { "Home" } else { "Away" };
            let name = setup.users.get(k).cloned().flatten();
            let record = name
                .as_deref()
                .and_then(|n| story.records.user(n))
                .map(|i| {
                    let u = &story.records.users[i];
                    format!("  {}W-{}L", u.wins, u.losses)
                })
                .unwrap_or_default();
            format!(
                "{}P {side}  < {} >{record}",
                k + 1,
                name.as_deref().unwrap_or("Guest")
            )
        })
        .collect();
    out.push("Continue".into());
    out
}

/// Left or right on seat `k`'s row: the next saved name (or Guest).
pub fn cycle_user(story: &Story, setup: &mut MatchSetup, k: usize, step: i32) {
    if setup.users.len() < setup.seats.len() {
        setup.users.resize(setup.seats.len(), None);
    }
    let Some(now) = setup.users.get(k) else {
        return;
    };
    // Guest, then the names; names other seats hold are skipped.
    let taken: Vec<String> = setup
        .users
        .iter()
        .enumerate()
        .filter(|&(j, u)| j != k && u.is_some())
        .filter_map(|(_, u)| u.clone())
        .collect();
    let mut choices: Vec<Option<String>> = vec![None];
    choices.extend(
        story
            .records
            .users
            .iter()
            .map(|u| u.name.clone())
            .filter(|n| !taken.contains(n))
            .map(Some),
    );
    let at = choices.iter().position(|c| c == now).unwrap_or(0) as i32;
    let next = (at + step).rem_euclid(choices.len() as i32) as usize;
    setup.users[k] = choices[next].clone();
}

/// A new name for seat `k`: added to the users and given to the seat.
pub fn name_seat(
    story: &mut Story,
    setup: &mut MatchSetup,
    k: usize,
    name: &str,
) -> Result<(), &'static str> {
    match story.records.add_user(name) {
        Ok(i) => {
            if setup.users.len() < setup.seats.len() {
                setup.users.resize(setup.seats.len(), None);
            }
            if let Some(u) = setup.users.get_mut(k) {
                *u = Some(story.records.users[i].name.clone());
            }
            Ok(())
        }
        // `entername.txt` 23, 32, 38.
        Err(NameError::Empty) => Err("No name entered."),
        Err(NameError::Exists) => Err("The name already exists."),
        Err(NameError::Full) => Err("Maximum number of names has been reached."),
    }
}

/// What a finished game counts toward: users' records and the tables.
/// `names` and `teams` are the match's player and team names.
pub fn record_match(
    story: &mut Story,
    world: &World,
    stats: &Stats,
    setup: &MatchSetup,
    winner: usize,
) {
    if setup.demo {
        return;
    }
    let side_of = |i: usize| world.players.get(i).map_or(0, |p| p.team);
    // Users: each named seat's side.
    for (k, &(_, side)) in setup.seats.iter().enumerate() {
        let Some(name) = setup.users.get(k).cloned().flatten() else {
            continue;
        };
        let Some(i) = story.records.user(&name) else {
            continue;
        };
        let sum = |v: &[u32]| -> u32 {
            v.iter()
                .enumerate()
                .filter(|&(p, _)| side_of(p) == side)
                .map(|(_, n)| n)
                .sum()
        };
        let career: [u32; CAREER.len()] = [
            world.score[side],
            world.teams[side].trick_points,
            sum(&stats.dunks),
            sum(&stats.steals),
            sum(&stats.blocks),
            sum(&stats.gb1) + sum(&stats.gb2),
            // A game won with the rim torn off (`0x400054` bit 3, set by
            // `FUN_001f6d98`): the winners' counter 7.
            u32::from(winner == side && world.rims.iter().any(|r| r.torn)),
        ];
        story.records.played(i, winner == side, career);
        // `FUN_001d4438`: the user's record to Win Percentage (table 1,
        // `FUN_001ee328` 0 and 1), then the side's totals for the game
        // (`FUN_001871e0(.., 1, stat)`: 0xb trick points, 0xc and 0xd the
        // gamebreakers, 2 alley-oops, 6 blocks, 5 steals, 8 long range
        // shots, 1 dunks) under the user's name.
        let user = &story.records.users[i];
        let (user_name, wins, losses) = (user.name.clone(), user.wins, user.losses);
        story.records.offer_win(&user_name, wins, losses);
        let figures = [
            (GameRecord::TrickPoints, world.teams[side].trick_points),
            (GameRecord::Gamebreakers1, sum(&stats.gb1)),
            (GameRecord::Gamebreakers2, sum(&stats.gb2)),
            (GameRecord::AlleyOops, sum(&stats.oops)),
            (GameRecord::Blocks, sum(&stats.blocks)),
            (GameRecord::Steals, sum(&stats.steals)),
            (GameRecord::LongRange, sum(&stats.long)),
            (GameRecord::Dunks, sum(&stats.dunks)),
            // The rewrite's own table, which no screen shows.
            (GameRecord::PassOuts, sum(&stats.passouts)),
        ];
        for (table, value) in figures {
            story.records.offer(
                table,
                Row {
                    name: user_name.clone(),
                    user: user_name.clone(),
                    value,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use street_sim::{Hoop, Player, Rules};

    use super::*;

    fn world() -> World {
        let players = (0..6)
            .map(|k| Player::new(k / 3, [0.0, k as f32], 0.0))
            .collect();
        let hoop = |x: f32| Hoop {
            rim: [x, 11.75, 0.0],
            rim_radius: 0.75,
        };
        World::new([hoop(37.0), hoop(-37.0)], players, Rules::default(), 1)
    }

    #[test]
    fn a_game_counts_for_its_named_players() {
        let mut story = Story::default();
        let mut setup = MatchSetup {
            teams: [0, 1],
            lineups: [[0, 1, 2], [3, 4, 5]],
            court: "RUCKER".into(),
            rules: Rules::default(),
            seed: 1,
            demo: false,
            watch: false,
            seats: vec![(crate::controls::Device::Keyboard, 1)],
            users: Vec::new(),
            online: false,
        };
        name_seat(&mut story, &mut setup, 0, "JAY").unwrap();
        assert_eq!(
            name_seat(&mut story, &mut setup, 0, "jay"),
            Err("The name already exists.")
        );
        let mut w = world();
        w.score = [21, 15];
        let mut stats = Stats::new(6);
        stats.dunks[4] = 3;
        stats.oops[4] = 2;
        record_match(&mut story, &w, &stats, &setup, 0);
        let u = &story.records.users[0];
        assert_eq!((u.wins, u.losses, u.career[0], u.career[2]), (0, 1, 15, 3));
        // The side's totals go in under the user's name (`FUN_001d4438`);
        // one game is too few for Win Percentage.
        let dunks = &story.records.games[GameRecord::Dunks.index()];
        assert_eq!(
            (
                dunks[0].name.as_str(),
                dunks[0].user.as_str(),
                dunks[0].value
            ),
            ("JAY", "JAY", 3)
        );
        assert!(story.records.win_rows.is_empty());
        // A demo game counts for nothing.
        setup.demo = true;
        record_match(&mut story, &w, &stats, &setup, 0);
        assert_eq!(story.records.users[0].losses, 1);
        // A game won with the rim torn off counts a busted rim for the
        // winners only (`0x1df244..0x1df264`), never shown.
        setup.demo = false;
        w.rims[0].tear();
        record_match(&mut story, &w, &stats, &setup, 0);
        assert_eq!(
            story.records.users[0].career[street_sim::records::BUSTED_RIMS],
            0
        );
        record_match(&mut story, &w, &stats, &setup, 1);
        assert_eq!(
            story.records.users[0].career[street_sim::records::BUSTED_RIMS],
            1
        );
    }

    #[test]
    fn seats_cycle_through_free_names() {
        let mut story = Story::default();
        story.records.add_user("A").unwrap();
        story.records.add_user("B").unwrap();
        let mut setup = MatchSetup {
            teams: [0, 1],
            lineups: [[0, 1, 2], [3, 4, 5]],
            court: String::new(),
            rules: Rules::default(),
            seed: 1,
            demo: false,
            watch: false,
            seats: vec![
                (crate::controls::Device::Keyboard, 0),
                (crate::controls::Device::Keyboard, 1),
            ],
            users: vec![Some("A".into()), None],
            online: false,
        };
        cycle_user(&story, &mut setup, 1, 1);
        // "A" is taken by 1P: 2P goes from Guest to B.
        assert_eq!(setup.users[1].as_deref(), Some("B"));
        cycle_user(&story, &mut setup, 1, 1);
        assert_eq!(setup.users[1], None);
    }
}
