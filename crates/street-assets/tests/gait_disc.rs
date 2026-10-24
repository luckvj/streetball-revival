//! Ordinary gait metadata from the user's original US disc and capture.
//! No original tables or executable bytes are included in this repository.

use std::{collections::BTreeSet, fs::File, io::BufReader};
use street_assets::{
    gait::{GaitData, RECORD_COUNT, TABLE_ADDRESS, TIME_BASE_HZ},
    iso::Iso,
};

fn disc() -> Option<Iso<BufReader<File>>> {
    let path = std::env::var_os("STREET_ISO")?;
    Some(Iso::open(path).expect("STREET_ISO is not a readable ISO9660 image"))
}

fn word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}

// Independent address translation for checking the production extractor.
fn source_offset(data: &[u8], address: u32) -> usize {
    let table = word(data, 28) as usize;
    let stride = u16::from_le_bytes(data[42..44].try_into().unwrap()) as usize;
    let count = u16::from_le_bytes(data[44..46].try_into().unwrap()) as usize;
    for i in 0..count {
        let at = table + i * stride;
        let base = word(data, at + 8);
        let size = word(data, at + 16);
        if word(data, at) == 1
            && let Some(delta) = address.checked_sub(base)
            && delta.checked_add(4).unwrap() <= size
        {
            return word(data, at + 4) as usize + delta as usize;
        }
    }
    panic!("unmapped test source address {address:#x}");
}

fn source_word(data: &[u8], address: u32) -> u32 {
    word(data, source_offset(data, address))
}

fn digest(words: impl IntoIterator<Item = u32>) -> u64 {
    words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
        })
}

#[test]
fn native_initializer_records_and_reference_tables_match_disc() {
    let Some(mut iso) = disc() else { return };
    let data = iso.read("/SLUS_206.51").unwrap();
    let gait = GaitData::read(&mut iso).unwrap();
    assert_eq!(TIME_BASE_HZ, 60);
    assert_eq!(gait.records.len(), RECORD_COUNT);
    let registry: BTreeSet<_> = (0..632)
        .map(|i| source_word(&data, 0x3e7240 + i * 8))
        .collect();
    for (i, record) in gait.records.iter().enumerate() {
        assert_eq!(record.address, TABLE_ADDRESS + i as u32 * 40);
        // Verify actual call sites, including the final record, rather than
        // accepting a collection with only the expected length.
        let call = source_word(&data, record.constructor_call);
        assert_eq!(call >> 26, 3);
        assert_eq!((call & 0x03ff_ffff) << 2, 0x191410);
        assert!(registry.contains(&record.script));
        for row in record.rows {
            assert_ne!(source_word(&data, 0x3de8a0 + u32::from(row) * 28), 0);
        }
    }
    // Independent original-word oracles, FNV64 over ordered little-endian
    // words. They cover all 1,080 fields, not a few selected records.
    assert_eq!(
        digest(gait.records.iter().flat_map(|record| record.words())),
        0x5ac6_bbde_ecce_8823
    );
    let own = &gait.records[104];
    assert_eq!(own.constructor_call, 0x166dd4);
    assert_eq!(own.script, 0x443eb8);
    assert_eq!(own.rows, [88, 89]);
    assert_eq!(own.root_distance.to_bits(), 0x3f17_23cd);
    assert_eq!(gait.records[107].constructor_call, 0x166f50);
    assert_eq!(gait.records[107].address + 40, 0x4b9870);

    assert_eq!(gait.row_classes.len(), 144);
    for (i, entry) in gait.row_classes.iter().enumerate() {
        assert_eq!(
            u32::from(entry.row),
            source_word(&data, 0x43f0e8 + i as u32 * 8)
        );
        assert_eq!(entry.flags, source_word(&data, 0x43f0ec + i as u32 * 8));
    }
    assert_eq!(
        digest(
            gait.row_classes
                .iter()
                .flat_map(|entry| [u32::from(entry.row), entry.flags])
        ),
        0x95e9_090b_3e8f_c3df
    );
    assert_eq!(gait.phase_rows.len(), 158);
    for (i, row) in gait.phase_rows.iter().enumerate() {
        assert_eq!(u32::from(*row), source_word(&data, 0x3d9290 + i as u32 * 4));
    }
    assert_eq!(
        digest(gait.phase_rows.iter().copied().map(u32::from)),
        0xfb30_cf03_0f52_c102
    );
    for (row, flags) in [(1, 1), (88, 17), (89, 17)] {
        assert_eq!(gait.row_flags(row), Some(flags));
        assert!(!gait.phase_member(row));
    }
    assert_eq!(gait.row_flags(1144), None);
    assert!(!gait.phase_member(1144));
}

#[test]
fn native_initializer_matches_every_retained_table_word() {
    let Some(mut iso) = disc() else { return };
    let Some(path) = std::env::var_os("STREET_NATIVE_GAIT_CAPTURE") else {
        return;
    };
    let capture: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(capture["metadata"]["serial"], "SLUS-20651");
    let observed = capture["metadata"]["gait_records"].as_array().unwrap();
    let calculated = GaitData::read(&mut iso).unwrap();
    assert_eq!(observed.len(), RECORD_COUNT);
    let mut checked = 0;
    for (i, (record, captured)) in calculated.records.iter().zip(observed).enumerate() {
        let address = captured["address"]
            .as_str()
            .unwrap()
            .trim_start_matches("0x");
        assert_eq!(record.address, u32::from_str_radix(address, 16).unwrap());
        let captured_words = [
            captured["script"].as_u64().unwrap() as u32,
            captured["row_a"].as_u64().unwrap() as u32,
            captured["row_b"].as_u64().unwrap() as u32,
            captured["category_mask"].as_u64().unwrap() as u32,
            captured["player_restriction"].as_u64().unwrap() as u32,
            captured["minimum_handles"].as_u64().unwrap() as u32,
            (captured["root_distance"].as_f64().unwrap() as f32).to_bits(),
            (captured["minimum_motion"].as_f64().unwrap() as f32).to_bits(),
            (captured["maximum_motion"].as_f64().unwrap() as f32).to_bits(),
            captured["flags"].as_u64().unwrap() as u32,
        ];
        for (field, (actual, expected)) in
            record.words().into_iter().zip(captured_words).enumerate()
        {
            assert_eq!(actual, expected, "gait record {i}, word {field}");
            checked += 1;
        }
    }
    assert_eq!(checked, 1080);
    eprintln!("ordinary gait: all {checked} constructed words match the retained original capture");
}

#[test]
fn ordinary_gait_rejects_other_and_patched_executables() {
    assert!(
        GaitData::from_executable(&[])
            .unwrap_err()
            .to_string()
            .contains("recovered US")
    );
    let Some(mut iso) = disc() else { return };
    let data = iso.read("/SLUS_206.51").unwrap();
    // Identity covers constructor, callee, time getter, and both lookup tables.
    for address in [0x163e20, 0x191410, 0x30fa50, 0x43f0e8, 0x3d9290] {
        let mut patched = data.clone();
        patched[source_offset(&data, address)] ^= 1;
        assert!(
            GaitData::from_executable(&patched)
                .unwrap_err()
                .to_string()
                .contains("recovered US")
        );
    }
    assert!(GaitData::from_executable(&data[..data.len() - 1]).is_err());
}
