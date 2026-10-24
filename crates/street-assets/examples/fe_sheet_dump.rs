//! Write every image of a front-end sheet archive (`/FE/XA*.BIN`) or a
//! single sheet (`/FE/HUD.SSH`) as raw RGBA, for inspection.
//!
//!     cargo run -p street-assets --example fe_sheet_dump -- <iso> <archive> <out dir>
//!
//! Prints `sheet name width height` per image; each
//! `<out dir>/<sheet>_<name>.rgba` holds width * height * 4 bytes.

use street_assets::{fe, iso::Iso};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [iso, archive, out] = &args[..] else {
        return Err("usage: fe_sheet_dump <iso> <archive> <out dir>".into());
    };
    let mut iso = Iso::open(iso)?;
    let sheets = if archive.to_ascii_uppercase().ends_with(".SSH") {
        vec![fe::sheet(&mut iso, archive)?]
    } else {
        fe::read(&mut iso, archive)?
    };
    std::fs::create_dir_all(out)?;
    for (k, sheet) in sheets.iter().enumerate() {
        for t in &sheet.textures {
            let name: String = t
                .name
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                .collect();
            std::fs::write(format!("{out}/{k}_{name}.rgba"), &t.rgba)?;
            println!("{k} {name} {} {}", t.width, t.height);
        }
    }
    Ok(())
}
