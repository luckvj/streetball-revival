//! Static parity checks against the shipped program (STREET_ISO=/path/to/
//! the disc). Every expected value is read from `SLUS_206.51` itself, its
//! tables or the instructions that build a constant, then compared with the
//! rewrite's production path. Without the disc the tests return early.
use super::*;
use street_assets::scripts::{executable_constant, executable_words};

fn executable() -> Option<Vec<u8>> {
    let path = std::env::var_os("STREET_ISO")?;
    Some(
        street_assets::iso::Iso::open(path)
            .unwrap()
            .read("/SLUS_206.51")
            .unwrap(),
    )
}

fn words(elf: &[u8], address: u32, count: usize) -> Vec<u32> {
    executable_words(elf, address, count).unwrap()
}

fn float(elf: &[u8], address: u32) -> f32 {
    f32::from_bits(executable_constant(elf, address).unwrap())
}

/// A three-on-three with player 0 holding the ball mid-trick.
fn tricking(identity: crate::player::TrickIdentity) -> World {
    let mut w = three_on_three(5);
    w.players[0].ratings.handles = 1.0;
    w.players[0].action = Action::Trick {
        id: identity,
        play: crate::trick::Play::new(identity.script, false, 0),
    };
    w.phase = Phase::Live;
    w.take_ball(0);
    w
}

#[test]
fn trick_awards_follow_fun_001438b8() {
    let Some(elf) = executable() else {
        return;
    };
    // 0x1438bc..0x1438d0: the ladder at 0x43df20 indexed by actor+0x248.
    assert_eq!(executable_constant(&elf, 0x1438bc).unwrap(), 0x44 << 16);
    let ladder = words(&elf, 0x43df20, 5);
    assert_eq!(
        ladder,
        street::points::TRICK.to_vec(),
        "the production ladder is the table"
    );
    // The repeat discount multiplies by the 0.5 built at 0x143914 and rounds
    // up (0x14397c..0x1439a0), then clamps to 1 (0x1439ac..0x1439b8).
    let half = float(&elf, 0x143914);
    let award = |base: u32, repeats: usize| {
        let scaled = base as f32 * (0..repeats).fold(1.0f32, |f, _| f * half);
        let mut whole = scaled as i32 as f32;
        if whole < scaled {
            whole += 1.0;
        }
        (whole as i32).max(1) as u32
    };
    for class in 0..5u8 {
        let identity = crate::player::TrickIdentity {
            slot: 0,
            script: 40 + u16::from(class),
            level: class,
            equipped_row: None,
        };
        let key = i32::from(identity.script);
        for qualified in [false, true] {
            // actor+0x218 == 0 halves the base with `sra` (0x1438e4..0x1438f0).
            let base = if qualified {
                ladder[usize::from(class)]
            } else {
                ((ladder[usize::from(class)] as i32) >> 1) as u32
            };
            for repeats in 0..=4 {
                let mut w = tricking(identity);
                let older = [7, 8, 9, 10];
                for (k, slot) in w.teams[0].recent_moves.iter_mut().enumerate() {
                    *slot = if k < repeats { key } else { older[k] };
                }
                let before = w.teams[0].recent_moves;
                // A fakeout (actor+0x218) qualifies the full base.
                w.score_trick(0, identity, qualified.then_some(3));
                // Classes 0 and 1 skip the ring (0x1438f8..0x1438fc).
                let expected = if class < 2 {
                    base
                } else {
                    award(base, repeats)
                };
                assert_eq!(
                    w.teams[0].trick_points, expected,
                    "class {class}, qualified {qualified}, {repeats} repeats"
                );
                let after = w.teams[0].recent_moves;
                if class < 2 {
                    assert_eq!(after, before);
                } else {
                    // 0x143944..0x143974: three shifts, then the new id.
                    assert_eq!(after, [key, before[0], before[1], before[2]]);
                }
            }
        }
    }
}

#[test]
fn combo_tables_and_matcher_follow_fun_00131970() {
    let Some(elf) = executable() else {
        return;
    };
    let weights = words(&elf, 0x43be58, combo::WEIGHT_ROWS * 5);
    for (row, values) in combo::WEIGHTS.iter().enumerate() {
        assert_eq!(
            values.map(f32::to_bits).to_vec(),
            weights[row * 5..row * 5 + 5],
            "weight row {row}"
        );
    }
    let patterns = words(&elf, 0x43c500, 55 * 12);
    for (row, kinds) in combo::PATTERNS.iter().enumerate() {
        let native = &patterns[row * 12..row * 12 + 12];
        assert_eq!(
            kinds.map(|k| i32::from(k) as u32).to_vec(),
            native[..6],
            "pattern row {row}"
        );
        assert!(native[6..].iter().all(|&e| e == u32::MAX), "extras {row}");
    }
    assert_eq!(words(&elf, 0x3d0708, 1)[0], combo::DOUBLE_SWIPE_FLAG);
    // The constructor FUN_001313d0 (0x131468..0x131500) by its own walk.
    let newest = |row: usize| patterns[row * 12 + 5] as i32;
    for class in 0..22 {
        let first = if class == 0 {
            Some(0)
        } else {
            (1..55).find(|&row| newest(row) == class)
        };
        let expected = first.map_or((0, 0), |first| {
            let count = (first..55).take_while(|&r| newest(r) == class).count();
            (first, count)
        });
        assert_eq!(combo::pattern_block(class), expected, "class {class}");
    }

    // Through the live scorer: a basket with a trick under it pays the
    // two-event row at column 0, the weights read from the table.
    let weight = |row: usize, column: usize| f32::from_bits(weights[row * 5 + column]);
    let mut w = face_off(1);
    let now = w.scoring_now();
    w.combo.own(0);
    w.combo.push(combo::Slot::new(10, 0, now, 500), false);
    let before = w.teams[0].trick_points;
    w.score_basket(0, Basket::Jumper { long: false });
    let chain = (250.0 * weight(4, 0) + 500.0 * weight(27, 0)) as u32;
    assert_eq!(w.teams[0].trick_points - before, 250 + chain);
    // A lone basket: the class-4 rows all ask for two events, no payout.
    let mut w = face_off(1);
    let before = w.teams[0].trick_points;
    w.score_basket(0, Basket::Jumper { long: false });
    assert_eq!(w.teams[0].trick_points - before, 250);
    // An alley-oop with extra 1 reads row 46.
    let mut c = combo::Chain::default();
    c.push(combo::Slot::new(10, 1, 0, 1_000));
    c.push(combo::Slot::new(1, 1, 0, 3_000));
    assert_eq!(
        c.total(0),
        (3_000.0 * weight(46, 0) + 1_000.0 * weight(28, 0)) as i32
    );
}

#[test]
fn meter_blend_and_cap_follow_fun_00143458_and_fun_00143d38() {
    let Some(elf) = executable() else {
        return;
    };
    let f = |a| float(&elf, a);
    // Easy (0x1434d0..0x1434f0), Medium (0x1434f4..0x143534), Hard
    // (0x143538..0x143558): {human mix, cpu mix, outer weight}.
    let mixes = |first: f32, second: f32| {
        [
            (first, first * f(0x1434d0) + second * f(0x1434d8), 0.0),
            (
                first * f(0x1434f4) + second * f(0x1434fc),
                first * f(0x143504) + second * f(0x143510),
                f(0x143520),
            ),
            (
                first * f(0x143538) + second * f(0x143540),
                second,
                f(0x14354c),
            ),
        ]
    };
    let rows = words(&elf, 0x43e298, 8)
        .into_iter()
        .map(f32::from_bits)
        .collect::<Vec<_>>();
    for (d, difficulty) in [Difficulty::Easy, Difficulty::Medium, Difficulty::Hard]
        .into_iter()
        .enumerate()
    {
        for controlled in [[true, false], [false, true], [true, true], [false, false]] {
            let expected = |pair: usize| {
                controlled.map(|human| {
                    // FUN_00143248: row 0 for a crew-controlled side, row 8
                    // bytes on for the other; the pair averaged when no side
                    // is controlled.
                    let at = pair * 4 + if human { 0 } else { 2 };
                    let (first, second) = (rows[at], rows[at + 1]);
                    if controlled.contains(&true) {
                        let (h, c, w) = mixes(first, second)[d];
                        (1.0 - w) * h + w * c
                    } else {
                        (first + second) * 0.5
                    }
                })
            };
            let mut w = World::new(
                hoops(),
                vec![],
                Rules {
                    difficulty,
                    ..Default::default()
                },
                1,
            );
            w.configure_meter_fill(controlled, None);
            assert_eq!(
                [w.teams[0].meter_fill, w.teams[1].meter_fill].map(f32::to_bits),
                expected(0).map(f32::to_bits),
                "{difficulty:?} {controlled:?}"
            );
            assert_eq!(
                [
                    w.teams[0].meter_sequence_fill,
                    w.teams[1].meter_sequence_fill
                ]
                .map(f32::to_bits),
                expected(1).map(f32::to_bits)
            );
        }
    }
    // League Challenge progress weights at 0x43df08, and the completed-region
    // Medium weight 0x3f35c28f (0x1437b4..0x1437b8).
    let odds = words(&elf, 0x43df08, 6);
    for (beaten, &word) in odds.iter().enumerate() {
        assert_eq!(
            meter::league_weight(Difficulty::Hard, false, beaten).to_bits(),
            word
        );
    }
    assert_eq!(
        meter::league_weight(Difficulty::Medium, true, 5).to_bits(),
        executable_constant(&elf, 0x1437b4).unwrap()
    );
    // The Easy cap (0x143e54): a CPU crew one point up stays there.
    let cap = f(0x143e54);
    assert_eq!(meter::EASY_CPU_CAP.to_bits(), cap.to_bits());
    let rules = Rules {
        difficulty: Difficulty::Easy,
        ..Default::default()
    };
    let mut w = World::new(hoops(), vec![], rules, 1);
    w.configure_meter_fill([true, false], None);
    w.teams[1].meter = 40_000;
    w.award(1, 50_000, false);
    assert_eq!(w.teams[1].meter as f32 + w.teams[1].meter_fraction, cap);
}

#[test]
fn turbo_rates_are_the_static_initializer_divisions() {
    let Some(elf) = executable() else {
        return;
    };
    // f21 = 1.0 (0x1788b8), the drain 1 / (fps × 5.0) and the fill
    // 1 / (fps × 7.5) (0x178a64, 0x178ad4) stored at 0x4b99a0 and 0x4b99a4.
    let one = float(&elf, 0x1788b8);
    let (drain, fill) = (float(&elf, 0x178a64), float(&elf, 0x178ad4));
    assert_eq!(tuning::TURBO_DRAIN_SECONDS, drain);
    assert_eq!(tuning::TURBO_REFILL_SECONDS, fill);
    let code = words(&elf, 0x1589a0, 12);
    let jal = |target: u32| 0x0c00_0000 | target >> 2;
    // 0x1589a0: jal FUN_00190c08 (spend) with lwc1 f12, -0x6660(v1);
    // 0x1589c8: jal FUN_00190c68 (add) with lwc1 f12, -0x665c(v1).
    assert_eq!(code[0], jal(0x190c08));
    assert_eq!(code[1] & 0xffff, 0x99a0);
    assert_eq!(code[10], jal(0x190c68));
    assert_eq!(code[11] & 0xffff, 0x99a4);
    let fps = TICK_HZ as f32;
    let (per_drain, per_fill) = (one / (fps * drain), one / (fps * fill));

    // Through World::step: one update of held turbo while running, then one
    // with nothing held.
    let mut w = solo([0.0, 0.0]);
    w.set_turbo(0, 0.5);
    w.step(&[Input::moving([1.0, 0.0]).with(button::L1)]);
    assert_eq!(w.players[0].turbo, 0.5 - per_drain);
    let turbo = w.players[0].turbo;
    w.step(&[Input::default()]);
    assert_eq!(w.players[0].turbo, turbo + per_fill);
}

#[test]
fn shot_duration_uses_the_launcher_constants() {
    let Some(elf) = executable() else {
        return;
    };
    // FUN_001505a0: the release height test against 14.0 (0x150754, a lone
    // lui) and the floor DAT_004b849c = fps × 0.5 frames (0x153d50 builds
    // the 0.5, 0x153e34..0x153e40 multiplies and stores it).
    let apex = float(&elf, 0x150754);
    let minimum = float(&elf, 0x153d50);
    assert_eq!((apex, minimum), (14.0, 0.5));
    let rim = [-36.95, 11.75, 0.0];
    for (height, angle) in [(6.0f32, 0.2f32), (9.0, 0.6), (13.5, 1.1), (15.0, 0.3)] {
        let from = [-30.0, height, 2.0];
        let dx = rim[0] - from[0];
        let dz = rim[2] - from[2];
        let range = libm::sqrtf(dx * dx + dz * dz);
        let rise = rim[1] - from[1];
        let vertical = range * libm::tanf(angle) - rise;
        let mut time = if vertical > 0.0 {
            libm::sqrtf(2.0 * vertical / GRAVITY)
        } else {
            0.0
        };
        let mut floor = minimum;
        if from[1] < apex {
            let climb = apex - from[1];
            let fall =
                libm::sqrtf(2.0 * climb / GRAVITY) + libm::sqrtf(2.0 * (climb - rise) / GRAVITY);
            if floor < fall {
                floor = fall;
            }
        }
        if time < floor {
            time = floor;
        }
        assert_eq!(crate::ball::shot_flight_time(from, rim, angle), time);
    }
}

#[test]
fn meter_hold_and_decay_follow_fun_00143c20() {
    let Some(elf) = executable() else {
        return;
    };
    use crate::street::{METER_DECAY, METER_HOLD, MeterClock};
    // 0x144c34: fps (the timing object's 60) × 30 into 0x4b7b14; the
    // 50,000 store of `FUN_00143d38` (0x144014) sets the hold from it.
    assert_eq!(executable_constant(&elf, 0x144c34).unwrap(), 30);
    assert_eq!(METER_HOLD, 30 * TICK_HZ as i32);
    // 0x142ea0 / 0x142ec0: 50000.0 / (fps × 600.0) into 0x4b7990.
    let (six_hundred, full) = (float(&elf, 0x142ea0), float(&elf, 0x142ec0));
    assert_eq!((six_hundred, full), (600.0, 50_000.0));
    assert_eq!(METER_DECAY, full / (TICK_HZ as f32 * six_hundred));
    // The drain table 0x43e248 behind the steal, block and counter drains,
    // and the shot clock's -12,500 (0x14428c, `FUN_00144260`).
    let drains = words(&elf, 0x43e248, 20);
    assert_eq!(drains[7], street::points::BLOCK);
    assert_eq!(drains[8], street::points::STEAL);
    assert_eq!(drains[16], street::points::COUNTER);
    assert_eq!(
        f32::from_bits(executable_constant(&elf, 0x14428c).unwrap()),
        -12_500.0
    );
    // Slot 64 (0x144708): hold 1 (0x144738) and the level compared with 2
    // (0x144744) before the meter is emptied.
    assert_eq!(executable_constant(&elf, 0x144738).unwrap(), 1);
    assert_eq!(executable_constant(&elf, 0x144744).unwrap(), 2);

    // Through World::step: one live update takes the decay times the fill;
    // a full meter's hold runs out after exactly METER_HOLD updates.
    let mut w = face_off(2);
    w.rules.shot_clock = None;
    w.meter_clock = MeterClock::On;
    w.award(0, 20_000, false);
    let before = w.teams[0].meter as f32 + w.teams[0].meter_fraction;
    w.step(&[Input::default(), Input::default()]);
    let after = w.teams[0].meter as f32 + w.teams[0].meter_fraction;
    assert_eq!(after, before - METER_DECAY * 1.0 * 1.0);
    w.award(0, 50_000, false);
    for _ in 0..METER_HOLD - 1 {
        w.step(&[Input::default(), Input::default()]);
    }
    assert!(w.teams[0].meter_full());
    w.step(&[Input::default(), Input::default()]);
    assert_eq!(w.teams[0].meter, 0);
}
