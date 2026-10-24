//! Choose Team on the game's own layout (class 26, blob 7): screen classes
//! 27 (`FUN_00270098`, one side of people) and 28 (`FUN_00270ee0`, people
//! on both sides) over the shared constructor `FUN_0026fb00` (layout 26,
//! backdrop 7) and build `FUN_0026fb88`. The trace is in
//! `docs/FRONTEND_RECOVERY.md`, "Choose Team".
//!
//! Two panels, AWAY on the left (list 0) and HOME on the right (list 33),
//! each framed by four line pieces and a turned corner piece in its
//! controller's colour, headed by a tab with the side's word. A panel
//! shows the highlighted team's large logo at 0.9, its city and name, its
//! Offense, Defense and Overall meters, and the teams as a grid of logo
//! tiles four across, two rows at a time: a blank, All-Stars and Old
//! School on double tiles, a blank, the created teams, Team Lunatics once
//! unlocked, the 29 league teams, Random League Team, then blanks to the row's
//! end (`FUN_0026e5c8`, `FUN_0026e3e0`, `FUN_0026e558`, `FUN_0026e7f0`).
//! A cursor frame sits on the highlighted tile; "Ready" covers a grid once
//! its side has chosen; the computer's side is darkened until a person
//! picks for it (`FUN_00270428`), and its frame takes the sheet's own
//! grey (`FUN_002103e8`).
//!
//! The rewrite picks the home team then the away team on its own pages
//! (then each side's players), so each page draws the panel it is picking
//! on with the cursor and the other as it stands.

use bevy::prelude::*;
use street_assets::db::Skill;

use crate::{
    art::Art,
    look::{self, Align, Look, Piece, colours, fonts},
    setup::Disc,
};

/// The screen's backdrop (`FUN_0026fb00`: `FUN_0026e1a8(screen, 0x1a, 7)`).
pub const BACKDROP: usize = 7;

/// The League Challenge's Choose Team, class 0x19 (`FUN_0026f4f0`):
/// `FUN_0026e1a8(screen, 0x1a, 0)`, the same layout 26 over backdrop 0,
/// whose picture of league players fills the right panel's place once
/// `FUN_0026f7c0` hides every element from element 0x1b's corner on (the
/// right panel, its word 0x42 and "Ready" 0x14 with them).
pub const LEAGUE_BACKDROP: usize = 0;

/// A grid cell: a team, or a blank the constructor registers as a hidden
/// placeholder (`FUN_00221770`, disabled and hidden). The double tiles'
/// frame is `wide` (`FUN_00222eb8`: All-Stars frame 1, or 2 under a game
/// setting bit; Old School frame 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub team: Option<usize>,
    pub wide: Option<usize>,
}

/// The teams the constructor names: Old School (38) and All-Stars (39) on
/// the double tiles, Team Lunatics (47) when unlocked, the league's 29 (0..28)
/// and Random League Team (48).
pub const OLD_SCHOOL: usize = 38;
pub const LEAGUE_STARS: usize = 39;
pub const LUNATICS: usize = 47;
pub const RANDOM: usize = 48;
pub const LEAGUE_TEAMS: usize = 29;
/// Created teams join the database's after the disc's 49.
const DISC_TEAMS: usize = 49;

pub const COLUMNS: usize = 4;
/// Two rows of nodes are authored (eight a list); the rest scroll.
pub const ROWS_SHOWN: usize = 2;

/// The grid, in the constructor's order.
pub fn grid(disc: &Disc, story: &street_sim::story::Story) -> Vec<Cell> {
    let blank = Cell {
        team: None,
        wide: None,
    };
    let mut out = vec![
        blank,
        Cell {
            team: Some(LEAGUE_STARS),
            wide: Some(0),
        },
        Cell {
            team: Some(OLD_SCHOOL),
            wide: Some(0),
        },
        blank,
    ];
    teams(disc, lunatics(disc, story), &mut out);
    out
}

/// Class 0x19's grid (`FUN_0026f568`): no row of legend teams, the rest
/// as [`grid`]'s.
pub fn league_grid(disc: &Disc, story: &street_sim::story::Story) -> Vec<Cell> {
    let mut out = Vec::new();
    teams(disc, lunatics(disc, story), &mut out);
    out
}

/// "Join a team"'s grid (class 0x4d, `FUN_00271730`): the league's 29
/// (`FUN_0026e558`) and blanks to the row's end (`FUN_0026e7f0`).
pub fn join_grid() -> Vec<Cell> {
    let mut out: Vec<Cell> = (0..LEAGUE_TEAMS)
        .map(|t| Cell {
            team: Some(t),
            wide: None,
        })
        .collect();
    while !out.len().is_multiple_of(COLUMNS) {
        out.push(Cell {
            team: None,
            wide: None,
        });
    }
    out
}

/// `FUN_001ed118`: Team Lunatics joins the grid once the Rewards card 2
/// (`XDBCARD`, `FUN_001d2438(card, 2)`) is the save's (`FUN_001e0398`),
/// or at once if it costs nothing (`FUN_001d26e8`).
pub fn lunatics(disc: &Disc, story: &street_sim::story::Story) -> bool {
    disc.cards
        .get(2)
        .is_some_and(|c| c.cost < 1 || story.owned.contains(&(c.id as u16)))
}

/// The created teams (`FUN_0026e3e0`: the save's list), Team Lunatics
/// when `lunatics` ([`lunatics`]), the league's 29 (`FUN_0026e558`), Random
/// league Team, and blanks to the row's end.
fn teams(disc: &Disc, lunatics: bool, out: &mut Vec<Cell>) {
    let playable = |t: usize| disc.teams.get(t).is_some_and(|team| team.roster.len() >= 3);
    let team = |t: usize| Cell {
        team: Some(t),
        wide: None,
    };
    let blank = Cell {
        team: None,
        wide: None,
    };
    out.extend(
        (DISC_TEAMS..disc.teams.len())
            .filter(|&t| playable(t))
            .map(team),
    );
    if lunatics && playable(LUNATICS) {
        out.push(team(LUNATICS));
    }
    out.extend((0..LEAGUE_TEAMS).map(team));
    out.push(team(RANDOM));
    // `FUN_0026e7f0` pads the last row with hidden placeholders.
    while !out.len().is_multiple_of(COLUMNS) {
        out.push(blank);
    }
}

/// Where `team` sits in the grid (the first league team when it is not
/// there).
pub fn position(grid: &[Cell], team: usize) -> usize {
    grid.iter()
        .position(|c| c.team == Some(team))
        .or_else(|| grid.iter().position(|c| c.team == Some(0)))
        .unwrap_or(0)
}

/// A step across (`dx`) or down (`dy`) the grid from `cursor`: the nodes
/// link to their neighbours (`FUN_0021f8f0` follows a link and skips an
/// unavailable node onward in the same direction), the edges of a row and
/// the ends of the grid stop (the top row's up and the bottom row's down
/// scroll the list, which here is the same as not moving).
pub fn step(grid: &[Cell], cursor: usize, dx: i32, dy: i32) -> usize {
    let mut at = cursor;
    loop {
        let (col, row) = ((at % COLUMNS) as i32, (at / COLUMNS) as i32);
        let (col, row) = (col + dx, row + dy);
        if col < 0 || col >= COLUMNS as i32 || row < 0 {
            return cursor;
        }
        let next = row as usize * COLUMNS + col as usize;
        if next >= grid.len() {
            return cursor;
        }
        if grid[next].team.is_some() {
            return next;
        }
        at = next;
    }
}

/// The first row shown so that the cursor's row is in view, moving from
/// the row shown before.
pub fn scroll(first: usize, cursor: usize) -> usize {
    let row = cursor / COLUMNS;
    if row < first {
        row
    } else if row >= first + ROWS_SHOWN {
        row + 1 - ROWS_SHOWN
    } else {
        first
    }
}

/// The team a cell picks: Random League Team draws one of the 29
/// (`FUN_0026ef88`: `rand() % 29`; `roll` stands in for the draw).
pub fn pick(grid: &[Cell], cursor: usize, roll: u32) -> Option<usize> {
    let team = grid.get(cursor)?.team?;
    Some(if team == RANDOM {
        roll as usize % LEAGUE_TEAMS
    } else {
        team
    })
}

/// A team's meters (`FUN_001ec028`, each 0..5, or 6 for the two legend
/// teams): offense (4) is each player's rounded mean of the Shots, Dunks
/// and Handles bars, defense (5) of the Blocks and Steals bars, averaged
/// over the roster as `round((sum + n - 1 + 0.5) / n)`; overall (6) is
/// `round((offense + defense + 0.5) / 2)`.
pub fn meters(disc: &Disc, team: usize) -> [u8; 3] {
    if team == OLD_SCHOOL || team == LEAGUE_STARS {
        return [6, 6, 6];
    }
    let Some(t) = disc.teams.get(team) else {
        return [0, 0, 0];
    };
    let bar = |r: usize, s: Skill| {
        disc.players.get(r).map_or(0, |p| {
            crate::progression_trace::skill_bar(s, p.ratings[s.rating()])
        }) as f32
    };
    let roster: Vec<usize> = t
        .roster
        .iter()
        .copied()
        .filter(|&r| r < disc.players.len())
        .collect();
    let n = roster.len() as f32;
    if roster.is_empty() {
        return [0, 0, 0];
    }
    let average = |per: &dyn Fn(usize) -> f32| -> u8 {
        let sum: f32 = roster.iter().map(|&r| per(r)).sum();
        (((sum + n - 1.0 + 0.5) / n).round() as u8).min(5)
    };
    let offense = average(&|r| {
        ((bar(r, Skill::Shots) + bar(r, Skill::Dunks) + bar(r, Skill::Handles) + 0.5) / 3.0).round()
    });
    let defense =
        average(&|r| ((bar(r, Skill::Blocks) + bar(r, Skill::Steals) + 0.5) / 2.0).round());
    let overall = (((f32::from(offense) + f32::from(defense) + 0.5) / 2.0).round() as u8).min(5);
    [offense, defense, overall]
}

/// How a panel stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    /// Its side is choosing: the cursor on `cursor`, the grid scrolled to
    /// `first` row; `player` the controller colouring it (none: the sheet's
    /// grey).
    Picking {
        cursor: usize,
        first: usize,
        player: Option<usize>,
    },
    /// Its side has chosen ("Ready" over the grid).
    Ready { player: Option<usize> },
    /// The computer's, not yet picked for: darkened, with the badge.
    Cpu,
}

/// One side: its team and panel.
#[derive(Clone, Copy, Debug)]
pub struct Side {
    pub team: usize,
    pub panel: Panel,
}

/// A panel's geometry: the left one's layout elements, and the right one
/// 271 further on for most pieces.
struct Geometry {
    /// Lines 21..24 (27..30): x, y, width, height.
    lines: [(f32, f32, f32, f32); 4],
    /// The corner piece 26 (32).
    corner: Vec2,
    /// The heading tab 25 (31) and the word 65 (66).
    tab: Vec2,
    word: Vec2,
    /// The large logo 14 (47), centred, and the name 10 (43), by its top's
    /// middle.
    logo: Vec2,
    name: Vec2,
    /// The meters 60, 12, 61 (63, 45, 64): left edges.
    meters: [f32; 3],
    /// "Ready" 20 (53), centred.
    ready: Vec2,
    /// The grid nodes' x, and the two rows' y.
    nodes_x: [f32; 4],
    /// The list's scroll arrows: the deleted elements 19 and 18 (52, 51).
    arrows: [Vec2; 2],
    /// The darkening rectangle `FUN_00270428` builds, 263 by 321.
    overlay: Vec2,
    /// Its badge.
    badge: Vec2,
}

const NODES_Y: [f32; 2] = [321.0, 373.0];
const METER_Y: f32 = 231.0;
const METER_LABEL_Y: f32 = 266.0;
const METER_LABELS_X: [[f32; 3]; 2] = [[96.0, 182.0, 263.0], [367.0, 453.0, 534.0]];
/// The meter labels' colour (the layout's prop 10, 0xff8394df).
const METER_LABEL: [u8; 4] = [0x83, 0x94, 0xdf, 0xff];
/// The large logo's scale (`FUN_0026f1c8`, 0.9) and the name's.
const LOGO_SCALE: f32 = 0.9;
/// A double tile's place from its node: All-Stars half a cell left, Old
/// School half a cell right, so the two cover cells 0-1 and 2-3. The
/// cursor's offsets (`FUN_0026e9c8`: `0x4d1200` and `0x4d1208`, read from
/// RAM) are -28 and +28; the tiles' own `+0xd4` (`FUN_0026e5c8`: `0x4d1210`,
/// `0x4d1218`) hold -14 and +14, and the grabs show them under the cursor.
fn wide_shift(team: usize) -> f32 {
    if team == LEAGUE_STARS { -28.0 } else { 28.0 }
}

const GEOMETRY: [Geometry; 2] = [
    Geometry {
        lines: [
            (53.0, 93.0, 4.0, 306.0),
            (53.0, 93.0, 262.0, 4.0),
            (311.0, 93.0, 4.0, 324.0),
            (72.0, 413.0, 240.0, 4.0),
        ],
        corner: Vec2::new(52.0, 396.0),
        tab: Vec2::new(218.0, 74.0),
        word: Vec2::new(276.0, 78.0),
        logo: Vec2::new(183.0, 149.0),
        name: Vec2::new(183.0, 196.0),
        meters: [70.0, 157.0, 237.0],
        ready: Vec2::new(192.0, 345.0),
        nodes_x: [110.0, 165.0, 220.0, 275.0],
        arrows: [Vec2::new(69.0, 304.0), Vec2::new(69.0, 387.0)],
        overlay: Vec2::new(53.0, 97.0),
        badge: Vec2::new(53.0, 401.0),
    },
    Geometry {
        lines: [
            (324.0, 93.0, 4.0, 306.0),
            (324.0, 93.0, 261.0, 4.0),
            (581.0, 93.0, 4.0, 324.0),
            (342.0, 413.0, 240.0, 4.0),
        ],
        corner: Vec2::new(323.0, 396.0),
        tab: Vec2::new(323.0, 74.0),
        word: Vec2::new(379.0, 78.0),
        logo: Vec2::new(454.0, 149.0),
        name: Vec2::new(454.0, 196.0),
        meters: [341.0, 428.0, 508.0],
        ready: Vec2::new(443.0, 345.0),
        nodes_x: [362.0, 417.0, 472.0, 527.0],
        arrows: [Vec2::new(567.0, 304.0), Vec2::new(567.0, 387.0)],
        overlay: Vec2::new(323.0, 97.0),
        badge: Vec2::new(323.0, 401.0),
    },
];
const OVERLAY_SIZE: Vec2 = Vec2::new(263.0, 321.0);

/// A kind-20 meter (`FUN_002182f0`, set by `FUN_00218458`) at `cell`, its
/// element's corner: the crown (54x17) there and the bar (51x8) 2 right and
/// 17 down under it (`FUN_002185c0`, the meter's move), so the two fill the
/// 60x25 cell from its corner. The bar's frame 10 is in the sheet's own
/// palette (no segment lit, `FUN_002103e8`); levels 1..5 draw it in
/// `XPAL_FE` sheet 1's palette of the level (`0x467180`), which lights that
/// many segments; a full meter (6) wears frames 1 and 11 (`FUN_00218628`
/// then turns its crown through `0x467198`'s frames, not drawn here).
pub fn meter(pm: &look::PrematchArt, cell: Vec2, level: u8) -> [Piece; 2] {
    let full = level >= 6;
    let frame = cell;
    let fill = cell + Vec2::new(2.0, 17.0);
    let fill_image = match level {
        0 => &pm.meter[2],
        1..=5 => &pm.meter_fill_levels[usize::from(level) - 1],
        _ => &pm.meter[3],
    };
    [
        Piece::new(
            pm.meter[usize::from(full)].clone(),
            Rect::from_corners(frame, frame + pm.meter_frame_size),
        ),
        Piece::new(
            fill_image.clone(),
            Rect::from_corners(fill, fill + pm.meter_fill_size),
        ),
    ]
}

/// A side's place: the away side on the left, the home side on the right
/// (rewrite side 0 is home).
fn panel_of(side: usize) -> usize {
    if side == 0 { 1 } else { 0 }
}

/// The screen's pieces for the two sides (by rewrite side: home, away).
#[allow(clippy::too_many_arguments)]
pub fn pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    art: &Art,
    disc: &Disc,
    grid: &[Cell],
    sides: [Side; 2],
    ticks: u64,
) -> Vec<Piece> {
    let mut out = Vec::new();
    for (side, state) in sides.into_iter().enumerate() {
        let word = look
            .text("chooseteam.txt", if side == 1 { 0 } else { 1 })
            .unwrap_or(if side == 1 { "AWAY" } else { "HOME" })
            .to_owned();
        panel(
            &mut out,
            (look, labels, images, art, disc),
            grid,
            panel_of(side),
            &word,
            state,
            ticks,
        );
    }
    out
}

/// Class 0x19's pieces: the left panel only, AWAY, for the person whose
/// challenge it is (`FUN_0026f568`; `FUN_0026eff8(screen, 0x41, owner)`).
#[allow(clippy::too_many_arguments)]
pub fn league_pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    art: &Art,
    disc: &Disc,
    grid: &[Cell],
    side: Side,
    ticks: u64,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let word = look.text("chooseteam.txt", 0).unwrap_or("AWAY").to_owned();
    panel(
        &mut out,
        (look, labels, images, art, disc),
        grid,
        0,
        &word,
        side,
        ticks,
    );
    out
}

/// "Join a team"'s panel (class 0x4d, layout 77): the right one, for the
/// person whose challenge it was (`FUN_0026eff8(screen, 0x41, owner)`),
/// its tab without a word (element 65 carries none).
#[allow(clippy::too_many_arguments)]
pub fn join_pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    art: &Art,
    disc: &Disc,
    grid: &[Cell],
    side: Side,
    ticks: u64,
) -> Vec<Piece> {
    let mut out = Vec::new();
    panel(
        &mut out,
        (look, labels, images, art, disc),
        grid,
        1,
        "",
        side,
        ticks,
    );
    out
}

/// One panel's pieces at `place` (0 left, 1 right) under `word`.
fn panel(
    out: &mut Vec<Piece>,
    (look, labels, images, art, disc): (&Look, &mut look::Labels, &mut Assets<Image>, &Art, &Disc),
    grid: &[Cell],
    place: usize,
    word: &str,
    state: Side,
    ticks: u64,
) {
    let Some(pm) = look.prematch.as_ref() else {
        return;
    };
    let mut text = |out: &mut Vec<Piece>, font: usize, s: &str, colour, at: Vec2, align| {
        if let Some(p) = labels.text(look, images, font, s, colour, None, at, align) {
            out.push(p);
        }
    };
    {
        let g = &GEOMETRY[place];
        let player = match state.panel {
            Panel::Picking { player, .. } | Panel::Ready { player } => player,
            Panel::Cpu => None,
        };
        // A person's controller swaps the frame's palette to `XPAL_FE`
        // sheet 0's (`FUN_0026f0c0`, `FUN_00210368(w, 0, k)`); the
        // computer's keeps the sheet's own (`FUN_002103e8`).
        let variant = player.map_or(0, |p| p.min(3) + 1);
        for &(x, y, w, h) in &g.lines {
            out.push(Piece::new(
                pm.line[variant].clone(),
                Rect::new(x, y, x + w, y + h),
            ));
        }
        out.push(Piece::new(
            pm.corner[variant].clone(),
            Rect::from_corners(g.corner, g.corner + pm.corner_size),
        ));
        out.push(Piece::new(
            pm.tab_wide[variant].clone(),
            Rect::from_corners(g.tab, g.tab + pm.tab_wide_size),
        ));
        // The side's word (`chooseteam.txt` 0, 1), font 0, pale; black on
        // 2P's yellow tab (`FUN_0026eff8`: `DAT_003f7f88`).
        let colour = if player == Some(1) {
            [0, 0, 0, 0xff]
        } else {
            colours::PALE
        };
        text(out, fonts::SMALL, word, colour, g.word, Align::Centre);
        // The highlighted team's large logo, name and meters.
        let shown = match state.panel {
            Panel::Picking { cursor, .. } => {
                grid.get(cursor).and_then(|c| c.team).unwrap_or(state.team)
            }
            _ => state.team,
        };
        if let Some(logo) = disc
            .teams
            .get(shown)
            .and_then(|t| usize::try_from(t.logo).ok())
            .and_then(|l| art.logos_big.get(l))
            .and_then(|l| l.as_ref())
        {
            let half = logo.1 * LOGO_SCALE / 2.0;
            out.push(Piece::new(
                logo.0.clone(),
                Rect::from_corners(g.logo - half, g.logo + half),
            ));
        }
        if let Some(t) = disc.teams.get(shown) {
            // Elements 10 and 0x2b anchor their text by its top's middle
            // (align 1), scaled about that point.
            let name = t.full_name();
            let from = out.len();
            text(
                out,
                fonts::LIST,
                &name,
                colours::CREAM,
                g.name,
                Align::Centre,
            );
            for p in &mut out[from..] {
                let half = p.rect.half_size() * LOGO_SCALE;
                let centre = g.name + (p.rect.center() - g.name) * LOGO_SCALE;
                p.rect = Rect::from_center_half_size(centre, half);
            }
        }
        // The values show on every panel (`FUN_00218408` sets `+0xba`); the
        // `+0xb4` `FUN_00270428` clears on the computer's is not the value
        // (`chooseteam.png`: its full crowns under the dark).
        let levels = meters(disc, shown);
        for (k, &x) in g.meters.iter().enumerate() {
            let level = levels[k];
            out.extend(meter(pm, Vec2::new(x, METER_Y), level));
            let label = look
                .text("common_fe.txt", [1, 3, 2][k])
                .unwrap_or(["Offense", "Defense", "Overall"][k]);
            let line = look
                .fonts
                .get(fonts::LIST_SMALL)
                .map_or(18.0, |f| f.line_height as f32);
            text(
                out,
                fonts::LIST_SMALL,
                label,
                METER_LABEL,
                Vec2::new(METER_LABELS_X[place][k], METER_LABEL_Y - line / 2.0),
                Align::Centre,
            );
        }
        // The grid: the rows from `first`, each tile centred on its node
        // (`FUN_00222420`), the double tiles half a cell left.
        let (first, cursor) = match state.panel {
            Panel::Picking { cursor, first, .. } => (first, Some(cursor)),
            Panel::Cpu | Panel::Ready { .. } => (position(grid, state.team) / COLUMNS, None),
        };
        let first = first.min((grid.len() / COLUMNS).saturating_sub(ROWS_SHOWN));
        // A list that is off (ready, or the computer's before it is picked
        // for) draws its tiles at half alpha.
        let dim = if matches!(state.panel, Panel::Ready { .. } | Panel::Cpu) {
            0.5
        } else {
            1.0
        };
        let mut cursor_rect = None;
        for (row, &node_y) in NODES_Y.iter().enumerate().take(ROWS_SHOWN) {
            for col in 0..COLUMNS {
                let index = (first + row) * COLUMNS + col;
                let Some(cell) = grid.get(index) else {
                    continue;
                };
                let Some(team) = cell.team else {
                    continue;
                };
                let node = Vec2::new(g.nodes_x[col], node_y);
                let logo = disc
                    .teams
                    .get(team)
                    .and_then(|t| usize::try_from(t.logo).ok());
                let (image, size, centre) = match cell.wide {
                    Some(frame) => {
                        let Some((frames, size)) = logo
                            .and_then(|l| art.logos_wide.get(l))
                            .and_then(|w| w.as_ref())
                        else {
                            continue;
                        };
                        (
                            frames[frame.min(1)].clone(),
                            *size,
                            node + Vec2::new(wide_shift(team), 0.0),
                        )
                    }
                    None => {
                        let Some(image) = logo.and_then(|l| art.logos.get(l)) else {
                            continue;
                        };
                        (image.clone(), crate::art::LOGO_TILE, node)
                    }
                };
                let rect = Rect::from_center_size(centre, size);
                let mut p = Piece::new(image, rect);
                p.colour = if cursor.is_some() {
                    // `FUN_0026eb08` sets the list's `+0x124`: the tiles it
                    // does not focus sit at 0x9f (`FUN_00222420`), the
                    // focused one at 0xff (`FUN_002222c8`).
                    let b = if cursor == Some(index) {
                        crate::choose::FOCUSED
                    } else {
                        crate::choose::UNFOCUSED
                    };
                    Color::srgb(b, b, b)
                } else {
                    Color::WHITE.with_alpha(dim)
                };
                out.push(p);
                if cursor == Some(index) {
                    let (image, size) = if cell.wide.is_some() {
                        (pm.cursor_wide.clone(), pm.cursor_wide_size)
                    } else {
                        (pm.cursor.clone(), pm.cursor_size)
                    };
                    cursor_rect = Some((image, Rect::from_center_size(centre, size)));
                }
            }
        }
        if let Some((image, rect)) = cursor_rect {
            // The cursor's own pulse, as the row widgets'.
            let mut p = Piece::new(image, rect);
            p.colour = Color::WHITE.with_alpha(f32::from(look::row_pulse(ticks)) / 255.0);
            out.push(p);
        }
        // The list's scroll arrows, where more rows lie.
        let rows = grid.len().div_ceil(COLUMNS);
        if matches!(state.panel, Panel::Picking { .. }) {
            let half = pm.arrow_size / 2.0;
            if first > 0 {
                out.push(Piece::new(
                    pm.arrow_up.clone(),
                    Rect::from_corners(g.arrows[0] - half, g.arrows[0] + half),
                ));
            }
            if first + ROWS_SHOWN < rows {
                out.push(Piece::new(
                    pm.arrow_down.clone(),
                    Rect::from_corners(g.arrows[1] - half, g.arrows[1] + half),
                ));
            }
        }
        match state.panel {
            Panel::Ready { .. } => {
                let ready = look.text("common_fe.txt", 10).unwrap_or("Ready");
                let line = look
                    .fonts
                    .get(fonts::TITLE)
                    .map_or(30.0, |f| f.line_height as f32);
                text(
                    out,
                    fonts::TITLE,
                    ready,
                    colours::CREAM,
                    g.ready - Vec2::new(0.0, line / 2.0),
                    Align::Centre,
                );
            }
            Panel::Cpu => {
                let mut dark = Piece::new(
                    Handle::default(),
                    Rect::from_corners(g.overlay, g.overlay + OVERLAY_SIZE),
                );
                // `FUN_00233e00(.., 0x7f000000)`: black at half alpha.
                dark.colour = Color::srgba(0.0, 0.0, 0.0, look::black_alpha(127.0 / 255.0));
                out.push(dark);
                out.push(Piece::new(
                    pm.badge.clone(),
                    Rect::from_corners(g.badge, g.badge + pm.badge_size),
                ));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(teams: &[Option<usize>]) -> Vec<Cell> {
        teams
            .iter()
            .map(|&team| Cell { team, wide: None })
            .collect()
    }

    #[test]
    fn steps_follow_the_rows_and_skip_blanks_without_wrapping() {
        // The authored first row: blank, All-Stars, Old School, blank.
        let g = cells(&[
            None,
            Some(39),
            Some(38),
            None,
            Some(0),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            None,
            None,
            None,
        ]);
        assert_eq!(
            step(&g, 1, -1, 0),
            1,
            "left of All-Stars is a blank, then the edge"
        );
        assert_eq!(step(&g, 1, 1, 0), 2);
        assert_eq!(
            step(&g, 2, 1, 0),
            2,
            "right of Old School is a blank, then the edge"
        );
        assert_eq!(step(&g, 1, 0, 1), 5);
        assert_eq!(step(&g, 5, 0, -1), 1);
        assert_eq!(
            step(&g, 4, 0, -1),
            4,
            "up from Atlanta: a blank, then the top"
        );
        assert_eq!(
            step(&g, 7, 0, 1),
            7,
            "down from Cleveland: a blank, then the end"
        );
        assert_eq!(step(&g, 5, 0, 1), 5);
        assert_eq!(step(&g, 8, 1, 0), 8);
        assert_eq!(step(&g, 4, 0, 0), 4);
    }

    #[test]
    fn the_window_scrolls_a_row_at_a_time() {
        assert_eq!(scroll(0, 3), 0);
        assert_eq!(scroll(0, 7), 0);
        assert_eq!(scroll(0, 8), 1);
        assert_eq!(scroll(1, 8), 1);
        assert_eq!(scroll(3, 4), 1);
        assert_eq!(scroll(1, 1), 0);
    }

    #[test]
    fn the_league_challenges_grid_has_no_legend_teams_row() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let story = street_sim::story::Story::default();
        let g = league_grid(&disc, &story);
        assert!(g.len().is_multiple_of(COLUMNS));
        assert!(
            g.iter()
                .all(|c| c.wide.is_none() && !matches!(c.team, Some(OLD_SCHOOL | LEAGUE_STARS)))
        );
        // The disc's created teams come first; with none, Atlanta.
        let first_league = g.iter().position(|c| c.team == Some(0)).unwrap();
        assert!(
            g[..first_league]
                .iter()
                .all(|c| c.team.is_some_and(|t| t >= DISC_TEAMS || t == LUNATICS))
        );
        let teams: Vec<usize> = g[first_league..].iter().filter_map(|c| c.team).collect();
        assert_eq!(
            teams[..LEAGUE_TEAMS],
            (0..LEAGUE_TEAMS).collect::<Vec<_>>()[..]
        );
        assert_eq!(teams[LEAGUE_TEAMS], RANDOM);
        // The same cells as Choose Team's after its first row.
        assert_eq!(grid(&disc, &story)[COLUMNS..], g[..]);
        // Team Lunatics once card 2 is bought.
        let mut bought = story.clone();
        bought.owned.push(2);
        assert_eq!(league_grid(&disc, &bought)[0].team, Some(LUNATICS));
        assert_eq!(g[0].team, Some(0));
    }

    #[test]
    fn random_draws_one_of_the_league_teams() {
        let g = cells(&[Some(RANDOM), Some(5), None, None]);
        assert_eq!(pick(&g, 0, 0), Some(0));
        assert_eq!(pick(&g, 0, 30), Some(1));
        assert_eq!(pick(&g, 1, 30), Some(5));
        assert_eq!(pick(&g, 2, 0), None);
        assert_eq!(position(&g, 5), 1);
    }

    #[test]
    fn the_panels_mirror_the_layout_271_apart() {
        let [l, r] = &GEOMETRY;
        for k in 0..4 {
            assert_eq!(r.nodes_x[k] - l.nodes_x[k], 252.0);
        }
        assert_eq!(r.logo.x - l.logo.x, 271.0);
        assert_eq!(r.name - l.name, Vec2::new(271.0, 0.0));
        assert_eq!(r.meters[0] - l.meters[0], 271.0);
        assert_eq!(r.overlay - l.overlay, Vec2::new(270.0, 0.0));
        assert_eq!(panel_of(0), 1);
        assert_eq!(panel_of(1), 0);
        // All-Stars (node 1) over cells 0-1, Old School (node 2) over
        // cells 2-3: within a pixel of the middle between their nodes.
        let stars = l.nodes_x[1] + wide_shift(LEAGUE_STARS);
        let old = l.nodes_x[2] + wide_shift(OLD_SCHOOL);
        assert!((stars - (l.nodes_x[0] + l.nodes_x[1]) / 2.0).abs() <= 1.0);
        assert!((old - (l.nodes_x[2] + l.nodes_x[3]) / 2.0).abs() <= 1.0);
    }
}
