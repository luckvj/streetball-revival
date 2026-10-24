//! List every image and its authored sprite frames in a front-end sheet
//! archive (`/FE/XA*.BIN`) or a single sheet (`/FE/HUD.SSH`).
//!
//!     cargo run -p street-assets --example sheet_frames -- <iso> <archive> [sheet]
//!
//! Prints `sheet image name width height` then one `  frame k: x y w h`
//! line per authored frame (the bounding box of its four corners), for
//! reading which frame a constructor picks.

use street_assets::{fe, iso::Iso};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (iso, archive, only) = match &args[..] {
        [iso, archive] => (iso, archive, None),
        [iso, archive, sheet] => (iso, archive, Some(sheet.parse::<usize>()?)),
        _ => return Err("usage: sheet_frames <iso> <archive> [sheet]".into()),
    };
    let mut iso = Iso::open(iso)?;
    let sheets = if archive.to_ascii_uppercase().ends_with(".SSH") {
        vec![fe::sheet(&mut iso, archive)?]
    } else {
        fe::read(&mut iso, archive)?
    };
    for (k, sheet) in sheets.iter().enumerate() {
        if only.is_some_and(|o| o != k) {
            continue;
        }
        for (i, t) in sheet.textures.iter().enumerate() {
            println!("sheet {k} image {i} {:?} {}x{}", t.name, t.width, t.height);
            if let Some(frames) = sheet.frames.get(i) {
                for (f, frame) in frames.iter().enumerate() {
                    let [x, y, w, h] = frame.bounds();
                    println!("  frame {f}: {x} {y} {w} {h}");
                }
            }
        }
    }
    Ok(())
}
