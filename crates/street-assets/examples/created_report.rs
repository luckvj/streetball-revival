//! The created players (records 234..239): their kit entries, the texture
//! slots of the kit models, and the images their sheets hold.
//!
//!     cargo run -p street-assets --example created_report -- <iso>

use std::collections::BTreeSet;

use street_assets::{big, db, eagl, iso::Iso, ssh};

const KIT: &str = "/XF/XF000001";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: created_report <iso>")?;
    let mut iso = Iso::open(path)?;
    let players = db::players(&mut iso)?;
    let entries = big::read_index(&mut iso, KIT)?;
    let mut read = |i: usize| big::read_entry(&mut iso, KIT, &entries[i]);
    for p in players.iter().filter(|p| p.team == 37) {
        let Some(m) = p.created_model(None) else {
            println!("{}: no created model", p.name());
            continue;
        };
        println!("{} {:?}", p.name(), m);
        for (what, i) in [("body", Some(m.body)), ("head", m.head)] {
            let Some(i) = i else { continue };
            let model = eagl::parse(&read(i)?)?;
            let slots: BTreeSet<_> = model
                .meshes
                .iter()
                .filter_map(|x| x.texture.clone())
                .collect();
            let groups: BTreeSet<_> = model
                .meshes
                .iter()
                .filter_map(|x| x.group.clone())
                .collect();
            println!(
                "  {what} {} slots {:?} groups {:?}",
                model.name, slots, groups
            );
        }
        for (what, i) in [
            ("skin", Some(m.skin_sheet)),
            ("hair", m.hair_sheet),
            ("beard", m.beard_sheet),
            ("shoe", Some(m.shoe_sheet)),
            ("top", m.top_sheet),
            ("bottom", Some(m.bottom_sheet)),
        ] {
            let Some(i) = i else { continue };
            let sheet = ssh::parse(&read(i)?)?;
            let names: Vec<_> = sheet
                .textures
                .iter()
                .map(|t| format!("{} {}x{}", t.name, t.width, t.height))
                .collect();
            println!("  {what} sheet {i}: {:?}", names);
        }
    }
    Ok(())
}
