//! The League Challenge's Choose Region (screen class 0x29, layout blob 12,
//! backdrop 6): the constructor `FUN_0026b408`, the build `FUN_0026b760`,
//! the refreshes `FUN_0026c430` (the region list), `FUN_0026c5c8` (the
//! team list) and `FUN_0026c6f8` (the panels). The trace is in
//! `docs/FRONTEND_RECOVERY.md`, "Choose Region".
//!
//! One screen in two states. Picking a region: the region's court picture
//! (`XACOURTS`, node 14) between the list's arrows (at (76, 327), 214
//! apart), its name ("North West" .. "South East", font 3) over the
//! court's, and on the right the "Vol. 2" logo over the meters at nought
//! and the region's teams' tiles (their logo sheets' image 1), all
//! disabled, a check on each team beaten and the boss's card (the
//! question mark on the star until the boss is beaten). Picking a team:
//! the focused team's big logo and name, its meters, the tiles live with
//! the cursor, and "Play against the %1S %2S." for help.

use bevy::prelude::*;
use street_assets::font::Layout;

use crate::{
    art::Art,
    choose,
    look::{self, Look, Piece, colours, fonts},
    setup::Disc,
};

pub const CLASS: u32 = 41;
/// The court picture's node, the list's arrows.
const PICTURE: u32 = 14;
const ARROWS: [Vec2; 2] = [Vec2::new(76.0, 327.0), Vec2::new(290.0, 327.0)];
/// The region's and the court's names, the team's logo and name.
const REGION: u32 = 22;
const COURT: u32 = 26;
const LOGO: u32 = 5;
const TEAM_NAME: u32 = 27;
/// The meters (Offense, Defense, Overall) and their words.
const METERS: [u32; 3] = [9, 8, 10];
const WORDS: [u32; 3] = [0, 1, 2];
/// The tiles' nodes (15..20, left-anchored on their middles), the boss's
/// (21), the checks (0x22..0x27 top-left, 0x28 centred at 1.5).
const TILES: [u32; 6] = [15, 16, 17, 18, 19, 20];
const BOSS: u32 = 21;
const CHECKS: [u32; 6] = [0x22, 0x23, 0x24, 0x25, 0x26, 0x27];
const BOSS_CHECK: u32 = 0x28;
/// The big court picture (`XACOURTS` frame 0).
const PICTURE_SIZE: Vec2 = Vec2::new(248.0, 174.0);
/// A logo sheet's tile, image 1's frame 0 of its 64x64.
const TILE: Rect = Rect {
    min: Vec2::ZERO,
    max: Vec2::new(55.0, 53.0),
};
/// A disabled piece's brightness: measured, a tile, the court picture and
/// the boss's card keep 0.30..0.33 of their light disabled (the list's
/// 0x9f under the disabled 0x7f, 0.31); the "Vol. 2" logo (a widget of its
/// own, `FUN_0036a0d0`) 0.15.
const DISABLED: f32 = 0.31;
const VOL2: f32 = 0.15;

/// The tiles' order: three across, two down, then the boss's card; a step
/// right off the row's end goes on to the next row, off the last tile to
/// the boss (the nodes' links, props 29..32); left from the first goes to
/// the boss too.
pub fn step(at: usize, dx: i32, dy: i32, teams: usize) -> usize {
    let boss = teams;
    let to = if at == boss {
        match (dx, dy) {
            (-1, 0) => Some(teams.saturating_sub(1)),
            _ => None,
        }
    } else {
        match (dx, dy) {
            (1, 0) => Some(at + 1),
            (-1, 0) if at == 0 => Some(boss),
            (-1, 0) => Some(at - 1),
            // Up and down only reach a tile (a region of five has no sixth).
            (0, 1) if at < 3 && at + 3 < teams => Some(at + 3),
            (0, -1) if at >= 3 => Some(at - 3),
            _ => None,
        }
    };
    match to {
        Some(t) if t < teams || t == boss => t,
        _ => at,
    }
}

/// What the screen shows.
pub struct View<'a> {
    pub region: usize,
    /// The region's teams (5 or 6) and its boss's team.
    pub teams: &'a [u16],
    pub boss: u16,
    /// The region's court.
    pub court: usize,
    /// Each team beaten, then the boss.
    pub beaten: Vec<bool>,
    /// The focused team (the boss at `teams.len()`), or none while the
    /// region is picked.
    pub focus: Option<usize>,
}

fn element_at(layout: &Layout, id: u32) -> Option<Vec2> {
    choose::element(layout, id).and_then(choose::spot)
}

/// The screen's pieces over its layout (the title, the help line and the
/// Help legend come with it).
pub fn pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    art: &Art,
    disc: &Disc,
    view: &View,
    ticks: u64,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let (Some(layout), Some(pm)) = (
        look.layouts.iter().find(|l| l.class == CLASS),
        look.prematch.as_ref(),
    ) else {
        return out;
    };
    let picking_region = view.focus.is_none();
    let tint = |on: bool| {
        let b = if on { 1.0 } else { DISABLED };
        Color::srgb(b, b, b)
    };
    // The region: its court's picture, its arrows while it is picked, its
    // name and its court's.
    // The region list is disabled while a team is picked.
    if let (Some(at), Some(Some(court))) = (element_at(layout, PICTURE), art.courts.get(view.court))
        && let Some(picture) = &court.picture
    {
        let mut p = Piece::new(picture.clone(), Rect::from_center_size(at, PICTURE_SIZE));
        p.colour = tint(picking_region);
        out.push(p);
    }
    for (k, &at) in ARROWS.iter().enumerate() {
        let image = if k == 0 {
            &pm.arrow_left
        } else {
            &pm.arrow_right
        };
        let mut p = Piece::new(
            image.clone(),
            Rect::from_center_size(at, pm.arrow_side_size),
        );
        p.colour = tint(picking_region);
        out.push(p);
    }
    let text = |out: &mut Vec<Piece>,
                labels: &mut look::Labels,
                images: &mut Assets<Image>,
                id: u32,
                s: &str,
                bright: bool| {
        if let Some(e) = choose::element(layout, id)
            && let Some(at) = choose::spot(e)
        {
            let font = e.props.get(&13).map_or(fonts::LIST, |&f| f as usize);
            let mut colour = e
                .props
                .get(&10)
                .map_or(colours::CREAM, |&c| street_assets::font::argb(c));
            if !bright {
                for c in &mut colour[..3] {
                    *c = (u16::from(*c) * 0x7f / 0xff) as u8;
                }
            }
            choose::place(out, labels, look, images, font, s, colour, at, e.align);
        }
    };
    let region = look.text("choosergn.txt", 7 + view.region).unwrap_or("");
    text(&mut out, labels, images, REGION, region, true);
    if let Some(court) = disc.courts.get(view.court) {
        text(&mut out, labels, images, COURT, &court.name, true);
    }
    // The right panel: the Vol. 2 logo, or the focused team's.
    let shown = view.focus.and_then(|k| {
        view.teams
            .get(k)
            .copied()
            .or((k == view.teams.len()).then_some(view.boss))
    });
    if let Some(at) = element_at(layout, LOGO) {
        match shown {
            // The logo widget (0x8c9) is disabled: half brightness.
            None => {
                if let Some((image, size)) = &pm.vol2 {
                    let mut p = Piece::new(image.clone(), Rect::from_center_size(at, *size));
                    p.colour = Color::srgb(VOL2, VOL2, VOL2);
                    out.push(p);
                }
            }
            Some(team) => {
                if let Some((image, size)) = disc
                    .teams
                    .get(usize::from(team))
                    .and_then(|t| usize::try_from(t.logo).ok())
                    .and_then(|l| art.logos_big.get(l))
                    .and_then(|l| l.as_ref())
                {
                    out.push(Piece::new(image.clone(), Rect::from_center_size(at, *size)));
                }
            }
        }
    }
    if let Some(t) = shown.and_then(|t| disc.teams.get(usize::from(t))) {
        text(&mut out, labels, images, TEAM_NAME, &t.full_name(), true);
    }
    let levels = shown.map_or([0; 3], |t| crate::chooseteam::meters(disc, usize::from(t)));
    for (k, &id) in METERS.iter().enumerate() {
        if let Some(at) = element_at(layout, id) {
            for mut p in crate::chooseteam::meter(pm, at, levels[k]) {
                p.colour = tint(!picking_region);
                out.push(p);
            }
        }
    }
    for &id in &WORDS {
        if let Some(word) = choose::element(layout, id)
            .and_then(|e| e.props.get(&14))
            .and_then(|&t| look.text_id(t as u16))
        {
            text(&mut out, labels, images, id, word, !picking_region);
        }
    }
    // The tiles, the checks, the boss's card and the cursor.
    let mut cursor = None;
    for (k, &id) in TILES.iter().enumerate() {
        let (Some(node), Some(&team)) = (element_at(layout, id), view.teams.get(k)) else {
            continue;
        };
        let rect = Rect::from_corners(
            Vec2::new(node.x, node.y - TILE.height() / 2.0),
            Vec2::new(node.x + TILE.width(), node.y + TILE.height() / 2.0),
        );
        if let Some(logo) = disc
            .teams
            .get(usize::from(team))
            .and_then(|t| usize::try_from(t.logo).ok())
            .and_then(|l| art.logos.get(l))
        {
            let mut p = Piece::part(logo.clone(), TILE, rect);
            p.colour = tint(!picking_region);
            out.push(p);
        }
        if view.beaten.get(k) == Some(&true)
            && let Some(at) = element_at(layout, CHECKS[k])
        {
            out.push(Piece::new(
                pm.check.clone(),
                Rect::from_corners(at, at + pm.check_size),
            ));
        }
        if view.focus == Some(k) {
            cursor = Some(rect);
        }
    }
    if let Some(node) = element_at(layout, BOSS) {
        let boss_beaten = view.beaten.get(view.teams.len()) == Some(&true);
        let (image, size) = match disc
            .teams
            .get(usize::from(view.boss))
            .and_then(|t| usize::try_from(t.logo).ok())
            .and_then(|l| art.logos_big.get(l))
            .and_then(|l| l.as_ref())
        {
            Some((logo, size)) if boss_beaten => (logo.clone(), *size),
            _ => (pm.star_card.clone(), pm.star_card_size),
        };
        let rect = Rect::from_corners(
            Vec2::new(node.x, node.y - size.y / 2.0),
            Vec2::new(node.x + size.x, node.y + size.y / 2.0),
        );
        // The boss's card stays disabled until the boss is beaten.
        let mut p = Piece::new(image, rect);
        p.colour = tint(boss_beaten && !picking_region);
        out.push(p);
        if boss_beaten && let Some(at) = element_at(layout, BOSS_CHECK) {
            out.push(Piece::new(
                pm.check.clone(),
                Rect::from_center_size(at, pm.check_size * 1.5),
            ));
        }
        if view.focus == Some(view.teams.len()) {
            cursor = Some(rect);
        }
    }
    if let Some(rect) = cursor {
        let mut c = Piece::new(pm.cursor.clone(), rect);
        c.colour = Color::WHITE.with_alpha(f32::from(look::row_pulse(ticks)) / 255.0);
        out.push(c);
    }
    out
}

/// The help line: "Select a region in which to play." (0xf0e) while a
/// region is picked, "Play against the %1S %2S." (0xf01) on a team.
pub fn help(look: &Look, disc: &Disc, team: Option<u16>) -> String {
    match team.and_then(|t| disc.teams.get(usize::from(t))) {
        None => look
            .text("choosergn.txt", 14)
            .unwrap_or("Select a region in which to play.")
            .to_owned(),
        Some(t) => look
            .text("choosergn.txt", 1)
            .unwrap_or("Play against the %1S %2S.")
            .replace("%1S", &t.city)
            .replace("%2S", &t.nickname),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tiles_link_across_down_and_to_the_boss() {
        assert_eq!(step(0, 1, 0, 6), 1);
        assert_eq!(step(2, 1, 0, 6), 3, "on to the next row");
        assert_eq!(step(5, 1, 0, 6), 6, "the last tile to the boss");
        assert_eq!(step(0, -1, 0, 6), 6, "the first tile's left to the boss");
        assert_eq!(step(6, -1, 0, 6), 5);
        assert_eq!(step(1, 0, 1, 6), 4);
        assert_eq!(step(4, 0, -1, 6), 1);
        assert_eq!(step(4, 1, 0, 5), 5, "a region of five: the boss next");
        assert_eq!(step(2, 0, 1, 5), 2, "no sixth tile");
    }
}
