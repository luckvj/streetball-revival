//! Create A Baller's tables: builds, sizes, outfits, skill bars and their
//! prices in development points, and what a player owns to wear
//! ([`Wardrobe`]). Shared by `street-game`'s editor and `street-export`'s
//! catalog (the launcher's builder offers the same).

/// Builds, in database order.
pub const BUILDS: [&str; 6] = ["Slender", "Big", "Cross Trainer", "Long", "Active", "Toned"];

/// What a build costs in development points (`0x460368` for the four men's
/// builds, `0x4603a8` for the two women's).
pub const BUILD_DP: [u32; 6] = [0, 400, 200, 300, 0, 200];

/// Each build's four sizes (M, L, XL, XXL): price, height (in) and weight (lb).
pub const SIZES: [[(u32, u8, u16); 4]; 6] = [
    [(0, 69, 200), (200, 73, 220), (300, 77, 240), (400, 81, 260)],
    [(0, 72, 210), (200, 76, 220), (300, 80, 250), (400, 84, 275)],
    [(0, 70, 185), (200, 74, 205), (300, 78, 225), (400, 82, 240)],
    [(0, 71, 175), (200, 75, 200), (300, 79, 215), (400, 83, 235)],
    [(0, 65, 130), (200, 69, 140), (300, 73, 170), (400, 77, 190)],
    [(0, 67, 140), (200, 71, 155), (300, 75, 180), (400, 79, 210)],
];

/// The sizes' names (`createplyr_build.txt` 3..6).
pub const SIZE_NAMES: [&str; 4] = ["M", "L", "XL", "XXL"];

/// Nine outfit choices per build, in the original price order. The
/// table's global clothing ids become local kit ids after subtracting
/// its first preset (`FUN_001e49c8`).
pub const OUTFITS: [[u8; 9]; 6] = [
    [4, 0, 1, 2, 3, 7, 8, 6, 5],
    [4, 2, 3, 0, 7, 1, 5, 8, 6],
    [2, 3, 0, 4, 5, 1, 7, 8, 6],
    [3, 2, 0, 5, 4, 7, 1, 8, 6],
    [8, 0, 1, 2, 3, 6, 7, 4, 5],
    [8, 0, 1, 3, 6, 2, 7, 5, 4],
];

/// The first outfit code past the nine kits (0x36): the cards' jerseys
/// (54..79 but 71's Jordan), the team uniform (80) and the Street Champ
/// jersey (81). They cost nothing and are always the player's
/// (`FUN_001e43c0`, `FUN_001e44e0`).
pub const SPECIAL_OUTFIT: u8 = 54;

/// The price of each position in [`OUTFITS`].
pub const OUTFIT_PRICES: [u32; 9] = [0, 50, 50, 100, 100, 150, 150, 200, 250];

/// The shoes (record field 38) in the menu's order: the `{u16 price, u16
/// id}` rows at `0x4604c8` (men) and `0x460518` (women, the same rows),
/// entries 12 and 13 of the create tables at `0x4602a0`. The menu walks
/// rows (`FUN_001e4ee8`: row to id; `FUN_001e2020`: id to row) and
/// prices a row (`FUN_001e4e30`), nothing for a free row or one already
/// owned (bit `row` of profile+0x678, `FUN_001e7758`). Shoe 2 is row 10
/// at 75.
pub const SHOES: [(u32, u8); 20] = [
    (0, 0),
    (25, 1),
    (25, 3),
    (25, 4),
    (25, 5),
    (50, 6),
    (50, 7),
    (50, 8),
    (50, 9),
    (50, 10),
    (75, 2),
    (75, 11),
    (75, 12),
    (75, 13),
    (100, 14),
    (100, 15),
    (100, 17),
    (125, 16),
    (125, 18),
    (125, 19),
];

/// The menu row of shoe `id` (`FUN_001e2020`), row 0 for an unknown id.
pub fn shoe_row(id: u8) -> usize {
    SHOES.iter().position(|&(_, i)| i == id).unwrap_or(0)
}

/// What shoe `id` costs when not yet owned: its row's price
/// (`FUN_001e4e30`).
pub fn shoe_price(id: u8) -> u32 {
    SHOES[shoe_row(id)].0
}

/// What a created player has bought to wear, kept with the career in the
/// original: outfit rows (+0x670, bit = the row in the build's
/// [`OUTFITS`]), outfit rows by colour (+0x674, bit = row * 3 + colour) and
/// shoe rows (+0x678, bit = the row in [`SHOES`]). `FUN_001e7680` and
/// `FUN_001e7758` read them, `FUN_001e7820` and `FUN_001e7940` write them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wardrobe {
    pub outfits: u32,
    pub colours: u32,
    pub shoes: u32,
}

impl Wardrobe {
    /// `FUN_001e7680`: outfit `row` is owned in `colour` (rows below 9,
    /// colours below 3).
    pub fn owns_outfit(&self, row: usize, colour: usize) -> bool {
        row < 9
            && colour < 3
            && self.outfits & (1 << row) != 0
            && self.colours & (1 << (row * 3 + colour)) != 0
    }

    /// `FUN_001e7820(career, row, colour, editing)`: bought. Creating adds
    /// the row and its colour to what is owned; editing keeps them alone.
    pub fn buy_outfit(&mut self, row: usize, colour: usize, editing: bool) {
        if row >= 9 || colour >= 3 {
            return;
        }
        let (outfit, coloured) = (1 << row, 1 << (row * 3 + colour));
        if editing {
            self.outfits = outfit;
            self.colours = coloured;
        } else {
            self.outfits |= outfit;
            self.colours |= coloured;
        }
    }

    /// `FUN_001e7758`: shoe `row` is owned (rows below 20).
    pub fn owns_shoes(&self, row: usize) -> bool {
        row < 20 && self.shoes & (1 << row) != 0
    }

    /// `FUN_001e7940`: shoe `row` bought.
    pub fn buy_shoes(&mut self, row: usize) {
        if row < 20 {
            self.shoes |= 1 << row;
        }
    }
}

/// What shoe `id` costs the player now (`FUN_001e4e30`): nothing for a free
/// row (`FUN_001e4d38`) or one owned, else the row's price. Buying
/// (`FUN_001e5058`) takes it and owns the row, so it never costs again.
pub fn shoe_cost(id: u8, wardrobe: &Wardrobe) -> u32 {
    let row = shoe_row(id);
    if wardrobe.owns_shoes(row) {
        0
    } else {
        SHOES[row].0
    }
}

/// Creating a player, the value `FUN_001e4580` asks of an outfit with no
/// price of its own (`FUN_001e4418`: a row priced 0) that is not owned in
/// that colour.
pub const FREE_OUTFIT_COLOUR: u32 = 50;

/// What outfit `row` (in [`OUTFITS`] order) in `colour` costs
/// (`FUN_001e4580`). Creating a player: nothing when owned in that colour
/// (`FUN_001e7680`), [`FREE_OUTFIT_COLOUR`] for a free row, else the row's
/// price. Editing one (the editor's +8, set by the edit screen
/// `FUN_00273dc8`): the row's price less the worn row's, which refunds when
/// it is negative (`FUN_001e70a8(career, 3, -cost)`).
pub fn outfit_cost(row: usize, colour: usize, worn: Option<usize>, wardrobe: &Wardrobe) -> i64 {
    let price = |row: usize| i64::from(OUTFIT_PRICES[row.min(8)]);
    match worn {
        Some(worn) => price(row) - price(worn),
        None if wardrobe.owns_outfit(row, colour) => 0,
        None if price(row) == 0 => i64::from(FREE_OUTFIT_COLOUR),
        None => price(row),
    }
}

/// `FUN_001e4af8`'s ownership after an outfit is bought: editing keeps the
/// new row alone, with the first row (`FUN_001e7820(career, 0, 0, 0)`) when
/// the new row is another.
pub fn buy_outfit(wardrobe: &mut Wardrobe, row: usize, colour: usize, editing: bool) {
    wardrobe.buy_outfit(row, colour, editing);
    if editing && row != 0 {
        wardrobe.buy_outfit(0, 0, false);
    }
}

/// What raising a skill bar one step costs (`FUN_001e5138` reads the next
/// of the entries at `0x460568`).
pub const SKILL_STEP_PRICES: [u32; 6] = [100, 200, 300, 400, 500, 600];

/// The rating at the top of each of the seven bars: shots (row 0), then
/// every other skill (row 1).
pub const SKILL_LEVELS: [[u8; 7]; 2] = [[9, 30, 50, 70, 80, 90, 98], [0, 20, 40, 60, 80, 90, 98]];
