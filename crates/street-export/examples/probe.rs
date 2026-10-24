use street_assets::{db, iso::Iso, player};
fn main() {
    let path = std::env::var("STREET_ISO").unwrap();
    let mut iso = Iso::open(path).unwrap();
    let sk = player::skeleton(&mut iso).unwrap();
    for (i, b) in sk.bones.iter().enumerate() {
        println!(
            "{i} {} {:?} t={:?} r={:?} s={:?}",
            b.name, b.parent, b.translation, b.rotation, b.scale
        );
    }
    let players = db::players(&mut iso).unwrap();
    for r in [0usize, 1, 2, 145, 158, 184, 234] {
        let p = &players[r];
        let look = player::look(&mut iso, p, player::Side::Road).unwrap();
        let m = &look.models[0];
        println!(
            "{r} {} {} bbox {:?} bounds {:?} scale {}",
            p.name(),
            m.name,
            m.bbox,
            m.bounds(),
            look.scale
        );
        let c =
            street_export::vol2::character(&mut iso, p, &sk, &street_export::hires::None_).unwrap();
        let out = format!("{}/probe_{r}.glb", std::env::var("OUT").unwrap());
        std::fs::write(&out, street_export::glb::character(&c)).unwrap();
    }
}
