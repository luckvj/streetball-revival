//! Checks of the database decoder against a real USA disc image of the original game.
//! Skipped unless STREET_ISO points at the image, since the repo ships no game data.

use std::{collections::BTreeMap, fs::File, io::BufReader};

use street_assets::{
    big::{self, BigEntry},
    court,
    db::{self, PlayerKind, PlayerModel, Position},
    iso::Iso,
};

type Disc = Iso<BufReader<File>>;

fn disc() -> Option<Disc> {
    let path = std::env::var_os("STREET_ISO")?;
    Some(Iso::open(path).expect("STREET_ISO is not a readable ISO9660 image"))
}

#[test]
fn first_player_is_shareef_abdur_rahim_of_the_hawks() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let players = db::players(&mut iso).unwrap();
    let teams = db::teams(&mut iso).unwrap();
    assert_eq!(players.len(), 240);
    let p = &players[0];
    assert_eq!(p.name(), "Shareef Abdur-Rahim");
    assert_eq!(p.code, "sabd");
    assert_eq!(p.jersey(), Some(3));
    assert_eq!(teams[p.team as usize].full_name(), "Atlanta Hawks");
    assert_eq!((p.height_in, p.kind()), (81, PlayerKind::League));
    // Shaquille O'Neal: Lakers #34, center, 7'1".
    let shaq = players
        .iter()
        .find(|p| p.last_name == "O'Neal" && p.first_name == "Shaquille");
    let shaq = shaq.expect("Shaquille O'Neal");
    assert_eq!(teams[shaq.team as usize].nickname, "Lakers");
    assert_eq!(
        (shaq.jersey(), shaq.position(), shaq.height_in),
        (Some(34), Some(Position::Center), 85)
    );
}

#[test]
fn league_rosters_are_the_first_145_records() {
    let Some(mut iso) = disc() else { return };
    let players = db::players(&mut iso).unwrap();
    let teams = db::teams(&mut iso).unwrap();
    let mut seen = [false; db::LEAGUE_PLAYERS];
    for team in &teams[..29] {
        assert_eq!(team.roster.len(), 5, "{}", team.full_name());
        for &p in &team.roster {
            assert_eq!(
                players[p].team as usize,
                team.index,
                "{}",
                players[p].name()
            );
            assert_eq!(players[p].kind(), PlayerKind::League);
            seen[p] = true;
        }
    }
    assert!(
        seen.iter().all(|&s| s),
        "every league record is on exactly one roster"
    );
    let created = &teams[db::CREATED_PLAYER_TEAM as usize];
    assert_eq!(created.name, "CP Team");
}

#[test]
fn court_ids_match_the_court_archives() {
    let Some(mut iso) = disc() else { return };
    let courts = db::courts(&mut iso).unwrap();
    let mut ids: Vec<String> = courts.iter().map(|c| c.id.clone()).collect();
    ids.sort();
    let mut archives = court::court_ids(&mut iso).unwrap();
    // GREENC is a floor-and-backdrop scene, not a playable court.
    archives.retain(|id| id != "GREENC");
    archives.sort();
    assert_eq!(ids, archives);
    let rucker = courts.iter().find(|c| c.id == "RUCKER").unwrap();
    assert_eq!(
        (rucker.name.as_str(), rucker.location.as_str()),
        ("Rucker Park", "Harlem, NY")
    );
    for c in &courts {
        let sum: f32 = c.tier_mix.iter().sum();
        // Rucker Park '78 has no mix at all.
        assert!(
            sum.abs() < 1e-4 || (sum - 1.0).abs() < 1e-4,
            "{}: {sum}",
            c.name
        );
    }
}

#[test]
fn nicknames_have_seven_per_skill() {
    let Some(mut iso) = disc() else { return };
    let names = db::nicknames(&mut iso).unwrap();
    for skill in db::Skill::ALL {
        let n = names.iter().filter(|n| n.skill == Some(skill)).count();
        assert!((6..=7).contains(&n), "{skill:?}: {n}");
    }
    assert_eq!(db::moves(&mut iso).unwrap().len(), 93);
}

/// Reads archive entries by index, caching the index and each entry.
struct Archive {
    path: &'static str,
    entries: Vec<BigEntry>,
    cache: BTreeMap<usize, Vec<u8>>,
}

impl Archive {
    fn open(iso: &mut Disc, path: &'static str) -> Self {
        let entries = big::read_index(iso, path).unwrap();
        Self {
            path,
            entries,
            cache: BTreeMap::new(),
        }
    }

    fn bytes(&mut self, iso: &mut Disc, i: usize) -> &[u8] {
        let (path, entry) = (self.path, &self.entries[i]);
        self.cache
            .entry(i)
            .or_insert_with(|| big::read_entry(iso, path, entry).unwrap())
    }

    /// The `__Model:::` name of an EAGL object.
    fn model(&mut self, iso: &mut Disc, i: usize) -> String {
        symbol(self.bytes(iso, i), b"__Model:::")
            .unwrap_or_else(|| panic!("entry {i} is not a model"))
    }

    fn is_morph(&mut self, iso: &mut Disc, i: usize) -> bool {
        symbol(self.bytes(iso, i), b"__Morph:::").is_some()
    }

    /// Image names of an SHPS sheet, from its directory alone.
    fn sheet(&mut self, iso: &mut Disc, i: usize) -> Vec<String> {
        let e = &self.entries[i];
        let head = iso.read_range(self.path, e.offset as u64, 16).unwrap();
        assert_eq!(&head[..4], b"SHPS", "entry {i} is not a sheet");
        let n = u32::from_le_bytes(head[8..12].try_into().unwrap()) as usize;
        let dir = iso
            .read_range(self.path, e.offset as u64 + 16, 8 * n)
            .unwrap();
        dir.chunks(8)
            .map(|d| String::from_utf8_lossy(&d[..4]).into_owned())
            .collect()
    }
}

fn symbol(bytes: &[u8], tag: &[u8]) -> Option<String> {
    let at = bytes.windows(tag.len()).position(|w| w == tag)? + tag.len();
    let len = bytes[at..].iter().position(|&b| b == 0)?;
    Some(String::from_utf8_lossy(&bytes[at..at + len]).into_owned())
}

#[test]
fn player_models_point_at_the_right_archive_entries() {
    let Some(mut iso) = disc() else { return };
    let players = db::players(&mut iso).unwrap();
    let mut xf1 = Archive::open(&mut iso, "/XF/XF000001");
    let mut xf2 = Archive::open(&mut iso, "/XF/XF000002");
    let mut unique_names = BTreeMap::new();
    for p in &players {
        let who = p.name();
        match p.model().unwrap_or_else(|| panic!("{who}: no model")) {
            PlayerModel::League(m) => {
                let size = ["Small", "Medium", "Large"][p.build as usize];
                for (v, (&body, &morph)) in m.bodies.iter().zip(&m.morphs).enumerate() {
                    assert_eq!(
                        xf2.model(&mut iso, body),
                        format!("{size}{}Body", v + 1),
                        "{who}"
                    );
                    assert!(xf2.is_morph(&mut iso, morph), "{who}: entry {morph}");
                }
                for sheet in [m.road_sheet, m.home_sheet] {
                    assert!(xf2.sheet(&mut iso, sheet).contains(&"jers".into()), "{who}");
                }
                for a in &m.accessories {
                    let Some(model) = a.model else { continue };
                    let name = xf2.model(&mut iso, model);
                    let (kind, side) = a.name.split_at(4);
                    let size = ["sml", "med", "lrg"][p.build as usize];
                    assert_eq!(name, format!("{kind}{size}{side}"), "{who}");
                }
            }
            PlayerModel::Unique(m) => {
                let name = xf2.model(&mut iso, m.model);
                for sheet in [m.road_sheet, m.home_sheet] {
                    assert!(!xf2.sheet(&mut iso, sheet).is_empty(), "{who}");
                }
                unique_names.insert(who, name);
            }
            PlayerModel::Created(m) => {
                assert_eq!(p.kind(), PlayerKind::Created);
                let body = xf1.model(&mut iso, m.body);
                assert!(
                    body.starts_with(&format!("body{}_outfit", p.build)),
                    "{who}: {body}"
                );
                let head = xf1.model(&mut iso, m.head.unwrap());
                let hair = if p.female { "femalehair" } else { "malehair" };
                assert!(head.starts_with(hair), "{who}: {head}");
                assert!(
                    head.ends_with(&format!("_body{}", p.build)),
                    "{who}: {head}"
                );
                for morph in [m.face_morph, m.shoe_morph]
                    .into_iter()
                    .chain(m.beard_morph)
                {
                    assert!(xf1.is_morph(&mut iso, morph), "{who}: entry {morph}");
                }
                let skin = xf1.sheet(&mut iso, m.skin_sheet);
                assert!(skin.contains(&"head".into()), "{who}: {skin:?}");
                for sheet in [m.shoe_sheet, m.bottom_sheet]
                    .into_iter()
                    .chain(m.hair_sheet)
                    .chain(m.beard_sheet)
                    .chain(m.top_sheet)
                {
                    assert!(!xf1.sheet(&mut iso, sheet).is_empty(), "{who}");
                }
            }
        }
    }
    for (who, model) in [
        ("Stretch", "strc"),
        ("Murphy Lee", "lmur"),
        ("Julius Erving", "jerv"),
        ("Oscar Robertson", "orob"),
        ("Tyson", "male01"),
        ("Natalie", "female12"),
    ] {
        assert_eq!(unique_names[who], model);
    }
}
