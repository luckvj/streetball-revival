//! The online lobby before a match, as Street V3's online games run
//! theirs (its disc's `selsides`, `exh_selteam2`, `exh_selplayer2`
//! screens: everyone joins, picks a side, then each side picks its team
//! and players at once), on Vol. 2's own screens.
//!
//! Everyone joins first, onto User Setup's columns (one per person, the
//! side chooser: up for AWAY, down for HOME; the server moves them to that
//! side's free seat). Once everyone has set a side, each side's first
//! person (its lowest seat) picks its team and players on Choose Team and
//! Choose Players, both sides at once on their own machines; the match's
//! creator also picks for a side nobody plays, then the court and the
//! rules. What each machine picked goes to everyone in the lobby's list
//! ([`street_net::SeatEntry::pick`]); with every side picked the creator
//! builds the match from them and hands it to the server
//! ([`street_net::Client::propose_setup`]), which checks it as it checks
//! any match, and everyone loads it and plays.

use street_net::SeatEntry;

/// Where this machine stands in the lobby.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    /// Choosing a side.
    #[default]
    Sides,
    /// Side set, waiting for everyone else's.
    SidesDone,
    /// On the menus, picking.
    Picking,
    /// Picked; waiting for the other side (the creator: to build the match).
    Picked,
    /// The creator, loading the match as picked.
    Building,
    /// The creator handed the server the match.
    Proposed,
}

impl Stage {
    /// What the lobby's list shows of it: 0 choosing a side, 1 set, 2
    /// picking, 3 picked.
    pub fn shown(self) -> u8 {
        match self {
            Stage::Sides => 0,
            Stage::SidesDone => 1,
            Stage::Picking => 2,
            Stage::Picked | Stage::Building | Stage::Proposed => 3,
        }
    }
}

/// A side's pick: its team and its three players (records).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SidePick {
    pub side: usize,
    pub team: usize,
    pub lineup: [usize; 3],
}

/// A machine's pick as the lobby carries it: the stage shown, then each
/// side it picked (side, team, three records; little-endian u16s).
pub fn encode(stage: Stage, picks: &[SidePick]) -> Vec<u8> {
    let mut out = vec![stage.shown(), picks.len() as u8];
    for p in picks {
        out.push(p.side as u8);
        out.extend_from_slice(&(p.team as u16).to_le_bytes());
        for r in p.lineup {
            out.extend_from_slice(&(r as u16).to_le_bytes());
        }
    }
    out
}

/// [`encode`] read back: the stage shown and the sides picked (none from
/// bytes that don't read).
pub fn decode(bytes: &[u8]) -> (u8, Vec<SidePick>) {
    let stage = bytes.first().copied().unwrap_or(0);
    let n = usize::from(bytes.get(1).copied().unwrap_or(0));
    let mut picks = Vec::new();
    let word = |at: usize| -> Option<usize> {
        Some(usize::from(u16::from_le_bytes([
            *bytes.get(at)?,
            *bytes.get(at + 1)?,
        ])))
    };
    for k in 0..n.min(2) {
        let at = 2 + k * 9;
        let (Some(&side), Some(team), Some(a), Some(b), Some(c)) = (
            bytes.get(at),
            word(at + 1),
            word(at + 3),
            word(at + 5),
            word(at + 7),
        ) else {
            break;
        };
        if side < 2 {
            picks.push(SidePick {
                side: usize::from(side),
                team,
                lineup: [a, b, c],
            });
        }
    }
    (stage, picks)
}

/// The stage a lobby entry shows.
pub fn stage_of(e: &SeatEntry) -> u8 {
    decode(&e.pick).0
}

/// What this machine picks: the sides whose first person (lowest seat) is
/// one of ours, and, for the creator, the sides nobody plays and the court
/// and rules.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Duties {
    pub sides: Vec<usize>,
    pub court: bool,
}

pub fn duties(lobby: &[SeatEntry], mine: &[u8], creator: bool) -> Duties {
    let mut sides = Vec::new();
    for side in 0..2u8 {
        let first = lobby
            .iter()
            .filter(|e| e.team == side)
            .map(|e| e.seat)
            .min();
        match first {
            Some(seat) if mine.contains(&seat) => sides.push(usize::from(side)),
            None if creator => sides.push(usize::from(side)),
            _ => {}
        }
    }
    Duties {
        sides,
        court: creator,
    }
}

/// Whether everyone in the lobby shows at least stage `stage`.
pub fn everyone_at(lobby: &[SeatEntry], stage: u8) -> bool {
    !lobby.is_empty() && lobby.iter().all(|e| stage_of(e) >= stage)
}

/// The match's teams and line-ups as everyone picked them: each side's
/// pick from whichever machine made it, the creator's own `fallback` for a
/// side nobody's list carries (yet). `None` until everyone has picked.
pub fn compose(
    lobby: &[SeatEntry],
    fallback: ([usize; 2], [[usize; 3]; 2]),
) -> Option<([usize; 2], [[usize; 3]; 2])> {
    if !everyone_at(lobby, 3) {
        return None;
    }
    let (mut teams, mut lineups) = fallback;
    for e in lobby {
        for p in decode(&e.pick).1 {
            teams[p.side] = p.team;
            lineups[p.side] = p.lineup;
        }
    }
    Some((teams, lineups))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(seat: u8, team: u8, pick: Vec<u8>) -> SeatEntry {
        SeatEntry {
            seat,
            team,
            name: format!("P{}", seat + 1),
            ready: false,
            pick,
        }
    }

    #[test]
    fn a_pick_reads_back() {
        let picks = [
            SidePick {
                side: 1,
                team: 39,
                lineup: [200, 7, 1234],
            },
            SidePick {
                side: 0,
                team: 3,
                lineup: [1, 2, 3],
            },
        ];
        let bytes = encode(Stage::Picked, &picks);
        assert!(bytes.len() <= street_net::protocol::MAX_PICK);
        assert_eq!(decode(&bytes), (3, picks.to_vec()));
        assert_eq!(decode(&[]), (0, Vec::new()));
        assert_eq!(decode(&[3, 2, 1]), (3, Vec::new()));
    }

    #[test]
    fn each_sides_first_person_picks_and_the_creator_picks_for_nobodys_side() {
        // 1v1: the creator home (1P), the guest away (2P).
        let lobby = [entry(0, 0, vec![1]), entry(1, 1, vec![1])];
        assert_eq!(
            duties(&lobby, &[0], true),
            Duties {
                sides: vec![0],
                court: true
            }
        );
        assert_eq!(
            duties(&lobby, &[1], false),
            Duties {
                sides: vec![1],
                court: false
            }
        );
        // Both home: the creator picks the computer's side too; the
        // guest (3P) waits.
        let lobby = [entry(0, 0, vec![1]), entry(2, 0, vec![1])];
        assert_eq!(duties(&lobby, &[0], true).sides, vec![0, 1]);
        assert!(duties(&lobby, &[2], false).sides.is_empty());
    }

    #[test]
    fn the_match_is_composed_once_every_side_has_picked() {
        let home = encode(
            Stage::Picked,
            &[SidePick {
                side: 0,
                team: 5,
                lineup: [10, 11, 12],
            }],
        );
        let away = encode(
            Stage::Picking,
            &[SidePick {
                side: 1,
                team: 9,
                lineup: [20, 21, 22],
            }],
        );
        let fallback = ([0, 1], [[0; 3], [1; 3]]);
        let mut lobby = vec![entry(0, 0, home.clone()), entry(1, 1, away)];
        assert_eq!(compose(&lobby, fallback), None);
        lobby[1].pick[0] = 3;
        assert_eq!(
            compose(&lobby, fallback),
            Some(([5, 9], [[10, 11, 12], [20, 21, 22]]))
        );
    }
}
