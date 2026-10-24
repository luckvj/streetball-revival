//! Create Team: up to ten teams of five ("Select 5 players for your team."),
//! drawn from the league players, the league and street legends unlocked in the
//! Rewards, and the created ballers; each with a city ("Where's Your Team
//! From?"), a name ("What's Your Team's Name?") and a logo from
//! `/FE/XATEAMS.BIN`. Saved teams join the database's teams, after them, so
//! Pick Up games can choose them.
//!
//! The game keeps created teams in its memory card data, not in the
//! database; the rewrite keeps them in the save ([`Story::teams`]).

use bevy::prelude::*;
use street_assets::db::{self, PlayerKind};
use street_sim::story::{CreatedTeam, Story, TEAM_PLAYERS, TEAM_SLOTS};

use crate::setup::Disc;

/// The logos a team can wear: every image of `XATEAMS`.
pub const LOGOS: u8 = 50;
/// A new team's logo: the generic ballers' (team 36's).
const NEW_LOGO: u8 = 41;

/// The editor's rows, in order (then the players, Save and Delete).
pub const ROWS: [&str; 3] = ["City", "Name", "Logo"];
/// The first player row.
pub const PLAYER_ROW: usize = ROWS.len();
/// The Save Team row.
pub const SAVE_ROW: usize = PLAYER_ROW + TEAM_PLAYERS;
/// The Delete Team row (only for a saved team).
pub const DELETE_ROW: usize = SAVE_ROW + 1;

/// Puts every saved team after the database's teams (replacing any put
/// there before), and each league team's sixth man, a created baller who
/// joined it (`FUN_001ebf80(team, 5)`: the database's field 9, else the
/// game data's +0x3c), after its five.
pub fn apply_all(disc: &mut Disc, story: &Story) {
    let base = disc.base_teams;
    disc.teams.truncate(base);
    let created: Vec<usize> = story.ballers.iter().map(|b| b.record()).collect();
    for (k, t) in disc
        .teams
        .iter_mut()
        .take(street_sim::story::LEAGUE_TEAMS)
        .enumerate()
    {
        t.roster.retain(|r| !created.contains(r));
        if t.roster.len() < 6
            && let Some(record) = story.sixth_man(k as u16)
            && created.contains(&usize::from(record))
        {
            t.roster.push(usize::from(record));
        }
    }
    for (k, t) in story.teams.iter().enumerate() {
        disc.teams.push(db::Team {
            index: base + k,
            name: format!("{} {}", t.city, t.name),
            city: t.city.clone(),
            nickname: t.name.clone(),
            roster: t.roster.iter().map(|&r| usize::from(r)).collect(),
            logo: i16::from(t.logo),
        });
    }
}

/// Who can join a team, by group: (group, records).
pub fn pool(disc: &Disc, story: &Story) -> Vec<(&'static str, Vec<usize>)> {
    let of = |kind: PlayerKind, unlocked: bool| -> Vec<usize> {
        disc.players
            .iter()
            .filter(|p| p.kind() == kind)
            .map(|p| p.index)
            .filter(|&r| !unlocked || story.unlocked_players.contains(&(r as u16)))
            .collect()
    };
    let mut ballers: Vec<usize> = story.ballers.iter().map(|b| b.record()).collect();
    ballers.sort_unstable();
    vec![
        ("League Players", of(PlayerKind::League, false)),
        ("Legends", of(PlayerKind::Legend, true)),
        ("Street Legends", of(PlayerKind::Streetball, true)),
        ("Created Ballers", ballers),
    ]
}

/// Everyone in the pool, in the pool's order.
pub fn everyone(disc: &Disc, story: &Story) -> Vec<usize> {
    pool(disc, story).into_iter().flat_map(|(_, r)| r).collect()
}

/// The team being edited, if any.
#[derive(Resource, Default)]
pub struct TeamEditing {
    pub team: Option<CreatedTeam>,
    pub slot: usize,
    /// The player row whose pick is open (Choose a player).
    pub picking: usize,
    /// Cities and names to cycle through with left and right: the
    /// database's own.
    pub cities: Vec<String>,
    pub names: Vec<String>,
    /// The saved teams changed: [`apply_commit`] puts them on the Disc.
    pub dirty: bool,
}

/// After a save or a delete: the Disc's teams follow the save, and a pick
/// that pointed past them falls back to the database's.
pub fn apply_commit(
    mut e: ResMut<TeamEditing>,
    mut disc: ResMut<Disc>,
    save: Res<crate::story::Save>,
    mut setup: ResMut<crate::setup::MatchSetup>,
) {
    if !std::mem::take(&mut e.dirty) {
        return;
    }
    apply_all(&mut disc, &save.story);
    for t in &mut setup.teams {
        if *t >= disc.teams.len() {
            *t = 0;
        }
    }
}

/// A new team for `slot`: the game's own default ("NEW YORK" "BALLERS")
/// with the first five league players.
pub fn new(disc: &Disc, story: &Story, city: &str, name: &str) -> CreatedTeam {
    let mut roster = [0u16; TEAM_PLAYERS];
    for (r, record) in roster.iter_mut().zip(everyone(disc, story)) {
        *r = record as u16;
    }
    CreatedTeam {
        city: city.to_owned(),
        name: name.to_owned(),
        logo: NEW_LOGO,
        roster,
    }
}

/// The cities and nicknames of the database's teams, for cycling.
pub fn presets(disc: &Disc) -> (Vec<String>, Vec<String>) {
    let mut cities: Vec<String> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for t in disc.teams.iter().take(disc.base_teams) {
        if !cities.contains(&t.city) {
            cities.push(t.city.clone());
        }
        if !names.contains(&t.nickname) {
            names.push(t.nickname.clone());
        }
    }
    (cities, names)
}

/// The editor's lines.
pub fn rows(t: &CreatedTeam, disc: &Disc, saved: bool) -> Vec<String> {
    let mut out = vec![
        format!("City  < {} >", t.city),
        format!("Name  < {} >", t.name),
        format!("Logo  < {} of {LOGOS} >", t.logo + 1),
    ];
    for (k, &r) in t.roster.iter().enumerate() {
        let name = disc
            .players
            .get(usize::from(r))
            .map_or_else(|| "?".to_owned(), |p| p.name());
        out.push(format!("Player {}  < {name} >", k + 1));
    }
    out.push("Save Team".into());
    if saved {
        out.push("Delete Team".into());
    }
    out
}

fn cycle(list: &[String], now: &str, step: i32) -> Option<String> {
    if list.is_empty() {
        return None;
    }
    let at = list.iter().position(|s| s == now).map_or(-1, |k| k as i32);
    let next = (at + step).rem_euclid(list.len() as i32) as usize;
    Some(list[next].clone())
}

/// Left or right on `row`: the next city, name, logo or player.
pub fn adjust(e: &mut TeamEditing, row: usize, step: i32, disc: &Disc, story: &Story) {
    let (cities, names) = (e.cities.clone(), e.names.clone());
    let Some(t) = e.team.as_mut() else { return };
    match row {
        0 => t.city = cycle(&cities, &t.city, step).unwrap_or_else(|| t.city.clone()),
        1 => t.name = cycle(&names, &t.name, step).unwrap_or_else(|| t.name.clone()),
        2 => t.logo = (i32::from(t.logo) + step).rem_euclid(i32::from(LOGOS)) as u8,
        r if (PLAYER_ROW..SAVE_ROW).contains(&r) => {
            let all = everyone(disc, story);
            let k = r - PLAYER_ROW;
            let now = all.iter().position(|&p| p == usize::from(t.roster[k]));
            // The next player not already on the team.
            let n = all.len() as i32;
            let mut at = now.map_or(-1, |a| a as i32);
            for _ in 0..n {
                at = (at + step).rem_euclid(n.max(1));
                let record = all[at as usize] as u16;
                if !t.roster.contains(&record) {
                    t.roster[k] = record;
                    break;
                }
            }
        }
        _ => {}
    }
}

/// Puts `record` in `row`'s place; if it was already on the team, the two
/// trade places.
pub fn pick(t: &mut CreatedTeam, row: usize, record: u16) {
    if let Some(other) = t.roster.iter().position(|&r| r == record) {
        t.roster.swap(other, row);
    } else {
        t.roster[row] = record;
    }
}

/// Saves the edited team in its slot ("There are already 10 existing
/// Created Teams." keeps a new one out when all are taken).
pub fn commit(e: &mut TeamEditing, story: &mut Story) -> Result<(), &'static str> {
    let Some(t) = e.team.take() else {
        return Ok(());
    };
    if e.slot < story.teams.len() {
        story.teams[e.slot] = t;
    } else if story.teams.len() < TEAM_SLOTS {
        story.teams.push(t);
    } else {
        e.team = Some(t);
        return Err("There are already 10 existing Created Teams.");
    }
    Ok(())
}

/// The list page: the saved teams, a new team while there's room, Back.
pub fn slot_items(story: &Story) -> Vec<String> {
    let mut out: Vec<String> = story
        .teams
        .iter()
        .enumerate()
        .map(|(k, t)| format!("{}. {} {}", k + 1, t.city, t.name))
        .collect();
    if story.teams.len() < TEAM_SLOTS {
        out.push("New Team".into());
    }
    out.push("Back".into());
    out
}

/// The pick page's lines: each group's players, with the group named.
pub fn pick_items(disc: &Disc, story: &Story) -> Vec<String> {
    pool(disc, story)
        .into_iter()
        .flat_map(|(group, records)| records.into_iter().map(move |r| (group, r)))
        .map(|(group, r)| {
            let p = &disc.players[r];
            let from = match p.kind() {
                PlayerKind::League => disc
                    .teams
                    .iter()
                    .find(|t| t.roster.contains(&r) && t.index < 29)
                    .map_or(group.to_owned(), |t| t.nickname.clone()),
                _ => group.trim_end_matches('s').to_owned(),
            };
            format!(
                "{}  {}' {}\"  {from}",
                p.name(),
                p.height_in / 12,
                p.height_in % 12
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn team() -> CreatedTeam {
        CreatedTeam {
            city: "New York".into(),
            name: "Ballers".into(),
            logo: 41,
            roster: [1, 2, 3, 4, 5],
        }
    }

    #[test]
    fn picking_someone_on_the_team_trades_places() {
        let mut t = team();
        pick(&mut t, 0, 4);
        assert_eq!(t.roster, [4, 2, 3, 1, 5]);
        pick(&mut t, 1, 99);
        assert_eq!(t.roster, [4, 99, 3, 1, 5]);
    }

    #[test]
    fn the_eleventh_team_waits() {
        let mut story = Story::default();
        let mut e = TeamEditing::default();
        for k in 0..TEAM_SLOTS {
            e.team = Some(team());
            e.slot = k;
            assert!(commit(&mut e, &mut story).is_ok());
        }
        e.team = Some(team());
        e.slot = TEAM_SLOTS;
        assert!(commit(&mut e, &mut story).is_err());
        assert!(e.team.is_some(), "kept for another try");
        // Editing a saved one still works.
        e.slot = 3;
        assert!(commit(&mut e, &mut story).is_ok());
        assert_eq!(story.teams.len(), TEAM_SLOTS);
    }

    #[test]
    fn names_cycle_through_the_presets() {
        let list = vec!["A".to_owned(), "B".to_owned()];
        assert_eq!(cycle(&list, "A", 1).as_deref(), Some("B"));
        assert_eq!(cycle(&list, "A", -1).as_deref(), Some("B"));
        assert_eq!(cycle(&list, "typed", 1).as_deref(), Some("A"));
        assert_eq!(cycle(&[], "x", 1), None);
    }
}
