//! Checks of the locale (`c0 fb` archive) decoder against a real the original game
//! Vol. 2 (USA) disc. Skipped unless STREET_ISO points at the image, since
//! the repo ships no game data.

use std::{fs::File, io::BufReader};

use street_assets::{iso::Iso, locale};

type Disc = Iso<BufReader<File>>;

fn disc() -> Option<Disc> {
    let path = std::env::var_os("STREET_ISO")?;
    Some(Iso::open(path).expect("STREET_ISO is not a readable ISO9660 image"))
}

/// The front-end and in-game text decode; `overlays.txt` and `school.txt`
/// come from `XLOC_GM` (see `docs/TRACE.md` §4).
#[test]
fn the_locale_text_decodes() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let fe = locale::read(&mut iso, "/LOCALE/XLOC_FE.EUS").unwrap();
    let gm = locale::read(&mut iso, "/LOCALE/XLOC_GM.EUS").unwrap();
    assert_eq!(fe.first().map(|f| f.name.as_str()), Some("common.txt"));
    let overlays = gm.iter().find(|f| f.name == "overlays.txt").unwrap();
    assert_eq!(overlays.texts.len(), 58);
    // #55 is the pocket hint, and it names the button by itself.
    assert!(overlays.texts[55].contains("SELECT"));
    // The in-game text carries the school lessons (TRACE.md §5: #0..158):
    let school = gm.iter().find(|f| f.name == "school.txt").unwrap();
    assert!(school.texts.len() > 158);
    // Punctuation arrives as \xNN text and must not remain escaped.
    for f in fe.iter().chain(gm.iter()) {
        assert!(
            f.texts.iter().all(|s| !s.contains("\\x")),
            "left an escape in {}",
            f.name
        );
    }
}

/// A text id `file << 8 | index` names a file and a string in it: `fe` leads
/// with `common.txt`; overlays' #2 is the HUD's turbo word (TRACE.md §2).
#[test]
fn text_ids_resolve_to_the_goods() {
    let Some(mut iso) = disc() else { return };
    let fe = locale::read(&mut iso, "/LOCALE/XLOC_FE.EUS").unwrap();
    assert_eq!(fe[0].name, "common.txt");
    let gm = locale::read(&mut iso, "/LOCALE/XLOC_GM.EUS").unwrap();
    let over = gm.iter().find(|f| f.name == "overlays.txt").unwrap();
    assert_eq!(over.texts[2], "turbo");
    assert_eq!(over.texts[28], "hookup");
}
