//! Create A Baller: making and editing your own players.
//!
//! A baller replaces one of the six create-a-player records (team 37,
//! records 234..239) with the appearance and skills chosen here; the
//! game's kit (`/XF/XF000001`) dresses it the way the game's appearance
//! object does (`street_assets::player::look`). Ballers live in the save
//! (`street_sim::story::Story::ballers`) and are written over the Disc's
//! records at start and on every save, so every match and screen sees them.
//!
//! The editor offers what the kit holds for the build (faces, hair styles,
//! facial hair: `street_assets::db::kit_limits`) and the names the game's
//! own RANDOM button picks from (`random_first.txt`,
//! `random_first_female.txt`, `random_last.txt` in `XLOC_FE`). The height
//! ranges and prices come from the build tables. Skills use the original
//! seven-bar rating ladders and 100..600 DP step prices; each baller has
//! its own balance.
//!
//! Moves: the named shots, dunks and tricks (`XDBMOVE` kinds 0, 1, 2), bought
//! with the save's development points at the table's prices (50, 125 and
//! 250 by level), up to six shots, eleven dunks and eleven tricks
//! (`street_sim::story::MOVE_SLOTS`), each shown in its film-room clip.
//! A match plays the chosen rows: the game reads each slot's row per
//! created player (0x13a1f8), and the row resolves to its script and its
//! own blend clips through the row→script map (`0x45f9c8`,
//! `game::clip::move_clip`).

use std::{fs::File, io::BufReader};

use bevy::prelude::*;
use street_assets::{db, iso::Iso, locale};
use street_sim::story::{BALLER_RECORDS, BALLER_SLOTS, CreatedBaller, EMPTY_MOVE_SLOT, Story};

use crate::setup::Disc;

/// The game's own name lists, for the editor's names.
#[derive(Resource, Default)]
pub struct Names {
    pub first: Vec<String>,
    pub first_female: Vec<String>,
    pub last: Vec<String>,
}

impl Names {
    pub fn load(iso: &mut Iso<BufReader<File>>) -> Self {
        let mut names = Names::default();
        if let Ok(files) = locale::read(iso, "/LOCALE/XLOC_FE.EUS") {
            for f in files {
                let list = f
                    .texts
                    .into_iter()
                    .filter(|t| !t.is_empty() && t != "n/a")
                    .collect();
                match f.name.as_str() {
                    "random_first.txt" => names.first = list,
                    "random_first_female.txt" => names.first_female = list,
                    "random_last.txt" => names.last = list,
                    _ => {}
                }
            }
        }
        names
    }
}

use street_sim::cab::{BUILD_DP, OUTFITS, SIZES};
/// Builds, sizes, outfits and their prices (`street_sim::cab`, shared
/// with the exporter's catalog).
pub use street_sim::cab::{BUILDS, SIZE_NAMES};

/// The size nearest the baller's height.
pub fn size_of(b: &CreatedBaller) -> usize {
    let sizes = &SIZES[usize::from(b.build.min(5))];
    (0..4)
        .min_by_key(|&k| (i32::from(sizes[k].1) - i32::from(b.height_in)).abs())
        .unwrap_or(0)
}

/// What the baller's build and size cost in development points; an edit
/// pays the difference against what was paid for them (the rewrite's own
/// reading of the build and size prices).
pub fn price(b: &CreatedBaller) -> u32 {
    let build = usize::from(b.build.min(5));
    BUILD_DP[build] + SIZES[build][size_of(b)].0
}

/// The rows `FUN_001e4720` lists round the build's nine kits: the Street
/// Champ jersey first, then after the nine the jerseys and Jordan's.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutfitExtras {
    /// The Street Champ jersey (code 81) heads the list: the baller's
    /// career has cleared the Rucker Park '78 Legendary Challenge (the
    /// event on court 10, `FUN_001e8140`).
    pub champ: bool,
    /// The league team the player joined (`FUN_001d84d0`, Choose Team's join):
    /// its uniform (code 80) comes next.
    pub team: Option<u16>,
    /// After the nine: each owned jersey card's code ([`JERSEY_CODES`] by
    /// the card's index, `FUN_001d26e8` for owned), then Jordan's (71)
    /// once record 167 is unlocked (`FUN_001d9ca0`).
    pub tail: Vec<u8>,
}

/// The jersey cards' outfit codes by the card's index (`0x460330`).
pub const JERSEY_CODES: [u8; 25] = [
    56, 62, 77, 68, 73, 55, 79, 75, 69, 64, 54, 74, 58, 66, 57, 60, 61, 65, 72, 59, 76, 78, 70, 63,
    67,
];

/// The Street Champ jersey's code.
pub const STREET_CHAMP_OUTFIT: u8 = 81;

/// The joined league team's uniform's code.
pub const TEAM_OUTFIT: u8 = 80;

/// The fourth trophy's Jordan's code, listed while record 167 is unlocked.
pub const JORDAN_OUTFIT: u8 = 71;

/// What the outfit list adds for the baller of `record` from the save.
pub fn outfit_extras(story: &Story, disc: &crate::setup::Disc, record: u16) -> OutfitExtras {
    let champ_event = street_sim::story::EVENTS.iter().position(|e| e.court == 10);
    let champ = (0..street_sim::story::CAREER_SLOTS)
        .filter_map(|slot| story.career_at(slot))
        .find(|c| c.crew.first() == Some(&record))
        .zip(champ_event)
        .is_some_and(|(c, ev)| c.cleared(ev));
    let mut tail: Vec<u8> = (0..JERSEY_CODES.len())
        .filter(|&k| {
            disc.cards.iter().any(|c| {
                c.category == crate::rewards::JERSEYS
                    && c.index == k as i16
                    && crate::rewards::card_owned(story, c)
            })
        })
        .map(|k| JERSEY_CODES[k])
        .collect();
    if crate::lineup::unlocked(disc, story)(167) {
        tail.push(JORDAN_OUTFIT);
    }
    let team = story
        .sixth_men
        .iter()
        .find(|&&(_, r)| r == record)
        .map(|&(t, _)| t);
    OutfitExtras { champ, team, tail }
}

/// The outfit list's codes for the baller's build (`FUN_001e4720`).
pub fn outfit_list(b: &CreatedBaller, extras: &OutfitExtras) -> Vec<u8> {
    let mut list = Vec::with_capacity(10 + extras.tail.len());
    if extras.champ {
        list.push(STREET_CHAMP_OUTFIT);
    }
    if extras.team.is_some() {
        list.push(TEAM_OUTFIT);
    }
    list.extend_from_slice(&OUTFITS[usize::from(b.build.min(5))]);
    list.extend_from_slice(&extras.tail);
    list
}

/// The row of the outfit list the baller wears (the first kit's when the
/// list lacks it).
pub fn outfit_index(b: &CreatedBaller, extras: &OutfitExtras) -> usize {
    outfit_list(b, extras)
        .iter()
        .position(|&c| c == b.clothing)
        .unwrap_or(usize::from(extras.champ) + usize::from(extras.team.is_some()))
}

/// Keeps [`Editing::outfit_extras`] to the save while a baller is edited.
pub fn refresh_outfit_extras(
    mut editing: bevy::prelude::ResMut<Editing>,
    save: bevy::prelude::Res<crate::story::Save>,
    disc: Option<bevy::prelude::Res<crate::setup::Disc>>,
) {
    let Some(record) = editing.baller.as_ref().map(|b| b.record() as u16) else {
        return;
    };
    let extras = disc
        .map(|d| outfit_extras(&save.story, &d, record))
        .unwrap_or_default();
    if editing.outfit_extras != extras {
        editing.outfit_extras = extras;
    }
}

/// What the outfit worn costs, by the original's wardrobe
/// (`street_sim::cab::outfit_cost`): `worn` is the outfit row the saved
/// baller wore, `None` while creating one. A special outfit is free.
fn outfit_cost(b: &CreatedBaller, worn: Option<usize>) -> i64 {
    if b.clothing >= street_sim::cab::SPECIAL_OUTFIT {
        return 0;
    }
    street_sim::cab::outfit_cost(
        b.outfit_row(),
        usize::from(b.clothing_variant.min(2)),
        worn,
        &b.wardrobe,
    )
}

/// What the shoes worn cost: nothing once owned (`street_sim::cab::shoe_cost`).
fn shoe_cost(b: &CreatedBaller) -> u32 {
    street_sim::cab::shoe_cost(b.shoes, &b.wardrobe)
}

/// What a premade costs a new career out of its [`START_POINTS`]
/// (`FUN_001e6518`, after `FUN_001e6ac8`'s 1,500): its build's and size's
/// prices, its look (create table 2, every row free), each skill's steps up
/// to its bar (`0x460568`, the first free) and its outfit bought as
/// creating buys one.
pub fn preset_charges(b: &CreatedBaller) -> i64 {
    let skills: u32 = (0..SKILLS.len())
        .map(|k| {
            let bar = skill_level(k, b.skills[k]);
            (0..bar).map(skill_step_price).sum::<u32>()
        })
        .sum();
    i64::from(price(b)) + i64::from(skills) + outfit_cost(b, None)
}

/// The outfit and shoes worn, charged as the original buys them
/// (`FUN_001e4af8`, `FUN_001e5058`).
pub fn wardrobe_cost(b: &CreatedBaller, worn: Option<usize>) -> i64 {
    outfit_cost(b, worn) + i64::from(shoe_cost(b))
}

/// "n DP" for a cost or a refund.
fn dp_signed(n: i64) -> String {
    match n {
        0 => dp(0),
        n if n < 0 => format!("-{} DP", -n),
        n => dp(n as u32),
    }
}

/// "Free" or "n DP" (`createplyr.txt` 5 and 4).
fn dp(n: u32) -> String {
    if n == 0 {
        "Free".into()
    } else {
        format!("{n} DP")
    }
}

/// What raising one of the original seven skill bars costs (0x460568).
fn skill_step_price(step: usize) -> u32 {
    // FUN_001e5138 reads the next of seven entries at 0x460568.
    use street_sim::cab::SKILL_STEP_PRICES as STEPS;
    STEPS[step.min(STEPS.len() - 1)]
}

use street_sim::cab::SKILL_LEVELS;

pub fn skill_level(skill: usize, rating: u8) -> usize {
    SKILL_LEVELS[usize::from(skill != 0)]
        .iter()
        .position(|&r| rating <= r)
        .unwrap_or(6)
}

/// Original max-skills check: every bar at least five, one at six.
pub fn max_skills(b: &CreatedBaller) -> bool {
    let bars = std::array::from_fn::<_, 7, _>(|k| skill_level(k, b.skills[k]));
    bars.iter().all(|&bar| bar >= 5) && bars.contains(&6)
}

/// The front end's seven skills and the rating each sets (0x1d7f48's table).
pub const SKILLS: [(&str, usize); 7] = [
    ("Shots", 0),
    ("Dunks", 2),
    ("Power", 4),
    ("Blocks", 5),
    ("Steals", 6),
    ("Handles", 7),
    ("Rebounds", 3),
];

/// The editor's rows, in order.
pub const ROWS: [&str; 15] = [
    "First Name",
    "Last Name",
    "Gender",
    "Build",
    "Height",
    "Weight",
    "Face",
    "Hair",
    "Hair Color",
    "Facial Hair",
    "Facial Hair Color",
    "Clothes",
    "Clothes Color",
    "Shoes",
    "Skills",
];

/// The row after the editor's [`ROWS`] that opens the moves page, and the
/// one after it that saves.
pub const MOVES_ROW: usize = ROWS.len();
pub const SAVE_ROW: usize = ROWS.len() + 1;

/// The Look list's faces by row (`FUN_001e3168` over the table at
/// `0x4602b0`: 18 rows for a man at `0x4603c8`, 6 for a woman at
/// `0x460410`): "Look A" is face 15 for a man, 3 for a woman.
pub const LOOK_ROWS: [&[u8]; 2] = [
    &[15, 13, 8, 14, 2, 6, 9, 17, 11, 5, 1, 10, 3, 16, 4, 0, 7, 12],
    &[3, 4, 0, 5, 1, 2],
];

/// The Look list's row for the baller's face.
pub fn look_row(b: &CreatedBaller) -> usize {
    LOOK_ROWS[usize::from(b.female)]
        .iter()
        .position(|&face| face == b.look)
        .unwrap_or(0)
}

/// A new baller for Create Baller's wizard (`FUN_00273a08(screen, 3,
/// gender)`, classes 0x4a and 0x4b): the slot's record in the gender the
/// dialog picked, at each list's first row as the panels open on them
/// (`my_team_wizard_*.png`: Stocky in M, Look A, Bald, no facial hair,
/// Outfit A in Color A, the first icon) and "NAME" until the name panel.
pub fn fresh(slot: usize, record: &db::Player, female: bool) -> CreatedBaller {
    let mut b = new(slot, record);
    b.female = female;
    b.build = if female { 4 } else { 0 };
    let (_, height, weight) = SIZES[usize::from(b.build)][0];
    b.height_in = height;
    b.weight_lb = weight;
    b.look = LOOK_ROWS[usize::from(female)][0];
    b.hair_style = -1;
    b.hair_color = 0;
    b.facial_hair_style = -1;
    b.facial_hair_color = 0;
    b.clothing = OUTFITS[usize::from(b.build)][0];
    b.clothing_variant = 0;
    b.first_name = "NAME".into();
    b.last_name = String::new();
    b.icon = 0;
    // What it starts in is its own (Outfit A in Color A: "Item: 0 DP").
    b.wardrobe = b.worn_wardrobe();
    // The player's reset (`FUN_001e68a8` → `FUN_001d96a8`) puts the kinds'
    // own moves in its slots (`my_team_edit_moves.png`: Spin da LP,
    // Buckdown, Shockwave in Dunk Lvl 3's first three).
    b.moves = crate::edit_moves::FREE.map(<[u8]>::to_vec);
    b
}

/// A move on a wizard panel's lists: `dy` along list 0x14 (the build, the
/// look, the hair and facial hair styles, the outfit, the skill row),
/// `dx` along list 0x2c (the size, the hair and facial hair colours, the
/// outfit's colour, a skill's crowns, the icon). The lists stop at their
/// ends; each move sets the baller as the panels' +0x1c4 and +0x1cc do.
pub fn wizard_move(e: &mut Editing, step: crate::cab_view::Step, dy: i32, dx: i32, names: &Names) {
    use crate::cab_view::Step;
    let skill = e.skill;
    if step == Step::Skills {
        e.skill = (skill as i32 + dy).clamp(0, SKILLS.len() as i32 - 1) as usize;
        if dx != 0 {
            adjust(e, 14, dx, names, 0);
        }
        return;
    }
    let extras = e.outfit_extras.clone();
    let Some(b) = e.baller.as_mut() else { return };
    let build = usize::from(b.build.min(5));
    let (looks, hairs, beards) = db::kit_limits(build);
    let step_in = |v: i32, d: i32, lo: i32, hi: i32| (v + d).clamp(lo, hi);
    match step {
        Step::Build => {
            if dy != 0 {
                let (lo, hi) = if b.female { (4, 5) } else { (0, 3) };
                let size = size_of(b);
                b.build = step_in(i32::from(b.build), dy, lo, hi) as u8;
                let (_, height, weight) = SIZES[usize::from(b.build)][size];
                b.height_in = height;
                b.weight_lb = weight;
                fit_build(b);
            }
            if dx != 0 {
                let size = step_in(size_of(b) as i32, dx, 0, 3) as usize;
                let (_, height, weight) = SIZES[usize::from(b.build.min(5))][size];
                b.height_in = height;
                b.weight_lb = weight;
            }
        }
        Step::Look => {
            let faces = LOOK_ROWS[usize::from(b.female)];
            let rows = faces.len().min(looks) as i32;
            let row = step_in(look_row(b) as i32, dy, 0, rows - 1) as usize;
            b.look = faces[row];
        }
        Step::Hair => {
            b.hair_style = step_in(i32::from(b.hair_style), dy, -1, hairs as i32 - 1) as i8;
            b.hair_color =
                step_in(i32::from(b.hair_color), dx, 0, db::HAIR_COLORS as i32 - 1) as u8;
        }
        Step::Facial => {
            b.facial_hair_style =
                step_in(i32::from(b.facial_hair_style), dy, -1, beards as i32 - 1) as i8;
            b.facial_hair_color = step_in(
                i32::from(b.facial_hair_color),
                dx,
                0,
                db::FACIAL_HAIR_COLORS as i32 - 1,
            ) as u8;
        }
        Step::Outfit => {
            let list = outfit_list(b, &extras);
            let row = step_in(
                outfit_index(b, &extras) as i32,
                dy,
                0,
                list.len() as i32 - 1,
            );
            b.clothing = list[row as usize];
            b.clothing_variant = step_in(i32::from(b.clothing_variant), dx, 0, 2) as u8;
        }
        Step::Icon => b.icon = step_in(i32::from(b.icon), dx, 0, 6) as u8,
        Step::Skills | Step::First | Step::Last => {}
    }
}

/// A move on an Edit Baller panel's lists: the shoes walk their rows in
/// price order (`FUN_001e4ee8`); the rest as the wizard's panels.
pub fn edit_move(e: &mut Editing, edit: crate::cab_view::Edit, dy: i32, dx: i32, names: &Names) {
    match edit.step() {
        Some(step) => wizard_move(e, step, dy, dx, names),
        None => {
            if let Some(b) = e.baller.as_mut() {
                let rows = street_sim::cab::SHOES.len() as i32;
                let row = (street_sim::cab::shoe_row(b.shoes) as i32 + dy).clamp(0, rows - 1);
                b.shoes = street_sim::cab::SHOES[row as usize].1;
            }
        }
    }
}

/// The focused item's price on an Edit Baller panel ("Item: %1d DP"):
/// the outfit and the shoes against what the baller owns, the skills
/// bought; the hair is free.
pub fn edit_price(e: &Editing, edit: crate::cab_view::Edit) -> i64 {
    use crate::cab_view::Edit;
    let Some(b) = e.baller.as_ref() else { return 0 };
    match edit {
        Edit::Outfit => outfit_cost(b, e.worn),
        Edit::Shoes => i64::from(shoe_cost(b)),
        Edit::Skills => e.skill_spend,
        Edit::Hair | Edit::Facial => 0,
    }
}

/// Edit Baller's Yes to an outfit's or shoes' price (`FUN_0028e710`,
/// `FUN_00295e40`; the buy, `FUN_001e4af8`, `FUN_001e5058`): the item worn
/// is the baller's from now on, its price taken with the edit's. `None`
/// when there is nothing to pay.
pub fn edit_buy(e: &mut Editing, edit: crate::cab_view::Edit) -> Option<u32> {
    use crate::cab_view::Edit;
    let worn = e.worn;
    let b = e.baller.as_mut()?;
    let cost = match edit {
        Edit::Outfit => outfit_cost(b, worn),
        Edit::Shoes => i64::from(shoe_cost(b)),
        _ => 0,
    };
    if cost <= 0 {
        return None;
    }
    match edit {
        Edit::Outfit => {
            let row = b.outfit_row();
            street_sim::cab::buy_outfit(
                &mut b.wardrobe,
                row,
                usize::from(b.clothing_variant.min(2)),
                true,
            );
            e.worn = Some(row);
        }
        _ => b.wardrobe.buy_shoes(street_sim::cab::shoe_row(b.shoes)),
    }
    e.bought += cost;
    Some(cost as u32)
}

/// The focused item's price on a wizard panel ("Item: %1d DP", the
/// panel's +0x1bc): the build and size (`FUN_00277478`: `FUN_001e28c8` +
/// `FUN_001e2470`), the outfit worn, the skills bought; the looks and
/// hair are free.
pub fn item_price(e: &Editing, step: crate::cab_view::Step) -> i64 {
    use crate::cab_view::Step;
    let Some(b) = e.baller.as_ref() else { return 0 };
    match step {
        Step::Build => i64::from(price(b)),
        Step::Outfit => outfit_cost(b, e.worn),
        Step::Skills => e.skill_spend,
        _ => 0,
    }
}

/// The skills' next crowns' prices (`createplyr_skills.txt` 4), `None` at
/// the sixth.
pub fn skill_prices(b: &CreatedBaller) -> [Option<u32>; 7] {
    std::array::from_fn(|k| {
        let bar = skill_level(k, b.skills[k]);
        (bar < 6).then(|| skill_step_price(bar))
    })
}

/// Development points left to spend while editing: the save's, less what
/// this edit has bought (moves, skills, the build and size, the outfit and
/// shoes worn).
pub fn points_left(e: &Editing, story: &Story) -> i64 {
    let build = e.baller.as_ref().map_or(0, |b| {
        i64::from(price(b)) - i64::from(e.paid) + wardrobe_cost(b, e.worn)
    });
    let points = match (e.start, e.baller.as_ref()) {
        (Some(start), _) => start,
        (None, Some(b)) => story.points_for(b.record() as u16),
        (None, None) => story.development_points,
    };
    i64::from(points) - e.move_spent - e.skill_spend - e.bought - build
}

/// What a new baller has to spend (`FUN_002740b8`: the wizard's
/// `FUN_001e68a8(record, 1)` resets the player and `FUN_001eb0d0` sets its
/// points, `+0x30`, to 1,500; "Total: 1,500 DP" on a new baller).
pub const START_POINTS: u32 = 1500;

/// Puts a create-a-player record back as the disc has it (a deleted
/// baller's).
pub fn restore(disc: &mut Disc, record: usize) {
    if let Ok(all) = db::players(&mut disc.iso)
        && let (Some(original), Some(p)) = (all.get(record), disc.players.get_mut(record))
    {
        *p = original.clone();
    }
}

/// Writes every saved baller over its record.
pub fn apply_all(disc: &mut Disc, story: &Story) {
    // Career stars past the disc's records start from the first
    // create-a-player record, as the original's career players do.
    let wanted = story.ballers.iter().map(|b| b.record() + 1).max();
    if let Some(wanted) = wanted
        && wanted > disc.players.len()
        && let Some(template) = disc.players.get(usize::from(BALLER_RECORDS)).cloned()
    {
        disc.players.resize(wanted, template);
    }
    for b in &story.ballers {
        if let Some(p) = disc.players.get_mut(b.record()) {
            apply(p, b);
        }
    }
}

/// Writes one baller's fields into a database player.
pub fn apply(p: &mut db::Player, b: &CreatedBaller) {
    p.first_name = b.first_name.clone();
    p.last_name = b.last_name.clone();
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
    p.portrait = i16::from(b.icon.min(6));
    for (k, &(_, rating)) in SKILLS.iter().enumerate() {
        p.ratings[rating] = i16::from(b.skills[k]);
    }
    // Long range follows shots; speed follows the build (lighter is quicker).
    p.ratings[1] = [0, 10, 20, 40, 60, 80, 90][skill_level(0, b.skills[0])];
    p.ratings[8] = [85, 75, 65, 55, 80, 70][usize::from(b.build.min(5))];
}

/// A new baller for `slot`, starting from the record's own look.
pub fn new(slot: usize, record: &db::Player) -> CreatedBaller {
    let build = record.build.clamp(0, 5) as u8;
    CreatedBaller {
        slot: slot as u8,
        first_name: record.first_name.clone(),
        last_name: record.last_name.clone(),
        female: record.female,
        build,
        height_in: record.height_in.clamp(60, 92) as u8,
        weight_lb: record.weight_lb.clamp(100, 350) as u16,
        look: record.look.max(0) as u8,
        hair_style: record.hair_style.clamp(-1, 30) as i8,
        hair_color: record.hair_color.max(0) as u8,
        facial_hair_style: record.facial_hair_style.clamp(-1, 30) as i8,
        facial_hair_color: record.facial_hair_color.max(0) as u8,
        clothing: OUTFITS[usize::from(build)][0],
        clothing_variant: ((record.clothing as u16) >> 14).min(2) as u8,
        shoes: record.shoes.clamp(0, db::SHOE_STYLES as i16 - 1) as u8,
        skills: [9, 0, 0, 0, 0, 0, 0],
        moves: Default::default(),
        unlocked: Vec::new(),
        wardrobe: Default::default(),
        icon: record.portrait.clamp(0, 6) as u8,
    }
}

/// The baller being edited, if any, and the skill row under the cursor.
#[derive(Resource, Default)]
pub struct Editing {
    pub baller: Option<CreatedBaller>,
    pub skill: usize,
    /// A baller just saved, for [`apply_commit`] to write over its record.
    pub committed: Option<CreatedBaller>,
    /// Development points this edit has spent on moves (less refunds),
    /// taken from the save when the baller is saved.
    pub move_spent: i64,
    /// Net development price of skill changes, negative for refunds.
    pub skill_spend: i64,
    /// What the baller's build and size were paid for when it was last
    /// saved (nothing for a new one).
    pub paid: u32,
    /// The outfit row the saved baller wore: the edit screen's pricing
    /// (the editor's +8 flag, `FUN_00273dc8`); `None` while creating one.
    pub worn: Option<usize>,
    /// A new baller's own points ([`START_POINTS`] from the wizard), in
    /// place of the save's balance for its record until it is saved.
    pub start: Option<u32>,
    /// What the editor's Yes to its price bought (outfits and shoes,
    /// [`edit_buy`]), taken from the save when the baller is saved.
    pub bought: i64,
    /// The outfit list's rows round the nine ([`refresh_outfit_extras`]).
    pub outfit_extras: OutfitExtras,
}

impl Editing {
    /// The last step's price (used by the rows() readout).
    pub fn skill_step(&self) -> u32 {
        let Some(b) = &self.baller else { return 0 };
        let at = self.skill.min(b.skills.len().max(1) - 1);
        let s = b.skills[at];
        skill_step_price(skill_level(at, s))
    }
}

/// The editor's lines for `b`.
pub fn rows(b: &CreatedBaller, skill: usize, editing: &Editing) -> Vec<String> {
    let (looks, hairs, beards) = db::kit_limits(usize::from(b.build));
    let feet = |h: u8| format!("{}' {}\"", h / 12, h % 12);
    let opt = |v: i8, n: usize| {
        if v < 0 {
            "None".to_owned()
        } else {
            format!("{} of {n}", v + 1)
        }
    };
    let mut out = vec![
        format!("First Name  < {} >", b.first_name),
        format!("Last Name  < {} >", b.last_name),
        format!("Gender  < {} >", if b.female { "Female" } else { "Male" }),
        format!(
            "Build  < {} >  {}",
            BUILDS[usize::from(b.build.min(5))],
            dp(BUILD_DP[usize::from(b.build.min(5))])
        ),
        format!(
            "Height  < {} {} >  {}",
            SIZE_NAMES[size_of(b)],
            feet(b.height_in),
            dp(SIZES[usize::from(b.build.min(5))][size_of(b)].0)
        ),
        format!("Weight  < {} lbs >", b.weight_lb),
        format!("Face  < {} of {looks} >", b.look + 1),
        format!("Hair  < {} >", opt(b.hair_style, hairs)),
        format!(
            "Hair Color  < {} of {} >",
            b.hair_color + 1,
            db::HAIR_COLORS
        ),
        if b.female {
            "Facial Hair  -".to_owned()
        } else {
            format!("Facial Hair  < {} >", opt(b.facial_hair_style, beards))
        },
        format!(
            "Facial Hair Color  < {} of {} >",
            b.facial_hair_color + 1,
            db::FACIAL_HAIR_COLORS
        ),
        format!(
            "Clothes  < {} of 9 >  {}",
            b.clothing + 1,
            dp_signed(outfit_cost(b, editing.worn))
        ),
        format!("Clothes Color  < {} of 3 >", b.clothing_variant + 1),
        format!(
            "Shoes  < {} of {} >  {}",
            street_sim::cab::shoe_row(b.shoes) + 1,
            db::SHOE_STYLES,
            dp(shoe_cost(b))
        ),
        format!(
            "Skills (next: {} DP)  < {} {} / 6 >",
            editing.skill_step(),
            SKILLS[skill].0,
            skill_level(skill, b.skills[skill]),
        ),
    ];
    out.push(format!(
        "Edit Moves  ({} in slots)",
        b.moves
            .iter()
            .flatten()
            .filter(|&&row| row != EMPTY_MOVE_SLOT)
            .count()
    ));
    out.push("Save Baller".into());
    out
}

fn cycle(v: i32, step: i32, lo: i32, hi: i32) -> i32 {
    let n = hi - lo + 1;
    lo + (v - lo + step).rem_euclid(n.max(1))
}

/// Left or right on row `row`. On the Skills row, up and down pick the skill
/// (`Editing::skill`) and left and right spend or refund points.
pub fn adjust(e: &mut Editing, row: usize, step: i32, names: &Names, roll: u32) {
    let skill = e.skill;
    let Some(b) = e.baller.as_mut() else { return };
    let pick = |list: &[String], current: &str| -> String {
        if list.is_empty() {
            return current.to_owned();
        }
        let at = list
            .iter()
            .position(|n| n == current)
            .unwrap_or(roll as usize);
        list[cycle(at as i32, step, 0, list.len() as i32 - 1) as usize].clone()
    };
    match row {
        0 => {
            b.first_name = pick(
                if b.female {
                    &names.first_female
                } else {
                    &names.first
                },
                &b.first_name,
            )
        }
        1 => b.last_name = pick(&names.last, &b.last_name),
        2 => {
            b.female = !b.female;
            b.build = if b.female { 4 } else { 0 };
            b.facial_hair_style = -1;
            b.first_name = pick(
                if b.female {
                    &names.first_female
                } else {
                    &names.first
                },
                "",
            );
            fit_build(b);
        }
        3 => {
            let size = size_of(b);
            b.build = if b.female {
                cycle(i32::from(b.build), step, 4, 5) as u8
            } else {
                cycle(i32::from(b.build), step, 0, 3) as u8
            };
            b.height_in = SIZES[usize::from(b.build)][size].1;
            fit_build(b);
        }
        4 => {
            // The four sizes, each with its own height and weight.
            let size = (size_of(b) as i32 + step).clamp(0, 3) as usize;
            let (_, height, weight) = SIZES[usize::from(b.build.min(5))][size];
            b.height_in = height;
            b.weight_lb = weight;
        }
        5 => b.weight_lb = (i32::from(b.weight_lb) + 5 * step).clamp(100, 350) as u16,
        6 => {
            let (looks, _, _) = db::kit_limits(usize::from(b.build));
            b.look = cycle(i32::from(b.look), step, 0, looks as i32 - 1) as u8;
        }
        7 => {
            let (_, hairs, _) = db::kit_limits(usize::from(b.build));
            b.hair_style = cycle(i32::from(b.hair_style), step, -1, hairs as i32 - 1) as i8;
        }
        8 => {
            b.hair_color = cycle(i32::from(b.hair_color), step, 0, db::HAIR_COLORS as i32 - 1) as u8
        }
        9 if !b.female => {
            let (_, _, beards) = db::kit_limits(usize::from(b.build));
            b.facial_hair_style =
                cycle(i32::from(b.facial_hair_style), step, -1, beards as i32 - 1) as i8;
        }
        10 => {
            b.facial_hair_color = cycle(
                i32::from(b.facial_hair_color),
                step,
                0,
                db::FACIAL_HAIR_COLORS as i32 - 1,
            ) as u8;
        }
        11 => {
            let choices = &OUTFITS[usize::from(b.build.min(5))];
            let at = choices.iter().position(|&id| id == b.clothing).unwrap_or(0);
            b.clothing = choices[cycle(at as i32, step, 0, 8) as usize];
        }
        12 => b.clothing_variant = cycle(i32::from(b.clothing_variant), step, 0, 2) as u8,
        // The menu walks the shoe rows in price order (`FUN_001e4ee8`).
        13 => {
            let row = street_sim::cab::shoe_row(b.shoes);
            let rows = street_sim::cab::SHOES.len() as i32 - 1;
            b.shoes = street_sim::cab::SHOES[cycle(row as i32, step, 0, rows) as usize].1;
        }
        14 => {
            let bar = skill_level(skill, b.skills[skill]);
            if step > 0 && bar < 6 {
                b.skills[skill] = SKILL_LEVELS[usize::from(skill != 0)][bar + 1];
                e.skill_spend += i64::from(skill_step_price(bar));
            } else if step < 0 && bar > 0 {
                b.skills[skill] = SKILL_LEVELS[usize::from(skill != 0)][bar - 1];
                e.skill_spend -= i64::from(skill_step_price(bar - 1));
            }
        }
        _ => {}
    }
}

/// Keeps height, weight, face and hair inside what the new build offers.
fn fit_build(b: &mut CreatedBaller) {
    // The same size in the new build.
    let size = size_of(b);
    let (_, height, weight) = SIZES[usize::from(b.build.min(5))][size];
    b.height_in = height;
    b.weight_lb = weight;
    let (looks, hairs, _) = db::kit_limits(usize::from(b.build));
    b.look = b.look.min(looks as u8 - 1);
    b.hair_style = b.hair_style.min(hairs as i8 - 1);
    b.clothing = OUTFITS[usize::from(b.build.min(5))][0];
}

/// Stores the edited baller in the save; [`apply_commit`] then writes it over
/// the Disc's record.
pub fn commit(e: &mut Editing, story: &mut Story) -> Result<(), String> {
    // The build, size and skills must be paid for.
    if points_left(e, story) < 0 {
        return Err("You do not have enough development points to unlock this item.".into());
    }
    let Some(mut b) = e.baller.take() else {
        return Ok(());
    };
    // The moves bought (or handed back) in this edit, the skill steps spent,
    // the build and size's price against what was paid before, and the
    // outfit and shoes worn as the wardrobe prices them; then they are
    // owned (`FUN_001e7820`, `FUN_001e7940`).
    let record = b.record() as u16;
    // A new baller's points are its own (`START_POINTS`), whatever the
    // record held before.
    let balance = e.start.take().unwrap_or_else(|| story.points_for(record));
    let worn = std::mem::take(&mut e.worn);
    let wardrobe = wardrobe_cost(&b, worn);
    let points = i64::from(balance)
        - std::mem::take(&mut e.move_spent)
        - std::mem::take(&mut e.skill_spend)
        - std::mem::take(&mut e.bought)
        - (i64::from(price(&b)) - i64::from(std::mem::take(&mut e.paid)))
        - wardrobe;
    let (row, colour) = (b.outfit_row(), usize::from(b.clothing_variant.min(2)));
    if b.clothing < street_sim::cab::SPECIAL_OUTFIT {
        street_sim::cab::buy_outfit(&mut b.wardrobe, row, colour, worn.is_some());
    }
    b.wardrobe.buy_shoes(street_sim::cab::shoe_row(b.shoes));
    story.credit_baller(record, 0);
    let now = story.points_for(record);
    if points <= i64::from(now) {
        story.spend_baller(record, now - points.max(0) as u32);
    } else {
        story.credit_baller(
            record,
            (points - i64::from(now)).min(i64::from(u32::MAX)) as u32,
        );
    }
    if max_skills(&b) {
        story.reach_milestone(4);
    }
    story.ballers.retain(|x| x.slot != b.slot);
    story.ballers.push(b.clone());
    story.ballers.sort_by_key(|x| x.slot);
    e.committed = Some(b);
    Ok(())
}

/// Writes a just-saved baller over its record (after the menu's input).
pub fn apply_commit(mut e: ResMut<Editing>, mut disc: ResMut<Disc>) {
    if let Some(b) = e.committed.take()
        && let Some(p) = disc.players.get_mut(b.record())
    {
        apply(p, &b);
    }
}

/// The six slots' lines for the list page.
pub fn slot_items(disc: &Disc, story: &Story) -> Vec<String> {
    let mut out: Vec<String> = (0..BALLER_SLOTS)
        .map(|slot| {
            let record = usize::from(BALLER_RECORDS) + slot;
            let made = story.ballers.iter().any(|b| usize::from(b.slot) == slot);
            let p = &disc.players[record];
            format!(
                "{}. {}{}",
                slot + 1,
                p.name(),
                if made { "" } else { "  (premade)" }
            )
        })
        .collect();
    out.push("Back".into());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `FUN_001e4720`'s codes and tables, read from the disc's executable:
    /// 81 (0x1e4774), 80 (0x1e47ac), record 167 and Jordan's 71 (0x1e4944,
    /// 0x1e4960), the jerseys' codes (`0x460330`) and each special code's
    /// name (`0x3f94c8`: text id `0x3100 | n` in `editplyr_clothes.txt`).
    #[test]
    fn the_special_outfits_are_the_executables() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let elf = street_assets::iso::Iso::open(path)
            .unwrap()
            .read("/SLUS_206.51")
            .unwrap();
        let word =
            |address: u32| street_assets::scripts::executable_words(&elf, address, 1).unwrap()[0];
        assert_eq!(word(0x1e4774) & 0xffff, u32::from(STREET_CHAMP_OUTFIT));
        assert_eq!(word(0x1e47ac) & 0xffff, 80);
        assert_eq!(word(0x1e4944) & 0xffff, 167);
        assert_eq!(word(0x1e4960) & 0xffff, u32::from(JORDAN_OUTFIT));
        let codes: Vec<u8> = (0..13)
            .flat_map(|k| {
                let w = word(0x460330 + 4 * k);
                [w as u16, (w >> 16) as u16]
            })
            .take(25)
            .map(|c| c as u8)
            .collect();
        assert_eq!(codes, JERSEY_CODES);
        for (k, &n) in crate::cab_view::SPECIAL_OUTFIT_TEXT.iter().enumerate() {
            let code = u32::from(street_sim::cab::SPECIAL_OUTFIT) + k as u32;
            assert_eq!(word(0x3f94c8 + 8 * code), 0x3100 | n as u32, "code {code}");
        }
    }

    #[test]
    fn the_outfit_list_puts_the_champ_first_and_the_jerseys_after_the_nine() {
        let mut b = baller();
        let extras = OutfitExtras {
            champ: true,
            team: Some(5),
            tail: vec![56, JORDAN_OUTFIT],
        };
        let list = outfit_list(&b, &extras);
        assert_eq!(list.len(), 13);
        assert_eq!(&list[..2], &[STREET_CHAMP_OUTFIT, TEAM_OUTFIT]);
        assert_eq!(&list[2..11], &OUTFITS[usize::from(b.build)]);
        assert_eq!(&list[11..], &[56, JORDAN_OUTFIT]);
        // A special costs nothing and is no row of the wardrobe.
        b.clothing = 56;
        assert_eq!(outfit_index(&b, &extras), 11);
        assert_eq!(outfit_cost(&b, None), 0);
        assert_eq!(outfit_cost(&b, Some(3)), 0);
        assert_eq!(b.worn_wardrobe(), {
            let mut w = street_sim::cab::Wardrobe::default();
            w.buy_shoes(street_sim::cab::shoe_row(b.shoes));
            w
        });
        // Without its row the list starts at the first kit.
        b.clothing = 80;
        assert_eq!(outfit_index(&b, &OutfitExtras::default()), 0);
    }

    fn baller() -> CreatedBaller {
        CreatedBaller {
            build: 2,
            height_in: 76,
            weight_lb: 200,
            clothing: 2,
            skills: [9, 0, 0, 0, 0, 0, 0],
            ..Default::default()
        }
    }

    #[test]
    fn a_premade_costs_its_build_size_skills_and_outfit() {
        // `FUN_001e6518`: build and size, the skills' steps to their bars
        // (the first free), the outfit bought as creating buys it.
        let mut b = baller();
        let base = i64::from(price(&b)) + outfit_cost(&b, None);
        assert_eq!(preset_charges(&b), base);
        // Two bars up in one skill: 100 + 200.
        b.skills[1] = SKILL_LEVELS[1][2];
        assert_eq!(preset_charges(&b), base + 300);
        // An outfit already owned in its colour costs nothing more.
        b.wardrobe = b.worn_wardrobe();
        assert_eq!(preset_charges(&b), i64::from(price(&b)) + 300);
    }

    #[test]
    fn a_new_baller_starts_with_its_own_points() {
        // The wizard's baller spends from 1,500 (`FUN_001eb0d0`), whatever
        // its record held, and keeps the rest when it is saved.
        let mut story = Story::default();
        let b = baller();
        let record = b.record() as u16;
        story.credit_baller(record, 40);
        let mut e = Editing {
            start: Some(START_POINTS),
            paid: price(&b),
            baller: Some(CreatedBaller {
                wardrobe: b.worn_wardrobe(),
                ..b
            }),
            ..Default::default()
        };
        e.skill_spend = 200;
        assert_eq!(points_left(&e, &story), 1300);
        commit(&mut e, &mut story).unwrap();
        assert_eq!(story.points_for(record), 1300);
        assert_eq!(e.start, None);
    }

    #[test]
    fn the_editors_yes_buys_once() {
        use crate::cab_view::Edit;
        let mut story = Story::default();
        let mut b = baller();
        b.wardrobe = b.worn_wardrobe();
        let record = b.record() as u16;
        story.credit_baller(record, 1000);
        let worn = b.outfit_row();
        let mut e = Editing {
            paid: price(&b),
            worn: Some(worn),
            baller: Some(b),
            ..Default::default()
        };
        let before = points_left(&e, &story);
        // Shoes not owned: the editor's Yes takes their price and owns them.
        let row = street_sim::cab::SHOES
            .iter()
            .position(|&(cost, _)| cost > 0)
            .unwrap();
        e.baller.as_mut().unwrap().shoes = street_sim::cab::SHOES[row].1;
        let cost = edit_price(&e, Edit::Shoes);
        assert!(cost > 0);
        assert_eq!(edit_buy(&mut e, Edit::Shoes), Some(cost as u32));
        assert_eq!(edit_price(&e, Edit::Shoes), 0);
        assert_eq!(edit_buy(&mut e, Edit::Shoes), None);
        assert_eq!(points_left(&e, &story), before - cost);
        commit(&mut e, &mut story).unwrap();
        assert_eq!(i64::from(story.points_for(record)), before - cost);
    }

    #[test]
    fn skills_use_the_original_bars_and_refund_exactly() {
        let mut e = Editing {
            baller: Some(baller()),
            ..Default::default()
        };
        let names = Names::default();
        for _ in 0..6 {
            adjust(&mut e, 14, 1, &names, 0);
        }
        let b = e.baller.as_ref().unwrap();
        assert_eq!(b.skills[0], 98);
        assert_eq!(e.skill_spend, 2100);
        for _ in 0..6 {
            adjust(&mut e, 14, -1, &names, 0);
        }
        assert_eq!(e.baller.as_ref().unwrap().skills[0], 9);
        assert_eq!(e.skill_spend, 0);
    }

    #[test]
    fn max_skills_matches_the_original_seven_bar_condition() {
        let mut b = baller();
        b.skills = [90; 7];
        assert!(!max_skills(&b));
        b.skills[0] = 98;
        assert!(max_skills(&b));
        b.skills[1] = 80;
        assert!(!max_skills(&b));
    }

    #[test]
    fn skill_commit_spends_only_this_baller_and_refunds_a_saved_skill() {
        let b = baller();
        let mut s = Story::default();
        s.credit_baller(234, 500);
        s.credit_baller(235, 200);
        let mut b = b;
        b.wardrobe = b.worn_wardrobe();
        let mut e = Editing {
            paid: price(&b),
            worn: Some(b.outfit_row()),
            baller: Some(b),
            ..Default::default()
        };
        adjust(&mut e, 14, 1, &Names::default(), 0);
        commit(&mut e, &mut s).unwrap();
        assert_eq!(s.points_for(234), 400);
        assert_eq!(s.points_for(235), 200);
        let b = s.ballers[0].clone();
        e.paid = price(&b);
        e.worn = Some(b.outfit_row());
        e.baller = Some(b);
        adjust(&mut e, 14, -1, &Names::default(), 0);
        commit(&mut e, &mut s).unwrap();
        assert_eq!(s.points_for(234), 500);
    }

    #[test]
    fn a_new_build_keeps_the_baller_inside_its_limits() {
        let mut e = Editing {
            baller: Some(baller()),
            ..Default::default()
        };
        let names = Names::default();
        // Switch to a woman: no facial hair, a women's build, a fitting height.
        adjust(&mut e, 2, 1, &names, 0);
        let b = e.baller.as_ref().unwrap();
        assert!(b.female && b.build >= 4 && b.facial_hair_style == -1);
        let sizes = SIZES[usize::from(b.build)];
        assert!(sizes.iter().any(|s| s.1 == b.height_in));
    }

    #[test]
    fn shoes_walk_and_cost_by_their_menu_row() {
        // `FUN_001e4ee8`/`FUN_001e4e30`: the menu goes row by row through
        // 0x4604c8 and prices the row; shoe 2 sits in row 10 at 75.
        let mut e = Editing {
            baller: Some(CreatedBaller {
                shoes: 0,
                ..Default::default()
            }),
            ..Default::default()
        };
        let names = Names::default();
        let mut seen = Vec::new();
        for _ in 0..20 {
            adjust(&mut e, 13, 1, &names, 0);
            seen.push(e.baller.as_ref().unwrap().shoes);
        }
        let order: Vec<u8> = street_sim::cab::SHOES.iter().map(|&(_, id)| id).collect();
        assert_eq!(seen[..19], order[1..]);
        assert_eq!(seen[19], 0, "the walk wraps to row 0");
        let mut b = CreatedBaller {
            shoes: 2,
            ..Default::default()
        };
        assert_eq!(shoe_cost(&b), 75);
        // Bought once, the row is the player's: it never costs again.
        b.wardrobe.buy_shoes(10);
        assert_eq!(shoe_cost(&b), 0);
        b.shoes = 0;
        assert_eq!(shoe_cost(&b), 0, "row 0 is free");
    }

    /// The original's wardrobe (`FUN_001e4580`, `FUN_001e4e30`,
    /// `FUN_001e7820`, `FUN_001e7940`, career +0x670..+0x678) through the
    /// editor: a new baller pays 50 for the free outfit's colour and owns
    /// it; shoes bought stay owned, so wearing them again is free and
    /// switching away refunds nothing; an edit prices the outfit by the
    /// row's difference against the worn one, refunding a cheaper one.
    #[test]
    fn the_wardrobe_charges_and_keeps_what_is_bought() {
        let mut story = Story::default();
        story.credit_baller(234, 1000);
        let mut b = baller();
        b.shoes = 2; // row 10, 75
        let mut e = Editing {
            baller: Some(b),
            ..Default::default()
        };
        assert_eq!(points_left(&e, &story), 1000 - 400 - 50 - 75);
        commit(&mut e, &mut story).unwrap();
        assert_eq!(story.points_for(234), 475);
        let saved = story.ballers[0].clone();
        assert!(saved.wardrobe.owns_outfit(0, 0));
        assert!(saved.wardrobe.owns_shoes(10));
        // Editing: other shoes cost theirs, the old ones nothing again.
        let mut b = saved.clone();
        b.shoes = 14; // row 14, 100
        let mut e = Editing {
            paid: price(&b),
            worn: Some(saved.outfit_row()),
            baller: Some(b),
            ..Default::default()
        };
        commit(&mut e, &mut story).unwrap();
        assert_eq!(story.points_for(234), 375, "no refund for the old pair");
        let mut b = story.ballers[0].clone();
        b.shoes = 2;
        let mut e = Editing {
            paid: price(&b),
            worn: Some(b.outfit_row()),
            baller: Some(b),
            ..Default::default()
        };
        commit(&mut e, &mut story).unwrap();
        assert_eq!(story.points_for(234), 375, "owned shoes are free");
        // The outfit row 7 (200) and back to row 0: the difference each way.
        let mut b = story.ballers[0].clone();
        b.clothing = OUTFITS[2][7];
        let mut e = Editing {
            paid: price(&b),
            worn: Some(0),
            baller: Some(b),
            ..Default::default()
        };
        commit(&mut e, &mut story).unwrap();
        assert_eq!(story.points_for(234), 175);
        let w = story.ballers[0].wardrobe;
        assert!(w.owns_outfit(7, 0) && w.owns_outfit(0, 0) && !w.owns_outfit(0, 1));
        let mut b = story.ballers[0].clone();
        b.clothing = OUTFITS[2][0];
        let mut e = Editing {
            paid: price(&b),
            worn: Some(7),
            baller: Some(b),
            ..Default::default()
        };
        commit(&mut e, &mut story).unwrap();
        assert_eq!(story.points_for(234), 375, "the cheaper outfit refunds");
    }

    #[test]
    fn builds_and_sizes_cost_development_points() {
        let mut b = CreatedBaller {
            build: 0,
            height_in: 69,
            clothing: 4,
            ..Default::default()
        };
        assert_eq!((size_of(&b), price(&b)), (0, 0));
        let mut e = Editing {
            baller: Some(b.clone()),
            ..Default::default()
        };
        let names = Names::default();
        adjust(&mut e, 4, 1, &names, 0); // size L
        adjust(&mut e, 3, 1, &names, 0); // the next build, same size
        let edited = e.baller.clone().unwrap();
        assert_eq!(
            (edited.build, edited.height_in, edited.weight_lb),
            (1, 76, 220)
        );
        assert_eq!(price(&edited), 400 + 200);
        // Without the points it can't be saved; with them it costs them,
        // and the new baller's free outfit costs its colour (50).
        let mut story = Story::default();
        assert!(commit(&mut e, &mut story).is_err());
        story.credit_baller(234, 700);
        assert!(commit(&mut e, &mut story).is_ok());
        assert_eq!(story.points_for(234), 50);
        b.build = 1;
        b.height_in = 84;
        assert_eq!(price(&b), 400 + 400);
    }

    #[test]
    fn skills_and_steps_cost_the_point_buy_ladder_matching_native_table() {
        // Native FUN_001e5138 reads table 14/15 at 0x460568: [0, 100, 200, 300, 400, 500, 600].
        // Next step price from level i (0..5) is table[i + 1].
        let native_ladder = [0u32, 100, 200, 300, 400, 500, 600];
        for i in 0..6 {
            assert_eq!(skill_step_price(i), native_ladder[i + 1]);
        }
    }

    #[test]
    fn the_wizards_lists_stop_at_their_ends_and_set_the_baller() {
        use crate::cab_view::Step;
        let names = Names::default();
        let mut e = Editing {
            baller: Some(CreatedBaller {
                build: 0,
                height_in: SIZES[0][0].1,
                look: LOOK_ROWS[0][0],
                hair_style: -1,
                facial_hair_style: -1,
                clothing: OUTFITS[0][0],
                ..baller()
            }),
            ..Default::default()
        };
        // Build: up stops at Stocky, down walks the four; the size keeps.
        wizard_move(&mut e, Step::Build, -1, 0, &names);
        assert_eq!(e.baller.as_ref().unwrap().build, 0);
        wizard_move(&mut e, Step::Build, 0, 1, &names);
        wizard_move(&mut e, Step::Build, 1, 0, &names);
        let b = e.baller.as_ref().unwrap();
        assert_eq!((b.build, size_of(b)), (1, 1));
        assert_eq!(b.height_in, SIZES[1][1].1);
        for _ in 0..9 {
            wizard_move(&mut e, Step::Build, 0, 1, &names);
        }
        assert_eq!(size_of(e.baller.as_ref().unwrap()), 3);
        // Look: rows through `0x4603c8`'s faces.
        wizard_move(&mut e, Step::Look, 1, 0, &names);
        assert_eq!(e.baller.as_ref().unwrap().look, LOOK_ROWS[0][1]);
        assert_eq!(look_row(e.baller.as_ref().unwrap()), 1);
        // Hair: Bald is row 0 (style -1), the colours stop at eight.
        wizard_move(&mut e, Step::Hair, -1, 9, &names);
        let b = e.baller.as_ref().unwrap();
        assert_eq!((b.hair_style, b.hair_color), (-1, 7));
        // Outfit: rows through the build's table, three colours.
        wizard_move(&mut e, Step::Outfit, 2, 5, &names);
        let b = e.baller.as_ref().unwrap();
        assert_eq!((b.outfit_row(), b.clothing_variant), (2, 2));
        // The icons, 0..6.
        wizard_move(&mut e, Step::Icon, 0, 9, &names);
        assert_eq!(e.baller.as_ref().unwrap().icon, 6);
        // Skills: the row and a crown bought.
        wizard_move(&mut e, Step::Skills, 9, 0, &names);
        assert_eq!(e.skill, 6);
        wizard_move(&mut e, Step::Skills, 0, 1, &names);
        assert!(e.skill_spend > 0);
        assert_eq!(item_price(&e, Step::Skills), e.skill_spend);
    }

    #[test]
    fn the_editors_shoes_walk_their_rows_in_price_order() {
        use crate::cab_view::Edit;
        let names = Names::default();
        let mut e = Editing {
            baller: Some(CreatedBaller {
                shoes: street_sim::cab::SHOES[0].1,
                ..baller()
            }),
            ..Default::default()
        };
        edit_move(&mut e, Edit::Shoes, 2, 0, &names);
        assert_eq!(
            e.baller.as_ref().unwrap().shoes,
            street_sim::cab::SHOES[2].1
        );
        edit_move(&mut e, Edit::Shoes, -9, 0, &names);
        assert_eq!(
            e.baller.as_ref().unwrap().shoes,
            street_sim::cab::SHOES[0].1
        );
        assert_eq!(edit_price(&e, Edit::Shoes), 0);
        edit_move(&mut e, Edit::Shoes, 1, 0, &names);
        assert_eq!(
            edit_price(&e, Edit::Shoes),
            i64::from(street_sim::cab::SHOES[1].0)
        );
        // The other panels share the wizard's lists.
        edit_move(&mut e, Edit::Hair, 0, 3, &names);
        assert_eq!(e.baller.as_ref().unwrap().hair_color, 3);
    }

    #[test]
    fn the_look_rows_are_the_games_faces() {
        // Each list holds every face of its kit once.
        for (female, rows) in LOOK_ROWS.iter().enumerate() {
            let (looks, _, _) = db::kit_limits(if female == 1 { 4 } else { 0 });
            let mut faces: Vec<u8> = rows.to_vec();
            faces.sort_unstable();
            assert_eq!(faces, (0..looks as u8).collect::<Vec<_>>());
        }
        assert_eq!(LOOK_ROWS[0][0], 15);
        assert_eq!(LOOK_ROWS[1][0], 3);
    }

    #[test]
    fn the_skill_prices_end_at_the_sixth_crown() {
        let mut b = baller();
        b.skills = [98, 0, 0, 0, 0, 0, 0];
        let prices = skill_prices(&b);
        assert_eq!(prices[0], None);
        assert_eq!(prices[1], Some(skill_step_price(0)));
    }

    #[test]
    fn the_viewers_cameras_follow_the_traced_offsets() {
        use crate::cab_view::Framing;
        // The whole baller: 76.13 in front, 4.29 above the hips, level.
        let (at, tilt) = viewer_camera(Framing::Body, 0.0);
        assert!((at - Vec3::new(0.0, 4.2932, 76.1291)).length() < 1e-3);
        assert_eq!(tilt, 0.0);
        // The head: level with the head bone, 26.74 in front.
        let (at, _) = viewer_camera(Framing::Head, 6.5);
        assert!((at - Vec3::new(0.0, 6.5, 26.7402)).length() < 1e-3);
        // The premade picker: 6.18 up, 36.7 in front.
        let (at, _) = viewer_camera(Framing::Picker, 6.5);
        assert!((at - Vec3::new(0.0, 6.1833, 36.7)).length() < 1e-3);
        // The projection read at run time: 5 degrees across the 199
        // pixels, 6.83 down the 272.
        assert!((viewer_fov().to_degrees() - 6.8304).abs() < 1e-3);
    }
}

/// The editor's 3D preview: the baller in the create-a-player idle, drawn to
/// an image the menu shows beside the options (the model viewer
/// `FUN_002db8b8`, its viewport (66, 134) 199 by 272 on the layouts'
/// screen: `FUN_003235d0(66, 134 - 16, 199, 272)` on the PS2's frame).
#[derive(Resource, Default)]
pub struct Preview {
    /// The baller now shown (moves aside), to rebuild only on a change.
    shown: Option<CreatedBaller>,
    image: Option<Handle<Image>>,
    /// The picture panel shows something else (the film room).
    pub hidden: bool,
    /// The shown model's hips' height and its head bone's (bone 21) above
    /// them, in world units (ten inches).
    model: Option<(f32, f32)>,
}

/// The viewer's projection (`FUN_00323728`: `FUN_00378328(5.0, w / h,
/// 10, 300)`), read at run time through a save state of Edit Baller
/// (the camera context at 0x9a7380: `+0x110` 5, `+0x114` 199 / 272, the
/// scales `+0x120`, `+0x124` 1 and 1, `+0x190` 1.3, the viewport (66, 118)
/// 199 by 272, the matrix's x and y 17.618 and -12.890, the viewport's
/// 129.35 and 176.8): 2,278.9 pixels at depth 1 both ways, so 5 degrees
/// across the 199 pixels and `2 atan(tan 2.5° × 272 / 199)` down.
fn viewer_fov() -> f32 {
    2.0 * (2.5f32.to_radians().tan() * VIEWPORT.y / VIEWPORT.x).atan()
}

/// The viewer's viewport (`FUN_003235d0(66, 134 - 16, 199, 272)`) and its
/// near and far planes.
const VIEWPORT: Vec2 = Vec2::new(199.0, 272.0);
const NEAR: f32 = 10.0;
const FAR: f32 = 300.0;

/// Where the viewer's camera sits for `framing`, from the model's place
/// on the floor (world units: with the projection read at run time,
/// [`viewer_fov`], the translates are the model's own units, not tenths;
/// `head` the head bone's height; the model faces the camera) and how far
/// it tips down, in radians (`FUN_002dc0e0`: the camera's translate, then
/// a turn about x, then the half turn about y that faces the model to it).
pub fn viewer_camera(framing: crate::cab_view::Framing, head: f32) -> (Vec3, f32) {
    use crate::cab_view::Framing;
    let (t, tilt) = match framing {
        // (0, -4.2932, 76.1291), level (the -13.61 degree turn,
        // 0xc159c361, is mode 2's, which no panel asks).
        Framing::Body => (Vec3::new(0.0, -4.2932, 76.1291), 0.0),
        // Level with the head bone, 23.7402 less the zoom (held at -3).
        Framing::Head => (Vec3::new(0.0, -head, 23.7402 + 3.0), 0.0),
        // (0, -6.1833, 36.7), level.
        Framing::Picker => (Vec3::new(0.0, -6.1833, 36.7), 0.0),
    };
    // The camera's place is the translate undone through the turn.
    let at = Quat::from_rotation_x(-tilt) * -t;
    (Vec3::new(at.x, at.y, -at.z), tilt)
}

#[derive(Component)]
pub struct PreviewPart;

/// The viewer's pose for `framing`: the head's (`FUN_002dc4a8`, animation
/// 0x476) or the stand the whole baller and the premade picker take
/// (`FUN_002dc4e0`, `FUN_002dc560`: 0x477).
pub fn viewer_pose(framing: crate::cab_view::Framing) -> &'static str {
    match framing {
        crate::cab_view::Framing::Head => crate::clip::CREATE_LOOK,
        _ => crate::clip::CREATE_STAND,
    }
}

/// Where the preview stands, away from anything else in the menus.
const STAGE: Vec3 = Vec3::new(1000.0, 0.0, 0.0);

/// Shows, rebuilds or clears the preview to match the baller being edited.
pub fn preview(
    mut commands: Commands,
    editing: Res<Editing>,
    mut state: ResMut<Preview>,
    mut disc: ResMut<Disc>,
    clips: Res<crate::setup::GameClips>,
    parts: Query<Entity, With<PreviewPart>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bindposes: ResMut<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
    mut panels: Query<&mut Visibility, (With<PreviewPart>, With<ImageNode>)>,
    mut cameras: Query<&mut Transform, (With<Camera3d>, With<PreviewPart>)>,
    mut animators: Query<&mut street_render::animation::Animator, With<PreviewPart>>,
    (skins, joints): (
        Query<(&bevy::mesh::skinning::SkinnedMesh, &ChildOf)>,
        Query<&GlobalTransform>,
    ),
    menus: Res<crate::menu::Menus>,
) {
    let want = if state.hidden {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut v in &mut panels {
        if *v != want {
            *v = want;
        }
    }
    // The camera for the screen's framing (each frame: the panels change
    // it without a new baller).
    let framing = crate::menu::preview_framing(&menus);
    if let Some((hips, head)) = state.model {
        // The head panels' camera follows bone 21 as the pose moves it
        // (`FUN_002dbb50` places the camera each update, `FUN_002dc0e0`
        // reading the bone through the model's `vtable + 0x1c`).
        let posed = skins
            .iter()
            .find(|(_, c)| animators.contains(c.parent()))
            .and_then(|(s, _)| s.joints.get(21))
            .and_then(|&j| joints.get(j).ok())
            .map(|g| g.translation().y - STAGE.y);
        let (at, tilt) = viewer_camera(framing, posed.unwrap_or(hips + head));
        let eye = STAGE + at;
        let want = Transform::from_translation(eye)
            .looking_to(Vec3::new(0.0, tilt.sin(), -tilt.cos()), Vec3::Y);
        for mut t in &mut cameras {
            if *t != want {
                *t = want;
            }
        }
        // The framing's pose (`FUN_002daec8` with 0x476 or 0x477).
        let pose = clips.get(viewer_pose(framing));
        for mut a in &mut animators {
            if !std::sync::Arc::ptr_eq(&a.current.clip, &pose) {
                a.play(&pose, 0.0, true);
            }
        }
    }
    // Moves don't change the look.
    let shown = editing.baller.clone().map(|mut b| {
        b.moves = Default::default();
        b
    });
    if state.shown == shown {
        return;
    }
    for e in &parts {
        commands.entity(e).despawn();
    }
    state.shown = shown;
    state.model = None;
    let Some(b) = &editing.baller else { return };
    // A career's own star past the disc's records starts from the first
    // create-a-player record, as `apply_all` makes it.
    let Some(mut record) = disc
        .players
        .get(b.record())
        .or(disc.players.get(usize::from(BALLER_RECORDS)))
        .cloned()
    else {
        return;
    };
    apply(&mut record, b);
    let assets = match street_render::player::PlayerAssets::load(
        &mut disc.iso,
        &street_render::player::Choice::Player {
            record: Box::new(record),
            side: street_assets::player::Side::Road,
            // The team uniform (code 80) in the team its player joined.
            team: (b.clothing == TEAM_OUTFIT)
                .then_some(editing.outfit_extras.team)
                .flatten()
                .map(usize::from),
            // A created baller wears his own clothes, not team shorts.
            shorts: 0,
        },
    ) {
        Ok(a) => a,
        Err(e) => {
            warn!("baller preview: {e}");
            return;
        }
    };
    let image = state
        .image
        .get_or_insert_with(|| {
            images.add(Image::new_target_texture(
                398,
                544,
                bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                None,
            ))
        })
        .clone();
    let player = street_render::player::spawn(
        &mut commands,
        &assets,
        // The game's animations face the other way from the bind pose: a
        // half turn faces the camera (the viewer's spin at 0).
        Transform::from_translation(STAGE)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        &mut meshes,
        &mut materials,
        &mut images,
        &mut bindposes,
    );
    commands.entity(player).insert((
        street_render::animation::Animator::new(clips.get(viewer_pose(framing))),
        PreviewPart,
        DespawnOnExit(crate::Screen::Menu),
    ));
    let hips = assets.hip_height();
    // Bone 21's rest place above the hips (the head camera's height,
    // `FUN_002dc0e0` mode 0); the model's origin is at the hips.
    let head = assets
        .skeleton
        .bones
        .get(21)
        .map(|b| Mat4::from_cols_array(&b.inverse_bind).inverse().w_axis.y * assets.scale)
        .unwrap_or(hips * 0.8);
    state.model = Some((hips, head));
    let (at, tilt) = viewer_camera(framing, hips + head);
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        Projection::from(PerspectiveProjection {
            fov: viewer_fov(),
            aspect_ratio: VIEWPORT.x / VIEWPORT.y,
            near: NEAR,
            far: FAR,
            ..default()
        }),
        bevy::camera::RenderTarget::from(image.clone()),
        Transform::from_translation(STAGE + at)
            .looking_to(Vec3::new(0.0, tilt.sin(), -tilt.cos()), Vec3::Y),
        PreviewPart,
        DespawnOnExit(crate::Screen::Menu),
    ));
    commands.spawn((
        ImageNode::new(image),
        Node {
            position_type: PositionType::Absolute,
            // The viewer's viewport, (66, 134) 199 by 272 on the 640x480
            // screen (the backdrop's picture panel).
            left: Val::Percent(66.0 / 6.4),
            top: Val::Percent(134.0 / 4.8),
            width: Val::Percent(199.0 / 6.4),
            height: Val::Percent(272.0 / 4.8),
            ..default()
        },
        PreviewPart,
        DespawnOnExit(crate::Screen::Menu),
    ));
}
