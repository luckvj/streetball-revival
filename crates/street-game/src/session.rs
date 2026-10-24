//! The session's generator: the original's shared `FUN_003a16b8` generator
//! (`0x4309f0`, [`street_sim::Rng`]), one for the whole time the game runs
//! (docs/TRACE.md "The session's generator").
//!
//! - The boot seeds it from the clock (`FUN_002f5578`:
//!   [`street_sim::rng::boot_seed`] of the console's clock).
//! - `FUN_001dbe30` reseeds it from the 100 Hz timer's count since the boot
//!   (`0x4b32e4`, `FUN_003998c0`) whenever the front end sets a team for a
//!   side no person holds (`FUN_001dbb20`: Choose Team's commit, the
//!   attract match's picks, a story event's opponent): [`Session::reseed`].
//! - Every game is built on it as it stands
//!   ([`street_sim::World::from_generator`]) and runs it on; the generator
//!   the match leaves goes on to the next ([`Session::carry_on`]), so a
//!   rematch draws from the state the last game ended in. Nothing else
//!   reseeds it: eight games in one captured boot all walk back to its one
//!   reseed.
//!
//! The front end's own draws from it are not ported (the rewrite's menus
//! roll their own) but for the loading cards' start at the boot
//! ([`Session::draw`]), so the stream between a reseed and the next game is
//! the construction's alone. `FUN_001dbe30`'s own draws, for the side's three,
//! come from the C library's generator ([`Session::choose`]).
//!
//! The session also keeps the C library's generator ([`Libc`]), which the
//! boot and `FUN_001dbe30` seed with the same values, and which the audio
//! draws its first values from (docs/TRACE.md "The audio's generator, the
//! commentator's duck and the speech clock"):
//!
//! - the audio controller's constructor (`FUN_0019f810`, called from the
//!   boot `FUN_002f4fb8` straight after the clock's seed) draws the loading
//!   loop `rand() % 7` and the in-game song `rand() % 13 + 1`
//!   ([`Session::audio_controller`]);
//! - `AuController::Init` (`FUN_0019f910`) ends with the path manager's
//!   constructor (`srand(timer)`, `0x1aa240`), the speech manager's
//!   (`rand() % 255`, its counter, `0x1ac620`) and the ambience manager's
//!   (`srand(timer)` again, `0x1a8910`) ([`Session::speech_counter`]).
//!
//! A live boot of the original in PCSX2 read `srand(79638)`, the two draws
//! (2 and 6), then `srand(632)`, the counter 18 and `srand(632)`; every save
//! state of two other boots holds a middleware generator seeded with 18.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use bevy::prelude::Resource;
use street_sim::Rng;

use crate::setup::MatchSetup;

/// The timer's count when `AuController::Init` seeds the C library's
/// generator on a pinned session: 632 hundredths, what a live boot of the
/// original in PCSX2 (fast boot) read there. It gives the counter 18 that
/// every captured boot's speech manager started from.
pub const PINNED_AUDIO_INIT_COUNT: u32 = 632;

/// The C library's generator (`FUN_003b42a0`, `rand`; `FUN_003b4290`,
/// `srand`): one word at `*0x434284 + 0x58` (newlib's reentrancy block),
/// stepped as `x × 1103515245 + 12345` (`mult` and `addiu 0x3039` at
/// `0x3b42bc`), `rand` returning its low 31 bits. `srand` stores the seed
/// itself, and the word starts at 1 before the boot seeds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Libc(u32);

impl Libc {
    /// `srand(seed)`.
    pub fn seeded(seed: u32) -> Self {
        Libc(seed)
    }

    /// `rand()`.
    pub fn rand(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        self.0 & 0x7fff_ffff
    }

    /// The word, as a save state holds it.
    pub fn state(self) -> u32 {
        self.0
    }
}

/// The generator across matches.
#[derive(Resource, Clone, Debug)]
pub struct Session {
    rng: Rng,
    /// The C library's generator, seeded alongside `rng`.
    libc: Libc,
    /// When the session began, for the timer's count; `None` for a pinned
    /// session ([`Session::pinned`]), which keeps one stream.
    began: Option<Instant>,
}

impl Session {
    /// Seeded as the boot seeds it, from the clock's hour, minute and
    /// second. The console's clock keeps Japan time (UTC+9), which is what
    /// `sceCdReadClock` reads.
    pub fn boot() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let japan = now + 9 * 3600;
        let (hour, minute, second) = (japan / 3600 % 24, japan / 60 % 60, japan % 60);
        // `FUN_002f5578` seeds both generators with the one value.
        let seed = street_sim::rng::boot_seed(hour as u8, minute as u8, second as u8);
        Self {
            rng: Rng::new(seed),
            libc: Libc::seeded(seed),
            began: Some(Instant::now()),
        }
    }

    /// One stream from `seed`, for runs that must repeat (`--seed`, and the
    /// scripted runs): the reseeds read the wall clock, which a second run
    /// would not read the same, so a pinned session ignores them. Both
    /// generators start from `seed`, as the boot's seed would start them.
    pub fn pinned(seed: u32) -> Self {
        Self {
            rng: Rng::new(seed),
            libc: Libc::seeded(seed),
            began: None,
        }
    }

    /// The timer's count, hundredths of a second since the session began
    /// (`FUN_003998c0`); `None` on a pinned session.
    fn count(&self) -> Option<u32> {
        self.began
            .map(|began| (began.elapsed().as_millis() / 10) as u32)
    }

    /// `FUN_001dbe30`: a team set for a side no person holds seeds both
    /// generators from the timer's count, hundredths of a second since the
    /// session began (`FUN_003995c0` runs timer 0 at 100 Hz from the boot).
    /// The C library's draws it then makes for the side's three are
    /// [`Session::choose`]'s.
    pub fn reseed(&mut self) {
        if let Some(count) = self.count() {
            self.rng = Rng::new(count);
            self.libc = Libc::seeded(count);
        }
    }

    /// `FUN_001dbe30`'s three for `team` ([`crate::lineup::choose`]). With
    /// no seed it draws from the C library's generator as the last
    /// [`Session::reseed`] left it; with one (a tournament's) both
    /// generators start from it, and end seeded from the timer again (a
    /// pinned session keeps what it had).
    pub fn choose(
        &mut self,
        book: &crate::lineup::Book,
        mode: &crate::lineup::Mode,
        team: usize,
        other: crate::lineup::Lineup,
        seed: Option<u32>,
    ) -> [usize; 3] {
        let before = self.libc;
        let three = crate::lineup::choose(book, mode, team, other, seed, &mut self.libc);
        if seed.is_some() {
            match self.count() {
                Some(count) => {
                    self.rng = Rng::new(count);
                    self.libc = Libc::seeded(count);
                }
                None => self.libc = before,
            }
        }
        three
    }

    /// `FUN_0019f810`, the audio controller's constructor, the first thing
    /// to draw from the C library after the boot's seed: the loading
    /// screens' loop (`rand() % 7`, controller +0x24, `music/loadloop`, which
    /// `FUN_001a0e18` turns through) and the in-game song
    /// (`rand() % 13 + 1`, `DAT_003ea23c`). The retail disc's
    /// `audio_demo_cd` is 0, which would make them 6 and 3.
    pub fn audio_controller(&mut self) -> (u32, i32) {
        let load_loop = self.libc.rand() % 7;
        let song = (self.libc.rand() % 13) as i32 + 1;
        (load_loop, song)
    }

    /// The end of `AuController::Init` (`FUN_0019f910`): the path manager's
    /// constructor seeds the C library from the timer (`0x1aa240`), the
    /// speech manager's draws its counter `rand() % 255` (`0x1ac620`,
    /// manager +4), and the ambience manager's seeds it from the timer again
    /// (`0x1a8910`), within the same hundredth. A pinned session reads
    /// [`PINNED_AUDIO_INIT_COUNT`].
    pub fn speech_counter(&mut self) -> u32 {
        let count = self.count().unwrap_or(PINNED_AUDIO_INIT_COUNT);
        self.libc = Libc::seeded(count);
        let counter = self.libc.rand() % 255;
        self.libc = Libc::seeded(count);
        counter
    }

    /// The C library's generator as it stands.
    pub fn libc(&self) -> Libc {
        self.libc
    }

    /// [`Session::reseed`] after `side`'s team was set, when no person
    /// holds that side (`FUN_001dbb20` asks `FUN_001db280` for the
    /// controllers on it), as the match will seat them.
    pub fn team_set(&mut self, setup: &MatchSetup, side: usize) {
        if !held(setup, side) {
            self.reseed();
        }
    }

    /// A draw of the front end's own from the shared generator: the
    /// loading cards' start (`FUN_0029a510`), made at the boot's class 4
    /// (`FUN_0029dca8`) after the clock's seed and before any reseed.
    pub fn draw(&mut self) -> u32 {
        self.rng.next_u32()
    }

    /// The generator a new game is built on.
    pub fn generator(&self) -> Rng {
        self.rng.clone()
    }

    /// The match is over (or left): the generator runs on from where it
    /// left it.
    pub fn carry_on(&mut self, generator: &Rng) {
        self.rng = generator.clone();
    }
}

/// Whether a person plays `side` in the match `setup` describes, as
/// `setup::load_match` seats them.
pub fn held(setup: &MatchSetup, side: usize) -> bool {
    if setup.online {
        true
    } else if setup.demo {
        false
    } else if setup.seats.is_empty() {
        side == 0
    } else {
        setup.seats.iter().any(|&(_, team)| team == side)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> MatchSetup {
        MatchSetup {
            teams: [0, 1],
            lineups: [[0; 3]; 2],
            court: "RUCKER".into(),
            rules: street_sim::Rules::default(),
            seed: 2002,
            demo: false,
            watch: false,
            seats: Vec::new(),
            users: Vec::new(),
            online: false,
        }
    }

    #[test]
    fn a_pinned_session_keeps_one_stream_and_a_clocked_one_reseeds() {
        let mut pinned = Session::pinned(2002);
        pinned.team_set(&setup(), 1);
        assert_eq!(pinned.generator(), Rng::new(2002));
        // The person's side does not reseed; the computer's does, from a
        // count of hundredths (a session just begun counts near 0).
        let mut clocked = Session::boot();
        let before = clocked.generator();
        clocked.team_set(&setup(), 0);
        assert_eq!(clocked.generator(), before);
        clocked.team_set(&setup(), 1);
        let seed = clocked.generator().seeded_by().unwrap();
        assert!(seed < 1_000, "{seed}");
        // Watched or online: both sides the computer's, or both people's.
        let mut demo = setup();
        demo.demo = true;
        assert!(!held(&demo, 0) && !held(&demo, 1));
        let mut online = setup();
        online.online = true;
        assert!(held(&online, 0) && held(&online, 1));
    }

    /// The C library's generator on the values a live boot of the original
    /// read through PINE (`%TEMP%/street-original-speech-attract-1.jsonl`):
    /// the boot's seed 79,638, the controller's two draws leaving
    /// 0x176aea84, then `AuController::Init` at 632 hundredths, the counter
    /// 18 and the word back at 632.
    #[test]
    fn the_audio_draws_its_first_values_from_the_c_library_in_the_boots_order() {
        assert_eq!(Libc::seeded(1).rand(), 0x41c6_7ea6);
        let mut session = Session::pinned(79_638);
        assert_eq!(session.audio_controller(), (2, 6));
        assert_eq!(session.libc().state(), 0x176a_ea84);
        assert_eq!(session.speech_counter(), 18);
        assert_eq!(session.libc().state(), PINNED_AUDIO_INIT_COUNT);
        // The counter depends on the timer's count alone, whatever the
        // boot's seed.
        let mut other = Session::pinned(2002);
        other.audio_controller();
        assert_eq!(other.speech_counter(), 18);
        // The counts that give 18 come every 146 hundredths.
        let counter = |count: u32| Libc::seeded(count).rand() % 255;
        let eighteens: Vec<u32> = (0..1000).filter(|&c| counter(c) == 18).collect();
        assert_eq!(eighteens, [48, 194, 340, 486, 632, 778, 924]);
        // A reseed seeds both generators with one count.
        let mut clocked = Session::boot();
        clocked.reseed();
        let seed = clocked.generator().seeded_by().unwrap();
        assert_eq!(clocked.libc().state(), seed);
    }

    #[test]
    fn a_match_hands_its_generator_on_to_the_next() {
        let mut session = Session::pinned(7);
        let world = street_sim::World::from_generator(
            street_sim::Hoop::native(),
            Vec::new(),
            street_sim::Rules::default(),
            session.generator(),
        );
        assert_ne!(world.generator(), &session.generator());
        session.carry_on(world.generator());
        assert_eq!(&session.generator(), world.generator());
        // The rematch is built on it: another pair, from further on.
        let rematch = street_sim::World::from_generator(
            street_sim::Hoop::native(),
            Vec::new(),
            street_sim::Rules::default(),
            session.generator(),
        );
        let mut expect = session.generator();
        for _ in 0..street_sim::CONSTRUCTION_DRAWS {
            expect.next_u32();
        }
        let early = expect.next_u32() % 6;
        assert_eq!(rematch.stage_bounds, [early, 15 + expect.next_u32() % 3]);
    }
}
