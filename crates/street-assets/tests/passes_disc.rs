//! Opt-in test against the user's disc: STREET_ISO=/path/to/disc.iso.
use street_assets::{
    iso::Iso,
    passes::{PASS_RECORDS, pass_records},
};

#[test]
fn the_pass_table_is_read_word_for_word() {
    let Some(path) = std::env::var_os("STREET_ISO") else {
        return;
    };
    let mut iso = Iso::open(path).unwrap();
    let elf = iso.read("/SLUS_206.51").unwrap();
    let rows = pass_records(&elf).unwrap();
    assert_eq!(rows.len(), PASS_RECORDS);
    // G08: the raw words, as an independent dump of 0x3d65e8 read them.
    let raw = |r: &street_assets::passes::PassRecord| {
        [
            r.script,
            r.word_04 as u32,
            r.word_08 as u32,
            r.float_0c.to_bits(),
            r.float_10.to_bits(),
            r.flags,
            r.kind,
        ]
    };
    assert_eq!(
        raw(&rows[0]),
        [0x454dfa, 0xac, u32::MAX, 0xbf800000, 0x42480000, 0x201, 2]
    );
    // Row 1 is kind 2, not an alley-oop kind 0 as the old comment said.
    assert_eq!(
        raw(&rows[1]),
        [0x454ea6, 0xa6, u32::MAX, 0x40000000, 0x420c0000, 0x31e0, 2]
    );
    assert_eq!(
        raw(&rows[5]),
        [0x454e50, 0xa0, u32::MAX, 0x41000000, 0x41f00000, 0x4210, 4]
    );
    // Rows 6..9 are kind 5 but differ at +0x0c/+0x10.
    let kicks: Vec<_> = rows[6..10]
        .iter()
        .map(|r| (r.float_0c, r.float_10))
        .collect();
    assert_eq!(kicks, [(19.5, -1.0), (8.0, 20.5), (7.0, 35.0), (7.0, 35.0)]);
    assert!(rows[6..10].iter().all(|r| r.kind == 5));
    assert_eq!(
        raw(&rows[10]),
        [
            0x4548b8,
            u32::MAX,
            u32::MAX,
            0xbf800000,
            0xbf800000,
            0x102ff,
            4
        ]
    );
}
