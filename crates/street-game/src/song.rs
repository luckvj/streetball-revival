//! An in-game song played the way the game plays it: its PathFinder map
//! (`street_audio::path`) strings `MAIN.MUS`'s segments together and answers
//! the game's music events with the `WHOOSH` and `TRICKS` one-shots.
//!
//! The map runs inside the audio source, so segments follow one another on
//! the sample; the game side ([`cues`]) posts events and the value through
//! [`MusicLink`]. What fires which event is the game's `AuPathManager`
//! (`0x1aa4b0`..`0x1abf70`) driven by its audio listener (`0x45c3a0`):
//!
//! | Moment | Event |
//! |---|---|
//! | the match opens | 0, the song's intro, looping |
//! | play starts | 1, the main groove (after a transition) |
//! | a basket | the value: the leader's score × 127 / the target (`0x1a3b10`) |
//! | a meter fills | 3 (level 1) or 6 (level 2), the ready theme |
//! | a gamebreaker goes up | 9 or 10 |
//! | it scores | everything stops, then 1 (or the other side's ready theme) |
//! | a meter is pocketed or cancelled | everything stops, then 2 (or the other side's) |
//! | a level 4 or 5 trick | 13 or 14, a stinger on `TRICKS` (every 2 s at most) |
//! | the game ends | 15, the outro |
//!
//! Songs rotate (`audio_rotate_game_songs` and `audio_song_change_on_gb`,
//! both on): when a gamebreaker ends more than two minutes into a song
//! (`0x1ab760`: VBlanks since the song opened over `fps * 120.0`, unless the
//! other side's meter is full), or the groove has played past four minutes
//! (`0x1ab958`, checked on the listener's slot 41), the music stops and the
//! next song opens on its intro, then its groove. The rewrite counts its
//! fixed 60 Hz ticks for the VBlanks.
//!
//! The dunk mix (`0x1ab070`, docs/TRACE.md "The dunk mix drops the
//! music"): a dunker's take-off at a finish of 2 or more (slot 4) or a
//! pass to an alley-oop flier (slot 18), while the groove plays with
//! interactive music on, sets the song's stream dry level to 0 and sends
//! it to the effects, whose area the game never sets up: the music drops
//! out (0.5 s on, its volume goes to 0 too, `0x1ab198`) until the slam,
//! a block, a basket, a miss, a catch, a steal, a rebound, a pass to anyone
//! else, a meter filling or a violation puts it back (`0x1ab210`). The
//! one-shots (`WHOOSH`, `TRICKS`) are not the stream and keep playing.

use std::{
    fs::File,
    io::BufReader,
    sync::{Arc, Mutex},
    time::Duration,
};

use bevy::{
    audio::{ChannelCount, Decodable, SampleRate, Source},
    prelude::*,
};
use street_assets::iso::Iso;
use street_audio::{
    bank::Bank,
    music::Song,
    path::{self, Cue, Player, event},
    schl,
};
use street_sim::{Basket, Event, Goal, World};

use crate::game::{Sim, TickEvents};

/// What the game posts to the music.
#[derive(Default)]
struct Inbox {
    commands: Vec<Command>,
    value: u8,
    /// The dunk mix: the song's stream silent.
    dry_off: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Event(u8),
    /// Every track stops at once, before the next event.
    Stop,
}

/// The playing song's mailbox, for this match.
#[derive(Resource)]
pub struct MusicLink {
    inbox: Arc<Mutex<Inbox>>,
    started: bool,
    last_stinger: f64,
    over: bool,
    /// The simulation tick the song opened on.
    since: u64,
    /// The groove is playing (not a ready or gamebreaker theme).
    groove: bool,
    /// A gamebreaker shot is up, and its side (the listener's +0x28).
    breaking: Option<usize>,
    /// Time for the next song: the game side swaps it in.
    pub(crate) next: bool,
    /// The dunk mix is on (the manager's states 5 and 6).
    mixed: bool,
}

/// A gamebreaker ending more than this long into a song moves on to the
/// next (s; `0x1ab8a4`: `fps * 120.0`).
const NEXT_AFTER_GAMEBREAKER: u64 = 120;
/// The groove gives way to the next song after more than this long (s;
/// `0x1ab9b4`: `fps * 240.0`).
const NEXT_IN_GROOVE: u64 = 240;

impl MusicLink {
    fn new(inbox: Arc<Mutex<Inbox>>) -> Self {
        MusicLink {
            inbox,
            started: false,
            last_stinger: f64::NEG_INFINITY,
            over: false,
            since: 0,
            groove: false,
            breaking: None,
            next: false,
            mixed: false,
        }
    }

    /// `0x1ab070` / `0x1ab210`: the dunk mix on or off.
    fn mix(&mut self, on: bool) {
        if self.mixed != on {
            self.mixed = on;
            if let Ok(mut inbox) = self.inbox.lock() {
                inbox.dry_off = on;
            }
        }
    }

    fn post(&self, c: Command) {
        if let Ok(mut inbox) = self.inbox.lock() {
            inbox.commands.push(c);
        }
    }

    /// The song took over at `tick`, straight into its groove when `main`
    /// (a song that follows another: events 0 and 1 together, `0x1aa8d8`).
    pub fn opened(&mut self, tick: u64, main: bool) {
        self.since = tick;
        if main {
            self.started = true;
            self.groove = true;
            self.post(Command::Event(event::MAIN));
        }
    }

    /// Seconds of play since the song opened.
    #[cfg(test)]
    fn age(&self, w: &World) -> u64 {
        w.tick.saturating_sub(self.since) / u64::from(street_sim::TICK_HZ)
    }

    /// Whether the song has played more than `seconds` (`0x1ab760` and
    /// `0x1ab958` compare the VBlank count, not whole seconds).
    fn past(&self, w: &World, seconds: u64) -> bool {
        w.tick.saturating_sub(self.since) > seconds * u64::from(street_sim::TICK_HZ)
    }
}

/// A song ready to play.
#[derive(Asset, TypePath, Clone)]
pub struct PathSong {
    main: Arc<[u8]>,
    /// `MAIN.MUS`'s stream for each segment (by id - 1), if on track 0.
    streams: Arc<[Option<schl::Stream>]>,
    /// The banks' sounds, stereo at the song's rate: `WHOOSH`, `TRICKS`.
    sounds: [Arc<[Arc<[f32]>]>; 2],
    player: Player,
    inbox: Arc<Mutex<Inbox>>,
    rate: u32,
}

/// `samples` (interleaved, `channels`) as stereo at `rate`, linearly.
fn to_stereo(samples: &[i16], channels: u16, from: u32, rate: u32) -> Arc<[f32]> {
    let ch = usize::from(channels.max(1));
    let frames = samples.len() / ch;
    let out_frames = (frames as u64 * u64::from(rate) / u64::from(from.max(1))) as usize;
    let at = |f: usize, c: usize| {
        f32::from(samples[f.min(frames.saturating_sub(1)) * ch + c.min(ch - 1)]) / 32768.0
    };
    let mut out = Vec::with_capacity(out_frames * 2);
    for k in 0..out_frames {
        let x = k as f64 * f64::from(from) / f64::from(rate);
        let (i, t) = (x as usize, (x.fract()) as f32);
        for c in 0..2 {
            out.push(at(i, c) * (1.0 - t) + at(i + 1, c) * t);
        }
    }
    out.into()
}

/// Loads `song`'s map, segments and banks, and the link the game posts to.
pub fn load(
    iso: &mut Iso<BufReader<File>>,
    song: &Song,
    seed: u32,
) -> Option<(PathSong, MusicLink)> {
    let read =
        |iso: &mut Iso<BufReader<File>>, p: &str| iso.read(p).map_err(|e| warn!("{p}: {e}")).ok();
    let map = path::Map::parse(&read(iso, &song.map())?)
        .map_err(|e| warn!("{}: {e}", song.map()))
        .ok()?;
    let main = read(iso, &song.main())?;
    let found = schl::streams(&main).ok()?;
    let first = found.first()?.patch.clone();
    let rate = first.rate;
    let streams: Vec<Option<schl::Stream>> = map
        .segments
        .iter()
        .map(|s| {
            (s.track == 0)
                .then(|| found.iter().find(|f| f.start as u32 == s.at).cloned())
                .flatten()
                .filter(|f| f.patch.rate == rate)
        })
        .collect();
    let mut sounds: [Arc<[Arc<[f32]>]>; 2] = [Arc::new([]), Arc::new([])];
    for (k, p) in [song.whoosh(), song.tricks()].iter().enumerate() {
        let Some(bank) = read(iso, p).and_then(|d| Bank::parse(d).ok()) else {
            continue;
        };
        sounds[k] = (0..bank.len())
            .map(|i| match bank.clip(i) {
                Ok(c) => to_stereo(&c.samples, c.channels, c.rate, rate),
                Err(_) => Arc::from(Vec::new()),
            })
            .collect();
    }
    let inbox = Arc::new(Mutex::new(Inbox::default()));
    let link = MusicLink::new(inbox.clone());
    // The song opens on its intro.
    link.post(Command::Event(event::INTRO));
    let song = PathSong {
        main: main.into(),
        streams: streams.into(),
        sounds,
        player: Player::new(map, seed),
        inbox,
        rate,
    };
    Some((song, link))
}

enum Audio {
    Idle,
    Stream(schl::Reader, u16),
    /// Bank sounds in a row: which track's bank, the sounds, the one
    /// playing and the frame in it.
    Sounds {
        bank: usize,
        list: Vec<usize>,
        at: usize,
        pos: usize,
    },
}

struct Lane {
    playing: bool,
    frame: u64,
    length: u64,
    beat: u64,
    audio: Audio,
}

impl Lane {
    fn idle() -> Self {
        Lane {
            playing: false,
            frame: 0,
            length: 0,
            beat: 0,
            audio: Audio::Idle,
        }
    }
}

pub struct PathSource {
    song: PathSong,
    player: Player,
    lanes: Vec<Lane>,
    right: Option<f32>,
    frames: u64,
    /// The dunk mix: the stream's segments play on, unheard.
    dry_off: bool,
}

impl PathSource {
    /// The game's latest posts.
    fn poll(&mut self) {
        let Ok(mut inbox) = self.song.inbox.try_lock() else {
            return;
        };
        self.player.value = inbox.value;
        self.dry_off = inbox.dry_off;
        let commands = std::mem::take(&mut inbox.commands);
        drop(inbox);
        for c in commands {
            match c {
                Command::Stop => {
                    self.player.stop_all();
                    for lane in &mut self.lanes {
                        *lane = Lane::idle();
                    }
                }
                Command::Event(e) => {
                    for t in self.player.event(e) {
                        self.cue(t);
                    }
                }
            }
        }
    }

    /// The next segment on track `t`.
    fn cue(&mut self, t: u8) {
        let lane = Lane::idle();
        let next = self.player.next(t);
        let lane = match next {
            Cue::Stop => lane,
            Cue::Play(node) => self.start(node).unwrap_or(lane),
        };
        self.lanes[usize::from(t)] = lane;
    }

    fn start(&self, node: u16) -> Option<Lane> {
        let map = &self.player.map;
        let n = map.nodes.get(usize::from(node))?;
        let id = usize::from(n.segment).checked_sub(1)?;
        let seg = *map.segments.get(id)?;
        let beats = u64::from(n.beats.max(1));
        let length = u64::from(seg.millis) * u64::from(self.song.rate) / 1000;
        let audio = if seg.track == 0 {
            let s = self.song.streams.get(id)?.clone()?;
            let ch = s.patch.channels;
            Audio::Stream(schl::Reader::new(self.song.main.clone(), s, false), ch)
        } else {
            let bank = usize::from(seg.track - 1).min(1);
            let first = seg.at as usize;
            Audio::Sounds {
                bank,
                list: (first..first + beats as usize).collect(),
                at: 0,
                pos: 0,
            }
        };
        Some(Lane {
            playing: true,
            frame: 0,
            length: length.max(1),
            beat: (length / beats).max(1),
            audio,
        })
    }

    fn frame(&mut self) -> [f32; 2] {
        if self.frames.is_multiple_of(256) {
            self.poll();
        }
        self.frames += 1;
        let mut out = [0f32; 2];
        for t in 0..self.lanes.len() {
            let lane = &self.lanes[t];
            let t8 = t as u8;
            if lane.playing {
                let on_beat = lane.frame > 0 && lane.frame.is_multiple_of(lane.beat);
                if lane.frame >= lane.length || (on_beat && self.player.due(t8)) {
                    self.cue(t8);
                }
            } else if self.player.due(t8) {
                // A stop waiting on an idle track.
                self.cue(t8);
            }
            let sounds = &self.song.sounds;
            let lane = &mut self.lanes[t];
            if !lane.playing {
                continue;
            }
            lane.frame += 1;
            match &mut lane.audio {
                Audio::Idle => {}
                Audio::Stream(r, ch) => {
                    let l = r.next().map_or(0.0, |v| f32::from(v) / 32768.0);
                    let rr = if *ch >= 2 {
                        r.next().map_or(0.0, |v| f32::from(v) / 32768.0)
                    } else {
                        l
                    };
                    if !self.dry_off {
                        out[0] += l;
                        out[1] += rr;
                    }
                }
                Audio::Sounds {
                    bank,
                    list,
                    at,
                    pos,
                } => {
                    while let Some(&i) = list.get(*at) {
                        let s = sounds[*bank].get(i).map_or(&[][..], |s| &s[..]);
                        if *pos * 2 + 1 < s.len() {
                            out[0] += s[*pos * 2];
                            out[1] += s[*pos * 2 + 1];
                            *pos += 1;
                            break;
                        }
                        *at += 1;
                        *pos = 0;
                    }
                }
            }
        }
        out.map(|v| v.clamp(-1.0, 1.0))
    }
}

impl Iterator for PathSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if let Some(r) = self.right.take() {
            return Some(r);
        }
        let [l, r] = self.frame();
        let [l, r] = crate::audio::mono(l, r);
        self.right = Some(r);
        Some(l)
    }
}

impl Source for PathSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).expect("non-zero")
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(self.song.rate.max(1)).expect("non-zero")
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Decodable for PathSong {
    type Decoder = PathSource;

    fn decoder(&self) -> PathSource {
        let tracks = usize::from(self.player.map.tracks);
        PathSource {
            song: self.clone(),
            player: self.player.clone(),
            lanes: (0..tracks).map(|_| Lane::idle()).collect(),
            right: None,
            frames: 0,
            dry_off: false,
        }
    }
}

/// How far the leading side is to winning, 0..127 (`0x1a3b10`).
fn value(w: &World) -> u8 {
    let (now, of) = match w.rules.goal {
        Goal::Points(n) => (w.score[0].max(w.score[1]), n),
        Goal::TrickPoints(n) => (w.teams[0].trick_points.max(w.teams[1].trick_points), n),
        Goal::Gamebreaker(_) => (0, 1),
    };
    (u64::from(now) * 127 / u64::from(of.max(1))).min(127) as u8
}

/// After a gamebreaker or an emptied meter: the side with a full meter
/// gets its ready theme, else the groove comes back.
fn settle(link: &mut MusicLink, w: &World, then: u8) {
    link.post(Command::Stop);
    match (0..2).find(|&t| w.teams[t].meter_full()) {
        Some(t) => {
            link.groove = false;
            link.post(Command::Event(
                event::READY[usize::from(w.teams[t].gamebreaker().clamp(1, 2)) - 1],
            ));
        }
        None => {
            link.groove = true;
            link.post(Command::Event(then));
        }
    }
}

/// `team`'s gamebreaker is over (`0x1a6b80` calls `0x1ab760` with its side
/// and 0): the other side's full meter gets its ready theme (its own is
/// only asked when the third argument is set); else, past two minutes into
/// the song, the next song; else everything stops and the groove resumes.
fn gamebreaker_over(link: &mut MusicLink, w: &World, team: usize) {
    link.breaking = None;
    let other = 1 - team.min(1);
    if w.teams[other].meter_full() {
        link.post(Command::Stop);
        link.groove = false;
        link.post(Command::Event(
            event::READY[usize::from(w.teams[other].gamebreaker().clamp(1, 2)) - 1],
        ));
    } else if link.past(w, NEXT_AFTER_GAMEBREAKER) {
        link.post(Command::Stop);
        link.next = true;
    } else {
        link.groove = true;
        link.post(Command::Stop);
        link.post(Command::Event(event::MAIN));
    }
}

/// The groove starts at the tip-off: the music listener's slot `+0x144`
/// (`0x1a6f38`, the record `FUN_0014c858` posts when the referee's toss
/// goes up) posts event 1. A match without the toss (online, or not three
/// a side) starts it once the court's name has shown, the rewrite's own.
fn tip_off(link: &mut MusicLink, w: &World) {
    let start = match w.opening_toss {
        street_sim::OpeningToss::Flight => true,
        street_sim::OpeningToss::Waiting(_) => false,
        street_sim::OpeningToss::None => w.tick >= crate::hud::INTRO_TICKS,
    };
    if !link.started && start {
        link.started = true;
        link.groove = true;
        link.post(Command::Event(event::MAIN));
    }
}

/// The dunk mix's starts and ends among the listener's slots: `0x1ab070`
/// from slots 4 (a take-off at a finish of 2 or more, `0x1a4a7c`) and 18
/// (a pass to a flier, `0x1a6010`), only while the song is in its groove
/// (the manager's state 1) with interactive music on; `0x1ab210`, which
/// acts only in states 5 and 6, from slots 12, 15, 16, 17, 18 (a pass to
/// anyone else), 20, 21 (a knock that is no block, live only), 25 (a loose
/// ball picked up), 26, 27, 52 and 66. A catch (slot 19, an empty handler)
/// and the take's slot 24 leave it: an alley-oop's music stays down from
/// the pass to the slam.
fn dunk_mix(link: &mut MusicLink, e: &Event, interactive: bool, live: bool) {
    match *e {
        Event::TakeOff {
            leap: false,
            finish,
            ..
        } if finish >= 2 => {
            if link.groove && interactive {
                link.mix(true);
            }
        }
        Event::Launched { leaper: true, .. } => {
            if link.groove && interactive {
                link.mix(true);
            }
        }
        Event::Slam { .. }
        | Event::Steal { .. }
        | Event::Counter { .. }
        | Event::Block { .. }
        | Event::Rebound { .. }
        | Event::Pickup { .. }
        | Event::Launched { leaper: false, .. }
        | Event::Score { .. }
        | Event::RimOut
        | Event::ShotClockViolation { .. }
        | Event::GamebreakerReady { .. } => link.mix(false),
        Event::Deflection { .. } if live => link.mix(false),
        _ => {}
    }
}

/// The simulation's moments, as music events. Runs after every tick.
pub fn cues(
    link: Option<ResMut<MusicLink>>,
    sim: Res<Sim>,
    tick: Res<TickEvents>,
    time: Res<Time<Real>>,
    options: Option<Res<crate::options::Options>>,
) {
    let Some(mut link) = link else { return };
    if tick.replay || link.over {
        return;
    }
    let w = &sim.world;
    if let Ok(mut inbox) = link.inbox.lock() {
        inbox.value = value(w);
    }
    tip_off(&mut link, w);
    if link.next {
        return;
    }
    let interactive = options.as_ref().is_none_or(|o| o.0.interactive);
    let live = matches!(w.phase, street_sim::Phase::Live);
    for e in &tick.events {
        dunk_mix(&mut link, e, interactive, live);
        match *e {
            Event::GamebreakerReady { team } => {
                link.groove = false;
                let level = w.teams[team].gamebreaker().clamp(1, 2);
                link.post(Command::Event(event::READY[usize::from(level) - 1]));
            }
            Event::Gamebreaker { player, level } => {
                link.groove = false;
                link.breaking = w.players.get(player).map(|p| p.team.min(1));
                link.post(Command::Event(
                    event::GAMEBREAKER[usize::from(level.clamp(1, 2)) - 1],
                ));
            }
            Event::Score {
                team,
                basket: Basket::Gamebreaker { .. },
                ..
            } => gamebreaker_over(&mut link, w, team),
            // A gamebreaker that misses or is swatted ends too.
            Event::RimOut | Event::Block { .. } if link.breaking.is_some() => {
                let team = link.breaking.unwrap_or(0);
                gamebreaker_over(&mut link, w, team);
            }
            Event::Pocket { .. } | Event::Cancel { .. } => settle(&mut link, w, event::MAIN_NOW),
            // Slot 41 (a player's state change, `0x1a7370`) checks the
            // groove's age; a change of hands stands in for it here.
            Event::Catch { .. } | Event::Pickup { .. } | Event::Inbound { .. }
                if link.groove && link.past(w, NEXT_IN_GROOVE) =>
            {
                link.post(Command::Stop);
                link.next = true;
            }
            // The game's trick levels run 0..4; its top two get stingers.
            Event::Trick { level, .. } if level >= 4 => {
                let now = time.elapsed_secs_f64();
                if now - link.last_stinger > 2.0 {
                    link.last_stinger = now;
                    link.post(Command::Event(event::STINGER + level - 1));
                }
            }
            Event::GameOver { .. } => {
                link.post(Command::Event(event::END));
                link.over = true;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world_at(seconds: u64) -> World {
        let players = (0..6)
            .map(|k| street_sim::Player::new(k / 3, [0.0, k as f32], 0.0))
            .collect();
        let hoop = |x: f32| street_sim::Hoop {
            rim: [x, 11.75, 0.0],
            rim_radius: 0.75,
        };
        let mut w = World::new(
            [hoop(37.0), hoop(-37.0)],
            players,
            street_sim::Rules::default(),
            1,
        );
        w.tick = seconds * u64::from(street_sim::TICK_HZ);
        w
    }

    fn posted(link: &MusicLink) -> Vec<Command> {
        std::mem::take(&mut link.inbox.lock().unwrap().commands)
    }

    #[test]
    fn a_big_dunk_drops_the_music_until_the_slam() {
        let mut link = MusicLink::new(Arc::default());
        link.opened(0, true);
        let dry_off = |link: &MusicLink| link.inbox.lock().unwrap().dry_off;
        let take_off = |finish| Event::TakeOff {
            player: 0,
            leap: false,
            finish,
            dunk: 0,
        };
        // A finish of 1 keeps the music; 2 drops it until the slam.
        dunk_mix(&mut link, &take_off(1), true, true);
        assert!(!dry_off(&link));
        dunk_mix(&mut link, &take_off(2), true, true);
        assert!(dry_off(&link));
        dunk_mix(&mut link, &Event::Slam { player: 0, hoop: 0 }, true, true);
        assert!(!dry_off(&link));
        // A pass to a flier drops it too; not without interactive music,
        // nor outside the groove.
        let lob = |leaper| Event::Launched {
            from: 0,
            to: 1,
            kick: false,
            leaper,
        };
        dunk_mix(&mut link, &lob(true), false, true);
        assert!(!dry_off(&link));
        dunk_mix(&mut link, &lob(true), true, true);
        assert!(dry_off(&link));
        dunk_mix(&mut link, &lob(false), true, true);
        assert!(!dry_off(&link));
        // The flier's catch keeps it down; a knock restores it in live
        // play only.
        dunk_mix(&mut link, &lob(true), true, true);
        let catch = Event::Catch { player: 1 };
        dunk_mix(&mut link, &catch, true, true);
        assert!(dry_off(&link));
        let knock = Event::Deflection { player: 1 };
        dunk_mix(&mut link, &knock, true, false);
        assert!(dry_off(&link));
        dunk_mix(&mut link, &knock, true, true);
        assert!(!dry_off(&link));
        link.groove = false;
        dunk_mix(&mut link, &take_off(3), true, true);
        assert!(!dry_off(&link));
    }

    #[test]
    fn a_late_gamebreaker_moves_on_to_the_next_song() {
        let mut link = MusicLink::new(Arc::default());
        link.opened(0, true);
        posted(&link);

        // Under two minutes: back to the groove.
        gamebreaker_over(&mut link, &world_at(100), 0);
        assert_eq!(posted(&link), [Command::Stop, Command::Event(event::MAIN)]);
        assert!(!link.next);

        // The other side's full meter keeps its ready theme, however late.
        let mut w = world_at(130);
        w.teams[1].meter = street_sim::street::METER_FULL;
        gamebreaker_over(&mut link, &w, 0);
        assert_eq!(
            posted(&link),
            [Command::Stop, Command::Event(event::READY[0])]
        );
        assert!(!link.next);
        // The scoring side's own meter is not asked (third argument 0).
        gamebreaker_over(&mut link, &w, 1);
        assert_eq!(posted(&link), [Command::Stop]);
        assert!(link.next);
        link.next = false;

        // Past two minutes: the next song. `fps * 120.0` VBlanks is the
        // bound, so 120 s exactly stays and the next tick moves on.
        let mut w = world_at(120);
        gamebreaker_over(&mut link, &w, 0);
        assert!(!link.next);
        posted(&link);
        w.tick += 1;
        gamebreaker_over(&mut link, &w, 0);
        assert_eq!(posted(&link), [Command::Stop]);
        assert!(link.next);

        // The next one counts from when it opened.
        let mut next = MusicLink::new(Arc::default());
        next.opened(130 * u64::from(street_sim::TICK_HZ), true);
        assert_eq!(posted(&next), [Command::Event(event::MAIN)]);
        assert_eq!(next.age(&world_at(200)), 70);
    }

    #[test]
    fn the_groove_starts_when_the_referees_toss_goes_up() {
        let mut link = MusicLink::new(Arc::default());
        let mut w = world_at(0);
        w.start_opening_toss();
        // The referee holds the ball: nothing yet, however long he waits.
        for _ in 1..street_sim::toss::WAIT_UPDATES {
            w.step(&[street_sim::Input::default(); 6]);
            tip_off(&mut link, &w);
        }
        assert_eq!(posted(&link), []);
        // The toss goes up (slot 0x144, 0x1a6f38): event 1, once.
        w.step(&[street_sim::Input::default(); 6]);
        assert_eq!(w.opening_toss, street_sim::OpeningToss::Flight);
        tip_off(&mut link, &w);
        tip_off(&mut link, &w);
        assert_eq!(posted(&link), [Command::Event(event::MAIN)]);
        // Without a toss, the court's name showing starts it.
        let mut plain = MusicLink::new(Arc::default());
        tip_off(&mut plain, &world_at(1));
        assert_eq!(posted(&plain), []);
        tip_off(&mut plain, &world_at(4));
        assert_eq!(posted(&plain), [Command::Event(event::MAIN)]);
    }

    /// Seconds of `source`'s output: the share of frames that aren't silent.
    fn loudness(source: &mut PathSource, seconds: f32) -> f32 {
        let frames = (seconds * source.song.rate as f32) as usize;
        let loud = (0..frames)
            .filter(|_| source.frame().iter().any(|v| v.abs() > 0.01))
            .count();
        loud as f32 / frames as f32
    }

    #[test]
    fn a_song_plays_through_its_map() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let mut iso = Iso::open(path).unwrap();
        let cfg = iso.read(street_audio::music::PATHSONG).unwrap();
        let songs = street_audio::music::songs(&String::from_utf8_lossy(&cfg));
        for song in &songs {
            let (s, link) = load(&mut iso, song, 5).unwrap();
            let mut src = s.decoder();
            // The intro.
            assert!(loudness(&mut src, 3.0) > 0.5, "{}: the intro", song.folder);
            let intro = src.player.playing(0);
            assert!(intro.is_some(), "{}", song.folder);
            // Play starts: the main groove comes after the intro's segment.
            link.post(Command::Event(event::MAIN));
            assert!(
                loudness(&mut src, 40.0) > 0.5,
                "{}: the groove",
                song.folder
            );
            assert_ne!(src.player.playing(0), intro, "{}: moved on", song.folder);
            // A gamebreaker's ready theme cuts in on the beat, a stinger on
            // TRICKS plays over it.
            link.post(Command::Event(event::READY[0]));
            link.post(Command::Event(event::STINGER + 3));
            assert!(loudness(&mut src, 2.0) > 0.5, "{}", song.folder);
            assert!(
                src.player.playing(2).is_some(),
                "{}: the stinger",
                song.folder
            );
            // Back to the groove, then the end.
            link.post(Command::Stop);
            link.post(Command::Event(event::MAIN_NOW));
            assert!(
                loudness(&mut src, 5.0) > 0.5,
                "{}: the groove again",
                song.folder
            );
            link.post(Command::Event(event::END));
            let _ = loudness(&mut src, 40.0);
            assert_eq!(
                src.player.playing(0),
                None,
                "{}: the outro ends the song",
                song.folder
            );
        }
    }

    #[test]
    fn resampling_keeps_the_length_in_time() {
        let mono: Vec<i16> = (0..22050).map(|k| (k % 100) as i16).collect();
        let out = to_stereo(&mono, 1, 22050, 44100);
        assert_eq!(out.len(), 44100 * 2);
        assert_eq!(out[0], out[1], "mono goes to both sides");
    }
}
