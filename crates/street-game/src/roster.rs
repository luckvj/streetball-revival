//! Playing an edited roster (`docs/PLAYER_EDITOR.md`, "In game"): a roster
//! mod's tables (`--roster-db`, the Legendary Edition) and the MyBaller
//! builder's `roster.json` (`--roster`).
//!
//! Both work at the database layer: the mod through
//! `street_assets::db::set_roster_override`, `roster.json` through
//! `street_assets::roster_edits`, set here before the disc loads so every
//! `db::players` / `db::teams` (the [`Disc`], menus, team select, matches)
//! returns the edited roster. Models and move choices are looked up by
//! record (`street_assets::player::look`, `clip::move_script_id`).
//!
//! Online, the match creator's roster is the match's roster
//! (`docs/PLAYER_EDITOR.md`, "Online"): it goes out as data in the match
//! setup ([`match_roster`], `street_assets::roster_share`), checked against
//! the online rules (`street_assets::roster_rules`; [`host_check`] refuses
//! to host an illegal one), and every joiner checks it again and plays it
//! for that match over its own disc ([`play_shared`]), going back to its
//! own afterwards ([`leave_shared`]). Names, team lists, ratings, moves and
//! animation choices are then the creator's on every machine; what a joiner
//! lacks to draw (an HD pack, GameCube models) falls back locally without
//! touching the simulation.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use bevy::prelude::Resource;
use street_assets::{
    db,
    roster_edits::{self, CharId, Created, RosterFile},
    roster_rules::{self, Problem},
    roster_share::{Base, MatchRoster, SharedRoster},
};

use crate::setup::Disc;

/// What this game checks online rosters with, read once: the disc's own
/// tables and this game's roster mod (`--roster-db`), if any. A joiner
/// trusts a mod base it lacks (the server it joined has checked it); the
/// server a hosting game runs does not ([`Original::for_server`]).
#[derive(Resource, Clone)]
pub struct Original(pub Arc<street_net::Known>);

impl Original {
    pub fn read(disc: &mut Disc) -> Result<Self, String> {
        let text = |e: street_assets::Error| e.to_string();
        let players = db::Table::read(&mut disc.iso, db::PLAYERS).map_err(text)?;
        let teams = db::Table::read(&mut disc.iso, db::TEAMS).map_err(text)?;
        let mods = match db::roster_override() {
            Some(dir) => {
                let (p, t) = db::roster_mod_tables(&dir, Some((&players, &teams))).map_err(text)?;
                vec![street_net::ModTables {
                    id: db::roster_mod_id(&dir).map_err(text)?,
                    players: p,
                    teams: t,
                }]
            }
            None => Vec::new(),
        };
        Ok(Self(Arc::new(street_net::Known {
            disc: Some(street_net::DiscTables {
                players: db::parse_players(&players).map_err(text)?,
                teams: db::parse_teams(&teams).map_err(text)?,
            }),
            mods,
            trust_mods: true,
        })))
    }

    /// The disc's own tables.
    pub fn disc(&self) -> &street_net::DiscTables {
        self.0.disc.as_ref().expect("read with the disc")
    }

    /// This game's roster mod's id, if it plays one.
    pub fn mod_id(&self) -> Option<[u8; 32]> {
        self.0.mods.first().map(|m| m.id)
    }

    /// For the server a hosting game runs: it holds this game's mod and
    /// accepts no other.
    pub fn for_server(&self) -> Arc<street_net::Known> {
        Arc::new(street_net::Known {
            trust_mods: false,
            ..(*self.0).clone()
        })
    }
}

/// Whether this game's roster may be hosted online: `Err` says why not.
/// Read once at start (the roster mod and `roster.json` do not change).
#[derive(Resource, Clone)]
pub struct HostCheck(pub Result<(), String>);

/// `--roster-db` / `STREET_ROSTER_DB`: use a roster mod's tables (the
/// folder, or its `database` / `ISO Files/database`) instead of the disc's.
/// Call before the disc loads.
pub fn use_roster_db(dir: &Path) -> Result<PathBuf, String> {
    let found = db::find_roster_dir(dir).ok_or_else(|| {
        format!(
            "--roster-db: no {} in {} or its database folders",
            db::PLAYERS_FILE,
            dir.display()
        )
    })?;
    db::set_roster_override(Some(&found)).map_err(|e| format!("--roster-db: {e}"))?;
    Ok(found)
}

/// `--roster` / `STREET_ROSTER`: read `roster.json` and set it for every
/// later read of the tables. Call after [`use_roster_db`], before the disc
/// loads.
pub fn use_roster(path: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file = RosterFile::parse(&text)?;
    let modded = db::roster_override().is_some();
    match file.wants_mod() {
        Some(true) if !modded => {
            return Err(format!(
                "{} was made on the {} roster: start with --roster-db <its folder>",
                path.display(),
                file.base.as_deref().unwrap_or("modded")
            ));
        }
        Some(false) if modded => {
            return Err(format!(
                "{} was made on the original roster: start without --roster-db",
                path.display()
            ));
        }
        _ => {}
    }
    roster_edits::set(Some(file));
    Ok(())
}

/// With the save loaded: give `roster.json` the save's created ballers (for
/// `created:<n>` characters and sources) and re-read the tables if it
/// uses any. Returns the lines to print about the roster.
pub fn with_save(disc: &mut Disc, story: &street_sim::story::Story) -> Result<Vec<String>, String> {
    let Some(file) = roster_edits::active() else {
        return Ok(Vec::new());
    };
    if file.uses_created() {
        let template = disc
            .players
            .get(roster_edits::TEMPLATE_RECORD)
            .cloned()
            .ok_or("no create-a-player record")?;
        let created: BTreeMap<usize, Created> = story
            .ballers
            .iter()
            .map(|b| {
                let mut player = template.clone();
                player.index = b.record();
                crate::ballers::apply(&mut player, b);
                (
                    usize::from(b.slot),
                    Created {
                        player,
                        moves: b.moves.clone(),
                    },
                )
            })
            .collect();
        roster_edits::set_created(created);
        disc.players = db::players(&mut disc.iso).map_err(|e| e.to_string())?;
        disc.teams = db::teams(&mut disc.iso).map_err(|e| e.to_string())?;
        disc.base_teams = disc.teams.len();
    }
    let mut lines = roster_edits::notes();
    let custom = disc
        .teams
        .iter()
        .find(|t| t.name == "Custom Roster")
        .map(|t| format!("; team select: {}", t.full_name()))
        .unwrap_or_default();
    lines.push(format!(
        "{} players from roster.json{custom}",
        file.players.len()
    ));
    Ok(lines)
}

/// This game's roster as an online match shares it: the database's tables
/// (the disc's, or a roster mod's, with `roster.json` applied; never the
/// save's created ballers or teams, which travel with the match) as
/// differences from the disc's own.
pub fn shared_roster(disc: &mut Disc, original: &Original) -> Result<SharedRoster, String> {
    let players = db::players(&mut disc.iso).map_err(|e| e.to_string())?;
    let teams = db::teams(&mut disc.iso).map_err(|e| e.to_string())?;
    let hints: BTreeMap<usize, usize> = roster_edits::active()
        .map(|file| {
            file.records()
                .into_iter()
                .zip(&file.players)
                .filter_map(|(record, p)| {
                    let source = CharId::parse(&p.character).ok()?.record()?;
                    (source < street_assets::roster_share::DISC_RECORDS).then_some((record, source))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(SharedRoster::diff(
        original.mod_id().map_or(Base::Original, Base::Mod),
        (&original.disc().players, &original.disc().teams),
        (&players, &teams),
        &roster_edits::applied(),
        &roster_edits::created_moves(),
        &hints,
    ))
}

/// Whether this game's roster may be played online; the problems, said
/// plainly, if not. Offline anything goes.
pub fn host_check(disc: &mut Disc, original: &Original) -> HostCheck {
    let problems: Result<Vec<Problem>, String> = shared_roster(disc, original).map(|shared| {
        let base = match original.0.mods.first() {
            Some(m) => &m.players,
            None => &original.disc().players,
        };
        roster_rules::check_shared(
            &shared,
            &roster_rules::Context {
                disc: Some((&original.disc().players, &original.disc().teams)),
                base: Some(base),
                limits: roster_rules::Limits::from_players(base),
            },
        )
    });
    HostCheck(match problems {
        Ok(p) if p.is_empty() => Ok(()),
        Ok(p) => Err(refusal(
            &p.iter().map(ToString::to_string).collect::<Vec<_>>(),
        )),
        Err(e) => Err(format!("This roster cannot be played online: {e}")),
    })
}

/// The message for an illegal roster: the first few reasons.
pub fn refusal(reasons: &[String]) -> String {
    const SHOWN: usize = 3;
    let mut text = format!(
        "This roster breaks the online rules (docs/PLAYER_EDITOR.md, Rules): {}",
        reasons
            .iter()
            .take(SHOWN)
            .cloned()
            .collect::<Vec<_>>()
            .join("; ")
    );
    if reasons.len() > SHOWN {
        text.push_str(&format!(" (and {} more)", reasons.len() - SHOWN));
    }
    text
}

/// What a match this game creates carries: its roster and the line-up's
/// records, packed for the setup.
pub fn match_roster(
    disc: &mut Disc,
    original: &Original,
    lineup: &[db::Player],
) -> Result<Vec<u8>, String> {
    MatchRoster {
        roster: shared_roster(disc, original)?,
        lineup: lineup.to_vec(),
    }
    .pack()
}

/// A joiner: check the match's roster and world with this disc, and play
/// that roster for the match (re-reading the tables when it changes). A
/// match without one plays the disc's own tables, whatever roster this game
/// has. Errs, saying why, when the match is illegal.
pub fn play_shared(
    disc: &mut Disc,
    original: &Original,
    setup: &street_net::MatchSetup,
) -> Result<(), String> {
    let roster = match street_net::roster::check_setup(setup, &original.0) {
        Ok(Some(m)) => m.roster,
        Ok(None) => SharedRoster {
            players_len: original.disc().players.len() as u16,
            teams_len: original.disc().teams.len() as u16,
            ..Default::default()
        },
        Err(why) => {
            let reasons = why.reasons();
            return Err(format!(
                "The match's roster breaks the online rules: {}",
                reasons.join("; ")
            ));
        }
    };
    if roster_edits::shared().as_ref() == Some(&roster) {
        return Ok(());
    }
    roster_edits::set_shared(Some(roster));
    reload(disc)
}

/// Back to this game's own roster after an online match, with the save's
/// created ballers and teams on it again.
pub fn leave_shared(disc: &mut Disc, story: &street_sim::story::Story) -> Result<(), String> {
    if roster_edits::shared().is_none() {
        return Ok(());
    }
    roster_edits::set_shared(None);
    reload(disc)?;
    crate::ballers::apply_all(disc, story);
    crate::teams::apply_all(disc, story);
    Ok(())
}

fn reload(disc: &mut Disc) -> Result<(), String> {
    disc.players = db::players(&mut disc.iso).map_err(|e| e.to_string())?;
    disc.teams = db::teams(&mut disc.iso).map_err(|e| e.to_string())?;
    disc.base_teams = disc.teams.len();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn added_records_follow_the_career_stars() {
        use street_sim::story::{BALLER_RECORDS, STAR_SLOTS};
        assert_eq!(
            roster_edits::FIRST_ADDED,
            usize::from(BALLER_RECORDS) + STAR_SLOTS
        );
        assert_eq!(roster_edits::TEMPLATE_RECORD, usize::from(BALLER_RECORDS));
        assert_eq!(roster_edits::MOVE_SLOTS, street_sim::story::MOVE_SLOTS);
        assert_eq!(roster_edits::EMPTY, street_sim::story::EMPTY_MOVE_SLOT);
    }

    /// Disc-gated (`STREET_ISO`, `STREET_ROSTER_DB`): the roster override is
    /// process-wide, so the check runs in a child test process.
    #[test]
    fn the_legendary_roster_reaches_team_select() {
        if std::env::var_os("STREET_ISO").is_none()
            || std::env::var_os("STREET_ROSTER_DB").is_none()
        {
            return;
        }
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "roster::tests::legendary_child", "--ignored"])
            .args(["--nocapture", "--test-threads", "1"])
            .env("STREET_ROSTER_CHILD", "1")
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout).into_owned()
            + &String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success() && text.contains("1 passed"), "{text}");
    }

    #[test]
    #[ignore = "run by the_legendary_roster_reaches_team_select"]
    fn legendary_child() {
        if std::env::var_os("STREET_ROSTER_CHILD").is_none() {
            return;
        }
        let dir = std::env::var_os("STREET_ROSTER_DB").unwrap();
        use_roster_db(Path::new(&dir)).unwrap();
        let iso_path = std::env::var_os("STREET_ISO").unwrap();
        let disc = Disc::load(street_assets::iso::Iso::open(&iso_path).unwrap()).unwrap();
        assert_eq!(disc.players[0].name(), "Michael Redd");
        let teams: Vec<String> = disc
            .playable_teams()
            .into_iter()
            .map(|t| disc.teams[t].full_name())
            .collect();
        println!("teams: {teams:?}");
        for wanted in ["Brooklyn Nets", "Oklahoma City Thunder"] {
            assert!(teams.iter().any(|t| t == wanted), "{wanted} in {teams:?}");
        }
        let atlanta = disc.find_team("Atlanta").unwrap();
        let names: Vec<String> = disc.teams[atlanta]
            .roster
            .iter()
            .map(|&r| disc.players[r].name())
            .collect();
        println!("Atlanta: {names:?}");
        assert!(names.iter().any(|n| n == "Gianna Bryant"), "{names:?}");

        // roster.json on top: record 0 in Dr. J's model with his moves, an
        // added player on Atlanta, and the custom team.
        let path = std::env::temp_dir().join("street-roster-test.json");
        std::fs::write(
            &path,
            r#"{"version": 1, "base": "legendary", "players": [
                {"slot": 0, "character": "vol2:0", "model": "vol2:158",
                 "edits": {"firstName": "Test"}, "animations": {"from": "vol2:158"}},
                {"slot": null, "character": "vol2:1", "team": "vol2:0"},
                {"slot": null, "character": "vol2:2"}]}"#,
        )
        .unwrap();
        use_roster(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let mut disc = Disc::load(street_assets::iso::Iso::open(&iso_path).unwrap()).unwrap();
        let notes = with_save(&mut disc, &Default::default()).unwrap();
        println!("{notes:?}");
        assert_eq!(disc.players[0].name(), "Test Redd");
        let first = roster_edits::FIRST_ADDED;
        assert_eq!(disc.players.len(), first + 2);
        assert!(disc.teams[0].roster.contains(&first));
        let custom = disc.find_team("Custom").unwrap();
        assert_eq!(disc.teams[custom].roster, vec![0, first, first + 1]);
        assert_eq!(disc.base_teams, disc.teams.len());
        assert_eq!(
            roster_edits::look_for(&disc.players[0]).map(|p| p.index),
            Some(158)
        );
        assert_eq!(roster_edits::gait_source(0).map(|g| g.0), Some(158));
        // Dr. J's (0x9e) own slot-1 dunk, script 0x1b, now plays for record 0.
        let clips = crate::setup::load_clips(&mut disc.iso).unwrap();
        let dunk = |m: &crate::clip::PlayerMoves| {
            crate::clip::move_script_id(&clips, m, &disc, 0, street_assets::moves::Kind::Dunk, 1)
        };
        assert_eq!(
            dunk(&crate::clip::PlayerMoves(vec![(0, Default::default())])),
            Some(0x1b)
        );
        assert_eq!(
            dunk(&crate::clip::PlayerMoves(vec![(1, Default::default())])),
            clips.scripts.default_dunks.get(1).copied()
        );
    }
}
