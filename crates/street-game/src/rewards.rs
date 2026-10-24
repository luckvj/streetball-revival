//! The Rewards shop's cards (`XDBCARD.ADF`): what each is called, what it
//! unlocks and its picture.
//!
//! A card's category (field 1) is one of seven, each named in
//! `poster.txt`: 0 Street Legends (9 cards), 1 Legends (26), 2 Courts
//! (11), 3 Jerseys (25), 4 Development Points (5), 5 Tips (20) and 6 Codes
//! (17). Field 4 is the card's picture: an image of the third sheet of
//! `/FE/XAFESCRN.BIN`, by position (104 images; the last, Nelly and the
//! St. Lunatics, repeats the name `0061` of Just Blaze's): 0x01..0x05,
//! 0x60..0x62 and 103 the street legends, 0x06..0x1e and 0x63 the league
//! legends, 0x1f..0x29 the courts, 0x2a..0x42 the jerseys, 0x43..0x47 the
//! development points, 0x48..0x53 the street tips, 0x54..0x5f the cheat
//! codes, 0x65 the face-down card. Picture 0 (a "Temp" card) means none:
//! the eight tips past the twelfth show face down.
//!
//! The names come from `poster.txt`, in the cards' order: Street Legends
//! 52..60, Legends 61..86, Courts 41..51, Jerseys 94..118, Development
//! Points 119..123. Tips and codes are "Street Tip #n" and "Cheat Code
//! #n", as their pictures read.

use street_assets::db::Card;

/// The categories' names in `poster.txt`, by category.
const CATEGORY_TEXTS: [usize; 7] = [10, 9, 11, 12, 13, 14, 15];
/// The same in English, for a disc without the string.
const CATEGORY_NAMES: [&str; 7] = [
    "Street Legends",
    "Legends",
    "Courts",
    "Jerseys",
    "Development Points",
    "Tips",
    "Codes",
];

pub const STREET_LEGENDS: i16 = 0;
pub const LEAGUE_LEGENDS: i16 = 1;
pub const COURTS: i16 = 2;
pub const JERSEYS: i16 = 3;
pub const DEVELOPMENT: i16 = 4;
pub const TIPS: i16 = 5;
pub const CODES: i16 = 6;

/// The face-down card in the picture sheet (generic for non-legends).
pub const FACE_DOWN: usize = 0x65;
/// The legend face-down card in the picture sheet (silhouette for Street Legends and Legends,
/// as recovered from native `FUN_002c2a90` and table `0x0048f678`).
pub const LEGEND_FACE_DOWN: usize = 0x66;

/// The player records each Street Legends card unlocks (Nelly's card is
/// the five St. Lunatics).
const STREET_RECORDS: [&[u16]; 9] = [
    &[151],
    &[152],
    &[153, 154, 155, 156, 157],
    &[145],
    &[147],
    &[149],
    &[148],
    &[150],
    &[146],
];

/// The player record each Legends card unlocks: `FUN_001d2818`'s
/// category-1 table at `0x45e168` (the database's legends, 158..183; the
/// free card 15, index 6, is the Jordan of record 168, the fourth trophy's
/// card 34, index 25, the other, 167).
pub const LEAGUE_RECORDS: [u16; 26] = [
    165, 164, 162, 163, 161, 166, 168, 173, 177, 179, 183, 176, 182, 180, 174, 171, 175, 170, 160,
    159, 181, 169, 178, 172, 158, 167,
];

/// Once the disc is in: every owned (or free) legend card's players are
/// unlocked, for saves made while the rewrite read the two Jordans the
/// wrong way round (only ever adds).
pub fn repair_card_unlocks(
    disc: Option<bevy::prelude::Res<crate::setup::Disc>>,
    mut save: bevy::prelude::ResMut<crate::story::Save>,
    mut done: bevy::prelude::Local<bool>,
) {
    let Some(disc) = disc.filter(|_| !*done) else {
        return;
    };
    *done = true;
    let story = &mut save.story;
    let mut records = Vec::new();
    for card in &disc.cards {
        if matches!(card.category, STREET_LEGENDS | LEAGUE_LEGENDS)
            && story.owned.contains(&(card.id as u16))
        {
            records.extend_from_slice(card_records(card));
        }
    }
    for r in records {
        if !story.unlocked_players.contains(&r) {
            story.unlock_player(r);
        }
    }
}

/// FUN_001d2818's category-2 mapping at 0x45e1a0.
const COURT_RECORDS: [u16; 11] = [0, 1, 6, 3, 4, 5, 2, 7, 8, 9, 10];

/// A regional boss win grants its court card and every roster player's
/// legend card. Native FUN_001de8d0 consumes the team notice this way;
/// the court grant is in FUN_001de528's kind-2 branch. No RP is spent.
pub fn grant_league_region(
    story: &mut street_sim::story::Story,
    disc: &crate::setup::Disc,
    region: usize,
) {
    let Some(&(team, court)) = street_sim::story::REGION_REWARDS.get(region) else {
        return;
    };
    let Some(roster) = disc.teams.get(usize::from(team)).map(|team| &team.roster) else {
        return;
    };
    for card in &disc.cards {
        let grants = match card.category {
            LEAGUE_LEGENDS => card_records(card)
                .iter()
                .any(|&r| roster.contains(&usize::from(r))),
            COURTS => usize::try_from(card.index)
                .ok()
                .and_then(|index| COURT_RECORDS.get(index))
                .is_some_and(|&id| id == court),
            _ => false,
        };
        if !grants || card.id < 0 {
            continue;
        }
        if !card_owned(story, card) {
            story.owned.push(card.id as u16);
            if let Some(court) = court_of(card) {
                story.court_granted(court);
            }
        }
        for &record in card_records(card) {
            story.unlock_player(record);
        }
    }
}

/// A category's name, from the disc's `poster.txt` (`texts`).
pub fn category_name(texts: &[String], category: usize) -> String {
    CATEGORY_TEXTS
        .get(category)
        .and_then(|&i| texts.get(i))
        .cloned()
        .or_else(|| CATEGORY_NAMES.get(category).map(|s| (*s).to_owned()))
        .unwrap_or_default()
}

/// A card's name.
pub fn card_name(texts: &[String], card: &Card) -> String {
    let i = usize::try_from(card.index).unwrap_or(0);
    let text = match card.category {
        STREET_LEGENDS => Some(52 + i),
        LEAGUE_LEGENDS => Some(61 + i),
        COURTS => Some(41 + i),
        JERSEYS => Some(94 + i),
        DEVELOPMENT => [100, 200, 300, 500, 1000]
            .iter()
            .position(|&n| i32::from(card.index) == n)
            .map(|k| 119 + k),
        TIPS => return format!("Street Tip #{}", i + 1),
        CODES => return format!("Cheat Code #{}", i + 1),
        _ => None,
    };
    text.and_then(|t| texts.get(t))
        .cloned()
        .unwrap_or_else(|| format!("Reward #{}", card.id))
}

/// The milestones a finished game reaches (`milestone.txt`): a million
/// trick points (any mode), a shutout, 21 alley-oops, a win without being
/// blocked, 20 blocks or 20 steals in a win, and a game in each My Rules
/// mode. Only a side people played counts; not a lesson's practice, a demo
/// or a game over the network. Milestone 5 uses the original seven-bar
/// check: every bar at least five, at least one at six.
pub fn game_milestones(
    story: &mut street_sim::story::Story,
    w: &street_sim::World,
    stats: &crate::game::Stats,
    setup: &crate::setup::MatchSetup,
    winner: usize,
    (lesson, pickup): (bool, bool),
) -> Vec<street_sim::story::Notice> {
    use street_sim::story::Notice;
    let mut out = Vec::new();
    if setup.demo || setup.online || lesson {
        return out;
    }
    let people: Vec<usize> = if setup.seats.is_empty() {
        vec![0]
    } else {
        setup.seats.iter().map(|&(_, side)| side).collect()
    };
    let mut reach = |story: &mut street_sim::story::Story, n: u8| {
        if story.reach_milestone(n) {
            out.push(Notice::Milestone(n));
        }
    };
    if story.ballers.iter().any(crate::ballers::max_skills) {
        reach(story, 4);
    }
    for team in 0..2usize {
        if !people.contains(&team) {
            continue;
        }
        let other = 1 - team;
        let won = winner == team;
        let sum = |v: &[u32], t: usize| -> u32 {
            (0..w.players.len())
                .filter(|&i| w.team_of(i) == t)
                .map(|i| v.get(i).copied().unwrap_or(0))
                .sum()
        };
        if w.teams[team].trick_points >= 1_000_000 {
            reach(story, 0);
        }
        // FUN_001ded40 excludes Trick Point Challenge (1) and
        // Gamebreaker Wins (6), whose ordinary score does not decide a win.
        if won && matches!(w.rules.goal, street_sim::Goal::Points(_)) && w.score[other] == 0 {
            reach(story, 1);
        }
        if won && sum(&stats.oops, team) >= 21 {
            reach(story, 2);
        }
        if won && sum(&stats.blocks, other) == 0 {
            reach(story, 3);
        }
        if won && sum(&stats.blocks, team) >= 20 {
            reach(story, 12);
        }
        if won && sum(&stats.steals, team) >= 20 {
            reach(story, 13);
        }
    }
    // A Pick Up game in every My Rules mode.
    let mode = crate::menu::mode_of(&setup.rules);
    if pickup && let Some(k) = street_sim::Mode::ALL.iter().position(|&m| m == mode) {
        story.modes_played |= 1 << k;
    }
    if story.modes_played == 0xff {
        reach(story, 18);
    }
    out
}

/// A milestone's reward: its throwback jersey card (`0x460050`: card 46 +
/// the milestone), owned from now on; the line the post-game shows
/// (`milestone.txt` 0 and 2).
pub fn grant_milestone(
    story: &mut street_sim::story::Story,
    disc: &crate::setup::Disc,
    n: u8,
) -> String {
    let id = street_sim::story::MILESTONE_CARD + u16::from(n);
    if !story.owned.contains(&id) {
        story.owned.push(id);
    }
    let name = disc
        .cards
        .iter()
        .find(|c| c.id as u16 == id)
        .map(|c| card_name(&disc.poster, c))
        .unwrap_or_default();
    format!(
        "STREET MILESTONE #{}: you've unlocked the throwback jersey of {name}",
        n + 1
    )
}

/// The boss moves a milestone's jersey brings with it (`FUN_001e0910`,
/// 0x1e09c8..0x1e0a94): a jersey card (category 3) whose player is
/// record 173, 159 or 175 queues move 40, 41 or 47 in the first free
/// boss-move slot (kinds 7..9), as a notice only.
pub const MILESTONE_BOSS_MOVES: [(usize, u8); 3] = [(173, 40), (159, 41), (175, 47)];

/// The boss-move notice milestone `n`'s jersey queues, if any.
pub fn milestone_boss_move(disc: &crate::setup::Disc, n: u8) -> Option<street_sim::story::Notice> {
    let id = street_sim::story::MILESTONE_CARD + u16::from(n);
    let card = disc.cards.iter().find(|c| c.id as u16 == id)?;
    if card.category != JERSEYS {
        return None;
    }
    let player = crate::boards::card_player(disc, id)?;
    MILESTONE_BOSS_MOVES
        .iter()
        .find(|&&(p, _)| p == player)
        .map(|&(_, m)| street_sim::story::Notice::BossMove(m))
}

/// The player records a card unlocks.
pub fn card_records(card: &Card) -> &'static [u16] {
    let i = usize::try_from(card.index).unwrap_or(usize::MAX);
    match card.category {
        STREET_LEGENDS => STREET_RECORDS.get(i).copied().unwrap_or(&[]),
        LEAGUE_LEGENDS => LEAGUE_RECORDS
            .get(i)
            .map(std::slice::from_ref)
            .unwrap_or(&[]),
        _ => &[],
    }
}

/// The five unlock arms of `FUN_002453e8` grant cards without charging RP.
pub fn grant_cheat(
    story: &mut street_sim::story::Story,
    disc: &crate::setup::Disc,
    cheat: crate::cheats::Cheat,
) {
    use crate::cheats::Cheat;
    let category = match cheat {
        Cheat::UnlockCourts => Some(COURTS),
        Cheat::UnlockStreetLegends => Some(STREET_LEGENDS),
        Cheat::UnlockLeagueLegends => Some(LEAGUE_LEGENDS),
        Cheat::UnlockJerseys => Some(JERSEYS),
        Cheat::UnlockMichaelJordan => None,
        _ => return,
    };
    for card in &disc.cards {
        let include = match category {
            Some(c) => card.category == c && card.id != 34,
            None => card.id == 34,
        };
        if include {
            if !story.owned.contains(&(card.id as u16)) {
                story.owned.push(card.id as u16);
                if let Some(court) = court_of(card) {
                    story.court_granted(court);
                }
            }
            for &player in card_records(card) {
                story.unlock_player(player);
            }
        }
    }
}

/// The ballers who can receive a development card: saved created
/// players and the stars of existing careers, including premade stars.
pub fn development_recipients(story: &street_sim::story::Story) -> Vec<u16> {
    let mut records: Vec<u16> = story
        .ballers
        .iter()
        .map(|b| b.record() as u16)
        .chain(story.crew.first().copied())
        .chain(
            story
                .careers
                .iter()
                .flatten()
                .filter_map(|c| c.crew.first().copied()),
        )
        .collect();
    records.sort_unstable();
    records.dedup();
    records
}

/// Automatic rewards from eligible Pick Up wins (`FUN_001ded40`,
/// 0x1df6b0..0x1df780). Field 5 is a positive win threshold; already
/// owned or zero-price cards are skipped (`FUN_001d26e8`). No RP is spent.
pub fn pickup_rewards(story: &mut street_sim::story::Story, cards: &[Card]) -> Vec<u16> {
    let mut granted = Vec::new();
    let mut street_notice = false;
    for card in cards {
        if card.id < 0
            || card.param <= 0
            || card.cost <= 0
            || story.pickup_wins < card.param as u32
            || story.owned.contains(&(card.id as u16))
            || (card.category == STREET_LEGENDS && street_notice)
        {
            continue;
        }
        story.owned.push(card.id as u16);
        street_notice |= card.category == STREET_LEGENDS;
        for &record in card_records(card) {
            story.unlock_player(record);
        }
        granted.push(card.id as u16);
    }
    granted
}

/// A development card is repeatable, unlike an owned collectible card.
pub fn buy_development(story: &mut street_sim::story::Story, card: &Card, recipient: u16) -> bool {
    if card.category != DEVELOPMENT
        || card.cost < 0
        || card.index <= 0
        || !development_recipients(story).contains(&recipient)
    {
        return false;
    }
    story.spend_reward_points_for(recipient, card.cost as u32, card.index as u32)
}

/// The court a court card opens (its entry's court id).
pub fn court_of(card: &Card) -> Option<u16> {
    (card.category == COURTS)
        .then(|| usize::try_from(card.index).ok())
        .flatten()
        .and_then(|i| COURT_RECORDS.get(i).copied())
}

/// Be A Legend's event court (`FUN_001ded40` 0x1deedc..0x1deef0, kind 2,
/// and its handler 0x1de65c..0x1de704): once event `ev` is cleared its
/// court's card is granted (`FUN_001e0420`) unless the court is open
/// already (`FUN_001d37b0`); the court's notice when it was.
pub fn grant_event_court(
    story: &mut street_sim::story::Story,
    disc: &crate::setup::Disc,
    ev: usize,
) -> Option<street_sim::story::Notice> {
    let court = street_sim::story::EVENTS.get(ev)?.court;
    if !story.cleared(ev) || court_unlocked(story, disc, usize::from(court)) {
        return None;
    }
    let card = disc.cards.iter().find(|c| court_of(c) == Some(court))?;
    story.owned.push(card.id as u16);
    story.court_granted(court);
    Some(street_sim::story::Notice::Court(court))
}

/// Whether a court is open to play (`FUN_001d37b0`): the court card
/// (category 2) whose entry names it is owned (`FUN_001e0398`, through
/// `card_owned`'s zero-price rule); a court no card names is open. The
/// fresh save opens courts 0, 1 and 6, the three cards that cost nothing.
pub fn court_unlocked(
    story: &street_sim::story::Story,
    disc: &crate::setup::Disc,
    court: usize,
) -> bool {
    disc.cards
        .iter()
        .filter(|c| c.category == COURTS)
        .find(|c| {
            usize::try_from(c.index)
                .ok()
                .and_then(|i| COURT_RECORDS.get(i))
                .is_some_and(|&id| usize::from(id) == court)
        })
        .is_none_or(|card| card_owned(story, card))
}

/// The original category walker (`FUN_001d2b38`, `FUN_002c2a90`)
/// includes rows without a picture. Keep their place in the category;
/// unavailable artwork does not remove a tip or a code from the table.
pub fn in_shop(card: &Card) -> bool {
    card.cost >= 0
}

/// The original ownership getter (`FUN_001d26e8`) treats zero-price cards
/// as available even when their saved ownership bit is unset.
pub fn card_owned(story: &street_sim::story::Story, card: &Card) -> bool {
    card.cost == 0 || story.owned.contains(&(card.id as u16))
}

/// A bought cheat card's name and the actual four-button entry sequence.
/// `FUN_002c4730` draws four icons from `0x48f698 + index * 8`; the
/// matching packed sequences are checked by `FUN_002453e8`. The card's
/// field 5 is not the literal code. `0x48f850` is UTF-32 "%d", used for
/// reward/development points elsewhere on the screen.
pub fn cheat_info(common: &[String], card: &Card) -> String {
    if card.category != CODES {
        return String::new();
    }
    let Some(cheat) = usize::try_from(card.index)
        .ok()
        .and_then(|index| crate::cheats::ALL.get(index).copied())
    else {
        return String::new();
    };
    let name = cheat
        .common_index()
        .and_then(|index| common.get(index))
        .map(String::as_str)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| cheat.name());
    let buttons = cheat
        .button_sequence()
        .map(|button| button.name())
        .join(", ");
    format!("{name}: hold L1, press {buttons}")
}

/// The cards whose picture stays face down until owned: the two ids at
/// `0x45e1f0` (`FUN_001d2758`).
pub const HIDDEN_UNTIL_OWNED: [i16; 2] = [34, 45];

/// The card's picture in the sheet of `pictures` images (`FUN_002c2a90`):
/// its field 4, unless it is one of [`HIDDEN_UNTIL_OWNED`] and not owned,
/// which is face down by the category table `0x48f678` (3 for Street and
/// Legends: `0x66`, the silhouette; 4 for the rest: `0x65`). The
/// original shows field 4 whatever it is; a picture the sheet lacks is
/// face down here, the rewrite's own guard.
pub fn card_picture(card: &Card, owned: bool, pictures: usize) -> usize {
    let face_down = || {
        if card.category == STREET_LEGENDS || card.category == LEAGUE_LEGENDS {
            LEGEND_FACE_DOWN
        } else {
            FACE_DOWN
        }
    };
    if HIDDEN_UNTIL_OWNED.contains(&card.id) && !owned {
        return face_down();
    }
    match usize::try_from(card.poster) {
        Ok(p) if p < pictures => p,
        _ => face_down(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn league_boss_rewards_grant_roster_cards_and_native_mapped_courts() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let disc = crate::setup::Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let mut story = street_sim::story::Story {
            reward_points: 1234,
            ..Default::default()
        };
        for region in 0..5 {
            let (team, court) = street_sim::story::REGION_REWARDS[region];
            let before = story.clone();
            let count = street_sim::story::REGION_MATCHES[region].len();
            for index in 0..count {
                story.play_league(
                    region,
                    index,
                    &street_sim::story::MatchOutcome {
                        won: true,
                        ..Default::default()
                    },
                );
            }
            assert_eq!(
                story.owned, before.owned,
                "ordinary league wins grant no cards"
            );
            story.play_league(
                region,
                count,
                &street_sim::story::MatchOutcome {
                    won: true,
                    ..Default::default()
                },
            );
            let rp = story.reward_points;
            grant_league_region(&mut story, &disc, region);
            assert_eq!(story.reward_points, rp);
            for &player in &disc.teams[usize::from(team)].roster {
                assert!(story.unlocked_players.contains(&(player as u16)));
                let card = disc
                    .cards
                    .iter()
                    .find(|c| {
                        c.category == LEAGUE_LEGENDS && card_records(c).contains(&(player as u16))
                    })
                    .unwrap();
                assert!(card_owned(&story, card));
            }
            let court_card = disc
                .cards
                .iter()
                .find(|c| {
                    c.category == COURTS
                        && usize::try_from(c.index)
                            .ok()
                            .and_then(|i| COURT_RECORDS.get(i))
                            == Some(&court)
                })
                .unwrap();
            assert!(card_owned(&story, court_card));
            let granted = story.clone();
            grant_league_region(&mut story, &disc, region);
            assert_eq!(
                story, granted,
                "repeated consumption grants no duplicate cards"
            );
        }
        assert_eq!(
            street_sim::story::Story::read(&story.write()).unwrap(),
            story
        );
    }

    fn card(category: i16, index: i16, poster: i16) -> Card {
        Card {
            id: 0,
            category,
            index,
            cost: 0,
            poster,
            param: -1,
        }
    }

    #[test]
    fn pickup_thresholds_grant_without_spending_and_survive_reload() {
        use street_sim::story::Story;
        let mut s = Story::default();
        let cards = [
            Card {
                id: 2,
                category: STREET_LEGENDS,
                index: 2,
                cost: 500,
                poster: 103,
                param: 20,
            },
            Card {
                id: 76,
                category: TIPS,
                index: 0,
                cost: 100,
                poster: 0x48,
                param: 15,
            },
            Card {
                id: 96,
                category: CODES,
                index: 0,
                cost: 500,
                poster: 0x54,
                param: 40,
            },
            Card {
                id: 35,
                category: COURTS,
                index: 0,
                cost: 0,
                poster: 0x1f,
                param: 1,
            },
        ];
        for _ in 0..14 {
            s.win_pickup(0);
        }
        assert!(pickup_rewards(&mut s, &cards).is_empty());
        s.win_pickup(0);
        let balance = s.reward_points;
        assert_eq!(pickup_rewards(&mut s, &cards), [76]);
        assert_eq!(s.reward_points, balance);
        let mut s = Story::read(&s.write()).unwrap();
        assert_eq!(s.pickup_wins, 15);
        assert!(pickup_rewards(&mut s, &cards).is_empty());
        for _ in 15..20 {
            s.win_pickup(0);
        }
        assert_eq!(pickup_rewards(&mut s, &cards), [2]);
        assert!((153..=157).all(|r| s.unlocked_players.contains(&r)));
        assert!(!s.owned.contains(&35));
        assert!(!s.owned.contains(&96));
        for _ in 20..40 {
            s.win_pickup(0);
        }
        assert_eq!(pickup_rewards(&mut s, &cards), [96]);
    }

    #[test]
    fn every_disc_pickup_threshold_grants_its_card() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let cards = street_assets::db::cards(&mut iso).unwrap();
        let thresholds: Vec<_> = cards.iter().filter(|c| c.param > 0 && c.cost > 0).collect();
        assert!(!thresholds.is_empty());
        assert!(
            thresholds
                .iter()
                .all(|c| matches!(c.category, STREET_LEGENDS | TIPS | CODES))
        );
        let mut s = street_sim::story::Story::default();
        let last = thresholds.iter().map(|c| c.param).max().unwrap() as u32;
        for wins in 1..=last {
            s.win_pickup(0);
            let rp = s.reward_points;
            let granted = pickup_rewards(&mut s, &cards);
            assert_eq!(s.reward_points, rp);
            for card in &thresholds {
                assert_eq!(
                    s.owned.contains(&(card.id as u16)),
                    wins >= card.param as u32,
                    "card {} at win {wins}",
                    card.id
                );
            }
            assert!(granted.iter().all(|id| {
                cards
                    .iter()
                    .any(|c| c.id as u16 == *id && c.param as u32 == wins)
            }));
            assert!(pickup_rewards(&mut s, &cards).is_empty());
        }
    }

    #[test]
    fn non_score_modes_do_not_grant_a_shutout_from_zero_score() {
        use street_sim::{Hoop, Mode, Rules, World, story::Story};
        for mode in [
            Mode::Normal,
            Mode::TrickPointChallenge,
            Mode::GamebreakerWins,
        ] {
            let rules = Rules::mode(mode);
            let setup = crate::setup::MatchSetup {
                teams: [0, 1],
                lineups: [[0, 1, 2], [3, 4, 5]],
                court: "RUCKER".into(),
                rules,
                seed: 1,
                demo: false,
                watch: false,
                seats: Vec::new(),
                users: Vec::new(),
                online: false,
            };
            let mut world = World::new(
                [
                    Hoop {
                        rim: [40.0, 10.0, 0.0],
                        rim_radius: 0.75,
                    },
                    Hoop {
                        rim: [-40.0, 10.0, 0.0],
                        rim_radius: 0.75,
                    },
                ],
                Vec::new(),
                rules,
                1,
            );
            world.score = [21, 0];
            let mut story = Story::default();
            game_milestones(
                &mut story,
                &world,
                &crate::game::Stats::new(0),
                &setup,
                0,
                (false, true),
            );
            assert_eq!(
                story.milestones & (1 << 1) != 0,
                mode == Mode::Normal,
                "{mode:?}"
            );
        }
    }

    #[test]
    fn names_follow_the_pictures() {
        let mut texts = vec![String::new(); 124];
        texts[54] = "Nelly and the St. Lunatics".into();
        texts[61] = "Bill Walton".into();
        texts[41] = "NYC Rec Center".into();
        texts[121] = "300 Dev Points".into();
        assert_eq!(
            card_name(&texts, &card(STREET_LEGENDS, 2, 103)),
            "Nelly and the St. Lunatics"
        );
        assert_eq!(
            card_name(&texts, &card(LEAGUE_LEGENDS, 0, 14)),
            "Bill Walton"
        );
        assert_eq!(card_name(&texts, &card(COURTS, 0, 31)), "NYC Rec Center");
        assert_eq!(
            card_name(&texts, &card(DEVELOPMENT, 300, 69)),
            "300 Dev Points"
        );
        assert_eq!(card_name(&texts, &card(TIPS, 3, 75)), "Street Tip #4");
        assert_eq!(category_name(&[], 1), "Legends");
    }

    #[test]
    fn the_legend_cards_players_are_the_executables_tables() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let elf = street_assets::iso::Iso::open(path)
            .unwrap()
            .read("/SLUS_206.51")
            .unwrap();
        let halves = |address: u32, n: usize| -> Vec<u16> {
            street_assets::scripts::executable_words(&elf, address, n.div_ceil(2))
                .unwrap()
                .into_iter()
                .flat_map(|w| [w as u16, (w >> 16) as u16])
                .take(n)
                .collect()
        };
        assert_eq!(halves(0x45e168, 26), LEAGUE_RECORDS);
        assert_eq!(halves(0x45e1b8, 25), crate::boards::JERSEY_RECORDS);
        // FUN_001e0910's players (`addiu v0, zero, n` at 0x1e0a1c,
        // 0x1e0a2c, 0x1e0a40) and their moves (`addiu a2, zero, m` in the
        // delay slots at 0x1e0a60, 0x1e0a78, 0x1e0a90).
        let imm = |address: u32| {
            street_assets::scripts::executable_words(&elf, address, 1).unwrap()[0] & 0xffff
        };
        let players = [imm(0x1e0a1c), imm(0x1e0a2c), imm(0x1e0a40)];
        let moves = [imm(0x1e0a60), imm(0x1e0a78), imm(0x1e0a90)];
        for (k, (p, m)) in MILESTONE_BOSS_MOVES.iter().enumerate() {
            assert_eq!((players[k] as usize, moves[k] as u8), (*p, *m));
        }
    }

    #[test]
    fn cards_unlock_their_players() {
        assert_eq!(card_records(&card(STREET_LEGENDS, 2, 103)).len(), 5);
        assert_eq!(card_records(&card(LEAGUE_LEGENDS, 24, 30)), &[158]);
        assert!(card_records(&card(COURTS, 0, 31)).is_empty());
        // Every legend once.
        let mut all: Vec<u16> = LEAGUE_RECORDS.to_vec();
        all.sort_unstable();
        assert_eq!(all, (158..=183).collect::<Vec<u16>>());
    }

    #[test]
    fn missing_pictures_do_not_remove_category_rows() {
        assert!(in_shop(&card(CODES, 3, 87)));
        assert!(in_shop(&card(CODES, 12, 0)));
        assert!(in_shop(&card(TIPS, 12, 0)));
        assert!(card_owned(
            &street_sim::story::Story::default(),
            &card(CODES, 12, 0)
        ));
    }

    #[test]
    fn bought_codes_show_real_buttons_and_ignore_field_five() {
        let mut common = vec![String::new(); 40];
        common[27] = "Street Kids".into();
        common[31] = "Localized Unlimited Turbo".into();
        let mut code = Card {
            id: 96,
            category: CODES,
            index: 0,
            cost: 500,
            poster: 84,
            param: 0x28,
        };
        assert_eq!(
            cheat_info(&common, &code),
            "Street Kids: hold L1, press Triangle, Triangle, Circle, Square"
        );
        code.param = -1;
        assert_eq!(
            cheat_info(&common, &code),
            "Street Kids: hold L1, press Triangle, Triangle, Circle, Square"
        );
        code.index = 4;
        assert_eq!(
            cheat_info(&common, &code),
            "Localized Unlimited Turbo: hold L1, press Square, Square, Triangle, Triangle"
        );
        assert_eq!(
            cheat_info(&[], &code),
            "Unlimited Turbo: hold L1, press Square, Square, Triangle, Triangle"
        );
        code.index = -1;
        assert_eq!(cheat_info(&common, &code), "");
        code.index = i16::MAX;
        assert_eq!(cheat_info(&common, &code), "");
        assert_eq!(cheat_info(&common, &card(TIPS, 3, 75)), "");
    }

    #[test]
    fn missing_pictures_are_face_down() {
        assert_eq!(card_picture(&card(STREET_LEGENDS, 2, 103), false, 104), 103);
        // A picture the sheet lacks: the rewrite's guard.
        assert_eq!(
            card_picture(&card(STREET_LEGENDS, 2, 103), true, 0),
            LEGEND_FACE_DOWN
        );
        // Field 4 is shown whatever it is, owned or not (`FUN_002c2a90`).
        assert_eq!(card_picture(&card(LEAGUE_LEGENDS, 0, 0), false, 104), 0);
        assert_eq!(card_picture(&card(TIPS, 15, 0), false, 104), 0);
        assert_eq!(card_picture(&card(COURTS, 0, 31), false, 104), 31);
    }

    /// The Pick Up walk at 0x1df6b0..0x1df780 against the shipped program:
    /// only the Pick Up modes (2, 3: 0x1df6b4..0x1df6b8), the counter once
    /// (0x1df6d0), field 5 a positive threshold (0x1df6f0, 0x1df6fc), not
    /// owned (`FUN_001d26e8`), reached (0x1df718), field 1 the category
    /// (0x1df728; above 6 counts as granted directly, 0x1df6d4), category 0
    /// through notice 10 (0x1df754, one a game), over 0x71 rows (0x1df778).
    #[test]
    fn pickup_walk_is_the_executables() {
        use street_assets::scripts::{executable_constant, executable_words};
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let elf = iso.read("/SLUS_206.51").unwrap();
        let c = |a| executable_constant(&elf, a).unwrap();
        let low = |a| executable_words(&elf, a, 1).unwrap()[0] & 0xffff;
        assert_eq!(low(0x1df6b4), (-2i16) as u16 as u32);
        assert_eq!(low(0x1df6b8), 2);
        assert_eq!(low(0x1df6d0), 1);
        assert_eq!((c(0x1df6f0), c(0x1df728)), (5, 1));
        assert_eq!((c(0x1df6d4), c(0x1df754)), (6, 10));
        assert_eq!(low(0x1df778), 0x71);
        let cards = street_assets::db::cards(&mut iso).unwrap();
        assert!(cards.len() <= 0x71);
        // Two Street Legends with the same threshold: one a game.
        let legend = |id, param| Card {
            id,
            category: STREET_LEGENDS,
            index: 0,
            cost: 500,
            poster: 0,
            param,
        };
        let pair = [legend(2, 3), legend(3, 3)];
        let mut s = street_sim::story::Story::default();
        for _ in 0..3 {
            s.win_pickup(0);
        }
        assert_eq!(pickup_rewards(&mut s, &pair), [2]);
        s.win_pickup(0);
        assert_eq!(pickup_rewards(&mut s, &pair), [3]);
    }

    /// `FUN_002c2a90` against the shipped program and the disc's cards:
    /// only the two cards at `0x45e1f0` stay face down until owned, by
    /// the category table at `0x48f678`.
    #[test]
    fn hidden_cards_and_face_down_pictures_are_the_executables() {
        use street_assets::scripts::executable_words;
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let elf = iso.read("/SLUS_206.51").unwrap();
        let hidden = executable_words(&elf, 0x45e1f0, 1).unwrap()[0];
        assert_eq!(
            HIDDEN_UNTIL_OWNED,
            [hidden as u16 as i16, (hidden >> 16) as u16 as i16]
        );
        let table = executable_words(&elf, 0x48f678, 7).unwrap();
        assert_eq!(table, [3, 3, 4, 4, 4, 4, 4]);
        let cards = street_assets::db::cards(&mut iso).unwrap();
        for (row, card) in cards.iter().enumerate() {
            // The ownership bit and the hidden test key on the same id.
            assert_eq!(card.id as usize, row);
            let Some(&kind) = usize::try_from(card.category)
                .ok()
                .and_then(|c| table.get(c))
            else {
                continue;
            };
            let face_down = if kind == 3 {
                LEGEND_FACE_DOWN
            } else {
                FACE_DOWN
            };
            for owned in [false, true] {
                let expected = if HIDDEN_UNTIL_OWNED.contains(&card.id) && !owned {
                    face_down
                } else {
                    usize::try_from(card.poster).unwrap_or(face_down)
                };
                assert_eq!(card_picture(card, owned, 0x200), expected, "card {row}");
            }
        }
    }
}
