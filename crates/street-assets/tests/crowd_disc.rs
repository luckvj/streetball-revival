//! The crowds against a real disc. Skipped unless STREET_ISO is set.

use std::{fs::File, io::BufReader};

use street_assets::{crowd, iso::Iso};

fn disc() -> Option<Iso<BufReader<File>>> {
    let path = std::env::var_os("STREET_ISO")?;
    Some(Iso::open(path).expect("STREET_ISO is not a readable ISO9660 image"))
}

/// Every court with a crowd: Broad Street (PHILLY) is an empty alley, with
/// no `PHILLYC.VIV` and no seating plan.
const COURTS: [&str; 10] = [
    "CENTER", "FOREST", "FOSTER", "INDOOR", "MOSSWD", "NYCAGE", "RUCKER", "RUCKRC", "SEATTL",
    "SOLHOL",
];

/// Every court's crowd decodes: each model's strips give exactly the
/// triangles its header counts (the decoder rejects anything else), the
/// triangles index real vertices, and every seat finds its head and its
/// textures.
#[test]
fn every_crowd_decodes() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    for court in COURTS {
        let c = crowd::load(&mut iso, court).unwrap_or_else(|e| panic!("{court}: {e}"));
        assert!(!c.seats.is_empty(), "{court}: no seats");
        assert!(
            !c.body.animations.is_empty(),
            "{court}: the body has no animations"
        );
        for model in std::iter::once(&c.body).chain(c.heads.iter().flatten()) {
            assert!(
                model
                    .triangles
                    .iter()
                    .flatten()
                    .all(|k| (k.vertex as usize) < model.vertices)
            );
            for a in &model.animations {
                assert_eq!(a.key.len(), model.vertices);
                // Poses stay near the rest shape: an animation moves a
                // spectator, it does not scatter him.
                let mut pose = Vec::new();
                a.pose(a.frames / 2, &mut pose);
                let far = pose
                    .iter()
                    .zip(&model.rest)
                    .map(|(p, r)| {
                        ((p[0] - r[0]).powi(2) + (p[1] - r[1]).powi(2) + (p[2] - r[2]).powi(2))
                            .sqrt()
                    })
                    .fold(0f32, f32::max);
                assert!(far < 30.0, "{court} {}: a vertex moved {far}", model.name);
            }
        }
        // No court's plan names head kind 3 (`P`): the loader
        // (`0x30b9b8`) loads a kind only when seats name it, so the `P`
        // heads on the disc are never drawn, in the original either.
        assert!(c.seats.iter().all(|s| s.head < 3), "{court}: a P head");
        for seat in &c.seats {
            assert!(
                c.heads.get(seat.head).is_some_and(Option::is_some),
                "{court} seat {}: no head model {}",
                seat.name,
                seat.head
            );
        }
        assert!(!c.body_sheet.is_empty(), "{court}: no body textures");
    }
    assert!(matches!(
        crowd::load(&mut iso, "PHILLY"),
        Err(street_assets::Error::NotFound(_))
    ));
}

/// Every frame of every animation of every crowd model stays whole: no
/// vertex leaves the animation's box over its key (the deltas are unsigned
/// bytes, `0x3242b0`), and none moves more than half a spectator
/// between one frame and the next. Read as signed bytes, the cheering
/// animations' big deltas wrapped 256 steps the wrong way and tore the
/// spectators into zigzags (this test failed on both counts).
#[test]
fn every_crowd_frame_stays_whole() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let mut worst_step = 0f32;
    for court in COURTS {
        let c = crowd::load(&mut iso, court).unwrap_or_else(|e| panic!("{court}: {e}"));
        for model in std::iter::once(&c.body).chain(c.heads.iter().flatten()) {
            for (i, a) in model.animations.iter().enumerate() {
                assert!(
                    a.frames > 0,
                    "{court} {} animation {i}: no frames",
                    model.name
                );
                let mut pose = Vec::new();
                let mut before: Vec<[f32; 3]> = Vec::new();
                for f in 0..a.frames {
                    a.pose(f, &mut pose);
                    for (v, (p, k)) in pose.iter().zip(&a.key).enumerate() {
                        for axis in 0..3 {
                            let d = (p[axis] - k[axis]) / a.scale[axis].max(f32::MIN_POSITIVE);
                            assert!(
                                (-0.01..=255.01).contains(&d),
                                "{court} {} animation {i} frame {f} vertex {v}: outside the box",
                                model.name
                            );
                        }
                    }
                    if f > 0 {
                        let step = pose
                            .iter()
                            .zip(&before)
                            .map(|(p, q)| {
                                ((p[0] - q[0]).powi(2)
                                    + (p[1] - q[1]).powi(2)
                                    + (p[2] - q[2]).powi(2))
                                .sqrt()
                            })
                            .fold(0f32, f32::max);
                        assert!(
                            step < STEP,
                            "{court} {} animation {i}: a vertex jumped {step} between frames {} and {f}",
                            model.name,
                            f - 1
                        );
                        worst_step = worst_step.max(step);
                    }
                    std::mem::swap(&mut pose, &mut before);
                }
            }
        }
    }
    eprintln!("largest step between frames: {worst_step}");
}

/// A spectator stands about 22 model units tall (scaled by 0.3257 in the
/// stands). Waving hands move up to 8.8 units in a frame (vertex 122 of
/// the body's animation 23); signed deltas threw vertices up to 29 units
/// in a frame.
const STEP: f32 = 12.0;

/// The heads play the body's animation by index on their own clocks
/// (`FUN_0030dd50`), so every head model has the body's records with the
/// same frame counts.
#[test]
fn heads_have_the_bodys_animations() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    for court in COURTS {
        let c = crowd::load(&mut iso, court).unwrap_or_else(|e| panic!("{court}: {e}"));
        let body: Vec<usize> = c.body.animations.iter().map(|a| a.frames).collect();
        for head in c.heads.iter().flatten() {
            let frames: Vec<usize> = head.animations.iter().map(|a| a.frames).collect();
            assert_eq!(frames, body, "{court}: a head's animations");
        }
    }
}
