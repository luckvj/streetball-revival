//! The match's sound effects and crowd, played the way the game plays them:
//! through its AEMS scripts (`street_audio::aems`).
//!
//! The game fires events from an audio listener (vtable `0x45c3a0`) with a
//! position (turned into an angle and a distance from the camera,
//! `0x19ebe0`) and a parameter or two; the scripts in the court's banks pick
//! the sound and steer it. [`fire_events`] is that listener, for the
//! simulation's events; [`Effects`] runs the scripts every 10 ms and owns
//! the voices, which a single audio source ([`SfxBus`]) mixes.
//!
//! | Moment | Event (listener slot) |
//! |---|---|
//! | the court comes alive at the match start | 0x28, then the crowd loop 0x20 |
//! | play starts | sweetener 0x4a kind 14 (slot 40) |
//! | a dribble, a loose ball on the floor | 0x44 (slots 44, 30) |
//! | a catch / a rebound | 0x38 (slot 21) / 0x3a (slot 24) |
//! | a pass | nothing: the pass class `0x3c6500` dispatches to slot 9, an empty handler (`0x142ff8`) |
//! | a jump shot / a lay-up launched (`0x14aaf8` posts slot 11 with the ball's kind 2 / 5) | the long-flight whoosh 0x3f / sweetener 0x4a 6 |
//! | a shot or dunk leaving the hands | exertion 0x50 every other time, sweetener 0x4a 10 or 13 (slot 4) |
//! | the ball on the rim | 0x43 at the hoop (slot 42); a dunk's rattle 0x36 (slot 75) |
//! | the net | 0x45 at the hoop (slot 43), 0x3e (slots 26, 37) |
//! | a dunk's slam | 0x40 (slot 39), the crowd's 0x48 by the dunk's power (slot 26) |
//! | a block (slot 16, `0x1a4ce8`) | 0x39 (whether the blocker holds the ball), crowd 0x27, exertion; the ball contact 0x38 is the rewrite's own |
//! | a steal or counter (slot 15, `0x1a63e0`) | crowd 0x27, 0x49; the ball contact 0x3a and the grunt are the rewrite's own |
//! | the tip-off caught from the referee's toss (slot 22, `0x1a5d10`: ball state 7) | crowd 0x25 |
//! | a block jump's take-off | exertion every other time, counted with the shots' (slot 6); the swing, sweetener 0x4a 11 (slot 7) |
//! | a reach for the ball: a swipe, or the counter stance | sweetener 0x4a 12, every other time an exertion (slot 8) |
//! | a dunk let go at the rim | exertion (slot 12), HUD 0x4d kind 4 (slot 13, shared with trick lands) |
//! | a pick or a trick counter knocks a player off balance | body 0x4f 0 at the one knocked (slot 74) |
//! | a faked-out defender is knocked down | body 0x4f at the one knocked (slot 74): kind 0 a stagger, kind 1 the fall (`FUN_001a5288`'s knock level − 1; a fall adds sweetener 5) |
//! | an ordinary player's animated joint 10 crosses the floor threshold | body 0x4f kind 2 by the joint's drop, crowd 0x29, exertion (slot 76); scripted presentation suppresses the observer |
//! | a trick of level 1..3 | sweetener 0x4a kind 7 + level (slot 2); HUD 0x4d kind 4, crowd 0x29 and a taunt (slot 13, shared with dunk releases) |
//! | a meter fills | HUD 0x4d kind 2 (12 at level 2), the loop 0x22 (slot 66) |
//! | a gamebreaker goes up | transition 0x4c, the loop 0x21 (slot 64) |
//! | a meter is pocketed / cancelled | HUD 0x4d kind 16 / crowd 0x3d; the loop 0x22 stops (slots 70, 71) |
//! | a big trick bonus | HUD 0x4d kind 6 (over 5,000) or 7 (over 12,000) (slot 68) |
//! | the shot clock's last seconds | HUD 0x4d kind 10 (slot 48), the violation kind 11 (slot 52) |
//! | a basket | HUD 0x4d kind 9 (slot 26) |
//! | an instant replay | transition 0x4c, the loop 0x23 (slots 58, 59) |
//!
//! The players talk (event 0x1f, a line of their voice by category, posted
//! through the speech manager, `audio::SpeechManager`): a trick (3), a
//! block (9), a steal (10), a jump shot's launch (14), a rebound pickup (0,
//! or every other time 1 from a teammate), a teammate's dunk (5), the shot
//! clock at 5 seconds (2) and at 21 (7, the defence, half the time), a
//! violation (4 and 8 in turn). Each raw `jal 0x1ad068` site names its
//! masks; none has a caller-side chance except the 21-second call's
//! `rand() & 1`.
//!
//! Panning: an angle of 0 is straight ahead and grows to the left.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use bevy::{
    audio::{
        AddAudioSource, AudioPlayer, ChannelCount, Decodable, PlaybackSettings, SampleRate, Source,
        Volume,
    },
    prelude::*,
};
use street_audio::{
    aems::{Engine, EventMap, Mixer, Program, Sound, TICK_MS, VoiceParams},
    bank::Bank,
    courts, schl, speech,
};
use street_sim::{BallState, Basket, Event, Phase};

use crate::{
    Screen,
    audio::AudioSettings,
    game::{Paused, Replay, Sim, TickEvents},
    setup::{Disc, MatchSetup},
    story,
    view::SidelineCamera,
};

/// The output rate of the mix.
const RATE: u32 = 48_000;
/// Frames rendered per lock of the bus.
const BLOCK: usize = 256;

/// The game's default mix (`0x1a2340`), percent of the effects volume, by
/// AEMS global: crowd (bank, stream), player speech, crowd reactions,
/// effects, gamebreaker, environment.
const GLOBALS: [i32; 7] = [54, 40, 88, 90, 87, 86, 60];

/// The front end's: global 1 is its effects, 69 (`0x19df08`).
const FE_GLOBALS: [i32; 2] = [0, 69];

/// Loop 0x24's message (`FUN_0019ee90` mode 0 stores it at `0x4baf6c`):
/// `audio_aems_sfx_ducking` 38, `audio_aems_crowd_ducking` 40 and
/// `audio_aems_plyr_spch_ducking` 25 (`0x3e99b0`, `0x3e99d0`, `0x3e99f0`,
/// registered at `0x1a25fc`, `0x1a2638`, `0x1a265c`).
pub const COMMENTATOR_DUCK_MESSAGE: [i32; 4] = [ev::COMMENTATOR_DUCK, 38, 40, 25];

/// The events, by number.
pub(crate) mod ev {
    /// Sneaker squeaks (slot 77): the shared one and the court's own.
    pub const SQUEAK: i32 = 0x3c;
    pub const COURT_STEP: i32 = 0x46;
    /// A player's own line: `[angle, distance, stream]` (`0x1ac9f8`).
    pub const SPEECH: i32 = 0x1f;
    pub const CROWD_LOOP: i32 = 0x20;
    pub const GAMEBREAKER_LOOP: i32 = 0x21;
    pub const READY_LOOP: i32 = 0x22;
    pub const REPLAY_LOOP: i32 = 0x23;
    /// The commentator's duck (`0x1ac9f8` starts it as a line plays,
    /// `0x1acf58` releases it as the stream ends): the crowd banks' mixer
    /// (module 62793 of `CRD_*.ABK`, nodes 0x9f551/0x9f552/0xaf535) and the
    /// courts' ambience (module 14922) take its three percents.
    pub const COMMENTATOR_DUCK: i32 = 0x24;
    /// The crowd's calls through `FUN_001a75a8` (`[event, leads]`): 0x25
    /// a cheer (the tip-off, a dunk's slam), 0x27 a cheer for a block, a
    /// steal or a rebound, 0x29 a gasp (a fall, a trick landing).
    pub const TIP_CROWD: i32 = 0x25;
    pub const CROWD_CALL: i32 = 0x27;
    pub const COURT: i32 = 0x28;
    pub const FALL_CROWD: i32 = 0x29;
    pub const RIM_RATTLE: i32 = 0x36;
    /// A hand on the ball: a catch (slot 21), a swat.
    pub const HIT: i32 = 0x38;
    /// A block (slot 16, `0x1a4ce8`).
    pub const BLOCK: i32 = 0x39;
    /// A rebound (slot 24).
    pub const REBOUND: i32 = 0x3a;
    pub const CROWD: i32 = 0x3d;
    pub const NET: i32 = 0x3e;
    /// A jump shot's long flight (slot 11, `0x1a4738`).
    pub const LOB: i32 = 0x3f;
    pub const SLAM: i32 = 0x40;
    pub const CROWD_BIG: i32 = 0x41;
    pub const RIM: i32 = 0x43;
    pub const FLOOR: i32 = 0x44;
    pub const SWISH: i32 = 0x45;
    pub const DUNK_CROWD: i32 = 0x48;
    /// A steal (slot 15, `0x1a63e0`).
    pub const STEAL: i32 = 0x49;
    pub const SWEETENER: i32 = 0x4a;
    /// A body against a body (slot 74) or the floor (slot 76): `[angle,
    /// distance, intensity, kind]`.
    pub const BODY: i32 = 0x4f;
    pub const TRANSITION: i32 = 0x4c;
    pub const HUD: i32 = 0x4d;
    pub const EXERTION: i32 = 0x50;
}

// ---------------------------------------------------------------------------
// The mix

/// What a voice plays.
enum Src {
    /// A bank sound, interleaved, with its loop in frames.
    Pcm {
        data: Arc<[f32]>,
        channels: usize,
        looping: Option<(usize, usize)>,
    },
    /// A crowd stream, decoded as it plays.
    Stream {
        reader: schl::Reader,
        channels: usize,
        #[cfg(test)]
        bank: usize,
        #[cfg(test)]
        offset: u32,
    },
}

enum Cmd {
    Start {
        id: u32,
        src: Src,
        rate: u32,
        params: VoiceParams,
    },
    Update(u32, VoiceParams),
    Stop(u32),
    StopAll,
}

/// The mailbox between the scripts and the audio thread.
#[derive(Default)]
struct Bus {
    commands: Vec<Cmd>,
    /// Voice, milliseconds played.
    progress: Vec<(u32, u32)>,
    finished: Vec<u32>,
}

/// The match's effects mix, as an audio source.
#[derive(Asset, TypePath, Clone)]
pub struct SfxBus(Arc<Mutex<Bus>>);

struct Voice {
    id: u32,
    src: Src,
    /// Source frames per output frame.
    step: f64,
    /// Position in source frames; for streams, the fraction past `cur`.
    pos: f64,
    prev: [f32; 2],
    cur: [f32; 2],
    gains: [f32; 2],
    rate: u32,
    pitch: f32,
    played: u64,
    done: bool,
}

impl Voice {
    fn new(id: u32, src: Src, rate: u32, params: VoiceParams) -> Self {
        let mut v = Voice {
            id,
            src,
            step: 0.0,
            pos: 0.0,
            prev: [0.0; 2],
            cur: [0.0; 2],
            gains: [0.0; 2],
            rate: rate.max(1),
            pitch: 1.0,
            played: 0,
            done: false,
        };
        v.set(params);
        if let Src::Stream { .. } = v.src {
            v.cur = v.read_stream().unwrap_or([0.0; 2]);
        }
        v
    }

    fn set(&mut self, p: VoiceParams) {
        self.pitch = p.rate().clamp(0.05, 8.0);
        self.step = f64::from(self.rate) * f64::from(self.pitch) / f64::from(RATE);
        // Equal power, full on both sides at the centre.
        let g = p.gain();
        let a = (p.pan().clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
        let sqrt2 = std::f32::consts::SQRT_2;
        self.gains = [
            g * (a.cos() * sqrt2).min(1.0),
            g * (a.sin() * sqrt2).min(1.0),
        ];
    }

    fn read_stream(&mut self) -> Option<[f32; 2]> {
        let Src::Stream {
            reader, channels, ..
        } = &mut self.src
        else {
            return None;
        };
        let l = f32::from(reader.next()?) / 32768.0;
        let r = if *channels >= 2 {
            reader.next().map_or(l, |v| f32::from(v) / 32768.0)
        } else {
            l
        };
        for _ in 2..*channels {
            reader.next();
        }
        Some([l, r])
    }

    /// The next output frame, before gains.
    fn frame(&mut self) -> [f32; 2] {
        self.played += 1;
        match &self.src {
            Src::Pcm {
                data,
                channels,
                looping,
            } => {
                let ch = *channels;
                let frames = data.len() / ch.max(1);
                let mut i = self.pos as usize;
                if i + 1 >= frames {
                    if let Some((start, end)) = *looping
                        && end > start
                    {
                        self.pos -= (end.min(frames) - start) as f64;
                        i = self.pos.max(0.0) as usize;
                    } else {
                        self.done = true;
                        return [0.0; 2];
                    }
                }
                let t = (self.pos - i as f64) as f32;
                let at = |k: usize, c: usize| data[(k.min(frames - 1)) * ch + c.min(ch - 1)];
                let out = [
                    at(i, 0) + (at(i + 1, 0) - at(i, 0)) * t,
                    at(i, 1) + (at(i + 1, 1) - at(i, 1)) * t,
                ];
                self.pos += self.step;
                out
            }
            Src::Stream { .. } => {
                self.pos += self.step;
                while self.pos >= 1.0 {
                    self.pos -= 1.0;
                    self.prev = self.cur;
                    match self.read_stream() {
                        Some(f) => self.cur = f,
                        None => {
                            self.done = true;
                            return [0.0; 2];
                        }
                    }
                }
                let t = self.pos as f32;
                [
                    self.prev[0] + (self.cur[0] - self.prev[0]) * t,
                    self.prev[1] + (self.cur[1] - self.prev[1]) * t,
                ]
            }
        }
    }
}

pub struct SfxSource {
    bus: Arc<Mutex<Bus>>,
    voices: Vec<Voice>,
    block: Vec<f32>,
    at: usize,
}

impl SfxSource {
    fn render(&mut self) {
        if let Ok(mut bus) = self.bus.lock() {
            for cmd in bus.commands.drain(..) {
                match cmd {
                    Cmd::Start {
                        id,
                        src,
                        rate,
                        params,
                    } => self.voices.push(Voice::new(id, src, rate, params)),
                    Cmd::Update(id, p) => {
                        if let Some(v) = self.voices.iter_mut().find(|v| v.id == id) {
                            v.set(p);
                        }
                    }
                    Cmd::Stop(id) => self.voices.retain(|v| v.id != id),
                    Cmd::StopAll => self.voices.clear(),
                }
            }
            bus.progress.clear();
            for v in &self.voices {
                let ms = v.played * 1000 / u64::from(RATE);
                bus.progress
                    .push((v.id, ms.min(u64::from(u32::MAX)) as u32));
            }
        }
        self.block.clear();
        self.block.resize(BLOCK * 2, 0.0);
        for v in &mut self.voices {
            for f in 0..BLOCK {
                if v.done {
                    break;
                }
                let [l, r] = v.frame();
                self.block[2 * f] += l * v.gains[0];
                self.block[2 * f + 1] += r * v.gains[1];
            }
        }
        for s in &mut self.block {
            *s = s.clamp(-1.0, 1.0);
        }
        if self.voices.iter().any(|v| v.done) {
            let done: Vec<u32> = self
                .voices
                .iter()
                .filter(|v| v.done)
                .map(|v| v.id)
                .collect();
            self.voices.retain(|v| !v.done);
            if let Ok(mut bus) = self.bus.lock() {
                bus.finished.extend(done);
            }
        }
        self.at = 0;
    }
}

impl Iterator for SfxSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.at >= self.block.len() {
            self.render();
            for f in self.block.as_chunks_mut::<2>().0 {
                *f = crate::audio::mono(f[0], f[1]);
            }
        }
        let s = self.block[self.at];
        self.at += 1;
        Some(s)
    }
}

impl Source for SfxSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).expect("non-zero")
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(RATE).expect("non-zero")
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Decodable for SfxBus {
    type Decoder = SfxSource;

    fn decoder(&self) -> SfxSource {
        SfxSource {
            bus: self.0.clone(),
            voices: Vec::new(),
            block: Vec::new(),
            at: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// The scripts

/// A loaded bank's sounds, by engine slot.
struct BankAudio {
    bank: Bank,
    /// The `.AST` the bank streams from.
    ast: Option<Arc<[u8]>>,
    clips: HashMap<u16, (Arc<[f32]>, usize, u32, Option<(usize, usize)>)>,
}

/// The voices as the scripts see them.
struct Link<'a> {
    banks: &'a mut Vec<Option<BankAudio>>,
    bus: &'a Mutex<Bus>,
    /// Voice, (ms played, ms left).
    alive: &'a mut HashMap<u32, (u32, u32)>,
    next: &'a mut u32,
}

impl Mixer for Link<'_> {
    fn start(&mut self, bank: usize, sound: Sound, params: VoiceParams) -> Option<u32> {
        let b = self.banks.get_mut(bank)?.as_mut()?;
        let (src, rate, length) = match sound {
            Sound::Bank(i) => {
                if !b.clips.contains_key(&i) {
                    let clip = b.bank.clip(usize::from(i)).ok()?;
                    let data: Arc<[f32]> = clip
                        .samples
                        .iter()
                        .map(|&s| f32::from(s) / 32768.0)
                        .collect();
                    let looping = clip.looping.map(|(a, e)| (a as usize, e as usize));
                    b.clips.insert(
                        i,
                        (data, usize::from(clip.channels.max(1)), clip.rate, looping),
                    );
                }
                let (data, channels, rate, looping) = b.clips.get(&i)?.clone();
                let ms = (data.len() / channels) as u64 * 1000 / u64::from(rate.max(1));
                let length = if looping.is_some() {
                    u32::MAX
                } else {
                    ms as u32
                };
                (
                    Src::Pcm {
                        data,
                        channels,
                        looping,
                    },
                    rate,
                    length,
                )
            }
            Sound::Stream { offset, looping } => {
                let ast = b.ast.clone()?;
                let stream = schl::Stream::at(&ast, offset as usize).ok()?;
                let (rate, channels) =
                    (stream.patch.rate, usize::from(stream.patch.channels.max(1)));
                let reader = schl::Reader::new(ast, stream, looping);
                (
                    Src::Stream {
                        reader,
                        channels,
                        #[cfg(test)]
                        bank,
                        #[cfg(test)]
                        offset,
                    },
                    rate,
                    u32::MAX,
                )
            }
        };
        *self.next = self.next.wrapping_add(1).max(1);
        let id = *self.next;
        self.bus.lock().ok()?.commands.push(Cmd::Start {
            id,
            src,
            rate,
            params,
        });
        self.alive.insert(id, (0, length));
        Some(id)
    }

    fn update(&mut self, voice: u32, params: VoiceParams) {
        if let Ok(mut bus) = self.bus.lock() {
            bus.commands.push(Cmd::Update(voice, params));
        }
    }

    fn stop(&mut self, voice: u32) {
        self.alive.remove(&voice);
        if let Ok(mut bus) = self.bus.lock() {
            bus.commands.push(Cmd::Stop(voice));
        }
    }

    fn position(&mut self, voice: u32) -> Option<(u32, u32)> {
        self.alive.get(&voice).copied()
    }
}

/// The match's AEMS runtime and its voices.
#[derive(Resource)]
pub struct Effects {
    engine: Engine,
    banks: Vec<Option<BankAudio>>,
    bus: Arc<Mutex<Bus>>,
    alive: HashMap<u32, (u32, u32)>,
    next: u32,
    /// Milliseconds of real time not yet run.
    pending: f64,
    /// Handles of the looping events (crowd, gamebreaker, ready, replay).
    loops: HashMap<i32, i32>,
    /// What the listener remembers between ticks.
    replaying: bool,
    /// Play has started (the intro is over).
    started: bool,
    /// `CROWD_LOOP` was started but has not yet received an AEMS tick.
    /// The intro normally separates its start and release; a skipped intro
    /// can reach both callbacks in one frame.
    court_start_pending: bool,
    dribble: f32,
    ball_vy: f32,
    shot_clock: i32,
    /// Each player's grunt and shoe voice (`0x1a8288`).
    pub(crate) voices: Vec<i32>,
    /// Each player's speaking voice (`0x1acb68`).
    speakers: Vec<Option<u16>>,
    /// Milliseconds of play run so far.
    clock: f64,
    /// The rewrite's stand-in for the game's `rand()` where a listener
    /// handler draws from it.
    rng: u32,
    /// The tip-off's catch has been heard (slot 22).
    tipped: bool,
    /// Alternations the listener keeps: [1] the violations' line
    /// (`DAT_003ea248`, which the PS2 keeps for the session; the rewrite per
    /// match). [0] is unused since the rebound's turn moved to the speech
    /// manager.
    turns: [u32; 2],
    /// The listener's grunt count (+0x70), shared by the shots (slot 4,
    /// `0x1a4928`) and the block take-offs (slot 6, `0x1a4c58`): they grunt
    /// on its odd values.
    exertions: u32,
    /// The reaches' own count (`0x4bb2d0`, slot 8 `0x1a60f0`).
    reaches: u32,
    /// Effects level the globals were set for, and the percents they take.
    level: u8,
    percents: &'static [i32],
    /// The staged gamebreaker whose sound commands have fired, and up to
    /// which frame.
    staged: Option<(usize, u32)>,
}

impl Effects {
    fn link(&mut self) -> (&mut Engine, Link<'_>) {
        (
            &mut self.engine,
            Link {
                banks: &mut self.banks,
                bus: &self.bus,
                alive: &mut self.alive,
                next: &mut self.next,
            },
        )
    }

    /// Fires event `msg[0]` with `msg[1..]`.
    pub fn fire(&mut self, msg: &[i32]) -> Option<i32> {
        let (engine, mut link) = self.link();
        engine.fire(msg, &mut link)
    }

    /// Staged AEMS commands pass through the same parameter routing as
    /// FUN_0019ed30. Their opcode alone is not a ball or sound category.
    fn staged_sound(&mut self, sound: street_assets::timeline::Sound, angle: i32, distance: i32) {
        let event = i32::from(sound.event);
        let (level, parameter, extra) = (sound.intensity, sound.parameter, sound.extra);
        let message = match event {
            0x21 => {
                self.start_loop(&[event, parameter]);
                return;
            }
            0x25 | 0x26 | 0x29 | 0x35 => vec![event],
            0x27 | 0x3d | 0x41 | 0x4c | 0x51 => vec![event, parameter],
            0x28 => vec![event, level],
            0x2a | 0x32 => vec![event, angle],
            0x2b | 0x2f | 0x30 | 0x31 | 0x34 | 0x3a | 0x3e | 0x3f => vec![event, angle, distance],
            0x2c..=0x2e => vec![event, angle, parameter, extra],
            0x1f | 0x36 | 0x38 | 0x44 | 0x45 | 0x4e | 0x50 => vec![
                event,
                angle,
                distance,
                if matches!(event, 0x1f | 0x50) {
                    parameter
                } else {
                    level
                },
            ],
            0x33 => vec![event, angle, distance, parameter, extra],
            0x39 | 0x3c | 0x46 | 0x49 | 0x4a | 0x4f => {
                vec![event, angle, distance, level, parameter]
            }
            0x3b => vec![event, angle, distance, level.saturating_mul(4).min(1000)],
            0x40 => vec![event, angle, level, parameter],
            0x42 | 0x43 | 0x48 => vec![event, angle, level],
            0x47 => vec![event, angle, parameter],
            0x4d => vec![event, parameter, extra],
            _ => return,
        };
        self.fire(&message);
    }

    /// Starts a looping event (`0x19ee90`, mode 0); restarting replaces it.
    fn start_loop(&mut self, msg: &[i32]) {
        self.stop_loop(msg[0]);
        if let Some(h) = self.fire(msg) {
            self.loops.insert(msg[0], h);
        }
    }

    /// `FUN_0019ee90(aems, 0, 0x24)`: the commentator's duck. Mode 0 keeps
    /// the new handle over a running one without releasing it (its modules
    /// belong to other events, so a handle holds nothing of its own).
    pub fn duck_for_commentary(&mut self) {
        if let Some(h) = self.fire(&COMMENTATOR_DUCK_MESSAGE) {
            self.loops.insert(ev::COMMENTATOR_DUCK, h);
        }
    }

    /// `FUN_0019ee90(aems, 1, 0x24)`: released with the message it was
    /// started with, which writes the percents again before the release
    /// triggers.
    pub fn commentary_over(&mut self) {
        if let Some(h) = self.loops.remove(&ev::COMMENTATOR_DUCK) {
            let (engine, mut link) = self.link();
            engine.release(h, &COMMENTATOR_DUCK_MESSAGE, &mut link);
        }
    }

    /// Whether the commentator's duck is held.
    pub fn ducked_for_commentary(&self) -> bool {
        self.loops.contains_key(&ev::COMMENTATOR_DUCK)
    }

    /// Releases a looping event (`0x19ee90`, mode 1).
    fn stop_loop(&mut self, event: i32) {
        if let Some(h) = self.loops.remove(&event) {
            let (engine, mut link) = self.link();
            engine.release(h, &[event], &mut link);
        }
    }

    fn set_volumes(&mut self, settings: &AudioSettings) {
        self.level = settings.effects;
        // The game's volume option is 0..0x7fff; ours follow the same
        // curve as the rest of the mix.
        let option = (32767.0 * crate::audio::gain(settings.effects)) as i32;
        let speech = (32767.0 * crate::audio::gain(settings.speech)) as i32;
        for (g, &pct) in self.percents.iter().enumerate() {
            let base = if g == 2 && self.percents.len() > 2 {
                speech
            } else {
                option
            };
            self.engine.set_global(g, base * pct / 100);
        }
    }

    /// Runs the scripts for `ms` of time.
    fn run(&mut self, ms: f64) {
        self.clock += ms;
        self.pending = (self.pending + ms).min(f64::from(TICK_MS) * 10.0);
        // What the audio thread has played and finished.
        if let Ok(mut bus) = self.bus.lock() {
            for id in bus.finished.drain(..) {
                self.alive.remove(&id);
            }
            for &(id, played) in &bus.progress {
                if let Some(a) = self.alive.get_mut(&id) {
                    let total = a.0.saturating_add(a.1);
                    *a = (played, total.saturating_sub(played));
                }
            }
        }
        while self.pending >= f64::from(TICK_MS) {
            self.pending -= f64::from(TICK_MS);
            let (engine, mut link) = self.link();
            engine.tick(&mut link);
            self.court_start_pending = false;
        }
    }
}

// ---------------------------------------------------------------------------
// Loading and running

pub struct SfxPlugin;

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<SfxBus>()
            .add_systems(Startup, load_menu_sfx.after(crate::audio::load_front_end))
            .add_systems(OnEnter(Screen::Menu), start_menu_sfx)
            // The load back to the front end is a front-end screen: its
            // transition's sound plays there.
            .add_systems(OnEnter(Screen::Returning), start_return_sfx)
            .add_systems(
                Update,
                run_menu_sfx.run_if(|s: Res<State<Screen>>| {
                    matches!(s.get(), Screen::Menu | Screen::Returning)
                }),
            )
            .add_systems(OnEnter(Screen::Playing), start_match)
            .add_systems(OnExit(Screen::Playing), end_match)
            .add_systems(
                FixedUpdate,
                fire_events
                    .after(crate::game::SimStep)
                    .run_if(in_state(Screen::Playing)),
            )
            .add_systems(
                PostUpdate,
                floor_contacts
                    .after(TransformSystems::Propagate)
                    .run_if(in_state(Screen::Playing)),
            )
            .add_systems(Update, run_scripts.run_if(in_state(Screen::Playing)));
    }
}

impl Effects {
    /// Loads a match's banks and their scripts (`0x19e160`): the six shared
    /// banks, the court's ambience, and its crowd with the crowd's stream.
    /// `voices` is each player's grunt voice.
    pub fn load(
        iso: &mut street_assets::iso::Iso<std::io::BufReader<std::fs::File>>,
        court: Option<usize>,
        seed: u32,
        voices: Vec<i32>,
        speakers: Vec<Option<u16>>,
        settings: &AudioSettings,
    ) -> Option<(Effects, SfxBus)> {
        let (ambience, crowd, crowd_ast) = courts::banks(court);
        let mut files: Vec<(&str, Option<&str>)> =
            courts::COMMON.iter().map(|&f| (f, None)).collect();
        files.push((ambience, None));
        files.push((crowd, Some(crowd_ast)));
        files.push((speech::PLAYER_BANK, Some(speech::PLAYER_STREAMS)));
        let (mut fx, bus) =
            Effects::with_banks(iso, "/SFX/GAME_EVT.AEM", &files, seed, settings, &GLOBALS)?;
        fx.voices = voices;
        fx.speakers = speakers;
        Some((fx, bus))
    }

    /// The front end's scripts (`0x19df08`): `FE_EVT.AEM` over `FE_MOD.ABK`.
    /// Event 1 plays menu sound n with its own level, event 2 the other set.
    pub fn front_end(
        iso: &mut street_assets::iso::Iso<std::io::BufReader<std::fs::File>>,
        settings: &AudioSettings,
    ) -> Option<(Effects, SfxBus)> {
        Effects::with_banks(
            iso,
            "/SFX/FE_EVT.AEM",
            &[(courts::FRONT_END, None)],
            1,
            settings,
            &FE_GLOBALS,
        )
    }

    /// An event map and its banks, with the globals at `percents`.
    fn with_banks(
        iso: &mut street_assets::iso::Iso<std::io::BufReader<std::fs::File>>,
        map_path: &str,
        files: &[(&str, Option<&str>)],
        seed: u32,
        settings: &AudioSettings,
        percents: &'static [i32],
    ) -> Option<(Effects, SfxBus)> {
        let map = match iso.read(map_path).map(|d| EventMap::parse(&d)) {
            Ok(Ok(m)) => m,
            Ok(Err(e)) | Err(e) => {
                warn!("{map_path}: {e}");
                return None;
            }
        };
        let mut engine = Engine::new(map, seed);
        let mut banks = Vec::new();
        for &(path, ast) in files {
            let data = match iso.read(path) {
                Ok(d) => d,
                Err(e) => {
                    warn!("{path}: {e}");
                    continue;
                }
            };
            let (program, bank) = match (Program::parse(&data), Bank::parse(data)) {
                (Ok(p), Ok(b)) => (p, b),
                (Err(e), _) | (_, Err(e)) => {
                    warn!("{path}: {e}");
                    continue;
                }
            };
            let ast = ast
                .and_then(|a| iso.read(a).map_err(|e| warn!("{a}: {e}")).ok())
                .map(Arc::from);
            let slot = engine.load(program);
            if banks.len() <= slot {
                banks.resize_with(slot + 1, || None);
            }
            banks[slot] = Some(BankAudio {
                bank,
                ast,
                clips: HashMap::new(),
            });
        }
        let bus = Arc::new(Mutex::new(Bus::default()));
        let mut fx = Effects {
            engine,
            banks,
            bus: bus.clone(),
            alive: HashMap::new(),
            next: 0,
            pending: 0.0,
            loops: HashMap::new(),
            replaying: false,
            started: false,
            court_start_pending: false,
            dribble: 0.0,
            ball_vy: 0.0,
            shot_clock: i32::MAX,
            voices: Vec::new(),
            speakers: Vec::new(),
            clock: 0.0,
            rng: seed | 1,
            tipped: false,
            turns: [0; 2],
            exertions: 0,
            reaches: 0,
            level: settings.effects,
            percents,
            staged: None,
        };
        fx.set_volumes(settings);
        Some((fx, SfxBus(bus)))
    }

    /// The court comes alive (event 0x28 from the match intro), its crowd
    /// held down while the court is introduced (0x20, `0x1a1110`).
    pub fn open_court(&mut self) {
        self.fire(&[ev::COURT, 500]);
        self.start_loop(&[ev::CROWD_LOOP]);
        self.court_start_pending = true;
    }

    /// Play starts: the crowd comes up (0x20 released, slot 61) and the
    /// opening sweetener (slot 40).
    pub fn play_starts(&mut self) {
        // The native intro gives the AEMS graph a tick between the crowd
        // loop's start and release. A no-intro match can call this before
        // `run_scripts` has ticked the graph, so preserve that ordering.
        if self.court_start_pending {
            self.run(f64::from(TICK_MS) - self.pending);
        }
        self.stop_loop(ev::CROWD_LOOP);
        self.fire(&[ev::SWEETENER, 0, 0, 1000, 14]);
        self.started = true;
    }

    fn roll(&mut self) -> u32 {
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    /// `player` asks to say a line of `category` (`0x1ad068`, which asks
    /// with `1 << category` and the category's metadata mask) from where
    /// they stand. A staged script holds it back (the Script Manager's states
    /// 3..7, the same gate as the floor-contact poster).
    pub fn say(
        &self,
        speech: Option<&mut crate::audio::SpeechManager>,
        player: usize,
        category: u8,
        at: (i32, i32),
    ) {
        let Some(category_mask) = 1u32.checked_shl(u32::from(category)) else {
            return;
        };
        self.say_masks(
            speech,
            player,
            [category_mask, 0, request_mask(category)],
            at,
        );
    }

    /// Slot 11 (`0x1a4738`) for a launch of ball kind 2, an ordinary jump
    /// shot (`0x14aaf8`; a lay-up is kind 5): category 14, shot mask 1,
    /// action mask 2.
    fn say_launch(
        &self,
        speech: Option<&mut crate::audio::SpeechManager>,
        player: usize,
        at: (i32, i32),
    ) {
        self.say_masks(speech, player, [1 << 14, 1, 2], at);
    }

    pub(crate) fn say_masks(
        &self,
        speech: Option<&mut crate::audio::SpeechManager>,
        player: usize,
        masks: [u32; 3],
        at: (i32, i32),
    ) {
        if self.staged.is_some() {
            return;
        }
        if let Some(speech) = speech {
            let voice = self.speakers.get(player).copied().flatten();
            speech.request_player(player, voice, at, masks);
        }
    }
}

/// The native speech request's mask over a header line's second metadata
/// byte, per category: the `a2` argument at every raw `jal 0x1ad068` site.
/// The single-mood categories ask with `1`; the trick, a teammate's dunk and
/// the launches (`0x1a5ba4`, `0x1a41cc`, `0x1a4594`/`0x1a48a4`) with `2`,
/// the block and the steal (`0x1a4e0c`, `0x1a651c`) with `4`. Categories
/// 11..13 have no traced caller.
pub(crate) fn request_mask(category: u8) -> u32 {
    match category {
        3 | 5 | 14 => 2,
        9 | 10 => 4,
        0 | 1 | 2 | 4 | 6 | 7 | 8 => 1,
        _ => 0,
    }
}

/// Starts the match's effects: its banks, the mix, the court.
fn start_match(
    mut commands: Commands,
    mut disc: ResMut<Disc>,
    mut buses: ResMut<Assets<SfxBus>>,
    setup: Res<MatchSetup>,
    running: Res<story::Running>,
    settings: Res<AudioSettings>,
) {
    let school = matches!(running.0, Some(story::Run::School(_)));
    let court = disc
        .courts
        .iter()
        .position(|c| c.id.eq_ignore_ascii_case(&setup.court));
    // Each player's grunts: 2 for women, 1 over 200 lb, else 0 (0x1a8288).
    let voices = setup
        .lineups
        .iter()
        .flatten()
        .map(|&r| {
            disc.players.get(r).map_or(0, |p| {
                if p.female {
                    2
                } else {
                    i32::from(p.weight_lb > 200)
                }
            })
        })
        .collect();
    // Each player's speaking voice (`0x1acb68`), no two alike.
    let mut taken = Vec::new();
    let speakers = setup
        .lineups
        .iter()
        .flatten()
        .map(|&r| {
            let (pick, voice) = speech::player_voice(&disc.players.get(r)?.fields, &taken)?;
            taken.push(pick);
            Some(voice)
        })
        .collect();
    let Some((mut fx, bus)) = Effects::load(
        &mut disc.iso,
        if school { None } else { court },
        setup.seed,
        voices,
        speakers,
        &settings,
    ) else {
        return;
    };
    fx.open_court();
    commands.spawn((
        AudioPlayer(buses.add(bus)),
        PlaybackSettings::ONCE.with_volume(Volume::Linear(1.0)),
        crate::audio::Part::Scripted,
        DespawnOnExit(Screen::Playing),
    ));
    commands.insert_resource(fx);
    commands.insert_resource(FloorContacts::default());
}

/// The front end's scripts, for the menus' sounds.
#[derive(Resource)]
pub struct MenuSfx {
    fx: Effects,
    bus: SfxBus,
}

impl MenuSfx {
    /// Menu sound `index` (`0x19e750`: event 1 with the index).
    pub fn play(&mut self, index: usize) {
        self.fx.fire(&[1, index as i32]);
    }

    /// A screen transition's sound `k` (`FUN_0019e7c8`: event 2 with it,
    /// [`crate::transition::whoosh`]).
    pub fn transition(&mut self, k: i32) {
        self.fx.fire(&[2, k]);
    }
}

fn load_menu_sfx(mut commands: Commands, mut disc: ResMut<Disc>, settings: Res<AudioSettings>) {
    if let Some((fx, bus)) = Effects::front_end(&mut disc.iso, &settings) {
        commands.insert_resource(MenuSfx { fx, bus });
    }
}

fn start_menu_sfx(commands: Commands, menu: Option<Res<MenuSfx>>, buses: ResMut<Assets<SfxBus>>) {
    menu_sfx_player(commands, menu, buses, Screen::Menu);
}

fn start_return_sfx(commands: Commands, menu: Option<Res<MenuSfx>>, buses: ResMut<Assets<SfxBus>>) {
    menu_sfx_player(commands, menu, buses, Screen::Returning);
}

/// The front end's sounds' player while `screen` lasts.
fn menu_sfx_player(
    mut commands: Commands,
    menu: Option<Res<MenuSfx>>,
    mut buses: ResMut<Assets<SfxBus>>,
    screen: Screen,
) {
    let Some(menu) = menu else { return };
    commands.spawn((
        AudioPlayer(buses.add(menu.bus.clone())),
        PlaybackSettings::ONCE.with_volume(Volume::Linear(1.0)),
        crate::audio::Part::Scripted,
        DespawnOnExit(screen),
    ));
}

fn run_menu_sfx(
    menu: Option<ResMut<MenuSfx>>,
    time: Res<Time<Real>>,
    settings: Res<AudioSettings>,
) {
    let Some(mut menu) = menu else { return };
    if settings.is_changed() {
        menu.fx.set_volumes(&settings);
    }
    menu.fx.run(time.delta_secs_f64() * 1000.0);
}

fn end_match(mut commands: Commands, fx: Option<ResMut<Effects>>) {
    if let Some(fx) = fx
        && let Ok(mut bus) = fx.bus.lock()
    {
        bus.commands.push(Cmd::StopAll);
    }
    commands.remove_resource::<Effects>();
    commands.remove_resource::<FloorContacts>();
}

/// Slot 76's poster (`FUN_00156f48`, `DAT_003c6930`) compares joint 10's
/// height with its last rendered height: >1.5 becoming <=1.5. Its event
/// float is that drop, not distance or elapsed stumble time; `0x1a74f8`
/// scales it by 500 (`lui 0x43fa`), truncates (`cvt.w.s`), takes its
/// magnitude and caps it at 1000. The exclusion
/// at AIGame+0x424+0x50 is the *global Script Manager*: the AIGame
/// constructor (`FUN_0013c4e8`) stores `FUN_00335708`'s result at game
/// +0x424 (replay manager at +0x420, AI script handler at +0x428), so
/// +0x50 is the manager's own state — not a player action. Its states
/// 3..7 cover scripts and their replay/transition lifecycle (freeze state
/// 7, replay state 5). Staged sequences, intros and replays all run under
/// it, so floor contacts stay silent through them; all ordinary actions
/// are observed, including dives and recovery.
#[derive(Resource, Default)]
struct FloorContacts {
    tick: Option<u64>,
    heights: Vec<Option<f32>>,
}

impl FloorContacts {
    fn begin(&mut self, tick: u64, enabled: bool, players: usize) -> bool {
        if !enabled {
            *self = Self::default();
            return false;
        }
        if self.tick == Some(tick) {
            return false;
        }
        if self.tick.is_some_and(|before| tick < before) {
            self.heights.clear();
        }
        self.tick = Some(tick);
        self.heights.resize(players, None);
        true
    }

    fn sample(&mut self, player: usize, height: Option<f32>) -> Option<i32> {
        let previous = self.heights.get_mut(player)?;
        let height = height.filter(|h| h.is_finite());
        let before = std::mem::replace(previous, height)?;
        let height = height?;
        (before > 1.5 && height <= 1.5)
            .then(|| (((before - height) * 500.0).trunc() as i32).clamp(0, 1000))
    }
}

/// Read the posed rig after transform propagation. This presentation event
/// needs no network mutation and never manufactures a fall for a stagger.
fn floor_contacts(
    fx: Option<ResMut<Effects>>,
    contacts: Option<ResMut<FloorContacts>>,
    sim: Res<Sim>,
    replay: Res<Replay>,
    tick: Res<TickEvents>,
    paused: Option<Res<Paused>>,
    intro: Option<Res<crate::intro::MatchIntro>>,
    players: Query<(&crate::view::SimPlayer, &street_render::animation::Rig)>,
    joints: Query<&GlobalTransform>,
    camera: Query<&GlobalTransform, With<SidelineCamera>>,
    mut crowd: Option<ResMut<street_render::crowd::CrowdReactions>>,
) {
    let (Some(mut fx), Some(mut contacts)) = (fx, contacts) else {
        return;
    };
    let w = &sim.world;
    let enabled = replay.playing.is_none()
        && !tick.replay
        && !paused.is_some_and(|p| p.on)
        && !intro.is_some_and(|i| i.current.is_some())
        && w.sequence.is_none();
    if !contacts.begin(w.tick, enabled, w.players.len()) {
        return;
    }
    let Ok(cam) = camera.single() else {
        contacts.heights.fill(None);
        return;
    };
    for (actor, rig) in &players {
        let Some(p) = w.players.get(actor.0) else {
            continue;
        };
        let height = rig
            .joints
            .get(10)
            .and_then(|joint| joints.get(*joint).ok())
            .map(|joint| joint.translation().y);
        if let Some(intensity) = contacts.sample(actor.0, height) {
            let at = Vec3::new(p.position[0], p.height + 3.0, p.position[1]);
            floor_contact(
                &mut fx,
                crowd.as_deref_mut(),
                w,
                actor.0,
                place(cam, at),
                intensity,
            );
        }
    }
}

/// `FUN_001a5348`, slot 76's handler: the body against the floor (0x4f kind
/// 2 by the drop's strength), the crowd's call (`FUN_001a75a8` with 0x29: the
/// AEMS event, which its `FUN_0019ed30` routing gives no arguments, and the
/// spectators' gasp through `FUN_0030d618`), then the player's grunt.
fn floor_contact(
    fx: &mut Effects,
    crowd: Option<&mut street_render::crowd::CrowdReactions>,
    w: &street_sim::World,
    player: usize,
    (angle, distance): (i32, i32),
    intensity: i32,
) {
    fx.fire(&[ev::BODY, angle, distance, intensity, 2]);
    fx.fire(&[ev::FALL_CROWD]);
    if let Some(crowd) = crowd {
        crowd.0.push(street_render::crowd::Reaction {
            fraction: crate::game::crowd_fraction(w),
            cheer: false,
        });
    }
    let voice = fx.voices.get(player).copied().unwrap_or(0);
    fx.fire(&[ev::EXERTION, angle, distance, voice]);
}

/// Slot 22's moment (`0x1a5d10`, class `0x3c65e0`, posted by `FUN_0014cb28`
/// when a player takes a ball in state 7, the referee's toss): the
/// simulation's tip-off takes the toss and passes in the same tick, the
/// only pass while the toss is still up.
pub(crate) fn tip_taken(w: &street_sim::World, event: &Event) -> bool {
    matches!(event, Event::Pass { .. }) && w.opening_toss == street_sim::OpeningToss::Flight
}

/// Runs the scripts on real time, while the match isn't paused.
fn run_scripts(
    fx: Option<ResMut<Effects>>,
    paused: Option<Res<Paused>>,
    time: Res<Time<Real>>,
    settings: Res<AudioSettings>,
) {
    let Some(mut fx) = fx else { return };
    if settings.effects != fx.level || settings.is_changed() {
        fx.set_volumes(&settings);
    }
    if paused.is_some_and(|p| p.on) {
        return;
    }
    fx.run(time.delta_secs_f64() * 1000.0);
}

/// Where a sound is, as the game measures it from the camera (`0x19ebe0`):
/// an angle (0..0xffff, 0 ahead, growing to the left) and a distance
/// (0..1000).
pub(crate) fn place(camera: &GlobalTransform, at: Vec3) -> (i32, i32) {
    let local = camera.affine().inverse().transform_point3(at);
    let (x, z) = (local.x, -local.z / 4.0);
    let mut deg = if z.abs() < 1e-6 {
        90f32.copysign(-x)
    } else {
        (-x / z).atan().to_degrees()
    }
    .round() as i32;
    if z < 0.0 {
        deg += 180;
    } else if x > 0.0 {
        deg += 360;
    }
    let angle = (deg * 0xffff / 360) % 0xffff;
    let dist = ((x * x + z * z).round() as i32).saturating_mul(4).min(1000);
    (angle.max(0), dist.max(0))
}

/// `0x1a74f8`: an impact's speed as 0..1000 (its `cvt.w.s` truncates on
/// the EE). The ×50 scale for the rewrite's per-second speeds is its own.
pub(crate) fn intensity(speed: f32) -> i32 {
    ((speed * 50.0).trunc().abs() as i32).min(1000)
}

/// Counts one more and says whether the count is now odd: the listener's
/// "every other time" (`0x1a4928`, `0x1a4c58`, `0x1a60f0`).
fn every_other(count: &mut u32) -> bool {
    *count = count.wrapping_add(1);
    *count % 2 == 1
}

/// `FUN_001a7708` then `FUN_001a75a8`'s word: whether the acting side
/// (players 0..2 are the first) leads on the scoreboard.
fn leads(w: &street_sim::World, player: usize) -> bool {
    let side = usize::from(player >= 3);
    w.score[side] > w.score[1 - side]
}

/// HUD sounds pan to the team's side (`0x1a7548`).
pub(crate) fn side(team: usize) -> i32 {
    if team > 0 { 0xdfff } else { 0x2000 }
}

/// The teammate who answers `player` (`0x1a3b10`): the side's third
/// player, or its second when `player` is the third.
fn teammate(w: &street_sim::World, player: usize) -> usize {
    let team = w.players.get(player).map_or(0, |p| p.team);
    let side: Vec<usize> = (0..w.players.len())
        .filter(|&i| w.players[i].team == team)
        .collect();
    let me = side.iter().position(|&i| i == player).unwrap_or(0);
    let pick = if me == 2 { 1 } else { 2 };
    side.get(pick).or(side.first()).copied().unwrap_or(player)
}

/// A side's third player, who makes the team calls (players 2 and 5).
fn third(w: &street_sim::World, team: usize) -> usize {
    (0..w.players.len())
        .filter(|&i| w.players[i].team == team.min(1))
        .nth(2)
        .unwrap_or(0)
}

/// The simulation's events, as the game's listener fires them.
pub(crate) fn fire_events(
    fx: Option<ResMut<Effects>>,
    mut pause_speech: Option<ResMut<crate::audio::SpeechManager>>,
    sim: Res<Sim>,
    tick: Res<TickEvents>,
    replay: Res<Replay>,
    clips: Option<Res<crate::setup::GameClips>>,
    intro: Option<Res<crate::intro::MatchIntro>>,
    camera: Query<&GlobalTransform, With<SidelineCamera>>,
) {
    let Some(mut fx) = fx else { return };
    let Ok(cam) = camera.single() else { return };
    let w = &sim.world;
    let player_at = |i: usize| {
        w.players.get(i).map_or(Vec3::ZERO, |p| {
            Vec3::new(p.position[0], p.height + 3.0, p.position[1])
        })
    };
    let ball = Vec3::from_array(w.ball.position);
    let hoop_at = |h: usize| Vec3::from_array(w.hoops[h.min(1)].rim);
    let at = |pos: Vec3| place(cam, pos);
    let team_of = |i: usize| w.players.get(i).map_or(0, |p| p.team);

    // Slot 61's first call also resumes speech with its queue cleared and
    // introduces the court (`audio::match_sounds`, which runs first).
    if !fx.started && w.tick >= crate::hud::INTRO_TICKS {
        fx.play_starts();
    }

    // A staged gamebreaker's own sound commands (dispatch 2), as
    // `FUN_00331630` posts them through `FUN_0019e840`: event 0x21 restarts
    // the gamebreaker loop with the command's parameter; any other is
    // [event, angle, distance, intensity, parameter, extra] from the
    // command's spot (an intensity of 0 is 1000).
    match (w.sequence, clips.as_ref()) {
        (Some(s), Some(clips)) if !replay.playing.is_some() => {
            let from = match fx.staged {
                Some((script, done)) if script == s.script => Some(done),
                _ => None,
            };
            if let Some(timeline) = clips.timeline.scripts.get(s.script) {
                for record in &timeline.records {
                    let street_assets::timeline::Payload::Command(c) = &record.payload else {
                        continue;
                    };
                    let due = record.frame <= s.frame && from.is_none_or(|f| record.frame > f);
                    let Some(sound) = c.sound().filter(|_| due) else {
                        continue;
                    };
                    let mut spot = Vec3::from_array(sound.position);
                    if s.mirrored() {
                        spot.x = -spot.x;
                    }
                    let (a, d) = place(cam, spot);
                    fx.staged_sound(sound, a, d);
                }
            }
            fx.staged = Some((s.script, s.frame));
        }
        (None, _) => fx.staged = None,
        _ => {}
    }

    // A replay's start and end (slots 58, 59: `0x1a5410`, `0x1a54d8`): a
    // transition and its loop, and the speech paused, all only while no
    // gamebreaker task runs (`FUN_00140ff8`). The task's own two replays
    // (mode 5) and a staged script's (mode 7) play under its task, and the
    // director asks for no other while one runs (`FUN_00324cc8`), so those
    // two pass in silence; an intro's own replay command sounds.
    let replaying = replay.playing.is_some_and(|p| {
        p.mode != crate::tape::mode::GAMEBREAKER && p.mode != crate::tape::mode::STAGED
    }) || intro.is_some_and(|i| i.replaying());
    if replaying != fx.replaying {
        fx.replaying = replaying;
        if replaying {
            fx.start_loop(&[ev::REPLAY_LOOP]);
            fx.fire(&[ev::TRANSITION, 0]);
        } else {
            fx.stop_loop(ev::REPLAY_LOOP);
        }
        // Slots 58 and 59 (`0x1a5410`, `0x1a54d8`) outside a gamebreaker
        // task (`FUN_00140ff8`): speech paused with the queue cleared
        // (`FUN_001ad958(manager, 0, 1)`), then resumed
        // (`FUN_001ad9d0(manager, 0, 0)`). The session's manager is reached
        // through `speech` only off replays, so this takes it directly.
        if w.sequence.is_none()
            && let Some(manager) = pause_speech.as_deref_mut()
        {
            if replaying {
                manager.pause(true);
            } else {
                manager.resume(false);
            }
        }
    }

    // Requests go to the speech manager, which serves them each tick
    // (`audio::speech_service`); a replay's re-stepped ticks ask nothing.
    let mut speech = pause_speech.as_deref_mut().filter(|_| !tick.replay);
    for event in &tick.events {
        match *event {
            Event::Catch { .. } => {
                let (a, d) = at(ball);
                fx.fire(&[ev::HIT, a, d, 1000]);
            }
            // A live rebound taken (slot 17, `0x1a65c0`, class `0x3c6590`):
            // the crowd's call, then every other time (`DAT_004bb2d8`, for
            // the session) a teammate's line 1, else the player's line 0.
            Event::Rebound { player } => {
                let (a, d) = at(ball);
                fx.fire(&[ev::REBOUND, a, d]);
                fx.fire(&[ev::CROWD_CALL, i32::from(leads(w, player))]);
                if let Some(speech) = speech.as_deref_mut() {
                    speech.rebounds = speech.rebounds.wrapping_add(1);
                    let (who, category) = if speech.rebounds.is_multiple_of(2) {
                        (teammate(w, player), 1)
                    } else {
                        (player, 0)
                    };
                    fx.say(Some(speech), who, category, at(player_at(who)));
                }
            }
            Event::Shot { player, .. } | Event::Layup { player } => {
                let (a, d) = at(player_at(player));
                if every_other(&mut fx.exertions) {
                    let voice = fx.voices.get(player).copied().unwrap_or(0);
                    fx.fire(&[ev::EXERTION, a, d, voice]);
                }
                fx.fire(&[ev::SWEETENER, a, d, 1000, 10]);
                // Slot 11 (`0x1a4738`): the launcher posts the ball's kind,
                // 2 for a jump shot (its flight, at least half a second, is
                // over the handler's 2 frames: the whoosh 0x3f, and the
                // launch line), 5 for a lay-up (sweetener 6).
                let (ba, bd) = at(ball);
                if matches!(event, Event::Shot { .. }) {
                    fx.fire(&[ev::LOB, ba, bd]);
                    fx.say_launch(speech.as_deref_mut(), player, (a, d));
                } else {
                    fx.fire(&[ev::SWEETENER, ba, bd, 1000, 6]);
                }
            }
            Event::Dunk { player, .. } => {
                let (a, d) = at(player_at(player));
                if every_other(&mut fx.exertions) {
                    let voice = fx.voices.get(player).copied().unwrap_or(0);
                    fx.fire(&[ev::EXERTION, a, d, voice]);
                }
                fx.fire(&[ev::SWEETENER, a, d, 1000, 13]);
            }
            // A block jump's take-off (slot 6): the block scripts' callback
            // 35 (`0x176080`, posting at `0x177120`) grunts every other time
            // on the shots' count. Then the swing (slot 7): the block's
            // check (`0x16d888`, posting at `0x16dc78`) fires sweetener 11
            // once a jump, as the hand first misses the ball.
            Event::Jump {
                player,
                block: true,
            } => {
                let (a, d) = at(player_at(player));
                if every_other(&mut fx.exertions) {
                    let voice = fx.voices.get(player).copied().unwrap_or(0);
                    fx.fire(&[ev::EXERTION, a, d, voice]);
                }
                fx.fire(&[ev::SWEETENER, a, d, 1000, 11]);
            }
            // A reach for the ball (slot 8, posted at `0x15e390` as the
            // swipe or the counter stance starts): sweetener 12, and every
            // other time a grunt on its own count.
            Event::Reach { player, .. } => {
                let (a, d) = at(player_at(player));
                fx.fire(&[ev::SWEETENER, a, d, 1000, 12]);
                if every_other(&mut fx.reaches) {
                    let voice = fx.voices.get(player).copied().unwrap_or(0);
                    fx.fire(&[ev::EXERTION, a, d, voice]);
                }
            }
            // A dunk let go at the rim (slot 12, the shot release
            // `0x14aaf8` posting at `0x14bf18` for a dunk or an alley-oop
            // dunk): a grunt. Its handler also brings back the music that
            // slot 4 ducked for a power dunk (`0x1ab210`), not modelled.
            Event::Slam { player, .. } => {
                let (a, d) = at(player_at(player));
                let voice = fx.voices.get(player).copied().unwrap_or(0);
                fx.fire(&[ev::EXERTION, a, d, voice]);
                // Slot 13 (`FUN_001a5a78`, 0x4d kind 4): posted by the shot
                // setup at `0x14bf28` (class `DAT_003c6540`, conditional on
                // the dunk-release flag at 0xd4(sp)) — the same handler the
                // scoring listener routes trick lands (ring 10/11) to. It
                // sounds as the dunk leaves the hands, even on a miss.
                fx.fire(&[ev::HUD, side(team_of(player)), 4]);
            }
            Event::Score {
                team,
                player,
                basket,
                ..
            } => {
                let hoop = street_sim::target_hoop(team);
                let (ha, hd) = at(hoop_at(hoop));
                let (ba, bd) = at(ball);
                match basket {
                    Basket::Dunk { advanced } => {
                        fx.fire(&[ev::RIM_RATTLE, ha, hd, 1000]);
                        fx.fire(&[ev::SLAM, ha, 1000, i32::from(advanced)]);
                        let power = if advanced { 700 } else { 300 };
                        fx.fire(&[ev::DUNK_CROWD, ba, power]);
                    }
                    Basket::AlleyOop { .. } => {
                        fx.fire(&[ev::RIM_RATTLE, ha, hd, 1000]);
                        fx.fire(&[ev::SLAM, ha, 1000, 1]);
                        fx.fire(&[ev::DUNK_CROWD, ba, 900]);
                    }
                    Basket::Gamebreaker { .. } => {
                        fx.fire(&[ev::SLAM, ha, 1000, 1]);
                        fx.fire(&[ev::DUNK_CROWD, ba, 900]);
                        fx.stop_loop(ev::GAMEBREAKER_LOOP);
                    }
                    Basket::Jumper { .. } | Basket::Layup => {
                        fx.fire(&[ev::SWISH, ha, hd]);
                    }
                }
                fx.fire(&[ev::HUD, side(team_of(player)), 9]);
                fx.fire(&[ev::NET, ba, bd]);
                // A dunk: a teammate hypes it (slot 26, `0x1a41cc`).
                if matches!(basket, Basket::Dunk { .. } | Basket::AlleyOop { .. }) {
                    let mate = teammate(w, player);
                    fx.say(speech.as_deref_mut(), mate, 5, at(player_at(mate)));
                }
                if !matches!(w.phase, Phase::Over { .. }) {
                    fx.fire(&[ev::CROWD, 1]);
                }
            }
            Event::RimOut => {
                let hoop = if ball.x < 0.0 { 0 } else { 1 };
                let (a, _) = at(hoop_at(hoop));
                fx.fire(&[ev::RIM, a, 1000]);
                // A missed shot (slot 27, `0x1a4550`, posted by `0x14d1f8`
                // with the ball's shooter): the shooter asks category 14
                // with shot mask 2, then category 4 (raw `0x1a458c..`).
                if let Some(speech) = speech.as_deref_mut()
                    && let Some((shooter, _)) = speech.last_shot
                {
                    let spot = at(player_at(shooter));
                    fx.say_masks(Some(&mut *speech), shooter, [1 << 14, 2, 2], spot);
                    fx.say_masks(Some(speech), shooter, [1 << 4, 0, 1], spot);
                }
            }
            Event::Goaltend { .. } => {
                let (a, d) = at(ball);
                fx.fire(&[ev::HIT, a, d, 1000]);
            }
            // A block (slot 16, `0x1a4ce8`): 0x39 with whether the blocker
            // now holds the ball, the crowd's call, the blocker's grunt and
            // line 9.
            Event::Block { player, .. } => {
                let (a, d) = at(ball);
                fx.fire(&[ev::HIT, a, d, 1000]);
                let holds = w.ball.state == BallState::Held(player);
                fx.fire(&[ev::BLOCK, a, d, 1000, i32::from(holds)]);
                fx.fire(&[ev::CROWD_CALL, i32::from(leads(w, player))]);
                let (pa, pd) = at(player_at(player));
                let voice = fx.voices.get(player).copied().unwrap_or(0);
                fx.fire(&[ev::EXERTION, pa, pd, voice]);
                fx.say(speech.as_deref_mut(), player, 9, (pa, pd));
            }
            // A steal or a counter (slot 15, `0x1a63e0`): the crowd's call,
            // the stealer's line 10 and 0x49 with the stealer's flag +0xa6
            // bit 0 (`FUN_0016e9b0`, the flag `0x148378` tests before it
            // hands a player the ball): the rewrite's reading, the stealer
            // holding the ball, as 0x39 reads the blocker's.
            Event::Steal { player, .. } | Event::Counter { player, .. } => {
                let (a, d) = at(ball);
                fx.fire(&[ev::REBOUND, a, d]);
                let (pa, pd) = at(player_at(player));
                fx.fire(&[ev::CROWD_CALL, i32::from(leads(w, player))]);
                fx.say(speech.as_deref_mut(), player, 10, (pa, pd));
                let holds = w.ball.state == BallState::Held(player);
                fx.fire(&[ev::STEAL, pa, pd, 1000, i32::from(holds)]);
                let voice = fx.voices.get(player).copied().unwrap_or(0);
                fx.fire(&[ev::EXERTION, pa, pd, voice]);
                // A counter that takes the ball knocks the handler back
                // (slot 74, level 1, the collision posting at `0x134c14`):
                // the body sound 0x4f 0 where the handler stumbles.
                if let Event::Counter { from, .. } = *event {
                    let (fa, fd) = at(player_at(from));
                    fx.fire(&[ev::BODY, fa, fd, 1000, 0]);
                }
            }
            // The tip-off: the winner takes the referee's toss (ball state
            // 7) and passes at once (`FUN_0014cb08`). The take posts slot 22
            // (`0x1a5d10`, class `0x3c65e0`): the crowd's 0x25, and the
            // commentator's override (+0x68) cleared.
            _ if tip_taken(w, event) && !fx.tipped => {
                fx.tipped = true;
                fx.fire(&[ev::TIP_CROWD]);
                if let Some(speech) = speech.as_deref_mut() {
                    speech.override_busy = false;
                }
            }
            Event::Trick {
                player,
                level,
                fakeout,
                ..
            } => {
                // The game's levels count from 0; 0..2 get a sweetener, the
                // higher ones a music stinger instead (`song.rs`).
                let game_level = i32::from(level) - 1;
                let (a, d) = at(player_at(player));
                if (0..3).contains(&game_level) {
                    fx.fire(&[ev::SWEETENER, a, d, 1000, 7 + game_level]);
                }
                // The move lands (slot 13, `FUN_001a5a78` — the scoring
                // listener routes trick lands, ring classes 10/11, to it;
                // the shot setup posts the same handler for dunk releases,
                // wired on `Event::Slam`): a sting and a taunt. The handler
                // fires its crowd 0x29 + speech only when the event word at
                // +8 is nonzero — the trick value on the ring path, against
                // 0 on the dunk post — so tricks get the crowd's gasp too.
                fx.fire(&[ev::HUD, side(team_of(player)), 4]);
                fx.fire(&[ev::FALL_CROWD]);
                // FUN_001a5a78 posts this request without a caller-side
                // probability draw.
                fx.say(speech.as_deref_mut(), player, 3, (a, d));
                // A faked-out defender is knocked down (`FUN_001a5288`):
                // 0x4f's kind is the knockdown's level minus one — kind 0
                // a stagger, kind 1 the fall — and a fall (the knockdown's
                // level 2) also posts sweetener 5, its own sound. A pick
                // or a trick counter only stumbles, below.
                if let Some(defender) = fakeout {
                    let (fa, fd) = at(player_at(defender));
                    if level >= 2 {
                        fx.fire(&[ev::BODY, fa, fd, 1000, 1]);
                        fx.fire(&[ev::SWEETENER, fa, fd, 1000, 5]);
                    } else {
                        fx.fire(&[ev::BODY, fa, fd, 1000, 0]);
                    }
                }
            }
            Event::Dive { player } => {
                let (a, d) = at(player_at(player));
                let voice = fx.voices.get(player).copied().unwrap_or(0);
                fx.fire(&[ev::EXERTION, a, d, voice]);
            }
            Event::Pick { screener, defender } => {
                let (a, d) = at(player_at(screener));
                let voice = fx.voices.get(screener).copied().unwrap_or(0);
                fx.fire(&[ev::EXERTION, a, d, voice]);
                // The screen's contact (slot 74, posting at `0x136198`):
                // level 1 as the defender stumbles (level 2, a fall, adds
                // sweetener 5 in `0x1a5288`; picks here only stumble).
                let (da, dd) = at(player_at(defender));
                fx.fire(&[ev::BODY, da, dd, 1000, 0]);
            }
            // Slot 66 (`0x1a3638`): a full meter pauses speech with its
            // queue cleared (`FUN_001ad958(manager, 1, 1)`).
            Event::GamebreakerReady { team } => {
                let level = w.teams[team.min(1)].gamebreaker();
                fx.fire(&[ev::HUD, side(team), if level >= 2 { 0x12 } else { 2 }]);
                fx.start_loop(&[ev::READY_LOOP]);
                if let Some(speech) = speech.as_deref_mut() {
                    speech.pause(true);
                }
            }
            // Slot 67 (`0x1a3308`): the meter drops; below full, speech
            // resumes with its queue cleared (`FUN_001ad9d0(manager, 1, 1)`).
            Event::MeterDropped { team } => {
                if let Some(speech) = speech.as_deref_mut()
                    && !w.teams[team.min(1)].meter_full()
                {
                    speech.resume(true);
                }
            }
            Event::Gamebreaker { .. } => {
                fx.fire(&[ev::TRANSITION, 1]);
                fx.start_loop(&[ev::GAMEBREAKER_LOOP, 0]);
            }
            // Slots 70 and 71 (`0x1a3868`, `0x1a3498`): a pocket resumes
            // speech below a full meter, a cancel always.
            Event::Pocket { team } => {
                fx.fire(&[ev::HUD, side(team), 0x10]);
                fx.stop_loop(ev::READY_LOOP);
                if let Some(speech) = speech.as_deref_mut()
                    && !w.teams[team.min(1)].meter_full()
                {
                    speech.resume(true);
                }
            }
            Event::Cancel { .. } => {
                fx.fire(&[ev::CROWD, 1]);
                fx.stop_loop(ev::READY_LOOP);
                if let Some(speech) = speech.as_deref_mut() {
                    speech.resume(true);
                }
            }
            Event::Combo { team, points, .. } if points > 5000 => {
                fx.fire(&[ev::HUD, side(team), if points > 12_000 { 7 } else { 6 }]);
            }
            Event::ShotClockViolation { team } => {
                fx.fire(&[ev::HUD, side(team), 0xb]);
                // A word on it (slot 52): the violators, then next time
                // the other side.
                fx.turns[1] += 1;
                let (who, category) = if fx.turns[1] % 2 == 1 {
                    (team, 4)
                } else {
                    (1 - team.min(1), 8)
                };
                let p = third(w, who);
                fx.say(speech.as_deref_mut(), p, category, at(player_at(p)));
            }
            Event::GameOver { .. } => {
                fx.fire(&[ev::CROWD_BIG, 1]);
                fx.stop_loop(ev::READY_LOOP);
                fx.stop_loop(ev::GAMEBREAKER_LOOP);
            }
            _ => {}
        }
    }

    // The shot clock's last seconds (slot 48): a beep each second under 9.
    let left = w.shot_clock.ceil() as i32;
    if matches!(w.phase, Phase::Live) && left != fx.shot_clock {
        if left < 9 && left > 0 && !tick.replay {
            fx.fire(&[ev::HUD, side(w.possession), 10]);
        }
        // The calls (slot 48): five seconds left, the offence; at 21, now
        // and then, the defence.
        if left == 5 {
            let p = third(w, w.possession);
            fx.say(speech.as_deref_mut(), p, 2, at(player_at(p)));
        }
        // `rand() & 1` (`0x1a5cb4`): the rewrite's own draw stands in for
        // the game's global generator, which the presentation cannot take.
        if left == 21 && fx.roll() & 1 != 0 {
            let p = third(w, 1 - w.possession.min(1));
            fx.say(speech, p, 7, at(player_at(p)));
        }
        fx.shot_clock = left;
    }

    // The ball on the floor: a dribble each bounce (slot 44), and a loose
    // ball's fall turning into a rise (slot 30).
    if let BallState::Held(holder) = w.ball.state {
        let p = &w.players[holder];
        if matches!(
            p.action,
            street_sim::Action::None | street_sim::Action::Trick { .. }
        ) && p.height <= 0.01
        {
            let period = if p.speed > 8.0 { 0.32 } else { 0.48 };
            fx.dribble += 1.0 / street_sim::TICK_HZ as f32;
            if fx.dribble >= period {
                fx.dribble = 0.0;
                let (a, d) = at(player_at(holder));
                fx.fire(&[ev::FLOOR, a, d, 1000]);
            }
        }
    }
    let vy = w.ball.velocity[1];
    if w.ball.state == BallState::Loose && fx.ball_vy < -2.0 && vy > 0.0 {
        let (a, d) = at(ball);
        let level = intensity(-fx.ball_vy);
        fx.fire(&[ev::FLOOR, a, d, level]);
    }
    fx.ball_vy = vy;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_contact_uses_the_animated_height_drop_once() {
        let mut contacts = FloorContacts::default();
        assert!(contacts.begin(1, true, 1));
        assert_eq!(contacts.sample(0, Some(2.0)), None);
        assert!(contacts.begin(2, true, 1));
        assert_eq!(contacts.sample(0, Some(1.5)), Some(250));
        assert!(
            !contacts.begin(2, true, 1),
            "one sample per simulation tick"
        );
        assert!(contacts.begin(3, true, 1));
        assert_eq!(contacts.sample(0, Some(1.0)), None, "already on the floor");
        assert!(contacts.begin(4, true, 1));
        assert_eq!(contacts.sample(0, Some(4.0)), None, "getting up is silent");
        assert!(contacts.begin(5, true, 1));
        assert_eq!(contacts.sample(0, Some(1.0)), Some(1000));
        // `cvt.w.s` truncates: a 0.1999 drop is 99, not 100.
        assert!(contacts.begin(6, true, 1));
        assert_eq!(contacts.sample(0, Some(1.6999)), None);
        assert!(contacts.begin(7, true, 1));
        assert_eq!(contacts.sample(0, Some(1.5)), Some(99));
    }

    #[test]
    fn floor_contact_resets_across_replays_and_rewinds() {
        let mut contacts = FloorContacts::default();
        assert!(contacts.begin(10, true, 1));
        assert_eq!(contacts.sample(0, Some(2.0)), None);
        assert!(!contacts.begin(11, false, 1));
        assert!(contacts.begin(12, true, 1));
        assert_eq!(
            contacts.sample(0, Some(1.0)),
            None,
            "no replay boundary impact"
        );
        assert!(contacts.begin(13, true, 1));
        assert_eq!(contacts.sample(0, Some(2.0)), None);
        assert!(contacts.begin(1, true, 1));
        assert_eq!(
            contacts.sample(0, Some(1.0)),
            None,
            "no rollback boundary impact"
        );
    }

    #[test]
    fn floor_contact_ignores_staggers_and_missing_rigs() {
        let mut contacts = FloorContacts::default();
        assert!(contacts.begin(1, true, 2));
        assert_eq!(contacts.sample(0, Some(2.0)), None);
        assert_eq!(
            contacts.sample(1, Some(1.0)),
            None,
            "first sample is silent"
        );
        assert!(contacts.begin(2, true, 2));
        assert_eq!(
            contacts.sample(0, Some(1.8)),
            None,
            "stagger stays above threshold"
        );
        assert_eq!(contacts.sample(1, Some(f32::NAN)), None);
        assert!(contacts.begin(3, true, 2));
        assert_eq!(
            contacts.sample(0, None),
            None,
            "a missing rig clears its history"
        );
        assert_eq!(
            contacts.sample(1, Some(0.5)),
            None,
            "invalid height clears history"
        );
        assert!(contacts.begin(4, true, 2));
        assert_eq!(
            contacts.sample(0, Some(1.0)),
            None,
            "no impact from a missing rig's earlier pose"
        );
    }

    #[test]
    fn disc_fall_clips_cross_the_original_joint_threshold() {
        let Some(mut iso) = iso() else {
            eprintln!("STREET_ISO not set; skipping");
            return;
        };
        let skeleton = street_assets::player::skeleton(&mut iso).expect("player skeleton");
        let clips = street_render::animation::Clips::load(&mut iso, &crate::clip::STUMBLES)
            .expect("stumble clips");
        let mut impacts = 0;
        for name in crate::clip::STUMBLES {
            let clip = clips.get(name).expect("loaded stumble clip");
            let mut contacts = FloorContacts::default();
            for frame in 0..clip.frames() {
                let y = clip
                    .joint_position(&skeleton, 10, frame as f32, false, true)
                    .expect("joint 10")
                    .y;
                assert!(contacts.begin(frame as u64, true, 1));
                if let Some(intensity) = contacts.sample(0, Some(y)) {
                    eprintln!(
                        "{name}: joint 10 floor contact at frame {frame}, intensity {intensity}"
                    );
                    impacts += 1;
                }
            }
        }
        assert!(
            impacts > 0,
            "the played fall clips reach the traced floor threshold"
        );
    }

    #[test]
    fn disc_floor_observer_accepts_dives_and_keeps_locomotion_silent() {
        let Some(mut iso) = iso() else {
            eprintln!("STREET_ISO not set; skipping");
            return;
        };
        let skeleton = street_assets::player::skeleton(&mut iso).expect("player skeleton");
        let names = [
            crate::clip::DIVE,
            crate::clip::STAND,
            crate::clip::DEFEND,
            crate::clip::WALK,
            crate::clip::RUN,
            crate::clip::TURBO,
            crate::clip::STAND_BALL,
            crate::clip::WALK_BALL,
            crate::clip::RUN_BALL,
            crate::clip::TURBO_BALL,
        ];
        let clips = street_render::animation::Clips::load(&mut iso, &names).expect("action clips");
        for name in names {
            let clip = clips.get(name).expect("loaded action clip");
            let mut contacts = FloorContacts::default();
            let mut impacts = 0;
            for frame in 0..clip.frames() {
                let y = clip
                    .joint_position(&skeleton, 10, frame as f32, false, true)
                    .expect("joint 10")
                    .y;
                assert!(contacts.begin(frame as u64, true, 1));
                impacts += usize::from(contacts.sample(0, Some(y)).is_some());
            }
            if name == crate::clip::DIVE {
                assert!(
                    impacts > 0,
                    "a real dive reaches the body-contact threshold"
                );
            } else {
                assert_eq!(impacts, 0, "ordinary locomotion is silent: {name}");
            }
        }
    }

    fn iso() -> Option<street_assets::iso::Iso<std::io::BufReader<std::fs::File>>> {
        let path = std::env::var("STREET_ISO").ok()?;
        street_assets::iso::Iso::open(path).ok()
    }

    /// Runs the scripts and the mix for `ms`, returning the mix's RMS.
    fn listen(fx: &mut Effects, src: &mut SfxSource, ms: u32) -> f32 {
        let mut sum = 0f64;
        let mut n = 0u64;
        for _ in 0..ms / 10 {
            fx.run(10.0);
            for _ in 0..(RATE / 100 * 2) {
                let s = src.next().unwrap_or(0.0);
                sum += f64::from(s * s);
                n += 1;
            }
        }
        (sum / n.max(1) as f64).sqrt() as f32
    }

    /// Takes one queued voice after its latest gain update, without mixing in
    /// other banks' crowd and court voices.
    fn queued_voice(fx: &mut Effects, bank: usize, offset: u32) -> Voice {
        let mut voice = None;
        let mut id = None;
        let mut bus = fx.bus.lock().expect("audio bus");
        for command in bus.commands.drain(..) {
            match command {
                Cmd::Start {
                    id: started,
                    src:
                        src @ Src::Stream {
                            bank: source_bank,
                            offset: source_offset,
                            ..
                        },
                    rate,
                    params,
                } if source_bank == bank && source_offset == offset => {
                    assert!(voice.is_none(), "one start for the selected stream");
                    id = Some(started);
                    voice = Some(Voice::new(started, src, rate, params));
                }
                Cmd::Update(updated, params) if Some(updated) == id => {
                    if let Some(voice) = voice.as_mut() {
                        voice.set(params);
                    }
                }
                _ => {}
            }
        }
        voice.expect("selected speech stream start was queued")
    }

    /// The AST offset of a PLAY selector's stream (node 27 of module 0x1f6e).
    fn selected_stream_offset(program: &Program, stream: u16) -> u32 {
        let play = &program
            .modules
            .iter()
            .find(|m| m.id == 0x1f6e)
            .expect("native player speech module")
            .nodes[27];
        let sound = play.table[usize::from(stream)] as u16;
        assert_eq!(sound >> 14, 3, "player line selects a streamed sound");
        program.streams[usize::from(sound & 0x3fff)].offset
    }

    /// RMS of a single AEMS voice through the same pan and mono fold as the
    /// game mix.
    fn voice_rms(voice: &mut Voice, ms: usize) -> f32 {
        let frames = RATE as usize * ms / 1000;
        let mut sum = 0.0f64;
        for _ in 0..frames {
            let [left, right] = voice.frame();
            let [left, right] = crate::audio::mono(left * voice.gains[0], right * voice.gains[1]);
            sum += f64::from((left * left + right * right) * 0.5);
        }
        (sum / frames.max(1) as f64).sqrt() as f32
    }

    #[test]
    fn every_other_time_is_the_odd_counts() {
        let mut count = 0;
        let fired: Vec<bool> = (0..5).map(|_| every_other(&mut count)).collect();
        assert_eq!(fired, [true, false, true, false, true]);
        // Shared: a block take-off after a shot that grunted stays quiet.
        let mut shared = 0;
        assert!(every_other(&mut shared), "the shot grunts");
        assert!(!every_other(&mut shared), "the block jump does not");
    }

    #[test]
    fn court_start_release_preserves_native_controller_order() {
        let Some(mut iso) = iso() else { return };
        let (_, crowd, _) = courts::banks(Some(9));
        let program = Program::parse(&iso.read(crowd).unwrap()).unwrap();
        let module = program.modules.iter().position(|m| m.id == 0xf549).unwrap();
        for delay in [0.0, 5.0, 10.0] {
            let (mut fx, _) = Effects::load(
                &mut iso,
                Some(9),
                7,
                vec![0; 6],
                vec![Some(1); 6],
                &AudioSettings::default(),
            )
            .expect("banks");
            fx.open_court();
            fx.run(delay);
            let before = fx.clock;
            fx.play_starts();
            assert_eq!(fx.clock - before, 10.0 - delay);
            fx.run(10.0);
            let graph = fx.engine.inspect(fx.banks.len() - 2, module).unwrap();
            assert_eq!(graph[40].2[0], 1, "release selects EDGE table value 1");
            assert_eq!(graph[43].2[0], 2, "SUM adds one to the edge value");
            assert_eq!(graph[59].2[0], 32767, "SELECT enables native volume");
            for _ in 0..100 {
                fx.run(10.0);
            }
            let graph = fx.engine.inspect(fx.banks.len() - 2, module).unwrap();
            let inputs = &graph[106].1;
            let expected = inputs.iter().skip(1).fold(inputs[0], |v, x| {
                ((i64::from(v) * i64::from(*x)) / 32767) as i32
            });
            assert_eq!(graph[106].2[0], expected);
            assert!(expected > 0, "native ramp product enables speech");
        }
    }

    /// The commentator's duck, loop 0x24 (`0x1ac9f8`, `FUN_0019ee90`): the
    /// court's crowd bank's mixer (module 62793 of `CRD_RUCK.ABK`, bank
    /// 0x750d) takes the three ducking percents on its inputs 0x9f551,
    /// 0x9f552 and 0xaf535, as a live capture of the original read them at
    /// `0x4baf70..0x4baf78` for every line; a second start keeps the duck
    /// held, and the release clears it.
    #[test]
    fn the_commentators_duck_steers_the_crowd_mixer_with_its_percents() {
        let Some(mut iso) = iso() else { return };
        let (mut fx, _bus) = Effects::load(
            &mut iso,
            Some(9),
            7,
            vec![0; 6],
            vec![Some(1); 6],
            &AudioSettings::default(),
        )
        .expect("banks");
        fx.open_court();
        fx.play_starts();
        for _ in 0..100 {
            fx.run(10.0);
        }
        let (slot, module) = (0..16)
            .find_map(|slot| {
                let p = fx.engine.bank(slot)?;
                (p.bank == 0x750d)
                    .then(|| p.modules.iter().position(|m| m.id == 62793))
                    .flatten()
                    .map(|m| (slot, m))
            })
            .expect("the crowd bank's mixer");
        let nodes: Vec<usize> = [0x9f551, 0x9f552, 0xaf535]
            .iter()
            .map(|&hash| {
                fx.engine.bank(slot).unwrap().modules[module]
                    .nodes
                    .iter()
                    .position(|n| n.hash == hash)
                    .expect("an input")
            })
            .collect();
        let inputs = |fx: &Effects| {
            let cells = fx.engine.inspect(slot, module).expect("the mixer runs");
            nodes.iter().map(|&n| cells[n].2[0]).collect::<Vec<i32>>()
        };
        assert!(!fx.ducked_for_commentary());
        fx.duck_for_commentary();
        assert!(fx.ducked_for_commentary());
        assert_eq!(inputs(&fx), [38, 40, 25]);
        fx.duck_for_commentary();
        fx.run(10.0);
        assert!(fx.ducked_for_commentary());
        fx.commentary_over();
        assert!(!fx.ducked_for_commentary());
        assert_eq!(inputs(&fx), [38, 40, 25], "released with its message");
    }

    /// G03 through the production path: requests from `fire_events`'s
    /// handlers post to the session's speech manager with their acceptance
    /// draw; the next tick's service takes the newest, drops the older one,
    /// chooses through `PLAYERS.EVT` with the same middleware draws an
    /// independent chooser makes, uses the line up, and plays it where the
    /// last speaker stands. With the commentator talking the line is still
    /// chosen and used up, but not played (`0x1ac9f8` type 0).
    #[test]
    fn player_speech_serves_the_newest_request_through_the_shared_middleware() {
        use street_audio::speech::{PlayerCatalog, PlayerRequest};
        let Some(mut iso) = iso() else { return };
        let (mut fx, bus) = Effects::load(
            &mut iso,
            Some(9),
            7,
            vec![0; 6],
            vec![Some(1); 6],
            &AudioSettings::default(),
        )
        .expect("banks");
        fx.open_court();
        fx.play_starts();
        // Four native ramps multiply into the speech control; let the
        // court's fade-in finish rather than override its volume.
        for _ in 0..100 {
            fx.run(10.0);
        }
        bus.0.lock().expect("audio bus").commands.clear();
        let mut speech = crate::audio::SpeechManager::default();
        speech.init(&mut iso, None);
        let mut expected = speech.middleware.random.clone();
        fx.say_launch(Some(&mut speech), 0, (10, 200));
        fx.say(Some(&mut speech), 2, 3, (30, 400));
        assert_eq!(speech.middleware.queue.count(speech::PLAYERS), 2);
        // `FUN_00363a10`: one acceptance draw per request.
        expected.mode0_index(100, u16::MAX);
        expected.mode0_index(100, u16::MAX);
        assert_eq!(speech.middleware.random, expected);
        let reference = speech
            .players
            .clone()
            .select_request(
                PlayerRequest {
                    voice: 1,
                    category_mask: 1 << 3,
                    shot_mask: 0,
                    action_mask: 2,
                },
                &mut expected,
            )
            .expect("a trick line");
        speech.clock += 1;
        let selected = crate::audio::serve_players(&mut speech, Some(&mut fx)).unwrap();
        assert_eq!(selected, reference, "the newest request, chosen natively");
        assert_eq!(speech.middleware.random, expected);
        assert!(
            speech.middleware.queue.is_empty(),
            "the older launch is dropped"
        );
        assert_eq!(speech.speaker, Some((2, (30, 400))));
        let header = speech
            .players
            .headers
            .by_slot(selected.history_key)
            .unwrap();
        assert_eq!(header.recent(), [selected.ordinal]);
        fx.run(200.0);
        let program = Program::parse(&iso.read(speech::PLAYER_BANK).expect("player bank"))
            .expect("native speech program");
        let bank = fx.banks.len() - 1;
        let offset = selected_stream_offset(&program, selected.stream);
        let mut voice = queued_voice(&mut fx, bank, offset);
        assert!(
            voice.gains.iter().any(|gain| *gain > 0.0),
            "the line has gain"
        );
        assert!(
            voice_rms(&mut voice, 500) > 0.0,
            "the selected line is audible"
        );

        speech.commentary_playing = true;
        let started = fx.next;
        fx.say_launch(Some(&mut speech), 0, (10, 200));
        speech.clock += 1;
        let quiet = crate::audio::serve_players(&mut speech, Some(&mut fx)).unwrap();
        assert_eq!(quiet.catalog, PlayerCatalog::Shot);
        assert_eq!(fx.next, started, "no 0x1f while Bobbito talks");
        let shot = speech.players.headers.by_slot(quiet.history_key).unwrap();
        assert_eq!(
            shot.recent(),
            [quiet.ordinal],
            "chosen and used up all the same"
        );

        // A staged script holds a request back before its draw.
        fx.staged = Some((0, 0));
        let before = speech.middleware.random.clone();
        fx.say(Some(&mut speech), 0, 3, (0, 200));
        assert_eq!(speech.middleware.random, before);
        assert!(speech.middleware.queue.is_empty());
    }

    /// The request masks are the raw `jal 0x1ad068` sites' immediates:
    /// `a2` the action mask, `a3` the category bit, `t0` the shot mask.
    #[test]
    fn player_request_masks_are_the_executables_call_site_immediates() {
        let Some(mut iso) = iso() else { return };
        let elf = iso.read("/SLUS_206.51").unwrap();
        let word = |a: u32| street_assets::scripts::executable_words(&elf, a, 1).unwrap()[0];
        // (jal site, handler category) for every caller.
        let sites = [
            (0x1a41cc, 5),
            (0x1a4594, 14),
            (0x1a45b4, 4),
            (0x1a48a4, 14),
            (0x1a4c40, 6),
            (0x1a4e0c, 9),
            (0x1a5ba4, 3),
            (0x1a5c34, 2),
            (0x1a5cd8, 7),
            (0x1a651c, 10),
            (0x1a661c, 0),
            (0x1a6668, 1),
            (0x1a6dd8, 4),
            (0x1a6e18, 8),
        ];
        for (site, category) in sites {
            assert_eq!(word(site), 0x0c00_0000 | (0x1ad068 >> 2), "{site:#x}");
            let mut regs = std::collections::HashMap::new();
            for a in (site - 0x24..=site + 4).step_by(4) {
                let w = word(a);
                let (op, rs, rt) = (w >> 26, (w >> 21) & 31, (w >> 16) & 31);
                if op == 9 && rs == 0 {
                    regs.insert(rt, w & 0xffff);
                } else if op == 0 && rs == 0 && rt == 0 && matches!(w & 0x3f, 0x21 | 0x25 | 0x2d) {
                    regs.insert((w >> 11) & 31, 0); // move rd, zero
                }
            }
            let (a2, a3, t0) = (regs[&6], regs[&7], regs.get(&8).copied().unwrap_or(0));
            assert_eq!(a3, 1 << category, "{site:#x} category");
            assert_eq!(a2, request_mask(category), "{site:#x} action mask");
            if category == 14 {
                assert!(t0 == 1 || t0 == 2, "{site:#x} shot mask");
            } else {
                assert_eq!(t0, 0, "{site:#x} shot mask");
            }
        }
        // The jump-shot launch (`0x1a48a4`, slot 11 kind 2) asks shot mask 1.
        assert_eq!(word(0x1a48a8), 0x2408_0001, "addiu t0, zero, 1");
    }

    /// Slot 76's handler fires the body, the crowd's 0x29 and the grunt,
    /// and the same call makes the spectators gasp at `FUN_001a75a8`'s share.
    #[test]
    fn floor_contact_fires_the_crowds_gasp_with_its_sounds() {
        let Some(mut iso) = iso() else { return };
        let (mut fx, _bus) = Effects::load(
            &mut iso,
            Some(9),
            7,
            vec![0; 6],
            vec![Some(1); 6],
            &AudioSettings::default(),
        )
        .expect("banks");
        fx.open_court();
        fx.play_starts();
        let players = (0..6)
            .map(|k| street_sim::Player::new(k / 3, [0.0, k as f32], 0.0))
            .collect();
        let hoop = |x: f32| street_sim::Hoop {
            rim: [x, 11.75, 0.0],
            rim_radius: 0.75,
        };
        let mut w = street_sim::World::new(
            [hoop(37.0), hoop(-37.0)],
            players,
            street_sim::Rules::default(),
            1,
        );
        w.score = [9, 4];
        let mut crowd = street_render::crowd::CrowdReactions::default();
        let before = fx.next;
        floor_contact(&mut fx, Some(&mut crowd), &w, 4, (0, 200), 250);
        fx.run(50.0);
        assert!(
            fx.next > before,
            "the body, crowd and grunt events start voices"
        );
        assert_eq!(
            crowd.0,
            [street_render::crowd::Reaction {
                fraction: crate::game::crowd_fraction(&w),
                cheer: false,
            }]
        );
    }

    /// The tip-off's take is the one pass while the referee's toss is up.
    #[test]
    fn the_tip_off_take_is_found_once_on_a_real_toss() {
        let players = (0..6)
            .map(|k| {
                street_sim::Player::new(
                    k / 3,
                    [if k < 3 { -2.0 } else { 2.0 }, (k % 3) as f32 * 6.0],
                    0.0,
                )
            })
            .collect();
        let hoop = |x: f32| street_sim::Hoop {
            rim: [x, 11.75, 0.0],
            rim_radius: 0.75,
        };
        let mut w = street_sim::World::new(
            [hoop(37.0), hoop(-37.0)],
            players,
            street_sim::Rules::default(),
            1,
        );
        w.start_opening_toss();
        let mut takes = Vec::new();
        for _ in 0..600 {
            w.step(&[street_sim::Input::default(); 6]);
            takes.extend(w.events.iter().filter(|e| tip_taken(&w, e)).copied());
            if w.opening_toss == street_sim::OpeningToss::None {
                break;
            }
        }
        assert_eq!(takes.len(), 1, "{takes:?}");
        assert!(matches!(takes[0], Event::Pass { .. }));
        assert_eq!(w.opening_toss, street_sim::OpeningToss::None);
    }

    #[test]
    fn the_court_and_its_effects_make_sound() {
        let Some(mut iso) = iso() else {
            eprintln!("STREET_ISO not set; skipping");
            return;
        };
        for court in [Some(9), Some(4), None] {
            let (mut fx, bus) = Effects::load(
                &mut iso,
                court,
                7,
                vec![0; 6],
                vec![Some(1); 6],
                &AudioSettings::default(),
            )
            .expect("banks");
            let mut src = bus.decoder();
            assert!(
                listen(&mut fx, &mut src, 300) < 1e-6,
                "silent before the court opens"
            );
            fx.open_court();
            let hushed = listen(&mut fx, &mut src, 1500);
            fx.play_starts();
            let crowd = listen(&mut fx, &mut src, 2000);
            eprintln!("{court:?}: crowd {hushed} hushed, {crowd} after the tip");
            assert!(crowd > 0.002, "{court:?}: the crowd plays ({crowd})");
            // A dribble, a catch, a dunk's slam: each adds to the crowd.
            for msg in [
                vec![ev::FLOOR, 0, 200, 1000],
                vec![ev::REBOUND, 0, 200],
                vec![ev::SLAM, 0, 1000, 1],
                vec![ev::HUD, 0x2000, 9],
                vec![ev::EXERTION, 0, 200, 1],
                // A block's swing, a reach (slots 7, 8), a body knocked
                // off balance (slot 74).
                vec![ev::SWEETENER, 0, 200, 1000, 11],
                vec![ev::SWEETENER, 0, 200, 1000, 12],
                vec![ev::BODY, 0, 200, 1000, 0],
                // The knockdown and then the contact with the floor,
                // with the crowd's separate reaction (slots 74 and 76).
                vec![ev::BODY, 0, 200, 1000, 1],
                vec![ev::BODY, 0, 200, 250, 2],
                vec![ev::FALL_CROWD],
                vec![ev::SWEETENER, 0, 200, 1000, 5],
            ] {
                let before = fx.next;
                fx.fire(&msg);
                listen(&mut fx, &mut src, 50);
                if court.is_none() && msg[0] == ev::FALL_CROWD {
                    // Street School's event map has no fall crowd voice.
                    assert_eq!(fx.next, before);
                    continue;
                }
                assert!(
                    fx.next > before,
                    "{court:?}: event {:#x} started a voice",
                    msg[0]
                );
            }
            // A player talks: a line of voice 1 from the stream file.
            let before = fx.next;
            let mut speech = crate::audio::SpeechManager::default();
            speech.init(&mut iso, None);
            fx.say(Some(&mut speech), 0, 3, (0, 200));
            speech.clock += 1;
            crate::audio::serve_players(&mut speech, Some(&mut fx));
            let talk = listen(&mut fx, &mut src, 400);
            assert!(fx.next > before, "{court:?}: a player line played");
            eprintln!("{court:?}: with a line {talk}");
            let loud = listen(&mut fx, &mut src, 300);
            eprintln!("{court:?}: with effects {loud}");
            assert!(
                loud > crowd * 0.5,
                "{court:?}: effects over the crowd ({loud} vs {crowd})"
            );
        }
    }
}
