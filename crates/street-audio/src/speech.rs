//! Speech: the commentator (Bobbito Garcia, `/SPEECH/SPCHDATS.BIG`) and the
//! players' own lines (`/SPEECH/SPCH_PLY.AST`), chosen the way the game's
//! speech middleware (`0x362000..0x367200`) chooses them.
//!
//! `SPCHDATS.BIG` is a BIG archive of 156 `.dat` files named by situation
//! (`Dunk\DunkA.dat`, `Steal\StealB.dat`, `NicknameAction\05.dat`, ...),
//! each a run of mono XA `SCHl` streams, one per spoken line (36 kHz).
//! Files ending `_Clone`, `_Clone_1`, ... repeat their original for the
//! disc's seek layout and are skipped.
//!
//! The choice of a line is data: an event file (`COMNTARY.EVT`,
//! `PLAYERS.EVT`) roots each event in weighted branches, each naming a
//! header (`SPCHHDRS.BIG`, `PLYRHDRS.BIG`, `c0 fb` archives) whose lines
//! carry metadata bytes the event's request words filter. The speech
//! manager (`0x1ac6f0`) registers the players' headers and then the
//! commentary's into one table (`FUN_00363060`), so a header's slot in that
//! table keys the shared random generator's result ring.

use std::{
    collections::BTreeMap,
    io::{Read, Seek},
};

use street_assets::{
    Result,
    big::{self, BigEntry},
    iso::Iso,
};

use crate::schl::{self, Stream};

pub const ARCHIVE: &str = "/SPEECH/SPCHDATS.BIG";
pub const COMMENTARY_EVENTS: &str = "/SPEECH/COMNTARY.EVT";
pub const COMMENTARY_HEADERS: &str = "/SPEECH/SPCHHDRS.BIG";
pub const PLAYER_EVENTS: &str = "/SPEECH/PLAYERS.EVT";

/// The players' manager and the commentary's, as the queue and the service
/// number them: the order `0x1ac6f0` registers their event tables
/// (`FUN_001adac8(.., 0, "speech/players.evt", ..)`, then 1 for
/// `comntary.evt`), which a queued request keeps at slot +0x6b and
/// `FUN_00364620` serves by. A live capture of the original read players'
/// requests at 0 and the commentary's at 1, and the per-manager runs
/// (`DAT_00422b28`) with event 0x2000 at entry 0. The post's key byte is
/// the other way round (`FUN_00364600`: `0x1ad068` posts 1, `0x1ad178` 0):
/// it names the event table's own id, not the manager.
pub const PLAYERS: usize = 0;
pub const COMMENTARY: usize = 1;

/// `DAT_00422ba8`: a branch weight is its byte's low five bits times this
/// scale, picked by the top three (`FUN_00365020`).
pub const WEIGHT_SCALE: [u32; 8] = [1, 4, 0x10, 0x40, 0x100, 0x400, 0x1000, 0x4000];

/// The word at gp-0x7dd0 (`0x4b3220`; gp is `0x4baff0` from the entry
/// point's `lui/addiu a0` at `0x100034/0x100048`). Its initial data is 1 and
/// no instruction stores it, so `FUN_00365ba0` always switches off a line
/// of a partitioned header once it is used.
pub const CONSUME_PARTITIONED_LINES: bool = true;

/// A packet's 20 request words (`FUN_00363a10` copies `0x14` words).
pub type Packet = [u32; 20];

fn bytes(data: &[u8], at: usize, count: usize) -> Result<&[u8]> {
    data.get(at..at.saturating_add(count))
        .ok_or_else(|| street_assets::Error::Format("speech data outside its file".into()))
}

fn u16_at(data: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(bytes(data, at, 2)?.try_into().unwrap()))
}

fn u32_at(data: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(bytes(data, at, 4)?.try_into().unwrap()))
}

fn unsupported(what: String) -> street_assets::Error {
    street_assets::Error::Format(format!("unsupported speech {what}"))
}

/// `FUN_001a82d8`'s sex predicate: 3 (neutral or female lines) for a woman,
/// 5 (neutral or male) for a man, all bits (-1) for nobody.
pub fn sex_mask(female: Option<bool>) -> u32 {
    female.map_or(u32::MAX, |female| if female { 3 } else { 5 })
}

// ---------------------------------------------------------------------------
// Headers

/// One header as the middleware holds it in memory. Its recent-line ring
/// and its enabled-line bitmap live inside the loaded header data, so they
/// last as long as the header is registered.
#[derive(Clone, Debug)]
pub struct SpeechHeader {
    /// Its archive entry (`...\Name.hdr`).
    pub name: String,
    /// Bytes 0..2: the header type a descriptor names.
    pub kind: u16,
    /// Bytes 2..4: the id a keyed descriptor matches (`FUN_00363408`).
    pub id: u16,
    /// Its slot in the shared header table (`FUN_00363060`).
    pub slot: u16,
    /// After the lines, one word per metadata column: every line's values
    /// in that column (`FUN_00364c38`'s early rejection).
    columns: Vec<u32>,
    /// Each line's start (its big-endian u16 times 0x100, shifted left by
    /// byte 7: a byte offset in the `.dat`, or a player line's lookup key)
    /// and metadata bytes. Byte 7 is 1 in the headers whose `.dat` passes
    /// 16 MiB / 2 of lines (`TutLessonsB`, `DunkI`..`DunkN`, `SpecIntrosC`).
    pub lines: Vec<(u32, Vec<u8>)>,
    /// Byte 6 entries of recent line indices and the write cursor before
    /// them (`FUN_00363480`, `FUN_00363538`).
    ring: Vec<u8>,
    cursor: u8,
    /// Byte 4 bit 7: the lines are split into this many runs, one enabled
    /// per session (`FUN_003635d0`), and the bitmap of enabled lines.
    partitions: Option<u8>,
    enabled: Vec<bool>,
    /// Bytes 10..12: how many sub-keys a key-type-2 descriptor may name
    /// (`FUN_00363350`), and after the ring one bit per line and sub-key,
    /// `count * key + line` (`FUN_00364af0`). Only `ScoreUpdate.hdr` has
    /// any (22, keyed by the score's margin).
    sub_keys: u16,
    sub_enabled: Vec<bool>,
}

impl SpeechHeader {
    /// Parses a header as `FUN_00363060` accepts it: its ring must hold its
    /// cursor at 0 and every entry free (0xff), or it is not registered.
    pub fn parse(name: &str, h: &[u8], slot: u16) -> Result<Option<Self>> {
        let header = bytes(h, 0, 12)?;
        let columns = usize::from(header[4] & 0xf);
        let count = usize::from(header[5]);
        let stride = columns + 2;
        let shift = u32::from(header[7] & 0xf);
        let mut lines = Vec::with_capacity(count);
        for line in 0..count {
            let v = bytes(h, 12 + stride * line, stride)?;
            lines.push((
                (u32::from(u16::from_be_bytes([v[0], v[1]])) * 0x100) << shift,
                v[2..].to_vec(),
            ));
        }
        let masks = (count * stride + 0xf) & 0x3ffc;
        let column_words = (0..columns)
            .map(|j| u32_at(h, masks + 4 * j))
            .collect::<Result<Vec<_>>>()?;
        let mut at = masks + 4 * columns;
        let (partitions, enabled) = if header[4] & 0x80 != 0 {
            let map = bytes(h, at, ((count + 7) >> 3) + 1)?;
            at += map.len();
            (
                Some(map[0]),
                (0..count)
                    .map(|i| map[(i >> 3) + 1] & (1 << (i & 7)) != 0)
                    .collect(),
            )
        } else {
            (None, vec![true; count])
        };
        let ring_len = usize::from(header[6]);
        let (cursor, ring) = if ring_len == 0 {
            (0, Vec::new())
        } else {
            let ring = bytes(h, at, ring_len + 1)?;
            at += ring.len();
            (ring[0], ring[1..].to_vec())
        };
        let sub_keys = u16::from_le_bytes([header[10], header[11]]);
        let sub_enabled = if sub_keys == 0 || sub_keys == 0xffff {
            Vec::new()
        } else {
            let bits = count * usize::from(sub_keys);
            let map = bytes(h, at, bits.div_ceil(8))?;
            (0..bits)
                .map(|i| map[i >> 3] & (1 << (i & 7)) != 0)
                .collect()
        };
        if cursor != 0 || ring.iter().any(|&r| r != 0xff) {
            return Ok(None);
        }
        Ok(Some(SpeechHeader {
            name: name.to_owned(),
            kind: u16::from_le_bytes([header[0], header[1]]),
            id: u16::from_le_bytes([header[2], header[3]]),
            slot,
            columns: column_words,
            lines,
            ring,
            cursor,
            partitions,
            enabled,
            sub_keys,
            sub_enabled,
        }))
    }

    /// The `.dat` holding its lines (`FUN_001adac8`: the name cut at
    /// `_Clone`, or its extension dropped, then `.dat`).
    pub fn dat(&self) -> String {
        let stem = match self.name.find("_Clone") {
            Some(at) => &self.name[..at],
            None => self.name.strip_suffix(".hdr").unwrap_or(&self.name),
        };
        format!("{stem}.dat")
    }

    /// The recent-line ring's length (header byte 6).
    pub fn ring_len(&self) -> usize {
        self.ring.len()
    }

    /// The ring's lines, newest first (empty entries skipped).
    pub fn recent(&self) -> Vec<usize> {
        let n = self.ring.len();
        (1..=n)
            .map(|back| self.ring[(usize::from(self.cursor) + n - back) % n])
            .filter(|&line| line != 0xff)
            .map(usize::from)
            .collect()
    }

    /// `FUN_00363480`: how many uses ago `line` was chosen (1 the latest),
    /// 0 when it is not in the ring.
    pub fn age(&self, line: usize) -> usize {
        let n = self.ring.len();
        let mut index = usize::from(self.cursor);
        for age in 1..=n {
            index = if index == 0 { n - 1 } else { index - 1 };
            if usize::from(self.ring[index]) == line {
                return age;
            }
        }
        0
    }

    /// `FUN_00363538`: remembers `line` at the cursor, which wraps.
    fn push(&mut self, line: usize) {
        let n = self.ring.len();
        if n == 0 {
            return;
        }
        let cursor = usize::from(self.cursor);
        if cursor < n {
            self.ring[cursor] = line as u8;
        }
        self.cursor = if cursor + 1 >= n { 0 } else { cursor as u8 + 1 };
    }

    /// `FUN_00364af0` with no sub-key: whether `line` may be chosen.
    pub fn enabled(&self, line: usize) -> bool {
        self.enabled.get(line).copied().unwrap_or(false)
    }

    /// `FUN_00364af0` under `sub` (a key-type-2 descriptor's word): the
    /// line's bit for that key, then its partition bit.
    pub fn enabled_for(&self, line: usize, sub: Option<u16>) -> bool {
        let Some(sub) = sub else {
            return self.enabled(line);
        };
        sub < self.sub_keys
            && self
                .sub_enabled
                .get(self.lines.len() * usize::from(sub) + line)
                .copied()
                .unwrap_or(false)
            && self.enabled(line)
    }

    /// `FUN_00363350`: whether a key-type-2 descriptor may key this header
    /// by `word` (below its sub-key count, which 0xffff disables).
    pub fn takes_sub_key(&self, word: u32) -> bool {
        self.sub_keys != 0xffff && word < u32::from(self.sub_keys)
    }

    /// The number of runs a partitioned header's lines are split into.
    pub fn partitions(&self) -> Option<u8> {
        self.partitions
    }

    /// `FUN_00363688` then `FUN_003635d0`: switches every line off, then on
    /// for run `seed % partitions`, lines `k*n/p .. (k*n+n)/p`.
    pub fn partition(&mut self, seed: u32) {
        let Some(p) = self.partitions.filter(|&p| p != 0) else {
            return;
        };
        let n = self.lines.len() as u32;
        let k = seed % u32::from(p);
        let start = k * n / u32::from(p);
        let end = (k * n + n) / u32::from(p);
        for (i, on) in self.enabled.iter_mut().enumerate() {
            *on = (start..end).contains(&(i as u32));
        }
    }

    /// `FUN_00365ba0` after a line is posted: a partitioned header's line
    /// is switched off, then the ring remembers it.
    fn used(&mut self, line: usize) {
        if self.partitions.is_some()
            && CONSUME_PARTITIONED_LINES
            && let Some(on) = self.enabled.get_mut(line)
        {
            *on = false;
        }
        self.push(line);
    }
}

/// A `c0 fb` header archive registered from `first_slot` on.
#[derive(Clone, Debug, Default)]
pub struct HeaderTable {
    pub headers: Vec<SpeechHeader>,
}

impl HeaderTable {
    pub fn parse(archive: &[u8], first_slot: u16) -> Result<Self> {
        let mut headers = Vec::new();
        for entry in street_assets::locale::c0fb(archive)? {
            let h = bytes(archive, entry.offset as usize, entry.size as usize)?;
            let slot = first_slot + headers.len() as u16;
            if let Some(header) = SpeechHeader::parse(&entry.name, h, slot)? {
                headers.push(header);
            }
        }
        Ok(HeaderTable { headers })
    }

    /// `FUN_003633a0`: the first header of `kind`.
    pub fn by_kind(&self, kind: u16) -> Option<usize> {
        self.headers.iter().position(|h| h.kind == kind)
    }

    /// `FUN_00363408`: the first header of `kind` whose id is `id`.
    pub fn by_id(&self, kind: u16, id: u16) -> Option<usize> {
        self.headers
            .iter()
            .position(|h| h.kind == kind && h.id == id)
    }

    pub fn by_slot(&self, slot: u16) -> Option<&SpeechHeader> {
        self.headers.iter().find(|h| h.slot == slot)
    }

    /// Every partitioned header takes run `seed % partitions`.
    pub fn partition(&mut self, seed: u32) {
        for header in &mut self.headers {
            header.partition(seed);
        }
    }
}

// ---------------------------------------------------------------------------
// Events

/// One branch of an event: a weight, a chance and the header descriptor it
/// gathers from. Only the forms the disc's speech events use are accepted
/// (one descriptor, no root predicates, normal choice mode).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventBranch {
    pub weight: u32,
    pub chance: u8,
    /// The header type, and how the header is found: key type 0 the first of
    /// the type (`FUN_003633a0`), 1 the one whose id is request word
    /// `key_word` (`FUN_00363408`), 2 the first of the type with request
    /// word `key_word` as its sub-key (`FUN_00363350`).
    pub kind: u16,
    pub key_word: u8,
    pub key_type: u8,
    /// Per metadata column, the request word its value must be a bit of
    /// (0 none) and the values allowed at all (`FUN_00364938`).
    pub selectors: Vec<u8>,
    pub masks: Vec<u32>,
    /// After the descriptor indices, one word per root word predicate: the
    /// branch passes when a word is 0 or shares a bit with its predicate's
    /// request word (`FUN_00366240`).
    pub word_masks: Vec<u32>,
}

/// An event's root (`FUN_00363708`) and branches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpeechEvent {
    pub id: u16,
    /// Root +2: how long (VBlanks) a queued request may wait.
    pub hold: u16,
    /// Root +4: its queue priority.
    pub priority: u16,
    /// Root +9: the request's acceptance chance (`FUN_00363a10`).
    pub chance: u8,
    /// Root +7's high nibble counts the word predicates, three bytes each
    /// after the branch offsets and the (absent) callback predicates; the
    /// second byte names the request word (`FUN_00366240`). Dunk (1) and
    /// Shot (0xc) key their game-length variants by word 7 or 4.
    pub word_predicates: Vec<u8>,
    /// Root +8: how many times running the manager may post this event
    /// (`FUN_00366320` against `DAT_00422b28`); 0 is no limit. Only
    /// MissedDunk (0xe) has one, 1.
    pub repeat: u8,
    pub branches: Vec<EventBranch>,
}

impl SpeechEvent {
    /// Reads root `id` of an event file.
    pub fn parse(evt: &[u8], id: u16) -> Result<Self> {
        let roots = u32_at(evt, 16)? as usize;
        let root = (0..roots)
            .find_map(|i| {
                let at = usize::from(u16_at(evt, 24 + 2 * i).ok()?) * 4;
                (u16_at(evt, at).ok()? == id).then_some(at)
            })
            .ok_or_else(|| unsupported(format!("event {id:#x}: no root")))?;
        let r = bytes(evt, root, 12)?;
        // +7's low nibble callback predicates (`FUN_00366a48`), +10 flags
        // (linked, filtered, kept older requests, mode 1): none in the
        // speech events. Word predicates (+7's high nibble) and the repeat
        // limit (+8) are kept.
        if r[7] & 0xf != 0 || r[10] != 0 {
            return Err(unsupported(format!("event {id:#x} root")));
        }
        let predicates = root + ((usize::from(r[6]) * 2 + 3) & !3) + 0xc;
        let word_predicates = (0..usize::from(r[7] >> 4))
            .map(|k| Ok(bytes(evt, predicates + 3 * k, 3)?[1]))
            .collect::<Result<Vec<u8>>>()?;
        if word_predicates.iter().any(|&w| w >= 20) {
            return Err(unsupported(format!("event {id:#x} predicates")));
        }
        let mut branches = Vec::new();
        for b in 0..usize::from(r[6]) {
            let at = root + usize::from(u16_at(evt, root + 12 + 2 * b)?) * 4;
            let branch = bytes(evt, at, 9)?;
            // +2/+3 condition bits against no root predicates
            // (`FUN_00366bf8`), +4 one descriptor in mode 0 or 2
            // (`FUN_003653f8`), +5 the plain keyed choice (`FUN_00365928`).
            if branch[2] & branch[3] != 0
                || branch[4] & 3 == 1
                || branch[4] >> 2 != 1
                || branch[5] != 0
            {
                return Err(unsupported(format!("event {id:#x} branch {b}")));
            }
            let descriptors = usize::from(branch[4] >> 2);
            let words = at + ((descriptors + 3) & !3) + 8;
            let word_masks = (0..word_predicates.len())
                .map(|k| u32_at(evt, words + 4 * k))
                .collect::<Result<Vec<_>>>()?;
            let d = at + usize::from(branch[8]) * 4;
            let descriptor = bytes(evt, d, 8)?;
            let count = usize::from(descriptor[4]);
            let selectors = bytes(evt, d + 8, count)?.to_vec();
            if count >= 5
                || descriptor[3] > 2
                || descriptor[2] >= 20
                || selectors.iter().any(|&s| s >= 20)
            {
                return Err(unsupported(format!("event {id:#x} descriptor {b}")));
            }
            branches.push(EventBranch {
                weight: u32::from(branch[0] & 0x1f) * WEIGHT_SCALE[usize::from(branch[0] >> 5)],
                chance: branch[1],
                kind: u16_at(evt, d)?,
                key_word: descriptor[2],
                key_type: descriptor[3],
                selectors,
                masks: (0..count)
                    .map(|j| u32_at(evt, d + 0x14 + 4 * j))
                    .collect::<Result<_>>()?,
                word_masks,
            });
        }
        Ok(SpeechEvent {
            id,
            hold: u16::from_le_bytes([r[2], r[3]]),
            priority: u16::from_le_bytes([r[4], r[5]]),
            chance: r[9],
            word_predicates,
            repeat: r[8],
            branches,
        })
    }

    /// `FUN_00365020`: the branches in a weighted random order without
    /// replacement, every positive-weight draw taken, the weightless ones
    /// after in their own order.
    pub fn order(&self, random: &mut PlayerSelectorRandom) -> Vec<usize> {
        let mut weights: Vec<u32> = self.branches.iter().map(|b| b.weight).collect();
        let mut total: u32 = weights.iter().sum();
        let mut order = Vec::with_capacity(weights.len());
        while total >= 1 {
            let draw = random.mode0_index(total as usize, u16::MAX).unwrap_or(0) as i64;
            let mut rest = draw - i64::from(weights[0]);
            let mut pick = 0;
            if rest >= 0 {
                pick = 1;
                while pick < weights.len() {
                    rest -= i64::from(weights[pick]);
                    if rest < 0 {
                        break;
                    }
                    pick += 1;
                }
            }
            let pick = pick.min(weights.len() - 1);
            order.push(pick);
            total -= weights[pick];
            weights[pick] = 0;
        }
        order.extend((0..self.branches.len()).filter(|&b| self.branches[b].weight == 0));
        order
    }

    /// `FUN_00366320`'s branch filter after the chance draw
    /// (`FUN_00366240`): with word predicates, one of the branch's words is
    /// 0 or meets its request word.
    fn branch_passes(&self, b: &EventBranch, packet: &Packet) -> bool {
        self.word_predicates.is_empty()
            || self
                .word_predicates
                .iter()
                .zip(&b.word_masks)
                .any(|(&word, &mask)| mask == 0 || mask & packet[usize::from(word)] != 0)
    }

    /// `FUN_00366320` for this event: each branch in turn takes its chance
    /// draw, passes the word predicates, finds its header, gathers the
    /// eligible lines and, when there are any, picks one with a keyed draw
    /// (`FUN_00365928`).
    pub fn choose(
        &self,
        packet: &Packet,
        table: &HeaderTable,
        random: &mut PlayerSelectorRandom,
    ) -> Option<Choice> {
        for branch in self.order(random) {
            let b = &self.branches[branch];
            let draw = random.mode0_index(100, u16::MAX)?;
            if draw >= usize::from(b.chance) || !self.branch_passes(b, packet) {
                continue;
            }
            let word = packet[usize::from(b.key_word)];
            let (header, sub) = match b.key_type {
                0 => (table.by_kind(b.kind), None),
                1 => (table.by_id(b.kind, word as u16), None),
                _ => (
                    table
                        .by_kind(b.kind)
                        .filter(|&h| table.headers[h].takes_sub_key(word)),
                    Some(word as u16),
                ),
            };
            let Some(index) = header else { continue };
            let h = &table.headers[index];
            let candidates = gather(h, b, packet, sub);
            if candidates.is_empty() {
                continue;
            }
            let pick = random.mode0_index(candidates.len(), h.slot)?;
            let line = candidates[pick];
            return Some(Choice {
                branch,
                header: index,
                slot: h.slot,
                line,
                start: h.lines[line].0,
            });
        }
        None
    }
}

/// A chosen line: the branch, the header (its index and table slot) and
/// the line with its start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    pub branch: usize,
    pub header: usize,
    pub slot: u16,
    pub line: usize,
    pub start: u32,
}

impl Choice {
    /// `FUN_00365ba0`: the posted line is used up in its header.
    pub fn commit(&self, table: &mut HeaderTable) {
        if let Some(h) = table.headers.get_mut(self.header) {
            h.used(self.line);
        }
    }
}

/// `FUN_00364938`: a line's metadata against the descriptor's masks and the
/// request words its selectors name.
fn line_passes(branch: &EventBranch, packet: &Packet, metadata: &[u8]) -> bool {
    branch.masks.iter().enumerate().all(|(j, &mask)| {
        let Some(&v) = metadata.get(j) else {
            return false;
        };
        if v > 31 || mask & (1 << v) == 0 {
            return false;
        }
        match branch.selectors[j] {
            0 => true,
            s => packet[usize::from(s)] & (1 << v) != 0,
        }
    })
}

/// `FUN_00364c38`: a request word sharing no bit with its column's words
/// rules the header out; then the enabled lines that pass, fresh ones (not
/// in the ring) in header order up to 200, else the eligible line chosen
/// longest ago.
fn gather(h: &SpeechHeader, branch: &EventBranch, packet: &Packet, sub: Option<u16>) -> Vec<usize> {
    for (j, &column) in h.columns.iter().enumerate() {
        let s = branch.selectors.get(j).copied().unwrap_or(0);
        if s != 0 && column & packet[usize::from(s)] == 0 {
            return Vec::new();
        }
    }
    let mut fresh = Vec::new();
    let mut oldest: Option<(usize, usize)> = None;
    for (i, (_, metadata)) in h.lines.iter().enumerate() {
        if !line_passes(branch, packet, metadata) || !h.enabled_for(i, sub) {
            continue;
        }
        let age = h.age(i);
        if age < 1 {
            if fresh.len() < 200 {
                fresh.push(i);
            }
        } else if oldest.is_none_or(|(best, _)| best < age) {
            oldest = Some((age, i));
        }
    }
    if fresh.is_empty()
        && let Some((_, line)) = oldest
    {
        fresh.push(line);
    }
    fresh
}

// ---------------------------------------------------------------------------
// The commentator

/// The commentary events the rewrite posts through the native chooser
/// (every `FUN_001ad178` caller but the tutorial's 9 and 0xa): 0 a block
/// (`0x1a4ce8`), 1 a dunk, 0xb the score, 0xc a shot and 0xf the winning
/// basket (`0x1a3b10`), 2 a gamebreaker's basket (`0x1a6b80`), 3 the court
/// (`0x1a56b0`), 4 a trick move (`0x1a5a78`), 6 the end (`0x1a5638`), 8 a
/// steal (`0x1a63e0`), 0xd a turnover (every fifth steal, every third shot
/// clock violation), 0xe a miss (`0x1a4550`), 0x11 the shot clock
/// (`0x1a6d88`), 0x12 a nickname (`0x1a83e0`), 0x13 a passout off a head (`0x1a5d50`,
/// slot 39: `PassOut.hdr`), 9 Street School's narration (`0x1ad510`:
/// `Tutorial\TutLessonsA/B`).
pub const COMMENTARY_EVENT_IDS: [u16; 16] = [
    0, 1, 2, 3, 4, 6, 8, 9, 0xb, 0xc, 0xd, 0xe, 0xf, 0x11, 0x12, 0x13,
];

/// What Street School's narration asks for (`0x1ad510`, the sound
/// listener's slots 78, 79 and 82 through the speech manager).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lesson {
    /// Slot 78, the lesson's launch (`FUN_0018ec28`): its introduction
    /// (word 3 = 1), once nothing is said and no tutorial page is due.
    Intro,
    /// Slot 79, the lesson's result at its commit (`FUN_0018eec0`): word 3
    /// is 2 passed, 4 failed (+0x4d4).
    Result { passed: bool },
    /// Slot 82, a tutorial page (`0x3c6990`), read once nothing is said:
    /// word 3 = 8, the page's own entry.
    Page,
}

/// `0x1ad510`'s table (`0x45d810`, 31 cases): an index, a lesson's (the
/// game's vt+0xb0) or a tutorial page's (26..30), as event 9's word 1 (the
/// group 1, 2 or 4) and word 2 (its bit). Index 28 is the Shot Clock page
/// read (`Lesson::Page`), else a lesson's bit 0x80000. Out of the table, or
/// group 0, matches no line.
pub fn lesson_words(index: usize, kind: Lesson) -> (u32, u32) {
    const TABLE: [(u32, u32); 31] = [
        (1, 1),
        (1, 2),
        (1, 4),
        (1, 0x20),
        (1, 0x8000_0000),
        (1, 0x80),
        (1, 0x200),
        (2, 0x400),
        (2, 0x800),
        (2, 0x1000),
        (2, 0x2000),
        (2, 0x4000_0000),
        (4, 0x4000),
        (4, 0x8000),
        (4, 0x2_0000),
        (4, 0x8_0000),
        (4, 0x10_0000),
        (1, 0x2000_0000),
        (4, 0x40_0000),
        (4, 0x80_0000),
        (4, 0x100_0000),
        (4, 0x20),
        (4, 0x200_0000),
        (4, 0x400_0000),
        (4, 0x800_0000),
        (4, 0x1000_0000),
        (1, 0x10),
        (0, 0x80),
        (4, 0x8_0000),
        (1, 0x100),
        (4, 0x1_0000),
    ];
    if index == 28 && kind == Lesson::Page {
        return (1, 8);
    }
    TABLE.get(index).copied().unwrap_or((0, 0))
}

/// Event 9's packet for `index` (`0x1ad510`'s post: words 1..3).
pub fn lesson_packet(index: usize, kind: Lesson) -> Packet {
    let (group, bit) = lesson_words(index, kind);
    let situation = match kind {
        Lesson::Intro => 1,
        Lesson::Result { passed: true } => 2,
        Lesson::Result { passed: false } => 4,
        Lesson::Page => 8,
    };
    words_packet(9, &[group, bit, situation])
}

/// The commentary archive's index, its headers and events.
#[derive(Clone)]
pub struct Commentary {
    pub entries: Vec<BigEntry>,
    pub headers: HeaderTable,
    events: BTreeMap<u16, SpeechEvent>,
}

/// A native commentary choice: its archive file and byte offset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedCommentary {
    pub name: String,
    pub offset: u32,
    pub choice: Choice,
}

/// The lines of one `.dat` file, ready to decode one at a time.
pub struct Lines {
    pub name: String,
    pub data: Vec<u8>,
    pub streams: Vec<Stream>,
}

impl Commentary {
    /// Reads the commentary's events and headers. The headers take the
    /// table slots after the players' (`FUN_001ac6f0` registers
    /// `plyrhdrs.big` first).
    pub fn open<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Self> {
        let first = street_assets::locale::c0fb(&iso.read(PLAYER_HEADERS)?)?.len() as u16;
        let headers = HeaderTable::parse(&iso.read(COMMENTARY_HEADERS)?, first)?;
        let evt = iso.read(COMMENTARY_EVENTS)?;
        let events = COMMENTARY_EVENT_IDS
            .into_iter()
            .map(|id| Ok((id, SpeechEvent::parse(&evt, id)?)))
            .collect::<Result<_>>()?;
        Ok(Commentary {
            entries: big::read_index(iso, ARCHIVE)?,
            headers,
            events,
        })
    }

    pub fn event(&self, id: u16) -> Option<&SpeechEvent> {
        self.events.get(&id)
    }

    /// The event's choice for `packet`, without using the line up.
    pub fn choose(
        &self,
        packet: &Packet,
        random: &mut PlayerSelectorRandom,
    ) -> Option<SelectedCommentary> {
        let event = self.events.get(&(packet[0] as u16))?;
        let choice = event.choose(packet, &self.headers, random)?;
        let header = &self.headers.headers[choice.header];
        Some(SelectedCommentary {
            name: header.dat(),
            offset: choice.start,
            choice,
        })
    }

    pub fn commit(&mut self, selected: &SelectedCommentary) {
        selected.choice.commit(&mut self.headers);
    }

    /// The session's runs of the partitioned headers (`FUN_00363060` at
    /// registration, `FUN_00363230` at each later initialisation).
    pub fn partition(&mut self, seed: u32) {
        self.headers.partition(seed);
    }

    /// The files whose name starts with `prefix` (case-insensitive, `\`
    /// separated), without the `_Clone` copies.
    pub fn files(&self, prefix: &str) -> impl Iterator<Item = &BigEntry> {
        let prefix = prefix.to_ascii_lowercase();
        self.entries.iter().filter(move |e| {
            let n = e.name.to_ascii_lowercase();
            n.starts_with(&prefix) && !n.contains("_clone")
        })
    }

    /// Reads one file and finds its lines.
    pub fn lines<R: Read + Seek>(&self, iso: &mut Iso<R>, entry: &BigEntry) -> Result<Lines> {
        let data = big::read_entry(iso, ARCHIVE, entry)?;
        let streams = schl::streams(&data)?;
        Ok(Lines {
            name: entry.name.clone(),
            data,
            streams,
        })
    }
}

/// `0x1ad178` event 4 from `0x1a5a78`: words 1, the actor's sex and the
/// nearest opponent's.
pub fn move_packet(female: bool, opponent: Option<bool>) -> Packet {
    let mut p = [0; 20];
    p[..4].copy_from_slice(&[4, 1, sex_mask(Some(female)), sex_mask(opponent)]);
    p
}

/// Event 0 from `0x1a4ce8`: the blocker's nickname row (`FUN_001a8310`, 0
/// for none), the shooter's sex (the listener's last shooter, +0x5c) and
/// the blocker's.
pub fn block_packet(alias: u16, shooter_female: Option<bool>, blocker_female: bool) -> Packet {
    let mut p = [0; 20];
    p[..4].copy_from_slice(&[
        0,
        u32::from(alias),
        sex_mask(shooter_female),
        sex_mask(Some(blocker_female)),
    ]);
    p
}

/// Event 8 from `0x1a63e0`: the stealer's nickname row, the stealer's sex
/// and the robbed player's.
pub fn steal_packet(alias: u16, stealer_female: bool, victim_female: Option<bool>) -> Packet {
    let mut p = [0; 20];
    p[..4].copy_from_slice(&[
        8,
        u32::from(alias),
        sex_mask(Some(stealer_female)),
        sex_mask(victim_female),
    ]);
    p
}

/// Event 0x12 from `0x1a83e0`: the nickname id and the actor's sex.
pub fn nickname_packet(id: u16, female: bool) -> Packet {
    let mut p = [0; 20];
    p[..3].copy_from_slice(&[0x12, u32::from(id), sex_mask(Some(female))]);
    p
}

/// A packet of `words` after the event id.
pub fn words_packet(event: u16, words: &[u32]) -> Packet {
    let mut p = [0; 20];
    p[0] = u32::from(event);
    p[1..=words.len()].copy_from_slice(words);
    p
}

/// `0x1ac6f0`'s length word (manager +0x60) from the points to win
/// (profile +0xf0): below 16 is 1, 26 2, 36 4, 43 8, else 0x10. Dunk (1)
/// and Shot (0xc) key their `_Clone` headers by it.
pub fn game_length_word(points_to_win: u32) -> u32 {
    match points_to_win {
        0..16 => 1,
        16..26 => 2,
        26..36 => 4,
        36..43 => 8,
        _ => 0x10,
    }
}

/// `FUN_001a7800`: the dunker's Street Legend bit from their roster
/// record's field 0, the persistent player id (`FUN_001da4e0(record, 0)`
/// on the actor's record `+0x288`): Biggie Little 147, Bonafide 146, Dime
/// 148, Osmosis 151, Stretch 145 and Whitewater 149; anyone else 0.
pub fn street_legend_word(id: i16) -> u32 {
    match id {
        145 => 0x10,
        146 => 2,
        147 => 1,
        148 => 4,
        149 => 0x20,
        151 => 8,
        _ => 0,
    }
}

/// `FUN_001a7948`: the Legend bit of a persistent id (the Legends'
/// own records; Jordan's is 167, not his Wizards 140), which the caller
/// sends as 0 while the listener's +0x74 is set ([`names_off`]).
pub fn league_legend_word(id: i16) -> u32 {
    match id {
        152 => 0x1000000, // West
        153 => 0x800,     // Baylor
        154 => 0x10,      // Maravich
        155 => 0x80,      // Gervin
        156 => 0x100000,  // Thompson
        157 => 2,         // Bird
        158 => 0x100,     // Wilkins
        159 => 0x40000,   // Thomas
        160 => 4,         // Johnson
        161 => 0x10000,   // Worthy
        162 => 0x80000,   // Malone
        163 => 8,         // Chamberlain
        164 => 0x400,     // Russell
        165 => 0x20000,   // Archibald
        166 => 0x20,      // Dawkins
        167 => 0x800000,  // Jordan
        168 => 0x400000,  // Cousy
        169 => 1,         // Erving
        170 => 0x1000,    // Drexler
        171 => 0x4000,    // Hawkins
        172 => 0x2000,    // Monroe
        229 => 0x8000,    // Robertson
        230 => 0x200,     // Frazier
        231 => 0x200000,  // Barry
        _ => 0,
    }
}

/// `FUN_001a7bd0`: the league star bit of a persistent id, 0 under the same
/// flag as [`league_legend_word`].
pub fn league_star_word(id: i16) -> u32 {
    match id {
        5 => 1,          // Pierce
        16 => 0x200,     // Andre Miller
        20 => 0x10,      // Finley
        21 => 0x20,      // Nowitzki
        22 => 0x40,      // Nash
        30 => 0x1000000, // Stackhouse
        32 => 0x80,      // Wallace
        40 => 0x100,     // Francis
        50 => 0x400,     // Odom
        51 => 4,         // Miles
        55 => 0x800,     // O'Neal
        56 => 0x1000,    // Bryant
        60 => 0x2000,    // Gasol
        75 => 0x4000,    // Garnett
        80 => 0x8000,    // Kidd
        82 => 0x10000,   // Martin
        86 => 0x20000,   // Davis
        95 => 0x40000,   // McGrady
        100 => 0x80000,  // Iverson
        105 => 0x100000, // Marbury
        115 => 0x200000, // Webber
        116 => 0x400000, // Bibby
        130 => 0x800000, // Carter
        501 => 2,        // Jay Williams
        512 => 8,        // Wagner
        _ => 0,
    }
}

/// Event 1's three identity words for a dunker with persistent id `id`:
/// the Street Legend, Legend and league star bits, the last two 0 while
/// the listener's +0x74 is set.
pub fn dunker_words(id: i16, names_off: bool) -> [u32; 3] {
    if names_off {
        [street_legend_word(id), 0, 0]
    } else {
        [
            street_legend_word(id),
            league_legend_word(id),
            league_star_word(id),
        ]
    }
}

/// `FUN_001a56b0` at slot 61's first call: the listener's +0x74 is set when
/// fewer than two of the players have a star or legend bit (the flag is
/// read as the reset left it, clear, while they are counted).
pub fn names_off(ids: impl IntoIterator<Item = i16>) -> bool {
    ids.into_iter()
        .filter(|&id| league_star_word(id) != 0 || league_legend_word(id) != 0)
        .count()
        < 2
}

/// Event 1 from `0x1a3b10` as `0x1ad178` sends it (seven words; the
/// caller's fifth argument, `FUN_001a7e78`, and its last, `FUN_001a85f0`,
/// are not passed on, raw `0x1ad298..0x1ad2d8`): an alley-oop finish
/// (the dunker's +0xa4 bit 15) 2 else 1, the dunker's three identity bit
/// words ([`dunker_words`]: `FUN_001a7800`, `FUN_001a7948`,
/// `FUN_001a7bd0`), the dunker's sex, their nickname row and the game
/// length.
pub fn dunk_packet(
    alley_oop: bool,
    identity: [u32; 3],
    female: bool,
    alias: u16,
    length: u32,
) -> Packet {
    words_packet(
        1,
        &[
            if alley_oop { 2 } else { 1 },
            identity[0],
            identity[1],
            identity[2],
            sex_mask(Some(female)),
            u32::from(alias),
            length,
        ],
    )
}

/// Event 0xc from `0x1a3b10`: a shot of 3 points or more 2 else 1, the
/// shooter's nickname row and sex, and the game length.
pub fn shot_packet(points: u32, alias: u16, female: bool, length: u32) -> Packet {
    words_packet(
        0xc,
        &[
            if points < 3 { 1 } else { 2 },
            u32::from(alias),
            sex_mask(Some(female)),
            length,
        ],
    )
}

impl Lines {
    pub fn len(&self) -> usize {
        self.streams.len()
    }

    pub fn is_empty(&self) -> bool {
        self.streams.is_empty()
    }

    pub fn clip(&self, i: usize) -> Option<crate::Clip> {
        self.streams.get(i).map(|s| s.decode(&self.data))
    }
}

// ---------------------------------------------------------------------------
// The players' own lines (`/SPEECH/PLYRHDRS.BIG`, `SPCH_PLY.AST`)

pub const PLAYER_HEADERS: &str = "/SPEECH/PLYRHDRS.BIG";
pub const PLAYER_BANK: &str = "/SPEECH/SPCH_PLY.ABK";
pub const PLAYER_STREAMS: &str = "/SPEECH/SPCH_PLY.AST";

/// The players' event (`0x1ad068` posts `FUN_00364600(1, 0, 0x2000)`).
pub const PLAYER_EVENT: u16 = 0x2000;

/// One of a voice's lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerLine {
    /// What it's for: the bit the game asks with (`0x1ad068`, `1 << category`).
    pub category: u8,
    /// The second metadata byte, filtered by the request's last word.
    pub mood: u8,
    /// The line's start in the header, in 256-byte units. `0x1af230`
    /// receives this value multiplied by `0x100`.
    pub start: u16,
    /// Its PLAY selector from the game's lookup table.
    pub stream: Option<u16>,
}

/// Every voice's lines: voices 1..16 (shared by the league players, picked by
/// skin tone) and the streetball legends' own (145..151), with the headers
/// and event the middleware chooses them by.
#[derive(Clone, Debug, Default)]
pub struct PlayerSpeech {
    pub voices: BTreeMap<u16, Vec<PlayerLine>>,
    /// Type-0x2003 shot headers, resolved by `FUN_001c4b20`.
    pub shot_voices: BTreeMap<u16, Vec<PlayerLine>>,
    pub headers: HeaderTable,
    event: Option<SpeechEvent>,
}

/// Request words 1..4 of `PLAYERS.EVT` event 0x2000 (`0x1ad068`: the
/// voice, its fourth argument, its fifth, its third).
#[derive(Clone, Copy, Debug)]
pub struct PlayerRequest {
    pub voice: u16,
    pub category_mask: u32,
    pub shot_mask: u32,
    pub action_mask: u32,
}

impl PlayerRequest {
    pub fn packet(&self) -> Packet {
        let mut p = [0; 20];
        p[..5].copy_from_slice(&[
            0x0100_0000 | u32::from(PLAYER_EVENT),
            u32::from(self.voice),
            self.category_mask,
            self.shot_mask,
            self.action_mask,
        ]);
        p
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerCatalog {
    Action,
    Shot,
}

/// A chosen line and the header it is used up in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedPlayerLine {
    pub catalog: PlayerCatalog,
    pub ordinal: usize,
    pub stream: u16,
    pub history_key: u16,
    pub choice: Choice,
}

impl PlayerSpeech {
    /// Reads the voices' headers (a `c0 fb` archive) and the event, and
    /// resolves each line with the game's per-voice lookup (`0x1af230`,
    /// shots `0x1c4b20`).
    ///
    /// An `action\\NNN.hdr` (`02 20`, u16 voice, u8 columns, u8 line count,
    /// u8 ring length, ...) lists its lines as (u16 big-endian start in
    /// 256-byte units, u8 category, u8 mood). The game scales the start by
    /// `0x100`, dispatches on the voice, and returns the PLAY selector.
    pub fn load<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Self> {
        let headers = HeaderTable::parse(&iso.read(PLAYER_HEADERS)?, 0)?;
        let event = SpeechEvent::parse(&iso.read(PLAYER_EVENTS)?, PLAYER_EVENT)?;
        let mut voices = BTreeMap::new();
        let mut shot_voices = BTreeMap::new();
        for h in &headers.headers {
            let is_shot = match h.kind {
                0x2002 => false,
                0x2003 => true,
                _ => continue,
            };
            let lines = h
                .lines
                .iter()
                .map(|(start, metadata)| {
                    let start = (start >> 8) as u16;
                    PlayerLine {
                        category: metadata.first().copied().unwrap_or(0xff),
                        mood: metadata.get(1).copied().unwrap_or(0xff),
                        start,
                        stream: if is_shot {
                            crate::speech_lookup::lookup_shot(h.id, start)
                        } else {
                            crate::speech_lookup::lookup(h.id, start)
                        },
                    }
                })
                .collect();
            if is_shot {
                shot_voices.insert(h.id, lines);
            } else {
                voices.insert(h.id, lines);
            }
        }
        Ok(PlayerSpeech {
            voices,
            shot_voices,
            headers,
            event: Some(event),
        })
    }

    pub fn event(&self) -> Option<&SpeechEvent> {
        self.event.as_ref()
    }

    /// The placed lines of `voice` for `category`: (mood, stream).
    pub fn lines(&self, voice: u16, category: u8) -> impl Iterator<Item = (u8, u16)> + '_ {
        self.voices
            .get(&voice)
            .into_iter()
            .flatten()
            .filter(move |l| l.category == category)
            .filter_map(|l| Some((l.mood, l.stream?)))
    }

    fn header(&self, kind: u16, voice: u16) -> Option<&SpeechHeader> {
        self.headers
            .by_id(kind, voice)
            .map(|i| &self.headers.headers[i])
    }

    /// The action header's table slot: its key in `FUN_00366d70`'s ring.
    pub fn history_key(&self, voice: u16) -> Option<u16> {
        self.header(0x2002, voice).map(|h| h.slot)
    }

    pub fn shot_history_key(&self, voice: u16) -> Option<u16> {
        self.header(0x2003, voice).map(|h| h.slot)
    }

    /// The action header's ring length (byte 6).
    pub fn history_limit(&self, voice: u16) -> usize {
        self.header(0x2002, voice).map_or(0, SpeechHeader::ring_len)
    }

    pub fn shot_history_limit(&self, voice: u16) -> usize {
        self.header(0x2003, voice).map_or(0, SpeechHeader::ring_len)
    }

    /// Event 0x2000's choice for `request` (`FUN_00366320`), without using
    /// the line up.
    pub fn select_request(
        &self,
        request: PlayerRequest,
        random: &mut PlayerSelectorRandom,
    ) -> Option<SelectedPlayerLine> {
        let choice = self
            .event
            .as_ref()?
            .choose(&request.packet(), &self.headers, random)?;
        let header = &self.headers.headers[choice.header];
        let (catalog, lines) = match header.kind {
            0x2003 => (PlayerCatalog::Shot, self.shot_voices.get(&header.id)?),
            _ => (PlayerCatalog::Action, self.voices.get(&header.id)?),
        };
        Some(SelectedPlayerLine {
            catalog,
            ordinal: choice.line,
            stream: lines.get(choice.line)?.stream?,
            history_key: header.slot,
            choice,
        })
    }

    pub fn commit(&mut self, selected: &SelectedPlayerLine) {
        selected.choice.commit(&mut self.headers);
    }
}

/// The native speech middleware's six-word random generator and its shared
/// 32-result ring (`FUN_00366c80`, `FUN_00366d70`, `FUN_00366f40`).
///
/// One global state (`0x423038..0x42304c`) serves every draw of both
/// managers: request acceptance, branch order and chance, and line picks.
/// `FUN_00366f40` seeds it once, from the speech manager's counter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerSelectorRandom {
    state: [u32; 6],
    recent: [(u16, u16); 32],
    cursor: usize,
}

impl Default for PlayerSelectorRandom {
    fn default() -> Self {
        Self::new(0)
    }
}

impl PlayerSelectorRandom {
    pub fn new(manager_slot: u32) -> Self {
        let seed = manager_slot.wrapping_mul(0x1_0001);
        Self {
            state: [
                seed.wrapping_sub(0x0dd2_f1aa),
                seed.wrapping_sub(0x77ce_d917),
                seed.wrapping_sub(0x39db_22d1),
                seed.wrapping_add(0x0702_c49c),
                seed.wrapping_sub(0x61ca_c083),
                seed.wrapping_add(0x6fdf_3b64),
            ],
            recent: [(u16::MAX, u16::MAX); 32],
            cursor: 0,
        }
    }

    pub fn next_u32(&mut self) -> u32 {
        self.next()
    }

    fn next(&mut self) -> u32 {
        let [state0, state1, state2, state3, state4, state5] = self.state;
        let sum1 = state5.wrapping_add(state4);
        let carry1 = u32::from(sum1 < state5 || sum1 < state4);
        let sum2 = sum1.wrapping_add(state3).wrapping_add(carry1);
        let sum3 = sum2
            .wrapping_add(state2)
            .wrapping_add(u32::from(sum2 < state3));
        let next5 = state5.wrapping_add(1);
        let sum4 = sum3
            .wrapping_add(state1)
            .wrapping_add(u32::from(sum3 < state2));
        let next0 = sum4
            .wrapping_add(state0)
            .wrapping_add(u32::from(sum4 < state1));
        let mut next = [next0, sum4, sum3, sum2, sum1, next5];

        if next5 == 0 {
            next[4] = next[4].wrapping_add(1);
            if next[4] == 0 {
                next[3] = next[3].wrapping_add(1);
                if next[3] == 0 {
                    next[2] = next[2].wrapping_add(1);
                    if next[2] == 0 {
                        next[1] = next[1].wrapping_add(1);
                        if next[1] == 0 {
                            next[0] = next[0].wrapping_add(1);
                        }
                    }
                }
            }
        }
        self.state = next;
        next[0]
    }

    /// `FUN_00366d70`: map the RNG high word into `count`, retry recent
    /// results for this header-table key, remember the best candidate, and
    /// return the final draw. `u16::MAX` is the native no-history sentinel.
    pub fn mode0_index(&mut self, count: usize, key: u16) -> Option<usize> {
        let count = u32::try_from(count).ok()?;
        if count == 0 {
            return None;
        }
        let draw = |value: u32| ((u64::from(value >> 16) * u64::from(count)) >> 16) as usize;
        let mut best = draw(self.next());
        if key == u16::MAX {
            return Some(best);
        }

        let recent_limit = (count / 2).min(10) as usize;
        let mut best_age: Option<usize> = None;
        let mut result = best;
        for _ in 0..32 {
            let candidate = draw(self.next());
            result = candidate;
            let mut seen_for_key = 0;
            let mut age = None;
            if recent_limit != 0 {
                for offset in 0..=32 {
                    if seen_for_key >= recent_limit {
                        break;
                    }
                    let slot = (self.cursor + 32 - offset) % 32;
                    let (previous, previous_key) = self.recent[slot];
                    if previous_key == key {
                        if usize::from(previous) == candidate {
                            age = Some(seen_for_key);
                            break;
                        }
                        seen_for_key += 1;
                    }
                }
            }
            match age {
                None => {
                    best = candidate;
                    break;
                }
                Some(age) if best_age.is_none_or(|previous| age > previous) => {
                    best = candidate;
                    best_age = Some(age);
                }
                Some(_) => {}
            }
        }

        self.cursor = (self.cursor + 1) % 32;
        self.recent[self.cursor] = (best as u16, key);
        Some(result)
    }
}

/// The middleware's request queue capacity (`FUN_00363898`: 16 slots).
pub const QUEUE_CAPACITY: usize = 16;

/// A queued request (`FUN_00363a10`'s 0x60-byte slot): its manager, its
/// event's hold and priority, when it came (a VBlank count) with its
/// sequence number within that VBlank, and its 20 words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuedRequest {
    pub manager: usize,
    pub priority: u16,
    pub hold: u16,
    pub time: u32,
    pub sequence: u16,
    pub packet: Packet,
}

/// The middleware's request queue, shared by both managers.
#[derive(Clone, Debug, Default)]
pub struct SpeechQueue {
    slots: [Option<QueuedRequest>; QUEUE_CAPACITY],
    last_time: Option<u32>,
    sequence: u16,
}

impl SpeechQueue {
    /// `FUN_00363898` then the store in `FUN_00363a10`: the first free slot,
    /// else the first expired request, else the first request of the same
    /// manager whose priority is at most this one's; otherwise the request
    /// is lost. Returns the slot.
    pub fn enqueue(
        &mut self,
        event: &SpeechEvent,
        manager: usize,
        now: u32,
        packet: Packet,
    ) -> Option<usize> {
        let slot = self
            .slots
            .iter()
            .position(Option::is_none)
            .or_else(|| {
                self.slots.iter().position(|s| {
                    s.as_ref().is_some_and(|r| {
                        r.hold != 0 && now.wrapping_sub(r.time) > u32::from(r.hold)
                    })
                })
            })
            .or_else(|| {
                self.slots.iter().position(|s| {
                    s.as_ref()
                        .is_some_and(|r| r.priority <= event.priority && r.manager == manager)
                })
            })?;
        // `gp-0x7dbc` counts the requests within one clock value.
        if self.last_time == Some(now) {
            self.sequence = self.sequence.wrapping_add(1);
        } else {
            self.sequence = 0;
            self.last_time = Some(now);
        }
        self.slots[slot] = Some(QueuedRequest {
            manager,
            priority: event.priority,
            hold: event.hold,
            time: now,
            sequence: self.sequence,
            packet,
        });
        Some(slot)
    }

    /// `FUN_003644c0` for one manager: drops its expired requests, takes
    /// the best (highest priority, then the newest, then the latest within
    /// its VBlank: `FUN_00363d58`) and drops every older one of the same
    /// manager (`FUN_00364350`).
    pub fn service(&mut self, manager: usize, now: u32) -> Option<QueuedRequest> {
        let mut best: Option<(usize, u16, u32, u16)> = None;
        for i in 0..QUEUE_CAPACITY {
            let Some(r) = self.slots[i].as_ref().filter(|r| r.manager == manager) else {
                continue;
            };
            let age = now.wrapping_sub(r.time);
            if r.hold != 0 && age > u32::from(r.hold) {
                self.slots[i] = None;
                continue;
            }
            let better = best.is_none_or(|(_, priority, best_age, sequence)| {
                r.priority > priority
                    || (r.priority == priority
                        && (age < best_age || (age == best_age && r.sequence > sequence)))
            });
            if better {
                best = Some((i, r.priority, age, r.sequence));
            }
        }
        let (slot, ..) = best?;
        let taken = self.slots[slot].take()?;
        for s in &mut self.slots {
            if s.as_ref().is_some_and(|r| {
                r.manager == manager
                    && (r.time < taken.time
                        || (r.time == taken.time && r.sequence < taken.sequence))
            }) {
                *s = None;
            }
        }
        Some(taken)
    }

    pub fn len(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn count(&self, manager: usize) -> usize {
        self.slots
            .iter()
            .flatten()
            .filter(|r| r.manager == manager)
            .count()
    }

    pub fn entry(&self, slot: usize) -> Option<&QueuedRequest> {
        self.slots.get(slot)?.as_ref()
    }
}

/// The speech middleware's shared state: one request queue and one random
/// sequence for the commentary and the players' lines alike.
#[derive(Clone, Debug, Default)]
pub struct SpeechMiddleware {
    pub queue: SpeechQueue,
    pub random: PlayerSelectorRandom,
    /// `DAT_00422b28`: per manager, the last event a line was posted for
    /// and how many times running (`FUN_00365f48`), against a root's repeat
    /// limit (`FUN_00366320`). Cleared with the queue (`FUN_00363018`).
    pub runs: [Option<(u16, u32)>; 2],
}

impl SpeechMiddleware {
    /// `FUN_00362f18`: the generator seeded from the speech manager's
    /// counter (`0x1ac6f0` passes `param_1[1]`, which the constructor
    /// `0x1ac5e0` starts at `rand() % 255`).
    pub fn new(seed: u32) -> Self {
        Self {
            queue: SpeechQueue::default(),
            random: PlayerSelectorRandom::new(seed),
            runs: [None; 2],
        }
    }

    /// `FUN_00363a10`: the request's acceptance draw (the root's chance,
    /// compared as signed bytes), then a queue slot. A request without a
    /// slot is lost after its draw.
    pub fn post(&mut self, event: &SpeechEvent, manager: usize, now: u32, packet: Packet) -> bool {
        let draw = self.random.mode0_index(100, u16::MAX).unwrap_or(0) as u8 as i8;
        if (event.chance as i8) < draw {
            return false;
        }
        self.queue.enqueue(event, manager, now, packet).is_some()
    }

    /// `FUN_00364190`: every queued request dropped, and the managers'
    /// runs with them (`FUN_00363018`).
    pub fn clear(&mut self) {
        self.queue = SpeechQueue::default();
        self.runs = [None; 2];
    }

    /// `FUN_00366320`'s first test: the event has posted `repeat` times
    /// running for this manager, so it is refused before any draw.
    pub fn repeated(&self, event: &SpeechEvent, manager: usize) -> bool {
        let limit = event.repeat as i8;
        limit > 0
            && self
                .runs
                .get(manager)
                .copied()
                .flatten()
                .is_some_and(|(id, count)| id == event.id && i64::from(limit) <= i64::from(count))
    }

    /// `FUN_00365f48` after a line is posted: the manager's run of this
    /// event grows, or a new one starts.
    pub fn posted(&mut self, event: u16, manager: usize) {
        if let Some(run) = self.runs.get_mut(manager) {
            *run = match *run {
                Some((id, count)) if id == event => Some((id, count.wrapping_add(1))),
                _ => Some((event, 1)),
            };
        }
    }
}

/// The voice the game gives a player (`0x1acb68`). A streetball legend has
/// their own (their record's field 7 holds it). Everyone else gets the
/// first voice not yet taken on the court from the six their skin tone
/// allows: created players by their face (field 32), women from tones 5
/// and 6. `taken` holds the picks already made; the answer is (the pick,
/// the voice it plays), as picks 17..20 are voices 13..16 again.
pub fn player_voice(fields: &[i16], taken: &[u16]) -> Option<(u16, u16)> {
    const BY_TONE: [[u16; 6]; 6] = [
        [2, 3, 4, 5, 6, 9],
        [3, 4, 1, 2, 9, 10],
        [5, 6, 7, 8, 10, 1],
        [10, 11, 12, 5, 6, 1],
        [13, 14, 15, 16, 17, 18],
        [15, 16, 13, 14, 19, 20],
    ];
    const CREATED_MEN: [i16; 18] = [4, 4, 2, 4, 4, 4, 1, 3, 1, 3, 4, 3, 4, 2, 2, 1, 4, 3];
    const CREATED_WOMEN: [i16; 6] = [6, 5, 5, 6, 6, 5];
    let field = |i: usize| fields.get(i).copied().unwrap_or(0);
    let tone = if field(3) == 0x25 {
        let face = field(32).max(0) as usize;
        if field(12) > 0 {
            CREATED_WOMEN.get(face).copied().unwrap_or(5)
        } else {
            CREATED_MEN.get(face).copied().unwrap_or(4)
        }
    } else {
        field(7)
    };
    if tone >= 7 {
        return Some((tone as u16, tone as u16));
    }
    let row = if tone < 1 { 3 } else { (tone - 1) as usize };
    // Voices 17..20 are 13..16 again.
    let alias = |v: u16| if v > 16 { v - 4 } else { v };
    BY_TONE[row.min(5)]
        .iter()
        .copied()
        .find(|&v| !taken.contains(&v))
        .map(|v| (v, alias(v)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A header in the disc's layout: `columns` metadata bytes per line,
    /// ring `ring`, partitioned into `partitions` runs when given.
    fn header_bytes(lines: &[(u16, &[u8])], ring: u8, partitions: Option<u8>) -> Vec<u8> {
        let columns = lines.first().map_or(0, |l| l.1.len());
        let mut h = vec![0u8; 12];
        h[0..2].copy_from_slice(&0x2002u16.to_le_bytes());
        h[2..4].copy_from_slice(&1u16.to_le_bytes());
        h[4] = columns as u8 | if partitions.is_some() { 0x80 } else { 0 };
        h[5] = lines.len() as u8;
        h[6] = ring;
        for (start, metadata) in lines {
            h.extend_from_slice(&start.to_be_bytes());
            h.extend_from_slice(metadata);
        }
        h.resize((h.len() + 3) & !3, 0);
        for j in 0..columns {
            let union = lines.iter().fold(0u32, |m, l| m | 1 << l.1[j]);
            h.extend_from_slice(&union.to_le_bytes());
        }
        if let Some(p) = partitions {
            h.push(p);
            h.extend(std::iter::repeat_n(0, lines.len().div_ceil(8)));
        }
        if ring != 0 {
            h.push(0);
            h.extend(std::iter::repeat_n(0xff, usize::from(ring)));
        }
        h
    }

    fn branch(selectors: &[u8], masks: &[u32]) -> EventBranch {
        EventBranch {
            weight: 100,
            chance: 100,
            kind: 0x2002,
            key_word: 1,
            key_type: 1,
            selectors: selectors.to_vec(),
            masks: masks.to_vec(),
            word_masks: Vec::new(),
        }
    }

    fn packet(words: &[u32]) -> Packet {
        let mut p = [0; 20];
        p[..words.len()].copy_from_slice(words);
        p
    }

    #[test]
    fn header_ring_ages_and_wraps_like_fun_00363480_and_00363538() {
        let lines: Vec<(u16, &[u8])> = (0..6).map(|i| (i, &[3u8, 1][..])).collect();
        let mut h = SpeechHeader::parse("action\\001.hdr", &header_bytes(&lines, 3, None), 7)
            .unwrap()
            .unwrap();
        assert_eq!((h.kind, h.id, h.slot, h.ring_len()), (0x2002, 1, 7, 3));
        assert_eq!(h.lines[2], (0x200, vec![3, 1]), "starts scale by 0x100");
        assert_eq!(h.age(0), 0);
        for line in [4, 1, 5] {
            h.used(line);
        }
        assert_eq!((h.age(5), h.age(1), h.age(4), h.age(2)), (1, 2, 3, 0));
        h.used(2);
        assert_eq!(h.recent(), [2, 5, 1], "the oldest entry is overwritten");
        assert_eq!(h.age(4), 0);
        // A ring that is not clean on disc is not registered.
        let mut dirty = header_bytes(&lines, 3, None);
        let last = dirty.len() - 1;
        dirty[last] = 0;
        assert!(SpeechHeader::parse("x", &dirty, 0).unwrap().is_none());
    }

    #[test]
    fn partitions_enable_one_run_and_use_lines_up() {
        let lines: Vec<(u16, &[u8])> = (0..10).map(|i| (i, &[0u8][..])).collect();
        let mut h = SpeechHeader::parse("Block\\BlockA.hdr", &header_bytes(&lines, 0, Some(3)), 0)
            .unwrap()
            .unwrap();
        assert!((0..10).all(|i| !h.enabled(i)), "nothing until partitioned");
        // FUN_003635d0: run k of p covers k*n/p .. (k*n+n)/p.
        for (seed, run) in [(0, 0..3), (1, 3..6), (2, 6..10), (5, 6..10)] {
            h.partition(seed);
            let on: Vec<usize> = (0..10).filter(|&i| h.enabled(i)).collect();
            assert_eq!(on, run.collect::<Vec<_>>(), "seed {seed}");
        }
        h.partition(1);
        h.used(4);
        assert!(!h.enabled(4), "gp-0x7dd0 is 1: a posted line is used up");
        assert_eq!(h.dat(), "Block\\BlockA.dat");
        h.name = "Dunk\\DunkA_Clone_2.hdr".into();
        assert_eq!(h.dat(), "Dunk\\DunkA.dat");
    }

    #[test]
    fn gather_prefers_fresh_lines_then_the_longest_unused() {
        let lines: Vec<(u16, &[u8])> = vec![
            (0, &[3, 0]),
            (1, &[3, 1]),
            (2, &[4, 1]),
            (3, &[3, 1]),
            (4, &[3, 2]),
        ];
        let mut h = SpeechHeader::parse("a", &header_bytes(&lines, 5, None), 0)
            .unwrap()
            .unwrap();
        let b = branch(&[2, 4], &[0x7fff, 7]);
        let request = packet(&[0, 1, 1 << 3, 0, 2]);
        assert_eq!(gather(&h, &b, &request, None), [1, 3]);
        h.used(1);
        assert_eq!(gather(&h, &b, &request, None), [3]);
        h.used(3);
        h.used(0);
        // Both eligible lines are recent: the one used longest ago returns.
        assert_eq!(gather(&h, &b, &request, None), [1]);
        // A request word sharing no bit with its column's word rules the
        // header out before any line (`FUN_00364c38`).
        assert!(gather(&h, &b, &packet(&[0, 1, 1 << 9, 0, 2]), None).is_empty());
        // The descriptor's own mask bounds the values (mood 2 needs bit 2).
        let narrow = branch(&[2, 4], &[0x7fff, 3]);
        assert!(gather(&h, &narrow, &packet(&[0, 1, 1 << 3, 0, 4]), None).is_empty());
        assert_eq!(gather(&h, &b, &packet(&[0, 1, 1 << 3, 0, 4]), None), [4]);
    }

    #[test]
    fn branch_order_takes_every_weighted_draw_like_fun_00365020() {
        let mut event = SpeechEvent {
            id: 4,
            hold: 200,
            priority: 500,
            chance: 100,
            word_predicates: Vec::new(),
            repeat: 0,
            branches: vec![branch(&[], &[]), branch(&[], &[]), branch(&[], &[])],
        };
        event.branches[1].weight = 0;
        let mut random = PlayerSelectorRandom::new(0);
        let mut expected = random.clone();
        let order = event.order(&mut random);
        // Two positive weights: draws over 200 then 100; the weightless
        // branch comes last.
        let first = expected.mode0_index(200, u16::MAX).unwrap();
        expected.mode0_index(100, u16::MAX).unwrap();
        assert_eq!(
            order,
            [
                if first < 100 { 0 } else { 2 },
                if first < 100 { 2 } else { 0 },
                1
            ]
        );
        assert_eq!(random.state, expected.state);
    }

    #[test]
    fn queue_serves_the_newest_request_and_drops_older_ones() {
        let event = SpeechEvent {
            id: PLAYER_EVENT,
            hold: 200,
            priority: 500,
            chance: 100,
            word_predicates: Vec::new(),
            repeat: 0,
            branches: Vec::new(),
        };
        let mut queue = SpeechQueue::default();
        for (i, now) in [10, 10, 11].into_iter().enumerate() {
            queue.enqueue(&event, PLAYERS, now, packet(&[0, i as u32]));
        }
        queue.enqueue(&event, COMMENTARY, 11, packet(&[0, 9]));
        assert_eq!(
            (0..3)
                .map(|i| queue.entry(i).unwrap().sequence)
                .collect::<Vec<_>>(),
            [0, 1, 0]
        );
        let served = queue.service(PLAYERS, 12).unwrap();
        assert_eq!(served.packet[1], 2, "the newest request wins");
        assert_eq!(queue.count(PLAYERS), 0, "older requests are superseded");
        assert_eq!(
            queue.count(COMMENTARY),
            1,
            "the other manager keeps its own"
        );
        // Within one VBlank the later sequence number wins.
        queue.enqueue(&event, PLAYERS, 20, packet(&[0, 5]));
        queue.enqueue(&event, PLAYERS, 20, packet(&[0, 6]));
        assert_eq!(queue.service(PLAYERS, 20).unwrap().packet[1], 6);
        // A request older than its hold is dropped unserved.
        queue.enqueue(&event, PLAYERS, 30, packet(&[0, 7]));
        assert!(queue.service(PLAYERS, 30 + 201).is_none());
        // A full queue takes an expired slot, else a same-manager slot of
        // no higher priority, else loses the request.
        let mut queue = SpeechQueue::default();
        for _ in 0..QUEUE_CAPACITY {
            assert!(queue.enqueue(&event, COMMENTARY, 0, packet(&[])).is_some());
        }
        let urgent = SpeechEvent {
            priority: 499,
            ..event.clone()
        };
        assert_eq!(queue.enqueue(&urgent, PLAYERS, 1, packet(&[])), None);
        assert_eq!(queue.enqueue(&event, COMMENTARY, 1, packet(&[])), Some(0));
        assert_eq!(queue.enqueue(&urgent, PLAYERS, 202, packet(&[])), Some(0));
    }

    #[test]
    fn a_post_takes_its_acceptance_draw_first() {
        let event = SpeechEvent {
            id: 4,
            hold: 200,
            priority: 500,
            chance: 100,
            word_predicates: Vec::new(),
            repeat: 0,
            branches: Vec::new(),
        };
        let mut middleware = SpeechMiddleware::new(0);
        let mut expected = middleware.random.clone();
        expected.mode0_index(100, u16::MAX);
        assert!(middleware.post(&event, COMMENTARY, 0, packet(&[4])));
        assert_eq!(middleware.random.state, expected.state);
        // A chance below the draw loses the request after the draw.
        let never = SpeechEvent { chance: 0, ..event };
        let mut middleware = SpeechMiddleware::new(0);
        assert!(!middleware.post(&never, COMMENTARY, 0, packet(&[4])));
        assert!(middleware.queue.is_empty());
    }

    /// `FUN_00366320`'s first test against `DAT_00422b28` (`FUN_00365f48`
    /// keeps each manager's run of one event): an event with a repeat
    /// limit is refused once it has posted that many times running;
    /// another event between, or a clear (`FUN_00363018`), starts the run
    /// again.
    #[test]
    fn a_roots_repeat_limit_refuses_the_event_after_its_run() {
        let event = SpeechEvent {
            id: 0xe,
            hold: 200,
            priority: 500,
            chance: 100,
            word_predicates: Vec::new(),
            repeat: 1,
            branches: Vec::new(),
        };
        let mut m = SpeechMiddleware::new(0);
        assert!(!m.repeated(&event, COMMENTARY));
        m.posted(0xe, COMMENTARY);
        assert!(m.repeated(&event, COMMENTARY));
        assert!(!m.repeated(&event, PLAYERS), "runs are per manager");
        m.posted(4, COMMENTARY);
        assert!(!m.repeated(&event, COMMENTARY));
        m.posted(0xe, COMMENTARY);
        assert!(m.repeated(&event, COMMENTARY));
        m.clear();
        assert!(!m.repeated(&event, COMMENTARY));
        m.posted(0xe, COMMENTARY);
        m.posted(0xe, COMMENTARY);
        let unlimited = SpeechEvent { repeat: 0, ..event };
        assert!(!m.repeated(&unlimited, COMMENTARY));
    }

    #[test]
    fn middleware_random_generator_matches_native_vectors() {
        let mut random = PlayerSelectorRandom::new(0);
        assert_eq!(
            (0..8).map(|_| random.next()).collect::<Vec<_>>(),
            [
                0x559a_51ed,
                0x274e_a7f5,
                0xe827_f7e0,
                0x76b6_5632,
                0x7f1b_3feb,
                0xeae8_d5ee,
                0x5101_186f,
                0x6575_225a,
            ]
        );

        let mut random = PlayerSelectorRandom::new(0);
        assert_eq!(random.mode0_index(4, u16::MAX), Some(1));
        // The sentinel call consumes one draw and leaves the shared result
        // ring untouched. The next keyed call returns its first fresh draw.
        assert_eq!(random.mode0_index(4, 0), Some(3));
        assert_eq!(random.cursor, 1);
        assert_eq!(random.recent[1], (3, 0));
    }

    #[test]
    fn middleware_random_generator_ripples_state5_overflow() {
        let mut random = PlayerSelectorRandom::new(0);
        random.state = [5, 4, 3, 2, 1, u32::MAX];
        assert_eq!(random.next(), 15);
        // The native short-circuit assignment increments state[4] even
        // when it does not wrap, changing the following generator output.
        assert_eq!(random.state, [15, 10, 6, 3, 1, 0]);
        assert_eq!(random.next(), 35);
    }

    fn disc() -> Option<Iso<std::io::BufReader<std::fs::File>>> {
        Iso::open(std::env::var_os("STREET_ISO")?).ok()
    }

    #[test]
    fn disc_speech_events_are_the_native_subset_with_equal_weights() {
        let Some(mut iso) = disc() else { return };
        let elf = iso.read("/SLUS_206.51").unwrap();
        // DAT_00422ba8, the weight scales, and gp-0x7dd0's data word.
        assert_eq!(
            street_assets::scripts::executable_words(&elf, 0x422ba8, 8).unwrap(),
            WEIGHT_SCALE
        );
        assert_eq!(
            street_assets::scripts::executable_words(&elf, 0x4baff0 - 0x7dd0, 1).unwrap(),
            [u32::from(CONSUME_PARTITIONED_LINES)]
        );
        let players = PlayerSpeech::load(&mut iso).unwrap();
        let event = players.event().unwrap();
        assert_eq!((event.hold, event.priority, event.chance), (200, 500, 100));
        let rules: Vec<_> = event
            .branches
            .iter()
            .map(|b| {
                (
                    b.weight,
                    b.chance,
                    b.kind,
                    b.key_word,
                    b.key_type,
                    b.selectors.clone(),
                    b.masks.clone(),
                )
            })
            .collect();
        assert_eq!(
            rules,
            [
                (100, 100, 0x2002, 1, 1, vec![2, 4], vec![0x7fff, 7]),
                (100, 100, 0x2003, 1, 1, vec![2, 3], vec![0x7fff, 0x3f]),
            ]
        );
        let commentary = Commentary::open(&mut iso).unwrap();
        for id in COMMENTARY_EVENT_IDS {
            let event = commentary.event(id).unwrap();
            // The roots: hold, priority, acceptance chance and repeat limit
            // (`FUN_00363708`'s +2, +4, +9 and +8) and the word predicates'
            // request words. Dunk keys its game-length variants by word 7
            // and Shot by word 4; MissedDunk may not post twice running;
            // ShotClock and the score call take a request three times in
            // four, the slam one in two.
            let root = (
                event.hold,
                event.priority,
                event.chance,
                event.repeat,
                event.word_predicates.clone(),
            );
            let expected = match id {
                1 => (0, 600, 100, 0, vec![7]),
                2 | 3 | 6 => (200, 700, 100, 0, vec![]),
                0xb | 0x11 => (200, 500, 75, 0, vec![]),
                0xc => (200, 599, 100, 0, vec![4]),
                0xe => (200, 500, 100, 1, vec![]),
                0xf => (400, 500, 100, 0, vec![]),
                0x13 => (200, 500, 50, 0, vec![]),
                // Street School's narration outranks every call.
                9 => (200, 800, 100, 0, vec![]),
                _ => (200, 500, 100, 0, vec![]),
            };
            assert_eq!(root, expected, "event {id:#x}");
            // Every branch takes its own draw at 100. The weights are equal
            // but for Dunk (76 and 60 among the keyed headers), Shot (52,
            // 30, 25 and 76) and the winning basket (25 for the hype).
            assert!(
                event.branches.iter().all(|b| b.chance == 100),
                "event {id:#x}"
            );
            let weights: std::collections::BTreeSet<u32> =
                event.branches.iter().map(|b| b.weight).collect();
            let expected: &[u32] = match id {
                1 => &[60, 76, 100],
                0xc => &[25, 30, 52, 76, 100],
                0xf => &[25, 100],
                _ => &[100],
            };
            assert_eq!(
                weights.into_iter().collect::<Vec<_>>(),
                expected,
                "event {id:#x}"
            );
        }
        let kinds = |id| -> Vec<(u16, u8)> {
            commentary
                .event(id)
                .unwrap()
                .branches
                .iter()
                .map(|b| (b.kind, b.key_type))
                .collect()
        };
        assert_eq!(kinds(4), [(20, 0), (21, 0), (57, 0)]);
        assert_eq!(kinds(0), [(1, 0), (0, 0), (66, 1)]);
        assert_eq!(kinds(8), [(35, 0), (36, 0), (66, 1)]);
        assert_eq!(kinds(0x12), [(159, 1), (213, 1)]);
        assert_eq!(kinds(0xd), [(0xa3, 0), (0xa4, 0)]);
        assert_eq!(kinds(0x11), [(0xa2, 0)]);
    }

    /// `FUN_001a7800`, `FUN_001a7948` and `FUN_001a7bd0` on the roster's
    /// persistent ids (`XDBPLYR` field 0), and `FUN_001a56b0`'s count.
    #[test]
    fn the_dunkers_identity_words_follow_the_three_tables() {
        // The Street Legends: records 145..150, whose ids skip 150.
        assert_eq!(
            [145, 146, 147, 148, 149, 150, 151].map(street_legend_word),
            [0x10, 2, 1, 4, 0x20, 0, 8]
        );
        assert_eq!(street_legend_word(0), 0);
        // The Legends, 24 bits in all; Walton (232) and Jordan's other
        // records (140, 401) have none.
        let legends = [
            (169, 1),
            (157, 2),
            (160, 4),
            (163, 8),
            (154, 0x10),
            (166, 0x20),
            (155, 0x80),
            (158, 0x100),
            (230, 0x200),
            (164, 0x400),
            (153, 0x800),
            (170, 0x1000),
            (172, 0x2000),
            (171, 0x4000),
            (229, 0x8000),
            (161, 0x10000),
            (165, 0x20000),
            (159, 0x40000),
            (162, 0x80000),
            (156, 0x100000),
            (231, 0x200000),
            (168, 0x400000),
            (167, 0x800000),
            (152, 0x1000000),
        ];
        for (id, bit) in legends {
            assert_eq!(league_legend_word(id), bit, "legend {id}");
        }
        assert_eq!(legends.iter().fold(0, |m, (_, b)| m | b), 0x1ffffff & !0x40);
        assert_eq!([140, 232, 401].map(league_legend_word), [0; 3]);
        // The league stars, 25 bits.
        let stars = [
            (5, 1),
            (501, 2),
            (51, 4),
            (512, 8),
            (20, 0x10),
            (21, 0x20),
            (22, 0x40),
            (32, 0x80),
            (40, 0x100),
            (16, 0x200),
            (50, 0x400),
            (55, 0x800),
            (56, 0x1000),
            (60, 0x2000),
            (75, 0x4000),
            (80, 0x8000),
            (82, 0x10000),
            (86, 0x20000),
            (95, 0x40000),
            (100, 0x80000),
            (105, 0x100000),
            (115, 0x200000),
            (116, 0x400000),
            (130, 0x800000),
            (30, 0x1000000),
        ];
        for (id, bit) in stars {
            assert_eq!(league_star_word(id), bit, "star {id}");
        }
        assert_eq!(stars.iter().fold(0, |m, (_, b)| m | b), 0x1ffffff);
        assert_eq!([0, 6, 140, 145].map(league_star_word), [0; 4]);
        // The words, and the flag that blanks the last two.
        assert_eq!(dunker_words(56, false), [0, 0, 0x1000]);
        assert_eq!(dunker_words(56, true), [0, 0, 0]);
        assert_eq!(dunker_words(145, true), [0x10, 0, 0]);
        assert_eq!(dunker_words(169, false), [0, 1, 0]);
        // Fewer than two named players switch the names off; Street
        // Legends do not count.
        assert!(names_off([0, 1, 2, 3, 4, 56]));
        assert!(names_off([145, 146, 147, 148, 149, 151]));
        assert!(!names_off([56, 0, 0, 0, 0, 169]));
        assert!(!names_off([5, 16]));
        assert!(names_off([]));
    }

    #[test]
    fn disc_headers_share_one_table_players_first() {
        let Some(mut iso) = disc() else { return };
        let players = PlayerSpeech::load(&mut iso).unwrap();
        let commentary = Commentary::open(&mut iso).unwrap();
        assert_eq!(players.headers.headers.len(), 44);
        assert_eq!(
            players
                .headers
                .headers
                .iter()
                .map(|h| h.slot)
                .collect::<Vec<_>>(),
            (0..44).collect::<Vec<_>>()
        );
        let first = &commentary.headers.headers[0];
        assert_eq!(
            (first.name.as_str(), first.slot),
            ("BobbitoCrowHype.hdr", 44)
        );
        let slot = |name: &str| {
            commentary
                .headers
                .headers
                .iter()
                .find(|h| h.name == name)
                .map(|h| (h.slot, h.ring_len(), h.lines.len(), h.partitions()))
        };
        assert_eq!(slot("Block\\BlockA.hdr"), Some((49, 14, 73, Some(4))));
        assert_eq!(slot("Block\\BlockB.hdr"), Some((50, 16, 68, Some(3))));
        assert_eq!(slot("Move\\MoveA.hdr").map(|s| s.0), Some(95));
        assert_eq!(slot("NicknameAction\\05.hdr"), Some((101, 0, 2, None)));
        // Player headers are not partitioned; every line is enabled.
        assert!(
            players
                .headers
                .headers
                .iter()
                .all(|h| h.partitions().is_none())
        );
    }

    #[test]
    fn disc_move_choice_matches_the_native_draw_sequence() {
        let Some(mut iso) = disc() else { return };
        let mut commentary = Commentary::open(&mut iso).unwrap();
        commentary.partition(0);
        let request = move_packet(false, Some(false));
        let mut random = PlayerSelectorRandom::new(0);
        let mut expected = random.clone();
        let selected = commentary.choose(&request, &mut random).unwrap();
        // FUN_00365020's three order draws (300, 200, 100), the winner's
        // chance draw and its keyed candidate draw.
        let first = expected.mode0_index(300, u16::MAX).unwrap() / 100;
        expected.mode0_index(200, u16::MAX).unwrap();
        expected.mode0_index(100, u16::MAX).unwrap();
        expected.mode0_index(100, u16::MAX).unwrap();
        let event = commentary.event(4).unwrap();
        let header = commentary
            .headers
            .by_kind(event.branches[first].kind)
            .unwrap();
        let h = &commentary.headers.headers[header];
        let candidates = gather(h, &event.branches[first], &request, None);
        let pick = expected.mode0_index(candidates.len(), h.slot).unwrap();
        assert_eq!(selected.choice.header, header);
        assert_eq!(selected.choice.line, candidates[pick]);
        assert_eq!(selected.offset, h.lines[candidates[pick]].0);
        assert_eq!(random.state, expected.state);
        assert_eq!(random.recent, expected.recent);
        // Every enabled candidate is in the session's run and passes the
        // sex predicates; the line exists in its .dat.
        let entry = commentary
            .entries
            .iter()
            .find(|e| e.name == selected.name)
            .unwrap();
        let lines = commentary.lines(&mut iso, entry).unwrap();
        assert!(
            lines
                .streams
                .iter()
                .any(|s| s.start as u32 == selected.offset)
        );
        for &line in &candidates {
            assert!(h.enabled(line));
            let metadata = &h.lines[line].1;
            assert_eq!(metadata[0], 0);
            assert_ne!(5 & (1 << metadata[1]), 0);
        }
        commentary.commit(&selected);
        let h = &commentary.headers.headers[header];
        assert_eq!(h.recent(), [selected.choice.line]);
        assert!(!h.enabled(selected.choice.line), "used up for this session");
    }

    #[test]
    fn disc_block_keys_the_nickname_branch_by_the_actors_row() {
        let Some(mut iso) = disc() else { return };
        let mut commentary = Commentary::open(&mut iso).unwrap();
        commentary.partition(0);
        let event = commentary.event(0).unwrap().clone();
        // Row 5's NicknameAction header holds two lines; the descriptor's
        // mask 2 admits metadata value 1 only.
        let h = commentary.headers.by_id(66, 5).unwrap();
        assert_eq!(
            gather(
                &commentary.headers.headers[h],
                &event.branches[2],
                &block_packet(5, Some(false), false),
                None,
            )
            .len(),
            1
        );
        assert!(
            commentary.headers.by_id(66, 0).is_none(),
            "no row 0 header: an ordinary blocker never gets one"
        );
        // Over many requests a block names Block files, or the actor's row.
        let mut random = PlayerSelectorRandom::new(3);
        let mut names = std::collections::BTreeSet::new();
        for _ in 0..40 {
            if let Some(selected) =
                commentary.choose(&block_packet(5, Some(false), false), &mut random)
            {
                names.insert(selected.name.clone());
                commentary.commit(&selected);
            }
        }
        assert!(names.contains("NicknameAction\\05.dat"));
        assert!(
            names
                .iter()
                .all(|n| n.starts_with("Block\\") || n == "NicknameAction\\05.dat")
        );
    }

    #[test]
    fn disc_nickname_event_uses_alias_id_and_sex() {
        let Some(mut iso) = disc() else { return };
        let names = street_assets::db::nicknames(&mut iso).unwrap();
        let mut c = Commentary::open(&mut iso).unwrap();
        c.partition(0);
        assert_eq!(
            c.headers.headers.iter().filter(|h| h.kind == 159).count(),
            46
        );
        assert_eq!(
            c.headers
                .headers
                .iter()
                .filter(|h| h.kind == 213)
                .map(|h| h.id)
                .collect::<Vec<_>>(),
            [1, 2, 4]
        );
        let mut random = PlayerSelectorRandom::new(0);
        for name in &names {
            for female in [false, true] {
                let selected = c.choose(&nickname_packet(name.id as u16, female), &mut random);
                if female && [1, 2, 4].contains(&name.id) {
                    assert!(selected.is_none(), "male-only native nickname header");
                    continue;
                }
                let selected = selected.unwrap();
                assert_eq!(selected.offset, 0);
                let entry = c.entries.iter().find(|e| e.name == selected.name).unwrap();
                let lines = c.lines(&mut iso, entry).unwrap();
                assert_eq!(lines.streams.len(), 1);
                let clip = lines.clip(0).unwrap();
                assert!(clip.frames() > 0 && clip.samples.iter().any(|s| *s != 0));
            }
        }
        assert!(
            c.choose(&nickname_packet(0xffff, false), &mut random)
                .is_none()
        );
    }

    #[test]
    fn disc_player_event_selects_action_and_launch_lines() {
        let Some(mut iso) = disc() else { return };
        let mut speech = PlayerSpeech::load(&mut iso).unwrap();
        let mut random = PlayerSelectorRandom::new(0);
        let voices: Vec<u16> = speech.voices.keys().copied().collect();
        for voice in voices {
            let trick = PlayerRequest {
                voice,
                category_mask: 1 << 3,
                shot_mask: 0,
                action_mask: 2,
            };
            let selected = speech.select_request(trick, &mut random).unwrap();
            assert_eq!(selected.catalog, PlayerCatalog::Action);
            let line = speech.voices[&voice][selected.ordinal];
            assert_eq!(
                (line.category, line.mood, line.stream),
                (3, 1, Some(selected.stream))
            );
            assert_eq!(Some(selected.history_key), speech.history_key(voice));
            speech.commit(&selected);
            let launch = PlayerRequest {
                voice,
                category_mask: 1 << 14,
                shot_mask: 1,
                action_mask: 2,
            };
            let selected = speech.select_request(launch, &mut random).unwrap();
            assert_eq!(selected.catalog, PlayerCatalog::Shot);
            let line = speech.shot_voices[&voice][selected.ordinal];
            assert_eq!(
                (line.category, line.mood, line.stream),
                (14, 0, Some(selected.stream))
            );
            speech.commit(&selected);
            let action = speech
                .headers
                .by_slot(speech.history_key(voice).unwrap())
                .unwrap();
            let shot = speech
                .headers
                .by_slot(speech.shot_history_key(voice).unwrap())
                .unwrap();
            assert_eq!((action.recent().len(), shot.recent().len()), (1, 1));
        }
    }

    #[test]
    fn selector_random_seeds_from_the_executable_increments() {
        // STREET_ISO: FUN_00366f40 seeds slot × 0x10001 plus six successive
        // increments, each built by a lui/ori pair (0x366f78..0x366fd0),
        // and clears the 32-pair ring to 0xffff with the cursor at 0.
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let elf = street_assets::iso::Iso::open(path)
            .unwrap()
            .read("/SLUS_206.51")
            .unwrap();
        let w = |a: u32| street_assets::scripts::executable_words(&elf, a, 1).unwrap()[0];
        let pair = |hi: u32, lo: u32| {
            let (hi, lo) = (w(hi), w(lo));
            assert_eq!((hi >> 26, lo >> 26), (0x0f, 0x0d));
            (hi & 0xffff) << 16 | (lo & 0xffff)
        };
        let increments = [
            pair(0x366f78, 0x366f80),
            pair(0x366f88, 0x366f90),
            pair(0x366f9c, 0x366fa4),
            pair(0x366fa8, 0x366fb0),
            pair(0x366fb8, 0x366fc0),
            pair(0x366fc8, 0x366fd0),
        ];
        // sll v1, a0, 16; addu a0, a0, v1.
        assert_eq!((w(0x366f74), w(0x366f7c)), (0x0004_1c00, 0x0083_2021));
        for slot in [0u32, 1, 2, 7, 0xffff] {
            let mut value = slot.wrapping_add(slot << 16);
            let state = increments.map(|increment| {
                value = value.wrapping_add(increment);
                value
            });
            let random = PlayerSelectorRandom::new(slot);
            assert_eq!(random.state, state, "slot {slot}");
            assert_eq!(random.cursor, 0);
            assert!(random.recent.iter().all(|&r| r == (u16::MAX, u16::MAX)));
        }
    }

    #[test]
    fn disc_every_commentary_line_starts_a_stream_in_its_dat() {
        // Each header's line starts, byte 7's shift taken, are the starts of
        // the streams in its `.dat` (1 in `TutLessonsB`, `DunkI`, `DunkJ`,
        // `DunkK`, `DunkN` and `SpecIntrosC`, whose lines pass 0x100 x
        // 0xffff bytes).
        let Some(mut iso) = disc() else { return };
        let commentary = Commentary::open(&mut iso).unwrap();
        let mut checked = 0;
        for header in &commentary.headers.headers {
            // ScoreUpdate's lines are keyed by the margin (22 sub-keys) and
            // its starts past the fifth fall in padding: its layout is not
            // read yet.
            if header.sub_keys != 0 && header.sub_keys != 0xffff {
                continue;
            }
            let Some(entry) = commentary
                .entries
                .iter()
                .find(|e| e.name.eq_ignore_ascii_case(&header.dat()))
                .cloned()
            else {
                continue;
            };
            let lines = commentary.lines(&mut iso, &entry).unwrap();
            for &(start, _) in &header.lines {
                assert!(
                    lines.streams.iter().any(|s| s.start == start as usize),
                    "{}: no stream at {start}",
                    header.name
                );
                checked += 1;
            }
        }
        assert!(checked > 1000, "{checked}");
    }

    #[test]
    fn disc_event_9_narrates_each_lesson_and_page() {
        // `0x1ad510`'s table against event 9's headers: every lesson
        // 0..25 has an introduction and a passed and a failed line, the
        // pages 26, 28..30 a reading (situation 8) and nothing else; the
        // Player Indicator's page (27, group 0) none.
        let Some(mut iso) = disc() else { return };
        let commentary = Commentary::open(&mut iso).unwrap();
        let says = |i: usize, kind: Lesson| {
            let mut random = PlayerSelectorRandom::new(1);
            commentary
                .choose(&lesson_packet(i, kind), &mut random)
                .is_some_and(|c| c.name.starts_with(r"Tutorial\TutLessons"))
        };
        for i in 0..=25 {
            for kind in [
                Lesson::Intro,
                Lesson::Result { passed: true },
                Lesson::Result { passed: false },
            ] {
                assert!(says(i, kind), "lesson {i} {kind:?}");
            }
        }
        for page in [26, 28, 29, 30] {
            assert!(says(page, Lesson::Page), "page {page}");
        }
        assert!(!says(27, Lesson::Page));
        assert!(!says(26, Lesson::Intro));
        assert!(!says(3, Lesson::Page));
    }

    #[test]
    fn disc_event_0x13_is_the_passout_call() {
        // Slot 39 (`0x1a5d50`) posts event 0x13 when a passout off a
        // defender's head goes on to its second leg (`FUN_00148378`): its
        // one branch is the commentator's PassOut header, not a dunk's.
        let Some(mut iso) = disc() else { return };
        let commentary = Commentary::open(&mut iso).unwrap();
        let event = commentary.event(0x13).unwrap();
        let names: Vec<_> = event
            .branches
            .iter()
            .flat_map(|b| {
                commentary
                    .headers
                    .headers
                    .iter()
                    .filter(move |h| h.kind == b.kind)
                    .map(|h| h.name.as_str())
            })
            .collect();
        assert_eq!(names, ["PassOut.hdr"]);
    }
}
