//! The "Legendary Edition" GameCube roster mod read over a Vol. 2 (USA) disc.
//! Skipped unless STREET_ISO points at the PS2 image and STREET_ROSTER_DB at
//! the mod's folder (or its `ISO Files/database`).
//!
//! The roster override is process-wide, so this binary holds a single test.

use std::path::PathBuf;

use street_assets::{db, iso::Iso};

#[test]
fn legendary_roster_replaces_names_ratings_and_teams() {
    let (Some(iso), Some(dir)) = (
        std::env::var_os("STREET_ISO"),
        std::env::var_os("STREET_ROSTER_DB"),
    ) else {
        eprintln!("STREET_ISO or STREET_ROSTER_DB not set; skipping");
        return;
    };
    let mut iso = Iso::open(iso).expect("STREET_ISO is not a readable ISO9660 image");
    let dir = db::find_roster_dir(&PathBuf::from(dir)).expect("no xdbplyr.adf in STREET_ROSTER_DB");

    let original = db::players(&mut iso).unwrap();
    db::set_roster_override(Some(&dir)).unwrap();
    let players = db::players(&mut iso).unwrap();
    let teams = db::teams(&mut iso).unwrap();
    db::set_roster_override(None).unwrap();

    assert_eq!(players.len(), original.len());
    assert_eq!(players[0].name(), "Michael Redd");
    assert_eq!(players[132].name(), "LeBron James");
    assert_ne!(players[132].ratings, original[132].ratings);
    // Records the mod left alone keep the disc's names, not the fragments
    // their stale string offsets point at.
    for i in [145, 146, 147, 148, 149, 184] {
        assert_eq!(players[i].name(), original[i].name(), "record {i}");
    }
    assert_eq!(players[150].name(), "Osmosis");
    assert_eq!(teams[16].full_name(), "Brooklyn Nets");
    assert_eq!(teams[25].full_name(), "Oklahoma City Thunder");
    // Every league team lists five players. The mod did not keep each player's
    // own team field in step with the lists (Jalen Johnson is listed by
    // Atlanta but says Boston), and Atlanta also lists record 231, a generic
    // baller's slot turned into Gianna Bryant.
    let mut out_of_step = 0;
    for t in &teams[..29] {
        assert_eq!(t.roster.len(), 5, "{}", t.full_name());
        for &p in &t.roster {
            assert!(p < players.len(), "{} lists record {p}", t.full_name());
            out_of_step += usize::from(players[p].team as usize != t.index);
        }
    }
    assert!(
        out_of_step < 20,
        "{out_of_step} listed players name another team"
    );
    // Back on the disc's own tables afterwards.
    assert_eq!(
        db::players(&mut iso).unwrap()[0].name(),
        "Shareef Abdur-Rahim"
    );
}
