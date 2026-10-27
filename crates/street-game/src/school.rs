//! Street School's 26 lessons and their selected-user practice runs.
//!
//! The listener at 0x18df00 and manager at 0x18eec0 keep action-history
//! latches and delay results. This adapter consumes equivalent simulation
//! callbacks; raw actor/animation fields absent from the sim remain open.
//! See docs/SCHOOL_PROFILE_RECOVERY.md and the retained School trigger probe.

use bevy::prelude::*;
use street_sim::World;

use crate::story;

/// One lesson: its title, the prompt the game's own text uses, and the task
/// id the table carries.
#[derive(Clone, Copy, Debug)]
pub struct Lesson {
    pub title: &'static str,
    pub prompt: &'static str,
    /// The input category from the table's `+0x1c`; the lesson index is the task.
    pub action: i8,
}

pub const LESSONS: [Lesson; 26] = [
    Lesson {
        title: "Pass",
        prompt: "press",
        action: 0,
    },
    Lesson {
        title: "Short Range Shot",
        prompt: "press",
        action: 1,
    },
    Lesson {
        title: "Long Range Shot",
        prompt: "press",
        action: 1,
    },
    Lesson {
        title: "Dunks",
        prompt: "press any turbo +",
        action: 1,
    },
    Lesson {
        title: "Dunk to Lay-up",
        prompt: "press any turbo +",
        action: 1,
    },
    Lesson {
        title: "Alley-Oop",
        prompt: "press",
        action: 0,
    },
    Lesson {
        title: "Trick Move",
        prompt: "press",
        action: 2,
    },
    Lesson {
        title: "Switch Players",
        prompt: "press",
        action: 0,
    },
    Lesson {
        title: "Steal",
        prompt: "press",
        action: 2,
    },
    Lesson {
        title: "Block",
        prompt: "press",
        action: 3,
    },
    Lesson {
        title: "Trick Counter",
        prompt: "press 2 turbos +",
        action: 2,
    },
    Lesson {
        title: "Turbo Block",
        prompt: "press 2 turbos +",
        action: 3,
    },
    Lesson {
        title: "Pump Fake",
        prompt: "tap",
        action: 1,
    },
    Lesson {
        title: "Fadeaway",
        prompt: "move away from hoop and press",
        action: 1,
    },
    Lesson {
        title: "Gamebreaker",
        prompt: "press 2 turbos +",
        action: 1,
    },
    Lesson {
        title: "Pick",
        prompt: "press",
        action: 6,
    },
    Lesson {
        title: "Dive",
        prompt: "double tap any turbo",
        action: -1,
    },
    Lesson {
        title: "Kick Pass",
        prompt: "press",
        action: 0,
    },
    Lesson {
        title: "Off the Heezay",
        prompt: "press 2 turbos +",
        action: 0,
    },
    Lesson {
        title: "Back 2 Papa",
        prompt: "move towards hoop + 2 turbos +",
        action: 0,
    },
    Lesson {
        title: "Advanced Trick Move",
        prompt: "press all turbos +",
        action: 2,
    },
    Lesson {
        title: "Advanced Dunks",
        prompt: "press all turbos +",
        action: 1,
    },
    Lesson {
        title: "Give 'n Go",
        prompt: "move",
        action: 7,
    },
    Lesson {
        title: "Pocket Gamebreaker",
        prompt: "press",
        action: 4,
    },
    Lesson {
        title: "Cancel Gamebreaker",
        prompt: "press any turbo +",
        action: 5,
    },
    Lesson {
        title: "Level 2 Gamebreaker",
        prompt: "press 2 turbos +",
        action: 1,
    },
];

/// Match-owned native lesson history ([`street_sim::school::Run`]).
/// Committed results belong to LessonPassed; this resource never writes a
/// profile itself.
#[derive(Resource, Default)]
pub struct LessonRun(street_sim::school::Run);

pub use street_sim::school::Before;

/// The lesson's launch, `FUN_0018ec28`: the side it starts with the ball
/// (`FUN_0018ebe0`: the person's, the other for 7..11 and 16), the restart
/// at the spots and the gamebreaker lessons' meters
/// ([`World::launch_lesson`]). The game's vt+0xb4 is the lesson's index:
/// the meter stops at 49,999 outside the gamebreaker lessons (14, 23..25;
/// `FUN_00143d38`). The tutorial page due, which the narration waits for
/// ([`crate::audio::Narration::launch`]).
pub fn stage_world(
    world: &mut World,
    lesson: usize,
    selected: usize,
    seen: u8,
    inputs: &[street_sim::Input],
) -> Option<u8> {
    let side = world.team_of(selected);
    let offense = street_sim::school::expected_side(lesson, side);
    let page = street_sim::school::page_due(lesson, seen);
    world.launch_lesson(lesson as u8, offense, page.is_some(), inputs);
    page
}

impl LessonRun {
    /// The fixed step's callbacks through the sim's lesson manager. A
    /// result is returned once the native pending delay has elapsed; the
    /// caller then commits it and stops advancing this run.
    pub fn advance(&mut self, lesson: usize, before: Before, world: &World) -> Option<bool> {
        self.0.advance(lesson, before, world)
    }

    /// What the manager posted in the last update.
    pub fn posts(&self) -> &[street_sim::school::Post] {
        self.0.posts()
    }

    pub fn started(&self) -> bool {
        self.0.started()
    }

    /// The tutorial pages the user has seen.
    pub fn set_seen(&mut self, seen: u8) {
        self.0.seen = seen;
    }
}

/// The tutorial pages the selected user has seen: their lesson nibbles
/// 26..32 with bit 3 set (`FUN_001eebb8`), page 0x1a as bit 0.
pub fn pages_seen(progress: &story::Story) -> u8 {
    progress.school_profile.challenges[26..33]
        .iter()
        .enumerate()
        .fold(0, |bits, (k, &v)| bits | (u8::from(v & 8 != 0) << k))
}

/// `FUN_001eedb0`: a page seen, its nibble 8.
pub fn see_page(progress: &mut story::Story, page: u8) {
    if let Some(v) = progress
        .school_profile
        .challenges
        .get_mut(usize::from(page))
        .filter(|_| (26..33).contains(&page))
    {
        *v = 8;
        progress.sync_school_user();
    }
}

/// Native School binds the selected controller's live user payload.
pub fn begin_for_setup(progress: &mut story::Story, setup: &crate::setup::MatchSetup) {
    let user = setup
        .users
        .first()
        .and_then(Option::as_deref)
        .and_then(|name| progress.records.user(name));
    progress.select_school_user(user);
}

fn bind_for_setup(progress: &mut story::Story, setup: &crate::setup::MatchSetup) -> bool {
    // One person, on the side the School seats them (the Lakers, side 1).
    if setup.demo || setup.online || setup.seats.len() != 1 {
        return false;
    }
    let selected = setup
        .users
        .first()
        .and_then(Option::as_deref)
        .and_then(|name| progress.records.user(name))
        .map(|index| progress.records.users[index].name.clone());
    if !progress.school_profile_selected || progress.school_user != selected {
        begin_for_setup(progress, setup);
    }
    true
}

pub fn start_for_setup(
    progress: &mut story::Story,
    setup: &crate::setup::MatchSetup,
    lesson: usize,
) -> bool {
    bind_for_setup(progress, setup) && progress.start_school_lesson(lesson)
}

pub fn finish_for_setup(
    progress: &mut story::Story,
    setup: &crate::setup::MatchSetup,
    lesson: usize,
    passed: bool,
) -> bool {
    bind_for_setup(progress, setup) && progress.finish_school_lesson(lesson, passed)
}

pub fn completion_rewards_for_setup(
    progress: &mut story::Story,
    setup: &crate::setup::MatchSetup,
) -> Vec<street_sim::story::Notice> {
    if !bind_for_setup(progress, setup) {
        return Vec::new();
    }
    progress.school_completion_rewards()
}

/// The School's teams (`FUN_001db968`): the Nets and the Lakers, each
/// its first three (`team * 5 + k`).
pub const NETS: usize = 16;
pub const LAKERS: usize = 11;

/// The School's one game (`FUN_001dba68` mode 1, `FUN_001db968`): court 0
/// (`FUN_001dbc88(.., 0)`), the Nets against the Lakers, the person on the
/// Lakers (side 1, as the School's captures hold it); native lesson
/// callbacks are separate.
pub fn prepare(
    disc: &crate::setup::Disc,
    setup: &mut crate::setup::MatchSetup,
) -> Result<(), String> {
    setup.rules = street_sim::Rules::default();
    // 99 to win is unreachable: each lesson ends itself on the pass or the
    // failure callback (the game side drives both), and the game runs
    // until it is quit or every lesson is done.
    setup.rules.goal = street_sim::Goal::Points(99);
    setup.court = disc
        .courts
        .first()
        .map(|c| c.id.clone())
        .ok_or("the disc has no courts")?;
    for (side, team) in [(0, NETS), (1, LAKERS)] {
        let roster = disc
            .teams
            .get(team)
            .map(|t| t.roster.clone())
            .filter(|r| r.len() >= 3)
            .ok_or("the School's teams are missing")?;
        setup.teams[side] = team;
        setup.lineups[side] = std::array::from_fn(|k| roster[k]);
    }
    for seat in &mut setup.seats {
        seat.1 = 1;
    }
    Ok(())
}
