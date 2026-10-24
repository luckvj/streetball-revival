//! List the animations in the banks and how many decode.
//!
//!     cargo run -p street-assets --example anim_report -- <iso> [FILTER]
//!
//! With FILTER, lists the matching animations; an exact name also prints its
//! root motion.

use street_assets::{anim, iso::Iso};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut iso = Iso::open(args.next().ok_or("usage: anim_report <iso> [FILTER]")?)?;
    let filter = args.next().unwrap_or_default();
    let lower = filter.to_ascii_lowercase();
    for path in anim::BANKS {
        let bank = anim::load(&mut iso, path)?;
        let (mut total, mut rotations, mut roots) = (0, 0, 0);
        for name in bank.animations() {
            total += 1;
            let r = bank.rotations(name)?;
            let root = bank.root_motion(name)?;
            rotations += usize::from(r.is_some());
            roots += usize::from(root.is_some());
            if filter.is_empty() || !name.to_ascii_lowercase().contains(&lower) {
                continue;
            }
            match &r {
                Some(r) => println!("   {name}: {} frames, bones {:?}", r.frames.len(), r.bones),
                None => println!("   {name}: rotation channel type not decoded yet"),
            }
            if name == filter
                && let Some(root) = &root
            {
                for (f, t) in root.frames.iter().enumerate().step_by(10) {
                    println!("      root motion frame {f}: {t:.3?}");
                }
            }
        }
        println!(
            "{path} ({}): {rotations}/{total} rotation tracks, {roots}/{total} root tracks decoded",
            bank.name()
        );
    }
    Ok(())
}
