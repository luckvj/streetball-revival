//! Decode a movie from the disc and save some of its frames as PNG, to look
//! at. The frames are the game's own pictures: keep them out of the
//! repository.
//!
//!     cargo run -p street-video --release --example movie_dump -- <iso> <MOVIE> <out dir> [every]
//!
//! `MOVIE` is a disc path such as `/MOVIES/XMT0000.NFX`; `every` saves one
//! frame in that many (default 30).

use std::{fs, path::Path};

use street_assets::iso::Iso;
use street_video::{mpeg2::Decoder, nfx::Movie};

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                0xedb8_8320 ^ (crc >> 1)
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// A PNG with stored (uncompressed) deflate blocks: no dependencies.
fn png(path: &Path, w: usize, h: usize, rgba: &[u8]) -> std::io::Result<()> {
    let mut raw = Vec::with_capacity(h * (w * 4 + 1));
    for row in rgba.chunks(w * 4) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    for (k, block) in raw.chunks(65535).enumerate() {
        let last = (k + 1) * 65535 >= raw.len();
        z.push(u8::from(last));
        let n = block.len() as u16;
        z.extend_from_slice(&n.to_le_bytes());
        z.extend_from_slice(&(!n).to_le_bytes());
        z.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &v in &raw {
        a = (a + u32::from(v)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&(b << 16 | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |tag: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut c = tag.to_vec();
        c.extend_from_slice(data);
        out.extend_from_slice(&c);
        out.extend_from_slice(&crc32(&c).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    fs::write(path, out)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(iso), Some(name), Some(out)) = (args.first(), args.get(1), args.get(2)) else {
        return Err("usage: movie_dump <iso> <MOVIE> <out dir> [every]".into());
    };
    let every: usize = args.get(3).map_or(Ok(30), |s| s.parse())?;
    let data = Iso::open(iso)?.read(name)?;
    let movie = Movie::parse(&data)?;
    let mut dec = Decoder::new(&movie.video)?;
    println!(
        "{name}: {}x{} at {:.2} fps, sound: {}",
        dec.width,
        dec.height,
        dec.rate,
        movie.audio.as_ref().map_or(0, |a| a.len())
    );
    fs::create_dir_all(out)?;
    let stem = name
        .rsplit('/')
        .next()
        .unwrap_or(name)
        .split('.')
        .next()
        .unwrap_or(name)
        .to_ascii_lowercase();
    let (mut n, mut rgba) = (0, Vec::new());
    let start = std::time::Instant::now();
    loop {
        match dec.next_frame() {
            Ok(Some(f)) => {
                if n % every == 0 {
                    f.to_rgba(&mut rgba);
                    png(
                        &Path::new(out).join(format!("{stem}_{n:04}.png")),
                        f.width,
                        f.height,
                        &rgba,
                    )?;
                }
                n += 1;
            }
            Ok(None) => break,
            Err(e) => {
                println!("frame {n}: {e}");
                break;
            }
        }
    }
    println!("{n} frames in {:.2}s", start.elapsed().as_secs_f32());
    Ok(())
}
