//! Opt-in test against the user's disc: STREET_ISO=/path/to/disc.iso.
//! The sim's script clocks (`street_sim::timing`) are read from the
//! executable: every pass row, shot script, dunk and lay-up row and catch
//! row is walked again here and compared.
use street_assets::{
    iso::Iso,
    scripts::{
        executable_constant, executable_words, script_callback_tick, script_catch_tail,
        script_end_tick, script_fades, script_gate,
    },
};
use street_sim::timing::{
    CATCH_ROWS, CLIP_TAIL, EXTRA_ROWS, LEAP_ROWS, MOVE_ROWS, PASS_ROWS, SHOT_SCRIPTS, airtime,
};

fn elf() -> Option<Vec<u8>> {
    let path = std::env::var_os("STREET_ISO")?;
    let mut iso = Iso::open(path).unwrap();
    Some(iso.read("/SLUS_206.51").unwrap())
}

#[test]
fn the_pass_rows_release_at_their_scripts_callback_38() {
    let Some(elf) = elf() else {
        return;
    };
    for (r, row) in PASS_ROWS.iter().enumerate() {
        let w = executable_words(&elf, 0x3d65e8 + r as u32 * 0x1c, 7).unwrap();
        assert_eq!(row.script, w[0], "row {r} script");
        assert_eq!(row.legend, w[1] as i32 as i16, "row {r} legend");
        assert_eq!(row.min_rating, w[2] as i32 as i16, "row {r} rating");
        assert_eq!(row.min_dist, f32::from_bits(w[3]), "row {r} min");
        assert_eq!(row.max_dist, f32::from_bits(w[4]), "row {r} max");
        assert_eq!(row.flags, w[5], "row {r} flags");
        assert_eq!(u32::from(row.kind), w[6], "row {r} kind");
        let release = script_callback_tick(&elf, row.script, 38)
            .unwrap()
            .unwrap_or(0);
        assert_eq!(u32::from(row.release), release, "row {r} release");
    }
}

#[test]
fn the_shot_scripts_release_at_callback_21_on_the_held_path() {
    let Some(elf) = elf() else {
        return;
    };
    for (k, s) in SHOT_SCRIPTS.iter().enumerate() {
        if k < 27 {
            assert_eq!(
                executable_words(&elf, 0x3d8fd8 + k as u32 * 4, 1).unwrap()[0],
                s.script
            );
        }
        let release = script_callback_tick(&elf, s.script, 21).unwrap();
        assert_eq!(Some(u32::from(s.release)), release, "{} release", s.name);
        let end = script_end_tick(&elf, s.script).unwrap();
        assert_eq!(Some(u32::from(s.end)), end, "{} end", s.name);
        assert_eq!(
            script_fades(&elf, s.script).unwrap(),
            s.fades,
            "{} fades",
            s.name
        );
    }
}

#[test]
fn the_shot_scripts_release_gates_open_by_the_button_or_the_landing() {
    // `86 19` on the held path: callback 19 (`FUN_00174c10`) lets a
    // person's script on once the button is up, or once the actor is
    // within 15 updates of landing (the 26 clips of `0x3d9218`; 5 for
    // any other clip), the landing being `FUN_0019bed8` of the script's
    // own jump. The gate and the tick it opens by itself are the table's.
    let Some(elf) = elf() else {
        return;
    };
    let listed = executable_words(&elf, 0x3d9218, 26).unwrap();
    for s in SHOT_SCRIPTS.iter() {
        let gate = script_gate(&elf, s.script).unwrap();
        assert_eq!(
            gate.map_or(0, |g| g.tick),
            u32::from(s.gate),
            "{} gate",
            s.name
        );
        let latest = match gate {
            Some(g) => match g.jump {
                Some((jump, vy)) => {
                    let cap = if listed.contains(&(g.record as u32)) {
                        15
                    } else {
                        5
                    };
                    (jump + u32::from(airtime(vy, 0.0))).saturating_sub(cap)
                }
                None => 0,
            },
            None => 0,
        };
        assert_eq!(latest, u32::from(s.latest), "{} latest", s.name);
        // Every gated script jumps before its gate (the side fades
        // through callback 18), so every gate opens by itself.
        assert!(s.gate == 0 || s.latest >= s.gate, "{}", s.name);
    }
}

#[test]
fn the_side_fades_jump_through_callback_18() {
    // Game callback 18 (`FUN_00172788`, raw `0x172788..0x1729f4`) is the
    // side fades' jump (`83 12 1`): 15 / fps up (`lui 0x4170` at
    // `0x1727b4`), a drift of 7.2 / fps (`0x1727f8`), and the landing
    // `FUN_0019bed8(g, vy, y)` with the `9f` handler's g (`0x172984`,
    // `0x172994`) stored at actor+0x64, 0 at +0x60. Their gates open 15
    // updates before it for the clips of `0x3d9218` (320, 321), 5 for the
    // others (322, 323).
    let Some(elf) = elf() else {
        return;
    };
    assert_eq!(executable_constant(&elf, 0x1727b4).unwrap(), 0x4170_0000);
    assert_eq!(executable_constant(&elf, 0x1727f8).unwrap(), 0x40e6_6666);
    assert_eq!(executable_constant(&elf, 0x172984).unwrap(), 0x4200_9be3);
    assert_eq!(executable_constant(&elf, 0x172994).unwrap(), 0xbfa1_3ba4);
    // The game callback table (`0x3e86d0`): 18 the jump, 19 the gate, 21
    // the release.
    let table = executable_words(&elf, 0x3e86d0, 22).unwrap();
    assert_eq!(
        (table[18], table[19], table[21]),
        (0x172788, 0x174c10, 0x1729f8)
    );
    let listed = executable_words(&elf, 0x3d9218, 26).unwrap();
    let land = u32::from(airtime(0.25, 0.0));
    for (id, jump, record, cap) in [
        (27, 15, 320, 15),
        (28, 15, 321, 15),
        (29, 10, 322, 5),
        (30, 10, 323, 5),
    ] {
        let s = &SHOT_SCRIPTS[id];
        let g = script_gate(&elf, s.script).unwrap().unwrap();
        assert_eq!(g.jump, Some((jump, 0.25)), "{}", s.name);
        assert_eq!(g.record, record, "{}", s.name);
        assert_eq!(listed.contains(&(record as u32)), cap == 15, "{}", s.name);
        assert_eq!(u32::from(s.latest), jump + land - cap, "{}", s.name);
    }
}

#[test]
fn the_dunk_and_layup_rows_are_the_candidate_table() {
    let Some(elf) = elf() else {
        return;
    };
    for (i, row) in MOVE_ROWS.iter().enumerate() {
        let w = executable_words(&elf, 0x3d7b28 + i as u32 * 0x14, 5).unwrap();
        assert_eq!(row.script, w[0], "row {i}");
        assert_eq!(row.legend, w[1] as i32 as i16, "row {i}");
        assert_eq!(row.mask, w[2], "row {i}");
        assert_eq!(u32::from(row.family), w[3], "row {i}");
        assert_eq!(u32::from(row.id), w[4], "row {i}");
        let release = script_callback_tick(&elf, row.script, 21)
            .unwrap()
            .unwrap_or(0);
        assert_eq!(u32::from(row.release), release, "row {i} {}", row.name);
        let end = script_end_tick(&elf, row.script).unwrap().unwrap_or(0);
        assert_eq!(u32::from(row.end), end, "row {i} {}", row.name);
    }
    // The two-turbo leaps past the table (`0x3d70d0`), clocked alike.
    for (k, row) in LEAP_ROWS.iter().enumerate() {
        assert_eq!(
            executable_words(&elf, 0x3d70d0 + 4 * k as u32, 1).unwrap()[0],
            row.script
        );
        let release = script_callback_tick(&elf, row.script, 21)
            .unwrap()
            .unwrap_or(0);
        let end = script_end_tick(&elf, row.script).unwrap().unwrap_or(0);
        assert_eq!(
            (u32::from(row.release), u32::from(row.end)),
            (release, end),
            "leap {k} {}",
            row.name
        );
    }
    // `layup_3_bboard` (`FUN_0016b4b8`, `lui a1, 0x44; addiu a1, 0x6416`
    // at `0x16bef8` and `0x16c280`, lay-up kind 5), then the ten of
    // `0x3d8f60` `FUN_0016b1e8` draws: the script, a word no code reads
    // (0x14..0x16) and the lay-up kind it writes at +0x26c (`lw v0, 8(s0)`
    // at `0x16b2d4`), 0.
    assert_eq!(EXTRA_ROWS[0].script, 0x446416);
    assert_eq!(EXTRA_ROWS[0].id, 5);
    for (k, row) in EXTRA_ROWS[1..].iter().enumerate() {
        let w = executable_words(&elf, 0x3d8f60 + 12 * k as u32, 3).unwrap();
        assert_eq!(
            (row.script, u32::from(row.id), w[2]),
            (w[0], w[2], 0),
            "{}",
            row.name
        );
        assert!((0x14..=0x16).contains(&w[1]), "{}", row.name);
    }
    for row in &EXTRA_ROWS {
        let release = script_callback_tick(&elf, row.script, 21)
            .unwrap()
            .unwrap_or(0);
        let end = script_end_tick(&elf, row.script).unwrap().unwrap_or(0);
        assert_eq!(
            (u32::from(row.release), u32::from(row.end)),
            (release, end),
            "{}",
            row.name
        );
    }
}

#[test]
fn the_catch_rows_hold_for_their_scripts_tail() {
    let Some(elf) = elf() else {
        return;
    };
    for (i, row) in CATCH_ROWS.iter().enumerate() {
        let w = executable_words(&elf, 0x3d75c8 + i as u32 * 0x14, 5).unwrap();
        assert_eq!(row.script, w[0], "row {i}");
        assert_eq!(row.min_flight, w[1] as i32 as i16, "row {i}");
        assert_eq!(row.flags, w[2], "row {i}");
        assert_eq!(u32::from(row.record), w[3], "row {i}");
        assert_eq!(u32::from(row.frame), w[4], "row {i}");
        let tail = script_catch_tail(&elf, row.script).unwrap();
        let expected = tail.map_or(CLIP_TAIL, |t| t as u8);
        assert_eq!(row.tail, expected, "row {i} {}", row.name);
    }
}

#[test]
fn the_swipe_scripts_are_the_executables() {
    // `0x3d7220`: the low swipes `r & 1` picks (in, out); each script
    // blends five updates (`8e f5 05`) to its mirrored record with the
    // left-handed one as the alternate (the post-up steal: `99` on the post
    // stance, its own pair), sets the swipe bit at its first byte after the
    // opening (`9b 40`), waits to its frame (`8d`) and clears the bit
    // (`9c 40`).
    use street_assets::playback::{self, Op, WaitFrame, WaitTicks};
    use street_sim::swipe::{LOW, SCRIPTS};
    let Some(elf) = elf() else {
        return;
    };
    let low = executable_words(&elf, 0x3d7220, 2).unwrap();
    assert_eq!(low, LOW.map(|k| SCRIPTS[usize::from(k)].address));
    for s in SCRIPTS {
        let program = playback::compile(&elf, s.address, playback::Path::Plain).unwrap();
        let Op::Blend {
            frames,
            start,
            targets,
            ..
        } = &program.ops[0]
        else {
            panic!("{:#x}: {:?}", s.address, program.ops[0]);
        };
        assert_eq!((*frames, *start), (5, 0), "{:#x}", s.address);
        assert_eq!(targets[0].record as u16, s.records[0], "{:#x}", s.address);
        if let Some(alternate) = targets.get(1) {
            assert_eq!(alternate.record as u16, s.records[1], "{:#x}", s.address);
        }
        let wait = program
            .ops
            .iter()
            .find_map(|op| match op {
                Op::Wait { to, ticks } => Some((*to, *ticks)),
                _ => None,
            })
            .unwrap();
        let frame = match wait.0 {
            WaitFrame::Absolute(f) | WaitFrame::Relative(f) => f as f32,
        };
        assert_eq!(frame, s.frame, "{:#x}", s.address);
        assert_eq!(
            wait.1,
            WaitTicks::Fixed(s.updates as u8),
            "{:#x}",
            s.address
        );
        // The bit: `9b 40000000` before the blend, `9c 40000000` after the wait.
        let bytes = executable_words(&elf, s.address & !3, 64).unwrap();
        let body: Vec<u8> = bytes.iter().flat_map(|w| w.to_le_bytes()).collect();
        let body = &body[(s.address & 3) as usize..];
        let set = body.windows(5).position(|b| b == [0x9b, 0x40, 0, 0, 0]);
        let clear = body.windows(5).position(|b| b == [0x9c, 0x40, 0, 0, 0]);
        let blend = body.iter().position(|&b| b == 0x8e);
        assert!(
            set.is_some() && set < blend && clear > blend,
            "{:#x}",
            s.address
        );
    }
    // The post-up steal's second blend (`0x457692`): the left-handed record.
    let alternate = playback::compile(&elf, 0x457692, playback::Path::Plain).unwrap();
    assert!(matches!(&alternate.ops[0], Op::Blend { targets, .. } if targets[0].record == 398));
}

#[test]
fn the_block_jump_lists_and_aims_are_the_executables() {
    // `FUN_0015d3a0`'s lists (`0x3d71f0`, counted at `0x43f840`) name the
    // scripts of `street_sim::block::KINDS`, and every script but the glass
    // pair runs callback 35 on its plain path at the tick the sim aims at.
    use street_sim::block::{Jump, KINDS, SCRIPTS};
    let Some(elf) = elf() else {
        return;
    };
    let lists = executable_words(&elf, 0x3d71f0, 12).unwrap();
    let counts = executable_words(&elf, 0x43f840, 12).unwrap();
    for (k, kind) in KINDS.iter().enumerate() {
        let scripts = executable_words(&elf, lists[k], counts[k] as usize).unwrap();
        let ours: Vec<u32> = kind
            .iter()
            .map(|&s| SCRIPTS[usize::from(s)].address)
            .collect();
        assert_eq!(scripts, ours, "kind {k}");
    }
    for (k, s) in SCRIPTS.iter().enumerate() {
        let jump = Jump {
            script: k as u8,
            branch: false,
            from: None,
            tick: 0,
            word: 0,
            vy: 0.0,
            drift: [0.0; 2],
            up: false,
            meet: 15,
            airtime: 0,
            flown: 0,
            turbo: false,
        };
        let aim = script_callback_tick(&elf, s.address, 35).unwrap();
        if s.glass {
            assert_eq!(aim, None, "{:#x}", s.address);
        } else {
            assert_eq!(aim, Some(u32::from(jump.aim_tick())), "{:#x}", s.address);
        }
    }
}

#[test]
fn the_passout_rows_toss_at_their_scripts_callback_53() {
    // The rows a passout to oneself draws from (flag 0x200000) toss the
    // ball at callback 53 (`FUN_001741a8`); none has a callback 38.
    let Some(elf) = elf() else {
        return;
    };
    for t in street_sim::passout::PASSOUT_TOSSES {
        let row = &PASS_ROWS[usize::from(t.row)];
        assert!(row.flags & 0x200000 != 0, "row {}", t.row);
        let tick = script_callback_tick(&elf, row.script, 53).unwrap();
        assert_eq!(Some(u32::from(t.tick)), tick, "row {}", t.row);
        assert_eq!(
            script_callback_tick(&elf, row.script, 38).unwrap(),
            None,
            "row {}",
            t.row
        );
    }
}

#[test]
fn the_rebound_scripts_clips_and_waits_are_the_executables() {
    // Each rebound jump script (the goaltend pair aside, a block jump's):
    // along callback 47's plain path and its branch (`84 2f` answered) a
    // 4-update blend to the path's record at the script's start frame, then
    // the waits: before the aim (`jump - 4` updates), the aim's frame over
    // the updates callback 36 aimed for, the landing, the tail; the
    // standing catch's one wait over its tail after the blend. Callback 6
    // (`83 06`) carries each path's turn.
    use street_assets::playback::{self, Op, Path, WaitFrame, WaitTicks};
    use street_sim::rebound::SCRIPTS;
    let Some(elf) = elf() else {
        return;
    };
    for (k, s) in SCRIPTS.iter().enumerate() {
        if (14..16).contains(&k) {
            continue;
        }
        for (b, path) in [Path::Plain, Path::Branch].into_iter().enumerate() {
            let program = playback::compile(&elf, s.address, path).unwrap();
            let blend = program.ops.iter().find_map(|op| match op {
                Op::Blend {
                    frames,
                    start,
                    targets,
                    ..
                } => Some((*frames, *start, targets[0].record)),
                _ => None,
            });
            let at = format!("{k} {:#x} path {b}", s.address);
            assert_eq!(
                blend,
                Some((4, i32::from(s.start), i32::from(s.records[b]))),
                "{at}"
            );
            let waits: Vec<(i32, WaitTicks)> = program
                .ops
                .iter()
                .filter_map(|op| match op {
                    Op::Wait {
                        to: WaitFrame::Absolute(f),
                        ticks,
                    } => Some((*f, *ticks)),
                    _ => None,
                })
                .collect();
            if s.jump == 0 {
                assert_eq!(
                    waits,
                    [(i32::from(s.before), WaitTicks::Fixed(s.tail - 4))],
                    "{at}"
                );
                continue;
            }
            assert_eq!(
                waits,
                [
                    (i32::from(s.before), WaitTicks::Fixed(s.jump - 4)),
                    (i32::from(s.frame), WaitTicks::Other),
                    (i32::from(s.land), WaitTicks::Airtime),
                    (i32::from(s.last), WaitTicks::Fixed(s.tail)),
                ],
                "{at}"
            );
        }
        // Callback 6's turns in the script's bytes (to the next script's):
        // `83 06 00 00 00` and its i32.
        let end = SCRIPTS
            .iter()
            .map(|t| t.address)
            .filter(|&a| a > s.address)
            .min()
            .unwrap_or(s.address + 0xc8)
            .min(s.address + 0xc8);
        let bytes = executable_words(&elf, s.address & !3, 64).unwrap();
        let body: Vec<u8> = bytes.iter().flat_map(|w| w.to_le_bytes()).collect();
        let body = &body[(s.address & 3) as usize..(end - (s.address & !3)) as usize];
        let turns: Vec<i16> = body
            .windows(9)
            .filter(|w| w[..5] == [0x83, 6, 0, 0, 0])
            .map(|w| i32::from_le_bytes([w[5], w[6], w[7], w[8]]) as i16)
            .collect();
        match s.turn {
            None => assert!(turns.is_empty(), "{k}"),
            Some([a, b]) if a == b && turns.len() == 1 => assert_eq!(turns, [a], "{k}"),
            Some(both) => assert_eq!(turns, both, "{k}"),
        }
    }
}
