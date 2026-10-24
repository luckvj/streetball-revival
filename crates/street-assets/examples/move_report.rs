use street_assets::{iso::Iso, moves::Kind, scripts::MoveScripts};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: move_report <disc.iso>")?;
    let mut iso = Iso::open(path)?;
    let scripts = MoveScripts::read(&mut iso)?;
    for kind in [Kind::Shot, Kind::Dunk, Kind::Trick] {
        let count = match kind {
            Kind::Shot => scripts.shots.len(),
            Kind::Dunk => scripts.dunks.len(),
            Kind::Trick => scripts.tricks.len(),
        };
        for id in 0..count {
            if let Some(script) = scripts.script(kind, id) {
                println!(
                    "{kind:?} {id}: {} {:#x} {:?} jumps {:?} callbacks {:?} args {:?}",
                    script.name,
                    script.address,
                    script.clip_targets,
                    script.external_jumps,
                    script.callbacks,
                    script.callback_args
                );
            } else {
                println!("{kind:?} {id}: absent");
            }
        }
    }
    for kind in [Kind::Shot, Kind::Dunk, Kind::Trick] {
        for slot in 0..if kind == Kind::Shot { 6 } else { 11 } {
            println!(
                "default {kind:?} {slot}: {:?}",
                scripts
                    .default_script(kind, slot)
                    .map(|s| (&s.name, &s.clip_targets))
            );
        }
    }
    Ok(())
}
