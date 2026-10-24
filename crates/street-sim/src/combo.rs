//! The combo engine, as the game computes it: the combo manager at game
//! +0xd0 (constructor `FUN_001313d0`, vtable `0x43c208`), its ring of the
//! last six events and the pattern matcher `FUN_00131970`. The full
//! evidence lives in `docs/TRACE.md`, "Gameplay values" and "The combo
//! ring, the meter's listeners and the statistics".
//!
//! - The ring of the last six events: data sits at the top (`slot 5` =
//!   the newest, at manager +0x7c); a push (`FUN_00131788`) copies slot
//!   k+1 into slot k when either is filled and writes the new event into
//!   slot 5, so slot 0 becomes the oldest. Each slot holds `{class, extra,
//!   tick, value, span, weight}`: the tick is the game's update count
//!   (game+0x128) at the push, the span the updates the event has lasted
//!   since (the manager's update `FUN_00131560` adds one to the newest's
//!   each update while its play goes on), the weight the matcher's last
//!   read of it.
//! - One ring for the match, not one a side: every push first hands it to
//!   the pushing side (`FUN_00131738`), which clears it, the matched row
//!   and the total when the side changes.
//! - The window: the matcher walks from slot 5 down while the slot is
//!   filled and no more than [`WINDOW`] updates (`DAT_004b71a8`, fps × 2,
//!   written at `0x132b78..0x132b8c`) have passed between the end of its
//!   event (tick + span) and the next newer event's tick, or the game's
//!   count for slot 5 (`0x1319b0..0x131a20`).
//! - The float weight matrix (`0x43be58`), rows of five. A slot's row is
//!   `extra + base` for the six dynamic classes (0 dunks, 1 alley-oops,
//!   7 gamebreakers, 10/11 trick moves and fakeouts, 17 passout specials)
//!   and the class number itself for every other class.
//! - The pattern matcher `FUN_00131970` ([`Chain::evaluate`]): the newest
//!   event's class picks its block of the 55-row table at `0x43c500`
//!   (the constructor's `{first row, count}`), rows asking for more events
//!   than the window holds are skipped, and the first row whose kinds match
//!   decides the payout, which sums only the slots that row walked, at the
//!   column `4 − oldest walked slot`, less one per pair of neighbouring 9/10
//!   events (`DAT_003d0708` is 0 in the shipped data). A lone event matches
//!   no row and pays nothing. A slot pushed with value −1 pays its class's
//!   value ([`CLASS_VALUES`], `0x4b7150`).
//!
//! The HUD multiplier ([`Chain::multiplier`]) is the rewrite's own reading
//! of the depth.

/// `DAT_004b71a8`: the matcher's window, fps × 2 updates (the boot reset
/// `0x132b78..0x132b8c` shifts `FUN_0030fa50`'s frame rate +0x24 left by
/// one; 120 in all three runtime save states).
pub const WINDOW: i32 = 2 * crate::TICK_HZ as i32;

/// `0x4b7150`: the value of each class's event, the fallback for a slot
/// pushed with value −1, filled at boot by the reset at `0x132a40` from the
/// base table `0x43c4b0` (read from the runtime save states' RAM, where all
/// three agree): 0 dunk, 1 alley-oop, 2 lay-up, 3/4 long and short shots,
/// 5 tip slam, 6 game-winning dunk, 7 gamebreaker, 8/9/14 passes, 10/11
/// trick moves and fakeouts, 12 pick, 13 pump fake, 15 dive, 16 rebound,
/// 17 passout special, 18 cancel, 19 block, 20 steal, 21 trick counter.
pub const CLASS_VALUES: [i32; 22] = [
    1_500, 3_000, 1_000, 750, 250, 2_000, 25_000, 25_000, 750, 250, 500, 1_000, 500, 500, 1_000,
    500, 500, 3_000, 5_000, 2_500, 1_000, 3_000,
];

/// One event in the ring (24 bytes in the original). `extra` is the
/// event's own index: the dunk's finish (`actor+0x24c`), the alley-oop's
/// style, the trick move's value class, the gamebreaker's kind, −1 for most
/// others.
///
/// `slot 5` is the newest event and `slot 0` the oldest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub class: i32,
    pub extra: i32,
    /// +0x8: the game's update count (game+0x128) at the push.
    pub tick: i32,
    /// +0xc: the event's value, −1 for its class's ([`CLASS_VALUES`]).
    pub value: i32,
    /// +0x10: the updates the event has lasted since ([`Manager::update`]).
    pub span: i32,
}

impl Slot {
    pub const EMPTY: Slot = Slot {
        class: -1,
        extra: -1,
        tick: 0,
        value: -1,
        span: 0,
    };

    /// An event pushed at `tick` (span 0, as every dispatcher writes it).
    pub fn new(class: i32, extra: i32, tick: i32, value: i32) -> Self {
        Slot {
            class,
            extra,
            tick,
            value,
            span: 0,
        }
    }

    pub fn is_empty(self) -> bool {
        self.class < 0
    }

    /// The value the matcher pays (`0x131c74..0x131c88`): the slot's own,
    /// or its class's for −1.
    pub fn paid_value(self) -> i32 {
        if self.value == -1 {
            CLASS_VALUES.get(self.class as usize).copied().unwrap_or(-1)
        } else {
            self.value
        }
    }
}

/// The ring: slot 5 newest, slot 0 oldest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chain {
    pub slots: [Slot; 6],
}

impl Default for Chain {
    fn default() -> Self {
        Self {
            slots: [Slot::EMPTY; 6],
        }
    }
}

impl Chain {
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.is_empty())
    }

    /// Empty every slot (the original writes −1 into each class word).
    pub fn clear(&mut self) {
        *self = Chain::default();
    }

    /// `FUN_00131788`'s shift and write: each slot takes the next newer
    /// one's event, and `slot` becomes the newest.
    pub fn push(&mut self, slot: Slot) {
        for k in 0..5 {
            if !self.slots[k].is_empty() || !self.slots[k + 1].is_empty() {
                self.slots[k] = self.slots[k + 1];
            }
        }
        self.slots[5] = slot;
    }

    /// How many slots hold events.
    pub fn depth(&self) -> usize {
        self.slots.iter().filter(|s| !s.is_empty()).count()
    }

    /// The depth ramp's index, 0 (two events) through 4 (six), for the HUD's
    /// multiplier.
    pub fn ramp_index(&self) -> usize {
        self.depth().saturating_sub(2).min(4)
    }

    /// The events the matcher may walk at the game's count `now`, newest
    /// first (`FUN_001318d8`, the matcher's own pre-scan): the filled slots
    /// from slot 5 down whose event ended (tick + span) no more than
    /// [`WINDOW`] updates before the next newer event's tick, or before
    /// `now` for slot 5.
    pub fn window(&self, now: i32) -> usize {
        let mut fresh = 0;
        for k in (0..6).rev() {
            let slot = self.slots[k];
            if slot.is_empty() {
                break;
            }
            let after = if k == 5 { now } else { self.slots[k + 1].tick };
            if WINDOW < after - (slot.tick + slot.span) {
                break;
            }
            fresh += 1;
        }
        fresh
    }

    /// `FUN_00131970` over the ring at the game's count `now`, with the
    /// row block of the newest event's class: the matched row (`+0x148`),
    /// the weight column, the oldest walked event's tick (`+0x14c`) and the
    /// payout (`+0x150`), or `None` where the matcher returns without a
    /// match (it then writes none of them). Raw `0x131970..0x131cf4`:
    ///
    /// - `t1` = `5 − window` (−1 for a full window); the block's leading
    ///   rows whose kind at `t1` is not −1 (they ask for an event outside
    ///   the window) are skipped (`0x131a24..0x131a88`);
    /// - each row is walked from slot 5 down while its kind is not −1 and
    ///   the slot is inside the window: a real kind must equal the slot's
    ///   class (the table's six extras are all −1, "don't check"), and a
    ///   9/10 class right after a 9/10 class counts a pair
    ///   (`0x131b04..0x131b88`);
    /// - the first row that walks clean pays: the column is `4 − t4` (`t4`
    ///   the oldest slot walked) less the pairs when `DAT_003d0708` is 0, and
    ///   the sum runs from slot 5 down to `t4`, newest first, `value ×
    ///   0x43be58[row × 5 + column]` in f32, truncated (`0x131b94..0x131cdc`).
    pub fn evaluate(&self, now: i32) -> Option<Payout> {
        self.evaluate_block(now, pattern_block(self.slots[5].class))
    }

    /// [`Chain::evaluate`] with the dispatcher's own `{first row, count}`:
    /// the basket dispatcher hands the block of the class it pushed, the
    /// block's and the steal's theirs.
    pub fn evaluate_block(&self, now: i32, (start, count): (usize, usize)) -> Option<Payout> {
        let limit = 5 - self.window(now) as i32;
        let end = start + count;
        let mut row = start;
        if limit != -1 && row < end && PATTERNS[row][limit as usize] != -1 {
            loop {
                row += 1;
                if row >= end || PATTERNS[row][limit as usize] == -1 {
                    break;
                }
            }
        }
        let swipe = |class: i32| (class - 9) as u32 <= 1;
        while row < end {
            let kinds = &PATTERNS[row];
            let mut t = 5i32;
            let mut clean = true;
            let mut pairs = 0;
            if limit + 1 < 6 && kinds[5] != -1 {
                let mut previous = -1;
                loop {
                    let class = self.slots[t as usize].class;
                    let kind = i32::from(kinds[t as usize]);
                    if kind != -2 && class != kind {
                        clean = false;
                        break;
                    }
                    if swipe(class) && swipe(previous) {
                        pairs += 1;
                    }
                    previous = class;
                    t -= 1;
                    if t < limit + 1 || kinds[t as usize] == -1 {
                        break;
                    }
                }
            }
            if clean {
                let oldest = t + 1;
                let column = 4 - oldest - if DOUBLE_SWIPE_FLAG == 0 { pairs } else { 0 };
                let mut sum = 0.0f32;
                for slot in self.slots.iter().skip(oldest as usize).rev() {
                    sum += slot.paid_value() as f32 * weight(slot_row(*slot), column);
                }
                return Some(Payout {
                    row,
                    column,
                    since: self.slots[oldest.clamp(0, 5) as usize].tick,
                    total: sum as i32,
                });
            }
            row += 1;
        }
        None
    }

    /// The classes of the slots a payout's row walked, oldest first, −1 for
    /// the slots it leaves out: what the HUD's combo line reads
    /// (`FUN_002b87c0` starts at the row's first kind other than −1 in the
    /// same table, `0x48d400` in `.data`, and reads each slot's class).
    pub fn walked(&self, payout: &Payout) -> [i8; 6] {
        let kinds = PATTERNS.get(payout.row);
        std::array::from_fn(|k| match kinds {
            Some(kinds) if kinds[k] != -1 => self.slots[k].class as i8,
            _ => -1,
        })
    }

    /// The payout ([`Chain::evaluate`]) at `now`, 0 without a match.
    pub fn total(&self, now: i32) -> i32 {
        self.evaluate(now).map_or(0, |p| p.total)
    }

    /// The multiplier the HUD shows, the game's ramp as a whole number
    /// (the tips' "the multiplier goes up as the combo is kept").
    pub fn multiplier(&self) -> u32 {
        (self.ramp_index() as u32 + 1).min(5)
    }
}

/// What [`Chain::evaluate`] answers for a matched ring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Payout {
    /// The pattern row (`game+0x148`).
    pub row: usize,
    /// The weight column. It never goes below 0 for the shipped rows: the
    /// pairs are always fewer than the walked events less one.
    pub column: i32,
    /// The oldest walked event's tick (`+0x14c`, `FUN_00131d18`: where the
    /// combo's replay starts).
    pub since: i32,
    /// The payout (`game+0x150`).
    pub total: i32,
}

/// `DAT_003d0708`, 0 in the shipped `.data`: the matcher then drops a
/// column per pair of neighbouring 9/10 events.
pub const DOUBLE_SWIPE_FLAG: u32 = 0;

/// The weight row of a ring slot (`0x131bf8..0x131c70`): the six dynamic
/// classes add their base to the extra (0 → 22, 10 → 27, 11 → 32, 17 → 37,
/// 7 → 39, 1 → 45), every other class is its own row.
fn slot_row(slot: Slot) -> i32 {
    match slot.class {
        0 => 22 + slot.extra,
        10 => 27 + slot.extra,
        11 => 32 + slot.extra,
        17 => 37 + slot.extra,
        7 => 39 + slot.extra,
        1 => 45 + slot.extra,
        c => c,
    }
}

/// `0x43be58[row × 5 + column]`, the flat index the original reads, −1.0
/// weights included. An index past the 47 weight rows reads unrelated words
/// in the original; the rewrite answers 0.0 there.
fn weight(row: i32, column: i32) -> f32 {
    let flat = row * 5 + column;
    if (0..(WEIGHT_ROWS * 5) as i32).contains(&flat) {
        WEIGHTS[flat as usize / 5][flat as usize % 5]
    } else {
        0.0
    }
}

/// The constructor `FUN_001313d0`'s `{first row, row count}` for a class
/// (game `+0x94 + class × 8`): the first row whose newest kind is the class
/// (class 0 tests row 0 only, the others rows 1..54) and the run of rows
/// from it with the same newest kind; `(0, 0)` for a class with no row.
pub fn pattern_block(class: i32) -> (usize, usize) {
    if !(0..=21).contains(&class) {
        return (0, 0);
    }
    let newest = |row: usize| i32::from(PATTERNS[row][5]);
    let first = if class == 0 {
        0
    } else {
        match (1..PATTERNS.len()).find(|&row| newest(row) == class) {
            Some(row) => row,
            None => return (0, 0),
        }
    };
    let count = (first..PATTERNS.len())
        .take_while(|&row| newest(row) == class)
        .count();
    (first, count)
}

/// The combo manager's state (game+0xd0, `FUN_001313d0`'s 0x170 bytes):
/// the ring and the words its dispatchers and its update keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Manager {
    /// +0x4..+0x9b.
    pub ring: Chain,
    /// +0x144: the side the ring belongs to (0 at construction).
    pub side: usize,
    /// +0x148: the last matched row, −1 for none.
    pub row: i32,
    /// +0x14c: the matched row's oldest walked event's tick.
    pub since: i32,
    /// +0x150: the last match's payout, which every meter and statistics
    /// listener adds to its own value in the update it is made (−1 at
    /// construction, 0 from the first update).
    pub total: i32,
    /// +0x154: the pass's thrower in the air (slot 9, `FUN_00131d88`): 0 on
    /// the floor, 1 in the air, 2 in the air with +0x164 set.
    pub air: i32,
    /// +0x158: a basket was dispatched; the next update clears the ring.
    pub basket: bool,
    /// +0x160: a pump fake (slot 5, `FUN_00131df0`) arms the next pump
    /// fake's points (slot 73, `FUN_00131f78`).
    pub pump_fake: bool,
    /// +0x164: the holder is in a dunk or an alley-oop's leap (slot 24's
    /// `FUN_00131e78`; slot 4's `FUN_00131e00`).
    pub selected: bool,
    /// +0x168: a pass, steal, block or rebound was pushed since the last
    /// catch (slot 24 pushes a dive only without one).
    pub pushed: bool,
    /// +0x16c: the pass dispatcher has seen a pass (it skips the first).
    pub ready: bool,
}

impl Default for Manager {
    fn default() -> Self {
        Self {
            ring: Chain::default(),
            side: 0,
            row: -1,
            since: 0,
            total: -1,
            air: 0,
            basket: false,
            pump_fake: false,
            selected: false,
            pushed: false,
            ready: false,
        }
    }
}

/// What the manager's update reads (`FUN_00131560`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scene {
    /// The game's count before this update's (game+0x128).
    pub tick: i32,
    /// The ball's state (ball+0x180).
    pub ball: i32,
    /// The holder's state bits (+0xa4), `None` without a holder.
    pub holder: Option<u32>,
    /// The script manager in states 3..7 (a staged sequence).
    pub staged: bool,
}

impl Manager {
    /// `FUN_00131738`: the ring goes to `side`; a change of side clears it,
    /// the matched row and the total.
    pub fn own(&mut self, side: usize) {
        if self.side != side {
            self.side = side;
            self.row = -1;
            self.total = 0;
            self.ring.clear();
        }
    }

    /// `FUN_00131788`: while the script manager is in states 3..7 only the
    /// gamebreaker classes 6 and 7 enter the ring (and only when the
    /// staged script answers 1, `FUN_00337af0`, taken as so).
    pub fn push(&mut self, slot: Slot, staged: bool) {
        if staged && !(6..=7).contains(&slot.class) {
            return;
        }
        self.ring.push(slot);
    }

    /// The matcher run by the basket, block and steal dispatchers: on a
    /// match the row, its oldest tick and the payout are kept (`+0x148`,
    /// `+0x14c`, `+0x150`); without one they stand.
    pub fn evaluate(&mut self, now: i32, class: i32) -> Option<Payout> {
        let payout = self.ring.evaluate_block(now, pattern_block(class))?;
        self.row = payout.row as i32;
        self.since = payout.since;
        self.total = payout.total;
        Some(payout)
    }

    /// `FUN_00131560`, at the top of each game update before its count
    /// moves on (`FUN_0013d868`, `0x13db20`): the newest event's span grows
    /// while its play lasts — the ball in a shot (states 5, 6), a staged
    /// sequence, the holder in a jump shot, alley-oop leap or dunk (bits 3,
    /// 15, 7), or, on updates whose count has bits 0 and 4 not both set,
    /// the ball in a pass (state 4) or the holder in a trick move (bit
    /// 10). Outside a staged sequence the row and the total are then
    /// dropped, and after a basket the ring is cleared.
    pub fn update(&mut self, scene: Scene) {
        let bits = scene.holder.unwrap_or(0);
        let extends = (5..=6).contains(&scene.ball)
            || scene.staged
            || bits & 0x8088 != 0
            || (scene.tick & 0x11 != 0x11 && (scene.ball == 4 || bits & 0x400 != 0));
        if extends && !self.ring.slots[5].is_empty() {
            self.ring.slots[5].span += 1;
        }
        if !scene.staged {
            self.row = -1;
            self.total = 0;
            if self.basket {
                self.basket = false;
                self.ring.clear();
            }
        }
    }
}

/// The weight matrix, from the executable's `.rodata` at `0x43be58`, five
/// floats per row: the 47 rows the matcher's row indices reach (row 46 is
/// an alley-oop with extra 1, `FUN_001322d0`'s `(FUN_00171e28 >> 1) & 1`).
/// The words after them are not weights.
pub const WEIGHT_ROWS: usize = 47;
pub static WEIGHTS: [[f32; 5]; WEIGHT_ROWS] = [
    [-1.0; 5],                     // row 0 (classes that never use it)
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 1
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 2
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 3 (long shots)
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 4 (short shots)
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 5
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 6 (level 1 gamebreakers)
    [-1.0; 5],                     // 7
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 8
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 9
    [-1.0; 5],                     // 10
    [-1.0; 5],                     // 11
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 12
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 13
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 14
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 15
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 16
    [-1.0; 5],                     // 17
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 18
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 19
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 20
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 21
    [0.75, 1.0, 1.5, 2.0, 2.5],    // 22 lay-up finish 0 (1,000)
    [1.0, 1.25, 1.75, 2.25, 2.75], // 23 finish 1 (2,000)
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 24 finish 2 (3,000)
    [1.5, 1.75, 2.25, 2.75, 3.25], // 25 finish 3 (5,000)
    [1.75, 2.0, 2.5, 3.0, 3.5],    // 26 finish 4 (7,500)
    [0.1, 0.2, 0.3, 0.4, 0.5],     // 27 trick value class 0 (500)
    [0.2, 0.3, 0.4, 0.5, 0.6],     // 28 class 1 (1,000)
    [0.3, 0.4, 0.5, 0.6, 0.7],     // 29 class 2 (2,000)
    [0.4, 0.5, 0.6, 0.7, 0.8],     // 30 class 3 (3,000)
    [0.5, 0.6, 0.7, 0.8, 0.9],     // 31 class 4 (5,000)
    [0.75, 1.0, 1.5, 2.0, 2.5],    // 32 fakeout class 0
    [1.0, 1.25, 1.75, 2.25, 2.75], // 33 class 1
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 34 class 2
    [1.5, 1.75, 2.25, 2.75, 3.25], // 35 class 3
    [1.75, 2.0, 2.5, 3.0, 3.5],    // 36 class 4
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 37 drains
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 38
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 39 gamebreaker bases
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 40
    [1.0, 1.1, 1.2, 1.3, 1.5],     // 41
    [1.0, 1.1, 1.2, 1.3, 1.5],     // 42
    [1.0, 1.05, 1.1, 1.15, 1.25],  // 43
    [1.0, 1.05, 1.1, 1.15, 1.25],  // 44
    [1.25, 1.5, 2.0, 2.5, 3.0],    // 45 alley-oops (style 0)
    [1.75, 2.0, 2.5, 3.0, 3.5],    // 46 alley-oops (style 1)
];

/// The pattern table, verbatim from the executable's `.rodata` at
/// `0x43c500`: 55 rows of six kinds. Each kind −1 ends the requirement
/// (the slots deeper than it are not checked), −2 is a wildcard, and any
/// real kind must sit in the matching ring slot. Rows 0..39 are the eight
/// basket classes' five recency rows each (row `k*5 + j` asks for the class
/// as the newest event with the `j` older ones behind it); rows 40..44 a
/// full passout ring of class 19, 45..49 and 50..54 four 19s and a 20 or 21.
pub static PATTERNS: [[i8; 6]; 55] = [
    [-2, -2, -2, -2, -2, 0],
    [-1, -2, -2, -2, -2, 0],
    [-1, -1, -2, -2, -2, 0],
    [-1, -1, -1, -2, -2, 0],
    [-1, -1, -1, -1, -2, 0],
    [-2, -2, -2, -2, -2, 1],
    [-1, -2, -2, -2, -2, 1],
    [-1, -1, -2, -2, -2, 1],
    [-1, -1, -1, -2, -2, 1],
    [-1, -1, -1, -1, -2, 1],
    [-2, -2, -2, -2, -2, 2],
    [-1, -2, -2, -2, -2, 2],
    [-1, -1, -2, -2, -2, 2],
    [-1, -1, -1, -2, -2, 2],
    [-1, -1, -1, -1, -2, 2],
    [-2, -2, -2, -2, -2, 3],
    [-1, -2, -2, -2, -2, 3],
    [-1, -1, -2, -2, -2, 3],
    [-1, -1, -1, -2, -2, 3],
    [-1, -1, -1, -1, -2, 3],
    [-2, -2, -2, -2, -2, 4],
    [-1, -2, -2, -2, -2, 4],
    [-1, -1, -2, -2, -2, 4],
    [-1, -1, -1, -2, -2, 4],
    [-1, -1, -1, -1, -2, 4],
    [-2, -2, -2, -2, -2, 5],
    [-1, -2, -2, -2, -2, 5],
    [-1, -1, -2, -2, -2, 5],
    [-1, -1, -1, -2, -2, 5],
    [-1, -1, -1, -1, -2, 5],
    [-2, -2, -2, -2, -2, 6],
    [-1, -2, -2, -2, -2, 6],
    [-1, -1, -2, -2, -2, 6],
    [-1, -1, -1, -2, -2, 6],
    [-1, -1, -1, -1, -2, 6],
    [-2, -2, -2, -2, -2, 7],
    [-1, -2, -2, -2, -2, 7],
    [-1, -1, -2, -2, -2, 7],
    [-1, -1, -1, -2, -2, 7],
    [-1, -1, -1, -1, -2, 7],
    [19, 19, 19, 19, 19, 19],
    [-1, 19, 19, 19, 19, 19],
    [-1, -1, 19, 19, 19, 19],
    [-1, -1, -1, 19, 19, 19],
    [-1, -1, -1, -1, 19, 19],
    [19, 19, 19, 19, 19, 20],
    [-1, 19, 19, 19, 19, 20],
    [-1, -1, 19, 19, 19, 20],
    [-1, -1, -1, 19, 19, 20],
    [-1, -1, -1, -1, 19, 20],
    [19, 19, 19, 19, 19, 21],
    [-1, 19, 19, 19, 19, 21],
    [-1, -1, 19, 19, 19, 21],
    [-1, -1, -1, 19, 19, 21],
    [-1, -1, -1, -1, 19, 21],
];

#[cfg(test)]
mod tests {
    use super::*;

    /// A ring of events pushed in the same update (all inside the window).
    fn chain(events: &[(i32, i32, i32)]) -> Chain {
        let mut c = Chain::default();
        for &(class, extra, value) in events {
            c.push(Slot::new(class, extra, 0, value));
        }
        c
    }

    #[test]
    fn push_keeps_slot_five_as_the_newest() {
        let mut c = Chain::default();
        c.push(Slot::new(3, -1, 10, 750));
        assert_eq!(c.slots[5], Slot::new(3, -1, 10, 750));
        assert!(c.slots[0].is_empty());
        assert_eq!(c.depth(), 1);
        c.push(Slot::new(4, -1, 11, 250));
        // The first event has moved one slot toward the older end.
        assert_eq!(c.slots[4], Slot::new(3, -1, 10, 750));
        assert_eq!(c.slots[5], Slot::new(4, -1, 11, 250));
        assert_eq!(c.depth(), 2);
    }

    #[test]
    fn a_full_ring_rolls_off_the_oldest() {
        let c = chain(&[(2, -1, 1_000); 7]);
        // Six slots hold the last six events; the oldest has rolled off.
        assert_eq!(c.depth(), 6);
        assert_eq!(c.slots[0], Slot::new(2, -1, 0, 1_000));
        assert_eq!(
            c.total(0),
            (6.0f32 * 1_000.0 * 3.0) as i32,
            "a full chain reads the ramp's 3.0 column"
        );
    }

    #[test]
    fn ramp_scales_with_the_chain() {
        let mut c = Chain::default();
        c.push(Slot::new(4, -1, 0, 250));
        // One event: every row of the class-4 block asks for at least two,
        // so the skip runs past the block and the matcher pays nothing.
        assert_eq!(c.evaluate(0), None, "a lone event matches no row");
        for (n, ramp) in [
            (2.0f32, 1.25f32),
            (3.0, 1.5),
            (4.0, 2.0),
            (5.0, 2.5),
            (6.0, 3.0),
        ] {
            c.push(Slot::new(4, -1, 0, 250));
            assert_eq!(c.total(0), (n * 250.0 * ramp) as i32);
        }
    }

    #[test]
    fn trick_moves_count_a_fraction() {
        // A 500-point trick (class 10, extra 0) under a short shot: the trick
        // sits at row 27 = {0.1, 0.2, ...}, the shot at row 4 = {1.25..}.
        let c = chain(&[(10, 0, 500), (4, -1, 250)]);
        assert_eq!(c.total(0), (250.0f32 * 1.25 + 500.0 * 0.1) as i32);
        // The same trick at the fifth level (extra 4) counts half of itself.
        let c = chain(&[(10, 4, 5_000), (4, -1, 250)]);
        assert_eq!(c.total(0), (250.0f32 * 1.25 + 5_000.0 * 0.5) as i32);
        // A trick on top has no row block: only scorers match.
        assert_eq!(chain(&[(4, -1, 250), (10, 0, 500)]).evaluate(0), None);
    }

    #[test]
    fn fakeouts_scale_up_by_level() {
        // A level-2 fakeout (class 11, extra 2) lands in row 34
        // = {1.25, 1.5, 2.0, 2.5, 3.0}, so the chain's first column is 1.25.
        let c = chain(&[(11, 2, 1_000), (4, -1, 250)]);
        assert_eq!(c.total(0), (250.0f32 * 1.25 + 1_000.0 * 1.25) as i32);
        // Level 4 fakeouts scale further (row 36 = {0.75..3.5}: 1.75 first).
        let c = chain(&[(11, 4, 1_000), (4, -1, 250)]);
        assert_eq!(c.total(0), (250.0f32 * 1.25 + 1_000.0 * 1.75) as i32);
    }

    #[test]
    fn out_of_table_rows_contribute_nothing() {
        let c = chain(&[(1, 82, 3_000), (4, -1, 250)]); // 45 + 82, past the table
        assert_eq!(c.total(0), (250.0f32 * 1.25) as i32);
    }

    #[test]
    fn empty_chain_totals_zero() {
        assert_eq!(Chain::default().total(0), 0);
        assert_eq!(Chain::default().multiplier(), 1);
        assert!(Chain::default().is_empty());
    }

    #[test]
    fn a_value_of_minus_one_pays_the_class_value() {
        // A short shot pushed with −1 (the basket dispatcher's own value
        // for jump shots) over a pass of class 8 (−1, 750).
        let c = chain(&[(8, -1, -1), (4, -1, -1)]);
        assert_eq!(c.total(0), (250.0f32 * 1.25 + 750.0 * 1.25) as i32);
        assert_eq!(Slot::new(20, -1, 0, -1).paid_value(), 1_000);
        assert_eq!(Slot::new(21, -1, 0, -1).paid_value(), 3_000);
        assert_eq!(Slot::new(19, -1, 0, -1).paid_value(), 2_500);
    }

    #[test]
    fn matched_row_gates_and_pays_the_chain() {
        // Rows 20..24 are the short-shot family (class 4). With two events
        // in the window the skip passes rows 20..23 (they ask for a third)
        // and stops at row 24, the newest event and one wildcard.
        let c = chain(&[(10, 0, 500), (4, -1, 250)]);
        assert_eq!(
            c.evaluate(0),
            Some(Payout {
                row: 24,
                column: 0,
                since: 0,
                total: (250.0f32 * 1.25 + 500.0 * 0.1) as i32
            }),
            "a short-shot ending matches the row of its own depth"
        );
        let c = chain(&[(10, 0, 500), (2, -1, 1_000), (4, -1, 250)]);
        assert_eq!(c.evaluate(0).map(|p| p.row), Some(23));
        let c = chain(&[(10, 0, 500), (10, 0, 500)]);
        assert_eq!(
            c.evaluate(0),
            None,
            "a chain of only trick moves has no row"
        );
        // Six blocks match the all-19 row.
        let c = chain(&[(19, -1, -1); 6]);
        assert_eq!(c.evaluate(0).map(|p| p.row), Some(40));
        // Two blocks under a trick counter match the first 21-row whose
        // 19s are deep enough: kinds −1×3, 19, 19, 21 (row 53).
        let c = chain(&[(19, -1, -1), (19, -1, -1), (21, -1, -1)]);
        assert_eq!(c.evaluate(0).map(|p| p.row), Some(53));
    }

    #[test]
    fn the_double_swipe_penalty_lowers_the_column() {
        // A basket over two 9s and a 10: the walk meets the pairs (9, 9)
        // and (9, 10), so four walked events read column 2 − 2 = 0.
        let c = chain(&[(10, 0, 500), (9, -1, 250), (9, -1, 250), (4, -1, 250)]);
        let payout = c.evaluate(0).unwrap();
        assert_eq!((payout.row, payout.column), (22, 0));
        assert_eq!(
            payout.total,
            (250.0f32 * 1.25 + 250.0 * 1.25 + 250.0 * 1.25 + 500.0 * 0.1) as i32
        );
        // Without the swipes the same depth reads column 2.
        let c = chain(&[(10, 0, 500), (2, -1, 250), (2, -1, 250), (4, -1, 250)]);
        assert_eq!(c.evaluate(0).unwrap().column, 2);
    }

    #[test]
    fn block_rows_pay_only_the_slots_they_walk() {
        // Two old baskets under four blocks: rows 40 and 41 fail on the
        // baskets, row 42 walks slots 2..5 and pays them at column 2.
        let c = chain(&[
            (3, -1, 750),
            (3, -1, 750),
            (19, -1, -1),
            (19, -1, -1),
            (19, -1, -1),
            (19, -1, -1),
        ]);
        assert_eq!(
            c.evaluate(0),
            Some(Payout {
                row: 42,
                column: 2,
                since: 0,
                total: (4.0f32 * 2_500.0 * 2.0) as i32
            })
        );
    }

    #[test]
    fn pattern_blocks_follow_the_constructor() {
        for class in 0..8 {
            assert_eq!(pattern_block(class), (class as usize * 5, 5));
        }
        assert_eq!(pattern_block(19), (40, 5));
        assert_eq!(pattern_block(20), (45, 5));
        assert_eq!(pattern_block(21), (50, 5));
        for class in [8, 9, 10, 11, 12, 16, 17, 18, -1, 22] {
            assert_eq!(pattern_block(class), (0, 0));
        }
    }

    #[test]
    fn the_window_ends_two_seconds_after_an_event() {
        // A trick at 100 lasting 30 updates, then a short shot: within 120
        // updates of the trick's end it pays, a update later it is alone.
        let mut fresh = Chain::default();
        fresh.push(Slot {
            span: 30,
            ..Slot::new(10, 0, 100, 500)
        });
        let mut stale = fresh;
        fresh.push(Slot::new(4, -1, 250, -1));
        stale.push(Slot::new(4, -1, 251, -1));
        assert_eq!(fresh.window(250), 2);
        assert_eq!(fresh.total(250), (250.0f32 * 1.25 + 500.0 * 0.1) as i32);
        assert_eq!(stale.window(251), 1);
        assert_eq!(stale.evaluate(251), None, "the trick is past the window");
        // The newest event against the game's count.
        assert_eq!(fresh.window(250 + WINDOW), 2);
        assert_eq!(fresh.window(251 + WINDOW), 0);
    }

    #[test]
    fn a_steal_after_its_sides_block_pays_both() {
        // The played capture `fades-1` (update 3,937): the person's side
        // blocked a shot (class 19) and picked the ball up 72 updates later
        // (a steal, class 20). Row 49 walks both at column 0: 2,500 × 1.25
        // + 1,000 × 1.25 = 4,375; the steal's meter fill was 1,000 + 4,375.
        let mut c = Chain::default();
        c.push(Slot::new(19, -1, 3_865, -1));
        c.push(Slot::new(20, -1, 3_937, -1));
        let payout = c.evaluate(3_937).unwrap();
        assert_eq!((payout.row, payout.column, payout.total), (49, 0, 4_375));
        assert_eq!(payout.since, 3_865);
    }

    /// Two rings the original matched, from the runtime save states' RAM
    /// (`pcsx2-runtime/sstates`, slots 2 and 9: the combo manager at
    /// game+0xd0, read offline from `eeMemory.bin`). The next update's
    /// clear after the basket wrote −1 over each class; the classes here
    /// are the ones the stored values, extras and the weights the matcher
    /// wrote back (slot +0x14) fit. Each `(class, extra, tick, value, span,
    /// weight)`; the value −1 where the dispatcher pushes −1 (the matcher
    /// wrote the class's value back).
    fn saved(slots: &[(i32, i32, i32, i32, i32, f32)]) -> (Chain, Vec<f32>) {
        let mut c = Chain::default();
        for &(class, extra, tick, value, span, _) in slots {
            c.push(Slot {
                class,
                extra,
                tick,
                value,
                span,
            });
        }
        (c, slots.iter().map(|s| s.5).collect())
    }

    fn weights_read(c: &Chain, payout: &Payout, n: usize) -> Vec<f32> {
        c.slots[6 - n..]
            .iter()
            .map(|&s| weight(slot_row(s), payout.column))
            .collect()
    }

    #[test]
    fn the_save_states_matched_rings_pay_as_the_original_weighed_them() {
        // Slot 2 (count 470): a plain trick of value class 2 (2,000 / 2) and
        // a dunk of finish 3 (5,000, `0x43df38[3]`), 48 updates apart after
        // the trick's 89: row 4, column 0, since 332 (+0x14c).
        let (c, stored) = saved(&[(10, 2, 332, 1_000, 89, 0.3), (0, 3, 469, 5_000, 0, 1.5)]);
        let p = c.evaluate(469).unwrap();
        assert_eq!((p.row, p.column, p.since), (4, 0, 332));
        assert_eq!(weights_read(&c, &p, 2), stored);
        assert_eq!(p.total, (1_000.0f32 * 0.3 + 5_000.0 * 1.5) as i32);
        // Slot 9 (count 15,005, the attract match): a trick counter (class
        // 21, −1 → 3,000), two value-class-2 tricks (the second halved by
        // the repeat ring), a value-class-3 fakeout (3,000) and the lay-up
        // (class 2, −1 → 1,000), each within 120 updates of the next after
        // its span: row 11, column 4 − 1 − one (10, 10) pair = 2.
        let (c, stored) = saved(&[
            (21, -1, 14_542, -1, 47, 2.0),
            (10, 2, 14_649, 1_000, 46, 0.5),
            (10, 2, 14_738, 500, 49, 0.5),
            (11, 3, 14_809, 3_000, 106, 2.25),
            (2, -1, 14_953, -1, 0, 2.0),
        ]);
        assert_eq!(c.window(14_953), 5);
        let p = c.evaluate(14_953).unwrap();
        assert_eq!((p.row, p.column, p.since), (11, 2, 14_542));
        assert_eq!(weights_read(&c, &p, 5), stored);
        assert_eq!(
            c.slots[1..]
                .iter()
                .map(|s| s.paid_value())
                .collect::<Vec<_>>(),
            [3_000, 1_000, 500, 3_000, 1_000]
        );
        assert_eq!(p.total, 6_000 + 500 + 250 + 6_750 + 2_000);
    }

    #[test]
    fn the_manager_hands_the_ring_to_the_pushing_side() {
        let mut m = Manager::default();
        m.own(0);
        m.push(Slot::new(10, 0, 1, 500), false);
        m.own(0);
        assert_eq!(m.ring.depth(), 1, "the same side keeps it");
        m.total = 99;
        m.own(1);
        assert_eq!((m.ring.depth(), m.side, m.row, m.total), (0, 1, -1, 0));
        // A staged sequence lets only the gamebreaker classes in.
        m.push(Slot::new(4, -1, 2, -1), true);
        assert!(m.ring.is_empty());
        m.push(Slot::new(7, 4, 2, 25_000), true);
        assert_eq!(m.ring.slots[5].class, 7);
    }

    #[test]
    fn the_update_grows_the_newest_span_and_drops_the_total() {
        let mut m = Manager::default();
        m.push(Slot::new(10, 0, 5, 500), false);
        let scene = |tick, ball, holder| Scene {
            tick,
            ball,
            holder,
            staged: false,
        };
        // A jump shot's holder (bit 3) grows it every update.
        m.update(scene(0x11, 3, Some(0x8)));
        assert_eq!(m.ring.slots[5].span, 1);
        // A trick (bit 10) only on counts without bits 0 and 4 both set.
        m.update(scene(0x11, 3, Some(0x400)));
        assert_eq!(m.ring.slots[5].span, 1);
        m.update(scene(0x12, 3, Some(0x400)));
        assert_eq!(m.ring.slots[5].span, 2);
        // A pass in flight (state 4) likewise; a plain holder never.
        m.update(scene(0x14, 4, None));
        assert_eq!(m.ring.slots[5].span, 3);
        m.update(scene(0x16, 3, Some(0)));
        assert_eq!(m.ring.slots[5].span, 3);
        // The total lives one update; a basket's ring goes the next.
        m.total = 1_234;
        m.row = 24;
        m.basket = true;
        m.update(scene(0x17, 1, None));
        assert_eq!((m.total, m.row, m.basket), (0, -1, false));
        assert!(m.ring.is_empty());
        // In a staged sequence both stand.
        m.total = 7;
        m.update(Scene {
            staged: true,
            ..scene(0x18, 3, None)
        });
        assert_eq!(m.total, 7);
    }
}
