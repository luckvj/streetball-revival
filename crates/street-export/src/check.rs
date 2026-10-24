//! `street-export check-roster`: whether a `roster.json` may be played
//! online (`street_assets::roster_rules`, `docs/PLAYER_EDITOR.md`, "Rules").
//!
//! ```text
//! street-export check-roster <roster.json> --vol2 <iso> [--roster-db <dir>]
//!                            [--save <street_save.dat>]
//! ```
//!
//! prints `{"version":1,"legal":bool,"fingerprint":"<hex>","problems":[...]}`
//! and exits 0 when legal, 2 when not, 1 on an error (no disc, a file that
//! does not parse or apply). `--save` gives `created:<n>` characters their
//! saved ballers (else they start from the create-a-player record).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use street_assets::db::{self, Player};
use street_assets::iso::Iso;
use street_assets::roster_edits::{Created, RosterFile, TEMPLATE_RECORD};
use street_assets::roster_rules::{self, Report};
use street_sim::story::{CreatedBaller, Story};

/// The command line, parsed.
#[derive(Debug, Default, PartialEq)]
pub struct Options {
    pub roster: PathBuf,
    pub vol2: Option<PathBuf>,
    pub roster_db: Option<PathBuf>,
    pub save: Option<PathBuf>,
}

pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        vol2: std::env::var_os("STREET_ISO").map(PathBuf::from),
        ..Default::default()
    };
    let mut roster = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{arg} needs a value (see usage)"))
        };
        match arg.as_str() {
            "--vol2" => o.vol2 = Some(value()?),
            "--roster-db" => o.roster_db = Some(value()?),
            "--save" => o.save = Some(value()?),
            flag if flag.starts_with("--") => {
                return Err(format!("unknown option {flag} (see usage)"));
            }
            path if roster.is_none() => roster = Some(PathBuf::from(path)),
            extra => return Err(format!("unknown argument {extra} (see usage)")),
        }
    }
    o.roster = roster.ok_or("check-roster needs a roster.json (see usage)")?;
    Ok(o)
}

/// A saved created baller as its record (as `street-game`'s
/// `ballers::apply` writes one over the create-a-player template).
pub fn created_player(template: &Player, b: &CreatedBaller) -> Player {
    const SKILLS: [usize; 7] = [0, 2, 4, 5, 6, 7, 3];
    let mut p = template.clone();
    p.index = b.record();
    p.first_name.clone_from(&b.first_name);
    p.last_name.clone_from(&b.last_name);
    p.female = b.female;
    p.build = i16::from(b.build);
    p.height_in = i16::from(b.height_in);
    p.model_height_in = i16::from(b.height_in);
    p.weight_lb = b.weight_lb as i16;
    p.model_weight_lb = b.weight_lb as i16;
    p.look = i16::from(b.look);
    p.hair_style = i16::from(b.hair_style);
    p.hair_color = i16::from(b.hair_color);
    p.facial_hair_style = if b.female {
        -1
    } else {
        i16::from(b.facial_hair_style)
    };
    p.facial_hair_color = i16::from(b.facial_hair_color);
    p.clothing = i16::from(b.clothing) | (i16::from(b.clothing_variant) << 14);
    p.shoes = i16::from(b.shoes);
    for (k, &rating) in SKILLS.iter().enumerate() {
        p.ratings[rating] = i16::from(b.skills[k]);
    }
    let bar = street_sim::cab::SKILL_LEVELS[0]
        .iter()
        .position(|&r| b.skills[0] <= r)
        .unwrap_or(6);
    p.ratings[1] = [0, 10, 20, 40, 60, 80, 90][bar];
    p.ratings[8] = [85, 75, 65, 55, 80, 70][usize::from(b.build.min(5))];
    p
}

/// Check the roster: the report, or an error.
pub fn run(o: &Options) -> Result<Report, String> {
    let text =
        std::fs::read_to_string(&o.roster).map_err(|e| format!("{}: {e}", o.roster.display()))?;
    let file = RosterFile::parse(&text)?;
    let vol2 = o
        .vol2
        .as_ref()
        .ok_or("check-roster needs --vol2 <iso> or $STREET_ISO (see usage)")?;
    let mut iso = Iso::open(vol2).map_err(|e| format!("{}: {e}", vol2.display()))?;
    let disc_players = db::original_players(&mut iso).map_err(|e| e.to_string())?;
    let disc_teams = db::original_teams(&mut iso).map_err(|e| e.to_string())?;
    match (&o.roster_db, file.wants_mod()) {
        (None, Some(true)) => {
            return Err(format!(
                "{} was made on the {} roster: give --roster-db <its folder>",
                o.roster.display(),
                file.base.as_deref().unwrap_or("modded")
            ));
        }
        (Some(_), Some(false)) => {
            return Err(format!(
                "{} was made on the original roster: leave out --roster-db",
                o.roster.display()
            ));
        }
        _ => {}
    }
    let mut base = street_assets::roster_share::Base::Original;
    if let Some(dir) = &o.roster_db {
        let found = db::find_roster_dir(dir)
            .ok_or_else(|| format!("no {} under {}", db::PLAYERS_FILE, dir.display()))?;
        db::set_roster_override(Some(&found)).map_err(|e| e.to_string())?;
        base = street_assets::roster_share::Base::Mod(
            db::roster_mod_id(&found).map_err(|e| e.to_string())?,
        );
    }
    let players = db::players(&mut iso).map_err(|e| e.to_string())?;
    let teams = db::teams(&mut iso).map_err(|e| e.to_string())?;
    let created = match &o.save {
        Some(path) => saved_ballers(path, &players)?,
        None => BTreeMap::new(),
    };
    roster_rules::check_file(
        &file,
        (&players, &teams),
        (&disc_players, &disc_teams),
        &created,
        base,
    )
}

fn saved_ballers(path: &Path, players: &[Player]) -> Result<BTreeMap<usize, Created>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let story = Story::read(&bytes).ok_or_else(|| format!("{}: not a save", path.display()))?;
    let template = players
        .get(TEMPLATE_RECORD)
        .ok_or("no create-a-player record")?;
    Ok(story
        .ballers
        .iter()
        .map(|b| {
            (
                usize::from(b.slot),
                Created {
                    player: created_player(template, b),
                    moves: b.moves.clone(),
                },
            )
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn the_command_line() {
        let o = parse(&args("r.json --vol2 d.iso --roster-db L --save s.dat")).unwrap();
        assert_eq!(o.roster, PathBuf::from("r.json"));
        assert_eq!(o.vol2, Some("d.iso".into()));
        assert_eq!(o.roster_db, Some("L".into()));
        assert_eq!(o.save, Some("s.dat".into()));
        assert!(parse(&args("--vol2 d.iso")).is_err());
        assert!(parse(&args("r.json --frob 1")).is_err());
        assert!(parse(&args("r.json other.json")).is_err());
    }

    #[test]
    fn create_a_baller_sizes_and_caps_match_the_sims() {
        use roster_rules::{CAB_RATING_CAPS, CAB_SIZES, CAB_TOTAL_CAP};
        use street_sim::cab::{SIZES, SKILL_LEVELS};
        for (build, sizes) in SIZES.iter().enumerate() {
            let h = sizes.map(|s| i16::from(s.1));
            let w = sizes.map(|s| s.2 as i16);
            assert_eq!(
                CAB_SIZES[build],
                [
                    *h.iter().min().unwrap(),
                    *h.iter().max().unwrap(),
                    *w.iter().min().unwrap(),
                    *w.iter().max().unwrap()
                ]
            );
        }
        let top = i16::from(SKILL_LEVELS[1][6]);
        assert_eq!(top, i16::from(SKILL_LEVELS[0][6]));
        // A baller with every bar full, on the quickest build.
        let b = CreatedBaller {
            skills: [
                SKILL_LEVELS[0][6],
                top as u8,
                top as u8,
                top as u8,
                top as u8,
                top as u8,
                top as u8,
            ],
            build: 0,
            height_in: 69,
            weight_lb: 200,
            ..Default::default()
        };
        let p = created_player(&street_assets::roster_share::blank(234), &b);
        for (k, &cap) in CAB_RATING_CAPS.iter().enumerate().take(9) {
            assert!(p.ratings[k] <= cap, "rating {k}");
        }
        assert_eq!(roster_rules::total(&p), CAB_TOTAL_CAP);
    }
}
