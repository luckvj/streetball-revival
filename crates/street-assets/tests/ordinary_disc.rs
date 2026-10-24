//! Source bytecode/idle prerequisites. Requires the user's STREET_ISO;
//! no captured actor state or original game payload is shipped here.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::BufReader,
};
use street_assets::{
    anim, gait,
    iso::Iso,
    ordinary::{EntryKind, OrdinaryPrograms},
    timeline::ScriptBank,
};

fn disc() -> Option<Iso<BufReader<File>>> {
    Some(Iso::open(std::env::var_os("STREET_ISO")?).unwrap())
}

fn source(data: &[u8], address: u32, length: usize) -> &[u8] {
    let word = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
    let table = word(28) as usize;
    let stride = u16::from_le_bytes(data[42..44].try_into().unwrap()) as usize;
    let count = u16::from_le_bytes(data[44..46].try_into().unwrap()) as usize;
    for i in 0..count {
        let at = table + i * stride;
        if word(at) == 1
            && let Some(delta) = address.checked_sub(word(at + 8))
            && delta as usize + length <= word(at + 16) as usize
        {
            let offset = word(at + 4) as usize + delta as usize;
            return &data[offset..offset + length];
        }
    }
    panic!("unmapped original source {address:#x}");
}

fn source_word(data: &[u8], address: u32) -> u32 {
    u32::from_le_bytes(source(data, address, 4).try_into().unwrap())
}

#[test]
fn ordinary_source_closure_keeps_every_byte_and_static_edge() {
    let Some(mut iso) = disc() else { return };
    let data = iso.read("/SLUS_206.51").unwrap();
    let programs = OrdinaryPrograms::from_executable(&data).unwrap();
    let gait = gait::GaitData::from_executable(&data).unwrap();
    assert_eq!(programs.entries.len(), 110);
    assert_eq!(programs.entries[0].kind, EntryKind::Idle);
    assert_eq!(programs.entries[1].kind, EntryKind::Placement);
    for (i, entry) in programs.entries[2..].iter().enumerate() {
        assert_eq!(entry.kind, EntryKind::Gait(i as u16));
        assert_eq!(entry.address, gait.records[i].script);
    }
    assert_eq!(programs.bodies.len(), 93);
    assert_eq!(programs.instructions.len(), 1110);
    assert_eq!(
        programs.bodies.iter().map(|b| b.bytes.len()).sum::<usize>(),
        8811
    );
    let starts: BTreeSet<_> = (0..632)
        .map(|i| source_word(&data, 0x3e7240 + i * 8))
        .collect();
    for body in &programs.bodies {
        assert!(starts.contains(&body.start));
        assert_eq!(starts.range((body.start + 1)..).next(), Some(&body.end));
        assert_eq!(
            body.bytes,
            source(&data, body.start, (body.end - body.start) as usize)
        );
    }
    let mut opcodes = BTreeMap::new();
    let mut callbacks = BTreeMap::new();
    let mut cross_body = 0;
    for instruction in &programs.instructions {
        *opcodes.entry(instruction.opcode).or_insert(0) += 1;
        assert_eq!(source(&data, instruction.address, 1)[0], instruction.opcode);
        let body = programs.body_containing(instruction.address).unwrap();
        assert!(instruction.next <= body.end);
        let next = programs
            .instructions
            .iter()
            .find(|i| i.address > instruction.address);
        assert!(next.is_none_or(|i| instruction.next <= i.address));
        for target in instruction
            .branches
            .iter()
            .copied()
            .chain(instruction.fallthrough.then_some(instruction.next))
        {
            assert!(programs.instruction_at(target).is_some());
            cross_body += usize::from(target < body.start || target >= body.end);
        }
        if let Some(callback) = instruction.callback {
            assert_eq!(instruction.opcode, 0x84);
            assert_eq!(callback.id, source_word(&data, instruction.address + 1));
            for (i, argument) in callback.arguments.into_iter().enumerate() {
                assert_eq!(
                    argument,
                    source_word(&data, instruction.address + 5 + i as u32 * 4)
                );
            }
            assert_eq!(
                callback.native_function,
                source_word(&data, 0x3e86d0 + callback.id * 4)
            );
            assert_eq!(
                instruction.branches,
                [source_word(&data, instruction.address + 17)]
            );
            *callbacks.entry(callback.id).or_insert(0) += 1;
        }
    }
    assert_eq!(cross_body, 226);
    assert_eq!(callbacks, BTreeMap::from([(15, 48), (22, 3), (52, 86)]));
    assert_eq!(
        opcodes,
        BTreeMap::from([
            (0x81, 2),
            (0x82, 229),
            (0x84, 137),
            (0x89, 2),
            (0x8a, 1),
            (0x8c, 1),
            (0x8e, 229),
            (0x91, 224),
            (0x95, 1),
            (0x96, 94),
            (0x97, 2),
            (0x9b, 93),
            (0xa7, 93),
            (0xab, 2)
        ])
    );
    let live_loop = programs.instruction_at(0x456326).unwrap();
    assert_eq!(live_loop.opcode, 0xab);
    assert_eq!(
        programs.instruction_at(live_loop.next).unwrap().opcode,
        0x81
    );
    // This is static structure, not a claim that the running VM dispatches
    // that81: native loop waits keep it pending while the channel advances.
}

#[test]
fn ordinary_idle_pairs_match_original_producers() {
    let Some(mut iso) = disc() else { return };
    let data = iso.read("/SLUS_206.51").unwrap();
    let programs = OrdinaryPrograms::from_executable(&data).unwrap();
    let literals = [0x178ca8, 0x178cec, 0x178d38, 0x178d8c, 0x178dd8];
    let factors = [0x178a64, 0x178d00, 0x178d54, 0x178a64, 0x178a64];
    for (i, record) in programs.idle.iter().enumerate() {
        assert_eq!(record.table_address, 0x3d65c0 + i as u32 * 8);
        assert_eq!(
            u32::from(record.row),
            source_word(&data, literals[i]) & 0xffff
        );
        assert_eq!(source_word(&data, record.row_store) >> 26, 0x2b);
        assert_eq!(source_word(&data, record.rate_store) >> 26, 0x39);
        assert_eq!(
            record.rate.to_bits(),
            (source_word(&data, factors[i]) & 0xffff) << 16
        );
    }
    assert_eq!(programs.idle[0].row, 66);
    assert_eq!(programs.idle[4].row, 67);
    assert_eq!(programs.idle[0].rate.to_bits(), 5f32.to_bits());
    // Reject an altered source before interpreting its initializer or CFG.
    let mut changed = data;
    changed[0] ^= 1;
    assert!(OrdinaryPrograms::from_executable(&changed).is_err());
}

#[test]
fn ordinary_rows_resolve_existing_frame_and_state_decoders() {
    let Some(mut iso) = disc() else { return };
    let programs = OrdinaryPrograms::read(&mut iso).unwrap();
    let scripts = ScriptBank::read(&mut iso).unwrap();
    let names: BTreeSet<_> = programs
        .animation_rows
        .iter()
        .map(|row| {
            scripts.animations[usize::from(*row)]
                .name
                .as_ref()
                .unwrap()
                .clone()
        })
        .collect();
    let mut checked = BTreeSet::new();
    for path in anim::BANKS {
        let bank = anim::load(&mut iso, path).unwrap();
        for name in bank.animations().filter(|name| names.contains(*name)) {
            if !checked.insert(name.to_owned()) {
                continue;
            }
            let states = bank.states(name).unwrap().unwrap();
            let frames = bank.rotations(name).unwrap().unwrap().frames.len();
            assert!(frames > 0);
            assert_eq!(states.len(), frames, "{name}: state/frame count");
        }
    }
    assert_eq!(checked, names);
    eprintln!(
        "ordinary metadata: {} rows resolve {} unique frame/state tracks",
        programs.animation_rows.len(),
        checked.len()
    );
}

#[test]
fn ordinary_diagnostic_asset_footprint() {
    let Some(mut iso) = disc() else { return };
    let programs = OrdinaryPrograms::read(&mut iso).unwrap();
    // Diagnostic candidate only. This is neither World/config encoding nor
    // permission to increase any setup, config or snapshot limit.
    let mut image = Vec::from(*b"ORDM");
    image.extend(1u32.to_le_bytes());
    image.extend(gait::SOURCE_SHA256);
    image.extend(gait::TIME_BASE_HZ.to_le_bytes());
    let mut put = |word: u32| image.extend(word.to_le_bytes());
    put(programs.idle.len() as u32);
    for idle in &programs.idle {
        for value in [
            idle.table_address,
            idle.row_store,
            idle.rate_store,
            u32::from(idle.row),
            idle.rate.to_bits(),
        ] {
            put(value);
        }
    }
    put(programs.entries.len() as u32);
    for entry in &programs.entries {
        let (kind, index) = match entry.kind {
            EntryKind::Idle => (0, 0),
            EntryKind::Placement => (1, 0),
            EntryKind::Gait(index) => (2, u32::from(index)),
        };
        for value in [kind, index, entry.address] {
            put(value);
        }
    }
    put(programs.bodies.len() as u32);
    for body in &programs.bodies {
        image.extend(body.start.to_le_bytes());
        image.extend(body.end.to_le_bytes());
        image.extend((body.name.len() as u32).to_le_bytes());
        image.extend(body.name.as_bytes());
        image.extend((body.bytes.len() as u32).to_le_bytes());
        image.extend(&body.bytes);
    }
    image.extend((programs.instructions.len() as u32).to_le_bytes());
    for instruction in &programs.instructions {
        for value in [
            instruction.address,
            u32::from(instruction.opcode),
            instruction.next,
            u32::from(instruction.fallthrough),
            instruction.branches.len() as u32,
        ] {
            image.extend(value.to_le_bytes());
        }
        for target in &instruction.branches {
            image.extend(target.to_le_bytes());
        }
        image.push(u8::from(instruction.callback.is_some()));
        if let Some(c) = instruction.callback {
            for value in [
                c.id,
                c.native_function,
                c.arguments[0],
                c.arguments[1],
                c.arguments[2],
            ] {
                image.extend(value.to_le_bytes());
            }
        }
    }
    image.extend((programs.animation_rows.len() as u32).to_le_bytes());
    for row in &programs.animation_rows {
        image.extend(row.to_le_bytes());
    }
    let json = serde_json::json!({
        "idle":programs.idle.iter().map(|r| serde_json::json!([r.table_address,r.row_store,r.rate_store,r.row,r.rate.to_bits()])).collect::<Vec<_>>(),
        "entries":programs.entries.iter().map(|e| serde_json::json!([format!("{:?}",e.kind),e.address])).collect::<Vec<_>>(),
        "bodies":programs.bodies.iter().map(|b|serde_json::json!([b.start,b.end,b.name,b.bytes])).collect::<Vec<_>>(),
        "instructions":programs.instructions.iter().map(|i|serde_json::json!([i.address,i.opcode,i.next,i.fallthrough,i.branches,i.callback.map(|c|[c.id,c.native_function,c.arguments[0],c.arguments[1],c.arguments[2]])])).collect::<Vec<_>>(),
        "rows":programs.animation_rows,
        "source":gait::SOURCE_SHA256,"hz":gait::TIME_BASE_HZ
    });
    let json = serde_json::to_vec(&json).unwrap();
    let gait = gait::GaitData::read(&mut iso).unwrap();
    let mut combined = image.clone();
    combined.extend((gait.records.len() as u32).to_le_bytes());
    for record in &gait.records {
        for word in [record.address, record.constructor_call]
            .into_iter()
            .chain(record.words())
        {
            combined.extend(word.to_le_bytes());
        }
    }
    combined.extend((gait.row_classes.len() as u32).to_le_bytes());
    for entry in &gait.row_classes {
        combined.extend(u32::from(entry.row).to_le_bytes());
        combined.extend(entry.flags.to_le_bytes());
    }
    combined.extend((gait.phase_rows.len() as u32).to_le_bytes());
    for row in &gait.phase_rows {
        combined.extend(u32::from(*row).to_le_bytes());
    }
    let report = serde_json::json!({"raw_body_bytes":programs.bodies.iter().map(|b|b.bytes.len()).sum::<usize>(),"compact_diagnostic_bytes":image.len(),"compact_zlib6_bytes":miniz_oxide::deflate::compress_to_vec_zlib(&image,6).len(),"combined_with_gait_bytes":combined.len(),"combined_with_gait_zlib6_bytes":miniz_oxide::deflate::compress_to_vec_zlib(&combined,6).len(),"json_diagnostic_bytes":json.len(),"json_zlib6_bytes":miniz_oxide::deflate::compress_to_vec_zlib(&json,6).len(),"bodies":programs.bodies.len(),"instructions":programs.instructions.len(),"rows":programs.animation_rows.len(),"limit":"Diagnostic data only; includes no clip geometry, actor clocks or World/config format. Existing caps remain separate."});
    eprintln!("ordinary footprint: {report}");
    if let Some(path) = std::env::var_os("STREET_ORDINARY_ASSET_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
}
