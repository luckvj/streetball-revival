//! Print a front-end screen's layout: each element's kind, id, place,
//! size, font and text ids (`file << 8 | index`).
//!
//!     cargo run -p street-assets --example layout_report -- <iso> <class> [XLAY_GM]
//!
//! With `LAYOUT_ALL` set, every element's props follow it.

use street_assets::{font, iso::Iso};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(iso), Some(class)) = (args.first(), args.get(1)) else {
        return Err("usage: layout_report <iso> <class> [XLAY_GM]".into());
    };
    let path = if args.get(2).is_some() {
        "/FE/XLAY_GM.BIN"
    } else {
        "/FE/XLAY_FE.BIN"
    };
    let class: u32 = class.parse()?;
    for (blob, layout) in font::layouts(&mut Iso::open(iso)?, path)?
        .iter()
        .enumerate()
    {
        if layout.class != class {
            continue;
        }
        println!(
            "blob {blob}, class {class}: {} elements",
            layout.elements.len()
        );
        for e in &layout.elements {
            let texts: Vec<String> = e.texts().iter().map(|t| format!("{t:#06x}")).collect();
            let tabs: Vec<String> = [25, 26, 27, 28, 29, 30, 31, 32, 33, 34]
                .into_iter()
                .filter_map(|k| e.props.get(&k).map(|v| format!("{k}={v}")))
                .collect();
            let rows: Vec<String> = [39, 40, 41, 42, 43, 44, 45, 46, 47]
                .into_iter()
                .filter_map(|k| e.props.get(&k).map(|v| format!("{k}={v}")))
                .collect();
            println!(
                "  kind {:3} id {:4} align {} at {:?} size {:?} font {:?} texts {:?}{}{}",
                e.kind,
                e.id,
                e.align,
                e.position(),
                e.size(),
                e.props.get(&13),
                texts,
                if tabs.is_empty() {
                    String::new()
                } else {
                    format!(" tabs {:?}", tabs)
                },
                if rows.is_empty() {
                    String::new()
                } else {
                    format!(" rows {:?}", rows)
                },
            );
            if std::env::var_os("LAYOUT_ALL").is_some() {
                println!("      props {:x?}", e.props);
            }
        }
    }
    Ok(())
}
