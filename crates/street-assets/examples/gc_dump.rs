//! Lists or extracts files from a GameCube disc, for format work.
//!
//! `cargo run -p street-assets --example gc_dump -- <disc> list`
//! `cargo run -p street-assets --example gc_dump -- <disc> big <archive>`
//! `cargo run -p street-assets --example gc_dump -- <disc> dol <out file>`
//! `cargo run -p street-assets --example gc_dump -- <disc> get <out dir> <path> [<entry glob>...]`
//!
//! `get` writes a disc file, or with entry names (a trailing `*` matches a
//! prefix) the named entries of a BIG archive, unpacked.

use street_assets::gcdisc::GcDisc;

/// `*` matches any run of characters; case is ignored.
fn matches(pattern: &str, name: &str) -> bool {
    fn glob(p: &[u8], n: &[u8]) -> bool {
        match p.split_first() {
            None => n.is_empty(),
            Some((b'*', rest)) => (0..=n.len()).any(|k| glob(rest, &n[k..])),
            Some((c, rest)) => n.first() == Some(c) && glob(rest, &n[1..]),
        }
    }
    glob(
        pattern.to_ascii_lowercase().as_bytes(),
        name.to_ascii_lowercase().as_bytes(),
    )
}

fn main() -> street_assets::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [disc, command, rest @ ..] = &args[..] else {
        eprintln!("usage: gc_dump <disc> list | big <archive> | get <out> <path> [entries]");
        std::process::exit(2);
    };
    let mut disc = GcDisc::open(disc)?;
    match (command.as_str(), rest) {
        ("list", _) => {
            println!("{} {}", disc.game_id(), disc.title());
            for (path, size) in disc.files() {
                println!("{size:>10} {path}");
            }
        }
        ("dol", [out]) => std::fs::write(out, disc.main_dol()?)?,
        ("big", [archive]) => {
            for e in disc.big_index(archive)? {
                println!("{:>10} {:>9} {}", e.offset, e.size, e.name);
            }
        }
        ("get", [out, path, entries @ ..]) => {
            std::fs::create_dir_all(out)?;
            if entries.is_empty() {
                let name = path.rsplit('/').next().unwrap_or(path);
                std::fs::write(std::path::Path::new(out).join(name), disc.read(path)?)?;
            } else {
                for e in disc.big_index(path)? {
                    if entries.iter().any(|p| matches(p, &e.name)) {
                        let bytes = disc.big_entry(path, &e)?;
                        let name = e.name.replace(['/', '\\'], "_");
                        std::fs::write(std::path::Path::new(out).join(name), bytes)?;
                    }
                }
            }
        }
        _ => {
            eprintln!("unknown command");
            std::process::exit(2);
        }
    }
    Ok(())
}
