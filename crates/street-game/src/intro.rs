//! The match intros: before tip-off the game plays two staged scripts from
//! `SCRIPTS.BIN` (the intro task `FUN_00184bb8`), the court's own and then
//! one about the away side's players, on the scripts' own cameras. A
//! button skips them.
//!
//! The picks (`FUN_00184ed8`, `FUN_00184f20`, `FUN_00185138`):
//! - the court's pair at `0x3db318` (`IntCourt1a`/`1b` .. `11a`/`11b`, by
//!   the court record);
//! - then, for the first away player who has one: a streetball legend
//!   (team 29, records 145..) their boss intro (`0x3db3c8`, two per
//!   record; record 146 on court 9 always `IntBoss05`), an legend
//!   (teams 30..35, records 158..) theirs (`0x3db498`);
//! - else a generic one (`0x3db2b0`): `IntGen01`..`05` (with their `B`
//!   halves) when a generic crew player (team 36) is on that side, else
//!   all thirteen with `IntGen11`..`14`.
//!
//! Each list plays every entry before repeating (`FUN_001851f0`, the used
//! counts living in `.data` for the session). The intros only draw: the
//! simulation waits until they end. They are left out in scripted runs.
//!
//! Online every machine plays them too: each draws the picks from the
//! match's seed ([`Variety::seeded`]), the creator sends how many updates
//! they last ([`ticks`]), and the match holds play that long on every
//! machine (`street-net`'s `Match::intro_hold`), a fresh Cross from anyone
//! skipping them for all. The shown frame follows the match's updates
//! ([`MatchIntro::follow`]) rather than this machine's clock.
//!
//! A script's own commands shape its playing: it stops at its stop command
//! (most intros end short of their last frame), a freeze holds it while
//! another of its cameras moves, and a replay command plays its span again
//! slowed on a camera of its own (the legends' intros replay their dunk).
//! A Level 2 gamebreaker's script is the simulation's, and its replay the
//! game's replay (`street_sim::replay`, mode 7; `crate::tape`).

use std::collections::{HashSet, VecDeque};

use bevy::prelude::*;
use street_sim::gamebreaker::{SequencePlan, bind};

use crate::{
    game::Sim,
    menu::MenuInput,
    setup::{Disc, GameClips, MatchSetup},
};

/// Staged frames a second.
const FPS: f32 = 60.0;

/// The court intro pairs, by court record (script indices).
const COURT_FIRST: usize = 29;
/// The generic intros: 13, or the first 9 with a generic crew player.
const GENERIC: [usize; 13] = [16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28];
const GENERIC_CREW: usize = 9;
/// Streetball legends' boss intros, two per record from 145 (`0x3db3c8`).
const BOSS: [[Option<usize>; 2]; 6] = [
    [Some(51), Some(52)],
    [Some(59), None],
    [Some(53), Some(54)],
    [Some(56), Some(57)],
    [Some(55), None],
    [Some(60), Some(61)],
];
const BOSS_FIRST: usize = 145;
/// Record 146 on court 9: `IntBoss05` itself.
const BOSS_SPECIAL: (usize, usize, usize) = (146, 9, 58);
/// legends' intros by record (`0x3db498`, the first of each pair).
const LEGEND: [(usize, usize); 8] = [
    (158, 63),
    (166, 64),
    (167, 67),
    (169, 69),
    (172, 62),
    (175, 68),
    (178, 66),
    (181, 65),
];

/// Whether matches open with their intros (off for scripted runs, as the
/// boot movies are; `--intro`/`--no-intro` decide).
#[derive(Resource, Clone, Copy)]
pub struct Enabled(pub bool);

/// Which intros have played this session (`FUN_001851f0`'s used counts),
/// and the dice.
#[derive(Resource)]
pub struct Variety {
    used: HashSet<usize>,
    rng: crate::progression_trace::NativeRandom,
}

impl Default for Variety {
    fn default() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.subsec_nanos() | 1);
        Self {
            used: HashSet::new(),
            rng: crate::progression_trace::NativeRandom::new(seed),
        }
    }
}

impl Variety {
    /// A fresh session's counts with dice from `seed`: an online match's,
    /// which every machine draws alike.
    pub fn seeded(seed: u32) -> Self {
        Self {
            used: HashSet::new(),
            rng: crate::progression_trace::NativeRandom::new(seed | 1),
        }
    }

    /// One of `choices` not played yet; when all have, they all count
    /// again.
    fn pick(&mut self, choices: &[usize]) -> Option<usize> {
        if choices.is_empty() {
            return None;
        }
        let mut free: Vec<usize> = choices
            .iter()
            .copied()
            .filter(|c| !self.used.contains(c))
            .collect();
        if free.is_empty() {
            for c in choices {
                self.used.remove(c);
            }
            free = choices.to_vec();
        }
        let chosen = free[self.rng.draw() as usize % free.len()];
        self.used.insert(chosen);
        Some(chosen)
    }
}

/// The two scripts a match opens with: the court's, then the away side's
/// (with the player it features, for a legend's own).
pub fn choose(
    court: usize,
    away: &[(usize, i16)],
    variety: &mut Variety,
) -> (Option<usize>, Option<(usize, Option<usize>)>) {
    let first = (court < 11).then(|| {
        let pair = [COURT_FIRST + 2 * court, COURT_FIRST + 2 * court + 1];
        variety.pick(&pair)
    });
    let mut second = None;
    for &(record, team) in away.iter().take(3) {
        let pick = if (30..36).contains(&team) {
            LEGEND
                .iter()
                .find(|&&(r, _)| r == record)
                .and_then(|&(_, s)| variety.pick(&[s]))
        } else if team == 29 {
            if (record, court) == (BOSS_SPECIAL.0, BOSS_SPECIAL.1) {
                Some(BOSS_SPECIAL.2)
            } else {
                record
                    .checked_sub(BOSS_FIRST)
                    .and_then(|k| BOSS.get(k))
                    .and_then(|pair| {
                        let choices: Vec<usize> = pair.iter().flatten().copied().collect();
                        variety.pick(&choices)
                    })
            }
        } else {
            None
        };
        if let Some(script) = pick {
            second = Some((script, Some(record)));
            break;
        }
    }
    if second.is_none() {
        let crew = away.iter().take(3).any(|&(_, team)| team == 36);
        let pool = if crew {
            &GENERIC[..GENERIC_CREW]
        } else {
            &GENERIC[..]
        };
        second = variety.pick(pool).map(|s| (s, None));
    }
    (first.flatten(), second)
}

/// One staged script playing: its plan, who plays its roles, and how.
pub struct Part {
    pub script: usize,
    pub plan: SequencePlan,
    pub actors: [Option<usize>; 7],
    /// Mirrored for team 0, toward its basket at +x (the scripts are
    /// authored attacking -x).
    pub mirrored: bool,
    pub hoop: usize,
    /// An intro shows its cast alone.
    pub cast_only: bool,
}

/// Where a part is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Phase {
    #[default]
    Play,
    /// Frames into the freeze.
    Hold(f32),
    /// The replay's own frame.
    Replay(f32),
}

/// The staged scripts playing over a stopped simulation: the match's
/// intro.
#[derive(Resource, Default)]
pub struct MatchIntro {
    queue: VecDeque<Part>,
    pub current: Option<Part>,
    /// Staged frames into the current part.
    pub frame: f32,
    pub phase: Phase,
    /// The part's freeze and replay have played.
    held: bool,
    replayed: bool,
    /// Updates run since the intro began (online, [`MatchIntro::follow`]).
    ticks: u32,
}

impl MatchIntro {
    /// The frame the actors stand at, and the camera on screen (its id and
    /// the frame of its track).
    pub fn shown(&self) -> Option<(f32, u8, f32)> {
        let part = self.current.as_ref()?;
        let c = part.plan.controls;
        Some(match self.phase {
            Phase::Play => (self.frame, 0, self.frame),
            Phase::Hold(t) => {
                let h = c.hold?;
                (self.frame, h.camera, h.frame as f32 + t)
            }
            Phase::Replay(f) => (f, c.rerun?.camera, f),
        })
    }

    /// `part` on screen from its first frame.
    #[cfg(test)]
    pub fn showing(part: Part) -> Self {
        Self {
            current: Some(part),
            ..default()
        }
    }

    /// A replay is on screen.
    pub fn replaying(&self) -> bool {
        matches!(self.phase, Phase::Replay(_)) && self.current.is_some()
    }

    /// Online: run on to `elapsed` updates since the intro began, one
    /// staged frame an update (the match's clock is the network's).
    pub fn follow(&mut self, elapsed: u32) {
        while self.ticks < elapsed && self.current.is_some() {
            self.advance(1.0);
            self.ticks += 1;
        }
    }

    /// The rest skipped.
    pub fn end(&mut self) {
        self.queue.clear();
        self.next();
    }

    fn next(&mut self) {
        self.current = self.queue.pop_front();
        self.frame = 0.0;
        self.phase = Phase::Play;
        self.held = false;
        self.replayed = false;
    }

    /// Run `frames` staged frames.
    fn advance(&mut self, frames: f32) {
        let Some(part) = self.current.as_ref() else {
            return;
        };
        let c = part.plan.controls;
        let last = part.plan.length().saturating_sub(1) as f32;
        match self.phase {
            Phase::Play => {
                let next = self.frame + frames;
                if let Some(h) = c.hold.filter(|h| !self.held && (h.frame as f32) < next) {
                    self.frame = h.frame as f32;
                    self.phase = Phase::Hold(0.0);
                } else if let Some(r) = c.rerun.filter(|r| !self.replayed && r.to as f32 <= next) {
                    self.frame = r.to as f32;
                    self.phase = Phase::Replay(r.from as f32);
                } else if next >= last {
                    self.next();
                } else {
                    self.frame = next;
                }
            }
            Phase::Hold(t) => {
                let t = t + frames;
                if c.hold.is_none_or(|h| t >= h.duration as f32) {
                    self.held = true;
                    self.phase = Phase::Play;
                } else {
                    self.phase = Phase::Hold(t);
                }
            }
            Phase::Replay(f) => {
                let speed = c.rerun.map_or(1.0, |r| f32::from(r.speed) / 100.0);
                let f = f + frames * speed;
                if c.rerun.is_none_or(|r| f >= r.to as f32) {
                    self.replayed = true;
                    self.phase = Phase::Play;
                } else {
                    self.phase = Phase::Replay(f);
                }
            }
        }
    }
}

/// True once the intro is over (or there is none): play may start.
pub fn finished(intro: Option<Res<MatchIntro>>) -> bool {
    intro.is_none_or(|i| i.current.is_none())
}

/// Queue the match's intro (on entering a match). Online the picks are the
/// seed's, and the match says how long they hold (an intro the creator
/// left out ends at once, [`MatchIntro::follow`]'s caller).
pub fn start(
    mut commands: Commands,
    enabled: Option<Res<Enabled>>,
    sim: Res<Sim>,
    setup: Res<MatchSetup>,
    disc: Res<Disc>,
    clips: Res<GameClips>,
    mut variety: ResMut<Variety>,
    running: Option<Res<crate::story::Running>>,
) {
    commands.insert_resource(MatchIntro::default());
    // Street School's game opens on its lesson menu, with no intro.
    let school = running.is_some_and(|r| matches!(r.0, Some(crate::story::Run::School(_))));
    if school || (!setup.online && !enabled.is_some_and(|e| e.0)) {
        return;
    }
    let queue = if setup.online {
        parts(
            &setup,
            &sim.world,
            &disc,
            &clips,
            &mut Variety::seeded(setup.seed),
        )
    } else {
        parts(&setup, &sim.world, &disc, &clips, &mut variety)
    };
    let mut intro = MatchIntro { queue, ..default() };
    intro.next();
    commands.insert_resource(intro);
}

/// How many updates `parts` play for, one staged frame an update.
pub fn ticks(parts: VecDeque<Part>) -> u32 {
    let mut intro = MatchIntro {
        queue: parts,
        ..default()
    };
    intro.next();
    let mut n = 0;
    while intro.current.is_some() {
        intro.advance(1.0);
        n += 1;
    }
    n
}

/// The match's intro: the court's script, then the away side's, each bound
/// to its cast.
pub fn parts(
    setup: &MatchSetup,
    w: &street_sim::World,
    disc: &Disc,
    clips: &GameClips,
    variety: &mut Variety,
) -> VecDeque<Part> {
    let Some(court) = disc
        .courts
        .iter()
        .find(|c| c.id.eq_ignore_ascii_case(&setup.court))
        .map(|c| c.index)
    else {
        return VecDeque::new();
    };
    let side = |team: usize| -> Vec<usize> {
        (0..w.players.len())
            .filter(|&i| w.team_of(i) == team)
            .collect()
    };
    let (home, away) = (side(0), side(1));
    let records: Vec<(usize, i16)> = setup.lineups[1]
        .iter()
        .map(|&r| (r, disc.players.get(r).map_or(-1, |p| p.team)))
        .collect();
    let (first, second) = choose(court, &records, variety);
    let mut queue = VecDeque::new();
    let mut add = |script: usize, featured: Option<usize>| {
        let Some(timeline) = clips.timeline.scripts.get(script) else {
            return;
        };
        let mut plan = crate::staged::plan(timeline, script);
        if let Err(error) = crate::staged::attach_playback(&mut plan, &clips.timeline, &clips.clips)
        {
            warn!("intro playback: {error}");
            return;
        }
        let lead = if featured.is_some() {
            &away
        } else if queue.is_empty() {
            &home
        } else {
            &away
        };
        // The featured legend leads (role 0); else that side's first.
        let shooter = featured
            .and_then(|record| {
                let k = setup.lineups[1].iter().position(|&r| r == record)?;
                away.get(k).copied()
            })
            .or_else(|| lead.first().copied());
        let Some(shooter) = shooter else {
            return;
        };
        let actors = bind(&w.players, shooter, plan.kind, plan.initial_positions());
        queue.push_back(Part {
            script,
            plan,
            actors,
            mirrored: false,
            hoop: 0,
            cast_only: true,
        });
    };
    if let Some(script) = first {
        add(script, None);
    }
    if let Some((script, featured)) = second {
        add(script, featured);
    }
    queue
}

/// Run the intro's clock; a button skips the rest. Online the match runs
/// it (`online::net_step`).
pub fn step(
    time: Res<Time>,
    input: MenuInput,
    intro: Option<ResMut<MatchIntro>>,
    paused: Res<crate::game::Paused>,
    setup: Res<MatchSetup>,
) {
    let Some(mut intro) = intro else {
        return;
    };
    if intro.current.is_none() || paused.on || setup.online {
        return;
    }
    if input.confirm() || input.pause() {
        intro.end();
        return;
    }
    intro.advance(time.delta_secs() * FPS);
}

/// The match's gate: play waits for the intro played alone. Online the
/// match holds its own world through the intro, and the network runs on.
pub fn finished_or_online(
    intro: Option<Res<MatchIntro>>,
    online: Option<Res<crate::online::Online>>,
) -> bool {
    online.is_some() || intro.is_none_or(|i| i.current.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variety() -> Variety {
        Variety {
            used: HashSet::new(),
            rng: crate::progression_trace::NativeRandom::new(1),
        }
    }

    fn part(controls: street_sim::gamebreaker::Controls) -> Part {
        Part {
            script: 0,
            plan: SequencePlan {
                script: 0,
                kind: 0,
                frame_count: 100,
                actors: vec![],
                curves: vec![],
                ball_cues: vec![],
                controls,
                finishes: vec![],
            },
            actors: [None; 7],
            mirrored: false,
            hoop: 0,
            cast_only: true,
        }
    }

    #[test]
    fn a_part_freezes_replays_and_stops_as_its_commands_say() {
        use street_sim::gamebreaker::{Controls, Hold, Rerun};
        let controls = Controls {
            stop: Some(80),
            hold: Some(Hold {
                frame: 20,
                duration: 10,
                camera: 1,
                holds_replays: true,
            }),
            rerun: Some(Rerun {
                from: 40,
                to: 60,
                speed: 50,
                camera: 2,
            }),
        };
        let mut intro = MatchIntro::default();
        intro.queue.push_back(part(controls));
        intro.next();
        let mut seen = Vec::new();
        for _ in 0..400 {
            let Some(shown) = intro.shown() else { break };
            seen.push(shown);
            intro.advance(1.0);
        }
        // Played 0..20, held at 20 on camera 1 for ten frames, played to
        // 60, replayed 40..60 at half speed on camera 2, played on and
        // stopped at 80 (its stop command), short of its 100 frames.
        let held = seen.iter().filter(|s| s.1 == 1).count();
        let replayed = seen.iter().filter(|s| s.1 == 2).count();
        assert_eq!(held, 10);
        assert_eq!(replayed, 40);
        assert!(seen.iter().all(|s| s.0 < 80.0));
        assert!(intro.current.is_none());
    }

    #[test]
    fn online_the_intro_ends_on_the_update_the_match_holds_it_for() {
        use street_sim::gamebreaker::{Controls, Hold};
        let controls = Controls {
            stop: Some(70),
            hold: Some(Hold {
                frame: 10,
                duration: 15,
                camera: 1,
                holds_replays: true,
            }),
            rerun: None,
        };
        let parts = || VecDeque::from([part(controls), part(Controls::default())]);
        let n = ticks(parts());
        let mut intro = MatchIntro {
            queue: parts(),
            ..default()
        };
        intro.next();
        intro.follow(n - 1);
        assert!(intro.current.is_some());
        intro.follow(n);
        assert!(intro.current.is_none());
        // The same seed draws the same picks on every machine.
        let a = choose(3, &[(200, 36)], &mut Variety::seeded(77));
        let b = choose(3, &[(200, 36)], &mut Variety::seeded(77));
        assert_eq!(a, b);
    }

    #[test]
    fn the_court_pair_plays_both_before_repeating() {
        let mut v = variety();
        let a = choose(0, &[], &mut v).0.unwrap();
        let b = choose(0, &[], &mut v).0.unwrap();
        assert_eq!([a.min(b), a.max(b)], [29, 30]);
        assert_eq!(choose(10, &[], &mut v).0.map(|s| s >= 49), Some(true));
    }

    #[test]
    fn legends_get_their_own_intro_and_crews_the_short_generic_list() {
        let mut v = variety();
        // An legend on the away side: their own.
        let (_, second) = choose(0, &[(1, 5), (166, 31)], &mut v);
        assert_eq!(second, Some((64, Some(166))));
        // Record 146 at court 9: IntBoss05.
        let (_, second) = choose(9, &[(146, 29)], &mut v);
        assert_eq!(second, Some((58, Some(146))));
        // A generic crew player: only IntGen01..05 and their halves.
        for _ in 0..30 {
            let (_, second) = choose(2, &[(200, 36)], &mut v);
            let (script, featured) = second.unwrap();
            assert!((16..25).contains(&script) && featured.is_none());
        }
    }
}
