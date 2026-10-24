//! Opt-in validation of user-owned files; no original save data is in the repo.
//! STREET_PS2_SAVE can name a MAX archive or extracted original game data.
//! STREET_GCI_SAVE can name the separately supplied GameCube save.
use street_assets::save::{GciSave, MaxSave, PS2_SAVE_LENGTH, Ps2Save, Section};

#[test]
fn supplied_original_ps2_save_has_lossless_six_section_layout() {
    let Some(path) = std::env::var_os("STREET_PS2_SAVE") else {
        return;
    };
    let bytes = std::fs::read(path).unwrap();
    let game_bytes = if bytes.starts_with(b"Ps2PowerSave") {
        let archive = MaxSave::parse(&bytes).unwrap();
        assert_eq!(archive.directory, "BASLUS-20651GAMEDATA");
        archive
            .files
            .into_iter()
            .find(|f| f.name == archive.directory)
            .unwrap()
            .bytes
    } else {
        bytes.clone()
    };
    let save = Ps2Save::read(&bytes).unwrap();
    assert_eq!(game_bytes.len(), PS2_SAVE_LENGTH);
    assert_eq!(save.to_bytes(), game_bytes);
    assert_eq!(save.user(31).unwrap().len(), 0xbc);
    assert_eq!(save.career(9).unwrap().len(), 0xc88);
    assert_eq!(save.team(9).unwrap().len(), 0x70);
    if bytes.starts_with(b"Ps2PowerSave") {
        let mut damaged = bytes;
        let last = damaged.len() - 1;
        damaged[last] ^= 1;
        assert!(MaxSave::parse(&damaged).is_err());
    }
}

#[test]
fn supplied_gamecube_save_has_valid_container_and_is_not_misread_as_ps2() {
    let Some(path) = std::env::var_os("STREET_GCI_SAVE") else {
        return;
    };
    let bytes = std::fs::read(path).unwrap();
    let save = GciSave::parse(&bytes).unwrap();
    assert!(save.is_vol_2_usa());
    assert_eq!(save.filename.as_bytes(), street_assets::save::GCI_FILENAME);
    assert_eq!(save.blocks, 7);
    assert_eq!(save.payload.len(), 57344);
    save.validate_sections().unwrap();
    let users = save.section(Section::Users).unwrap();
    assert_eq!(&users[..7], b"QUENTIN");
    assert_eq!(
        u32::from_be_bytes(users[0xc..0x10].try_into().unwrap()),
        105
    );
    let careers = save.section(Section::Careers).unwrap();
    assert_eq!(&careers[0x6f6..0x6fd], b"GILBERT");
    assert_eq!(
        u32::from_be_bytes(careers[0xc..0x10].try_into().unwrap()),
        92
    );
    assert!(Ps2Save::read(&bytes).is_err());
}
