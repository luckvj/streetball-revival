//! Write one of a court's decoded textures as raw RGBA, for inspection.
//!
//!     cargo run -p street-assets --example texture_dump -- <iso> <COURT_ID> <TEXTURE> <out.rgba>
//!
//! Prints the width and height; the file holds width * height * 4 bytes, top row first.

use street_assets::{court, iso::Iso};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [iso, id, name, out] = &args[..] else {
        return Err("usage: texture_dump <iso> <COURT_ID> <TEXTURE> <out.rgba>".into());
    };
    let c = court::load(&mut Iso::open(iso)?, id)?;
    let t = c
        .textures
        .get(name)
        .ok_or_else(|| format!("{id} has no texture {name:?}"))?;
    std::fs::write(out, &t.rgba)?;
    println!("{} {}", t.width, t.height);
    Ok(())
}
