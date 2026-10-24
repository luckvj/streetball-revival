//! Alpha-named draw groups must render translucent: every mesh in one has to
//! use a texture with alpha, which is what picks `Blend`/`Mask` in
//! `street-render` (`court::material`). Skipped unless STREET_ISO points at
//! a real USA disc image of the original game image.

use street_assets::{court, iso::Iso};

fn disc() -> Option<Iso<std::io::BufReader<std::fs::File>>> {
    let path = std::env::var_os("STREET_ISO")?;
    Some(Iso::open(path).expect("STREET_ISO is not a readable ISO9660 image"))
}

#[test]
fn alpha_groups_use_translucent_textures_on_every_court() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let mut opaque = Vec::new();
    let mut missing = Vec::new();
    let mut checked = 0;
    for id in court::court_ids(&mut iso).unwrap() {
        let c = court::load(&mut iso, &id).unwrap();
        for m in &c.models {
            for mesh in &m.meshes {
                let Some(group) = mesh.group.as_deref() else {
                    continue;
                };
                if !group.to_ascii_lowercase().contains("alpha") {
                    continue;
                }
                checked += 1;
                match mesh.texture.as_deref().and_then(|t| c.textures.get(t)) {
                    None => missing.push(format!(
                        "{id} {}:{group} texture {:?}",
                        m.name, mesh.texture
                    )),
                    Some(t) if !t.has_alpha() => opaque.push(format!(
                        "{id} {}:{group} texture {} is fully opaque",
                        m.name, t.name
                    )),
                    Some(_) => {}
                }
            }
        }
    }
    assert!(checked > 0, "no alpha groups found on any court");
    assert!(
        missing.is_empty(),
        "alpha groups without a texture:\n{}",
        missing.join("\n")
    );
    assert!(
        opaque.is_empty(),
        "alpha groups on fully opaque textures (would render solid):\n{}",
        opaque.join("\n")
    );
}
