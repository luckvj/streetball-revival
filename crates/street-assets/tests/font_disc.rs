//! The front end's fonts and layouts against a real disc. Skipped unless
//! STREET_ISO is set.

use std::{fs::File, io::BufReader};

use street_assets::{font, iso::Iso};

fn disc() -> Option<Iso<BufReader<File>>> {
    let path = std::env::var_os("STREET_ISO")?;
    Some(Iso::open(path).expect("STREET_ISO is not a readable ISO9660 image"))
}

/// All twelve English fonts read and draw; eleven carry the alphabet, the
/// last only the score digits and signs.
#[test]
fn the_fonts_draw_text() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let fonts = font::fonts(&mut iso).unwrap();
    assert_eq!(fonts.len(), 12);
    for (i, f) in fonts.iter().enumerate() {
        let text = if i == 11 { "+21" } else { "PLAY IT" };
        assert!(
            text.chars().all(|c| c == ' ' || f.glyphs.contains_key(&c)),
            "font {i}"
        );
        let r = f.render(text, [255; 4], [255; 4]);
        let ink = r.rgba.chunks(4).filter(|p| p[3] > 128).count();
        assert!(ink > 20, "font {i} drew {ink} pixels");
    }
}

/// The 61 front-end layouts each declare one screen; the main menu's is
/// class 3 with its title in font 6 and entries in font 4.
#[test]
fn the_layouts_read() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let fe = font::layouts(&mut iso, "/FE/XLAY_FE.BIN").unwrap();
    assert_eq!(fe.len(), 61);
    assert!(
        fe.iter()
            .all(|l| l.elements.iter().filter(|e| e.kind == 0).count() == 1)
    );
    let main = &fe[0];
    assert_eq!(main.class, 3);
    let title = main.elements.iter().find(|e| e.kind == 35).unwrap();
    assert_eq!(title.props.get(&13), Some(&6));
    assert_eq!(title.texts(), vec![0x300]);
    let entries: Vec<_> = main.elements.iter().filter(|e| e.kind == 19).collect();
    assert_eq!(entries.len(), 5);
    assert!(entries.iter().all(|e| e.props.get(&13) == Some(&4)));
    assert_eq!(
        font::layouts(&mut iso, "/FE/XLAY_GM.BIN").unwrap().len(),
        12
    );
}
