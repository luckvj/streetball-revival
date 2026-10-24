//! Scans every XLAY layout's element text ids, printing layouts that name
//! ids in a given file range: `layout_scan <iso> <XLAY_FE> <file>`.
use street_assets::{font, iso::Iso};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let iso = args.next().ok_or("layout_scan <iso> <path/def> <file>")?;
    let file = args.next().unwrap_or_else(|| "16".into());
    let file: u16 = u16::from_str_radix(file.trim_start_matches("0x"), 16)?;
    let mut iso = Iso::open(iso)?;
    let layouts = font::layouts(&mut iso, "/FE/XLAY_FE.BIN")?;
    println!("layouts: {}", layouts.len());
    for (blob, layout) in layouts.iter().enumerate() {
        let hit: Vec<u16> = layout
            .elements
            .iter()
            .flat_map(|e| e.texts())
            .filter(|t| t >> 8 == file)
            .collect();
        if !hit.is_empty() {
            println!(
                "blob {blob} class {} {} elements hits {:02x?}",
                layout.class,
                layout.elements.len(),
                hit
            );
        }
    }
    Ok(())
}
