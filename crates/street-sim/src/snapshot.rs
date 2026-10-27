//! The whole [`World`] as bytes, for net play: the server sends it to a
//! client whose checksums drift (and to a client joining a match already
//! under way), and the client carries on from it.
//!
//! The encoding is hand-written and compact: integers are LEB128 varints
//! (signed ones zigzagged), floats their exact IEEE bits (4 bytes, little
//! endian), so a world read back steps exactly as the original would. It
//! starts with the tag `SW` and a format version.
//!
//! The structs are taken apart exhaustively where their fields are visible,
//! so adding a field to [`World`], [`Player`] or the other state types fails
//! to compile here until the snapshot covers it.

use crate::ball::Via;
use crate::player::DunkStyle;
use crate::street::Combo;
use crate::{
    Action, Ball, BallState, Basket, Difficulty, Event, Gait, Goal, Hoop, Input, OpeningToss, Pass,
    PassStyle, Phase, Player, Ratings, Rules, Scoring, Shot, TeamState, Tip, World, combo,
    rng::Rng,
};

/// Format version, after the `SW` tag (73: the flight's knock and shove windows, the
/// men its move met and beat, and the jump's airtime and updates aloft; 48: the
/// alley-oop's leap on its
/// row's script (the rewrite's own leap gone) and the dive on its script,
/// the flight's slide (actor+0x184), each turbo button's last release for
/// the dive's tap; 47: each player's dribble
/// (actor+0x1a0), the computer defence's pass-jump timer, a dunk's hang on
/// the rim (callback 28) and the scripts' second join, the ball's last
/// shooter (+0xf8) and the release's last roll (`0x3d6580`); 46: each
/// player's flight on his move's script; 45: the held ball's blend anchor,
/// each player's handedness and the start of the script on his channel,
/// the scripts' handedness and side picks; 40: both 39s together, the dunk's
/// basket on its shot, the ball's hoop touch (ball+0x180 6) and the
/// rebound jump with the swipe's script, clip and test on a steal, each
/// player's model scales and the ball's last move; 39: either alone, on
/// its branch; 38: the court in the original's
/// frame, team 0 attacking +x, so every stored place, heading and hoop
/// means the mirror of what 37 stored; 37: both 36s together, the game's
/// phase machine and the ball's rebound flag with the pass's aim in the
/// windup, the first-pass flag, each player's received flight and the
/// tip-off jumpers' scripts; 36: either of those alone, on its branch;
/// 35: the combo event's walked ring,
/// payout and newest value, for the HUD's combo line; 34: the catch-up tables' stage
/// bounds the points game draws once a game; 33: the shot's release gate, the
/// tick it opens by itself and the updates stalled at it; 32: the
/// scripts' identity on the actions and the catch, for the display; 31:
/// the script clock;
/// 30: the defense's sags, press and
/// bite in the computer players' state; 29: a pass in the passer's hands,
/// [`crate::passing::Windup`]; 28: the meters' hold, scale and
/// gamebreaker flag, game+0x4a4 and the match's statistics; 27: the
/// computer players' pick
/// behaviours and pick fields, the contact counts, the players' weight;
/// 26: the restart's ready flags and
/// shared play in the computer players' state, the tip-off; 25: the
/// computer players' team
/// strategies, plays and new behaviours; 24: the opening toss and the
/// computer players' state; 22: the court's ceiling; 21: the ball's spin and
/// its airborne, through, bank and armed flags; 20: tricks' hand flags per
/// frame and the side a trick plays). 49: the combo manager's one ring with
/// its ticks and spans, the meters' repeat keys and flags, the assist window,
/// the pump fake's watch, the lesson, the statistics' flags; 50: the turbo
/// bars and their owners, the people's controllers, the counter stance;
/// 51: each player's power and overall ratings (4 and 9); 56: both 55s
/// together; 55, on its branch: the people's control between updates (the
/// give 'n go's hold, each side's pad, a receiver's frozen stick and the
/// last update's tables, [`crate::control`]), or the pass's spot run
/// (`DAT_003d70b4`, its spot, the receiver waiting there and the run the
/// pass start gave him), the passout to oneself off the board (its point,
/// legs and tail) and the AI's board point; 57: the pump fake's bite
/// ([`Event::PumpFakeBite`], slot 73); 58: the gamebreaker task
/// ([`crate::street::GamebreakerTask`]) and a staged gamebreaker's pending
/// start ([`World::staged_start`]); 59: a lesson's page at its launch
/// ([`World::lesson_page`], the lesson flag 2); 60: the disc's data out of
/// the snapshot, only the match's own values of it kept (the players' gait
/// profiles, the staged players' scales and width ratios;
/// [`crate::disc`]); 61: a staged script's replay command
/// ([`Event::StagedRerun`]); 62: the story modes' outer weight
/// ([`World::scenario`]); 63: the scores at the meters' fills
/// ([`crate::stats::Stats::fills`]) and the AI's game+0x4c0; 64: a
/// gamebreaker basket's dunk ([`Basket::Gamebreaker`]); 65: the sides'
/// database teams ([`World::side_teams`]); 66: the stored inputs' actors
/// off the screen ([`Input::off_screen`], the view word's top bits); 67:
/// the "loose ball" behaviour's wait, landing and press words
/// (`crate::ai`); 68: the drive's blocked-dunk tap (`crate::ai`'s drive
/// +0x60), the "loose ball" behaviour's free-ball count (+0x30) and its
/// take, a rebound jump's callback 47 path, held clip, airtime and facing
/// word; 69: the actors' update order ([`World::update_order`]); 70:
/// the rebound's scoring event, separate from a generic pickup; 71:
/// the passout's board/head identity through the catch; 72: the head
/// passout's defender through its row's release wait; 74: the
/// celebrations (each player's word, the last basket's scorer, the win's
/// waiting celebration, the hold's count game+0x464); 75: the
/// stands (each player's stand script, post count and box-out, a
/// looping span's clip length, the C library's word, each face's time);
/// 76: each head's look; 77: the box-out's boxed man and partners; 78:
/// posting up (state bit 11) and a script's asked test; 79: the restart's
/// walk; 80: "iso3pt"'s missed juke; 81: a stand's planted feet; 83:
/// the rewrite's own pick gone, a reaction's `a6 2` window and the
/// screen's reactions among the scripts; 84: the count a shot's timer
/// started from (the updates it has flown); 85: a computer screener's
/// three pick scripts among the reactions; 93: the "no turbo" message
/// ([`Event::NoTurbo`]); 94: the take-off and the throw from the hands
/// ([`Event::TakeOff`], [`Event::Launched`]); 95: the take-off's dunk id;
/// 96: the ball's contacts and the rim held ([`Event::BallHit`],
/// [`Event::RimHold`]), the dribble's speed and the slam's shake
/// ([`Event::SlamShake`]), every take ([`Event::Take`]).
const VERSION: u8 = 96;

impl World {
    /// The whole world as bytes ([`World::from_snapshot`] reads it back).
    /// The disc's data is not game state and stays out ([`crate::disc`]):
    /// of the gait and the staged gamebreakers only the match's own values
    /// go in (each player's gait profile, the staged players' scales), and
    /// the world read back holds those until its machine installs its own
    /// disc's tables under them ([`World::install`]). So the checksum the
    /// machines in a match compare covers the same game state.
    pub fn to_snapshot(&self) -> Vec<u8> {
        self.snapshot_bytes()
    }

    pub(crate) fn snapshot_bytes(&self) -> Vec<u8> {
        let mut w = Writer(Vec::with_capacity(1024));
        w.0.extend_from_slice(b"SW");
        w.u8(VERSION);
        let World {
            tick,
            hoops,
            players,
            ball,
            rules,
            score,
            stage_bounds,
            update_order,
            teams,
            possession,
            shot_clock,
            phase,
            game_phase,
            events,
            give_and_go,
            fresh_take,
            dribble_armed,
            rims,
            rim_rigs,
            windup,
            catch,
            turbo_releases,
            rng,
            previous_inputs,
            staged,
            gait,
            variety,
            sequence,
            combo,
            assist,
            pump_fake_watch,
            pump_fake_left,
            pump_fake_shot,
            clock,
            lesson,
            lesson_page,
            scenario,
            side_teams,
            opening_toss,
            tip,
            timed_dead_ball,
            scorer,
            celebrate_pending,
            libc,
            replays,
            contacts,
            ai,
            meter_clock,
            stats,
            // The disc's animations are not game state ([`crate::pose::Bank`]).
            poses: _,
            last_trick,
            catching,
            spot_run,
            passout,
            held_anchor,
            turbo,
            turbo_owner,
            people,
            control,
            gamebreaker_task,
            staged_start,
        } = self;
        w.uv(*tick);
        for h in hoops {
            let Hoop { rim, rim_radius } = h;
            w.f3(rim);
            w.f(*rim_radius);
        }
        w.uv(players.len() as u64);
        for p in players {
            player(&mut w, p);
        }
        ball_out(&mut w, ball);
        rules_out(&mut w, rules);
        w.uv(score[0].into());
        w.uv(score[1].into());
        w.uv(stage_bounds[0].into());
        w.uv(stage_bounds[1].into());
        for k in update_order {
            w.u8(*k);
        }
        for t in teams {
            team(&mut w, t);
        }
        w.us(*possession);
        w.f(*shot_clock);
        match *phase {
            Phase::Live => w.u8(0),
            Phase::DeadBall { left, team } => {
                w.u8(1);
                w.f(left);
                w.us(team);
            }
            Phase::Over { winner } => {
                w.u8(2);
                w.us(winner);
            }
        }
        let crate::game_phase::GamePhase {
            phase,
            possession,
            loose,
            hold,
            since,
            countdown,
        } = *game_phase;
        for word in [phase, possession, loose, hold, since, countdown] {
            w.iv(i64::from(word));
        }
        w.uv(events.len() as u64);
        for e in events {
            event(&mut w, e);
        }
        match give_and_go {
            None => w.u8(0),
            Some((a, b)) => {
                w.u8(1);
                w.us(*a);
                w.us(*b);
            }
        }
        w.bool(*fresh_take);
        w.bool(*dribble_armed);
        for rim in rims {
            rim_out(&mut w, rim);
        }
        for rig in rim_rigs {
            for v in rig.t.iter().flatten().chain(rig.r.iter().flatten()) {
                w.f(*v);
            }
        }
        match windup {
            None => w.u8(0),
            Some(crate::passing::Windup {
                from,
                to,
                style,
                left,
                row,
                aim: crate::passing::PassAim { spot, lead, reach },
            }) => {
                w.u8(1);
                w.us(*from);
                w.us(*to);
                w.u8(pass_style(*style));
                w.u8(*left);
                w.u8(*row);
                w.f2(spot);
                w.u8(u8::from(*lead));
                w.f(*reach);
            }
        }
        match catch {
            None => w.u8(0),
            Some(crate::Catch {
                player,
                until,
                row,
                flight,
                since,
            }) => {
                w.u8(1);
                w.us(*player);
                w.uv(*until);
                w.u8(*row);
                w.u8(*flight);
                w.uv(*since);
            }
        }
        w.uv(turbo_releases.len() as u64);
        for t in turbo_releases {
            for &k in t {
                // u64::MAX (no release yet) becomes 0, so it takes one byte.
                w.uv(k.wrapping_add(1));
            }
        }
        for word in rng.state() {
            w.0.extend_from_slice(&word.to_le_bytes());
        }
        w.uv(previous_inputs.len() as u64);
        for i in previous_inputs {
            w.0.extend_from_slice(&i.to_bytes());
        }
        // The match's own disc values, not the disc's tables.
        match gait {
            None => w.u8(0),
            Some(config) => {
                w.u8(1);
                profiles_out(&mut w, &config.profiles);
            }
        }
        match staged {
            None => w.u8(0),
            Some(config) => {
                w.u8(1);
                w.uv(config.player_scales.len() as u64);
                for &f in &config.player_scales {
                    w.f(f);
                }
                w.uv(config.player_width_ratios.len() as u64);
                for &f in &config.player_width_ratios {
                    w.f(f);
                }
            }
        }
        w.0.extend_from_slice(&variety.encode());
        match sequence {
            None => w.u8(0),
            Some(s) => {
                w.u8(1);
                let bytes = s.encode();
                w.uv(bytes.len() as u64);
                w.0.extend_from_slice(&bytes);
            }
        }
        combo_out(&mut w, combo);
        match assist {
            None => w.u8(0),
            Some((p, left)) => {
                w.u8(1);
                w.us(*p);
                w.uv(u64::from(*left));
            }
        }
        w.uv(pump_fake_watch.len() as u64);
        for &f in pump_fake_watch {
            w.bool(f);
        }
        w.iv(i64::from(*pump_fake_left));
        w.iv(i64::from(*pump_fake_shot));
        let crate::shot_clock::ShotClock {
            side,
            stopped,
            loose,
            pass,
            shot,
        } = clock;
        w.iv(i64::from(*side));
        for f in [stopped, loose, pass, shot] {
            w.bool(*f);
        }
        match lesson {
            None => w.u8(0),
            Some(l) => {
                w.u8(if *lesson_page { 2 } else { 1 });
                w.u8(*l);
            }
        }
        match scenario {
            None => w.u8(0),
            Some(s) => {
                w.u8(1);
                w.f(*s);
            }
        }
        // As i16s: none (`NO_TEAM`) is -1, a byte.
        for t in side_teams {
            w.iv(i64::from(*t as i16));
        }
        match *opening_toss {
            OpeningToss::None => w.u8(0),
            OpeningToss::Waiting(left) => {
                w.u8(1);
                w.uv(left.into());
            }
            OpeningToss::Flight => w.u8(2),
        }
        let Tip {
            timing,
            winner,
            jumped,
            first_pass,
        } = *tip;
        for t in timing {
            w.uv(u64::from(t as u32));
        }
        w.uv(u64::from(winner as u32));
        for j in jumped {
            w.u8(u8::from(j));
        }
        w.u8(u8::from(first_pass));
        w.u8(u8::from(*timed_dead_ball));
        w.iv(scorer.map_or(-1, |s| s as i64));
        w.bool(*celebrate_pending);
        w.uv(u64::from(*libc));
        w.u8(*replays);
        for k in 0..6 {
            w.iv(i64::from(contacts.with[k]));
            w.iv(i64::from(contacts.count[k]));
        }
        let bytes = ai.encode();
        w.uv(bytes.len() as u64);
        w.0.extend_from_slice(&bytes);
        w.u8(meter_clock_byte(*meter_clock));
        stats_out(&mut w, stats);
        w.u8(*last_trick);
        match catching {
            None => w.u8(0),
            Some(c) => {
                w.u8(1);
                w.us(c.player);
                w.u8(c.row);
                run_out(&mut w, &c.run);
                clip_out(&mut w, c.from);
                w.bool(c.catching);
                w.f2(&c.drift);
                w.iv(i64::from(c.facing.unwrap_or(-1)));
                w.bool(c.parked);
                w.bool(c.floor);
                match c.lower {
                    None => w.u8(0),
                    Some((record, time, rate)) => {
                        w.u8(1);
                        w.uv(u64::from(record));
                        w.f(time);
                        w.f(rate);
                    }
                }
            }
        }
        w.u8(spot_run.count);
        w.f2(&spot_run.spot);
        w.iv(spot_run.waiting.map_or(-1, |p| p as i64));
        match spot_run.steer {
            None => w.u8(0),
            Some((p, word, intensity)) => {
                w.u8(1);
                w.us(p);
                w.iv(i64::from(word));
                w.iv(i64::from(intensity));
            }
        }
        match passout {
            None => w.u8(0),
            Some(po) => {
                w.u8(1);
                w.us(po.passer);
                w.u8(po.kind as u8);
                match po.defender {
                    None => w.u8(0),
                    Some(d) => {
                        w.u8(1);
                        w.us(d);
                    }
                }
                w.f3(&po.point);
                match po.back {
                    None => w.u8(0),
                    Some((hand, total)) => {
                        w.u8(1);
                        w.f3(&hand);
                        w.iv(i64::from(total));
                    }
                }
                w.bool(po.tossed);
                w.iv(po.tail.map_or(-1, i64::from));
            }
        }
        match held_anchor {
            None => w.u8(0),
            Some(a) => {
                w.u8(1);
                w.us(a.player);
                w.f3(&a.local);
            }
        }
        for t in turbo {
            w.f(*t);
        }
        for (owner, person) in turbo_owner.iter().zip(people) {
            w.bool(*owner);
            w.bool(*person);
        }
        let crate::control::Control {
            hold,
            pad_before,
            frozen,
            tables,
        } = control;
        for h in hold {
            w.bool(*h);
        }
        for pad in pad_before {
            w.0.extend_from_slice(&pad.to_bytes());
        }
        match frozen {
            None => w.u8(0),
            Some(f) => {
                w.u8(1);
                w.us(f.player);
                w.0.extend_from_slice(&[f.stick[0] as u8, f.stick[1] as u8]);
                w.0.extend_from_slice(&[f.aim[0] as u8, f.aim[1] as u8]);
            }
        }
        match tables {
            None => w.u8(0),
            Some(t) => {
                w.u8(1);
                w.uv(t.places.len() as u64);
                for place in &t.places {
                    w.f2(place);
                }
                w.f3(&t.ball);
            }
        }
        match gamebreaker_task {
            None => w.u8(0),
            Some(t) => {
                w.u8(1);
                w.u8(t.kind);
                w.u8(t.state);
                w.iv(i64::from(t.count));
            }
        }
        match staged_start {
            None => w.u8(0),
            Some((s, wait)) => {
                w.u8(1);
                let bytes = s.encode();
                w.uv(bytes.len() as u64);
                w.0.extend_from_slice(&bytes);
                w.u8(*wait);
            }
        }
        w.0
    }

    /// A world from [`World::to_snapshot`]'s bytes.
    pub fn from_snapshot(bytes: &[u8]) -> Result<World, String> {
        let mut r = Reader { b: bytes, at: 0 };
        if r.take(2)? != b"SW" {
            return Err("not a world snapshot".into());
        }
        let version = r.u8()?;
        if version != VERSION {
            return Err(format!(
                "world snapshot version {version}, expected {VERSION}"
            ));
        }
        let tick = r.uv()?;
        let mut hoops = [Hoop {
            rim: [0.0; 3],
            rim_radius: 0.0,
        }; 2];
        for h in &mut hoops {
            h.rim = r.f3()?;
            h.rim_radius = r.f()?;
        }
        let n = r.len(64)?;
        let players = (0..n)
            .map(|_| player_in(&mut r))
            .collect::<Result<Vec<_>, _>>()?;
        let ball = ball_in(&mut r)?;
        let rules = rules_in(&mut r)?;
        let score = [r.u32()?, r.u32()?];
        let stage_bounds = [r.u32()?, r.u32()?];
        let mut update_order = [0u8; 6];
        for k in &mut update_order {
            *k = r.u8()?;
        }
        let teams = [team_in(&mut r)?, team_in(&mut r)?];
        let possession = r.us()?;
        let shot_clock = r.f()?;
        let phase = match r.u8()? {
            0 => Phase::Live,
            1 => Phase::DeadBall {
                left: r.f()?,
                team: r.us()?,
            },
            2 => Phase::Over { winner: r.us()? },
            k => return Err(format!("bad phase {k}")),
        };
        let game_phase = crate::game_phase::GamePhase {
            phase: r.i32()?,
            possession: r.i32()?,
            loose: r.i32()?,
            hold: r.i32()?,
            since: r.i32()?,
            countdown: r.i32()?,
        };
        let n = r.len(256)?;
        let events = (0..n)
            .map(|_| event_in(&mut r))
            .collect::<Result<Vec<_>, _>>()?;
        let give_and_go = match r.u8()? {
            0 => None,
            _ => Some((r.us()?, r.us()?)),
        };
        let fresh_take = r.bool()?;
        let dribble_armed = r.bool()?;
        let rims = [rim_in(&mut r)?, rim_in(&mut r)?];
        let mut rim_rigs = [crate::rim::Rig::default(); 2];
        for rig in &mut rim_rigs {
            for v in rig.t.iter_mut().flatten() {
                *v = r.f()?;
            }
            for v in rig.r.iter_mut().flatten() {
                *v = r.f()?;
            }
        }
        let windup = match r.u8()? {
            0 => None,
            1 => Some(crate::passing::Windup {
                from: r.us()?,
                to: r.us()?,
                style: pass_style_in(r.u8()?)?,
                left: r.u8()?,
                row: r.u8()?,
                aim: crate::passing::PassAim {
                    spot: r.f2()?,
                    lead: match r.u8()? {
                        0 => false,
                        1 => true,
                        k => return Err(format!("bad pass lead flag {k}")),
                    },
                    reach: r.f()?,
                },
            }),
            k => return Err(format!("bad pass windup flag {k}")),
        };
        let catch = match r.u8()? {
            0 => None,
            1 => Some(crate::Catch {
                player: r.us()?,
                until: r.uv()?,
                row: r.u8()?,
                flight: r.u8()?,
                since: r.uv()?,
            }),
            k => return Err(format!("bad catch flag {k}")),
        };
        let n = r.len(64)?;
        let turbo_releases = (0..n)
            .map(|_| -> Result<[u64; 4], String> {
                let mut t = [0u64; 4];
                for k in &mut t {
                    *k = r.uv()?.wrapping_sub(1);
                }
                Ok(t)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut state = [0_u32; 6];
        for word in &mut state {
            *word = u32::from_le_bytes(r.take(4)?.try_into().expect("4 bytes"));
        }
        let rng = Rng::from_state(state);
        let n = r.len(64)?;
        let previous_inputs = (0..n)
            .map(|_| {
                let b: [u8; Input::BYTES] = r.take(Input::BYTES)?.try_into().expect("8 bytes");
                Ok(Input::from_bytes(b))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let gait = match r.u8()? {
            0 => None,
            1 => Some(std::sync::Arc::new(crate::gait_select::GaitConfig {
                profiles: profiles_in(&mut r)?,
                ..Default::default()
            })),
            k => return Err(format!("bad gait profiles flag {k}")),
        };
        let staged = match r.u8()? {
            0 => None,
            1 => {
                let floats = |r: &mut Reader| -> Result<Vec<f32>, String> {
                    let n = r.len(6)?;
                    (0..n).map(|_| r.f()).collect()
                };
                let config = crate::gamebreaker::Config {
                    player_scales: floats(&mut r)?,
                    player_width_ratios: floats(&mut r)?,
                    ..Default::default()
                };
                config.validate()?;
                Some(std::sync::Arc::new(config))
            }
            k => return Err(format!("bad staged players flag {k}")),
        };
        let variety = crate::gamebreaker::Variety::decode(r.take(17)?)?;
        let sequence = match r.u8()? {
            0 => None,
            1 => {
                let n = r.len(1024)?;
                Some(crate::gamebreaker::Sequence::decode(r.take(n)?)?)
            }
            k => return Err(format!("bad staged sequence flag {k}")),
        };
        let combo = combo_in(&mut r)?;
        let assist = match r.u8()? {
            0 => None,
            1 => Some((
                r.us()?,
                u16::try_from(r.uv()?).map_err(|_| "assist window out of range")?,
            )),
            k => return Err(format!("bad assist flag {k}")),
        };
        let n = r.len(64)?;
        let pump_fake_watch = (0..n).map(|_| r.bool()).collect::<Result<Vec<_>, _>>()?;
        let pump_fake_left = r.i32()?;
        let pump_fake_shot = r.i32()?;
        let clock = crate::shot_clock::ShotClock {
            side: r.i32()?,
            stopped: r.bool()?,
            loose: r.bool()?,
            pass: r.bool()?,
            shot: r.bool()?,
        };
        let (lesson, lesson_page) = match r.u8()? {
            0 => (None, false),
            k @ (1 | 2) => (Some(r.u8()?), k == 2),
            k => return Err(format!("bad lesson flag {k}")),
        };
        let scenario = match r.u8()? {
            0 => None,
            1 => Some(r.f()?),
            k => return Err(format!("bad scenario flag {k}")),
        };
        let side_teams = [r.iv()? as i16 as u16, r.iv()? as i16 as u16];
        let opening_toss = match r.u8()? {
            0 => OpeningToss::None,
            1 => OpeningToss::Waiting(
                u16::try_from(r.uv()?).map_err(|_| "opening toss wait out of range")?,
            ),
            2 => OpeningToss::Flight,
            k => return Err(format!("bad opening toss {k}")),
        };
        let word = |r: &mut Reader| -> Result<i32, String> {
            Ok(u32::try_from(r.uv()?).map_err(|_| "tip word out of range")? as i32)
        };
        let timing = [word(&mut r)?, word(&mut r)?];
        let winner = word(&mut r)?;
        let mut jumped = [false; 2];
        for j in &mut jumped {
            *j = match r.u8()? {
                0 => false,
                1 => true,
                k => return Err(format!("bad tip flag {k}")),
            };
        }
        let first_pass = match r.u8()? {
            0 => false,
            1 => true,
            k => return Err(format!("bad first pass flag {k}")),
        };
        let tip = Tip {
            timing,
            winner,
            jumped,
            first_pass,
        };
        let timed_dead_ball = match r.u8()? {
            0 => false,
            1 => true,
            k => return Err(format!("bad dead ball flag {k}")),
        };
        let scorer = match r.i32()? {
            -1 => None,
            s => Some(usize::try_from(s).map_err(|_| "invalid scorer")?),
        };
        let celebrate_pending = r.bool()?;
        let libc = u32::try_from(r.uv()?).map_err(|_| "invalid libc word")?;
        let replays = r.u8()?;
        let mut contacts = crate::contact::Contacts::default();
        for k in 0..6 {
            contacts.with[k] = r.i32()?;
            contacts.count[k] = r.i32()?;
        }
        let n = r.len(1 << 20)?;
        let ai = crate::ai::State::decode(r.take(n)?)?;
        let meter_clock = match r.u8()? {
            0 => crate::street::MeterClock::Off,
            1 => crate::street::MeterClock::Inbound,
            2 => crate::street::MeterClock::On,
            k => return Err(format!("bad meter clock {k}")),
        };
        let stats = stats_in(&mut r)?;
        let last_trick = r.u8()?;
        let catching = match r.u8()? {
            0 => None,
            1 => Some(crate::touch::Catching {
                player: r.us()?,
                row: r.u8()?,
                run: run_in(&mut r)?,
                from: clip_in(&mut r)?,
                catching: r.bool()?,
                drift: r.f2()?,
                facing: match r.i32()? {
                    -1 => None,
                    v => Some(v),
                },
                parked: r.bool()?,
                floor: r.bool()?,
                lower: match r.u8()? {
                    0 => None,
                    1 => Some((
                        u16::try_from(r.uv()?).map_err(|_| "invalid clip")?,
                        r.f()?,
                        r.f()?,
                    )),
                    k => return Err(format!("bad lower clip flag {k}")),
                },
            }),
            k => return Err(format!("bad catching flag {k}")),
        };
        let spot_run = crate::receiver::SpotRun {
            count: r.u8()?,
            spot: r.f2()?,
            waiting: match r.i32()? {
                -1 => None,
                p => Some(usize::try_from(p).map_err(|_| "bad spot run receiver")?),
            },
            steer: match r.u8()? {
                0 => None,
                1 => Some((r.us()?, r.i32()?, r.i32()?)),
                k => return Err(format!("bad spot run steer flag {k}")),
            },
        };
        let passout = match r.u8()? {
            0 => None,
            1 => Some(crate::passout::Passout {
                passer: r.us()?,
                kind: match r.u8()? {
                    1 => crate::passout::Kind::Board,
                    2 => crate::passout::Kind::Head,
                    k => return Err(format!("bad passout kind {k}")),
                },
                defender: match r.u8()? {
                    0 => None,
                    1 => Some(r.us()?),
                    k => return Err(format!("bad passout defender flag {k}")),
                },
                point: r.f3()?,
                back: match r.u8()? {
                    0 => None,
                    1 => Some((r.f3()?, r.i32()?)),
                    k => return Err(format!("bad passout leg flag {k}")),
                },
                tossed: r.bool()?,
                tail: match r.i32()? {
                    -1 => None,
                    t => Some(u8::try_from(t).map_err(|_| "bad passout tail")?),
                },
            }),
            k => return Err(format!("bad passout flag {k}")),
        };
        let held_anchor = match r.u8()? {
            0 => None,
            1 => Some(crate::held::Anchor {
                player: r.us()?,
                local: r.f3()?,
            }),
            k => return Err(format!("bad held anchor flag {k}")),
        };
        let mut turbo = [0.0; crate::turbo::SLOTS];
        for t in &mut turbo {
            *t = r.f()?;
        }
        let mut turbo_owner = Vec::with_capacity(players.len());
        let mut people = Vec::with_capacity(players.len());
        for _ in 0..players.len() {
            turbo_owner.push(r.bool()?);
            people.push(r.bool()?);
        }
        let mut hold = [false; 2];
        for h in &mut hold {
            *h = r.bool()?;
        }
        let mut pad_before = [Input::default(); 2];
        for pad in &mut pad_before {
            let b: [u8; Input::BYTES] = r.take(Input::BYTES)?.try_into().expect("8 bytes");
            *pad = Input::from_bytes(b);
        }
        let frozen = match r.u8()? {
            0 => None,
            1 => {
                let player = r.us()?;
                let b = r.take(4)?;
                Some(crate::control::Frozen {
                    player,
                    stick: [b[0] as i8, b[1] as i8],
                    aim: [b[2] as i8, b[3] as i8],
                })
            }
            k => return Err(format!("bad frozen stick flag {k}")),
        };
        let tables = match r.u8()? {
            0 => None,
            1 => {
                let n = r.len(64)?;
                let places = (0..n).map(|_| r.f2()).collect::<Result<Vec<_>, String>>()?;
                Some(crate::control::Tables {
                    places,
                    ball: r.f3()?,
                })
            }
            k => return Err(format!("bad tables flag {k}")),
        };
        let control = crate::control::Control {
            hold,
            pad_before,
            frozen,
            tables,
        };
        let gamebreaker_task = match r.u8()? {
            0 => None,
            1 => Some(crate::street::GamebreakerTask {
                kind: r.u8()?,
                state: r.u8()?,
                count: i32::try_from(r.iv()?).map_err(|_| "bad gamebreaker task count")?,
            }),
            k => return Err(format!("bad gamebreaker task flag {k}")),
        };
        let staged_start = match r.u8()? {
            0 => None,
            1 => {
                let n = r.len(1024)?;
                let s = crate::gamebreaker::Sequence::decode(r.take(n)?)?;
                Some((s, r.u8()?))
            }
            k => return Err(format!("bad staged start flag {k}")),
        };
        if r.at != bytes.len() {
            return Err(format!("{} bytes left over", bytes.len() - r.at));
        }
        if turbo_releases.len() != players.len() || previous_inputs.len() != players.len() {
            return Err("per-player lists differ in length".into());
        }
        let world = World {
            tick,
            hoops,
            players,
            ball,
            rules,
            score,
            stage_bounds,
            update_order,
            teams,
            possession,
            shot_clock,
            phase,
            game_phase,
            events,
            give_and_go,
            fresh_take,
            dribble_armed,
            rims,
            rim_rigs,
            windup,
            catch,
            turbo_releases,
            rng,
            previous_inputs,
            staged,
            gait,
            variety,
            sequence,
            combo,
            assist,
            pump_fake_watch,
            pump_fake_left,
            pump_fake_shot,
            clock,
            lesson,
            lesson_page,
            scenario,
            side_teams,
            opening_toss,
            tip,
            timed_dead_ball,
            scorer,
            celebrate_pending,
            libc,
            replays,
            contacts,
            ai,
            meter_clock,
            stats,
            poses: crate::pose::Bank::default(),
            last_trick,
            catching,
            spot_run,
            passout,
            held_anchor,
            turbo,
            turbo_owner,
            people,
            control,
            gamebreaker_task,
            staged_start,
        };
        world.validate()?;
        Ok(world)
    }
}

fn player(w: &mut Writer, p: &Player) {
    let Player {
        team,
        ratings,
        position,
        heading,
        speed,
        course,
        wanted,
        guard,
        gait,
        action,
        action_buttons,
        native_tricks,
        native_dunks,
        height,
        turbo,
        gait_channel,
        pass_kind,
        received_flight,
        weight,
        body,
        rise,
        barred,
        fumble,
        fumble_timer,
        cooldown,
        celebrate,
        stand,
        stand_count,
        boxing,
        posting,
        restart_walk,
        boxed,
        partner,
        boxer,
        face_wait,
        look,
        left_handed,
        dribble,
        channel_start,
        flight,
        on_rim,
        aim_point,
    } = p;
    w.us(*team);
    let Ratings {
        shots,
        threes,
        dunks,
        rebounds,
        blocks,
        steals,
        handles,
        speed: rating_speed,
        power,
        overall,
        reach,
    } = ratings;
    for v in [
        shots,
        threes,
        dunks,
        rebounds,
        blocks,
        steals,
        handles,
        rating_speed,
        power,
        overall,
        reach,
    ] {
        w.f(*v);
    }
    w.f2(position);
    w.f(*heading);
    w.f(*speed);
    w.u8(match gait {
        Gait::Stand => 0,
        Gait::Walk => 1,
        Gait::Run => 2,
        Gait::Turbo => 3,
    });
    action_out(w, action);
    w.uv(u64::from(*action_buttons));
    match native_tricks {
        None => w.u8(0),
        Some(tricks) => {
            w.u8(1);
            for identity in tricks.slots {
                trick_identity_out(w, identity);
            }
        }
    }
    match native_dunks {
        None => w.u8(0),
        Some(mask) => {
            w.u8(1);
            w.uv(u64::from(*mask));
        }
    }
    w.f(*height);
    w.f(*turbo);
    gait_channel_out(w, gait_channel);
    w.uv(u64::from(*pass_kind));
    w.iv(i64::from(*received_flight));
    w.uv(u64::from(*weight));
    match body {
        None => w.u8(0),
        Some(b) => {
            w.u8(1);
            w.f(b.height);
            w.f(b.width);
            w.f(b.head);
        }
    }
    w.f(*rise);
    w.uv(u64::from(*barred));
    w.bool(*fumble);
    w.uv(u64::from(*fumble_timer));
    w.uv(u64::from(*cooldown));
    w.bool(*celebrate);
    match stand {
        None => w.u8(0),
        Some(s) => {
            w.u8(1);
            w.u8(s.script);
            run_out(w, &s.run);
            clip_out(w, s.from);
            match s.plant {
                None => w.u8(0),
                Some((bones, at)) => {
                    w.u8(1);
                    w.u8(bones[0]);
                    w.u8(bones[1]);
                    w.f(at[0]);
                    w.f(at[1]);
                }
            }
        }
    }
    w.uv(u64::from(*stand_count));
    w.bool(*boxing);
    w.bool(*posting);
    w.bool(*restart_walk);
    w.bool(*boxed);
    w.u8(partner.unwrap_or(u8::MAX));
    w.u8(boxer.unwrap_or(u8::MAX));
    w.iv(i64::from(*face_wait));
    w.u8(look.situation);
    w.u8(look.target);
    w.iv(i64::from(look.timer));
    w.iv(i64::from(look.last_state));
    w.u8(look.last_holder.unwrap_or(u8::MAX));
    w.uv(u64::from(look.last_bits));
    look_aim_out(w, look.aim);
    w.uv(u64::from(look.picks));
    w.bool(*left_handed);
    w.u8(*dribble as u8);
    match channel_start {
        None => w.u8(0),
        Some(c) => {
            w.u8(1);
            w.uv(u64::from(c.script));
            clip_out(w, c.from);
            w.bool(c.left);
            w.bool(c.side);
        }
    }
    w.f(*course);
    w.f(*wanted);
    for v in [guard.entered, guard.left, guard.jog_back, guard.other] {
        w.iv(v);
    }
    for v in guard.history {
        w.iv(i64::from(v));
    }
    flight_out(w, flight);
    w.bool(*on_rim);
    for v in aim_point {
        w.f(*v);
    }
}

/// A look's aim ([`crate::look::Aim`]).
fn look_aim_out(w: &mut Writer, aim: crate::look::Aim) {
    use crate::look::Aim;
    match aim {
        Aim::Clear => w.u8(0),
        Aim::Actor(j) => {
            w.u8(1);
            w.u8(j);
        }
        Aim::Ball { holder } => {
            w.u8(2);
            w.bool(holder);
        }
        Aim::Point(p) => {
            w.u8(3);
            for v in p {
                w.f(v);
            }
        }
        Aim::Guard => w.u8(4),
        Aim::Camera => w.u8(5),
    }
}

fn look_aim_in(r: &mut Reader) -> Result<crate::look::Aim, String> {
    use crate::look::Aim;
    Ok(match r.u8()? {
        0 => Aim::Clear,
        1 => Aim::Actor(r.u8()?),
        2 => Aim::Ball { holder: r.bool()? },
        3 => Aim::Point([r.f()?, r.f()?, r.f()?]),
        4 => Aim::Guard,
        5 => Aim::Camera,
        k => return Err(format!("bad look aim {k}")),
    })
}

/// A hoop's give ([`crate::rim::Rim`]).
fn rim_out(w: &mut Writer, rim: &crate::rim::Rim) {
    for b in [
        rim.bend,
        rim.sway,
        rim.held,
        rim.torn,
        rim.swaying,
        rim.dangles,
        rim.bending,
        rim.returning,
        rim.dropped,
    ] {
        w.bool(b);
    }
    for v in [
        rim.bend_phase,
        rim.bend_angle,
        rim.sway_phase,
        rim.sway_angle,
        rim.dangle_step,
        rim.sway_step,
        rim.amplitude,
        rim.dangle_amplitude,
        rim.kick,
        rim.decay,
        rim.dangle,
    ] {
        w.f(v);
    }
}

fn rim_in(r: &mut Reader) -> Result<crate::rim::Rim, String> {
    let mut b = [false; 9];
    for v in &mut b {
        *v = r.bool()?;
    }
    let mut f = [0.0f32; 11];
    for v in &mut f {
        *v = r.f()?;
    }
    Ok(crate::rim::Rim {
        bend: b[0],
        sway: b[1],
        held: b[2],
        torn: b[3],
        swaying: b[4],
        dangles: b[5],
        bending: b[6],
        returning: b[7],
        dropped: b[8],
        bend_phase: f[0],
        bend_angle: f[1],
        sway_phase: f[2],
        sway_angle: f[3],
        dangle_step: f[4],
        sway_step: f[5],
        amplitude: f[6],
        dangle_amplitude: f[7],
        kick: f[8],
        decay: f[9],
        dangle: f[10],
    })
}

/// A move's flight ([`crate::jump::Flight`]).
fn flight_out(w: &mut Writer, f: &crate::jump::Flight) {
    for v in f.velocity {
        w.f(v);
    }
    w.uv(u64::from(f.flags));
    w.iv(i64::from(f.landing));
    for v in f.moved {
        w.f(v);
    }
    w.uv(u64::from(f.script));
    w.iv(i64::from(f.updates));
    w.u8(f.bones[0]);
    w.u8(f.bones[1]);
    for v in f.point {
        w.f(v);
    }
    w.u8(f.meet);
    w.f(f.slide);
    w.u8(f.knock);
    w.u8(f.shove);
    w.u8(f.met);
    w.u8(f.beat);
    w.uv(u64::from(f.air));
    w.uv(u64::from(f.aloft));
    w.bool(f.hung);
}

fn flight_in(r: &mut Reader) -> Result<crate::jump::Flight, String> {
    let three = |r: &mut Reader| -> Result<[f32; 3], String> { Ok([r.f()?, r.f()?, r.f()?]) };
    Ok(crate::jump::Flight {
        velocity: three(r)?,
        flags: u32::try_from(r.uv()?).map_err(|_| "invalid flight flags")?,
        landing: i16::try_from(r.i32()?).map_err(|_| "invalid landing")?,
        moved: three(r)?,
        script: u32::try_from(r.uv()?).map_err(|_| "invalid flight script")?,
        updates: r.i32()?,
        bones: [r.u8()?, r.u8()?],
        point: three(r)?,
        meet: r.u8()?,
        slide: r.f()?,
        knock: r.u8()?,
        shove: r.u8()?,
        met: r.u8()?,
        beat: r.u8()?,
        air: u16::try_from(r.uv()?).map_err(|_| "invalid flight airtime")?,
        aloft: u16::try_from(r.uv()?).map_err(|_| "invalid flight updates aloft")?,
        hung: r.bool()?,
    })
}

fn gait_channel_out(w: &mut Writer, c: &crate::ordinary::PlayerGaitChannel) {
    w.uv(u64::from(c.row));
    w.f(c.phase);
    w.iv(i64::from(c.integer));
    w.f(c.frame_step);
    w.iv(i64::from(c.blend_counter));
    w.u8(u8::from(c.held));
    w.u8(u8::from(c.moving));
    w.iv(i64::from(c.heading));
    w.uv(u64::from(c.family));
    match c.record {
        None => w.u8(0),
        Some(record) => {
            w.u8(1);
            w.uv(u64::from(record));
        }
    }
    for row in c.rows {
        w.uv(u64::from(row));
    }
    w.f(c.root_distance);
    w.f(c.motion[0]);
    w.f(c.motion[1]);
    w.u8(c.hand);
    w.u8(c.trick_side);
    match c.blend_from {
        None => w.u8(0),
        Some((row, time)) => {
            w.u8(1);
            w.uv(u64::from(row));
            w.f(time);
        }
    }
}

fn profiles_out(w: &mut Writer, profiles: &[crate::gait_select::GaitProfile]) {
    w.uv(profiles.len() as u64);
    for p in profiles {
        w.uv(u64::from(p.category));
        w.iv(i64::from(p.identity));
        w.iv(i64::from(p.handles));
    }
}

fn profiles_in(r: &mut Reader) -> Result<Vec<crate::gait_select::GaitProfile>, String> {
    let word = |r: &mut Reader| u32::try_from(r.uv()?).map_err(|_| "invalid gait word".to_string());
    let n = r.len(64)?;
    let mut profiles = Vec::with_capacity(n);
    for _ in 0..n {
        profiles.push(crate::gait_select::GaitProfile {
            category: word(r)?,
            identity: r.i32()?,
            handles: r.i32()?,
        });
    }
    Ok(profiles)
}

impl crate::gait_select::GaitConfig {
    /// The whole config as bytes in a fixed order, for the digest the
    /// machines in a match compare (`street-net`'s `disc`).
    pub fn canonical(&self) -> Vec<u8> {
        let mut w = Writer(Vec::new());
        gait_config_out(&mut w, self);
        w.0
    }
}

fn gait_config_out(w: &mut Writer, c: &crate::gait_select::GaitConfig) {
    w.uv(c.records.len() as u64);
    for r in &c.records {
        w.uv(u64::from(r.rows[0]));
        w.uv(u64::from(r.rows[1]));
        w.uv(u64::from(r.category_mask));
        w.iv(i64::from(r.player));
        w.iv(i64::from(r.minimum_handles));
        w.f(r.root_distance);
        w.f(r.motion[0]);
        w.f(r.motion[1]);
        w.uv(u64::from(r.flags));
    }
    profiles_out(w, &c.profiles);
    w.uv(c.row_hands.len() as u64);
    for (row, frames) in &c.row_hands {
        w.uv(u64::from(*row));
        w.uv(frames.len() as u64);
        w.0.extend_from_slice(frames);
    }
    w.uv(c.stand_hands.len() as u64);
    w.0.extend_from_slice(&c.stand_hands);
    w.uv(c.trick_hands.len() as u64);
    for (script, sides) in &c.trick_hands {
        w.uv(u64::from(*script));
        for frames in sides {
            w.uv(frames.len() as u64);
            w.0.extend_from_slice(frames);
        }
    }
}

fn gait_channel_in(r: &mut Reader) -> Result<crate::ordinary::PlayerGaitChannel, String> {
    let row = u16::try_from(r.uv()?).map_err(|_| "invalid gait row")?;
    let phase = r.f()?;
    let integer = r.i32()?;
    let frame_step = r.f()?;
    let blend_counter = r.i32()?;
    let held = r.bool()?;
    let moving = r.bool()?;
    let heading = r.i32()?;
    let family = u32::try_from(r.uv()?).map_err(|_| "invalid gait family")?;
    let record = match r.u8()? {
        0 => None,
        1 => Some(u16::try_from(r.uv()?).map_err(|_| "invalid gait record")?),
        k => return Err(format!("bad gait record flag {k}")),
    };
    let rows = [
        u16::try_from(r.uv()?).map_err(|_| "invalid gait row")?,
        u16::try_from(r.uv()?).map_err(|_| "invalid gait row")?,
    ];
    let root_distance = r.f()?;
    let motion = [r.f()?, r.f()?];
    let hand = r.u8()?;
    let trick_side = r.u8()?;
    let blend_from = match r.u8()? {
        0 => None,
        1 => Some((
            u16::try_from(r.uv()?).map_err(|_| "invalid gait row")?,
            r.f()?,
        )),
        k => return Err(format!("bad gait blend flag {k}")),
    };
    Ok(crate::ordinary::PlayerGaitChannel {
        row,
        phase,
        integer,
        frame_step,
        blend_counter,
        held,
        moving,
        heading,
        family,
        record,
        rows,
        root_distance,
        motion,
        hand,
        trick_side,
        blend_from,
    })
}

fn combo_out(w: &mut Writer, m: &combo::Manager) {
    let combo::Manager {
        ring,
        side,
        row,
        since,
        total,
        air,
        basket,
        pump_fake,
        selected,
        pushed,
        ready,
    } = m;
    for slot in &ring.slots {
        let combo::Slot {
            class,
            extra,
            tick,
            value,
            span,
        } = slot;
        for v in [class, extra, tick, value, span] {
            w.iv(i64::from(*v));
        }
    }
    w.us(*side);
    for v in [row, since, total, air] {
        w.iv(i64::from(*v));
    }
    for f in [basket, pump_fake, selected, pushed, ready] {
        w.bool(*f);
    }
}

fn combo_in(r: &mut Reader) -> Result<combo::Manager, String> {
    let mut ring = combo::Chain::default();
    for slot in &mut ring.slots {
        *slot = combo::Slot {
            class: r.i32()?,
            extra: r.i32()?,
            tick: r.i32()?,
            value: r.i32()?,
            span: r.i32()?,
        };
    }
    Ok(combo::Manager {
        ring,
        side: r.us()?,
        row: r.i32()?,
        since: r.i32()?,
        total: r.i32()?,
        air: r.i32()?,
        basket: r.bool()?,
        pump_fake: r.bool()?,
        selected: r.bool()?,
        pushed: r.bool()?,
        ready: r.bool()?,
    })
}

fn player_in(r: &mut Reader) -> Result<Player, String> {
    let team = r.us()?;
    let mut v = [0.0; 11];
    for x in &mut v {
        *x = r.f()?;
    }
    let ratings = Ratings {
        shots: v[0],
        threes: v[1],
        dunks: v[2],
        rebounds: v[3],
        blocks: v[4],
        steals: v[5],
        handles: v[6],
        speed: v[7],
        power: v[8],
        overall: v[9],
        reach: v[10],
    };
    Ok(Player {
        team,
        ratings,
        position: r.f2()?,
        heading: r.f()?,
        speed: r.f()?,
        gait: match r.u8()? {
            0 => Gait::Stand,
            1 => Gait::Walk,
            2 => Gait::Run,
            3 => Gait::Turbo,
            k => return Err(format!("bad gait {k}")),
        },
        action: action_in(r)?,
        action_buttons: u16::try_from(r.uv()?).map_err(|_| "invalid action buttons")?,
        native_tricks: native_tricks_in(r)?,
        native_dunks: match r.u8()? {
            0 => None,
            1 => Some(u16::try_from(r.uv()?).map_err(|_| "invalid dunk slots")?),
            k => return Err(format!("bad dunk slots flag {k}")),
        },
        height: r.f()?,
        turbo: r.f()?,
        gait_channel: gait_channel_in(r)?,
        pass_kind: u32::try_from(r.uv()?).map_err(|_| "invalid pass_kind")?,
        received_flight: r.i32()?,
        weight: u16::try_from(r.uv()?).map_err(|_| "invalid weight")?,
        body: match r.u8()? {
            0 => None,
            1 => Some(crate::pose::Body {
                height: r.f()?,
                width: r.f()?,
                head: r.f()?,
            }),
            k => return Err(format!("bad body flag {k}")),
        },
        rise: r.f()?,
        barred: u16::try_from(r.uv()?).map_err(|_| "invalid bar")?,
        fumble: r.bool()?,
        fumble_timer: u16::try_from(r.uv()?).map_err(|_| "invalid fumble")?,
        cooldown: u16::try_from(r.uv()?).map_err(|_| "invalid cooldown")?,
        celebrate: r.bool()?,
        stand: match r.u8()? {
            0 => None,
            1 => Some(crate::stand::Standing {
                script: r.u8()?,
                run: run_in(r)?,
                from: clip_in(r)?,
                plant: match r.u8()? {
                    0 => None,
                    1 => Some(([r.u8()?, r.u8()?], [r.f()?, r.f()?])),
                    k => return Err(format!("bad plant flag {k}")),
                },
            }),
            k => return Err(format!("bad stand flag {k}")),
        },
        stand_count: u16::try_from(r.uv()?).map_err(|_| "invalid stand count")?,
        boxing: r.bool()?,
        posting: r.bool()?,
        restart_walk: r.bool()?,
        boxed: r.bool()?,
        partner: match r.u8()? {
            u8::MAX => None,
            k => Some(k),
        },
        boxer: match r.u8()? {
            u8::MAX => None,
            k => Some(k),
        },
        face_wait: r.i32()?,
        look: crate::look::Look {
            situation: r.u8()?,
            target: r.u8()?,
            timer: r.i32()?,
            last_state: r.i32()?,
            last_holder: match r.u8()? {
                u8::MAX => None,
                h => Some(h),
            },
            last_bits: u32::try_from(r.uv()?).map_err(|_| "invalid look bits")?,
            aim: look_aim_in(r)?,
            picks: u16::try_from(r.uv()?).map_err(|_| "invalid look picks")?,
        },
        left_handed: r.bool()?,
        dribble: r.u8()? as i8,
        channel_start: match r.u8()? {
            0 => None,
            1 => Some(crate::held::ChannelStart {
                script: u32::try_from(r.uv()?).map_err(|_| "invalid channel script")?,
                from: clip_in(r)?,
                left: r.bool()?,
                side: r.bool()?,
            }),
            k => return Err(format!("bad channel start flag {k}")),
        },
        course: r.f()?,
        wanted: r.f()?,
        guard: {
            let mut g = crate::guard::Guard {
                entered: r.iv()?,
                left: r.iv()?,
                jog_back: r.iv()?,
                other: r.iv()?,
                ..Default::default()
            };
            for v in &mut g.history {
                *v = r.i32()?;
            }
            g
        },
        flight: flight_in(r)?,
        on_rim: r.bool()?,
        aim_point: [r.f()?, r.f()?, r.f()?],
    })
}

fn trick_identity_out(w: &mut Writer, identity: crate::TrickIdentity) {
    w.u8(identity.slot);
    w.uv(u64::from(identity.script));
    w.u8(identity.level);
    w.u8(identity.equipped_row.map_or(0, |row| row + 1));
}

fn trick_identity_in(r: &mut Reader) -> Result<crate::TrickIdentity, String> {
    let slot = r.u8()?;
    let script = u16::try_from(r.uv()?).map_err(|_| "invalid trick script")?;
    let level = r.u8()?;
    let row = r.u8()?;
    Ok(crate::TrickIdentity {
        slot,
        script,
        level,
        equipped_row: row.checked_sub(1),
    })
}

/// A script's clock ([`crate::trick::Run`]).
fn run_out(w: &mut Writer, run: &crate::trick::Run) {
    w.u8(run.pc);
    w.iv(i64::from(run.count));
    w.iv(i64::from(run.tick));
    w.uv(run.record.map_or(0, |r| u64::from(r) + 1));
    w.f(run.frame);
    w.iv(i64::from(run.int_frame));
    match run.span {
        None => w.u8(0),
        Some(s) => {
            w.u8(1);
            w.bool(s.blend);
            w.uv(u64::from(s.length));
            w.uv(u64::from(s.counter));
            w.f(s.rate);
            w.u8(s.to);
            w.uv(s.to_record.map_or(0, |r| u64::from(r) + 1));
            w.u8(s.frames);
        }
    }
    w.iv(i64::from(run.landing));
    w.u8(run.flight);
    w.bool(run.left);
    w.bool(run.handed);
    w.bool(run.side);
    w.bool(run.done);
    w.iv(i64::from(run.land_at));
    for join in [run.join, run.outer] {
        match join {
            None => w.u8(0),
            Some((at, to)) => {
                w.u8(1);
                w.u8(at);
                w.u8(to);
            }
        }
    }
    w.u8(match run.hang {
        None => 0,
        Some(false) => 1,
        Some(true) => 2,
    });
    w.bool(run.asked);
    w.uv(run.before.map_or(0, |r| u64::from(r) + 1));
}

fn run_in(r: &mut Reader) -> Result<crate::trick::Run, String> {
    let small = |v: i32| i16::try_from(v).map_err(|_| "script clock out of range".to_string());
    let record = |v: u64| -> Result<Option<u16>, String> {
        match v {
            0 => Ok(None),
            v => Ok(Some(
                u16::try_from(v - 1).map_err(|_| "script record out of range")?,
            )),
        }
    };
    let pc = r.u8()?;
    let count = i8::try_from(r.i32()?).map_err(|_| "script count out of range")?;
    let tick = small(r.i32()?)?;
    let rec = record(r.uv()?)?;
    let frame = r.f()?;
    let int_frame = small(r.i32()?)?;
    let span = match r.u8()? {
        0 => None,
        1 => Some(crate::trick::Span {
            blend: r.bool()?,
            length: u16::try_from(r.uv()?).map_err(|_| "span out of range")?,
            counter: u16::try_from(r.uv()?).map_err(|_| "span out of range")?,
            rate: r.f()?,
            to: r.u8()?,
            to_record: record(r.uv()?)?,
            frames: r.u8()?,
        }),
        k => return Err(format!("bad span flag {k}")),
    };
    Ok(crate::trick::Run {
        pc,
        count,
        tick,
        record: rec,
        frame,
        int_frame,
        span,
        landing: small(r.i32()?)?,
        flight: r.u8()?,
        left: r.bool()?,
        handed: r.bool()?,
        side: r.bool()?,
        done: r.bool()?,
        land_at: small(r.i32()?)?,
        join: match r.u8()? {
            0 => None,
            1 => Some((r.u8()?, r.u8()?)),
            k => return Err(format!("bad join flag {k}")),
        },
        outer: match r.u8()? {
            0 => None,
            1 => Some((r.u8()?, r.u8()?)),
            k => return Err(format!("bad join flag {k}")),
        },
        hang: match r.u8()? {
            0 => None,
            1 => Some(false),
            2 => Some(true),
            k => return Err(format!("bad hang flag {k}")),
        },
        asked: r.bool()?,
        before: record(r.uv()?)?,
    })
}

/// A clip held through a blend (record and frame).
fn clip_out(w: &mut Writer, clip: Option<(u16, f32)>) {
    match clip {
        None => w.u8(0),
        Some((record, time)) => {
            w.u8(1);
            w.uv(u64::from(record));
            w.f(time);
        }
    }
}

fn clip_in(r: &mut Reader) -> Result<Option<(u16, f32)>, String> {
    Ok(match r.u8()? {
        0 => None,
        1 => Some((u16::try_from(r.uv()?).map_err(|_| "invalid clip")?, r.f()?)),
        k => return Err(format!("bad clip flag {k}")),
    })
}

fn native_tricks_in(r: &mut Reader) -> Result<Option<crate::NativeTricks>, String> {
    match r.u8()? {
        0 => Ok(None),
        1 => {
            let mut slots = [crate::TrickIdentity {
                slot: 0,
                script: 0,
                level: 0,
                equipped_row: None,
            }; 11];
            for identity in &mut slots {
                *identity = trick_identity_in(r)?;
            }
            Ok(Some(crate::NativeTricks { slots }))
        }
        tag => Err(format!("bad native trick slots flag {tag}")),
    }
}

fn dunk_style(s: DunkStyle) -> u8 {
    match s {
        DunkStyle::Normal => 0,
        DunkStyle::Advanced => 1,
        DunkStyle::Putback => 2,
    }
}

fn dunk_style_in(k: u8) -> Result<DunkStyle, String> {
    Ok(match k {
        0 => DunkStyle::Normal,
        1 => DunkStyle::Advanced,
        2 => DunkStyle::Putback,
        k => return Err(format!("bad dunk style {k}")),
    })
}

fn action_out(w: &mut Writer, a: &Action) {
    match *a {
        Action::None => w.u8(0),
        Action::Shooting {
            hoop,
            elapsed,
            gamebreaker,
            fade,
            tick,
            script,
            release,
            check,
            end,
            gate,
            latest,
            stall,
        } => {
            w.u8(1);
            w.us(hoop);
            w.f(elapsed);
            w.u8(gamebreaker);
            w.bool(fade);
            w.uv(u64::from(tick));
            w.u8(script);
            w.u8(release);
            w.u8(check);
            w.u8(end);
            w.u8(gate);
            w.u8(latest);
            w.u8(stall);
        }
        Action::Dunking {
            hoop,
            elapsed,
            from,
            to,
            style,
            layup,
            tick,
            release,
            end,
            row,
            hang,
        } => {
            w.u8(2);
            w.us(hoop);
            w.f(elapsed);
            w.f2(&from);
            w.f2(&to);
            w.u8(dunk_style(style));
            w.bool(layup);
            w.uv(u64::from(tick));
            w.u8(release);
            w.u8(end);
            w.u8(row);
            w.u8(match hang {
                None => 0,
                Some(false) => 1,
                Some(true) => 2,
            });
        }
        Action::Layup {
            hoop,
            elapsed,
            from,
            to,
            tick,
            release,
            end,
            row,
        } => {
            w.u8(3);
            w.us(hoop);
            w.f(elapsed);
            w.f2(&from);
            w.f2(&to);
            w.uv(u64::from(tick));
            w.u8(release);
            w.u8(end);
            w.u8(row);
        }
        Action::PumpFake {
            elapsed,
            script,
            tick,
        } => {
            w.u8(4);
            w.f(elapsed);
            w.u8(script);
            w.uv(u64::from(tick));
        }
        Action::Passing { elapsed, row, tick } => {
            w.u8(5);
            w.f(elapsed);
            w.u8(row);
            w.uv(u64::from(tick));
        }
        Action::Stealing {
            elapsed,
            counter,
            swipe,
            stance,
        } => {
            w.u8(6);
            w.f(elapsed);
            w.bool(counter);
            match swipe {
                None => w.u8(0),
                Some(s) => {
                    w.u8(1);
                    w.u8(s.script);
                    w.uv(u64::from(s.record));
                    match s.from {
                        None => w.u8(0),
                        Some((record, time)) => {
                            w.u8(1);
                            w.uv(u64::from(record));
                            w.f(time);
                        }
                    }
                    w.uv(u64::from(s.tick));
                    w.bool(s.tested);
                }
            }
            match stance {
                None => w.u8(0),
                Some(s) => {
                    w.u8(1);
                    w.u8(s.script);
                    run_out(w, &s.run);
                    clip_out(w, s.from);
                    w.iv(i64::from(s.word));
                }
            }
        }
        Action::Jumping {
            elapsed,
            high,
            block,
        } => {
            w.u8(7);
            w.f(elapsed);
            w.bool(high);
            match block {
                None => w.u8(0),
                Some(j) => {
                    w.u8(1);
                    w.u8(j.script);
                    w.bool(j.branch);
                    match j.from {
                        None => w.u8(0),
                        Some((record, time)) => {
                            w.u8(1);
                            w.uv(u64::from(record));
                            w.f(time);
                        }
                    }
                    w.uv(u64::from(j.tick));
                    w.iv(i64::from(j.word));
                    w.f(j.vy);
                    w.f2(&j.drift);
                    w.bool(j.up);
                    w.u8(j.meet);
                    w.uv(u64::from(j.airtime));
                    w.uv(u64::from(j.flown));
                    w.bool(j.turbo);
                }
            }
        }
        Action::Trick { id, play } => {
            w.u8(8);
            trick_identity_out(w, id);
            run_out(w, &play.run);
            w.f2(&play.drift);
            w.iv(i64::from(play.word));
            w.u8(play.row);
            w.u8(play.window);
            w.u8(play.faked.map_or(0, |d| d + 1));
            clip_out(w, play.from);
        }
        Action::Stumbling {
            elapsed,
            duration,
            reaction,
        } => {
            w.u8(9);
            w.f(elapsed);
            w.f(duration);
            match reaction {
                None => w.u8(0),
                Some(r) => {
                    w.u8(1);
                    w.u8(r.script);
                    run_out(w, &r.run);
                    clip_out(w, r.from);
                    w.f2(&r.drift);
                    w.f(r.vy);
                    w.bool(r.up);
                    w.u8(r.window);
                }
            }
        }
        Action::Dive { script, tick } => {
            w.u8(11);
            w.u8(script);
            w.uv(u64::from(tick));
        }
        Action::Scoop { play } => {
            w.u8(14);
            run_out(w, &play.run);
            clip_out(w, play.from);
            w.iv(i64::from(play.facing));
        }
        Action::TipJump {
            script,
            tick,
            vy,
            drift,
            up,
        } => {
            w.u8(12);
            w.u8(script);
            w.uv(tick.into());
            w.f(vy);
            w.f2(&drift);
            w.u8(u8::from(up));
        }
        Action::ReboundJump {
            script,
            tick,
            vy,
            drift,
            up,
            meet,
            end,
            barred,
            branch,
            from,
            airtime,
            word,
        } => {
            w.u8(13);
            w.u8(script);
            w.uv(tick.into());
            w.f(vy);
            w.f2(&drift);
            w.bool(up);
            w.u8(meet);
            w.uv(end.into());
            w.bool(barred);
            w.bool(branch);
            match from {
                None => w.u8(0),
                Some((record, time)) => {
                    w.u8(1);
                    w.uv(u64::from(record));
                    w.f(time);
                }
            }
            w.uv(airtime.into());
            w.iv(i64::from(word));
        }
    }
}

fn action_in(r: &mut Reader) -> Result<Action, String> {
    Ok(match r.u8()? {
        0 => Action::None,
        1 => Action::Shooting {
            hoop: r.us()?,
            elapsed: r.f()?,
            gamebreaker: r.u8()?,
            fade: r.bool()?,
            tick: r.uv()? as u16,
            script: r.u8()?,
            release: r.u8()?,
            check: r.u8()?,
            end: r.u8()?,
            gate: r.u8()?,
            latest: r.u8()?,
            stall: r.u8()?,
        },
        2 => Action::Dunking {
            hoop: r.us()?,
            elapsed: r.f()?,
            from: r.f2()?,
            to: r.f2()?,
            style: dunk_style_in(r.u8()?)?,
            layup: r.bool()?,
            tick: r.uv()? as u16,
            release: r.u8()?,
            end: r.u8()?,
            row: r.u8()?,
            hang: match r.u8()? {
                0 => None,
                1 => Some(false),
                2 => Some(true),
                k => return Err(format!("bad hang flag {k}")),
            },
        },
        3 => Action::Layup {
            hoop: r.us()?,
            elapsed: r.f()?,
            from: r.f2()?,
            to: r.f2()?,
            tick: r.uv()? as u16,
            release: r.u8()?,
            end: r.u8()?,
            row: r.u8()?,
        },
        4 => Action::PumpFake {
            elapsed: r.f()?,
            script: r.u8()?,
            tick: r.uv()? as u16,
        },
        5 => Action::Passing {
            elapsed: r.f()?,
            row: r.u8()?,
            tick: r.uv()? as u16,
        },
        6 => Action::Stealing {
            elapsed: r.f()?,
            counter: r.bool()?,
            swipe: match r.u8()? {
                0 => None,
                1 => Some(crate::swipe::Swipe {
                    script: r.u8()?,
                    record: u16::try_from(r.uv()?).map_err(|_| "invalid swipe record")?,
                    from: match r.u8()? {
                        0 => None,
                        1 => Some((
                            u16::try_from(r.uv()?).map_err(|_| "invalid swipe clip")?,
                            r.f()?,
                        )),
                        k => return Err(format!("bad swipe clip flag {k}")),
                    },
                    tick: u16::try_from(r.uv()?).map_err(|_| "invalid swipe tick")?,
                    tested: r.bool()?,
                }),
                k => return Err(format!("bad swipe flag {k}")),
            },
            stance: match r.u8()? {
                0 => None,
                1 => Some(crate::counter::Stance {
                    script: r.u8()?,
                    run: run_in(r)?,
                    from: clip_in(r)?,
                    word: i32::try_from(r.iv()?).map_err(|_| "invalid stance facing")?,
                }),
                k => return Err(format!("bad stance flag {k}")),
            },
        },
        7 => Action::Jumping {
            elapsed: r.f()?,
            high: r.bool()?,
            block: match r.u8()? {
                0 => None,
                1 => Some(crate::block::Jump {
                    script: r.u8()?,
                    branch: r.bool()?,
                    from: match r.u8()? {
                        0 => None,
                        1 => Some((
                            u16::try_from(r.uv()?).map_err(|_| "invalid block clip")?,
                            r.f()?,
                        )),
                        k => return Err(format!("bad block clip flag {k}")),
                    },
                    tick: u16::try_from(r.uv()?).map_err(|_| "invalid block tick")?,
                    word: r.i32()?,
                    vy: r.f()?,
                    drift: r.f2()?,
                    up: r.bool()?,
                    meet: r.u8()?,
                    airtime: u16::try_from(r.uv()?).map_err(|_| "invalid block airtime")?,
                    flown: u16::try_from(r.uv()?).map_err(|_| "invalid block flight")?,
                    turbo: r.bool()?,
                }),
                k => return Err(format!("bad block flag {k}")),
            },
        },
        8 => Action::Trick {
            id: trick_identity_in(r)?,
            play: crate::trick::Play {
                run: run_in(r)?,
                drift: r.f2()?,
                word: r.i32()?,
                row: r.u8()?,
                window: r.u8()?,
                faked: r.u8()?.checked_sub(1),
                from: clip_in(r)?,
            },
        },
        9 => Action::Stumbling {
            elapsed: r.f()?,
            duration: r.f()?,
            reaction: match r.u8()? {
                0 => None,
                1 => Some(crate::trick::Reacting {
                    script: r.u8()?,
                    run: run_in(r)?,
                    from: clip_in(r)?,
                    drift: r.f2()?,
                    vy: r.f()?,
                    up: r.bool()?,
                    window: r.u8()?,
                }),
                k => return Err(format!("bad reaction flag {k}")),
            },
        },
        11 => Action::Dive {
            script: r.u8()?,
            tick: u16::try_from(r.uv()?).map_err(|_| "invalid dive tick")?,
        },
        14 => Action::Scoop {
            play: crate::touch::Scooping {
                run: run_in(r)?,
                from: clip_in(r)?,
                facing: r.i32()?,
            },
        },
        12 => Action::TipJump {
            script: r.u8()?,
            tick: u16::try_from(r.uv()?).map_err(|_| "tip jump tick out of range")?,
            vy: r.f()?,
            drift: r.f2()?,
            up: match r.u8()? {
                0 => false,
                1 => true,
                k => return Err(format!("bad tip jump flag {k}")),
            },
        },
        13 => Action::ReboundJump {
            script: r.u8()?,
            tick: r.uv()? as u16,
            vy: r.f()?,
            drift: r.f2()?,
            up: r.bool()?,
            meet: r.u8()?,
            end: r.uv()? as u16,
            barred: r.bool()?,
            branch: r.bool()?,
            from: match r.u8()? {
                0 => None,
                1 => Some((
                    u16::try_from(r.uv()?).map_err(|_| "invalid rebound clip")?,
                    r.f()?,
                )),
                k => return Err(format!("bad rebound clip flag {k}")),
            },
            airtime: u16::try_from(r.uv()?).map_err(|_| "invalid rebound airtime")?,
            word: r.i32()?,
        },
        k => return Err(format!("bad action {k}")),
    })
}

fn ball_out(w: &mut Writer, b: &Ball) {
    let Ball {
        radius,
        position,
        velocity,
        state,
        shot,
        pass,
        ..
    } = b;
    w.f(*radius);
    w.f3(position);
    w.f3(velocity);
    match *state {
        BallState::Held(i) => {
            w.u8(0);
            w.us(i);
        }
        BallState::Shot => w.u8(1),
        BallState::Pass => w.u8(2),
        BallState::Loose => w.u8(3),
    }
    match shot {
        None => w.u8(0),
        Some(Shot {
            shooter,
            hoop,
            made,
            points,
            gamebreaker,
            layup,
            tried,
            slam,
        }) => {
            w.u8(1);
            w.us(*shooter);
            w.us(*hoop);
            w.bool(*made);
            w.uv((*points).into());
            w.u8(*gamebreaker);
            w.bool(*layup);
            w.u8(*tried);
            match slam {
                None => w.u8(0),
                Some(b) => {
                    w.u8(1);
                    basket(w, *b);
                }
            }
        }
    }
    match pass {
        None => w.u8(0),
        Some(Pass {
            from,
            to,
            tried,
            via,
            catch,
            since,
        }) => {
            w.u8(1);
            w.us(*from);
            w.us(*to);
            w.u8(*tried);
            w.u8(*catch);
            w.uv(*since);
            match *via {
                Via::Direct => w.u8(0),
                Via::Head { defender } => {
                    w.u8(1);
                    w.us(defender);
                }
                Via::Glass { hoop } => {
                    w.u8(2);
                    w.us(hoop);
                }
                Via::Floor => w.u8(3),
            }
        }
    }
    w.f(b.flight_left);
    w.f(b.flight_time);
    w.f(b.flight_start);
    w.f3(&b.spin_axis);
    w.f(b.spin);
    w.bool(b.airborne);
    w.bool(b.through);
    w.bool(b.bank);
    w.bool(b.armed);
    w.f(b.ceiling);
    w.bool(b.dunked);
    w.bool(b.rebound);
    w.bool(b.touched);
    w.f3(&b.moved);
    for (touch, long) in b.touch.iter().zip(b.long) {
        match touch {
            Some(p) => {
                w.u8(1);
                w.u8(*p);
            }
            None => w.u8(0),
        }
        w.bool(long);
    }
    match b.shooter {
        Some(p) => {
            w.u8(1);
            w.u8(p);
        }
        None => w.u8(0),
    }
    w.bool(b.rolled);
}

fn ball_in(r: &mut Reader) -> Result<Ball, String> {
    let radius = r.f()?;
    if !crate::ball::valid_radius(radius) {
        return Err("invalid native ball radius".into());
    }
    let position = r.f3()?;
    let velocity = r.f3()?;
    let state = match r.u8()? {
        0 => BallState::Held(r.us()?),
        1 => BallState::Shot,
        2 => BallState::Pass,
        3 => BallState::Loose,
        k => return Err(format!("bad ball state {k}")),
    };
    let shot = match r.u8()? {
        0 => None,
        _ => Some(Shot {
            shooter: r.us()?,
            hoop: r.us()?,
            made: r.bool()?,
            points: r.u32()?,
            gamebreaker: r.u8()?,
            layup: r.bool()?,
            tried: r.u8()?,
            slam: match r.u8()? {
                0 => None,
                _ => Some(basket_in(r)?),
            },
        }),
    };
    let pass = match r.u8()? {
        0 => None,
        _ => Some(Pass {
            from: r.us()?,
            to: r.us()?,
            tried: r.u8()?,
            catch: r.u8()?,
            since: r.uv()?,
            via: match r.u8()? {
                0 => Via::Direct,
                1 => Via::Head { defender: r.us()? },
                2 => Via::Glass { hoop: r.us()? },
                3 => Via::Floor,
                k => return Err(format!("bad pass route {k}")),
            },
        }),
    };
    let flight_left = r.f()?;
    let flight_time = r.f()?;
    let flight_start = r.f()?;
    let mut ball = Ball::loose_at(position);
    ball.radius = radius;
    ball.flight_left = flight_left;
    ball.flight_time = flight_time;
    ball.flight_start = flight_start;
    ball.spin_axis = r.f3()?;
    ball.spin = r.f()?;
    ball.airborne = r.bool()?;
    ball.through = r.bool()?;
    ball.bank = r.bool()?;
    ball.armed = r.bool()?;
    ball.ceiling = r.f()?;
    ball.dunked = r.bool()?;
    ball.rebound = r.bool()?;
    ball.touched = r.bool()?;
    ball.moved = r.f3()?;
    for side in 0..2 {
        ball.touch[side] = match r.u8()? {
            0 => None,
            _ => Some(r.u8()?),
        };
        ball.long[side] = r.bool()?;
    }
    ball.shooter = match r.u8()? {
        0 => None,
        _ => Some(r.u8()?),
    };
    ball.rolled = r.bool()?;
    ball.position = position;
    ball.velocity = velocity;
    ball.state = state;
    ball.shot = shot;
    ball.pass = pass;
    Ok(ball)
}

fn rules_out(w: &mut Writer, r: &Rules) {
    let Rules {
        goal,
        win_by,
        shot_clock,
        scoring,
        dunks_only,
        tricks,
        gamebreakers,
        gamebreakers_score,
        spot,
        spot_trick_points,
        unlimited_turbo,
        no_counters,
        easy_2_pointers,
        hard_2_pointers,
        difficulty,
    } = r;
    match *goal {
        Goal::Points(n) => {
            w.u8(0);
            w.uv(n.into());
        }
        Goal::TrickPoints(n) => {
            w.u8(1);
            w.uv(n.into());
        }
        Goal::Gamebreaker(l) => {
            w.u8(2);
            w.u8(l);
        }
    }
    w.uv((*win_by).into());
    match shot_clock {
        None => w.u8(0),
        Some(s) => {
            w.u8(1);
            w.f(*s);
        }
    }
    w.u8(match scoring {
        Scoring::Street => 0,
        Scoring::League => 1,
        Scoring::OldSchool => 2,
    });
    let flags = [
        dunks_only,
        tricks,
        gamebreakers,
        gamebreakers_score,
        unlimited_turbo,
        no_counters,
        easy_2_pointers,
        hard_2_pointers,
    ];
    let mut bits = 0u8;
    for (k, &&f) in flags.iter().enumerate() {
        bits |= u8::from(f) << k;
    }
    w.u8(bits);
    for v in spot.iter().chain(spot_trick_points) {
        w.uv((*v).into());
    }
    w.u8(match difficulty {
        Difficulty::Easy => 0,
        Difficulty::Medium => 1,
        Difficulty::Hard => 2,
    });
}

fn rules_in(r: &mut Reader) -> Result<Rules, String> {
    let goal = match r.u8()? {
        0 => Goal::Points(r.u32()?),
        1 => Goal::TrickPoints(r.u32()?),
        2 => Goal::Gamebreaker(r.u8()?),
        k => return Err(format!("bad goal {k}")),
    };
    let win_by = r.u32()?;
    let shot_clock = match r.u8()? {
        0 => None,
        _ => Some(r.f()?),
    };
    let scoring = match r.u8()? {
        0 => Scoring::Street,
        1 => Scoring::League,
        2 => Scoring::OldSchool,
        k => return Err(format!("bad scoring {k}")),
    };
    let bits = r.u8()?;
    let flag = |k: u8| bits & (1 << k) != 0;
    let spot = [r.u32()?, r.u32()?];
    let spot_trick_points = [r.u32()?, r.u32()?];
    let difficulty = match r.u8()? {
        0 => Difficulty::Easy,
        1 => Difficulty::Medium,
        2 => Difficulty::Hard,
        k => return Err(format!("bad difficulty {k}")),
    };
    Ok(Rules {
        goal,
        win_by,
        shot_clock,
        scoring,
        dunks_only: flag(0),
        tricks: flag(1),
        gamebreakers: flag(2),
        gamebreakers_score: flag(3),
        spot,
        spot_trick_points,
        unlimited_turbo: flag(4),
        no_counters: flag(5),
        easy_2_pointers: flag(6),
        hard_2_pointers: flag(7),
        difficulty,
    })
}

fn team(w: &mut Writer, t: &TeamState) {
    let TeamState {
        trick_points,
        combo,
        meter,
        meter_fraction,
        meter_fill,
        meter_scale,
        meter_sequence_fill,
        meter_hold,
        meter_gamebreaker,
        meter_cpu_limit,
        human,
        pocketed,
        recent_moves,
        paid,
        pump_fake,
        possession_seen,
    } = t;
    w.uv((*trick_points).into());
    w.f(*meter_scale);
    w.iv((*meter_hold).into());
    w.bool(*meter_gamebreaker);
    let Combo {
        points,
        multiplier,
        moves,
    } = combo;
    for v in [points, multiplier, moves, meter] {
        w.uv((*v).into());
    }
    w.bool(*pocketed);
    w.f(*meter_fraction);
    w.f(*meter_fill);
    w.f(*meter_sequence_fill);
    w.bool(*meter_cpu_limit);
    w.bool(*human);
    for &key in recent_moves {
        w.iv(key.into());
    }
    w.bool(*paid);
    w.bool(*pump_fake);
    w.iv((*possession_seen).into());
}

fn team_in(r: &mut Reader) -> Result<TeamState, String> {
    let trick_points = r.u32()?;
    let meter_scale = r.f()?;
    let meter_hold = r.i32()?;
    let meter_gamebreaker = r.bool()?;
    let combo = Combo {
        points: r.u32()?,
        multiplier: r.u32()?,
        moves: r.u32()?,
    };
    let meter = r.u32()?;
    let pocketed = r.bool()?;
    let meter_fraction = r.f()?;
    let meter_fill = r.f()?;
    let meter_sequence_fill = r.f()?;
    let meter_cpu_limit = r.bool()?;
    let human = r.bool()?;
    let mut recent_moves = [-1; 4];
    for m in &mut recent_moves {
        *m = r.i32()?;
    }
    let paid = r.bool()?;
    let pump_fake = r.bool()?;
    let possession_seen = r.i32()?;
    Ok(TeamState {
        trick_points,
        combo,
        meter,
        meter_fraction,
        meter_fill,
        meter_scale,
        meter_sequence_fill,
        meter_hold,
        meter_gamebreaker,
        meter_cpu_limit,
        human,
        pocketed,
        recent_moves,
        paid,
        pump_fake,
        possession_seen,
    })
}

fn stats_out(w: &mut Writer, s: &crate::stats::Stats) {
    let crate::stats::Stats {
        players,
        teams,
        sides,
        on_court,
        rebound,
        paid,
        pump_fake,
        fills,
    } = s;
    w.uv(players.len() as u64);
    for p in players {
        for &v in p {
            w.iv(v.into());
        }
    }
    for t in teams {
        for &v in t {
            w.iv(v.into());
        }
    }
    for t in sides {
        for &v in t {
            w.iv(v.into());
        }
    }
    w.uv(on_court.len() as u64);
    for &f in on_court {
        w.uv(f.into());
    }
    w.bool(*rebound);
    w.bool(*paid);
    w.bool(*pump_fake);
    for side in fills {
        for &v in side {
            w.iv(v.into());
        }
    }
}

fn stats_in(r: &mut Reader) -> Result<crate::stats::Stats, String> {
    use crate::stats::{ACTOR_STATS, SIDE_STATS};
    let n = r.len(64)?;
    let mut players = vec![[0; ACTOR_STATS]; n];
    for p in &mut players {
        for v in p.iter_mut() {
            *v = r.i32()?;
        }
    }
    let mut teams = [[0; ACTOR_STATS]; 2];
    for t in &mut teams {
        for v in t.iter_mut() {
            *v = r.i32()?;
        }
    }
    let mut sides = [[0; SIDE_STATS]; 2];
    for t in &mut sides {
        for v in t.iter_mut() {
            *v = r.i32()?;
        }
    }
    let n = r.len(64)?;
    let on_court = (0..n).map(|_| r.u32()).collect::<Result<Vec<_>, _>>()?;
    let rebound = r.bool()?;
    let paid = r.bool()?;
    let pump_fake = r.bool()?;
    let mut fills = [[0; 2]; 2];
    for side in &mut fills {
        for v in side.iter_mut() {
            *v = r.i32()?;
        }
    }
    Ok(crate::stats::Stats {
        players,
        teams,
        sides,
        on_court,
        rebound,
        paid,
        pump_fake,
        fills,
    })
}

fn meter_clock_byte(c: crate::street::MeterClock) -> u8 {
    match c {
        crate::street::MeterClock::Off => 0,
        crate::street::MeterClock::Inbound => 1,
        crate::street::MeterClock::On => 2,
    }
}

fn pass_style(s: PassStyle) -> u8 {
    match s {
        PassStyle::Normal => 0,
        PassStyle::GiveAndGo => 1,
        PassStyle::Kick => 2,
        PassStyle::Heezay => 3,
        PassStyle::Glass => 4,
        PassStyle::Oop => 5,
    }
}

fn pass_style_in(k: u8) -> Result<PassStyle, String> {
    Ok(match k {
        0 => PassStyle::Normal,
        1 => PassStyle::GiveAndGo,
        2 => PassStyle::Kick,
        3 => PassStyle::Heezay,
        4 => PassStyle::Glass,
        5 => PassStyle::Oop,
        k => return Err(format!("bad pass style {k}")),
    })
}

fn basket(w: &mut Writer, b: Basket) {
    match b {
        Basket::Jumper { long } => {
            w.u8(0);
            w.bool(long);
        }
        Basket::Dunk { advanced } => {
            w.u8(1);
            w.bool(advanced);
        }
        Basket::Layup => w.u8(2),
        Basket::AlleyOop { advanced } => {
            w.u8(3);
            w.bool(advanced);
        }
        Basket::Gamebreaker { level, long, dunk } => {
            w.u8(4);
            w.u8(level);
            w.bool(long);
            w.bool(dunk);
        }
    }
}

fn basket_in(r: &mut Reader) -> Result<Basket, String> {
    Ok(match r.u8()? {
        0 => Basket::Jumper { long: r.bool()? },
        1 => Basket::Dunk {
            advanced: r.bool()?,
        },
        2 => Basket::Layup,
        3 => Basket::AlleyOop {
            advanced: r.bool()?,
        },
        4 => Basket::Gamebreaker {
            level: r.u8()?,
            long: r.bool()?,
            dunk: r.bool()?,
        },
        k => return Err(format!("bad basket {k}")),
    })
}

fn event(w: &mut Writer, e: &Event) {
    match *e {
        Event::Shot {
            player,
            hoop,
            points,
        } => {
            w.u8(0);
            w.us(player);
            w.us(hoop);
            w.uv(points.into());
        }
        Event::Dunk { player, hoop } => {
            w.u8(1);
            w.us(player);
            w.us(hoop);
        }
        Event::Score {
            team,
            player,
            points,
            basket: b,
        } => {
            w.u8(2);
            w.us(team);
            w.us(player);
            w.uv(points.into());
            basket(w, b);
        }
        Event::RimOut => w.u8(3),
        Event::Block { player, shooter } => {
            w.u8(4);
            w.us(player);
            w.us(shooter);
        }
        Event::Pass { from, to, style } => {
            w.u8(5);
            w.us(from);
            w.us(to);
            w.u8(pass_style(style));
        }
        Event::Catch { player } => {
            w.u8(6);
            w.us(player);
        }
        Event::Steal {
            player,
            from,
            intercepted_pass,
        } => {
            w.u8(7);
            w.us(player);
            w.us(from);
            w.bool(intercepted_pass);
        }
        Event::Pickup { player } => {
            w.u8(8);
            w.us(player);
        }
        Event::Rebound { player } => {
            w.u8(34);
            w.us(player);
        }
        Event::ShotClockViolation { team } => {
            w.u8(9);
            w.us(team);
        }
        Event::Inbound { player } => {
            w.u8(10);
            w.us(player);
        }
        Event::GameOver { winner } => {
            w.u8(11);
            w.us(winner);
        }
        Event::Trick {
            player,
            level,
            script,
            fakeout,
            points,
        } => {
            w.u8(12);
            w.us(player);
            w.u8(level);
            w.uv(u64::from(script));
            // 0 for none, else the defender plus one.
            w.uv(fakeout.map_or(0, |d| d as u64 + 1));
            w.uv(points.into());
        }
        Event::Fumble { player } => {
            w.u8(13);
            w.us(player);
        }
        Event::Counter { player, from } => {
            w.u8(14);
            w.us(player);
            w.us(from);
        }
        Event::Deflection { player } => {
            w.u8(30);
            w.us(player);
        }
        Event::Combo {
            team,
            points,
            multiplier,
            walked,
            paid,
            newest,
        } => {
            w.u8(15);
            w.us(team);
            w.uv(points.into());
            w.uv(multiplier.into());
            for class in walked {
                w.iv(class.into());
            }
            w.uv(paid.into());
            w.iv(newest.into());
        }
        Event::GamebreakerReady { team } => {
            w.u8(16);
            w.us(team);
        }
        Event::Pocket { team } => {
            w.u8(17);
            w.us(team);
        }
        Event::Cancel { team } => {
            w.u8(18);
            w.us(team);
        }
        Event::Gamebreaker { player, level } => {
            w.u8(19);
            w.us(player);
            w.u8(level);
        }
        Event::GamebreakerFrame {
            script,
            frame,
            mirrored,
        } => {
            w.u8(28);
            w.us(script);
            w.uv(u64::from(frame));
            w.bool(mirrored);
        }
        Event::Layup { player } => {
            w.u8(20);
            w.us(player);
        }
        Event::PumpFake { player } => {
            w.u8(21);
            w.us(player);
        }
        Event::PumpFakeBite { player } => {
            w.u8(32);
            w.us(player);
        }
        Event::NoTurbo { player } => {
            w.u8(36);
            w.us(player);
        }
        Event::TakeOff {
            player,
            leap,
            finish,
            dunk,
        } => {
            w.u8(37);
            w.us(player);
            w.bool(leap);
            w.u8(finish);
            w.uv(u64::from(dunk));
        }
        Event::BallHit { surface, speed } => {
            w.u8(39);
            w.u8(surface as u8);
            w.f(speed);
        }
        Event::RimHold { player, on } => {
            w.u8(40);
            w.us(player);
            w.bool(on);
        }
        Event::Launched {
            from,
            to,
            kick,
            leaper,
        } => {
            w.u8(38);
            w.us(from);
            w.us(to);
            w.bool(kick);
            w.bool(leaper);
        }
        Event::Goaltend { player, shooter } => {
            w.u8(22);
            w.us(player);
            w.us(shooter);
        }
        Event::Dive { player } => {
            w.u8(23);
            w.us(player);
        }
        Event::Dribble { holder, speed } => {
            w.u8(35);
            w.us(holder);
            w.f(speed);
        }
        Event::SlamShake { player } => {
            w.u8(41);
            w.us(player);
        }
        Event::Take { player } => {
            w.u8(42);
            w.us(player);
        }
        Event::Pick { screener, defender } => {
            w.u8(24);
            w.us(screener);
            w.us(defender);
        }
        Event::Reach { player, counter } => {
            w.u8(25);
            w.us(player);
            w.bool(counter);
        }
        Event::Jump { player, block } => {
            w.u8(26);
            w.us(player);
            w.bool(block);
        }
        Event::Slam { player, hoop } => {
            w.u8(27);
            w.us(player);
            w.us(hoop);
        }
        Event::MeterDropped { team } => {
            w.u8(29);
            w.us(team);
        }
        Event::ReplayRoll { draw } => {
            w.u8(31);
            w.uv(u64::from(draw));
        }
        Event::StagedRerun {
            script,
            mirrored,
            from,
            frames,
            speed,
            camera,
        } => {
            w.u8(33);
            w.us(script);
            w.bool(mirrored);
            w.uv(u64::from(from));
            w.uv(u64::from(frames));
            w.u8(speed);
            w.u8(camera);
        }
    }
}

fn event_in(r: &mut Reader) -> Result<Event, String> {
    Ok(match r.u8()? {
        0 => Event::Shot {
            player: r.us()?,
            hoop: r.us()?,
            points: r.u32()?,
        },
        1 => Event::Dunk {
            player: r.us()?,
            hoop: r.us()?,
        },
        2 => Event::Score {
            team: r.us()?,
            player: r.us()?,
            points: r.u32()?,
            basket: basket_in(r)?,
        },
        3 => Event::RimOut,
        4 => Event::Block {
            player: r.us()?,
            shooter: r.us()?,
        },
        5 => Event::Pass {
            from: r.us()?,
            to: r.us()?,
            style: pass_style_in(r.u8()?)?,
        },
        6 => Event::Catch { player: r.us()? },
        7 => Event::Steal {
            player: r.us()?,
            from: r.us()?,
            intercepted_pass: r.bool()?,
        },
        8 => Event::Pickup { player: r.us()? },
        34 => Event::Rebound { player: r.us()? },
        9 => Event::ShotClockViolation { team: r.us()? },
        10 => Event::Inbound { player: r.us()? },
        11 => Event::GameOver { winner: r.us()? },
        12 => Event::Trick {
            player: r.us()?,
            level: r.u8()?,
            script: u16::try_from(r.uv()?).map_err(|_| "invalid trick script")?,
            fakeout: match r.us()? {
                0 => None,
                d => Some(d - 1),
            },
            points: r.u32()?,
        },
        13 => Event::Fumble { player: r.us()? },
        14 => Event::Counter {
            player: r.us()?,
            from: r.us()?,
        },
        15 => Event::Combo {
            team: r.us()?,
            points: r.u32()?,
            multiplier: r.u32()?,
            walked: {
                let mut walked = [0i8; 6];
                for class in &mut walked {
                    *class = i8::try_from(r.i32()?).map_err(|e| format!("combo class: {e}"))?;
                }
                walked
            },
            paid: r.u32()?,
            newest: r.i32()?,
        },
        16 => Event::GamebreakerReady { team: r.us()? },
        17 => Event::Pocket { team: r.us()? },
        18 => Event::Cancel { team: r.us()? },
        19 => Event::Gamebreaker {
            player: r.us()?,
            level: r.u8()?,
        },
        28 => Event::GamebreakerFrame {
            script: r.us()?,
            frame: r.u32()?,
            mirrored: r.bool()?,
        },
        20 => Event::Layup { player: r.us()? },
        21 => Event::PumpFake { player: r.us()? },
        22 => Event::Goaltend {
            player: r.us()?,
            shooter: r.us()?,
        },
        23 => Event::Dive { player: r.us()? },
        35 => Event::Dribble {
            holder: r.us()?,
            speed: r.f()?,
        },
        41 => Event::SlamShake { player: r.us()? },
        42 => Event::Take { player: r.us()? },
        24 => Event::Pick {
            screener: r.us()?,
            defender: r.us()?,
        },
        25 => Event::Reach {
            player: r.us()?,
            counter: r.bool()?,
        },
        26 => Event::Jump {
            player: r.us()?,
            block: r.bool()?,
        },
        27 => Event::Slam {
            player: r.us()?,
            hoop: r.us()?,
        },
        29 => Event::MeterDropped { team: r.us()? },
        30 => Event::Deflection { player: r.us()? },
        31 => Event::ReplayRoll { draw: r.u32()? },
        33 => Event::StagedRerun {
            script: r.us()?,
            mirrored: r.bool()?,
            from: r.u32()?,
            frames: r.u32()?,
            speed: r.u8()?,
            camera: r.u8()?,
        },
        32 => Event::PumpFakeBite { player: r.us()? },
        36 => Event::NoTurbo { player: r.us()? },
        37 => Event::TakeOff {
            player: r.us()?,
            leap: r.bool()?,
            finish: r.u8()?,
            dunk: u16::try_from(r.uv()?).map_err(|_| "invalid dunk id")?,
        },
        39 => Event::BallHit {
            surface: match r.u8()? {
                0 => crate::BallSurface::Floor,
                1 => crate::BallSurface::Board,
                2 => crate::BallSurface::Rim,
                3 => crate::BallSurface::Net,
                k => return Err(format!("bad ball surface {k}")),
            },
            speed: r.f()?,
        },
        40 => Event::RimHold {
            player: r.us()?,
            on: r.bool()?,
        },
        38 => Event::Launched {
            from: r.us()?,
            to: r.us()?,
            kick: r.bool()?,
            leaper: r.bool()?,
        },
        k => return Err(format!("bad event {k}")),
    })
}

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }

    fn bool(&mut self, v: bool) {
        self.0.push(u8::from(v));
    }

    fn uv(&mut self, mut v: u64) {
        loop {
            let byte = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.0.push(byte);
                return;
            }
            self.0.push(byte | 0x80);
        }
    }

    fn us(&mut self, v: usize) {
        self.uv(v as u64);
    }

    fn iv(&mut self, v: i64) {
        self.uv(((v << 1) ^ (v >> 63)) as u64);
    }

    fn f(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_bits().to_le_bytes());
    }

    fn f2(&mut self, v: &[f32; 2]) {
        v.iter().for_each(|&x| self.f(x));
    }

    fn f3(&mut self, v: &[f32; 3]) {
        v.iter().for_each(|&x| self.f(x));
    }
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.b.len());
        let end = end.ok_or("world snapshot cut short")?;
        let s = &self.b[self.at..end];
        self.at = end;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn bool(&mut self) -> Result<bool, String> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            k => Err(format!("bad flag {k}")),
        }
    }

    fn uv(&mut self) -> Result<u64, String> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.u8()?;
            v |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err("varint too long".into())
    }

    fn us(&mut self) -> Result<usize, String> {
        usize::try_from(self.uv()?).map_err(|_| "index out of range".into())
    }

    fn u32(&mut self) -> Result<u32, String> {
        u32::try_from(self.uv()?).map_err(|_| "value out of range".into())
    }

    fn i32(&mut self) -> Result<i32, String> {
        let u = self.uv()?;
        let v = ((u >> 1) as i64) ^ -((u & 1) as i64);
        i32::try_from(v).map_err(|_| "value out of range".into())
    }

    fn iv(&mut self) -> Result<i64, String> {
        let u = self.uv()?;
        Ok(((u >> 1) as i64) ^ -((u & 1) as i64))
    }

    /// A list length, at most `max`.
    fn len(&mut self, max: usize) -> Result<usize, String> {
        let n = self.us()?;
        if n > max {
            return Err(format!("list of {n}, more than {max}"));
        }
        Ok(n)
    }

    fn f(&mut self) -> Result<f32, String> {
        let b: [u8; 4] = self.take(4)?.try_into().expect("4 bytes");
        Ok(f32::from_bits(u32::from_le_bytes(b)))
    }

    fn f2(&mut self) -> Result<[f32; 2], String> {
        Ok([self.f()?, self.f()?])
    }

    fn f3(&mut self) -> Result<[f32; 3], String> {
        Ok([self.f()?, self.f()?, self.f()?])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ai, tests::hoops};

    fn three_on_three(seed: u32) -> World {
        let mut players = Vec::new();
        for team in 0..2 {
            let side = crate::restart::side(team);
            for k in 0..3 {
                players.push(Player::new(
                    team,
                    [side * 10.0, (k as f32 - 1.0) * 8.0],
                    -side * std::f32::consts::FRAC_PI_2,
                ));
            }
        }
        World::new(hoops(), players, Rules::default(), seed)
    }

    /// Computer players on both sides, so the game goes everywhere.
    fn step_ai(w: &mut World) {
        let inputs: Vec<Input> = (0..w.players.len()).map(|i| ai::input(w, i)).collect();
        w.step(&inputs);
    }

    #[test]
    fn steal_origin_survives_snapshot_and_distinguishes_events() {
        let mut world = three_on_three(11);
        for intercepted_pass in [false, true] {
            world.events = vec![Event::Steal {
                player: 0,
                from: 3,
                intercepted_pass,
            }];
            let restored = World::from_snapshot(&world.to_snapshot()).unwrap();
            assert_eq!(restored.events, world.events);
            assert_eq!(restored, world);
        }
    }

    #[test]
    fn a_rebounds_scoring_event_round_trips_separately_from_a_pickup() {
        let mut world = three_on_three(11);
        world.events = vec![Event::Rebound { player: 0 }, Event::Pickup { player: 0 }];
        let restored = World::from_snapshot(&world.to_snapshot()).unwrap();
        assert_eq!(restored.events, world.events);
        assert!(restored.validate().is_ok());
        world.events = vec![Event::Rebound { player: 6 }];
        assert!(world.validate().is_err());
    }

    #[test]
    fn generator_state_round_trips() {
        for seed in [0, 1, 7, 42, 0xdead_beef, u32::MAX] {
            let mut rng = Rng::new(seed);
            for _ in 0..50 {
                assert_eq!(Rng::from_state(rng.state()), rng);
                rng.next_u32();
            }
        }
    }

    #[test]
    fn world_round_trips_through_a_whole_game() {
        // Round-trip every update until a ball has been caught in the air
        // and 6000 more updates have passed (seed 3's computer players).
        let mut w = three_on_three(3);
        w.side_teams = [crate::WHITEWATER, 49];
        let mut in_flight = false;
        let mut left = 6000;
        for _ in 0..30_000 {
            if in_flight {
                left -= 1;
                if left == 0 {
                    break;
                }
            }
            let bytes = w.to_snapshot();
            let back = World::from_snapshot(&bytes).expect("reads back");
            assert_eq!(back, w, "tick {}", w.tick);
            assert_eq!(back.checksum(), w.checksum());
            in_flight |= matches!(w.ball.state, BallState::Shot | BallState::Pass);
            step_ai(&mut w);
        }
        assert!(in_flight, "the test never caught a ball in the air");
    }

    #[test]
    fn a_restored_world_plays_on_identically() {
        let mut w = three_on_three(3);
        w.configure_meter_fill([true, false], Some(0.71));
        w.teams[0].meter_fraction = 0.375;
        for _ in 0..900 {
            step_ai(&mut w);
        }
        let mut copy = World::from_snapshot(&w.to_snapshot()).expect("reads back");
        for _ in 0..1800 {
            step_ai(&mut w);
            step_ai(&mut copy);
            assert_eq!(copy.checksum(), w.checksum());
        }
        assert_eq!(copy, w);
    }

    #[test]
    fn snapshots_are_small_and_checked() {
        let w = three_on_three(5);
        let bytes = w.to_snapshot();
        // About 200 of these are the match's statistics (version 28); each
        // player's rise and bar add five more (version 41), his course,
        // wanted facing and guarding state about fifteen (version 43), his
        // flight about forty-five (version 46: nine floats and its flags,
        // landing, script, bones and meet), its slide and his four turbo
        // releases about ten more (version 48), his power and overall
        // ratings eight (version 51); the fresh take's flag and each
        // stored input's view word thirteen (version 53); the people's
        // control twenty, its tables sixty more once a step has built
        // them, and the spot run and the passout a dozen (version 56); the
        // sides' teams two (version 65); the actors' update order six
        // (version 69); each player's knock and shove windows, met, beat,
        // airtime and updates aloft six (version 73); each player's stand,
        // its count and the box-out three, the C library's word five
        // (version 74); each look a dozen (version 76); the two rims, their
        // rigs and each player's hold on the rim about 420 (version 91);
        // each look's aim and its picks' count two or more (version 92).
        assert!(bytes.len() < 2450, "{} bytes", bytes.len());
        assert!(World::from_snapshot(&bytes[..bytes.len() - 1]).is_err());
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(World::from_snapshot(&longer).is_err());
        assert!(World::from_snapshot(b"XX").is_err());
    }

    fn native_tricks() -> crate::NativeTricks {
        let defaults = [20, 17, 19, 22, 12, 16, 56, 57, 63, 66, 69];
        crate::NativeTricks {
            slots: std::array::from_fn(|slot| crate::TrickIdentity {
                slot: slot as u8,
                script: defaults[slot],
                level: crate::player::TRICK_SLOT_LEVELS[slot],
                equipped_row: None,
            }),
        }
    }

    #[test]
    fn native_trick_identity_owns_level_cost_repeat_key_and_replay() {
        let mut w = three_on_three(5);
        w.players[0].native_tricks = Some(native_tricks());
        w.players[0].ratings.handles = 1.0;
        w.phase = Phase::Live;
        w.take_ball(0);
        // Dribbling (a fresh take standing still clears out instead).
        for p in &mut w.players {
            p.dribble = 1;
        }
        w.settle_looks();
        let mut expected_rng = w.rng.clone();
        let initiating = crate::button::TRICK | crate::button::L2 | crate::button::R2;
        w.step(&[Input::default().with(initiating)]);
        assert!(matches!(
            w.players[0].action,
            Action::Trick {
                id: crate::TrickIdentity {
                    slot: 10,
                    script: 69,
                    level: 2,
                    equipped_row: None,
                },
                ..
            }
        ));
        // The direct slot draws no candidate: the face's draw and the
        // fumble's (value class 2) only, after the five others' stand
        // draws (the defence's `r & 1`, the offence's `r % 15`; the
        // actors update from the highest index).
        for _ in 0..7 {
            expected_rng.next_u32();
        }
        assert_eq!(w.rng.state(), expected_rng.state());
        // The bar's update, then the juke press's cost.
        assert_eq!(
            w.players[0].turbo,
            1.0 - crate::tuning::TURBO_DRAIN - crate::turbo::MOVE
        );
        let mut restored = World::from_snapshot(&w.to_snapshot()).unwrap();
        assert_eq!(restored, w);
        for _ in 0..60 {
            w.step(&[Input::default()]);
            restored.step(&[Input::default()]);
            assert_eq!(restored.checksum(), w.checksum());
        }
        assert_eq!(restored, w);
        assert_eq!(w.teams[0].recent_moves[0], 69);
        assert_eq!(w.teams[0].trick_points, crate::street::points::TRICK[2] / 2);
        assert_eq!(w.combo.ring.slots[5].extra, 2);
        assert_eq!(w.players[0].action, Action::None);
    }

    #[test]
    fn native_script_zero_is_not_a_repeat_of_empty_ring_entries() {
        let mut w = three_on_three(5);
        let mut tricks = native_tricks();
        tricks.slots[10].script = 0;
        w.players[0].native_tricks = Some(tricks);
        w.players[0].ratings.handles = 1.0;
        w.phase = Phase::Live;
        w.take_ball(0);
        // Dribbling (a fresh take standing still clears out instead).
        for p in &mut w.players {
            p.dribble = 1;
        }
        let buttons = crate::button::TRICK | crate::button::L2 | crate::button::R2;
        for _ in 0..2 {
            w.step(&[Input::default().with(buttons)]);
            for _ in 0..60 {
                w.step(&[Input::default()]);
            }
        }
        assert_eq!(w.teams[0].trick_points, 1000 + 500);
        assert_eq!(w.teams[0].recent_moves[0], 0);
        assert_eq!(World::from_snapshot(&w.to_snapshot()).unwrap(), w);
    }

    #[test]
    fn fourth_native_repeat_rounds_the_award_up() {
        for (slot, full, plain) in [(1, 188, 94), (0, 313, 157)] {
            for (qualified, expected) in [(false, plain), (true, full)] {
                let mut w = three_on_three(5);
                let identity = native_tricks().slots[slot];
                w.players[0].native_tricks = Some(native_tricks());
                w.players[0].ratings.handles = 1.0;
                w.phase = Phase::Live;
                w.take_ball(0);
                w.teams[0].recent_moves = [i32::from(identity.script); 4];
                // A fakeout (actor+0x218) qualifies the full base.
                w.score_trick(0, identity, qualified.then_some(3));
                assert_eq!(w.teams[0].trick_points, expected);
                assert_eq!(World::from_snapshot(&w.to_snapshot()).unwrap(), w);
            }
        }
    }

    #[test]
    fn an_unmatched_mask_draws_from_the_turbo_row() {
        // One turbo matches no slot (`0x15f358`): the draw at value class 1
        // among the scripts the ordinary row gives mask 2.
        let mut w = three_on_three(5);
        w.players[0].native_tricks = Some(native_tricks());
        w.phase = Phase::Live;
        w.take_ball(0);
        // Dribbling (a fresh take standing still clears out instead).
        for p in &mut w.players {
            p.dribble = 1;
        }
        w.step(&[Input::default().with(crate::button::TRICK | crate::button::L2)]);
        let Action::Trick { id, .. } = w.players[0].action else {
            panic!("no trick");
        };
        assert_eq!((id.slot, id.level), (crate::trick::DRAWN, 1));
        assert_eq!(crate::trick::SCRIPTS[usize::from(id.script)].mask, 2);
        assert_eq!(w.last_trick, id.script as u8);
        assert_eq!(World::from_snapshot(&w.to_snapshot()).unwrap(), w);
    }

    #[test]
    fn the_staged_players_values_change_the_checksum_and_the_discs_tables_do_not() {
        // The match's own values (the staged players' scales and width
        // ratios) are game state; the hand tracks are the disc's, which the
        // machines compare by digest before a match (crate::disc).
        let mut world = three_on_three(7);
        let config = crate::gamebreaker::Config {
            player_scales: vec![1.0; 6],
            player_width_ratios: vec![1.0; 6],
            hands: vec![crate::gamebreaker::HandTrack {
                animation: 296,
                positions: vec![[0.25, 2.0, 0.5]; 2],
                width_deltas: vec![[0.125, 0.0, 0.0]; 2],
            }],
            ..Default::default()
        };
        config.validate().unwrap();
        world.staged = Some(std::sync::Arc::new(config.clone()));
        let original = world.checksum();
        let back = World::from_snapshot(&world.to_snapshot()).unwrap();
        assert_eq!(back.checksum(), original);
        let shell = back.staged.as_deref().unwrap();
        assert_eq!(shell.player_scales, config.player_scales);
        assert_eq!(shell.player_width_ratios, config.player_width_ratios);
        assert!(shell.hands.is_empty() && shell.plans.is_empty());
        assert!(back.poses.get().is_none());
        for change in 0..3 {
            let mut altered = config.clone();
            match change {
                0 => altered.player_scales[0] = 1.1,
                1 => altered.player_width_ratios[0] = 1.2,
                _ => altered.hands[0].width_deltas[1][0] += 0.000001,
            }
            altered.validate().unwrap();
            world.staged = Some(std::sync::Arc::new(altered));
            assert_eq!(world.checksum() != original, change < 2);
            let restored = World::from_snapshot(&world.to_snapshot()).unwrap();
            assert_eq!(restored.checksum(), world.checksum());
        }
    }

    #[test]
    fn matched_native_trick_mask_plays_its_slot_on_the_juke_press_fifth() {
        // Every slot costs the juke press's 0.2 of the bar
        // (`FUN_0015c8e8`), whatever its value class: with just that left
        // after the held turbo's drain the slot plays and the bar is
        // empty; short of it, nothing.
        for (slot, mask) in crate::player::TRICK_SLOT_MASKS.into_iter().enumerate() {
            let buttons = [
                (0x4000, crate::button::L1),
                (0x8000, crate::button::R1),
                (0x10000, crate::button::L2),
                (0x20000, crate::button::R2),
            ]
            .into_iter()
            .filter(|&(native, _)| mask & native != 0)
            .fold(crate::button::TRICK, |buttons, (_, button)| {
                buttons | button
            });
            let mut w = three_on_three(9);
            w.phase = Phase::Live;
            w.take_ball(0);
            // Dribbling (a fresh take standing still clears out instead).
            for p in &mut w.players {
                p.dribble = 1;
            }
            w.players[0].native_tricks = Some(native_tricks());
            w.players[0].ratings.handles = 1.0;
            let mut short = w.clone();
            short.set_turbo(0, 0.19 + crate::tuning::TURBO_DRAIN);
            short.step(&[Input::default().with(buttons)]);
            assert_eq!(short.players[0].action, Action::None, "slot {slot}");
            w.set_turbo(0, crate::turbo::MOVE + crate::tuning::TURBO_DRAIN);
            let expected = native_tricks().slots[slot];
            w.settle_looks();
            let mut expected_rng = w.rng.clone();
            w.step(&[Input::default().with(buttons)]);
            assert!(
                matches!(w.players[0].action,
                Action::Trick { id, .. } if id == expected),
                "slot {slot}"
            );
            // The face's draw and the fumble's (every slot is class 2 or
            // more); no candidate draw; the five others' stand draws
            // before them.
            for _ in 0..7 {
                expected_rng.next_u32();
            }
            assert_eq!(w.rng, expected_rng, "slot {slot}");
            let mut restored = World::from_snapshot(&w.to_snapshot()).unwrap();
            for _ in 0..120 {
                w.step(&[Input::default()]);
                restored.step(&[Input::default()]);
                assert_eq!(restored.checksum(), w.checksum(), "slot {slot}");
            }
            assert_eq!(w.players[0].action, Action::None, "slot {slot}");
            assert_eq!(w.teams[0].recent_moves[0], i32::from(expected.script));
            assert_eq!(
                w.teams[0].trick_points,
                crate::street::points::TRICK[usize::from(expected.level)] / 2,
                "slot {slot}"
            );
        }
    }

    #[test]
    fn native_trick_configuration_changes_checksum_and_rejects_bad_identity() {
        let mut w = three_on_three(9);
        w.players[0].native_tricks = Some(native_tricks());
        let original = w.checksum();
        w.players[0].native_tricks.as_mut().unwrap().slots[5].script = 23;
        assert_ne!(w.checksum(), original);
        assert_eq!(World::from_snapshot(&w.to_snapshot()).unwrap(), w);
        w.players[0].native_tricks.as_mut().unwrap().slots[10].level = 3;
        assert!(World::from_snapshot(&w.to_snapshot()).is_err());
        w.players[0].native_tricks = Some(native_tricks());
        let id = crate::TrickIdentity {
            slot: 10,
            script: 23,
            level: 2,
            equipped_row: Some(82),
        };
        w.players[0].action = Action::Trick {
            id,
            play: crate::trick::Play::new(23, false, 0),
        };
        assert!(World::from_snapshot(&w.to_snapshot()).is_err());
    }

    #[test]
    fn trick_buttons_survive_release_snapshot_and_replay() {
        let mut w = three_on_three(5);
        w.phase = Phase::Live;
        w.take_ball(0);
        let initiating = crate::button::TRICK | crate::button::L2 | crate::button::R2;
        w.step(&[Input::default().with(initiating)]);
        assert!(matches!(w.players[0].action, Action::Trick { .. }));
        let mut restored = World::from_snapshot(&w.to_snapshot()).unwrap();
        assert_eq!(restored.players[0].action_buttons, initiating);
        assert_eq!(restored.checksum(), w.checksum());
        for _ in 0..8 {
            w.step(&[Input::default()]);
            restored.step(&[Input::default()]);
            assert!(matches!(w.players[0].action, Action::Trick { .. }));
            assert_eq!(w.players[0].action_buttons, initiating);
            assert_eq!(restored, w);
            assert_eq!(restored.checksum(), w.checksum());
        }
        // Same pose, level, variant and clock, but a different turbo pair
        // must remain distinct for authoritative playback identity.
        let mut different_pair = w.clone();
        different_pair.players[0].action_buttons =
            crate::button::TRICK | crate::button::L1 | crate::button::R1;
        assert_ne!(different_pair.checksum(), w.checksum());
        assert_eq!(
            World::from_snapshot(&different_pair.to_snapshot()).unwrap(),
            different_pair,
        );
    }

    #[test]
    fn checksum_detects_gameplay_state_without_a_position_change() {
        let w = three_on_three(5);
        for variant in 0..4 {
            let mut changed = w.clone();
            match variant {
                0 => changed.teams[0].meter_fraction = 0.25,
                1 => {
                    changed.rng.next_u32();
                }
                2 => changed.shot_clock += 1.0,
                _ => changed.rules.unlimited_turbo = true,
            }
            assert_ne!(changed.checksum(), w.checksum());
        }
    }
}
