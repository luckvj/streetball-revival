//! Lists a staged script's commands (category 1): frame, duration,
//! dispatch, opcode and the raw bytes. `script_report <iso> [script..]`.

use street_assets::{
    iso::Iso,
    timeline::{Payload, ScriptBank},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: script_report <iso> [script..]")?;
    let mut iso = Iso::open(path)?;
    let bank = ScriptBank::read(&mut iso)?;
    let wanted: Vec<usize> = args.filter_map(|a| a.parse().ok()).collect();
    for (i, script) in bank.scripts.iter().enumerate() {
        if !wanted.is_empty() && !wanted.contains(&i) {
            continue;
        }
        println!(
            "{i} {} kind {} frames {}",
            script.name, script.kind, script.frames
        );
        for (k, r) in script.records.iter().enumerate() {
            if let Payload::Key(key) = &r.payload {
                println!(
                    "  #{k} f{:4} d{:4} key prev #{} field {} value {} tangent {} interp {}",
                    r.frame,
                    r.duration,
                    key.previous,
                    key.field,
                    key.value,
                    key.tangent,
                    key.interpolation
                );
            }
            if let Payload::Camera(c) = &r.payload {
                println!(
                    "  #{k} f{:4} d{:4} camera id {} flags {:?} fov {:.1} actor {} at {:?} to {:?}",
                    r.frame,
                    r.duration,
                    c.flags[2],
                    c.flags,
                    c.fov,
                    c.target_actor,
                    c.position,
                    c.target
                );
            }
            if let Payload::Effect(e) = &r.payload {
                print!("  #{k}");
                let rgba: Vec<String> = e.rgba.iter().map(|c| format!("{c:08x}")).collect();
                println!(
                    "  f{:4} d{:4} effect actor {} flags {:?} rgba {} at {:?} to {:?}",
                    r.frame,
                    r.duration,
                    e.target_actor,
                    e.flags,
                    rgba.join(" "),
                    e.position,
                    e.target
                );
            }
            if let Payload::Command(c) = &r.payload {
                let hex: Vec<String> = c.bytes.iter().map(|b| format!("{b:02x}")).collect();
                println!(
                    "  f{:4} d{:4} disp {} op {:3} {}",
                    r.frame,
                    r.duration,
                    c.dispatch,
                    c.opcode,
                    hex.join("")
                );
            }
        }
    }
    Ok(())
}
