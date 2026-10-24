//! PathFinder maps (`/MUSIC/<SONG>/<SONG>.MPF`, `PFDx` version 3.4): the
//! graph that strings an in-game song's segments together and answers the
//! game's music events (the player is [`Player`]).
//!
//! A song has three tracks, loaded by the game's `AuPathManager` (`0x1aa4b0`):
//! 0 `MAIN.MUS` (SCHl segments), 1 `WHOOSH.MUS` and 2 `TRICKS.MUS` (BNKl
//! banks of one-shots).
//!
//! The file (all little-endian):
//!
//! - 0x00 `PFDx`, 0x04 version 3, 4; 0x0d tracks, 0x0e sections, 0x0f
//!   events, 0x10 routers, 0x11 variables, 0x12 u16 nodes; 0x24 a u16 per
//!   node, its offset in words.
//! - The nodes. Each is two words, a third, then its branches:
//!   - u16 segment (1-based over every track's segments; 0 a control node,
//!     0xffff a router), u8 track, u8 flags (high nibble a loop count, 0xf
//!     for ever; bit 3 a transition);
//!   - u8 transition list, u8 branch count (low 5 bits; the top 3 bits the
//!     choice: 1 random, else by the game's value), u8 beat layout (high
//!     nibble times low nibble beats), u8;
//!   - a word the walker does not read here;
//!   - branches of a word: i8 low, i8 high (inclusive), u16 next node. The
//!     first range holding the value is taken; no match ends the track.
//! - The event table: a router index per track, event and section.
//! - The routers, a word each: 0xff, flags (0x40: at the next beat, else
//!   at the end of the segment; bits 0..3 nonzero: a track to start), u16
//!   node (0xffff none: with 0x40, the track stops).
//! - The transition lists: word offsets of each list (one more than the
//!   lists), then words of `from << 16 | to`. While a node with list `k`
//!   plays, an event whose target is `from` plays `to` first: a short
//!   transition, after which the target plays (`0x342dd8`).
//! - A word offset per track (and one past the last) into the segment
//!   table, then the segments: u32 offset (in words, into `MAIN.MUS`; a
//!   bank's sound index for the banks), u32 length in milliseconds.
//!
//! What the walker does (`0x342be8`): from a node, while it has no segment,
//! take the branch for the value (random for random nodes, the loop count
//! for routers with one) and go on; a node with a segment plays it.

use street_assets::{Error, Result};

/// A segment to play: where it is and how long it lasts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    pub track: u8,
    /// For `MAIN.MUS`, the byte offset of its SCHl stream; for a bank, the
    /// sound's index.
    pub at: u32,
    pub millis: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Branch {
    pub low: i8,
    pub high: i8,
    pub next: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    /// The segment, 1-based; 0 for a control node, `0xffff` for a router.
    pub segment: u16,
    pub track: u8,
    /// The loop count of a router (`-1` for ever, 0 none).
    pub repeat: i8,
    /// A transition's own control node.
    pub transition: bool,
    /// The transition list in force while this node plays (0 none).
    pub transitions: u8,
    /// The choice among the branches is random.
    pub random: bool,
    /// Beats in the segment.
    pub beats: u8,
    pub branches: Vec<Branch>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Router {
    pub flags: u8,
    pub node: Option<u16>,
}

impl Router {
    /// Takes effect on the next beat rather than at the end of the segment.
    pub fn on_beat(&self) -> bool {
        self.flags & 0x40 != 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Map {
    pub tracks: u8,
    pub sections: u8,
    pub events: u8,
    pub nodes: Vec<Node>,
    /// Router index per `(track * events + event) * sections + section`.
    pub table: Vec<u8>,
    pub routers: Vec<Router>,
    /// Transition lists, list `k` at index `k - 1`: `(from, to)` nodes.
    pub transitions: Vec<Vec<(u16, u16)>>,
    /// Segments `1..`, at index `id - 1`.
    pub segments: Vec<Segment>,
}

/// The game's music events (`AuPathManager`, `0x1aa4b0`..`0x1abf70`).
pub mod event {
    /// The song's intro, looping (a match starting, `0x1aa8d8`).
    pub const INTRO: u8 = 0;
    /// The main groove, after what plays now (tip-off, `0x1a6f38`; after a
    /// gamebreaker, `0x1ab5f8`).
    pub const MAIN: u8 = 1;
    /// The main groove at once (a meter emptied, `0x1abbc0`).
    pub const MAIN_NOW: u8 = 2;
    /// A gamebreaker ready: level 1, level 2 (`0x1ab2f8`).
    pub const READY: [u8; 2] = [3, 6];
    /// The ready themes' endings, after the gamebreaker (`0x1ab5f8`).
    pub const READY_END: [u8; 2] = [5, 8];
    /// A gamebreaker going up: level 1, level 2 (`0x1ab3b8`).
    pub const GAMEBREAKER: [u8; 2] = [9, 10];
    /// A break in play (`0x1a5410`; `MAIN_NOW` ends it, `0x1a54d8`).
    pub const BREAK: u8 = 11;
    /// A trick stinger, for the top trick levels (`0x1a61b8`: event 10 +
    /// the level, levels 3 and 4 of 0..4, at most every two seconds).
    pub const STINGER: u8 = 10;
    /// The game is over (`0x1a5638`).
    pub const END: u8 = 15;
}

fn u16_at(d: &[u8], at: usize) -> Result<u16> {
    d.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| Error::Format(format!("PFDx: u16 at {at:#x} past the end")))
}

fn u32_at(d: &[u8], at: usize) -> Result<u32> {
    d.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| Error::Format(format!("PFDx: u32 at {at:#x} past the end")))
}

impl Map {
    pub fn parse(d: &[u8]) -> Result<Self> {
        if d.get(..4) != Some(b"xDFP") || d.get(4..6) != Some(&[3, 4]) {
            return Err(Error::Format("not a PathFinder 3.4 map (PFDx)".into()));
        }
        let (tracks, sections, events, routers) = (d[0x0d], d[0x0e], d[0x0f], d[0x10]);
        let count = usize::from(u16_at(d, 0x12)?);
        let offsets: Vec<usize> = (0..count)
            .map(|k| u16_at(d, 0x24 + 2 * k).map(|w| usize::from(w) * 4))
            .collect::<Result<_>>()?;
        let mut nodes = Vec::with_capacity(count);
        let mut end = 0;
        for &at in &offsets {
            let w0 = u32_at(d, at)?;
            let w1 = u32_at(d, at + 4)?;
            let n = (w1 >> 8 & 0x1f) as usize;
            let branches = (0..n)
                .map(|k| {
                    let w = u32_at(d, at + 12 + 4 * k)?;
                    Ok(Branch {
                        low: w as u8 as i8,
                        high: (w >> 8) as u8 as i8,
                        next: (w >> 16) as u16,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            end = end.max(at + 12 + 4 * n);
            nodes.push(Node {
                segment: w0 as u16,
                track: (w0 >> 16) as u8,
                repeat: ((w0 >> 24) as u8 as i8) >> 4,
                transition: w0 >> 27 & 1 != 0,
                transitions: w1 as u8,
                random: w1 >> 13 & 7 == 1,
                beats: ((w1 >> 20 & 0xf) * (w1 >> 16 & 0xf)) as u8,
                branches,
            });
        }
        // The event table follows the nodes, then the routers on a word.
        let size = usize::from(tracks) * usize::from(events) * usize::from(sections);
        let table = d
            .get(end..end + size)
            .ok_or_else(|| Error::Format("PFDx: event table past the end".into()))?
            .to_vec();
        let mut at = (end + size).next_multiple_of(4);
        let routers = (0..routers)
            .map(|k| {
                let w = u32_at(d, at + 4 * usize::from(k))?;
                let node = (w >> 16) as u16;
                Ok(Router {
                    flags: (w >> 8) as u8,
                    node: (node != 0xffff).then_some(node),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        at += 4 * routers.len();
        // The transition lists: offsets up to the first list's start.
        let first = u32_at(d, at)? as usize * 4;
        let marks: Vec<usize> = (0..(first.saturating_sub(at)) / 4)
            .map(|k| u32_at(d, at + 4 * k).map(|w| w as usize * 4))
            .collect::<Result<_>>()?;
        let transitions = marks
            .windows(2)
            .map(|w| {
                (w[0]..w[1])
                    .step_by(4)
                    .map(|p| u32_at(d, p).map(|v| ((v >> 16) as u16, v as u16)))
                    .collect::<Result<Vec<_>>>()
            })
            .collect::<Result<Vec<_>>>()?;
        // The segment table's per-track offsets, then the segments.
        let start = *marks
            .last()
            .ok_or_else(|| Error::Format("PFDx: no transition table".into()))?;
        let per_track: Vec<usize> = (0..=usize::from(tracks))
            .map(|k| u32_at(d, start + 4 * k).map(|w| w as usize * 4))
            .collect::<Result<_>>()?;
        let mut segments = Vec::new();
        for (track, w) in per_track.windows(2).enumerate() {
            for p in (w[0]..w[1]).step_by(8) {
                let (at, millis) = (u32_at(d, p)?, u32_at(d, p + 4)?);
                segments.push(Segment {
                    track: track as u8,
                    at: if track == 0 { at * 4 } else { at },
                    millis,
                });
            }
        }
        Ok(Map {
            tracks,
            sections,
            events,
            nodes,
            table,
            routers,
            transitions,
            segments,
        })
    }

    /// The router for `event` on `track` (section 0).
    pub fn router(&self, track: u8, event: u8) -> Option<Router> {
        let at = (usize::from(track) * usize::from(self.events) + usize::from(event))
            * usize::from(self.sections);
        self.table
            .get(at)
            .and_then(|&r| self.routers.get(usize::from(r)))
            .copied()
    }

    /// The segment a node plays, if any.
    pub fn segment(&self, node: u16) -> Option<Segment> {
        let n = self.nodes.get(usize::from(node))?;
        match n.segment {
            0 | 0xffff => None,
            s => self.segments.get(usize::from(s) - 1).copied(),
        }
    }

    /// `target` as the event meets it while `playing` plays: a transition
    /// node if the playing node's list has one for it.
    pub fn transition(&self, playing: Option<u16>, target: u16) -> Option<u16> {
        let list = self.nodes.get(usize::from(playing?))?.transitions;
        let list = self.transitions.get(usize::from(list).checked_sub(1)?)?;
        list.iter()
            .find(|&&(from, _)| from == target)
            .map(|&(_, to)| to)
    }
}

/// One track's walker state.
#[derive(Clone, Debug, Default)]
struct Walk {
    /// Routers with a loop count met so far and what they have left.
    counts: Vec<(u16, i8)>,
}

impl Map {
    /// From `node`, the first node that plays a segment (`0x342be8`): `None`
    /// when the path ends. `value` is the game's (0..127); `roll` gives a
    /// random 0..127 for random nodes.
    fn walk(
        &self,
        mut node: u16,
        value: u8,
        walk: &mut Walk,
        roll: &mut dyn FnMut() -> u8,
    ) -> Option<u16> {
        // A path that cycles through control nodes alone would never play.
        for _ in 0..self.nodes.len() {
            let n = self.nodes.get(usize::from(node))?;
            if !matches!(n.segment, 0 | 0xffff) {
                return Some(node);
            }
            let mut v = if n.random { roll() } else { value } as i32;
            if n.segment == 0xffff && n.repeat != 0 {
                // A loop counter: for ever (-1) always loops; a count runs
                // down to 0, which takes the way out.
                let at = walk.counts.iter().position(|&(k, _)| k == node);
                let left = match at {
                    Some(i) => walk.counts[i].1,
                    None => {
                        walk.counts.push((node, n.repeat));
                        n.repeat
                    }
                };
                v = if left < 0 { 0x7f } else { i32::from(left) };
                if left > 0 {
                    let i = walk.counts.iter().position(|&(k, _)| k == node)?;
                    walk.counts[i].1 = left - 1;
                } else if left == 0 {
                    walk.counts.retain(|&(k, _)| k != node);
                }
            }
            node = n
                .branches
                .iter()
                .find(|b| i32::from(b.low) <= v && v <= i32::from(b.high))?
                .next;
        }
        None
    }
}

/// What a track does next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    /// Play this node's segment.
    Play(u16),
    /// Nothing more on this track.
    Stop,
}

/// When a pending change takes over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    Beat,
    SegmentEnd,
}

#[derive(Clone, Debug, Default)]
struct TrackState {
    playing: Option<u16>,
    /// An event's target, waiting for its moment.
    pending: Option<(Option<u16>, When)>,
    /// After a transition's path ends, where to go.
    after: Option<u16>,
    walk: Walk,
}

/// Plays a map: events in, the next node per track out. It doesn't touch
/// audio; whatever plays the segments asks [`Player::next`] at each
/// segment's end and [`Player::due`] on each beat.
#[derive(Clone, Debug)]
pub struct Player {
    pub map: Map,
    /// The game's value (0..127): how far the leading side is to winning
    /// (`0x1a3b10`: score × 127 / the points to win).
    pub value: u8,
    tracks: Vec<TrackState>,
    seed: u32,
}

impl Player {
    pub fn new(map: Map, seed: u32) -> Self {
        let tracks = vec![TrackState::default(); usize::from(map.tracks)];
        Player {
            map,
            value: 0,
            tracks,
            seed: seed | 1,
        }
    }

    fn roll(seed: &mut u32) -> u8 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 17;
        *seed ^= *seed << 5;
        (*seed >> 8) as u8 & 0x7f
    }

    /// The node playing on `track`.
    pub fn playing(&self, track: u8) -> Option<u16> {
        self.tracks.get(usize::from(track))?.playing
    }

    /// Every track stops and forgets what was waiting (the game's
    /// `Path_stop` and queue clear before a fresh start, `0x1ab5f8`).
    pub fn stop_all(&mut self) {
        for t in &mut self.tracks {
            *t = TrackState::default();
        }
    }

    /// The game fires `event`: each track's router decides what's next.
    /// Returns the tracks that should cut over now (they were idle).
    pub fn event(&mut self, event: u8) -> Vec<u8> {
        let mut now = Vec::new();
        for t in 0..self.map.tracks {
            let Some(r) = self.map.router(t, event) else {
                continue;
            };
            let when = if r.on_beat() {
                When::Beat
            } else {
                When::SegmentEnd
            };
            let state = &mut self.tracks[usize::from(t)];
            match r.node {
                None if r.on_beat() => state.pending = Some((None, When::Beat)),
                None => {}
                Some(node) => {
                    state.pending = Some((Some(node), when));
                    if state.playing.is_none() {
                        now.push(t);
                    }
                }
            }
        }
        now
    }

    /// Whether `track` has a change waiting for the next beat.
    pub fn due(&self, track: u8) -> bool {
        matches!(
            self.tracks.get(usize::from(track)).and_then(|s| s.pending),
            Some((_, When::Beat))
        )
    }

    /// The segment on `track` ended (or a beat came with a change due):
    /// what plays next.
    pub fn next(&mut self, track: u8) -> Cue {
        let t = usize::from(track);
        let Some(state) = self.tracks.get_mut(t) else {
            return Cue::Stop;
        };
        let playing = state.playing;
        let mut seed = self.seed;
        let mut roll = || Self::roll(&mut seed);
        let target = if let Some((target, _)) = state.pending.take() {
            match target {
                None => {
                    state.after = None;
                    None
                }
                Some(target) => match self.map.transition(playing, target) {
                    Some(via) => {
                        state.after = Some(target);
                        self.map.walk(via, self.value, &mut state.walk, &mut roll)
                    }
                    None => {
                        state.after = None;
                        self.map
                            .walk(target, self.value, &mut state.walk, &mut roll)
                    }
                },
            }
        } else {
            // The path goes on from the node that played.
            let from = playing.and_then(|p| self.map.nodes.get(usize::from(p)));
            let next = from.and_then(|n| {
                let v = i32::from(self.value);
                n.branches
                    .iter()
                    .find(|b| i32::from(b.low) <= v && v <= i32::from(b.high))
                    .map(|b| b.next)
            });
            match next.and_then(|n| self.map.walk(n, self.value, &mut state.walk, &mut roll)) {
                Some(n) => Some(n),
                // The path ended: a transition hands over to its target.
                None => state
                    .after
                    .take()
                    .and_then(|a| self.map.walk(a, self.value, &mut state.walk, &mut roll)),
            }
        };
        self.seed = seed;
        let state = &mut self.tracks[t];
        state.playing = target;
        match target {
            Some(n) => Cue::Play(n),
            None => Cue::Stop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(segment: u16, branches: &[(i8, i8, u16)]) -> Node {
        Node {
            segment,
            track: 0,
            repeat: 0,
            transition: false,
            transitions: 0,
            random: false,
            beats: 4,
            branches: branches
                .iter()
                .map(|&(low, high, next)| Branch { low, high, next })
                .collect(),
        }
    }

    /// Intro (1) looping until the main event, which goes through a
    /// transition (2) into a two-segment loop (3, 4).
    fn map() -> Map {
        let mut intro = node(1, &[(0, 127, 1)]);
        intro.transitions = 1;
        let mut router = node(0xffff, &[(0, 0, 0xffff), (1, 127, 0)]);
        router.repeat = -1;
        Map {
            tracks: 1,
            sections: 1,
            events: 2,
            nodes: vec![
                node(0, &[(0, 127, 2)]), // 0: intro entry
                router,                  // 1: loops the intro for ever
                intro,                   // 2: the intro
                node(0, &[(0, 127, 4)]), // 3: main entry
                node(3, &[(0, 127, 5)]), // 4
                node(4, &[(0, 127, 4)]), // 5
                node(0, &[(0, 127, 7)]), // 6: transition entry
                node(2, &[]),            // 7: the transition, then the end
            ],
            table: vec![0, 1],
            routers: vec![
                Router {
                    flags: 0x41,
                    node: Some(0),
                },
                Router {
                    flags: 0x01,
                    node: Some(3),
                },
            ],
            transitions: vec![vec![(3, 6)]],
            segments: (1..=4)
                .map(|k| Segment {
                    track: 0,
                    at: k,
                    millis: 1000,
                })
                .collect(),
        }
    }

    #[test]
    fn intro_loops_then_the_transition_leads_into_main() {
        let mut p = Player::new(map(), 7);
        assert_eq!(p.event(0), vec![0]);
        let seg = |p: &Player, c: Cue| match c {
            Cue::Play(n) => p.map.nodes[usize::from(n)].segment,
            Cue::Stop => 0,
        };
        let c = p.next(0);
        assert_eq!(seg(&p, c), 1);
        let c = p.next(0);
        assert_eq!(seg(&p, c), 1, "the intro loops");
        assert!(p.event(1).is_empty(), "the track is busy: it waits");
        assert!(!p.due(0), "at the segment's end, not the beat");
        let c = p.next(0);
        assert_eq!(seg(&p, c), 2, "the transition first");
        let c = p.next(0);
        assert_eq!(seg(&p, c), 3, "then main");
        let c = p.next(0);
        assert_eq!(seg(&p, c), 4);
        let c = p.next(0);
        assert_eq!(seg(&p, c), 3, "main loops");
    }

    #[test]
    fn a_counted_router_runs_out() {
        let mut m = map();
        m.nodes[1].repeat = 2;
        let mut p = Player::new(m, 7);
        p.event(0);
        let plays: Vec<Cue> = (0..5).map(|_| p.next(0)).collect();
        assert_eq!(
            plays,
            vec![
                Cue::Play(2),
                Cue::Play(2),
                Cue::Play(2),
                Cue::Stop,
                Cue::Stop
            ]
        );
    }
}
