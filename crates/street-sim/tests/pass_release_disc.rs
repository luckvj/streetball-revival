//! Opt-in test against the user's disc: STREET_ISO=/path/to/disc.iso.
use street_assets::{iso::Iso, passes::pass_records, scripts::executable_words};

#[test]
fn the_inbound_pass_releases_when_row_10s_script_says() {
    // In phase 6 the pass start's row filter (`FUN_0015f930`, raw
    // `0x160aa4..0x160ac4`) keeps only rows with flag bit 16; row 10 is the
    // only one. Its script blends 6 updates (`8e f8 06`) and waits 12
    // (`8d ee 0a 0c`) before callback 38 (`83 26`), which lets the ball go:
    // `street_sim::INBOUND_RELEASE` updates after the press.
    let Some(path) = std::env::var_os("STREET_ISO") else {
        return;
    };
    let mut iso = Iso::open(path).unwrap();
    let elf = iso.read("/SLUS_206.51").unwrap();
    let rows = pass_records(&elf).unwrap();
    let inbound: Vec<usize> = (0..rows.len())
        .filter(|&r| rows[r].flags & 0x10000 != 0)
        .collect();
    assert_eq!(inbound, [10]);
    let script = rows[10].script;
    assert_eq!(script, 0x4548b8);
    let bytes: Vec<u8> = executable_words(&elf, script, 40)
        .unwrap()
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    let find = |pattern: &[u8]| bytes.windows(pattern.len()).position(|w| w == pattern);
    let blend = find(&[0x8e, 0xf8]).expect("a blend");
    let wait = find(&[0x8d, 0xee]).expect("a wait");
    let release = find(&[0x83, 0x26]).expect("callback 38");
    assert!(blend < wait && wait < release);
    let updates = u32::from(bytes[blend + 2]) + u32::from(bytes[wait + 3]);
    assert_eq!(updates, u32::from(street_sim::INBOUND_RELEASE));
}
