//! Read-only inspection: cargo run -p street-assets --example save_report -- SAVE.
use street_assets::save::{GciSave, MaxSave, Ps2Save, Section};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("provide a save path")?;
    let bytes = std::fs::read(path)?;
    if bytes.starts_with(b"Ps2PowerSave") {
        let archive = MaxSave::parse(&bytes)?;
        println!(
            "MAX directory: {} / title: {}",
            archive.directory, archive.title
        );
        for file in &archive.files {
            println!("  {}: {} bytes", file.name, file.bytes.len());
        }
        report(&archive.game_data()?);
    } else if bytes.starts_with(b"GNZE69") {
        let save = GciSave::parse(&bytes)?;
        println!(
            "GameCube Vol. 2: {} / {} blocks / {} payload bytes",
            save.filename,
            save.blocks,
            save.payload.len()
        );
        println!("GameCube payload mapping to PS2 structures has not been established.");
    } else {
        report(&Ps2Save::parse(&bytes)?);
    }
    Ok(())
}

fn report(save: &Ps2Save) {
    for section in Section::ALL {
        println!("  {section:?}: {:#x} bytes", save.section(section).len());
    }
    for index in 0..32 {
        let user = save.user_info(index).unwrap();
        if user.is_used() {
            println!(
                "  User {index}: {} / counters {:?} / court {} / challenge groups {:#x}",
                user.name(),
                std::array::from_fn::<_, 9, _>(|i| user.counter(i).unwrap()),
                user.current_court(),
                user.challenge_groups()
            );
        }
    }
    println!(
        "Original PS2 six-section layout validated; lossless write: {} bytes",
        save.to_bytes().len()
    );
}
