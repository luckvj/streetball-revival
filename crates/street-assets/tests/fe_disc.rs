//! Checks of the front-end art decoder against a real the original game
//! (USA) disc. Skipped unless STREET_ISO points at the image, since the repo
//! ships no game data.

use std::{fs::File, io::BufReader};

use street_assets::{fe, iso::Iso};

type Disc = Iso<BufReader<File>>;

fn disc() -> Option<Disc> {
    let path = std::env::var_os("STREET_ISO")?;
    Some(Iso::open(path).expect("STREET_ISO is not a readable ISO9660 image"))
}

/// The native 0x7c header is eight bytes; retain every corner, including
/// the first (which a generic sixteen-byte metadata header would lose).
#[test]
fn common_sprite_frames_match_native_star_tab_and_bar_coordinates() {
    let Some(mut iso) = disc() else { return };
    let common = fe::read(&mut iso, "/FE/XACOMMON.BIN").unwrap();
    let sheet = &common[0];
    assert_eq!(sheet.frames.len(), sheet.textures.len());
    assert_eq!(
        sheet.frames[1][0].corners,
        [[2, 1], [21, 1], [21, 19], [2, 19]]
    );
    assert_eq!(sheet.frames[2][0].bounds(), [0, 0, 63, 18]);
    assert_eq!(sheet.frames[5][0].bounds(), [2, 20, 23, 21]);
    assert_eq!(sheet.frames[5][2].bounds(), [3, 56, 27, 25]);
    assert_eq!(sheet.frames[6].len(), 10);
    for frame in &sheet.frames[6] {
        assert!(frame.corners.iter().all(|p| p[0] >= 0
            && p[1] >= 0
            && p[0] <= sheet.textures[6].width as i32
            && p[1] <= sheet.textures[6].height as i32));
    }
}

#[test]
fn native_menu_bar_frames_and_runtime_widget_kinds_match_the_disc() {
    let Some(mut iso) = disc() else { return };
    let layouts = street_assets::font::layouts(&mut iso, "/FE/XLAY_FE.BIN").unwrap();
    assert_eq!(layouts.len(), 61);
    assert!(
        layouts
            .iter()
            .flat_map(|layout| &layout.elements)
            .all(|element| !matches!(element.kind, 31 | 36))
    );
    let common = fe::read(&mut iso, "/FE/XACOMMON.BIN").unwrap();
    let segmented = &common[0].frames[6];
    assert_eq!(segmented[6].bounds(), [343, 4, 11, 25]);
    assert_eq!(segmented[7].bounds(), [357, 4, 10, 25]);

    // kind 6's native slider, FUN_002370c0: common sheet 2, image 0,
    // frames 4 (track), 0/1/0 (moving markers; the last is mirrored).
    let slider = &common[2].frames[0];
    assert_eq!(slider[0].bounds(), [7, 7, 4, 11]);
    assert_eq!(slider[1].bounds(), [14, 7, 2, 11]);
    assert_eq!(slider[4].bounds(), [3, 44, 176, 20]);
}

#[test]
fn native_screen_row_registration_element_ids_match_disc_layouts() {
    let Some(mut iso) = disc() else { return };
    let layouts = street_assets::font::layouts(&mut iso, "/FE/XLAY_FE.BIN").unwrap();

    // Main Menu (class 3): list 19 registers templates 0, 1, 23, 21, 26.
    let main = layouts.iter().find(|l| l.class == 3).unwrap();
    let main_ids: Vec<u32> = main
        .elements
        .iter()
        .filter(|e| e.kind == 19)
        .map(|e| e.id)
        .collect();
    assert!(main_ids.contains(&0));
    assert!(main_ids.contains(&1));
    assert!(main_ids.contains(&23));
    assert!(main_ids.contains(&21));
    assert!(main_ids.contains(&26));

    // Play It (class 5): list 0 registers elements 15, 17, 18, 16.
    let playit = layouts.iter().find(|l| l.class == 5).unwrap();
    let playit_ids: Vec<u32> = playit
        .elements
        .iter()
        .filter(|e| e.kind == 19)
        .map(|e| e.id)
        .collect();
    assert_eq!(playit_ids, vec![16, 18, 17, 15]); // Disc layout order; registered order is 15, 17, 18, 16

    // My Team (class 6): list 5 registers elements 1, 2.
    let myteam = layouts.iter().find(|l| l.class == 6).unwrap();
    let myteam_ids: Vec<u32> = myteam
        .elements
        .iter()
        .filter(|e| e.kind == 19)
        .map(|e| e.id)
        .collect();
    assert_eq!(myteam_ids, vec![2, 1]); // Disc layout order; registered order is 1, 2

    // Inside Vol. 2 (class 102): list 0 registers elements 15, 17, 19, 22, 18.
    let inside = layouts.iter().find(|l| l.class == 102).unwrap();
    let inside_ids: Vec<u32> = inside
        .elements
        .iter()
        .filter(|e| e.kind == 19)
        .map(|e| e.id)
        .collect();
    assert_eq!(inside_ids, vec![22, 19, 18, 17, 15]);

    // What's New (class 103): list 0 registers elements 22, 16, 18, 19, 21.
    let whats_new = layouts.iter().find(|l| l.class == 103).unwrap();
    let whats_new_ids: Vec<u32> = whats_new
        .elements
        .iter()
        .filter(|e| e.kind == 19)
        .map(|e| e.id)
        .collect();
    assert_eq!(whats_new_ids, vec![22, 19, 18, 16, 21]);

    // Bobbito (class 104): list 0 registers elements 15, 16.
    let bobbito = layouts.iter().find(|l| l.class == 104).unwrap();
    let bobbito_ids: Vec<u32> = bobbito
        .elements
        .iter()
        .filter(|e| e.kind == 19)
        .map(|e| e.id)
        .collect();
    assert_eq!(bobbito_ids, vec![16, 15]);
}

/// The background archive: 105 entries (TRACE.md §4), the front-end strips
/// in 24-bit RGB; every entry decodes.
#[test]
fn the_backgrounds_decode() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let bgs = fe::read(&mut iso, "/FE/XAFEBG.BIN").unwrap();
    assert_eq!(bgs.len(), 105);
    for (k, sheet) in bgs.iter().enumerate() {
        assert_eq!(sheet.textures.len(), 1, "entry {k}");
        let t = &sheet.textures[0];
        // The strips: 128- or 64-wide rows, 256 tall; the whole pixel body
        // must be present (the 24-bit kind is 3 bytes a pixel).
        assert_eq!(t.height, 256, "entry {k}");
        assert!(
            t.width == 128 || t.width == 64 || t.width == 512,
            "entry {k}"
        );
        assert_eq!(t.rgba.len() as u32, t.width * t.height * 4, "entry {k}");
    }
}

/// The team logos: 50 entries of one logo each, and every league team's
/// `logo` field points at its own entry.
#[test]
fn the_team_logos_decode() {
    let Some(mut iso) = disc() else { return };
    let logos = fe::read(&mut iso, "/FE/XATEAMS.BIN").unwrap();
    assert_eq!(logos.len(), 50);
    let teams = street_assets::db::teams(&mut iso).unwrap();
    for team in teams.iter().take(30) {
        let entry = usize::try_from(team.logo).unwrap();
        assert!(
            !logos[entry].textures.is_empty(),
            "team {} logo {entry}",
            team.full_name()
        );
    }
}

/// The plain SHPS files: the HUD atlas and the off-screen indicator.
#[test]
fn the_hud_atlas_decodes() {
    let Some(mut iso) = disc() else { return };
    let hud = fe::sheet(&mut iso, "/FE/HUD.SSH").unwrap();
    assert!(!hud.textures.is_empty());
    let indicator = fe::sheet(&mut iso, "/FE/OFFIND.SSH").unwrap();
    assert!(!indicator.textures.is_empty());
}

/// The pre-match screens' layouts and sprites as their constructors read
/// them (`docs/FRONTEND_RECOVERY.md`, "Difficulty", "User Setup" and
/// "Choose Team"): the elements the code binds, where the layouts put
/// them, and the authored frames of the pieces built in code.
#[test]
#[allow(clippy::type_complexity)]
fn the_prematch_layouts_and_sprites_hold_the_traced_pieces() {
    let Some(mut iso) = disc() else { return };
    let layouts = street_assets::font::layouts(&mut iso, "/FE/XLAY_FE.BIN").unwrap();
    let at = |e: &street_assets::font::Element| e.position().unwrap();

    // Difficulty (class 105): list 0's three nodes down the middle, the
    // rows 17, 16, 15 (`difficulty.txt` 3, 2, 1), the Back and Help
    // legends.
    let difficulty = layouts.iter().find(|l| l.class == 105).unwrap();
    let mut nodes: Vec<(u32, (u16, u16))> = difficulty
        .elements
        .iter()
        .filter(|e| e.kind == 10 && e.props.get(&33) == Some(&0))
        .map(|e| (e.props[&34], at(e)))
        .collect();
    nodes.sort();
    assert_eq!(
        nodes,
        vec![(0, (312, 142)), (1, (312, 181)), (2, (312, 220))]
    );
    for (id, text) in [(17, 0x4803), (16, 0x4802), (15, 0x4801)] {
        let row = difficulty
            .elements
            .iter()
            .find(|e| e.kind == 19 && e.id == id)
            .unwrap();
        assert_eq!(row.props[&16], text);
        assert_eq!(row.props[&13], 3);
    }
    let legends: Vec<(u16, u16)> = difficulty
        .elements
        .iter()
        .filter(|e| e.kind == 34)
        .map(at)
        .collect();
    assert!(legends.contains(&(540, 400)) && legends.contains(&(555, 67)));
    assert_eq!(
        at(difficulty.elements.iter().find(|e| e.kind == 5).unwrap()),
        (58, 420)
    );

    // User Setup (class 11): the four column widgets, the title, "vs" and
    // the help line.
    let users = layouts.iter().find(|l| l.class == 11).unwrap();
    let mut columns: Vec<(u32, (u16, u16), (u16, u16))> = users
        .elements
        .iter()
        .filter(|e| e.kind == 13)
        .map(|e| (e.props[&49], at(e), e.size().unwrap()))
        .collect();
    columns.sort();
    assert_eq!(
        columns,
        vec![
            (0, (57, 113), (129, 276)),
            (1, (189, 113), (129, 276)),
            (2, (321, 113), (129, 276)),
            (3, (453, 113), (129, 276)),
        ]
    );
    let title = users.elements.iter().find(|e| e.kind == 35).unwrap();
    assert_eq!((at(title), title.props[&15]), ((56, 58), 0x800));
    let vs = users.elements.iter().find(|e| e.kind == 4).unwrap();
    assert_eq!(
        (at(vs), vs.props[&15], vs.props[&13]),
        ((329, 154), 0x801, 5)
    );
    assert_eq!(
        at(users.elements.iter().find(|e| e.kind == 5).unwrap()),
        (58, 424)
    );

    // Choose Team (class 26): two lists of eight nodes, four across, the
    // large logos, names, meters, Ready texts, headings and frame lines.
    let team = layouts.iter().find(|l| l.class == 26).unwrap();
    for (list, xs) in [(0u32, [110u16, 165, 220, 275]), (33, [362, 417, 472, 527])] {
        let mut nodes: Vec<(u32, (u16, u16))> = team
            .elements
            .iter()
            .filter(|e| e.kind == 10 && e.props.get(&33) == Some(&list))
            .map(|e| (e.props[&34], at(e)))
            .collect();
        nodes.sort();
        let want: Vec<(u32, (u16, u16))> = (0..8)
            .map(|k| (k as u32, (xs[k % 4], if k < 4 { 321 } else { 373 })))
            .collect();
        assert_eq!(nodes, want);
    }
    let find = |kind: u8, id: u32| {
        team.elements
            .iter()
            .find(|e| e.kind == kind && e.id == id)
            .unwrap()
    };
    assert_eq!(
        (at(find(24, 14)), at(find(24, 47))),
        ((183, 149), (454, 149))
    );
    assert_eq!((at(find(4, 10)), at(find(4, 43))), ((183, 196), (454, 196)));
    for (id, x) in [
        (60, 70),
        (12, 187),
        (61, 237),
        (63, 341),
        (45, 458),
        (64, 508),
    ] {
        let meter = find(20, id);
        assert_eq!((at(meter), meter.size().unwrap()), ((x, 231), (60, 25)));
    }
    assert_eq!((at(find(4, 20)), at(find(4, 53))), ((192, 345), (443, 345)));
    assert_eq!((at(find(3, 25)), at(find(3, 31))), ((218, 74), (323, 74)));
    assert_eq!((at(find(3, 26)), at(find(3, 32))), ((52, 396), (323, 396)));
    assert_eq!(
        (find(4, 65).props[&14], find(4, 66).props[&14]),
        (0xa00, 0xa01)
    );
    let lines: Vec<(u32, (u16, u16), (u16, u16), u32)> = [21, 22, 23, 24, 27, 28, 29, 30]
        .iter()
        .map(|&id| {
            let e = find(32, id);
            (id, at(e), e.size().unwrap(), e.props[&8])
        })
        .collect();
    assert_eq!(
        lines,
        vec![
            (21, (53, 93), (4, 306), 0),
            (22, (53, 93), (262, 4), 0),
            (23, (311, 93), (4, 324), 0),
            (24, (72, 413), (240, 4), 0),
            (27, (324, 93), (4, 306), 2),
            (28, (324, 93), (261, 4), 2),
            (29, (581, 93), (4, 324), 2),
            (30, (342, 413), (240, 4), 2),
        ]
    );
    // The scroll arrows' places, taken from elements 19/18 and 52/51
    // before `FUN_0026eb08` deletes them.
    assert_eq!((at(find(3, 19)), at(find(3, 18))), ((69, 304), (69, 387)));
    assert_eq!((at(find(3, 52)), at(find(3, 51))), ((567, 304), (567, 387)));

    // The sprites the screens build in code, by their authored frames.
    let bounds = |sheet: &street_assets::ssh::Sheet, image: usize, frame: usize| {
        let [_, _, w, h] = sheet.frames[image][frame].bounds();
        (w, h)
    };
    let common = fe::read(&mut iso, "/FE/XACOMMON.BIN").unwrap();
    assert_eq!(bounds(&common[0], 0, 0), (20, 15), "the arrow");
    assert_eq!(bounds(&common[0], 2, 2), (4, 16), "the line piece");
    assert_eq!(bounds(&common[0], 2, 3), (22, 21), "the corner piece");
    assert_eq!(bounds(&common[0], 2, 6), (114, 20), "the heading tab");
    assert_eq!(bounds(&common[1], 2, 0), (48, 37), "the controller icon");
    let screens = fe::read(&mut iso, "/FE/XAFESCRN.BIN").unwrap();
    assert_eq!(bounds(&screens[0], 0, 0), (111, 54), "the wide cursor");
    assert_eq!(bounds(&screens[0], 0, 1), (55, 53), "the tile cursor");
    assert_eq!(bounds(&screens[0], 7, 33), (19, 19), "the CPU badge");
    assert_eq!(bounds(&screens[1], 0, 0), (54, 17), "the meter's frame");
    assert_eq!(bounds(&screens[1], 0, 10), (51, 8), "the meter's fill");
    let teams = fe::read(&mut iso, "/FE/XATEAMS.BIN").unwrap();
    assert_eq!(bounds(&teams[0], 1, 0), (55, 53), "a team's tile");
    assert_eq!(
        bounds(&teams[40], 0, 1),
        (111, 55),
        "All-Stars' double tile"
    );
    assert_eq!(
        bounds(&teams[39], 0, 1),
        (111, 55),
        "Old School's double tile"
    );
    assert_eq!(
        bounds(&teams[39], 0, 0),
        (214, 97),
        "Old School's large logo"
    );
}

/// Choose Court (class 33) and Set Challenge (class 39) as their
/// constructors read them (`FUN_00267f10`, `FUN_002c8d08`), and the sheets
/// they pick: the court sheets' two frames, the cursor, the arrows and the
/// palettes of `XPAL_FE`.
#[test]
fn the_court_and_rules_layouts_and_sheets_hold_the_traced_pieces() {
    let Some(mut iso) = disc() else { return };
    let layouts = street_assets::font::layouts(&mut iso, "/FE/XLAY_FE.BIN").unwrap();
    let at = |e: &street_assets::font::Element| e.position().unwrap();

    // Choose Court: list 97's twelve nodes by prop 34, the name and
    // location texts, the help, the stars' elements, the title.
    let court = layouts.iter().find(|l| l.class == 33).unwrap();
    let mut nodes: Vec<(u32, (u16, u16))> = court
        .elements
        .iter()
        .filter(|e| e.kind == 10 && e.props.get(&33) == Some(&97))
        .map(|e| (e.props[&34], at(e)))
        .collect();
    nodes.sort();
    assert_eq!(
        nodes,
        vec![
            (0, (230, 98)),
            (1, (147, 98)),
            (2, (66, 254)),
            (3, (66, 176)),
            (4, (147, 176)),
            (5, (230, 176)),
            (6, (66, 98)),
            (7, (147, 254)),
            (8, (230, 254)),
            (9, (66, 332)),
            (10, (147, 332)),
            (11, (230, 332)),
        ]
    );
    // Node 90 (prop 34 = 6, the top-left tile) is where the list opens;
    // its links: up to 93, down to 87, left to 95, right to 85.
    let first = court
        .elements
        .iter()
        .find(|e| e.kind == 10 && e.id == 90)
        .unwrap();
    assert_eq!(first.props[&34], 6);
    assert_eq!(
        (
            first.props[&26],
            first.props[&28],
            first.props[&30],
            first.props[&32]
        ),
        (93, 87, 95, 85)
    );
    let find = |layout: &street_assets::font::Layout, kind: u8, id: u32| {
        layout
            .elements
            .iter()
            .find(|e| e.kind == kind && e.id == id)
            .unwrap()
            .clone()
    };
    let name = find(court, 4, 0x62);
    assert_eq!((at(&name), name.props[&13], name.align), ((338, 107), 2, 4));
    let location = find(court, 4, 0x60);
    assert_eq!(
        (at(&location), location.props[&13], location.props[&10]),
        ((338, 128), 1, 0xff7c83d4)
    );
    assert_eq!(
        (at(&find(court, 3, 0x51)), at(&find(court, 3, 0x52))),
        ((374, 106), (454, 106))
    );
    assert_eq!(at(&find(court, 5, 0x50)), (58, 420));
    assert_eq!(
        (at(&find(court, 35, 0x53)), find(court, 35, 0x53).props[&15]),
        ((58, 46), 0xd00)
    );
    let list = find(court, 9, 0x61);
    assert_eq!((at(&list), list.size().unwrap()), ((60, 97), (249, 321)));

    // Set Challenge: list 38's five nodes, the rows the build registers
    // (37, 39, 41, 40, 36) with their texts, the help and the title.
    let rules = layouts.iter().find(|l| l.class == 39).unwrap();
    let mut nodes: Vec<(u32, (u16, u16))> = rules
        .elements
        .iter()
        .filter(|e| e.kind == 10 && e.props.get(&33) == Some(&38))
        .map(|e| (e.props[&34], at(e)))
        .collect();
    nodes.sort();
    assert_eq!(
        nodes,
        vec![
            (0, (151, 104)),
            (1, (151, 169)),
            (2, (151, 243)),
            (3, (151, 315)),
            (4, (395, 374)),
        ]
    );
    for (id, text, align) in [
        (37, 0xe01, 1),
        (39, 0xe02, 1),
        (41, 0xe03, 1),
        (40, 0xe04, 1),
        (36, 0xe24, 2),
    ] {
        let row = find(rules, 19, id);
        assert_eq!(
            (row.props[&14], row.props[&13], row.align & 3),
            (text, 3, align)
        );
    }
    // The last node (Play Game) links down to the first and up to the
    // Shot Clock node.
    let last = find(rules, 10, 34);
    assert_eq!((last.props[&26], last.props[&28]), (33, 30));
    assert_eq!(at(&find(rules, 5, 0x23)), (58, 422));
    assert_eq!(find(rules, 35, 0x2a).props[&15], 0xe00);

    // The court sheets: 14 entries, each one image with the big picture
    // (frame 0, 248x174) and the tile (frame 1, 81x77).
    let courts = street_assets::fe::read(&mut iso, "/FE/XACOURTS.BIN").unwrap();
    assert_eq!(courts.len(), 14);
    for (k, sheet) in courts.iter().enumerate() {
        let frames = &sheet.frames[0];
        // The Random sheet (12) authors the tile alone (`FUN_00267f10`
        // shows its frame 0 on widget 11; the courts' tiles are frame 1).
        if k == 12 {
            assert_eq!(frames.len(), 1);
            assert_eq!(frames[0].bounds(), [2, 177, 81, 77]);
        } else {
            assert_eq!(frames[0].bounds(), [0, 0, 248, 174], "sheet {k}");
            assert_eq!(frames[1].bounds(), [2, 177, 81, 77], "sheet {k}");
        }
    }
    // The cursor tag's frame (`FUN_0021b550` case 2: `XAFESCRN` sheet 0,
    // image 0, frame 3) is the tile's size less a pixel across.
    let screens = street_assets::fe::read(&mut iso, "/FE/XAFESCRN.BIN").unwrap();
    assert_eq!(screens[0].frames[0][3].bounds(), [172, 2, 80, 77]);
    // The arrows (`XACOMMON` sheet 0, image 0): frame 1 the left-pointing
    // one Set Challenge's element 0x45f shows; the name stars' frame
    // (image 1, frame 2).
    let common = street_assets::fe::read(&mut iso, "/FE/XACOMMON.BIN").unwrap();
    assert_eq!(common[0].frames[0][1].bounds(), [0, 0, 17, 20]);
    assert_eq!(common[0].frames[1][2].bounds(), [2, 19, 19, 20]);
    // The palettes: sheet 0 the four controllers', sheet 1 the meter's
    // five levels (`0x467180`), each 256 entries.
    let palettes = street_assets::fe::read(&mut iso, "/FE/XPAL_FE.BIN").unwrap();
    assert_eq!(palettes[0].palettes.len(), 4);
    assert_eq!(palettes[1].palettes.len(), 5);
    assert!(
        palettes
            .iter()
            .flat_map(|s| &s.palettes)
            .all(|p| p.len() == 256)
    );
    // The meter fill (sheet 1, image 0, frame 10) is palettised, so the
    // level palettes recolour it, and the five differ from one another.
    let fill = &screens[1].textures[0];
    assert!(fill.indices.is_some());
    let lit: Vec<Vec<u8>> = palettes[1]
        .palettes
        .iter()
        .map(|p| fill.in_palette(p).rgba)
        .collect();
    for k in 1..5 {
        assert_ne!(lit[k], lit[k - 1]);
    }
}
