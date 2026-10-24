//! HD texture packs for the GameCube Vol. 2 (`GNZE69`) against the user's own
//! discs and pack. Skipped unless STREET_GC_VOL2 names the GameCube Vol. 2
//! image (`.ciso`, `.rvz`, `.iso`, ...), STREET_ISO the PS2 Vol. 2 image and,
//! for the pack test, STREET_HIRES the pack's `Textures/GNZE69` folder (the
//! Legendary Edition's).

use std::collections::HashSet;

use street_assets::{big, gcdisc::GcDisc, hires, iso::Iso, player, ssh};

fn gc() -> Option<GcDisc> {
    let path = std::env::var_os("STREET_GC_VOL2")?;
    Some(GcDisc::open(path).expect("STREET_GC_VOL2 is not a readable GameCube disc"))
}

fn ps2() -> Option<Iso<std::io::BufReader<std::fs::File>>> {
    let path = std::env::var_os("STREET_ISO")?;
    Some(Iso::open(path).expect("STREET_ISO is not a readable ISO9660 image"))
}

/// The key of image `image` of the sheet at `entry` of `file`.
fn key(disc: &mut GcDisc, file: &str, entry: usize, image: usize) -> String {
    let index = disc.big_index(file).ok();
    let sheet = match index {
        Some(index) => disc.big_entry(file, &index[entry]).unwrap(),
        None => {
            let data = disc.read(file).unwrap();
            hires::sheets_in(&data)
                .into_iter()
                .find(|s| s.entry == Some(entry))
                .unwrap()
                .data
        }
    };
    let images = hires::sheet_keys(&sheet).unwrap();
    images[image].key.unwrap().to_string()
}

/// Dolphin's names for the disc's textures, as the Legendary Edition's
/// files are named: a CMPR face (one hash) and a C8 logo (texture and
/// palette range hashes).
#[test]
fn gamecube_textures_hash_to_the_names_dolphin_gives_them() {
    let Some(mut disc) = gc() else {
        eprintln!("STREET_GC_VOL2 not set; skipping");
        return;
    };
    assert_eq!(disc.game_id(), "GNZE69");
    // Player 132's road sheet (entry 5 * 132 + 3), image 1: the face.
    assert_eq!(
        key(&mut disc, "/xf/xf000002", 663, 1),
        "256x128_8569d0615b29fd73_14"
    );
    // Atlanta's logo sheet (entry 0), image 1: the 64x64 tile.
    assert_eq!(
        key(&mut disc, "/fe/xateams.bin", 0, 1),
        "64x64_1a57e4e44a0c18f5_f3edbb798cac696c_9"
    );
}

/// The built-in table comes back from the two discs.
#[test]
fn the_table_is_made_from_the_discs() {
    let (Some(mut disc), Some(mut iso)) = (gc(), ps2()) else {
        eprintln!("STREET_GC_VOL2 or STREET_ISO not set; skipping");
        return;
    };
    let shipped = hires::parse_table(hires::GNZE69_TABLE).unwrap();
    let keys: HashSet<hires::Key> = shipped.iter().map(|r| r.key).collect();
    let (mut rows, _) = hires::build_table(&mut disc, &mut iso, Some(&keys)).unwrap();
    rows.sort_by(|a, b| (&a.ps2, a.key).cmp(&(&b.ps2, b.key)));
    // Scores are written rounded.
    let rows = hires::parse_table(&hires::write_table(&rows)).unwrap();
    assert_eq!(rows.len(), shipped.len());
    assert!(rows == shipped);
}

/// With the Legendary Edition pack, player 132's face, jersey and shoes are
/// the pack's.
#[test]
fn a_pack_dresses_a_player_in_hd() {
    let (Some(mut iso), Some(dir)) = (ps2(), std::env::var_os("STREET_HIRES")) else {
        eprintln!("STREET_ISO or STREET_HIRES not set; skipping");
        return;
    };
    let pack = hires::Pack::open(&dir).unwrap();
    assert!(pack.used().len() > 600, "{} used", pack.used().len());
    let textures = player::player_textures(&mut iso, 132).unwrap();
    for slot in ["head", "jers", "shoe"] {
        let t = &textures[slot];
        let hd = pack
            .replace(t)
            .unwrap_or_else(|| panic!("no HD image for {slot}"));
        // The face is a new picture at the same size, the rest larger.
        assert!(hd.width >= t.width && hd.height >= t.height, "{slot}");
        assert!(hd.width > t.width || hd.rgba != t.rgba, "{slot}");
        assert_eq!(
            hd.width * t.height,
            hd.height * t.width,
            "{slot} proportions"
        );
        assert_eq!(hd.rgba.len(), (hd.width * hd.height * 4) as usize);
    }
    // A loading screen comes back whole: the GameCube's 320x256 or larger
    // (most of the pack's are new pictures at the GameCube's size).
    let pic = pack.picture(12).expect("loading screen 12");
    assert_eq!(pic.width * 256, pic.height * 320);
    assert!(pic.width >= 320);
    // Against the PS2's own: picture 5 is a new 640x512 one (Kobe Bryant),
    // 12 the GameCube's half-size Elgin Baylor card, kept from the disc.
    for (k, used) in [(5, true), (12, false)] {
        let original = ps2_picture(&mut iso, k);
        assert_eq!(pack.picture_over(k, &original).is_some(), used, "{k}");
    }
}

/// Loading screen `k` as the PS2 has it: ten 128x256 tiles, 5x2.
fn ps2_picture(iso: &mut Iso<std::io::BufReader<std::fs::File>>, k: usize) -> ssh::Texture {
    let index = big::read_index(iso, "/FE/XAFELOAD.BIN").unwrap();
    let sheet = ssh::parse(&big::read_entry(iso, "/FE/XAFELOAD.BIN", &index[k]).unwrap()).unwrap();
    let mut rgba = vec![0u8; 640 * 512 * 4];
    for (i, t) in sheet.textures.iter().take(10).enumerate() {
        let (x0, y0) = ((i % 5) * 128, (i / 5) * 256);
        for y in 0..256 {
            let from = y * 128 * 4;
            let to = ((y0 + y) * 640 + x0) * 4;
            rgba[to..to + 128 * 4].copy_from_slice(&t.rgba[from..from + 128 * 4]);
        }
    }
    ssh::Texture {
        name: String::new(),
        width: 640,
        height: 512,
        rgba,
        indices: None,
    }
}
