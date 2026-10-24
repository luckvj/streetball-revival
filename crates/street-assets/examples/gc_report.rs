//! The characters of a GameCube the original game disc (V3 or the first game).
//!
//! `cargo run -p street-assets --example gc_report -- <disc>` prints the
//! roster and teams.
//! `... -- <disc> --all` assembles every character and prints what decoded.
//! `... -- <disc> --out <dir> <key>... [--look away|alt]` writes each
//! character as `<key>.obj` + `.mtl` + PNG textures, and `<key>_render.png`:
//! front and side views of the bind pose, textured, for a quick look.
//! `... -- <disc> --skeletons [<vol2 iso>]` prints the skeletons, matched by
//! bone name against Vol. 2's when the PS2 disc is given.
//! `... --out <dir> <key> --pose <vol2 iso> <bank> <animation> <frame>` also
//! renders the character in a frame of a Vol. 2 animation (`<key>_pose.png`),
//! retargeted by bone name.
//! `... --out <dir> <key> --canon <vol2 iso> <bank> <animation> <frame>` puts
//! the character on Vol. 2's skeleton (`canonical::rebind`) and renders it in
//! that frame with the clip's rotations applied bone for bone, no retargeting
//! (`<key>_canon.png`).

use std::{fmt::Write as _, path::Path};

use street_assets::{
    character::Character,
    gc_character::{Library, Look},
    gcdisc::GcDisc,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: gc_report <disc> [--all | --out <dir> <key>... | --skeletons [iso]]");
        std::process::exit(2);
    };
    let mut disc = GcDisc::open(path)?;
    let lib = Library::open(&mut disc)?;
    let rest = &args[1..];
    match rest.first().map(String::as_str) {
        None => roster(&lib),
        Some("--all") => all(&lib, &mut disc),
        Some("--anims") => anims(&mut disc, rest.get(1))?,
        Some("--skeletons") => skeletons(&lib, &mut disc, rest.get(1))?,
        Some("--out") => {
            let out = Path::new(rest.get(1).ok_or("--out needs a directory")?);
            std::fs::create_dir_all(out)?;
            let mut look = Look::Home;
            let mut keys = Vec::new();
            let mut pose = None;
            let mut pose_v3 = None;
            let mut pose_v1 = None;
            let mut canon = None;
            let mut it = rest[2..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--canon" => {
                        let mut next = || it.next().cloned().unwrap_or_default();
                        canon =
                            Some((next(), next(), next(), next().parse::<usize>().unwrap_or(0)));
                    }
                    "--pose-v1" => {
                        let mut next = || it.next().cloned().unwrap_or_default();
                        pose_v1 = Some((next(), next(), next().parse::<usize>().unwrap_or(0)));
                    }
                    "--pose-v3" => {
                        let mut next = || it.next().cloned().unwrap_or_default();
                        pose_v3 = Some((next(), next(), next().parse::<usize>().unwrap_or(0)));
                    }
                    "--pose" => {
                        let mut next = || it.next().cloned().unwrap_or_default();
                        pose = Some((next(), next(), next(), next().parse::<usize>().unwrap_or(0)));
                    }
                    "--look" => {
                        look = match it.next().map(String::as_str) {
                            Some("away") => Look::Away,
                            Some("alt") => Look::Alternate,
                            _ => Look::Home,
                        }
                    }
                    k => keys.push(k.to_owned()),
                }
            }
            for key in keys {
                let c = lib.assemble(&mut disc, &key, look)?;
                let stem = c.id.replace(':', "_");
                write_obj(&c, out, &stem)?;
                std::fs::write(out.join(format!("{stem}_render.png")), render(&c, 512))?;
                if let Some((file, anim, frame)) = &pose_v1 {
                    let anims = street_assets::gc_character::V1Animations::load(&mut disc, file)?;
                    let index = anims
                        .index_of(anim)
                        .or_else(|| anim.parse().ok())
                        .ok_or("no such animation")?;
                    let track = anims.rotations(index)?;
                    let f = &track.frames[(*frame).min(track.frames.len() - 1)];
                    let locals =
                        street_assets::gc_character::v3_pose(&c, &c.skeleton, &track.bones, f);
                    let pose =
                        street_assets::character::pose_from_locals(&c.skeleton.bones, &locals);
                    std::fs::write(
                        out.join(format!("{stem}_v1pose.png")),
                        render_views(&c, &c.skin(&pose), 512),
                    )?;
                }
                if let Some((bank, anim, frame)) = &pose_v3 {
                    let bank = street_assets::gc_character::v3_animation_bank(&mut disc, bank)?;
                    let track = bank.rotations(anim)?.ok_or("rotations do not decode")?;
                    let f = &track.frames[(*frame).min(track.frames.len() - 1)];
                    let rigs = street_assets::gc_character::V3Rigs::load(&mut disc)?;
                    let rig = rigs.for_rig(bank.rig(anim)?);
                    let locals = street_assets::gc_character::v3_pose(&c, rig, &track.bones, f);
                    let pose =
                        street_assets::character::pose_from_locals(&c.skeleton.bones, &locals);
                    std::fs::write(
                        out.join(format!("{stem}_v3pose.png")),
                        render_views(&c, &c.skin(&pose), 512),
                    )?;
                }
                if let Some((iso, bank, anim, frame)) = &canon {
                    let (r, posed) = canon_pose(&c, iso, bank, anim, *frame)?;
                    println!(
                        "  canonical: scale {:.3}, hip {:.2}, height {:.2}, {} body bones",
                        r.scale, r.hip, r.height, r.matched
                    );
                    std::fs::write(
                        out.join(format!("{stem}_canon.png")),
                        render_views(&r.character, &posed, 512),
                    )?;
                }
                if let Some((iso, bank, anim, frame)) = &pose {
                    let posed = vol2_pose(&c, iso, bank, anim, *frame)?;
                    std::fs::write(
                        out.join(format!("{stem}_pose.png")),
                        render_views(&c, &posed, 512),
                    )?;
                }
                println!(
                    "{}: {} meshes, {} triangles, {} textures, {:.2} units tall, {} bones",
                    c.id,
                    c.meshes.len(),
                    c.triangles(),
                    c.textures.len(),
                    c.height_units(),
                    c.skeleton.bones.len()
                );
                for n in &c.notes {
                    println!("  note: {n}");
                }
            }
        }
        Some(other) => return Err(format!("unknown option {other}").into()),
    }
    Ok(())
}

/// The character's vertices in frame `frame` of a Vol. 2 animation.
fn vol2_pose(
    c: &Character,
    iso: &str,
    bank: &str,
    anim: &str,
    frame: usize,
) -> Result<Vec<Vec<[f32; 3]>>, Box<dyn std::error::Error>> {
    let mut iso = street_assets::iso::Iso::open(iso)?;
    let source = street_assets::player::skeleton(&mut iso)?;
    let bank = street_assets::anim::load(&mut iso, bank)?;
    let track = bank
        .rotations(anim)?
        .ok_or("that animation's rotations do not decode")?;
    let f = &track.frames[frame.min(track.frames.len() - 1)];
    let mut rotations: Vec<[f32; 4]> = source.bones.iter().map(|b| b.rotation).collect();
    for (channel, &bone) in track.bones.iter().enumerate() {
        if let Some(r) = rotations.get_mut(bone as usize) {
            *r = f[channel];
        }
    }
    let retarget = street_assets::retarget::Retarget::new(&source, &c.skeleton);
    let locals = retarget.pose(&rotations);
    let pose = street_assets::character::pose_from_locals(&c.skeleton.bones, &locals);
    Ok(c.skin(&pose))
}

/// Each mesh's vertices in some pose.
type Posed = Vec<Vec<[f32; 3]>>;

/// The character on Vol. 2's skeleton, posed in frame `frame` of a Vol. 2
/// animation by bone index (the canonical skeleton is Vol. 2's).
fn canon_pose(
    c: &Character,
    iso: &str,
    bank: &str,
    anim: &str,
    frame: usize,
) -> Result<(street_assets::canonical::Rebound, Posed), Box<dyn std::error::Error>> {
    let mut iso = street_assets::iso::Iso::open(iso)?;
    let vol2 = street_assets::player::skeleton(&mut iso)?;
    let r = street_assets::canonical::rebind(c, &vol2)?;
    let bank = street_assets::anim::load(&mut iso, bank)?;
    let track = bank
        .rotations(anim)?
        .ok_or("that animation's rotations do not decode")?;
    let f = &track.frames[frame.min(track.frames.len() - 1)];
    let bones = &r.character.skeleton.bones;
    let mut locals: Vec<([f32; 3], [f32; 4], [f32; 3])> = bones
        .iter()
        .map(|b| (b.scale, b.rotation, b.translation))
        .collect();
    for (channel, &bone) in track.bones.iter().enumerate() {
        if let Some(l) = locals.get_mut(bone as usize) {
            l.1 = f[channel];
        }
    }
    let pose = street_assets::character::pose_from_locals(bones, &locals);
    let posed = r.character.skin(&pose);
    Ok((r, posed))
}

/// V3's animation banks: per bank how many animations decode, and with a
/// filter the matching animations' frames and bones.
fn anims(disc: &mut GcDisc, filter: Option<&String>) -> Result<(), Box<dyn std::error::Error>> {
    if street_assets::gc_character::Game::from_id(disc.game_id())
        == Some(street_assets::gc_character::Game::V1)
    {
        for file in ["anibot.ani", "anifront.ani"] {
            let a = street_assets::gc_character::V1Animations::load(disc, file)?;
            println!("{file}: {} animations", a.len());
            for (i, name) in a.names().enumerate() {
                if filter.is_some_and(|f| name.contains(f.as_str())) {
                    let frames = a.rotations(i).map_or(0, |r| r.frames.len());
                    let root = a.root(i).ok().and_then(|r| r.first().copied());
                    println!("   {name}: {frames} frames, root {root:?}");
                }
            }
        }
        return Ok(());
    }
    for name in street_assets::gc_character::v3_animation_banks(disc)? {
        let bank = street_assets::gc_character::v3_animation_bank(disc, &name)?;
        let (mut total, mut rotations, mut roots) = (0, 0, 0);
        let mut max_bone = 0;
        for a in bank.animations() {
            total += 1;
            let r = bank.rotations(a).ok().flatten();
            if let Some(r) = &r {
                max_bone = max_bone.max(r.bones.iter().copied().max().unwrap_or(0));
                if filter.is_some_and(|f| a.contains(f.as_str())) {
                    let root = bank
                        .root_motion(a)
                        .ok()
                        .flatten()
                        .and_then(|m| m.frames.first().copied());
                    println!(
                        "   {a}: rig {:#06x}, {} frames, root {root:?}, bones {:?}",
                        bank.rig(a).unwrap_or(0),
                        r.frames.len(),
                        r.bones
                    );
                    if std::env::var_os("GC_REPORT_FRAMES").is_some() {
                        for (b, q) in r.bones.iter().zip(&r.frames[0]) {
                            println!("     channel bone {b}: {q:?}");
                        }
                    }
                }
            }
            rotations += usize::from(r.is_some());
            roots += usize::from(bank.root_motion(a).is_ok_and(|r| r.is_some()));
        }
        println!(
            "{name} ({}): {total} animations, {rotations} with rotations, {roots} with root motion, bones up to {max_bone}",
            bank.name()
        );
    }
    Ok(())
}

fn roster(lib: &Library) {
    println!(
        "{:?}: {} characters, {} teams",
        lib.game,
        lib.characters.len(),
        lib.teams.len()
    );
    for c in &lib.characters {
        println!(
            "{:28} {:7} {:24} team {:22} #{:<3} pos {:?} {:?}in {:?}lb ratings {:?}",
            c.id,
            c.kind.as_str(),
            c.name,
            c.team_name.as_deref().unwrap_or("-"),
            c.jersey.map_or("-".into(), |j| j.to_string()),
            c.position,
            c.height_in,
            c.weight_lb,
            c.ratings
        );
    }
    for t in &lib.teams {
        println!("team {:5} {:28} {}", t.id, t.name, t.roster.join(" "));
    }
}

fn all(lib: &Library, disc: &mut GcDisc) {
    let (mut ok, mut partial, mut failed) = (0, 0, 0);
    for c in &lib.characters {
        match lib.assemble(disc, &c.key, Look::Home) {
            Ok(ch) => {
                let skinned = ch.meshes.iter().all(|m| !m.joints.is_empty());
                let textured = ch.meshes.iter().filter(|m| m.texture.is_none()).count();
                let whole = !ch.notes.iter().any(|n| n.starts_with("no "))
                    && skinned
                    && !ch.meshes.is_empty();
                if whole {
                    ok += 1;
                } else {
                    partial += 1;
                }
                println!(
                    "{:30} {:4} meshes {:6} tris {:2} tex {:5.2} tall untextured {:2} skinned {} {}",
                    c.id,
                    ch.meshes.len(),
                    ch.triangles(),
                    ch.textures.len(),
                    ch.height_units(),
                    textured,
                    skinned,
                    ch.notes.join("; ")
                );
            }
            Err(e) => {
                failed += 1;
                println!("{:30} FAILED {e}", c.id);
            }
        }
    }
    println!("whole {ok}, partial {partial}, failed {failed}");
}

fn skeletons(
    lib: &Library,
    disc: &mut GcDisc,
    vol2: Option<&String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let vol2 = match vol2 {
        Some(p) => {
            let mut iso = street_assets::iso::Iso::open(p)?;
            Some(street_assets::player::skeleton(&mut iso)?)
        }
        None => None,
    };
    if let Some(v) = &vol2 {
        let names: Vec<&str> = v.bones.iter().map(|b| b.name.as_str()).collect();
        println!("vol2 ({}): {}", names.len(), names.join(" "));
    }
    let mut seen = std::collections::HashSet::new();
    for c in &lib.characters {
        let Ok(ch) = lib.assemble(disc, &c.key, Look::Home) else {
            continue;
        };
        let bones = &ch.skeleton.bones;
        let sig: String = bones
            .iter()
            .map(|b| format!("{}{:.2?}", b.name, b.translation))
            .collect();
        if !seen.insert(sig) {
            continue;
        }
        let names: Vec<&str> = bones.iter().map(|b| b.name.as_str()).collect();
        println!("{} ({} bones): {}", c.id, names.len(), names.join(" "));
        // Does parent * local(T R S) rebuild the bind pose the inverse binds hold?
        let from_locals = street_assets::character::pose_from_locals(bones, &[]);
        let bind = ch.bind_matrices();
        let err = from_locals
            .iter()
            .zip(&bind)
            .map(|(a, b)| {
                a.iter()
                    .zip(b)
                    .map(|(x, y)| (x - y).abs())
                    .fold(0.0f32, f32::max)
            })
            .fold(0.0f32, f32::max);
        println!("  locals vs inverse binds: max difference {err:.4}");
        let Some(v) = &vol2 else {
            for (i, b) in bones.iter().enumerate() {
                println!(
                    "  {i:2} {:14} parent {:?} t {:6.3?} r {:6.3?}",
                    b.name, b.parent, b.translation, b.rotation
                );
            }
            continue;
        };
        // Per shared bone: same parent?, the angle between the two rest
        // rotations (local), and the bone's offset length here / in Vol. 2.
        let vol2_bind = street_assets::character::pose_from_locals(&v.bones, &[]);
        let mut shared = 0;
        for (i, b) in bones.iter().enumerate() {
            let Some(j) = v
                .bones
                .iter()
                .position(|x| x.name.eq_ignore_ascii_case(&b.name))
            else {
                continue;
            };
            shared += 1;
            let w = &v.bones[j];
            let parent = |s: &street_assets::eagl::Skeleton, p: Option<usize>| {
                p.map_or("-".to_owned(), |p| s.bones[p].name.clone())
            };
            let same_parent = parent(&ch.skeleton, b.parent) == parent(v, w.parent);
            let dot: f32 = (0..4).map(|k| b.rotation[k] * w.rotation[k]).sum();
            let angle = 2.0 * dot.abs().min(1.0).acos().to_degrees();
            let len = |t: [f32; 3]| (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
            let ratio = len(b.translation) / len(w.translation).max(1e-6);
            let wa = bind[i];
            let wb = vol2_bind[j];
            // Angle between the two bones' rest orientations in model space.
            let unit = |m: &[f32; 16], c: usize| {
                let v = [m[4 * c], m[4 * c + 1], m[4 * c + 2]];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-9);
                v.map(|x| x / l)
            };
            let trace: f32 = (0..3)
                .map(|c| {
                    let (x, y) = (unit(&wa, c), unit(&wb, c));
                    x[0] * y[0] + x[1] * y[1] + x[2] * y[2]
                })
                .sum();
            let world = ((trace - 1.0) / 2.0).clamp(-1.0, 1.0).acos().to_degrees();
            if angle > 1.0 {
                println!("    here {:?} vol2 {:?}", b.rotation, w.rotation);
            }
            println!(
                "  {:12} parent {:5} local rest rotation differs {:6.1} deg, model-space {:6.1} deg, length x{:5.2}, rest pos {:6.2?} vs {:6.2?}",
                b.name,
                same_parent,
                angle,
                world,
                ratio,
                [wa[12], wa[13], wa[14]],
                [wb[12], wb[13], wb[14]],
            );
        }
        let missing: Vec<&str> = names
            .iter()
            .filter(|n| !v.bones.iter().any(|x| x.name.eq_ignore_ascii_case(n)))
            .copied()
            .collect();
        println!("  {shared} bones shared; not in vol2: {missing:?}");
    }
    Ok(())
}

fn write_obj(c: &Character, dir: &Path, stem: &str) -> std::io::Result<()> {
    let mut obj = format!("mtllib {stem}.mtl\n");
    let mut mtl = String::new();
    for (i, t) in c.textures.iter().enumerate() {
        let png = format!("{stem}_{}.png", t.name);
        std::fs::write(dir.join(&png), png_bytes(t.width, t.height, &t.rgba))?;
        let _ = writeln!(mtl, "newmtl t{i}\nmap_Kd {png}\nKd 1 1 1");
    }
    let _ = writeln!(mtl, "newmtl none\nKd 0.5 0.5 0.5");
    let mut base = 1;
    for m in &c.meshes {
        let _ = writeln!(obj, "o {}", m.name.replace('/', "."));
        let _ = writeln!(
            obj,
            "usemtl {}",
            m.texture.map_or("none".into(), |t| format!("t{t}"))
        );
        for p in &m.positions {
            let _ = writeln!(obj, "v {} {} {}", p[0], p[1], p[2]);
        }
        for uv in &m.uvs {
            let _ = writeln!(obj, "vt {} {}", uv[0], 1.0 - uv[1]);
        }
        for n in &m.normals {
            let _ = writeln!(obj, "vn {} {} {}", n[0], n[1], n[2]);
        }
        for t in m.indices.as_chunks::<3>().0 {
            let [a, b, c] = [t[0] + base, t[1] + base, t[2] + base];
            let _ = writeln!(obj, "f {a}/{a}/{a} {b}/{b}/{b} {c}/{c}/{c}");
        }
        base += m.positions.len() as u32;
    }
    std::fs::write(dir.join(format!("{stem}.obj")), obj)?;
    std::fs::write(dir.join(format!("{stem}.mtl")), mtl)
}

/// Front and side orthographic views of the bind pose, side by side.
fn render(c: &Character, size: usize) -> Vec<u8> {
    let posed: Vec<Vec<[f32; 3]>> = c.meshes.iter().map(|m| m.positions.clone()).collect();
    render_views(c, &posed, size)
}

pub fn render_views(c: &Character, posed: &[Vec<[f32; 3]>], size: usize) -> Vec<u8> {
    let (w, h) = (size * 2, size);
    let mut px = vec![40u8; w * h * 3];
    let mut depth = vec![f32::INFINITY; w * h];
    let all = posed.iter().flatten();
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in all {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let span = (0..3).map(|k| hi[k] - lo[k]).fold(0.0f32, f32::max) * 1.1;
    let centre: [f32; 3] = std::array::from_fn(|k| (lo[k] + hi[k]) / 2.0);
    for view in 0..2 {
        for (m, pos) in c.meshes.iter().zip(posed) {
            let tex = m.texture.map(|t| &c.textures[t]);
            let project = |p: [f32; 3]| -> [f32; 3] {
                // view 0 looks down -z (x right), view 1 down +x (z right... left)
                let (x, z) = if view == 0 {
                    (p[0], p[2])
                } else {
                    (-p[2], p[0])
                };
                let sx = (x - if view == 0 { centre[0] } else { -centre[2] }) / span + 0.5;
                let sy = 0.5 - (p[1] - centre[1]) / span;
                [
                    sx * size as f32 + (view * size) as f32,
                    sy * size as f32,
                    -(z - if view == 0 { centre[2] } else { centre[0] }),
                ]
            };
            for t in m.indices.as_chunks::<3>().0 {
                let v = [t[0] as usize, t[1] as usize, t[2] as usize];
                let s = v.map(|i| project(pos[i]));
                let area = (s[1][0] - s[0][0]) * (s[2][1] - s[0][1])
                    - (s[2][0] - s[0][0]) * (s[1][1] - s[0][1]);
                if area.abs() < 1e-6 {
                    continue;
                }
                let minx = s.iter().map(|p| p[0]).fold(f32::MAX, f32::min).max(0.0) as usize;
                let maxx = (s.iter().map(|p| p[0]).fold(f32::MIN, f32::max) as usize)
                    .min((view + 1) * size - 1);
                let miny = s.iter().map(|p| p[1]).fold(f32::MAX, f32::min).max(0.0) as usize;
                let maxy = (s.iter().map(|p| p[1]).fold(f32::MIN, f32::max) as usize).min(h - 1);
                let n = m.normals.get(v[0]).copied().unwrap_or([0.0, 0.0, 1.0]);
                for y in miny..=maxy {
                    for x in minx..=maxx {
                        let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                        let w0 = ((s[1][0] - fx) * (s[2][1] - fy)
                            - (s[2][0] - fx) * (s[1][1] - fy))
                            / area;
                        let w1 = ((s[2][0] - fx) * (s[0][1] - fy)
                            - (s[0][0] - fx) * (s[2][1] - fy))
                            / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                            continue;
                        }
                        let z = w0 * s[0][2] + w1 * s[1][2] + w2 * s[2][2];
                        let at = y * w + x;
                        if z >= depth[at] {
                            continue;
                        }
                        let mut col = [180.0f32, 180.0, 180.0];
                        if let Some(tex) = tex {
                            let uv = [0, 1].map(|k| {
                                w0 * m.uvs[v[0]][k] + w1 * m.uvs[v[1]][k] + w2 * m.uvs[v[2]][k]
                            });
                            let tx = ((uv[0].rem_euclid(1.0)) * tex.width as f32) as usize
                                % tex.width as usize;
                            let ty = ((uv[1].rem_euclid(1.0)) * tex.height as f32) as usize
                                % tex.height as usize;
                            let o = (ty * tex.width as usize + tx) * 4;
                            if tex.rgba[o + 3] < 100 {
                                continue;
                            }
                            col = [0, 1, 2].map(|k| f32::from(tex.rgba[o + k]));
                        }
                        let light = 0.55
                            + 0.45 * (if view == 0 { n[2] } else { n[0] }).abs().clamp(0.0, 1.0);
                        depth[at] = z;
                        for k in 0..3 {
                            px[at * 3 + k] = (col[k] * light).min(255.0) as u8;
                        }
                    }
                }
            }
        }
    }
    let rgba: Vec<u8> = px.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
    png_bytes(w as u32, h as u32, &rgba)
}

fn png_bytes(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    fn crc(data: &[u8]) -> u32 {
        let mut c = 0xffff_ffffu32;
        for &b in data {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
    let chunk = |out: &mut Vec<u8>, kind: &[u8], data: &[u8]| {
        out.extend((data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend(data);
        out.extend(&body);
        out.extend(crc(&body).to_be_bytes());
    };
    let mut raw = Vec::with_capacity((w * h * 4 + h) as usize);
    for row in rgba.chunks((w * 4) as usize) {
        raw.push(0);
        raw.extend(row);
    }
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend(w.to_be_bytes());
    ihdr.extend(h.to_be_bytes());
    ihdr.extend([8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(
        &mut out,
        b"IDAT",
        &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6),
    );
    chunk(&mut out, b"IEND", &[]);
    out
}
