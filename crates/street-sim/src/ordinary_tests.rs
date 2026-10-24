use super::*;
use std::{collections::BTreeMap, fs::File, io::BufReader};
use street_assets::{anim, iso::Iso, ordinary::OrdinaryPrograms, timeline::ScriptBank};

#[path = "ordinary_reference.rs"]
mod reference;

struct Assets {
    programs: OrdinaryPrograms,
    states: BTreeMap<i32, Vec<u32>>,
}

impl Source for Assets {
    fn command(&self, pc: u32) -> Option<Command<'_>> {
        let instruction = self.programs.instruction_at(pc)?;
        let body = self.programs.body_containing(pc)?;
        Some(Command {
            opcode: instruction.opcode,
            next: instruction.next,
            operands: &body.bytes
                [(pc - body.start + 1) as usize..(instruction.next - body.start) as usize],
        })
    }

    fn states(&self, row: i32) -> Option<&[u32]> {
        self.states.get(&row).map(Vec::as_slice)
    }
}

struct Fixture {
    commands: BTreeMap<u32, (u8, Vec<u8>)>,
    states: Vec<u32>,
}

impl Fixture {
    fn new(commands: &[(u8, Option<u32>)]) -> Self {
        let mut pc = 0x4574e2;
        Self {
            commands: commands
                .iter()
                .map(|&(opcode, operand)| {
                    let value = (opcode, operand.map(u32::to_le_bytes).unwrap_or([0; 4]));
                    let operands = if operand.is_some() {
                        value.1.to_vec()
                    } else {
                        Vec::new()
                    };
                    let address = pc;
                    pc += 1 + operands.len() as u32;
                    (address, (opcode, operands))
                })
                .collect(),
            states: (0..56).map(|i| i * 17).collect(),
        }
    }

    fn placement() -> Self {
        // Synthetic operand/control-flow fixture. Actual disc bytes and
        // decoded states are tested independently below.
        Self::new(&[
            (0xa7, None),
            (0x96, None),
            (0x97, None),
            (0x91, Some(0x20000)),
            (0x9b, Some(0)),
            (0x89, Some(65)),
            (0xab, Some(6.0_f32.to_bits())),
            (0x81, None),
        ])
    }
}

impl Source for Fixture {
    fn command(&self, pc: u32) -> Option<Command<'_>> {
        let (opcode, operands) = self.commands.get(&pc)?;
        Some(Command {
            opcode: *opcode,
            next: pc + 1 + operands.len() as u32,
            operands,
        })
    }

    fn states(&self, row: i32) -> Option<&[u32]> {
        (row == 65).then_some(&self.states)
    }
}

fn initial() -> Projection {
    // Deliberately nonzero prior fields demonstrate every reached entry
    // store. These are test inputs, not a claimed native constructor seed.
    Projection {
        actor: Actor {
            position: [91; 4],
            physical: [93; 4],
            elapsed: 17,
            flight: 29,
            flags: 0x60000,
            heading: -11,
            heading_b0: -12,
            state_198: 37,
            state_1a0: 41,
            state_1a4: 43,
            state_1a8: 47,
            state_1b8: 53,
            state_1d4: 59,
            state_2b4: 61,
            placement: [67; 4],
            placement_heading: -13,
        },
        primary: Channel::constructed(),
    }
}

fn authored_position() -> [u32; 4] {
    [12.0_f32, 0.0, -17.0, 1.0].map(f32::to_bits)
}

#[test]
fn ordinary_constructor_preserves_unwritten_numeric_fields() {
    let channel = Channel::constructed();
    assert_eq!(channel.pc, 0);
    assert_eq!(channel.wait, None);
    assert_eq!(channel.step, None);
    assert_eq!(channel.duration, None);
    assert_eq!(channel.counter, None);
    assert_eq!(channel.state_flags, None);
    assert_eq!((channel.row, channel.alternate, channel.link), (-1, -1, -1));
    assert_eq!((channel.integer, channel.target), (-1, -2));
    assert_eq!(channel.phase, (-1.0_f32).to_bits());
}

#[test]
fn ordinary_placement_reaches_real_entry_shape_and_outside_update_wait() {
    let (source, mut state, mut rng) = (Fixture::placement(), initial(), Rng::new(197));
    state.primary.flags = 0x020c_0000;
    state.primary.configured_step = 0.375_f32.to_bits();
    let stream = rng.clone();
    let effects = place(&mut state, &mut rng, &source, authored_position(), 731).unwrap();
    assert_eq!(rng, stream);
    assert_eq!(state.actor.position, authored_position());
    assert_eq!(state.actor.placement, authored_position());
    assert_eq!(state.actor.physical, [0; 4]);
    assert_eq!((state.actor.elapsed, state.actor.flight), (0, 1));
    assert_eq!(state.actor.flags, 0);
    assert_eq!(state.actor.heading, 731);
    assert_eq!(state.actor.heading_b0, 731);
    assert_eq!(state.actor.placement_heading, 731);
    assert_eq!((state.actor.state_198, state.actor.state_1a4), (0, 0));
    assert_eq!(state.actor.state_1b8, -1);
    assert_eq!(state.actor.state_2b4, 1);
    assert_eq!(state.actor.state_1a0, 0);
    assert_eq!(state.actor.state_1a8, 0);
    assert_eq!(state.actor.state_1d4, 0);
    assert_eq!(state.primary.flags, 0x2008);
    assert_eq!(state.primary.row, 65);
    assert_eq!(state.primary.pc, 0x4574f9);
    assert_eq!(state.primary.wait, Some(2));
    assert_eq!(state.primary.phase, (1.0_f32 / 6.0).to_bits());
    assert_eq!(state.primary.step, Some(0x3e2a_aaab));
    assert_eq!(state.primary.configured_step, 0.375_f32.to_bits());
    assert_eq!(effects.len(), 2);
    assert_eq!(effects[0], Effect::SelectRow(65));
    assert_eq!(state.primary.state_flags, Some([0; 2]));
    let before = state.clone();
    assert!(
        update_primary(&mut state, &mut rng, &source)
            .unwrap()
            .is_empty()
    );
    assert_eq!(state.primary.wait, Some(1));
    assert_eq!(state.primary.phase, before.primary.phase);
    for _ in 0..19 {
        assert_eq!(
            update_primary(&mut state, &mut rng, &source).unwrap().len(),
            1
        );
        assert_eq!(state.primary.wait, Some(1));
        assert_eq!(state.primary.pc, 0x4574f9); // active loop never dispatches81
    }
    assert_eq!(rng, stream);
}

#[test]
fn ordinary_unsupported_entry_rolls_back_prior_candidate_stores_and_rng() {
    for opcode in [0x8a, 0x8e, 0x84, 0x81, 0xff] {
        let source = Fixture::new(&[(0xa7, None), (0x96, None), (opcode, None)]);
        let (mut state, mut rng) = (initial(), Rng::new(29));
        let before = (state.clone(), rng.clone());
        assert_eq!(
            place(&mut state, &mut rng, &source, authored_position(), 31),
            Err(Unsupported::Opcode {
                pc: 0x4574e4,
                opcode
            })
        );
        assert_eq!((state, rng), before);
    }
    let (source, mut state, mut rng) = (Fixture::placement(), initial(), Rng::new(29));
    let before = (state.clone(), rng.clone());
    let mut airborne = authored_position();
    airborne[1] = 1.0_f32.to_bits();
    assert_eq!(
        place(&mut state, &mut rng, &source, airborne, 31),
        Err(Unsupported::GroundedReset)
    );
    assert_eq!((state, rng), before);
}

#[test]
fn ordinary_clock_rejects_transitions_before_committing_any_scalar() {
    let (source, mut start, mut rng) = (Fixture::placement(), initial(), Rng::new(3));
    place(&mut start, &mut rng, &source, authored_position(), 0).unwrap();
    start.primary.wait = Some(1);
    for mutation in 0..14 {
        let mut state = start.clone();
        match mutation {
            0 => state.primary.flags |= 0x200000,
            1 => state.primary.flags |= 0x100,
            2 => state.primary.flags |= 0x1000000,
            3 => state.primary.flags |= 0x10000,
            4 => state.primary.alternate = 54,
            5 => state.primary.link = 1,
            6 => state.primary.target = 0,
            7 => state.primary.duration = Some(6),
            8 => state.primary.counter = Some(0),
            9 => state.primary.held_offset = 1,
            10 => state.primary.step = None,
            11 => state.primary.step = Some(f32::NAN.to_bits()),
            12 => state.primary.phase = f32::INFINITY.to_bits(),
            13 => state.primary.phase = 54.875_f32.to_bits(),
            _ => unreachable!(),
        }
        let before = (state.clone(), rng.clone());
        assert!(update_primary(&mut state, &mut rng, &source).is_err());
        assert_eq!((state, rng.clone()), before, "mutation {mutation}");
    }
    let mut paused = start;
    paused.primary.flags |= 1;
    let before = paused.clone();
    assert!(
        update_primary(&mut paused, &mut rng, &source)
            .unwrap()
            .is_empty()
    );
    assert_eq!(paused, before);
}

fn disc() -> Option<(Vec<u8>, Assets)> {
    let mut iso: Iso<BufReader<File>> = Iso::open(std::env::var_os("STREET_ISO")?).unwrap();
    let data = iso.read("/SLUS_206.51").unwrap();
    let programs = OrdinaryPrograms::from_executable(&data).unwrap();
    let scripts = ScriptBank::read(&mut iso).unwrap();
    let name = scripts.animations[65].name.as_ref().unwrap();
    assert_eq!(name, "breathe_1");
    let mut states = None;
    for path in anim::BANKS {
        let bank = anim::load(&mut iso, path).unwrap();
        if bank.animations().any(|candidate| candidate == name) {
            let decoded = bank.states(name).unwrap().unwrap();
            assert_eq!(bank.rotations(name).unwrap().unwrap().frames.len(), 56);
            assert_eq!(decoded.len(), 56);
            states = Some(decoded);
            break;
        }
    }
    Some((
        data,
        Assets {
            programs,
            states: BTreeMap::from([(65, states.unwrap())]),
        },
    ))
}

#[test]
fn ordinary_actual_disc_placement_matches_original_opcode_projection() {
    let Some((elf, source)) = disc() else { return };
    let (mut state, mut rng) = (initial(), Rng::new(71));
    state.primary.flags = 0x020c_0000;
    state.primary.configured_step = 0.375_f32.to_bits();
    let mut original = reference::Machine::new(&elf, &state, source.states(65).unwrap());
    let expected = original.place(authored_position(), 731);
    let effects = place(&mut state, &mut rng, &source, authored_position(), 731).unwrap();
    assert_eq!(state, expected);
    assert_eq!(effects.len(), 2);
    for index in 0..240 {
        let expected = original.update();
        update_primary(&mut state, &mut rng, &source).unwrap();
        assert_eq!(state, expected, "source scalar projection update {index}");
    }
    assert_eq!(rng, Rng::new(71));
    // Actual idle first needs variable row-2000, then random8A. Reject the
    // full transaction after prior A7/96/97/91 stores without selecting an
    // invented row or consuming the later random draw.
    let before = (state.clone(), rng.clone());
    let result = transaction(&mut state, &mut rng, |state, rng| {
        install(state, rng, &source, street_assets::ordinary::IDLE_SCRIPT)
    });
    assert_eq!(result, Err(Unsupported::DynamicRow(-2000)));
    assert_eq!((state, rng), before);
}

#[test]
fn ordinary_original_add_words_distinguish_chop_and_nearest() {
    let Some((elf, source)) = disc() else { return };
    let (mut start, mut rng) = (initial(), Rng::new(1));
    place(&mut start, &mut rng, &source, authored_position(), 0).unwrap();
    let mut compared = 0;
    let mut differs_from_nearest = 0;
    for phase in [0.0_f32, 0.166_666_67, 1.25, 17.125, 32.5, 53.25] {
        for step in [1.0_f32 / 6.0, 0.0125, 0.125, 0.375, 1.0] {
            let mut state = start.clone();
            state.primary.phase = phase.to_bits();
            state.primary.integer = phase as i32;
            state.primary.step = Some(step.to_bits());
            state.primary.wait = Some(1);
            let mut original = reference::Machine::new(&elf, &state, source.states(65).unwrap());
            let expected = original.update();
            update_primary(&mut state, &mut rng, &source).unwrap();
            assert_eq!(state, expected);
            differs_from_nearest += usize::from(state.primary.phase != (phase + step).to_bits());
            compared += 1;
        }
    }
    assert_eq!(compared, 30);
    assert!(differs_from_nearest > 0);
}

#[test]
fn native_heading_matches_known_formation_headings() {
    let expected_team0 = [
        // (team, slot, heading, pos, tgt)
        (0, 0, 865, [-47.0, 0.0, 16.0, 1.0], [-35.0, 0.0, 8.0, 1.0]),
        (1, 0, 284, [-12.0, 0.0, 10.0, 1.0], [-47.0, 0.0, 16.0, 1.0]),
        (0, 1, 427, [-32.0, 0.0, -10.0, 1.0], [-47.0, 0.0, 16.0, 1.0]),
        (1, 1, 323, [13.0, 0.0, -10.0, 1.0], [-47.0, 0.0, 16.0, 1.0]),
        (0, 2, 352, [-35.0, 0.0, 8.0, 1.0], [-47.0, 0.0, 16.0, 1.0]),
        (1, 2, 282, [-22.0, 0.0, 12.0, 1.0], [-47.0, 0.0, 16.0, 1.0]),
    ];

    for (team, slot, exp_heading, exp_pos, exp_tgt) in expected_team0 {
        let spot = formation_spot(0, team, slot);
        assert_eq!(
            spot.heading, exp_heading,
            "team 0 possession, team {team} slot {slot}"
        );
        assert_eq!(spot.position, exp_pos);
        assert_eq!(spot.target, exp_tgt);
    }

    let expected_team1 = [
        (0, 0, 741, [12.0, 0.0, 10.0, 1.0], [47.0, 0.0, 16.0, 1.0]),
        (1, 0, 160, [47.0, 0.0, 16.0, 1.0], [35.0, 0.0, 8.0, 1.0]),
        (0, 1, 702, [-13.0, 0.0, -10.0, 1.0], [47.0, 0.0, 16.0, 1.0]),
        (1, 1, 598, [32.0, 0.0, -10.0, 1.0], [47.0, 0.0, 16.0, 1.0]),
        (0, 2, 743, [22.0, 0.0, 12.0, 1.0], [47.0, 0.0, 16.0, 1.0]),
        (1, 2, 673, [35.0, 0.0, 8.0, 1.0], [47.0, 0.0, 16.0, 1.0]),
    ];

    for (team, slot, exp_heading, exp_pos, exp_tgt) in expected_team1 {
        let spot = formation_spot(1, team, slot);
        assert_eq!(
            spot.heading, exp_heading,
            "team 1 possession, team {team} slot {slot}"
        );
        assert_eq!(spot.position, exp_pos);
        assert_eq!(spot.target, exp_tgt);
    }
}

#[test]
fn place_restart_resets_all_six_actors_in_interleaved_order() {
    let source = Fixture::placement();

    // Test possessing team 0
    let mut projections0 = std::array::from_fn(|_| initial());
    let mut rng0 = Rng::new(101);
    let ball_assignee0 = place_restart(&mut projections0, &mut rng0, &source, 0).unwrap();
    assert_eq!(ball_assignee0, 0);

    for (actor_idx, team, slot, spot) in restart_order(0) {
        let actor = &projections0[actor_idx].actor;
        let expected_pos = spot.position.map(f32::to_bits);
        assert_eq!(actor.placement, expected_pos);
        assert_eq!(actor.placement_heading, spot.heading);
        assert_eq!(actor.position, expected_pos);
        assert_eq!(actor.heading, spot.heading);
        assert_eq!(actor.heading_b0, spot.heading);
        assert_eq!(actor.flags, 0);
        assert_eq!(actor.physical, [0; 4]);
        assert_eq!(actor.state_1b8, -1);
        assert_eq!(actor.state_2b4, 1);
        assert_eq!(actor.state_1a0, 0);
        assert_eq!(actor.state_1a8, 0);
        assert_eq!(actor.state_1d4, 0);
        assert_eq!(actor_idx, team * 3 + slot);
    }

    // Test possessing team 1
    let mut projections1 = std::array::from_fn(|_| initial());
    let mut rng1 = Rng::new(202);
    let ball_assignee1 = place_restart(&mut projections1, &mut rng1, &source, 1).unwrap();
    assert_eq!(ball_assignee1, 3);

    for (actor_idx, team, slot, spot) in restart_order(1) {
        let actor = &projections1[actor_idx].actor;
        let expected_pos = spot.position.map(f32::to_bits);
        assert_eq!(actor.placement, expected_pos);
        assert_eq!(actor.placement_heading, spot.heading);
        assert_eq!(actor.position, expected_pos);
        assert_eq!(actor.heading, spot.heading);
        assert_eq!(actor.heading_b0, spot.heading);
        assert_eq!(actor_idx, team * 3 + slot);
    }
}

#[test]
fn ordinary_gait_channel_row_selection_and_stepping() {
    assert_eq!(select_gait_row(true, true), HELD_GAIT_ROW);
    assert_eq!(select_gait_row(false, true), UNHELD_GAIT_ROW);
    assert_eq!(select_gait_row(false, false), PLACEMENT_ROW);
    assert_eq!(select_gait_row(true, false), PLACEMENT_ROW);

    // Frame step calculation
    assert_eq!(calculate_gait_frame_step(0.0, 1.0, 1.0), 1.0 / 6.0);
    assert_eq!(calculate_gait_frame_step(0.366667, 0.0, 1.0), 1.0 / 6.0);
    // G04: speed arrives in court units per second; the native formula
    // divides one update's displacement by the root distance.
    let step = calculate_gait_frame_step(0.366667, 0.366667 * 60.0, 1.0);
    assert!((step - 1.0).abs() < 1e-4);
    let step = calculate_gait_frame_step(0.590390, 18.0, 1.0);
    assert!((step - 0.508_138_7).abs() < 1e-5, "{step}");

    let mut channel = PlayerGaitChannel::default();
    assert_eq!(channel.row, PLACEMENT_ROW);
    assert_eq!(channel.blend_counter, 0);

    let mut pos = [0.0, 0.0];
    let heading = 0.0_f32; // faces +z

    // Start moving unheld
    channel.update(false, true, 6.0, &mut pos, [heading; 2], 1.0);
    assert_eq!(channel.row, UNHELD_GAIT_ROW);
    assert_eq!(channel.blend_counter, 6);
    assert!(channel.phase > 0.0);
    assert!(pos[1] > 0.0); // moved in +z direction

    // Continue moving unheld
    channel.update(false, true, 6.0, &mut pos, [heading; 2], 1.0);
    assert_eq!(channel.row, UNHELD_GAIT_ROW);
    assert_eq!(channel.blend_counter, 5);

    // Switch to held
    channel.update(true, true, 6.0, &mut pos, [heading; 2], 1.0);
    assert_eq!(channel.row, HELD_GAIT_ROW);
    assert_eq!(channel.blend_counter, 6);
    assert_eq!(channel.phase, channel.frame_step);

    // Stop moving
    channel.update(true, false, 0.0, &mut pos, [heading; 2], 1.0);
    assert_eq!(channel.row, PLACEMENT_ROW);
    assert_eq!(channel.blend_counter, 6);
}
