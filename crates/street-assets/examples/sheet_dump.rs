//! Write every image of one texture sheet inside a BIG archive as raw RGBA, for inspection.
//!
//!     cargo run -p street-assets --example sheet_dump -- <iso> <archive> <entry index> <out dir>
//!
//! `<archive>` is a disc path such as `/XF/XF000002`. Prints `name width height`
//! per image; each `<out dir>/<entry>_<name>.rgba` holds width * height * 4 bytes.

use street_assets::{big::Big, iso::Iso, ssh};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [iso, archive, index, out] = &args[..] else {
        return Err("usage: sheet_dump <iso> <archive> <entry index> <out dir>".into());
    };
    let index: usize = index.parse()?;
    let bytes = Iso::open(iso)?.read(archive)?;
    let big = Big::parse(&bytes)?;
    let entry = big.entries().get(index).ok_or("no such entry")?;
    let sheet = ssh::parse(big.bytes(entry))?;
    std::fs::create_dir_all(out)?;
    for t in &sheet.textures {
        let name: String = t
            .name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        std::fs::write(format!("{out}/{index}_{name}.rgba"), &t.rgba)?;
        println!("{name} {} {}", t.width, t.height);
    }
    if !sheet.skipped.is_empty() {
        println!("skipped: {:?}", sheet.skipped);
    }
    Ok(())
}
