//! Sound: the game's own music, crowds, effects and commentary, decoded from
//! the disc by `street-audio` and played through Bevy's audio.
//!
//! - The menus play the front end's songs (`/MUSIC/FE_MUSIC`), one after
//!   another, and the front end's sounds on every move, change, select and
//!   back, by the indices the game's menu code passes (0x1a1908 callers:
//!   3 move, 2 change, 9 select, 4 back).
//! - A match plays one of the in-game songs (`PATHSONG.CFG`) through its
//!   PathFinder map (`song.rs`) and the commentary by situation
//!   (`SPCHDATS.BIG`'s files are named for them). The effects and the crowd
//!   are the game's own AEMS scripts, run by `sfx.rs`.
//!
//! The menu and commentary choices are traced from the game's code.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use bevy::{
    audio::{
        AddAudioSource, AudioPlayer, AudioSink, AudioSinkPlayback, ChannelCount, Decodable,
        PlaybackSettings, SampleRate, Source, Volume,
    },
    prelude::*,
};
use street_audio::{
    Clip,
    bank::Bank,
    courts, music, schl,
    speech::{self, Commentary, Lines},
};
use street_sim::{Basket, Event, Goal, Phase};

use crate::{
    Screen,
    game::{Paused, Sim, TickEvents},
    menu::MenuInput,
    setup::{Disc, MatchSetup},
    story,
};

/// Options → Audio Options: Mono. Every stereo source plays its two
/// sides mixed while it is set.
pub static MONO: AtomicBool = AtomicBool::new(false);

/// A stereo frame as the output wants it: both sides mixed under Mono.
pub fn mono(l: f32, r: f32) -> [f32; 2] {
    if MONO.load(Ordering::Relaxed) {
        let m = (l + r) * 0.5;
        [m, m]
    } else {
        [l, r]
    }
}

/// Decoded audio, for effects and lines.
#[derive(Asset, TypePath, Clone)]
pub struct Pcm {
    rate: u32,
    channels: u16,
    samples: Arc<[i16]>,
}

impl From<Clip> for Pcm {
    fn from(c: Clip) -> Self {
        Pcm {
            rate: c.rate,
            channels: c.channels,
            samples: c.samples.into(),
        }
    }
}

pub struct PcmSource {
    pcm: Pcm,
    pos: usize,
}

impl Iterator for PcmSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let s = *self.pcm.samples.get(self.pos)?;
        let v = if self.pcm.channels == 2 && MONO.load(Ordering::Relaxed) {
            let at = |k: usize| f32::from(self.pcm.samples.get(k).copied().unwrap_or(0));
            let first = self.pos & !1;
            (at(first) + at(first + 1)) * 0.5
        } else {
            f32::from(s)
        };
        self.pos += 1;
        Some(v / 32768.0)
    }
}

impl Source for PcmSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(self.pcm.channels.max(1)).expect("non-zero")
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(self.pcm.rate.max(1)).expect("non-zero")
    }

    fn total_duration(&self) -> Option<Duration> {
        let frames = self.pcm.samples.len() / self.pcm.channels.max(1) as usize;
        Some(Duration::from_secs_f64(
            frames as f64 / self.pcm.rate.max(1) as f64,
        ))
    }
}

impl Decodable for Pcm {
    type Decoder = PcmSource;

    fn decoder(&self) -> PcmSource {
        PcmSource {
            pcm: self.clone(),
            pos: 0,
        }
    }
}

/// SCHl streams played one after another, forever, decoded as they play:
/// a song's segments, or a crowd bed.
#[derive(Asset, TypePath, Clone)]
pub struct Streamed {
    data: Arc<[u8]>,
    streams: Arc<[schl::Stream]>,
    looping: bool,
}

impl Streamed {
    fn new(data: Vec<u8>, streams: Vec<schl::Stream>) -> Option<Self> {
        if streams.is_empty() {
            return None;
        }
        Some(Streamed {
            data: data.into(),
            streams: streams.into(),
            looping: true,
        })
    }
}

pub struct StreamedSource {
    s: Streamed,
    next: usize,
    reader: Option<schl::Reader>,
    channels: u16,
    rate: u32,
    /// Under Mono, a frame's second (mixed) sample.
    held: Option<f32>,
}

impl Iterator for StreamedSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if let Some(v) = self.held.take() {
            return Some(v);
        }
        let l = self.sample()?;
        if self.channels == 2 && MONO.load(Ordering::Relaxed) {
            let r = self.sample().unwrap_or(l);
            let m = (l + r) * 0.5;
            self.held = Some(m);
            return Some(m);
        }
        Some(l)
    }
}

impl StreamedSource {
    fn sample(&mut self) -> Option<f32> {
        // At most one full pass without a sample: an empty set of streams
        // ends instead of spinning.
        for _ in 0..=self.s.streams.len() {
            if let Some(v) = self.reader.as_mut().and_then(|r| r.next()) {
                return Some(v as f32 / 32768.0);
            }
            if !self.s.looping && self.next >= self.s.streams.len() {
                return None;
            }
            let stream = self.s.streams[self.next % self.s.streams.len()].clone();
            self.next += 1;
            // Every segment of one song shares its format; skip any that
            // does not, rather than play it at the wrong speed.
            if stream.patch.channels != self.channels || stream.patch.rate != self.rate {
                self.reader = None;
                continue;
            }
            self.reader = Some(schl::Reader::new(self.s.data.clone(), stream, false));
        }
        None
    }
}

impl Source for StreamedSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(self.channels.max(1)).expect("non-zero")
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(self.rate.max(1)).expect("non-zero")
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Decodable for Streamed {
    type Decoder = StreamedSource;

    fn decoder(&self) -> StreamedSource {
        let first = &self.streams[0].patch;
        StreamedSource {
            channels: first.channels,
            rate: first.rate,
            s: self.clone(),
            next: 0,
            reader: None,
            held: None,
        }
    }
}

/// Volume levels, 0 to 10, set on the Options page (the original's
/// defaults are all 10: `records::Settings::default`).
#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub struct AudioSettings {
    pub music: u8,
    pub effects: u8,
    pub speech: u8,
}

impl Default for AudioSettings {
    fn default() -> Self {
        let s = street_sim::records::Settings::default();
        Self {
            music: s.music,
            effects: s.effects,
            speech: s.speech,
        }
    }
}

/// A 0..10 level as a gain (squared, which sounds even).
pub(crate) fn gain(level: u8) -> f32 {
    let x = level.min(10) as f32 / 10.0;
    x * x
}

/// Which part of the mix a playing sound belongs to.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Part {
    Music,
    Effect,
    Speech,
    /// The match's scripted effects and crowd (`sfx.rs`), whose scripts set
    /// their own levels from the effects volume.
    Scripted,
}

/// The in-match share of the music: under the crowd and the commentary.
const MATCH_MUSIC: f32 = 0.55;

fn part_gain(part: Part, s: &AudioSettings, in_match: bool) -> f32 {
    match part {
        Part::Music => gain(s.music) * if in_match { MATCH_MUSIC } else { 1.0 },
        Part::Effect => gain(s.effects),
        Part::Speech => gain(s.speech),
        Part::Scripted => 1.0,
    }
}

/// The banks and lines read from the disc, and what is decoded so far.
#[derive(Resource, Default)]
pub struct Sounds {
    front_end: Option<Bank>,
    decoded: HashMap<usize, Handle<Pcm>>,
    commentary: Option<Commentary>,
    lines: HashMap<String, Lines>,
    songs: Vec<music::Song>,
    /// `DAT_003ea23c`: the in-game song, 1-based in `PATHSONG.CFG`'s order
    /// (the Trax word's bit `k + 11`, `audio_path_offset` 8 + 3), -1 when
    /// every one is switched off. It lasts the session.
    path_song: i32,
    /// The audio controller's +0x24: the loading screens' loop
    /// (`music/loadloop`, `FUN_001a0e18`), drawn at the boot. The rewrite
    /// plays no loading loops; it is kept for the C library's order.
    pub(crate) load_loop: u32,
    menu_turn: usize,
    menu_current: Option<usize>,
    /// Trax: the songs switched off and the rotation
    /// ([`Settings::songs_off`](street_sim::records::Settings)).
    songs_off: u32,
    sequential: bool,
    interactive: bool,
    /// The in-game song playing (for Track Info).
    current: Option<music::Song>,
    /// The rewrite's own choices (random songs, prefix commentary): kept
    /// apart from the speech middleware's native sequence.
    rng: u32,
}

impl Sounds {
    /// Whether in-game song `k` (1-based) is switched on.
    fn path_song_on(&self, k: i32) -> bool {
        (1..=32).contains(&k) && self.songs_off & (1 << (k - 1)) == 0
    }

    /// `FUN_001aaf78`: the first song switched on from the current one up
    /// to 15, then from 1 up to it; -1 for none.
    fn first_path_song_on(&self) -> i32 {
        let from = if self.path_song == -1 {
            1
        } else {
            self.path_song
        };
        (from..16)
            .chain(1..from)
            .find(|&k| self.path_song_on(k))
            .unwrap_or(-1)
    }

    /// `DAT_003eb340`: how many songs `pathsong.cfg` registers.
    fn path_songs(&self) -> i32 {
        self.songs.len() as i32
    }

    /// The song `k` and a seed for its map, as the one playing.
    fn path_song(&mut self, k: i32) -> Option<(music::Song, u32)> {
        let song = self.songs.get(usize::try_from(k - 1).ok()?)?.clone();
        self.current = Some(song.clone());
        Some((song, self.roll()))
    }

    /// A match's song (`0x1a0a38`): the session's current one, or when it
    /// is switched off the next one on. The Rucker Classic court (record
    /// 10, `FUN_001a0c48`) makes song 12 current (`FUN_001aa828`) and plays
    /// it, unless the court is the save's (`FUN_001d37b0`, `rucker_owned`)
    /// and song 12 is switched off: then the next one on, or none. (Its
    /// gate on the music manager's +0x1790, set while a song waits to
    /// resume, is clear at a match's start.)
    fn match_song(
        &mut self,
        court: Option<usize>,
        rucker_owned: bool,
    ) -> Option<(music::Song, u32)> {
        if court == Some(10) {
            self.path_song = 12;
            if rucker_owned && !self.path_song_on(12) {
                self.path_song = self.first_path_song_on();
            }
        } else if !self.path_song_on(self.path_song) {
            self.path_song = self.first_path_song_on();
        }
        self.path_song(self.path_song)
    }

    /// `FUN_001aabc8(manager, 0)` when the music moves on: in turn
    /// (`audio_rotate_game_songs` is 1) the next song, or the next one on;
    /// at random (the Trax word's bit 1) a draw of `rand() % count + 1`
    /// that, when it is the song just played or a song switched off, gives
    /// way to the next one on after the song just played. The draw is the
    /// rewrite's own generator.
    fn rotate_song(&mut self) -> Option<(music::Song, u32)> {
        let count = self.path_songs();
        if count == 0 {
            return None;
        }
        let wrap = |k: i32| if k > count { 1 } else { k };
        if self.sequential {
            self.path_song = wrap(self.path_song + 1);
            if !self.path_song_on(self.path_song) {
                self.path_song = self.first_path_song_on();
            }
            return self.path_song(self.path_song);
        }
        let old = self.path_song;
        let next = old + 1;
        self.path_song = wrap(next);
        let on = self.first_path_song_on();
        if on == -1 {
            self.path_song = -1;
            return None;
        }
        let (pick, last) = if on == old {
            (on, next)
        } else {
            ((self.roll() % count as u32) as i32 + 1, old)
        };
        self.path_song = pick;
        if pick == last {
            self.path_song = wrap(last + 1);
            self.path_song = self.first_path_song_on();
        }
        if !self.path_song_on(self.path_song) {
            self.path_song = wrap(last + 1);
            self.path_song = self.first_path_song_on();
        }
        self.path_song(self.path_song)
    }

    /// Trax's choices, from the options.
    pub fn trax(&mut self, songs_off: u32, sequential: bool, interactive: bool) {
        self.songs_off = songs_off;
        self.sequential = sequential;
        self.interactive = interactive;
    }

    /// The in-game songs, in `PATHSONG.CFG`'s order.
    pub fn songs(&self) -> &[music::Song] {
        &self.songs
    }

    /// The in-game song playing now.
    pub fn now_playing(&self) -> Option<&music::Song> {
        self.current.as_ref()
    }

    /// The front end's next song Trax allows.
    fn next_menu_song(&mut self) -> Option<&'static str> {
        let n = music::FRONT_END_SONGS.len();
        let bit = street_sim::records::FRONT_END_BIT;
        let enabled: Vec<_> = story::FRONT_END_ORDER
            .into_iter()
            .filter(|&k| self.songs_off & (1 << (bit + k as u32)) == 0)
            .collect();
        if enabled.is_empty() {
            return None;
        }
        let pick = if self.sequential {
            (0..n)
                .map(|offset| story::FRONT_END_ORDER[(self.menu_turn + offset) % n])
                .find(|k| enabled.contains(k))
                .unwrap_or(enabled[0])
        } else {
            let candidates: Vec<_> = enabled
                .iter()
                .copied()
                .filter(|&k| enabled.len() == 1 || Some(k) != self.menu_current)
                .collect();
            candidates[self.roll() as usize % candidates.len()]
        };
        self.menu_turn = story::FRONT_END_ORDER
            .iter()
            .position(|&k| k == pick)
            .unwrap_or(0)
            + 1;
        self.menu_current = Some(pick);
        Some(music::FRONT_END_SONGS[pick])
    }

    fn roll(&mut self) -> u32 {
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    /// The front end's decoded sound `index`.
    fn clip(&mut self, pcm: &mut Assets<Pcm>, index: usize) -> Option<Handle<Pcm>> {
        if let Some(h) = self.decoded.get(&index) {
            return Some(h.clone());
        }
        let clip = self.front_end.as_ref()?.clip(index).ok()?;
        let h = pcm.add(Pcm::from(clip));
        self.decoded.insert(index, h.clone());
        Some(h)
    }
}

/// The front end's menu sounds: the indices the game's menu code passes.
pub mod cue {
    pub const MENU_MOVE: usize = 3;
    pub const MENU_CHANGE: usize = 2;
    pub const MENU_SELECT: usize = 9;
    pub const MENU_BACK: usize = 4;
}

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Pcm>()
            .add_audio_source::<Streamed>()
            .add_audio_source::<crate::song::PathSong>()
            .init_resource::<AudioSettings>()
            .init_resource::<Sounds>()
            .init_resource::<SpeechManager>()
            .init_resource::<Narration>()
            .init_resource::<MusicDuck>()
            .init_resource::<MenuSongCue>()
            .add_systems(Startup, load_front_end)
            .add_systems(FixedUpdate, speech_clock.before(crate::game::SimStep))
            .add_systems(OnEnter(Screen::Menu), first_menu_song)
            .add_systems(Update, menu_sounds.run_if(in_state(Screen::Menu)))
            .add_systems(Update, rotate_menu_music.run_if(in_state(Screen::Menu)))
            .init_resource::<Preview>()
            .add_systems(Update, preview_menu_song.run_if(in_state(Screen::Menu)))
            .add_systems(OnEnter(Screen::Playing), match_audio)
            .add_systems(
                FixedUpdate,
                (
                    match_sounds.before(crate::sfx::fire_events),
                    speech_service
                        .after(match_sounds)
                        .after(crate::sfx::fire_events),
                    crate::song::cues,
                    next_song.after(crate::song::cues),
                    playlist_music.after(next_song),
                )
                    .after(crate::game::SimStep)
                    .run_if(in_state(Screen::Playing)),
            )
            .add_systems(Update, (pause_audio, apply_volumes))
            .add_systems(Update, duck_music.run_if(in_state(Screen::Playing)));
    }
}

pub(crate) fn load_front_end(
    mut disc: ResMut<Disc>,
    mut sounds: ResMut<Sounds>,
    mut manager: ResMut<SpeechManager>,
    session: Option<ResMut<crate::session::Session>>,
) {
    let iso = &mut disc.iso;
    sounds.rng = 0x9e37_79b9;
    match iso.read(courts::FRONT_END).map(Bank::parse) {
        Ok(Ok(bank)) => sounds.front_end = Some(bank),
        Ok(Err(e)) | Err(e) => warn!("no menu sounds: {e}"),
    }
    match iso.read(music::PATHSONG) {
        Ok(cfg) => sounds.songs = music::songs(&String::from_utf8_lossy(&cfg)),
        Err(e) => warn!("no song list: {e}"),
    }
    match Commentary::open(iso) {
        Ok(c) => sounds.commentary = Some(c),
        Err(e) => warn!("no commentary: {e}"),
    }
    sounds.menu_turn = sounds.roll() as usize;
    // The boot's C library draws, in its order (`session.rs`): the audio
    // controller's constructor (`FUN_0019f810`: the loading loop, then the
    // in-game song `rand() % 13 + 1`), then `AuController::Init`'s end, the
    // speech manager's counter (`0x1ac5e0`: `rand() % 255` straight after
    // the path manager's `srand(timer)`).
    let mut own = None;
    let session = match session {
        Some(session) => session.into_inner(),
        None => own.insert(crate::session::Session::pinned(2002)),
    };
    let (load_loop, song) = session.audio_controller();
    sounds.load_loop = load_loop;
    sounds.path_song = song;
    manager.inits = session.speech_counter();
    // The front end's first initialisation (`FUN_0019fbb8`, songs not yet
    // registered): the speech manager's first `0x1ac6f0` seeds the
    // middleware with the counter, registers the headers and counts up,
    // then `0x1ac988` shuts it down until a match starts it again.
    let commentary = sounds.commentary.as_mut();
    manager.init(iso, commentary);
    manager.shut_down();
}

/// Reads a whole SCHl file into a looping player.
fn streamed(disc: &mut Disc, path: &str) -> Option<Streamed> {
    let data = disc.iso.read(path).map_err(|e| warn!("{path}: {e}")).ok()?;
    let streams = schl::streams(&data)
        .map_err(|e| warn!("{path}: {e}"))
        .ok()?;
    Streamed::new(data, streams)
}

/// Whole songs end so the rotation can advance; crowd beds keep looping.
fn whole_song(disc: &mut Disc, path: &str) -> Option<Streamed> {
    let mut song = streamed(disc, path)?;
    song.looping = false;
    Some(song)
}

#[derive(Component)]
struct MenuSong;

/// Trax's Preview (Cross on a song's row): a front-end song to play in
/// the menu's place now, the rotation going on after it.
#[derive(Resource, Default)]
pub struct Preview(pub Option<&'static str>);

fn preview_menu_song(
    mut commands: Commands,
    mut disc: ResMut<Disc>,
    mut preview: ResMut<Preview>,
    mut streams: ResMut<Assets<Streamed>>,
    settings: Res<AudioSettings>,
    playing: Query<Entity, With<MenuSong>>,
) {
    let Some(path) = preview.0.take() else {
        return;
    };
    let Some(song) = whole_song(&mut disc, path) else {
        return;
    };
    for entity in &playing {
        commands.entity(entity).despawn();
    }
    commands.spawn((
        AudioPlayer(streams.add(song)),
        PlaybackSettings::ONCE.with_volume(Volume::Linear(part_gain(
            Part::Music,
            &settings,
            false,
        ))),
        Part::Music,
        MenuSong,
        DespawnOnExit(Screen::Menu),
    ));
}

fn menu_music(
    mut commands: Commands,
    mut disc: ResMut<Disc>,
    mut sounds: ResMut<Sounds>,
    mut streams: ResMut<Assets<Streamed>>,
    settings: Res<AudioSettings>,
) {
    let Some(path) = sounds.next_menu_song() else {
        return;
    };
    let Some(song) = whole_song(&mut disc, path) else {
        return;
    };
    commands.spawn((
        AudioPlayer(streams.add(song)),
        PlaybackSettings::ONCE.with_volume(Volume::Linear(part_gain(
            Part::Music,
            &settings,
            false,
        ))),
        Part::Music,
        MenuSong,
        DespawnOnExit(Screen::Menu),
    ));
}

/// The front end's song held off: the idle demo's turn to the title stops
/// it (`FUN_0019fd60(.., -5, 0)`: `FUN_001a8da0` ends the song's stream)
/// for the intro the title comes back with, and the next song starts as
/// the intro ends (the runtime's song turned at its last second).
#[derive(Resource, Default)]
pub struct MenuSongCue(pub bool);

/// The front end's song as the menus open, unless a transition's movies
/// hold it ([`crate::transition::Transition::holds_song`]: the boot's
/// start it with the zoom).
fn first_menu_song(
    commands: Commands,
    disc: ResMut<Disc>,
    sounds: ResMut<Sounds>,
    streams: ResMut<Assets<Streamed>>,
    settings: Res<AudioSettings>,
    transition: Option<Res<crate::transition::Transition>>,
) {
    if transition.is_some_and(|t| t.holds_song()) {
        return;
    }
    menu_music(commands, disc, sounds, streams, settings);
}

#[allow(clippy::too_many_arguments)]
fn rotate_menu_music(
    mut commands: Commands,
    disc: ResMut<Disc>,
    sounds: ResMut<Sounds>,
    streams: ResMut<Assets<Streamed>>,
    settings: Res<AudioSettings>,
    playing: Query<(Entity, Option<&AudioSink>), With<MenuSong>>,
    held: Res<MenuSongCue>,
    transition: Option<Res<crate::transition::Transition>>,
) {
    // Held for the idle demo's turn, and while a transition's movies hold
    // it (the title's until the zoom, a DVD page's to their end): stopped,
    // and the next song starts as the hold ends.
    if held.0 || transition.is_some_and(|t| t.holds_song()) {
        for (entity, _) in &playing {
            commands.entity(entity).despawn();
        }
        return;
    }
    if !playing.is_empty()
        && !playing
            .iter()
            .any(|(_, sink)| sink.is_some_and(|s| s.empty()))
    {
        return;
    }
    for (entity, _) in &playing {
        commands.entity(entity).despawn();
    }
    menu_music(commands, disc, sounds, streams, settings);
}

fn menu_sounds(
    input: MenuInput,
    mut commands: Commands,
    mut sounds: ResMut<Sounds>,
    mut pcm: ResMut<Assets<Pcm>>,
    settings: Res<AudioSettings>,
    scripted: Option<ResMut<crate::sfx::MenuSfx>>,
    menus: Option<ResMut<crate::menu::Menus>>,
) {
    // A sound the screen plays itself (Select's question as it opens).
    let cue = menus.and_then(|mut m| m.cue.take());
    let index = if let Some(cue) = cue {
        cue
    } else if input.back() {
        cue::MENU_BACK
    } else if input.confirm() {
        cue::MENU_SELECT
    } else if input.up() || input.down() {
        cue::MENU_MOVE
    } else if input.left() || input.right() {
        cue::MENU_CHANGE
    } else {
        return;
    };
    // Through the front end's scripts when they loaded (each sound at its
    // own level), else straight from the bank.
    if let Some(mut scripted) = scripted {
        scripted.play(index);
        return;
    }
    let Some(handle) = sounds.clip(&mut pcm, index) else {
        return;
    };
    commands.spawn((
        AudioPlayer(handle),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(part_gain(
            Part::Effect,
            &settings,
            false,
        ))),
        Part::Effect,
    ));
}

/// The match's song now playing.
#[derive(Component)]
struct MatchSong;

#[derive(Component)]
struct PlaylistSong;

/// The match's in-game song (`0x1a0a38`), played through its PathFinder
/// map (`song.rs`); without one, its segments in a loop.
fn start_song(
    commands: &mut Commands,
    disc: &mut Disc,
    sounds: &mut Sounds,
    (streams, path_songs): (&mut Assets<Streamed>, &mut Assets<crate::song::PathSong>),
    settings: &AudioSettings,
    (court, rucker_owned): (Option<usize>, bool),
) {
    if !sounds.interactive {
        commands.remove_resource::<crate::song::MusicLink>();
        let Some(path) = sounds.next_menu_song() else {
            return;
        };
        let Some(main) = whole_song(disc, path) else {
            return;
        };
        let index = music::FRONT_END_SONGS
            .iter()
            .position(|&p| p == path)
            .unwrap_or(0);
        let name = disc
            .radio
            .get(story::front_end_title(index))
            .cloned()
            .unwrap_or_default();
        let mut lines = name.split("\\n");
        sounds.current = Some(music::Song {
            folder: String::new(),
            artist_speech: 0,
            title: lines.next().unwrap_or("").to_owned(),
            artist: lines.next().unwrap_or("").to_owned(),
            album: String::new(),
        });
        commands.spawn((
            AudioPlayer(streams.add(main)),
            PlaybackSettings::ONCE.with_volume(Volume::Linear(part_gain(
                Part::Music,
                settings,
                true,
            ))),
            Part::Music,
            MatchSong,
            PlaylistSong,
            DespawnOnExit(Screen::Playing),
        ));
        return;
    }
    let Some((song, seed)) = sounds.match_song(court, rucker_owned) else {
        return;
    };
    if let Some((path_song, link)) = crate::song::load(&mut disc.iso, &song, seed) {
        info!("audio: song {}", song.title);
        spawn_song(commands, path_songs, settings, path_song, link);
    } else if let Some(main) = streamed(disc, &song.main()) {
        commands.spawn((
            AudioPlayer(streams.add(main)),
            song_playback(settings),
            Part::Music,
            MatchSong,
            DespawnOnExit(Screen::Playing),
        ));
    }
}

fn song_playback(settings: &AudioSettings) -> PlaybackSettings {
    PlaybackSettings::LOOP.with_volume(Volume::Linear(part_gain(Part::Music, settings, true)))
}

fn spawn_song(
    commands: &mut Commands,
    path_songs: &mut Assets<crate::song::PathSong>,
    settings: &AudioSettings,
    path_song: crate::song::PathSong,
    link: crate::song::MusicLink,
) {
    commands.insert_resource(link);
    commands.spawn((
        AudioPlayer(path_songs.add(path_song)),
        song_playback(settings),
        Part::Music,
        MatchSong,
        DespawnOnExit(Screen::Playing),
    ));
}

/// A song being read on a worker thread, with its own handle on the disc.
type LoadedSong = (String, crate::song::PathSong, crate::song::MusicLink);
type Loading = Arc<std::sync::Mutex<Option<Result<LoadedSong, ()>>>>;

/// Swaps in the next song when the music asks for it (`song::cues`). The
/// old one has stopped; it is read off the main thread (about 50 ms), and
/// takes over straight into its groove.
fn next_song(
    mut commands: Commands,
    mut link: Option<ResMut<crate::song::MusicLink>>,
    playing: Query<Entity, With<MatchSong>>,
    disc_path: Option<Res<crate::setup::DiscPath>>,
    mut sounds: ResMut<Sounds>,
    mut path_songs: ResMut<Assets<crate::song::PathSong>>,
    settings: Res<AudioSettings>,
    sim: Res<crate::game::Sim>,
    mut loading: Local<Option<(Entity, Loading, Option<music::Song>)>>,
) {
    if !sounds.interactive {
        *loading = None;
        return;
    }
    if let Some((owner, slot, previous)) = loading.as_ref() {
        // A rematch or a return from the menus must discard the old worker.
        if !playing.contains(*owner) {
            *loading = None;
            return;
        }
        let done = slot.lock().ok().and_then(|mut s| s.take());
        if let Some(done) = done {
            let previous = previous.clone();
            *loading = None;
            let Ok((title, path_song, mut next_link)) = done else {
                if let Some(link) = link.as_mut() {
                    link.next = false;
                    link.opened(sim.world.tick, true);
                }
                sounds.current = previous;
                return;
            };
            for e in &playing {
                commands.entity(e).despawn();
            }
            next_link.opened(sim.world.tick, true);
            info!("audio: next song {title}");
            spawn_song(
                &mut commands,
                &mut path_songs,
                &settings,
                path_song,
                next_link,
            );
        }
        return;
    }
    if !link.as_ref().is_some_and(|l| l.next) {
        return;
    }
    let previous = sounds.current.clone();
    let (Some(path), Some((song, seed))) = (disc_path, sounds.rotate_song()) else {
        return;
    };
    let Some(owner) = playing.iter().next() else {
        return;
    };
    let slot = Loading::default();
    let out = slot.clone();
    let path = path.0.clone();
    std::thread::spawn(move || {
        let loaded = street_assets::iso::Iso::open(&path)
            .map_err(|e| warn!("{}: {e}", path.display()))
            .ok()
            .and_then(|mut iso| crate::song::load(&mut iso, &song, seed));
        if let Ok(mut out) = out.lock() {
            *out = Some(
                loaded
                    .map(|(path_song, link)| (song.title, path_song, link))
                    .ok_or(()),
            );
        }
    });
    *loading = Some((owner, slot, previous));
}

/// Playlist uses the same full songs as the front end (`FUN_001a0208`).
/// Switching Music Experience in pause also swaps the active player.
fn playlist_music(
    mut commands: Commands,
    mut disc: ResMut<Disc>,
    mut sounds: ResMut<Sounds>,
    mut streams: ResMut<Assets<Streamed>>,
    mut path_songs: ResMut<Assets<crate::song::PathSong>>,
    settings: Res<AudioSettings>,
    setup: Res<MatchSetup>,
    save: Option<Res<crate::story::Save>>,
    playing: Query<(Entity, Option<&AudioSink>, Has<PlaylistSong>), With<MatchSong>>,
) {
    let swap = playing.is_empty()
        || playing.iter().any(|(_, sink, playlist)| {
            playlist == sounds.interactive || (playlist && sink.is_some_and(|s| s.empty()))
        });
    if !swap {
        return;
    }
    for (entity, _, _) in &playing {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<crate::song::MusicLink>();
    let court = court_of(&disc, &setup, save.as_deref());
    start_song(
        &mut commands,
        &mut disc,
        &mut sounds,
        (&mut streams, &mut path_songs),
        &settings,
        court,
    );
}

/// The match's court as its `XDBCOURT` record (profile +0xcc), and
/// whether the Rucker Classic court is the save's (`FUN_001d37b0`).
fn court_of(
    disc: &Disc,
    setup: &MatchSetup,
    save: Option<&crate::story::Save>,
) -> (Option<usize>, bool) {
    let court = disc
        .courts
        .iter()
        .position(|c| c.id.eq_ignore_ascii_case(&setup.court));
    let owned = save.is_some_and(|s| crate::rewards::court_unlocked(&s.story, disc, 10));
    (court, owned)
}

fn match_audio(
    mut commands: Commands,
    mut disc: ResMut<Disc>,
    mut sounds: ResMut<Sounds>,
    mut manager: ResMut<SpeechManager>,
    (mut streams, mut path_songs): (
        ResMut<Assets<Streamed>>,
        ResMut<Assets<crate::song::PathSong>>,
    ),
    settings: Res<AudioSettings>,
    setup: Res<MatchSetup>,
    save: Option<Res<crate::story::Save>>,
) {
    manager.init(&mut disc.iso, sounds.commentary.as_mut());

    commands.remove_resource::<crate::song::MusicLink>();
    let court = court_of(&disc, &setup, save.as_deref());
    start_song(
        &mut commands,
        &mut disc,
        &mut sounds,
        (&mut streams, &mut path_songs),
        &settings,
        court,
    );
}

/// The game's speech manager (`AuSpeechManager`, the singleton `0x1ac598`
/// returns). It lasts the session, as on the PS2: the middleware's random
/// sequence and queue, and the headers' rings and runs, carry on from one
/// match to the next.
#[derive(Resource, Default)]
pub struct SpeechManager {
    pub middleware: speech::SpeechMiddleware,
    /// The players' headers and event (`plyrhdrs.big`, `players.evt`).
    pub players: speech::PlayerSpeech,
    /// `param_1[1]`: seeds the generator at the first initialisation and
    /// picks the partitioned headers' run at every one, counting up. The
    /// constructor (`0x1ac5e0`) starts it at `rand() % 255`, the C library's
    /// first draw after `AuController::Init`'s `srand(timer)`
    /// (`session::Session::speech_counter`; 18 on every captured boot).
    inits: u32,
    /// `param_1[2]`: the header table is registered (the first
    /// initialisation took place).
    initialised: bool,
    /// `*param_1`: the manager is up (`0x1ac6f0` sets it, `0x1ac988` clears
    /// it); requests need it.
    up: bool,
    /// `param_1[5]` (+0x14): Bobbito's stream is playing.
    pub commentary_playing: bool,
    /// `param_1[0x1a]` (+0x68): set by the initialisation, cleared by
    /// listener slot 22 (`0x1a5d10`) as the tip-off is caught. While it is
    /// set, a new commentary line starts over the playing one (`0x1ac9f8`).
    pub override_busy: bool,
    /// `param_1[0x12]` (+0x48): the last player who asked to talk, and where
    /// they stood; the line plays there.
    pub speaker: Option<(usize, (i32, i32))>,
    /// The listener's session counters: `DAT_004bb2d4` (steals),
    /// `DAT_004bb2e0` (shot clock violations), `DAT_004bb2d8` (rebounds).
    pub steals: u32,
    pub violations: u32,
    pub rebounds: u32,
    /// The queue's clock (`FUN_00367128`): `0x1ac6f0` registers
    /// `FUN_003998c0`, the 100 Hz timer's count, as the middleware's clock
    /// (its callback word 2), so it counts hundredths of a second: the
    /// roots' hold of 200 is two seconds. The rewrite reads
    /// [`SpeechManager::hundredths`] of its fixed ticks, which run in every
    /// state as the timer does.
    pub clock: u32,
    /// Fixed ticks since the session began, for the clock.
    ticks: u64,
    /// `param_1[0x17]` (+0x5c): speech paused (`FUN_001ad958`, cleared by
    /// `FUN_001ad9d0`). The players' requests are refused before their
    /// draw and both callbacks drop a chosen line.
    pub paused: bool,
    /// `DAT_004bb2c0`: the session's baskets (`0x1a3b10`), every fourth of
    /// which may call the score.
    pub baskets: u32,
    /// The listener's per-match state (`0x1a30a8`): +4 the kind of the
    /// last gamebreaker basket (2 at first), read by `0x1a6b80`; +0x4c how
    /// many scripts have ended (slot 61).
    pub breaker_kind: u32,
    pub scripts_ended: u32,
    /// The last shot or dunk let go, and how (a miss's handler, slot 27,
    /// reads the ball's shooter).
    pub last_shot: Option<(usize, ShotKind)>,
    /// The listener's +0x74 (`0x1a56b0`, at slot 61's first call): fewer
    /// than two of the six players are legends or stars, after which
    /// `FUN_001a7948` and `FUN_001a7bd0` send 0 and no dunker is called by
    /// name.
    pub name_words_off: bool,
}

/// What a shot left the hands as: the ball's kind (`ball+0x184`, set by
/// the shot release `0x14aaf8`): 2 a jump shot, 5 a lay-up, 4 a dunk
/// (the dunker's +0xa4 bit 7), 3 an alley-oop's finish (bit 15). The
/// rewrite's own reading: the move flag 0x800 that makes any dunk 4 is not
/// consulted, as in `tape.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotKind {
    Jumper,
    Layup,
    Dunk,
    AlleyOop,
}

impl ShotKind {
    /// The ball's kind word.
    pub fn word(self) -> u32 {
        match self {
            ShotKind::Jumper => 2,
            ShotKind::Layup => 5,
            ShotKind::Dunk => 4,
            ShotKind::AlleyOop => 3,
        }
    }

    /// A basket's kind, as the score's record carries it (+0x10).
    pub fn of(basket: Basket) -> Self {
        match basket {
            Basket::Layup => ShotKind::Layup,
            Basket::Dunk { .. } => ShotKind::Dunk,
            Basket::AlleyOop { .. } => ShotKind::AlleyOop,
            Basket::Jumper { .. } | Basket::Gamebreaker { .. } => ShotKind::Jumper,
        }
    }

    /// The shot an event lets go, if any: a launch, or a dunker's slam
    /// (a leaper's an alley-oop's, `street_sim::leap`).
    pub fn released(event: &Event, world: &street_sim::World) -> Option<(usize, Self)> {
        match *event {
            Event::Shot { player, .. } => Some((player, ShotKind::Jumper)),
            Event::Layup { player } => Some((player, ShotKind::Layup)),
            Event::Slam { player, .. } => {
                let oop = world.players.get(player).is_some_and(|p| p.leaping());
                Some((
                    player,
                    if oop {
                        ShotKind::AlleyOop
                    } else {
                        ShotKind::Dunk
                    },
                ))
            }
            _ => None,
        }
    }
}

impl SpeechManager {
    /// `0x1ac6f0`, at each match's audio start (`0x1a0a38`): the first time
    /// seeds the generator (`FUN_00362f18`) and registers the headers, which
    /// takes the partitions' run (`FUN_00363060`); later times only run the
    /// partitions again (`FUN_00363230`). Then the counter moves on, the
    /// commentator is quiet and may be interrupted until the tip-off.
    pub fn init<R: std::io::Read + std::io::Seek>(
        &mut self,
        iso: &mut street_assets::iso::Iso<R>,
        commentary: Option<&mut Commentary>,
    ) {
        if !self.initialised {
            self.middleware = speech::SpeechMiddleware::new(self.inits);
            match speech::PlayerSpeech::load(iso) {
                Ok(players) => self.players = players,
                Err(e) => warn!("player speech: {e}"),
            }
            self.initialised = true;
        }
        self.players.headers.partition(self.inits);
        if let Some(commentary) = commentary {
            commentary.partition(self.inits);
        }
        self.inits = self.inits.wrapping_add(1);
        self.up = true;
        self.commentary_playing = false;
        self.override_busy = true;
        self.speaker = None;
        self.paused = false;
        // The listener's own reset (`0x1a30a8`).
        self.breaker_kind = 2;
        self.scripts_ended = 0;
        self.last_shot = None;
        self.name_words_off = false;
    }

    /// `0x1ac988`: the manager is shut down (the stream freed); requests wait
    /// for the next initialisation.
    pub fn shut_down(&mut self) {
        self.up = false;
    }

    /// The speech counter (`param_1[1]`).
    pub fn counter(&self) -> u32 {
        self.inits
    }

    /// One fixed tick: the clock reads the timer's hundredths
    /// (`FUN_003998c0`) at the tick.
    pub fn tick(&mut self) {
        self.ticks += 1;
        self.clock = Self::hundredths(self.ticks);
    }

    /// The 100 Hz timer's count after `ticks` updates at 60 Hz.
    pub fn hundredths(ticks: u64) -> u32 {
        (ticks * 100 / u64::from(street_sim::TICK_HZ)) as u32
    }

    /// `FUN_001ad958(manager, stream, clear)`: speech paused, the queue
    /// (and the runs) cleared when asked.
    pub fn pause(&mut self, clear: bool) {
        if clear {
            self.middleware.clear();
        }
        self.paused = true;
    }

    /// `FUN_001ad9d0(manager, stream, clear)`: speech resumed.
    pub fn resume(&mut self, clear: bool) {
        if clear {
            self.middleware.clear();
        }
        self.paused = false;
    }

    pub fn initialised(&self) -> bool {
        self.initialised
    }

    /// `0x1ad178`'s gates before a commentary request: an initialised
    /// manager, no staged script (only event 3 passes those), no Street
    /// School (game +0x9c = 4), and the interactive Music Experience
    /// (`FUN_001a0db8`: the profile's Trax word bit 0 clear, Playlist,
    /// silences the commentator).
    pub fn commentary_open(&self, sounds: &Sounds, scripted: bool, school: bool) -> bool {
        self.up && !scripted && !school && sounds.interactive
    }

    /// `0x1ad068`'s post of event 0x2000 for `player`'s `voice`: the masks
    /// are its fourth, fifth and third arguments.
    pub fn request_player(
        &mut self,
        player: usize,
        voice: Option<u16>,
        at: (i32, i32),
        masks: [u32; 3],
    ) {
        // `0x1ad068`: no request while paused (+0x5c).
        if !self.up || self.paused {
            return;
        }
        let Some(event) = self.players.event().cloned() else {
            return;
        };
        self.speaker = Some((player, at));
        let request = speech::PlayerRequest {
            voice: voice.unwrap_or(0),
            category_mask: masks[0],
            shot_mask: masks[1],
            action_mask: masks[2],
        };
        let clock = self.clock;
        self.middleware
            .post(&event, speech::PLAYERS, clock, request.packet());
    }

    /// `0x1ad178`'s post of a commentary event. Event 6 (the end) first
    /// sets the override (+0x68), so the closing line plays over a call.
    pub fn request_commentary(&mut self, commentary: &Commentary, packet: speech::Packet) {
        let Some(event) = commentary.event(packet[0] as u16) else {
            return;
        };
        if event.id == 6 {
            self.override_busy = true;
        }
        let clock = self.clock;
        self.middleware
            .post(event, speech::COMMENTARY, clock, packet);
    }
}

#[derive(Component)]
struct CommentaryVoice;

/// The commentary callback's stream (`0x1ac9f8` packet type 1): the line
/// at `offset` in `entry`.
fn play_commentary(
    commands: &mut Commands,
    disc: &mut Disc,
    sounds: &mut Sounds,
    pcm: &mut Assets<Pcm>,
    settings: &AudioSettings,
    entry: street_assets::big::BigEntry,
    offset: u32,
) -> bool {
    if !sounds.lines.contains_key(&entry.name) {
        let Some(commentary) = &sounds.commentary else {
            return false;
        };
        match commentary.lines(&mut disc.iso, &entry) {
            Ok(lines) => {
                sounds.lines.insert(entry.name.clone(), lines);
            }
            Err(e) => {
                warn!("{}: {e}", entry.name);
                return false;
            }
        }
    }
    let count = sounds.lines.get(&entry.name).map_or(0, Lines::len);
    if count == 0 {
        return false;
    }
    let line = sounds.lines[&entry.name]
        .streams
        .iter()
        .position(|s| s.start == offset as usize);
    let Some(clip) = line.and_then(|line| sounds.lines.get(&entry.name)?.clip(line)) else {
        return false;
    };
    commands.spawn((
        AudioPlayer(pcm.add(Pcm::from(clip))),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(part_gain(
            Part::Speech,
            settings,
            true,
        ))),
        Part::Speech,
        CommentaryVoice,
        DespawnOnExit(Screen::Playing),
    ));
    true
}

/// Street School's narration (`0x1ad510`, the sound listener's slots 78, 79
/// and 82): what the School posted since the speech manager's last update,
/// and the manager's waits. The School reports its launch, its results'
/// commits and its tutorial pages; [`speech_service`] asks for the lines.
#[derive(Resource, Debug, Default)]
pub struct Narration {
    /// The lesson (the game's vt+0xb0).
    lesson: usize,
    /// The singleton `0x19f7c8`'s +0x28: the tutorial page due. The launch
    /// (`FUN_0018ec28`, `0x18ecd4..0x18ed28`) notes the lesson's first page
    /// unless it is the Player Indicator's, `FUN_0018f540` the Shot Clock's
    /// (`0x18f68c`); the page closing clears it (`0x251328`).
    page_due: Option<u8>,
    /// Posted since the last update, in order.
    posts: Vec<speech::Lesson>,
    /// Manager +0x58: the introduction waits.
    intro_waits: bool,
    /// Manager +0x54: the page's reading waits.
    page_waits: bool,
}

impl Narration {
    /// Slot 78: lesson `lesson` launched, `page` its tutorial page due.
    pub fn launch(&mut self, lesson: usize, page: Option<u8>) {
        self.lesson = lesson;
        self.page_due = page.filter(|&p| p != street_sim::school::page::PLAYER_INDICATOR);
        self.posts.push(speech::Lesson::Intro);
    }

    /// Slot 79: the lesson's result committed.
    pub fn result(&mut self, passed: bool) {
        self.posts.push(speech::Lesson::Result { passed });
    }

    /// Slot 82: tutorial page `page` opened.
    pub fn page(&mut self, page: u8) {
        if page == street_sim::school::page::SHOT_CLOCK {
            self.page_due = Some(page);
        }
        self.posts.push(speech::Lesson::Page);
    }

    /// The page closed (`0x251328`: Cross, message 35).
    pub fn page_closed(&mut self) {
        self.page_due = None;
    }

    /// `0x1ad510(manager, kind)` with the commentator `busy` (the stream's
    /// status, `FUN_00351f30`): event 9's packet. A wait posts word 3 = 0,
    /// which matches no line, and the manager's update asks again. The
    /// gates on `0x3f9ae8`, `0x3f9bc8` and the one-shot +0x50 are not
    /// modelled (none is set in a lesson).
    fn request(&mut self, kind: speech::Lesson, busy: bool) -> speech::Packet {
        use speech::Lesson;
        let mut index = self.lesson;
        let mut waiting = false;
        match kind {
            Lesson::Intro => {
                waiting = busy || self.page_due.is_some();
                self.intro_waits = waiting;
            }
            Lesson::Result { .. } => {}
            Lesson::Page => {
                if !self.page_waits || busy {
                    self.page_waits = true;
                    waiting = true;
                } else {
                    self.page_waits = false;
                    index = self.page_due.map_or(0, usize::from);
                }
            }
        }
        let mut packet = speech::lesson_packet(index, kind);
        if waiting {
            packet[3] = 0;
        }
        packet
    }
}

/// The speech queue's clock: the timer's hundredths at each fixed tick, in
/// every state, as the PS2's timer counts from the boot.
fn speech_clock(mut manager: ResMut<SpeechManager>) {
    manager.tick();
}

/// The speech manager's per-frame update (`0x1acf58`): the players' queue
/// (`FUN_00364620(0)`), then the commentary's (`FUN_00364620(1)`); each
/// serves its newest request through the event chooser, uses the chosen
/// line up and hands it to the callback (`0x1ac9f8`), which plays it or
/// drops it. Both draw from the one middleware generator, so a frame that
/// serves both takes the players' draws first. Last, a finished commentary
/// stream frees the commentator.
#[allow(clippy::too_many_arguments)]
fn speech_service(
    mut commands: Commands,
    mut disc: ResMut<Disc>,
    mut sounds: ResMut<Sounds>,
    mut manager: ResMut<SpeechManager>,
    mut pcm: ResMut<Assets<Pcm>>,
    settings: Res<AudioSettings>,
    voices: Query<Entity, With<CommentaryVoice>>,
    (mut fx, mut duck, run): (
        Option<ResMut<crate::sfx::Effects>>,
        Option<ResMut<MusicDuck>>,
        Option<Res<story::Running>>,
    ),
    (mut log, sim, mut narration): (
        Option<ResMut<crate::game::EventLog>>,
        Option<Res<Sim>>,
        Option<ResMut<Narration>>,
    ),
) {
    let m = &mut *manager;
    // The profile's front-end mode (+0x78) is 1 in Street School.
    let school = run.is_some_and(|run| matches!(run.0, Some(story::Run::School(_))));
    let was_playing = m.commentary_playing;
    let mut started = false;
    // The School's posts, as the listener took them in the game's update:
    // `0x1ad510` asks at once, in the School only, with the manager up.
    if let Some(n) = narration.as_deref_mut() {
        let posts = std::mem::take(&mut n.posts);
        if school
            && m.up
            && let Some(commentary) = sounds.commentary.as_ref()
        {
            for kind in posts {
                let packet = n.request(kind, !voices.is_empty());
                m.request_commentary(commentary, packet);
            }
        }
    }
    serve_players(m, fx.as_deref_mut());
    let mut event = None;
    let chosen = serve_commentary(m, sounds.commentary.as_mut()).and_then(|(id, selected)| {
        event = Some(id);
        let commentary = sounds.commentary.as_ref()?;
        let entry = commentary
            .entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(&selected.name))
            .cloned();
        Some((entry, selected.offset))
    });
    let line = chosen
        .as_ref()
        .and_then(|(entry, offset)| Some((entry.as_ref()?.name.clone(), *offset)));
    // Type 1: not paused (+0x5c), a speech volume (+0x4c), and either a
    // quiet commentator (+0x14) or the override (+0x68). Then, out of
    // Street School, the AEMS loop 0x24 starts with the ducking percents
    // (`FUN_0019ee90(aems, 0, 0x24)`), the music ducks (`FUN_001abde8`),
    // the commentator is busy and the line streams in the manager's one
    // stream (`FUN_00351cc0`), which a line started over another replaces.
    if let Some((Some(entry), offset)) = chosen
        && !m.paused
        && settings.speech != 0
        && (!m.commentary_playing || m.override_busy)
        && play_commentary(
            &mut commands,
            &mut disc,
            &mut sounds,
            &mut pcm,
            &settings,
            entry,
            offset,
        )
    {
        for voice in &voices {
            commands.entity(voice).despawn();
        }
        if !school && let Some(fx) = fx.as_deref_mut() {
            fx.duck_for_commentary();
        }
        if let Some(duck) = duck.as_deref_mut() {
            duck.duck(settings.music);
        }
        m.commentary_playing = true;
        started = true;
        if let (Some(log), Some(sim), Some((name, offset))) = (log.as_deref_mut(), &sim, &line) {
            log.audio(
                &sim.world,
                &format!(
                    "commentary {name} {offset} event {} clock {}",
                    event.unwrap_or(u16::MAX),
                    m.clock
                ),
            );
        }
    }
    // The waits (`0x1acfb0..0x1acfd4`): the introduction (+0x58), then the
    // page's reading (+0x54), asked again with the stream as it now plays.
    if let Some(n) = narration.as_deref_mut()
        && school
        && m.up
        && let Some(commentary) = sounds.commentary.as_ref()
    {
        let busy = started || !voices.is_empty();
        for (waits, kind) in [
            (n.intro_waits, speech::Lesson::Intro),
            (n.page_waits, speech::Lesson::Page),
        ] {
            if waits {
                let packet = n.request(kind, busy);
                m.request_commentary(commentary, packet);
            }
        }
    }
    // The stream has run out (`FUN_00351f30` reports nothing left): loop
    // 0x24 released, the music back up (`FUN_001abe78`), the commentator
    // free (+0x14).
    if was_playing && !started && voices.is_empty() {
        if let Some(fx) = fx.as_deref_mut() {
            fx.commentary_over();
        }
        if let Some(duck) = duck.as_deref_mut() {
            duck.restore();
        }
        if let (Some(log), Some(sim)) = (log.as_deref_mut(), &sim) {
            log.audio(&sim.world, &format!("commentary over clock {}", m.clock));
        }
    }
    m.commentary_playing = started || !voices.is_empty();
}

/// `audio_path_music_ducking` (`0x3e9a10`, registered with 35 at
/// `0x1a2698`): the share the music loses while the commentator talks.
pub const MUSIC_DUCKING: i32 = 35;
/// `audio_internal_game_mus_vol` (`0x3e9870`, 98 at `0x1a242c`): the
/// match music's share of its option, `DAT_003ea214`.
pub const GAME_MUSIC_VOLUME: i32 = 98;
/// The duck's ramp: `FUN_001aefe8(track, volume, 100)`, 100 ms in ten
/// 10 ms steps of the stream's volume (`FUN_00352888`).
const DUCK_RAMP: f32 = 0.1;

/// The match music's duck under the commentator (the path manager's
/// `FUN_001abde8` and `FUN_001abe78`): its share of the music's level,
/// ramping linearly to the target in [`DUCK_RAMP`].
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct MusicDuck {
    level: f32,
    target: f32,
    /// Share per second the ramp moves.
    rate: f32,
}

impl Default for MusicDuck {
    fn default() -> Self {
        MusicDuck {
            level: 1.0,
            target: 1.0,
            rate: 0.0,
        }
    }
}

impl MusicDuck {
    /// The ducked volume over the full one for the music option `level`
    /// (0..10), in the game's integers: `FUN_001aba18` keeps `level * 10`
    /// (+0x1784); full is that * 98 / 100 (`FUN_001abe78`), ducked that *
    /// (100 - 35) / 100 (`FUN_001abde8`). 63 / 98 at level 10.
    pub fn ducked_share(level: u8) -> f32 {
        let full = i32::from(level.min(10)) * 10 * GAME_MUSIC_VOLUME / 100;
        if full == 0 {
            return 1.0;
        }
        (full * (100 - MUSIC_DUCKING) / 100) as f32 / full as f32
    }

    fn ramp_to(&mut self, target: f32) {
        self.target = target;
        self.rate = (self.level - target).abs() / DUCK_RAMP;
    }

    /// `FUN_001abde8`: down to the ducked share.
    pub fn duck(&mut self, level: u8) {
        self.ramp_to(Self::ducked_share(level));
    }

    /// `FUN_001abe78`: back to the full volume (+0x1710, the pause's mute
    /// for `audio_no_pause_menu_music`, is 0 on the retail disc).
    pub fn restore(&mut self) {
        self.ramp_to(1.0);
    }

    /// A song opening sets the path's volume to full at once
    /// (`FUN_001aa8d8`).
    pub fn reset(&mut self) {
        *self = MusicDuck::default();
    }

    /// The share now.
    pub fn level(&self) -> f32 {
        self.level
    }

    /// Moves the ramp on by `dt` seconds; whether the share changed.
    pub fn step(&mut self, dt: f32) -> bool {
        if self.level == self.target {
            return false;
        }
        let step = self.rate * dt;
        self.level = if (self.target - self.level).abs() <= step {
            self.target
        } else if self.target < self.level {
            self.level - step
        } else {
            self.level + step
        };
        true
    }
}

/// The duck reaches the match song's sink. A song opening plays at the
/// full volume (`FUN_001aa8d8` sets the path's volume as it opens).
fn duck_music(
    time: Res<Time>,
    mut duck: ResMut<MusicDuck>,
    settings: Res<AudioSettings>,
    opened: Query<(), Added<MatchSong>>,
    mut sinks: Query<&mut AudioSink, With<MatchSong>>,
) {
    if !opened.is_empty() {
        duck.reset();
    }
    if !duck.step(time.delta_secs()) {
        return;
    }
    let gain = part_gain(Part::Music, &settings, true) * duck.level();
    for mut sink in &mut sinks {
        sink.set_volume(Volume::Linear(gain));
    }
}

/// `FUN_00364620(1)`: the commentary's newest request through its event,
/// the chosen line used up (`FUN_00365ba0`); the event and the line.
pub(crate) fn serve_commentary(
    m: &mut SpeechManager,
    commentary: Option<&mut Commentary>,
) -> Option<(u16, speech::SelectedCommentary)> {
    let request = m.middleware.queue.service(speech::COMMENTARY, m.clock)?;
    let commentary = commentary?;
    let id = request.packet[0] as u16;
    if commentary
        .event(id)
        .is_some_and(|e| m.middleware.repeated(e, speech::COMMENTARY))
    {
        return None;
    }
    let selected = commentary.choose(&request.packet, &mut m.middleware.random)?;
    commentary.commit(&selected);
    m.middleware.posted(id, speech::COMMENTARY);
    Some((id, selected))
}

/// `FUN_00364620(0)` and the callback's packet type 0: the players' newest
/// request through event 0x2000, the line used up, and event 0x1f where the
/// last speaker stands unless the commentator is talking (+0x14, which the
/// override does not lift for players).
pub(crate) fn serve_players(
    m: &mut SpeechManager,
    fx: Option<&mut crate::sfx::Effects>,
) -> Option<speech::SelectedPlayerLine> {
    let p = m.middleware.queue.service(speech::PLAYERS, m.clock)?.packet;
    let request = speech::PlayerRequest {
        voice: p[1] as u16,
        category_mask: p[2],
        shot_mask: p[3],
        action_mask: p[4],
    };
    let selected = m
        .players
        .select_request(request, &mut m.middleware.random)?;
    m.players.commit(&selected);
    m.middleware.posted(speech::PLAYER_EVENT, speech::PLAYERS);
    if !m.commentary_playing
        && !m.paused
        && let (Some(fx), Some((_, at))) = (fx, m.speaker)
    {
        fx.fire(&[
            crate::sfx::ev::SPEECH,
            at.0,
            at.1,
            i32::from(selected.stream),
        ]);
    }
    Some(selected)
}

/// FUN_00170dc0 scans opposite-team players in index order using the XZ
/// distance cache (FUN_0013fbc8/FUN_0018cdb0), below 1000 court units.
fn nearest_move_opponent(players: &[street_sim::Player], player: usize) -> Option<usize> {
    let actor = players.get(player)?;
    let mut nearest = None;
    let mut distance = 1000.0;
    for (i, p) in players.iter().enumerate() {
        if p.team == actor.team {
            continue;
        }
        let dx = p.position[0] - actor.position[0];
        let dz = p.position[1] - actor.position[1];
        let d = (dx * dx + dz * dz).sqrt();
        if d < distance {
            distance = d;
            nearest = Some(i);
        }
    }
    nearest
}

/// FUN_001a83e0 checks the actor's nickname row, its matching action skill,
/// the Be A Legend mode and the profile's one-shot announcement flag; the
/// caller adds the playing-commentator gate (+0x14).
fn pending_action_nickname(
    legend: bool,
    scripted: bool,
    pending: bool,
    actor: &street_assets::db::Player,
    names: &[street_assets::db::Nickname],
    skills: &[street_assets::db::Skill],
) -> Option<u16> {
    if !legend || scripted || !pending {
        return None;
    }
    let row = *actor.fields.get(6)?;
    if row <= 0 {
        return None;
    }
    let nickname = names.get(row as usize)?;
    nickname.skill.filter(|skill| skills.contains(skill))?;
    u16::try_from(nickname.id).ok()
}

/// `FUN_001a8310`: a created player's (field 3 = 0x25) nickname id from
/// their row (field 6), 0 without one, and 0 while Be A Legend still has the
/// nickname's announcement pending.
fn nickname_word(
    actor: &street_assets::db::Player,
    names: &[street_assets::db::Nickname],
    legend_pending: bool,
) -> u16 {
    let field = |i: usize| actor.fields.get(i).copied().unwrap_or(0);
    if field(3) != 0x25 || field(6) <= 0 || legend_pending {
        return 0;
    }
    names
        .get(field(6) as usize)
        .and_then(|n| u16::try_from(n.id).ok())
        .unwrap_or(0)
}

/// Proven ball-event dispatches: shot deflection -> Blocks then Power;
/// held-ball counter -> Steals; loose-ball possession -> Rebounds;
/// dunk/alley-oop scores -> Dunks then Power; other scores -> Shots (FUN_001a3b10).
fn nickname_action(event: &Event) -> Option<(usize, &'static [street_assets::db::Skill])> {
    use street_assets::db::Skill;
    match *event {
        Event::Trick {
            player,
            fakeout: Some(_),
            ..
        } => Some((player, &[Skill::Handles])),
        Event::Block { player, .. } => Some((player, &[Skill::Blocks, Skill::Power])),
        Event::Counter { player, .. }
        | Event::Steal {
            player,
            intercepted_pass: false,
            ..
        } => Some((player, &[Skill::Steals])),
        Event::Rebound { player } => Some((player, &[Skill::Rebounds])),
        Event::Score { player, basket, .. } => match basket {
            Basket::Dunk { .. } | Basket::AlleyOop { .. } => {
                Some((player, &[Skill::Dunks, Skill::Power]))
            }
            _ => Some((player, &[Skill::Shots])),
        },
        _ => None,
    }
}

/// `FUN_001a81b0`: the court (profile +0xcc, the `XDBCOURT` record) as
/// the court intro's word: 0 0x80, 1 1, 2 2, 3 0x200, 4 0x20, 5 8, 6
/// 0x40, 7 0x100, 8 4, any other 0x10. Each `CourtIntroXX` header's lines
/// carry the one value their court's bit allows.
pub(crate) fn court_word(court: Option<usize>) -> u32 {
    match court {
        Some(0) => 0x80,
        Some(1) => 1,
        Some(2) => 2,
        Some(3) => 0x200,
        Some(4) => 0x20,
        Some(5) => 8,
        Some(6) => 0x40,
        Some(7) => 0x100,
        Some(8) => 4,
        _ => 0x10,
    }
}

/// `FUN_001a85f0`: 3 for a dunk of kind 4 on courts 2, 3, 5, 7, 8 and 9
/// (`FUN_001a85a0`), 1 otherwise.
pub(crate) fn court_dunk_word(court: Option<usize>, kind: u32) -> u32 {
    let set = matches!(court, Some(2 | 3 | 5 | 7 | 8 | 9));
    if set && kind == 4 { 3 } else { 1 }
}

/// `0x1a3b10`'s score call (event 0xb): from the side a person plays (the
/// first side, unless it is the computer's or both are people's and the
/// second scored), 1 ahead, 4 behind, 2 level, and the margin (the score
/// when level).
pub(crate) fn score_update_packet(
    score: [u32; 2],
    human: [bool; 2],
    scorer_team: usize,
) -> speech::Packet {
    let (mine, theirs) = if !human[0] || (human[1] && scorer_team == 1) {
        (score[1], score[0])
    } else {
        (score[0], score[1])
    };
    let (standing, margin) = match mine.cmp(&theirs) {
        std::cmp::Ordering::Greater => (1, mine - theirs),
        std::cmp::Ordering::Less => (4, theirs - mine),
        std::cmp::Ordering::Equal => (2, mine),
    };
    speech::words_packet(0xb, &[standing, margin])
}

/// The commentary requests the listener's handlers post for a tick's
/// events, in their order. A pending nickname that matches (`0x1a83e0`)
/// posts event 0x12 instead and is spent, whether or not the commentator
/// will speak; the others post their own event: `0x1a5a78` 4, `0x1a4ce8`
/// 0, `0x1a63e0` 8 or every fifth steal 0xd, `0x1a6d88` 0x11 or every
/// third violation 0xd, the basket's `0x1a3b10` (0xf for the winning one,
/// else every fourth basket past 3 a side 0xb, else 1 a dunk or 0xc a
/// shot; a gamebreaker's basket 2 through `0x1a6b80`), a miss's
/// `0x1a4550` 0xe, a head passout's `0x1a5d50` 0x13, the end's `0x1a5638` 6, and
/// at the first script's end (slot 61, `0x1a56b0`) the court's 3.
#[allow(clippy::too_many_arguments)]
fn match_sounds(
    disc: Res<Disc>,
    sounds: Res<Sounds>,
    mut manager: ResMut<SpeechManager>,
    sim: Res<Sim>,
    tick: Res<TickEvents>,
    setup: Res<MatchSetup>,
    run: Res<story::Running>,
    mut save: ResMut<story::Save>,
) {
    // Commentary only on the live game, not over replays.
    if tick.replay {
        return;
    }
    let world = &sim.world;
    let scripted = world.sequence.is_some();
    let school = matches!(run.0, Some(story::Run::School(_)));
    let legend = matches!(run.0, Some(story::Run::StoryEvent(_)));
    let court = disc
        .courts
        .iter()
        .position(|c| c.id.eq_ignore_ascii_case(&setup.court));
    let record = |player: usize| {
        setup
            .lineups
            .iter()
            .flatten()
            .nth(player)
            .and_then(|&record| disc.players.get(record))
    };
    let female = |player: usize| record(player).map(|p| p.female);
    let human = [world.teams[0].human, world.teams[1].human];
    // Manager +0x60 from the points to win (profile +0xf0). A trick point
    // or gamebreaker game has none; 21 there is the rewrite's own.
    let length = speech::game_length_word(match world.rules.goal {
        Goal::Points(to) => to,
        _ => 21,
    });
    let mut packets = Vec::new();

    // Slot 61's first call (the first script's end, the court's
    // presentation; play starts, `hud::INTRO_TICKS`): on the interactive
    // Music Experience speech resumes with its queue cleared and the
    // court is introduced (event 3, which a staged script lets through).
    // The bracket's final rounds (word 2: 2, from the tournament's rounds
    // left) and the second call's identity words (`FUN_001a78a0` and
    // `FUN_001a7a90` for the record at `*(game+0x428)+0x2c`) are not
    // ported.
    if manager.scripts_ended == 0 && world.tick >= crate::hud::INTRO_TICKS {
        manager.scripts_ended = 1;
        if sounds.interactive {
            manager.resume(true);
            packets.push((speech::words_packet(3, &[court_word(court), 1]), true));
        }
        // Then `0x1a56b0` counts the six players' star and legend bits
        // (`FUN_001a7bd0`, `FUN_001a7948`), whether or not Playlist
        // silenced the court: fewer than two set +0x74.
        manager.name_words_off = speech::names_off((0..6).filter_map(|k| record(k).map(|p| p.id)));
    }

    // `0x1a83e0`: no attempt while the commentator talks (+0x14). A
    // matching attempt spends the pending nickname.
    let busy = manager.commentary_playing;
    let attempt = |save: &mut story::Save, player: usize, skills: &[street_assets::db::Skill]| {
        world.players.get(player)?;
        let actor = record(player)?;
        let id = pending_action_nickname(
            legend,
            scripted,
            save.story.nickname_pending && !busy,
            actor,
            &disc.nicknames,
            skills,
        )?;
        save.story.nickname_pending = false;
        save.store();
        Some(speech::nickname_packet(id, actor.female))
    };
    let alias = |player: usize, pending: bool| {
        record(player).map_or(0, |actor| {
            nickname_word(actor, &disc.nicknames, legend && pending)
        })
    };
    for event in &tick.events {
        if let Some(shot) = ShotKind::released(event, world) {
            manager.last_shot = Some(shot);
        }
        // The event's nickname route (`nickname_action`), attempted lazily:
        // an attempt spends the pending nickname.
        let mut nickname = || nickname_action(event).and_then(|(p, s)| attempt(&mut save, p, s));
        match *event {
            // Slot 13's commentary (`0x1a5a78`) only for a trick that
            // faked a defender out (its word +8).
            Event::Trick {
                player,
                fakeout: Some(_),
                ..
            } => {
                if let Some(p) = nickname() {
                    packets.push((p, false));
                } else if let (Some(_), Some(actor)) = (world.players.get(player), female(player)) {
                    let nearest = nearest_move_opponent(&world.players, player);
                    packets.push((speech::move_packet(actor, nearest.and_then(female)), false));
                }
            }
            Event::Block { player, shooter } => {
                if let Some(p) = nickname() {
                    packets.push((p, false));
                } else if let Some(blocker) = female(player) {
                    packets.push((
                        speech::block_packet(
                            alias(player, save.story.nickname_pending),
                            female(shooter),
                            blocker,
                        ),
                        false,
                    ));
                }
            }
            // The rewrite's own: a goaltend as a block of an unknown shooter.
            Event::Goaltend { player, .. } => {
                if let Some(blocker) = female(player) {
                    packets.push((
                        speech::block_packet(
                            alias(player, save.story.nickname_pending),
                            None,
                            blocker,
                        ),
                        false,
                    ));
                }
            }
            Event::Steal { player, from, .. } => {
                let p = nickname();
                if let Some(p) = p {
                    packets.push((p, false));
                } else {
                    packets.extend(steal_packets(&mut manager, player, from, |p| {
                        (alias(p, save.story.nickname_pending), female(p))
                    }));
                }
            }
            Event::Counter { player, from } => {
                if let Some(p) = nickname() {
                    packets.push((p, false));
                } else {
                    packets.extend(steal_packets(&mut manager, player, from, |p| {
                        (alias(p, save.story.nickname_pending), female(p))
                    }));
                }
            }
            Event::Rebound { .. } => {
                if let Some(p) = nickname() {
                    packets.push((p, false));
                }
            }
            Event::ShotClockViolation { .. } => {
                manager.violations = manager.violations.wrapping_add(1);
                packets.push((
                    word_less(if manager.violations.is_multiple_of(3) {
                        0xd
                    } else {
                        0x11
                    }),
                    false,
                ));
            }
            Event::Score {
                team,
                player,
                points,
                basket,
            } => {
                let kind = ShotKind::of(basket);
                let dunk = matches!(kind, ShotKind::Dunk | ShotKind::AlleyOop);
                let breaker = matches!(basket, Basket::Gamebreaker { .. });
                manager.baskets = manager.baskets.wrapping_add(1);
                if matches!(world.phase, Phase::Over { .. }) {
                    // The game's +0xbc: the winning basket.
                    packets.push((
                        speech::words_packet(
                            0xf,
                            &[
                                court_dunk_word(court, kind.word()),
                                if dunk { 1 } else { 2 },
                            ],
                        ),
                        false,
                    ));
                } else if !breaker {
                    let early = world.score[0] < 4 && world.score[1] < 4;
                    // A trick point or gamebreaker game (profile +0xe8 1
                    // or 6) never calls the score.
                    let race = matches!(
                        world.rules.goal,
                        Goal::TrickPoints(_) | Goal::Gamebreaker(_)
                    );
                    if !early && manager.baskets & 3 == 0 && !race {
                        packets.push((score_update_packet(world.score, human, team), false));
                    } else if dunk {
                        if let Some(p) = nickname() {
                            packets.push((p, false));
                        } else if let Some(actor) = record(player) {
                            // The three identity words (`FUN_001a7800`,
                            // `FUN_001a7948`, `FUN_001a7bd0`): the dunker's
                            // persistent id's Street Legend, Legend and
                            // league star bits, the last two 0 under +0x74.
                            packets.push((
                                speech::dunk_packet(
                                    kind == ShotKind::AlleyOop,
                                    speech::dunker_words(actor.id, manager.name_words_off),
                                    actor.female,
                                    alias(player, save.story.nickname_pending),
                                    length,
                                ),
                                false,
                            ));
                        }
                    } else if let Some(p) = nickname() {
                        packets.push((p, false));
                    } else if let Some(actor) = female(player) {
                        packets.push((
                            speech::shot_packet(
                                points,
                                alias(player, save.story.nickname_pending),
                                actor,
                                length,
                            ),
                            false,
                        ));
                    }
                }
                if breaker {
                    // `0x1a6b80` with +0xc set: the last gamebreaker
                    // basket's kind (+4) words the hype, then +4 takes
                    // this one's. The scorer's meter below full resumes
                    // speech. A Level 2's call comes from the listener's
                    // update as its sequence ends; the rewrite posts it
                    // with the basket.
                    let last = manager.breaker_kind;
                    packets.push((
                        speech::words_packet(
                            2,
                            &[
                                court_dunk_word(court, last),
                                if (3..=4).contains(&last) { 1 } else { 2 },
                            ],
                        ),
                        false,
                    ));
                    manager.breaker_kind = kind.word();
                    if !world.teams[team.min(1)].meter_full() {
                        manager.resume(true);
                    }
                }
            }
            // Slot 27 (`0x1a4550`): a missed shot, 2 when its shooter was
            // dunking (+0xa4 bit 7), 1 otherwise. MissedDunk may not be
            // posted twice running (its root's repeat limit 1).
            Event::RimOut => {
                let dunking = matches!(manager.last_shot, Some((_, ShotKind::Dunk)));
                packets.push((
                    speech::words_packet(0xe, &[if dunking { 2 } else { 1 }]),
                    false,
                ));
            }
            // Slot 39 (`0x1a5d50`), which the passout's legs post as the
            // ball goes on to its second leg (`FUN_00148378`): with word 0,
            // off a defender's head, the commentator's PassOut (0x13, words
            // [2], root chance 50); off the board (word 1) nothing.
            Event::PassoutBounce { head: true } => {
                packets.push((speech::words_packet(0x13, &[2]), false));
            }
            // Slot 56 (`0x1a5638`, posted by `0x13e4c8` when a person
            // plays): the closing line, over any call (event 6 sets +0x68).
            Event::GameOver { .. } if human.contains(&true) => {
                packets.push((word_less(6), false));
            }
            _ => {}
        }
    }
    let Some(commentary) = sounds.commentary.as_ref() else {
        return;
    };
    for (packet, intro) in packets {
        // `0x1ad178`'s gates; only the court's event 3 passes a staged
        // script.
        if manager.commentary_open(&sounds, scripted && !intro, school) {
            manager.request_commentary(commentary, packet);
        }
    }
}

/// `0x1a63e0` without a nickname: every fifth steal of the session
/// (`DAT_004bb2d4`) the Turnover event 0xd, else the steal's event 8.
fn steal_packets(
    manager: &mut SpeechManager,
    player: usize,
    from: usize,
    who: impl Fn(usize) -> (u16, Option<bool>),
) -> Option<(speech::Packet, bool)> {
    manager.steals = manager.steals.wrapping_add(1);
    if manager.steals.is_multiple_of(5) {
        return Some((word_less(0xd), false));
    }
    let (alias, stealer) = who(player);
    Some((speech::steal_packet(alias, stealer?, who(from).1), false))
}

/// A commentary event posted without words (`0x1ad178`'s cases 6, 0xd,
/// 0x11).
fn word_less(id: u16) -> speech::Packet {
    let mut packet = [0; 20];
    packet[0] = u32::from(id);
    packet
}

/// The pause menu stops the match's sound; the music plays on. Pausing is
/// slot 57 (`0x1a70a0`, the pause class `0x3c6800`): on the interactive
/// Music Experience speech is paused with its queue cleared
/// (`FUN_001ad958(manager, 1, 1)`). Resuming (`0x1a7198` through
/// `0x1a1cc8`) resumes it, queue cleared, unless a meter is full or a
/// staged gamebreaker plays (`FUN_00140ff8`, read as the simulation's
/// sequence).
///
/// The commentator's stream is not paused: `FUN_001ad958` fades it to
/// nothing in 30 ms (`FUN_00352298(stream, 0x1e, 0)`) and lets it run on,
/// and the resume stops it (`FUN_00351ce0`), so the next service finds it
/// over and frees the commentator, the music and loop 0x24.
fn pause_audio(
    mut commands: Commands,
    paused: Option<Res<Paused>>,
    mut sinks: Query<(Entity, &mut AudioSink, &Part, Has<CommentaryVoice>)>,
    sounds: Res<Sounds>,
    sim: Option<Res<Sim>>,
    mut manager: ResMut<SpeechManager>,
    mut was: Local<bool>,
) {
    let Some(paused) = paused else { return };
    // The pause menu opening or closing (slots 57 and `0x1a7198`), not the
    // resource being written: `game.rs` sets `Paused::on` every frame.
    if paused.on == *was {
        return;
    }
    *was = paused.on;
    let (mut silence, mut stop) = (false, false);
    if manager.initialised() {
        if paused.on {
            if sounds.interactive {
                manager.pause(true);
                silence = true;
            }
        } else if sim.is_none_or(|sim| {
            let w = &sim.world;
            w.sequence.is_none() && !w.teams.iter().any(|t| t.meter_full())
        }) {
            manager.resume(true);
            stop = true;
        }
    }
    for (entity, mut sink, part, voice) in &mut sinks {
        if *part == Part::Music {
            continue;
        }
        if voice {
            if silence {
                sink.set_volume(Volume::Linear(0.0));
            } else if stop {
                commands.entity(entity).despawn();
            }
            continue;
        }
        if paused.on {
            sink.pause();
        } else {
            sink.play();
        }
    }
}

/// Plays `clip` once at the music's level (a movie's soundtrack), tagged
/// with `tag` so its owner can stop it.
pub fn play_once(
    commands: &mut Commands,
    pcm: &mut Assets<Pcm>,
    settings: &AudioSettings,
    clip: Clip,
    tag: impl Component,
) {
    commands.spawn((
        AudioPlayer(pcm.add(Pcm::from(clip))),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(part_gain(
            Part::Music,
            settings,
            false,
        ))),
        Part::Music,
        tag,
    ));
}

/// A movie's soundtrack: at the music's level (`FUN_0022d958` gives a
/// movie the path manager's volume, `FUN_001a1720(.., 2)`), or, for the
/// logos it leaves alone, at the movie player's own full level.
pub fn play_movie(
    commands: &mut Commands,
    pcm: &mut Assets<Pcm>,
    settings: &AudioSettings,
    clip: Clip,
    at_music_level: bool,
    tag: impl Component,
) {
    if at_music_level {
        play_once(commands, pcm, settings, clip, tag);
        return;
    }
    commands.spawn((
        AudioPlayer(pcm.add(Pcm::from(clip))),
        PlaybackSettings::DESPAWN,
        tag,
    ));
}

/// Volume changes reach the sounds already playing (the match song under
/// the commentator's duck).
fn apply_volumes(
    settings: Res<AudioSettings>,
    state: Res<State<Screen>>,
    duck: Res<MusicDuck>,
    mut sinks: Query<(&mut AudioSink, &Part, Has<MatchSong>)>,
) {
    if !settings.is_changed() {
        return;
    }
    let in_match = *state.get() == Screen::Playing;
    for (mut sink, part, song) in &mut sinks {
        let share = if song && in_match { duck.level() } else { 1.0 };
        sink.set_volume(Volume::Linear(
            part_gain(*part, &settings, in_match) * share,
        ));
    }
}

#[cfg(test)]
mod rotation_tests {
    use super::*;

    #[test]
    fn playlist_rotation_skips_disabled_tracks_and_wraps_in_disc_order() {
        let mut sounds = Sounds::default();
        let bit = street_sim::records::FRONT_END_BIT;
        let all = ((1 << music::FRONT_END_SONGS.len()) - 1) << bit;
        sounds.trax(all & !(1 << (bit + 1)) & !(1 << (bit + 4)), true, false);
        assert_eq!(sounds.next_menu_song(), Some(music::FRONT_END_SONGS[1]));
        assert_eq!(sounds.next_menu_song(), Some(music::FRONT_END_SONGS[4]));
        assert_eq!(sounds.next_menu_song(), Some(music::FRONT_END_SONGS[1]));
        sounds.trax(all, false, false);
        assert_eq!(sounds.next_menu_song(), None);
    }

    #[test]
    fn random_playlist_does_not_repeat_when_another_song_is_enabled() {
        let mut sounds = Sounds::default();
        let mut previous = None;
        for _ in 0..40 {
            let song = sounds.next_menu_song();
            assert!(song.is_some());
            assert_ne!(song, previous);
            previous = song;
        }
        let bit = street_sim::records::FRONT_END_BIT;
        let all = ((1 << music::FRONT_END_SONGS.len()) - 1) << bit;
        sounds.trax(all & !(1 << bit), false, false);
        assert_eq!(sounds.next_menu_song(), Some(music::FRONT_END_SONGS[0]));
        assert_eq!(sounds.next_menu_song(), Some(music::FRONT_END_SONGS[0]));
    }
    fn pathsong(n: usize) -> Vec<music::Song> {
        (1..=n)
            .map(|k| music::Song {
                folder: format!("S{k}"),
                artist_speech: 0,
                title: format!("Song {k}"),
                artist: String::new(),
                album: String::new(),
            })
            .collect()
    }

    /// The match's song (`0x1a0a38`) and `FUN_001aaf78`: the session's song
    /// stays unless it is switched off, when the first song on from it up
    /// to 15, else from 1 up to it, takes over; none on is -1; the Rucker
    /// Classic (court 10) plays song 12.
    #[test]
    fn the_matchs_song_is_the_sessions_unless_switched_off_like_fun_001aaf78() {
        let mut sounds = Sounds {
            songs: pathsong(15),
            ..Default::default()
        };
        sounds.path_song = 5;
        let folder = |s: Option<(music::Song, u32)>| s.map(|(s, _)| s.folder);
        assert_eq!(folder(sounds.match_song(None, false)), Some("S5".into()));
        // Songs 5..9 off: the first on from 5 is 10.
        sounds.trax(0b1_1111 << 4, false, false);
        assert_eq!(folder(sounds.match_song(None, false)), Some("S10".into()));
        assert_eq!(sounds.path_song, 10);
        // From 14 with 14 and 15 off the search wraps to 1; 16 and up are
        // never searched.
        sounds.path_song = 14;
        sounds.trax(0b11 << 13, false, false);
        assert_eq!(sounds.first_path_song_on(), 1);
        // Every song off: -1 and nothing plays.
        sounds.trax(u32::MAX, false, false);
        assert_eq!(sounds.match_song(None, false), None);
        assert_eq!(sounds.path_song, -1);
        // The Rucker Classic plays song 12 whatever the session's song;
        // switched off, it still does until the court is the save's, then
        // the next one on (`FUN_001a0c48`).
        sounds.trax(0, false, false);
        sounds.path_song = 3;
        assert_eq!(
            folder(sounds.match_song(Some(10), false)),
            Some("S12".into())
        );
        assert_eq!(sounds.path_song, 12);
        sounds.trax(1 << 11, false, false);
        assert_eq!(
            folder(sounds.match_song(Some(10), false)),
            Some("S12".into())
        );
        assert_eq!(
            folder(sounds.match_song(Some(10), true)),
            Some("S13".into())
        );
        assert_eq!(sounds.path_song, 13);
    }

    /// `FUN_001aabc8` in turn: the next song, or the next one on; past the
    /// last song the first.
    #[test]
    fn rotation_in_turn_steps_to_the_next_song_on_like_fun_001aabc8() {
        let mut sounds = Sounds {
            songs: pathsong(6),
            ..Default::default()
        };
        // Song 2 off.
        sounds.trax(0b10, true, false);
        sounds.path_song = 1;
        let folders: Vec<String> = (0..6)
            .map(|_| sounds.rotate_song().map(|(s, _)| s.folder).unwrap())
            .collect();
        assert_eq!(folders, ["S3", "S4", "S5", "S6", "S1", "S3"]);
    }

    /// `FUN_001aabc8` at random: a draw of `rand() % count + 1` (the
    /// rewrite's own generator), unless the next song on is the song just
    /// played, which then plays again without a draw; a draw of the song
    /// just played gives way to the next one on after it.
    #[test]
    fn random_rotation_gives_way_after_the_song_just_played_like_fun_001aabc8() {
        let mut sounds = Sounds {
            songs: pathsong(6),
            ..Default::default()
        };
        sounds.trax(0, false, false);
        sounds.path_song = 4;
        let mut probe = Sounds {
            rng: sounds.rng,
            ..Default::default()
        };
        let draw = (probe.roll() % 6) as i32 + 1;
        let played = sounds.rotate_song().map(|(s, _)| s.folder).unwrap();
        let expected = if draw == 4 { 5 } else { draw };
        assert_eq!(played, format!("S{expected}"));
        assert_eq!(sounds.path_song, expected);
        // Only song 3 on, just played: the next on after it is itself, so
        // it plays again and no draw is taken (the one roll is the map's
        // seed).
        sounds.trax(!(1 << 2), false, false);
        sounds.path_song = 3;
        let mut probe = Sounds {
            rng: sounds.rng,
            ..Default::default()
        };
        probe.roll();
        assert_eq!(
            sounds.rotate_song().map(|(s, _)| s.folder),
            Some("S3".into())
        );
        assert_eq!(sounds.rng, probe.rng);
    }

    /// The score call's words: the manager's length word (`0x1ac6f0`, from
    /// the points to win), `0x1a3b10`'s standing and margin from the side
    /// a person plays, `FUN_001a81b0`'s court word and `FUN_001a85f0`'s
    /// dunk word.
    #[test]
    fn the_score_calls_words_follow_the_listener() {
        assert_eq!(
            [15, 16, 25, 26, 35, 36, 42, 43, 99].map(speech::game_length_word),
            [1, 2, 2, 4, 4, 8, 8, 0x10, 0x10]
        );
        let words = |p: speech::Packet| (p[0], p[1], p[2]);
        // A person on the first side, ahead by 3.
        assert_eq!(
            words(score_update_packet([10, 7], [true, false], 0)),
            (0xb, 1, 3)
        );
        // Two people, the second scoring from 5-7: their side leads by 2.
        assert_eq!(
            words(score_update_packet([5, 7], [true, true], 1)),
            (0xb, 1, 2)
        );
        // The computer on the first side: the second side's view, behind.
        assert_eq!(
            words(score_update_packet([9, 6], [false, true], 0)),
            (0xb, 4, 3)
        );
        // Level: the score itself.
        assert_eq!(
            words(score_update_packet([8, 8], [true, false], 1)),
            (0xb, 2, 8)
        );
        assert_eq!(
            (0..11).map(|c| court_word(Some(c))).collect::<Vec<_>>(),
            [0x80, 1, 2, 0x200, 0x20, 8, 0x40, 0x100, 4, 0x10, 0x10]
        );
        assert_eq!(court_word(None), 0x10);
        assert_eq!(court_dunk_word(Some(2), 4), 3);
        assert_eq!(court_dunk_word(Some(2), 2), 1);
        assert_eq!(court_dunk_word(Some(0), 4), 1);
    }

    /// `FUN_001ad958` and `FUN_001ad9d0`: the pause flag (+0x5c), with the
    /// queue cleared (`FUN_00364190`) only when asked.
    #[test]
    fn pausing_speech_clears_the_queue_only_when_asked() {
        let mut manager = SpeechManager::default();
        let event = speech::SpeechEvent {
            id: 4,
            hold: 200,
            priority: 500,
            chance: 100,
            word_predicates: Vec::new(),
            repeat: 0,
            branches: Vec::new(),
        };
        let post = |m: &mut SpeechManager| {
            assert!(
                m.middleware
                    .post(&event, speech::COMMENTARY, 0, speech::words_packet(4, &[]),)
            );
        };
        post(&mut manager);
        manager.pause(false);
        assert!(manager.paused);
        assert!(!manager.middleware.queue.is_empty());
        manager.resume(false);
        assert!(!manager.paused);
        assert!(!manager.middleware.queue.is_empty());
        manager.pause(true);
        assert!(manager.paused);
        assert!(manager.middleware.queue.is_empty());
        post(&mut manager);
        manager.resume(true);
        assert!(!manager.paused);
        assert!(manager.middleware.queue.is_empty());
    }

    #[test]
    fn move_commentary_uses_nearest_opponent_and_native_range() {
        use street_sim::Player;
        let mut players = vec![
            Player::new(0, [0.0, 0.0], 0.0),
            Player::new(0, [0.01, 0.0], 0.0),
            Player::new(1, [3.0, 4.0], 0.0),
            Player::new(1, [-3.0, 4.0], 0.0),
        ];
        assert_eq!(
            nearest_move_opponent(&players, 0),
            Some(2),
            "first equal-distance opponent wins"
        );
        players[3].position = [0.0, 1.0];
        assert_eq!(nearest_move_opponent(&players, 0), Some(3));
        players[2].position = [50.0, 0.0];
        players[3].position = [60.0, 0.0];
        assert_eq!(
            nearest_move_opponent(&players, 0),
            Some(2),
            "native distance uses court units without audio-coordinate conversion"
        );
        players[2].position = [1001.0, 0.0];
        players[3].position = [0.0, 1001.0];
        assert_eq!(
            nearest_move_opponent(&players, 0),
            None,
            "outside native 1000 court-unit search"
        );
        players[2].position = [999.0, 0.0];
        assert_eq!(nearest_move_opponent(&players, 0), Some(2));
    }
    #[test]
    fn an_ordinary_pickup_does_not_spend_a_rebound_nickname() {
        assert!(nickname_action(&Event::Pickup { player: 0 }).is_none());
        assert_eq!(
            nickname_action(&Event::Rebound { player: 0 }),
            Some((0, &[street_assets::db::Skill::Rebounds][..]))
        );
    }

    #[test]
    fn disc_nickname_action_routes_only_proven_possession_events() {
        use street_assets::db::Skill;
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let mut actor = disc.players[0].clone();
        let events = [
            (
                Event::Block {
                    player: 0,
                    shooter: 1,
                },
                vec![Skill::Blocks, Skill::Power],
            ),
            (Event::Counter { player: 0, from: 1 }, vec![Skill::Steals]),
            (
                Event::Steal {
                    player: 0,
                    from: 1,
                    intercepted_pass: false,
                },
                vec![Skill::Steals],
            ),
            (Event::Rebound { player: 0 }, vec![Skill::Rebounds]),
            (
                Event::Score {
                    team: 0,
                    player: 0,
                    points: 1,
                    basket: Basket::Dunk { advanced: false },
                },
                vec![Skill::Dunks, Skill::Power],
            ),
            (
                Event::Score {
                    team: 0,
                    player: 0,
                    points: 1,
                    basket: Basket::AlleyOop { advanced: false },
                },
                vec![Skill::Dunks, Skill::Power],
            ),
            (
                Event::Score {
                    team: 0,
                    player: 0,
                    points: 1,
                    basket: Basket::Jumper { long: false },
                },
                vec![Skill::Shots],
            ),
        ];
        for (event, expected) in events {
            let (player, skills) = nickname_action(&event).unwrap();
            assert_eq!(player, 0);
            assert_eq!(skills, expected);
            for (row, nickname) in disc.nicknames.iter().enumerate().skip(1) {
                actor.fields[6] = row as i16;
                let selected =
                    pending_action_nickname(true, false, true, &actor, &disc.nicknames, skills);
                assert_eq!(
                    selected,
                    nickname
                        .skill
                        .filter(|s| skills.contains(s))
                        .map(|_| nickname.id as u16)
                );
            }
        }
        assert!(
            nickname_action(&Event::Steal {
                player: 0,
                from: 1,
                intercepted_pass: true
            })
            .is_none(),
            "pass interception must preserve pending nickname and ordinary commentary"
        );
    }

    #[test]
    fn disc_pending_nickname_requires_native_story_action_gates() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let row = disc
            .nicknames
            .iter()
            .enumerate()
            .find(|(i, n)| *i > 0 && n.skill == Some(street_assets::db::Skill::Handles))
            .unwrap()
            .0;
        let mut actor = disc.players[0].clone();
        actor.fields[6] = row as i16;
        assert_eq!(
            pending_action_nickname(
                true,
                false,
                true,
                &actor,
                &disc.nicknames,
                &[street_assets::db::Skill::Handles]
            ),
            Some(disc.nicknames[row].id as u16)
        );
        for (legend, scripted, pending) in [
            (false, false, true),
            (true, true, true),
            (true, false, false),
        ] {
            assert_eq!(
                pending_action_nickname(
                    legend,
                    scripted,
                    pending,
                    &actor,
                    &disc.nicknames,
                    &[street_assets::db::Skill::Handles]
                ),
                None
            );
        }
        actor.fields[6] = 0;
        assert_eq!(
            pending_action_nickname(
                true,
                false,
                true,
                &actor,
                &disc.nicknames,
                &[street_assets::db::Skill::Handles]
            ),
            None,
            "native property 6 must be strictly positive"
        );
        actor.fields[6] = disc
            .nicknames
            .iter()
            .position(|n| n.skill != Some(street_assets::db::Skill::Handles))
            .unwrap() as i16;
        assert_eq!(
            pending_action_nickname(
                true,
                false,
                true,
                &actor,
                &disc.nicknames,
                &[street_assets::db::Skill::Handles]
            ),
            None,
            "unrelated action does not consume the one-shot nickname flag"
        );
    }

    /// A match's commentary path without the rest of the game: the tick's
    /// events go through `match_sounds`, then the speech manager's
    /// service, as `AudioPlugin` schedules them.
    fn commentary_app(disc: Disc) -> App {
        commentary_app_with(disc, None)
    }

    /// The harness with its own line-ups (records), else the first two
    /// teams' first three.
    fn commentary_app_with(mut disc: Disc, lineups: Option<[[usize; 3]; 2]>) -> App {
        let commentary = Commentary::open(&mut disc.iso).unwrap();
        let mut sounds = Sounds {
            commentary: Some(commentary),
            interactive: true,
            ..Default::default()
        };
        let mut manager = SpeechManager::default();
        manager.init(&mut disc.iso, sounds.commentary.as_mut());
        let hoop = |x: f32| street_sim::Hoop {
            rim: [x, 11.75, 0.0],
            rim_radius: 0.75,
        };
        let players: Vec<_> = (0..6)
            .map(|k| street_sim::Player::new(k / 3, [if k < 3 { 4.0 } else { -4.0 }, 0.0], 0.0))
            .collect();
        let world = street_sim::World::new(
            [hoop(37.0), hoop(-37.0)],
            players,
            street_sim::Rules::default(),
            1,
        );
        let mut app = App::new();
        let mut setup = MatchSetup::new(&disc, 0, 1);
        if let Some(lineups) = lineups {
            setup.lineups = lineups;
        }
        app.insert_resource(setup)
            .insert_resource(disc)
            .insert_resource(sounds)
            .insert_resource(manager)
            .insert_resource(Assets::<Pcm>::default())
            .insert_resource(crate::game::Sim {
                world,
                inputs: vec![street_sim::Input::default(); 6],
                call: String::new(),
                pass_style: vec![street_sim::PassStyle::Normal; 6],
            })
            .init_resource::<TickEvents>()
            .insert_resource(story::Running(None))
            .insert_resource(story::Save {
                path: std::env::temp_dir()
                    .join(format!("street-commentary-{}.dat", std::process::id())),
                story: street_sim::story::Story::default(),
                last_error: None,
            })
            .insert_resource(AudioSettings::default())
            .insert_resource(MusicDuck::default())
            .add_systems(Update, (match_sounds, speech_service).chain());
        app
    }

    fn post(app: &mut App, events: Vec<Event>) {
        app.world_mut().resource_mut::<TickEvents>().events = events;
        app.update();
        app.world_mut().resource_mut::<TickEvents>().events.clear();
    }

    fn voices(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<CommentaryVoice>>()
            .iter(app.world())
            .count()
    }

    /// A trick move posts COMNTARY event 4 (`0x1a5a78`): one acceptance
    /// draw, then the service's order, chance and keyed draws, exactly as
    /// the chooser makes them on the same middleware state; the line is
    /// used up in its header and its stream plays.
    #[test]
    fn disc_trick_commentary_posts_event_4_and_serves_it_through_the_middleware() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let mut app = commentary_app(disc);
        let lineup = app.world().resource::<MatchSetup>().lineups;
        let sex = |record: usize| app.world().resource::<Disc>().players[record].female;
        let (actor, opponent) = (sex(lineup[0][0]), sex(lineup[1][0]));
        let mut random = app
            .world()
            .resource::<SpeechManager>()
            .middleware
            .random
            .clone();
        let mut reference =
            Commentary::open(&mut app.world_mut().resource_mut::<Disc>().iso).unwrap();
        reference.partition(0);
        random.mode0_index(100, u16::MAX);
        // Player 3 stands nearest player 0 (the first opponent in index order).
        let expected = reference
            .choose(&speech::move_packet(actor, Some(opponent)), &mut random)
            .unwrap();
        post(
            &mut app,
            vec![Event::Trick {
                player: 0,
                level: 1,
                script: 0,
                fakeout: Some(3),
                points: 500,
            }],
        );
        let manager = app.world().resource::<SpeechManager>();
        assert_eq!(manager.middleware.random, random);
        assert!(manager.middleware.queue.is_empty());
        assert!(manager.commentary_playing);
        let sounds = app.world().resource::<Sounds>();
        let header = &sounds.commentary.as_ref().unwrap().headers.headers[expected.choice.header];
        assert_eq!(header.recent(), [expected.choice.line]);
        assert!(!header.enabled(expected.choice.line));
        assert!(
            sounds.lines.len() == 1
                && sounds
                    .lines
                    .keys()
                    .all(|k| k.eq_ignore_ascii_case(&expected.name))
        );
        assert_eq!(voices(&mut app), 1);
    }

    /// Street School's narration (`0x1ad510`): a lesson launched with no
    /// page due is introduced at once (event 9, `Tutorial\TutLessons`);
    /// one with a page due waits, its introduction asked for every update
    /// with word 3 = 0 (no line), until the page closes; the page itself is
    /// read the update after it opens; a result is said at once.
    #[test]
    fn disc_street_school_narrates_launches_pages_and_results() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let mut app = commentary_app(disc);
        app.insert_resource(story::Running(Some(story::Run::School(0))))
            .init_resource::<Narration>();
        let line = |app: &App| {
            app.world()
                .resource::<Sounds>()
                .lines
                .keys()
                .any(|k| k.to_ascii_lowercase().starts_with(r"tutorial\tutlessons"))
        };
        app.world_mut().resource_mut::<Narration>().launch(0, None);
        app.update();
        assert!(line(&app), "the first lesson's introduction");
        assert_eq!(voices(&mut app), 1);

        // The Turbo lesson's page is due: nothing until it closes.
        let mut app = commentary_app(
            Disc::load(
                street_assets::iso::Iso::open(std::env::var("STREET_ISO").unwrap()).unwrap(),
            )
            .unwrap(),
        );
        app.insert_resource(story::Running(Some(story::Run::School(3))))
            .init_resource::<Narration>();
        let turbo = street_sim::school::page::TURBO;
        app.world_mut()
            .resource_mut::<Narration>()
            .launch(3, Some(turbo));
        for _ in 0..5 {
            app.update();
        }
        assert!(app.world().resource::<Narration>().intro_waits);
        assert_eq!(voices(&mut app), 0);
        // The page opens: read the update after.
        app.world_mut().resource_mut::<Narration>().page(turbo);
        app.update();
        assert_eq!(voices(&mut app), 0);
        app.update();
        assert!(line(&app), "the Turbo page's reading");
        assert_eq!(voices(&mut app), 1);
        assert!(!app.world().resource::<Narration>().page_waits);
        // Closed: the introduction comes once the reading is over.
        app.world_mut().resource_mut::<Narration>().page_closed();
        let voice = app
            .world_mut()
            .query_filtered::<Entity, With<CommentaryVoice>>()
            .single(app.world())
            .unwrap();
        app.world_mut().despawn(voice);
        app.update();
        app.update();
        assert!(!app.world().resource::<Narration>().intro_waits);
        assert_eq!(voices(&mut app), 1);
        // A result, at once (the stream free again).
        let voice = app
            .world_mut()
            .query_filtered::<Entity, With<CommentaryVoice>>()
            .single(app.world())
            .unwrap();
        app.world_mut().despawn(voice);
        app.update();
        app.world_mut().resource_mut::<Narration>().result(true);
        app.update();
        assert_eq!(voices(&mut app), 1);
    }

    /// `0x1a3b10`'s dunk posts event 1 with the dunker's identity words
    /// from their disc record (`FUN_001a7800`, `FUN_001a7948`,
    /// `FUN_001a7bd0`) under the flag slot 61's first call sets
    /// (`0x1a56b0`): six Legends keep the names on and Erving's dunks
    /// carry his bit, while a line-up with one named star (Pierce,
    /// record 5) switches the legend and star words off. Over a run of
    /// dunks the chooser's draws and the lines used up are the
    /// reference's for that packet, and at least once not those the other
    /// flag's packet would give.
    #[test]
    fn disc_a_dunk_packs_the_dunkers_identity_words() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        for (lineups, dunker, off, words) in [
            ([[158, 163, 164], [169, 170, 171]], 0, false, [0, 1, 0]),
            ([[0, 1, 2], [5, 6, 7]], 3, true, [0, 0, 0]),
        ] {
            let disc = Disc::load(street_assets::iso::Iso::open(&path).unwrap()).unwrap();
            let mut app = commentary_app_with(disc, Some(lineups));
            // Past the intro: the first update ends the first script and
            // counts the named players.
            app.world_mut()
                .resource_mut::<crate::game::Sim>()
                .world
                .tick = crate::hud::INTRO_TICKS;
            post(&mut app, Vec::new());
            let manager = app.world().resource::<SpeechManager>();
            assert_eq!(manager.scripts_ended, 1);
            assert_eq!(manager.name_words_off, off, "line-ups {lineups:?}");
            let record = app.world().resource::<MatchSetup>().lineups[dunker / 3][dunker % 3];
            let actor = app.world().resource::<Disc>().players[record].clone();
            assert_eq!(speech::dunker_words(actor.id, off), words);
            let other = speech::dunker_words(actor.id, !off);
            assert_ne!(other, words);
            let length = speech::game_length_word(
                match app.world().resource::<crate::game::Sim>().world.rules.goal {
                    Goal::Points(to) => to,
                    _ => 21,
                },
            );
            let packet = |words| speech::dunk_packet(false, words, actor.female, 0, length);
            let mut reference =
                Commentary::open(&mut app.world_mut().resource_mut::<Disc>().iso).unwrap();
            reference.partition(0);
            let mut decisive = false;
            for _ in 0..8 {
                // The reference choice on the same middleware state: the
                // acceptance draw, then the chooser's; the line is used up
                // as the service does.
                let mut random = app
                    .world()
                    .resource::<SpeechManager>()
                    .middleware
                    .random
                    .clone();
                random.mode0_index(100, u16::MAX);
                let mut unflagged = random.clone();
                let expected = reference.choose(&packet(words), &mut random).unwrap();
                let unexpected = reference.choose(&packet(other), &mut unflagged).unwrap();
                decisive |= (&expected.name, expected.choice.line)
                    != (&unexpected.name, unexpected.choice.line);
                expected.choice.commit(&mut reference.headers);
                post(
                    &mut app,
                    vec![Event::Score {
                        team: dunker / 3,
                        player: dunker,
                        points: 1,
                        basket: Basket::Dunk { advanced: false },
                    }],
                );
                let manager = app.world().resource::<SpeechManager>();
                assert_eq!(manager.middleware.random, random, "line-ups {lineups:?}");
                let sounds = app.world().resource::<Sounds>();
                let header =
                    &sounds.commentary.as_ref().unwrap().headers.headers[expected.choice.header];
                assert_eq!(
                    header.recent(),
                    reference.headers.headers[expected.choice.header].recent()
                );
                assert!(
                    sounds
                        .lines
                        .keys()
                        .any(|k| k.eq_ignore_ascii_case(&expected.name)),
                    "line-ups {lineups:?}: {:?}",
                    sounds.lines.keys().collect::<Vec<_>>()
                );
            }
            assert!(decisive, "line-ups {lineups:?}");
        }
    }

    /// `0x1ac9f8` type 1 drops the line while Bobbito talks unless the
    /// override (+0x68, until the tip-off) is set, but the middleware has
    /// chosen and used it up all the same; `0x1ad178` itself has no busy
    /// gate.
    #[test]
    fn disc_a_busy_commentator_drops_the_line_after_it_is_chosen() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let mut app = commentary_app(disc);
        let first = app.world_mut().spawn(CommentaryVoice).id();
        let mut manager = app.world_mut().resource_mut::<SpeechManager>();
        manager.commentary_playing = true;
        manager.override_busy = false;
        let before = app
            .world()
            .resource::<SpeechManager>()
            .middleware
            .random
            .clone();
        let block = Event::Block {
            player: 0,
            shooter: 3,
        };
        post(&mut app, vec![block]);
        let manager = app.world().resource::<SpeechManager>();
        assert_ne!(manager.middleware.random, before, "posted and served");
        assert!(manager.middleware.queue.is_empty());
        assert_eq!(voices(&mut app), 1, "no second voice over the first");
        assert!(app.world().resource::<Sounds>().lines.is_empty());
        let used: usize = app
            .world()
            .resource::<Sounds>()
            .commentary
            .as_ref()
            .unwrap()
            .headers
            .headers
            .iter()
            .map(|h| h.recent().len())
            .sum();
        assert_eq!(used, 1, "the chosen line is used up");
        // With the override (before the tip-off) the next line starts over
        // it, in the manager's one stream (`FUN_00351cc0`): it replaces the
        // line that played.
        app.world_mut()
            .resource_mut::<SpeechManager>()
            .override_busy = true;
        post(&mut app, vec![block]);
        assert_eq!(voices(&mut app), 1);
        assert!(
            app.world().get_entity(first).is_err(),
            "the old line is cut"
        );
    }

    /// The music's duck in the game's integers (`FUN_001abde8` over
    /// `FUN_001abe78`, `audio_internal_game_mus_vol` 98 and
    /// `audio_path_music_ducking` 35): 98 down to 63 at the default level,
    /// as a live capture of the original's path volume read for every line;
    /// a linear 100 ms ramp each way.
    #[test]
    fn the_music_ducks_to_the_originals_share_over_a_tenth_of_a_second() {
        assert_eq!(MusicDuck::ducked_share(10), 63.0 / 98.0);
        assert_eq!(MusicDuck::ducked_share(5), 31.0 / 49.0);
        assert_eq!(MusicDuck::ducked_share(0), 1.0);
        let mut duck = MusicDuck::default();
        assert!(!duck.step(0.016));
        duck.duck(10);
        assert!(duck.step(0.05));
        assert!((duck.level() - (1.0 + 63.0 / 98.0) / 2.0).abs() < 1e-6);
        duck.step(0.05);
        assert_eq!(duck.level(), 63.0 / 98.0);
        assert!(!duck.step(0.05));
        duck.restore();
        duck.step(0.1);
        assert_eq!(duck.level(), 1.0);
        duck.duck(10);
        duck.step(0.02);
        duck.reset();
        assert_eq!(duck, MusicDuck::default());
    }

    /// The queue's clock is the 100 Hz timer (`FUN_003998c0`, registered by
    /// `0x1ac6f0`): hundredths of a second at each 60 Hz update, so the
    /// roots' hold of 200 lasts two seconds (120 updates).
    #[test]
    fn the_speech_clock_counts_the_timers_hundredths() {
        assert_eq!(SpeechManager::hundredths(0), 0);
        assert_eq!(SpeechManager::hundredths(3), 5);
        assert_eq!(SpeechManager::hundredths(60), 100);
        assert_eq!(SpeechManager::hundredths(120), 200);
        let mut manager = SpeechManager::default();
        for _ in 0..121 {
            manager.tick();
        }
        assert_eq!(manager.clock, 201);
    }

    /// A line starting ducks the match music (`FUN_001abde8`) and its end
    /// (the stream run out, `0x1acf58`) brings it back (`FUN_001abe78`).
    #[test]
    fn disc_a_commentary_line_ducks_the_music_until_it_ends() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let mut app = commentary_app(disc);
        let block = Event::Block {
            player: 0,
            shooter: 3,
        };
        post(&mut app, vec![block]);
        assert_eq!(voices(&mut app), 1);
        let mut duck = *app.world().resource::<MusicDuck>();
        duck.step(1.0);
        assert_eq!(duck.level(), MusicDuck::ducked_share(10));
        // The stream runs out.
        let voice = app
            .world_mut()
            .query_filtered::<Entity, With<CommentaryVoice>>()
            .single(app.world())
            .unwrap();
        app.world_mut().despawn(voice);
        post(&mut app, Vec::new());
        assert!(!app.world().resource::<SpeechManager>().commentary_playing);
        let mut duck = *app.world().resource::<MusicDuck>();
        duck.step(1.0);
        assert_eq!(duck.level(), 1.0);
    }

    /// `0x1acf58` serves the players' queue (`FUN_00364620(0)`) before the
    /// commentary's (`FUN_00364620(1)`), both from the one generator: in a
    /// frame with a request of each, the players' chooser takes the first
    /// draws. A live capture of the original read players' requests at
    /// queue manager 0 and the commentary's at 1.
    #[test]
    fn disc_the_players_are_served_before_the_commentary() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let mut app = commentary_app(disc);
        let request = speech::PlayerRequest {
            voice: 1,
            category_mask: 1 << 3,
            shot_mask: 0,
            action_mask: 2,
        };
        let court = speech::words_packet(3, &[1, 1]);
        {
            let world = app.world_mut();
            let commentary = world.resource::<Sounds>().commentary.clone().unwrap();
            let mut manager = world.resource_mut::<SpeechManager>();
            manager.request_player(
                0,
                Some(request.voice),
                (10, 200),
                [
                    request.category_mask,
                    request.shot_mask,
                    request.action_mask,
                ],
            );
            manager.request_commentary(&commentary, court);
        }
        let manager = app.world().resource::<SpeechManager>();
        assert_eq!(manager.middleware.queue.count(speech::PLAYERS), 1);
        assert_eq!(manager.middleware.queue.count(speech::COMMENTARY), 1);
        let random = manager.middleware.random.clone();
        let players = manager.players.clone();
        let commentary = app.world().resource::<Sounds>().commentary.clone().unwrap();
        let in_order = |players_first: bool| {
            let mut random = random.clone();
            let (player, line) = if players_first {
                let player = players.select_request(request, &mut random).unwrap();
                (player, commentary.choose(&court, &mut random).unwrap())
            } else {
                let line = commentary.choose(&court, &mut random).unwrap();
                (players.select_request(request, &mut random).unwrap(), line)
            };
            (player, line.name, line.offset)
        };
        let expected = in_order(true);
        assert_ne!(expected, in_order(false), "the order shows");
        post(&mut app, Vec::new());
        let manager = app.world().resource::<SpeechManager>();
        let mut reference = players.clone();
        reference.commit(&expected.0);
        let rings = |p: &speech::PlayerSpeech| -> Vec<Vec<usize>> {
            p.headers.headers.iter().map(|h| h.recent()).collect()
        };
        assert_eq!(rings(&manager.players), rings(&reference));
        let sounds = app.world().resource::<Sounds>();
        let played: Vec<&String> = sounds.lines.keys().collect();
        assert_eq!(played.len(), 1);
        assert!(
            played[0].eq_ignore_ascii_case(&expected.1),
            "the commentary's line after the players' draws: {played:?}"
        );
    }

    /// Speech pauses and resumes with the pause menu (slot 57, `0x1a7198`),
    /// not whenever `Paused` is written (`game.rs` sets it every frame): a
    /// replay's pause (slot 58) holds through frames of play.
    #[test]
    fn speech_follows_the_pause_menu_not_the_frames() {
        let mut app = App::new();
        let manager = SpeechManager {
            initialised: true,
            up: true,
            ..Default::default()
        };
        app.insert_resource(manager)
            .insert_resource(Sounds {
                interactive: true,
                ..Default::default()
            })
            .init_resource::<Paused>()
            .add_systems(Update, pause_audio);
        app.update();
        app.world_mut().resource_mut::<SpeechManager>().pause(false);
        for _ in 0..3 {
            app.world_mut().resource_mut::<Paused>().on = false;
            app.update();
        }
        assert!(app.world().resource::<SpeechManager>().paused, "held");
        app.world_mut()
            .resource_mut::<SpeechManager>()
            .resume(false);
        app.world_mut().resource_mut::<Paused>().on = true;
        app.update();
        assert!(app.world().resource::<SpeechManager>().paused);
        app.world_mut().resource_mut::<Paused>().on = false;
        app.update();
        assert!(!app.world().resource::<SpeechManager>().paused);
    }

    /// The boot's C library draws in the original's order: the song and the
    /// speech counter from the session (`session::Session`), the front
    /// end's first initialisation counting it up once, a match's again, as
    /// every captured boot's first match reads 20.
    #[test]
    fn disc_the_speech_counter_starts_from_the_boots_c_library() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let mut app = App::new();
        app.insert_resource(disc)
            .init_resource::<Sounds>()
            .init_resource::<SpeechManager>()
            .insert_resource(crate::session::Session::pinned(79_638))
            .add_systems(Update, load_front_end);
        app.update();
        let sounds = app.world().resource::<Sounds>();
        assert_eq!((sounds.load_loop, sounds.path_song), (2, 6));
        let manager = app.world().resource::<SpeechManager>();
        assert_eq!(manager.counter(), 19);
        assert!(!manager.up, "shut down until a match");
        assert_eq!(
            manager.middleware.random,
            speech::SpeechMiddleware::new(18).random
        );
        let (mut disc, mut sounds) = (
            app.world_mut().remove_resource::<Disc>().unwrap(),
            app.world_mut().remove_resource::<Sounds>().unwrap(),
        );
        let mut manager = app.world_mut().resource_mut::<SpeechManager>();
        manager.init(&mut disc.iso, sounds.commentary.as_mut());
        assert_eq!(manager.counter(), 20);
        assert!(manager.up);
    }

    /// `0x1ad178`'s own gates drop a request before its draw: the Playlist
    /// Music Experience (`FUN_001a0db8`), Street School, a staged script.
    #[test]
    fn disc_closed_commentary_takes_no_middleware_draw() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let mut app = commentary_app(disc);
        let trick = Event::Trick {
            player: 0,
            level: 1,
            script: 0,
            fakeout: Some(3),
            points: 500,
        };
        app.world_mut().resource_mut::<Sounds>().interactive = false;
        let before = app
            .world()
            .resource::<SpeechManager>()
            .middleware
            .random
            .clone();
        post(&mut app, vec![trick]);
        assert_eq!(
            app.world().resource::<SpeechManager>().middleware.random,
            before
        );
        app.world_mut().resource_mut::<Sounds>().interactive = true;
        app.insert_resource(story::Running(Some(story::Run::School(0))));
        post(&mut app, vec![trick]);
        assert_eq!(
            app.world().resource::<SpeechManager>().middleware.random,
            before
        );
        let manager = app.world().resource::<SpeechManager>();
        let sounds = app.world().resource::<Sounds>();
        assert!(manager.commentary_open(sounds, false, false));
        assert!(!manager.commentary_open(sounds, true, false));
        assert_eq!(voices(&mut app), 0);
    }

    /// Every fifth steal (`DAT_004bb2d4`) posts the Turnover event 0xd
    /// instead of 8; every third violation (`DAT_004bb2e0`) 0xd, else
    /// ShotClock 0x11.
    #[test]
    fn disc_steal_and_violation_counters_pick_their_events() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let mut app = commentary_app(disc);
        // Keep the commentator quiet so each pick is visible as a file.
        app.insert_resource(AudioSettings {
            speech: 0,
            ..Default::default()
        });
        let steal = Event::Steal {
            player: 0,
            from: 3,
            intercepted_pass: false,
        };
        let used = |app: &App, prefix: &str| -> usize {
            app.world()
                .resource::<Sounds>()
                .commentary
                .as_ref()
                .unwrap()
                .headers
                .headers
                .iter()
                .filter(|h| h.name.starts_with(prefix))
                .map(|h| h.recent().len())
                .sum()
        };
        for n in 1..=5 {
            post(&mut app, vec![steal]);
            assert_eq!(used(&app, "Turnover\\"), usize::from(n == 5), "steal {n}");
        }
        assert_eq!(used(&app, "Steal\\"), 4);
        // ShotClock's root accepts three requests in four (chance 75), so
        // only the Turnover pick is certain.
        for _ in 1..=3 {
            post(&mut app, vec![Event::ShotClockViolation { team: 0 }]);
        }
        assert!(used(&app, "ShotClock") <= 2);
        assert_eq!(used(&app, "Turnover\\"), 2);
        assert_eq!(app.world().resource::<SpeechManager>().violations, 3);
        assert_eq!(app.world().resource::<SpeechManager>().steals, 5);
    }

    /// The manager lasts the session: a second match's initialisation keeps
    /// the middleware's state and the rings, and runs the partitions again
    /// with the next count (`FUN_00363230`).
    #[test]
    fn disc_the_speech_manager_outlives_a_match() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let mut commentary = Commentary::open(&mut iso).unwrap();
        let mut manager = SpeechManager::default();
        manager.init(&mut iso, Some(&mut commentary));
        let block_a = commentary.headers.by_kind(0).unwrap();
        let on = |c: &Commentary| -> Vec<usize> {
            let h = &c.headers.headers[block_a];
            (0..h.lines.len()).filter(|&i| h.enabled(i)).collect()
        };
        // BlockA: 73 lines in 4 runs; run 0 is lines 0..18.
        assert_eq!(on(&commentary), (0..18).collect::<Vec<_>>());
        manager.middleware.random.mode0_index(100, u16::MAX);
        let state = manager.middleware.random.clone();
        manager.override_busy = false;
        manager.init(&mut iso, Some(&mut commentary));
        assert_eq!(manager.middleware.random, state, "seeded once");
        assert!(manager.override_busy);
        assert_eq!(on(&commentary), (18..36).collect::<Vec<_>>());
    }

    /// `0x1a83e0` in Be A Legend: a matching pending nickname posts event
    /// 0x12 instead of the trick's Move request and is spent at once, even
    /// when the commentator then stays quiet; while Bobbito talks (+0x14)
    /// there is no attempt and the Move request is posted.
    #[test]
    fn disc_a_pending_nickname_replaces_the_move_request_and_is_spent() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let row = disc
            .nicknames
            .iter()
            .enumerate()
            .position(|(i, n)| i > 0 && n.skill == Some(street_assets::db::Skill::Handles))
            .unwrap();
        let id = disc.nicknames[row].id as u16;
        let actor = MatchSetup::new(&disc, 0, 1).lineups[0][0];
        disc.players[actor].fields[6] = row as i16;
        let female = disc.players[actor].female;
        let mut app = commentary_app(disc);
        app.insert_resource(story::Running(Some(story::Run::StoryEvent(0))));
        app.insert_resource(AudioSettings {
            speech: 0,
            ..Default::default()
        });
        app.world_mut()
            .resource_mut::<story::Save>()
            .story
            .nickname_pending = true;
        let trick = Event::Trick {
            player: 0,
            level: 1,
            script: 0,
            fakeout: Some(3),
            points: 500,
        };
        // Bobbito talking: no attempt; the pending flag stays and Move is
        // posted and served.
        app.world_mut().spawn(CommentaryVoice);
        app.world_mut()
            .resource_mut::<SpeechManager>()
            .commentary_playing = true;
        post(&mut app, vec![trick]);
        assert!(app.world().resource::<story::Save>().story.nickname_pending);
        let used = |app: &App, prefix: &str| -> usize {
            app.world()
                .resource::<Sounds>()
                .commentary
                .as_ref()
                .unwrap()
                .headers
                .headers
                .iter()
                .filter(|h| h.name.starts_with(prefix))
                .map(|h| h.recent().len())
                .sum()
        };
        assert_eq!(used(&app, "Move\\"), 1);
        // Quiet: the nickname's own request, served with its id and sex.
        let voice = app
            .world_mut()
            .query_filtered::<Entity, With<CommentaryVoice>>()
            .single(app.world())
            .unwrap();
        app.world_mut().despawn(voice);
        app.world_mut()
            .resource_mut::<SpeechManager>()
            .commentary_playing = false;
        let mut expected = app
            .world()
            .resource::<SpeechManager>()
            .middleware
            .random
            .clone();
        let reference = app
            .world()
            .resource::<Sounds>()
            .commentary
            .as_ref()
            .unwrap()
            .headers
            .clone();
        expected.mode0_index(100, u16::MAX);
        let event = app
            .world()
            .resource::<Sounds>()
            .commentary
            .as_ref()
            .unwrap()
            .event(0x12)
            .unwrap()
            .clone();
        let choice = event
            .choose(
                &speech::nickname_packet(id, female),
                &reference,
                &mut expected,
            )
            .unwrap();
        post(&mut app, vec![trick]);
        assert!(!app.world().resource::<story::Save>().story.nickname_pending);
        assert_eq!(used(&app, "Move\\"), 1, "no Move request beside it");
        assert_eq!(
            app.world().resource::<SpeechManager>().middleware.random,
            expected
        );
        assert!(
            reference.headers[choice.header]
                .name
                .starts_with("NicknameIntro\\")
        );
        // Spent: the next trick posts Move again.
        post(&mut app, vec![trick]);
        assert_eq!(used(&app, "Move\\"), 2);
    }
}
