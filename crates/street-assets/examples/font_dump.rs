//! Draw a line of text in one of the front end's fonts as raw RGBA, for
//! inspection.
//!
//!     cargo run -p street-assets --example font_dump -- <iso> <FONT> <TEXT> <out.rgba>
//!
//! Prints the width and height; the file holds width * height * 4 bytes, top row first.

use street_assets::{font, iso::Iso};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [iso, index, text, out] = &args[..] else {
        return Err("usage: font_dump <iso> <FONT> <TEXT> <out.rgba>".into());
    };
    let fonts = font::fonts(&mut Iso::open(iso)?)?;
    let f = fonts
        .get(index.parse::<usize>()?)
        .ok_or_else(|| format!("there are {} fonts", fonts.len()))?;
    let r = f.render(text, [0xdc, 0xdc, 0xbb, 255], [0xcd, 0x79, 0x03, 255]);
    std::fs::write(out, &r.rgba)?;
    println!("{} {}", r.width, r.height);
    Ok(())
}
