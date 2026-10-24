//! GameCube characters in the export (skipped without `STREET_ISO` and
//! `STREET_V3_GC` / `STREET_V1_GC`).

use std::path::{Path, PathBuf};

use street_export::export::{self, Options};
use street_export::hires;

fn var(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

fn out(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

type M = [f32; 16];

fn mul(a: &M, b: &M) -> M {
    std::array::from_fn(|i| {
        let (c, r) = (i / 4, i % 4);
        (0..4).map(|k| a[k * 4 + r] * b[c * 4 + k]).sum()
    })
}

fn point(m: &M, p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r])
}

/// The model's vertices in its rest pose, world space (metres), and its
/// joint names.
fn rest_vertices(path: &Path) -> (Vec<[f32; 3]>, Vec<String>) {
    let (doc, buffers, _) = gltf::import(path).unwrap();
    let nodes: Vec<gltf::Node> = doc.nodes().collect();
    let mut parent = vec![None; nodes.len()];
    for n in &nodes {
        for c in n.children() {
            parent[c.index()] = Some(n.index());
        }
    }
    let world = |i: usize| -> M {
        let mut m = nodes[i].transform().matrix();
        let mut at = parent[i];
        let mut out: M = std::array::from_fn(|k| m[k / 4][k % 4]);
        while let Some(p) = at {
            m = nodes[p].transform().matrix();
            let pm: M = std::array::from_fn(|k| m[k / 4][k % 4]);
            out = mul(&pm, &out);
            at = parent[p];
        }
        out
    };
    let skin = doc.skins().next().unwrap();
    let ibm: Vec<M> = skin
        .reader(|b| Some(&buffers[b.index()]))
        .read_inverse_bind_matrices()
        .unwrap()
        .map(|m| std::array::from_fn(|k| m[k / 4][k % 4]))
        .collect();
    let joints: Vec<M> = skin
        .joints()
        .zip(&ibm)
        .map(|(j, b)| mul(&world(j.index()), b))
        .collect();
    let names = skin
        .joints()
        .map(|j| j.name().unwrap_or_default().to_owned())
        .collect();
    let mut verts = Vec::new();
    for mesh in doc.meshes() {
        for prim in mesh.primitives() {
            let r = prim.reader(|b| Some(&buffers[b.index()]));
            let pos: Vec<[f32; 3]> = r.read_positions().unwrap().collect();
            let js: Vec<[u16; 4]> = r.read_joints(0).unwrap().into_u16().collect();
            let ws: Vec<[f32; 4]> = r.read_weights(0).unwrap().into_f32().collect();
            for ((p, j), w) in pos.iter().zip(&js).zip(&ws) {
                let mut v = [0.0; 3];
                for k in 0..4 {
                    let q = point(&joints[j[k] as usize], *p);
                    for c in 0..3 {
                        v[c] += w[k] * q[c];
                    }
                }
                verts.push(v);
            }
        }
    }
    (verts, names)
}

fn check_disc(var_name: &str, ids: &[&str], libraries: &[(&str, &str)]) {
    let (Some(vol2), Some(disc)) = (var("STREET_ISO"), var(var_name)) else {
        eprintln!("STREET_ISO or {var_name} not set; skipping");
        return;
    };
    let dir = out(&format!("export-{var_name}"));
    let mut opts = Options {
        out: dir.clone(),
        vol2,
        roster_db: None,
        hires: None,
        v3: None,
        v1: None,
        only: ids.iter().map(|s| (*s).to_owned()).collect(),
        all_anims: false,
        no_ui: true,
        threads: 4,
    };
    if var_name == "STREET_V3_GC" {
        opts.v3 = Some(disc);
    } else {
        opts.v1 = Some(disc);
    }
    let report = export::characters(&opts, &hires::None_).unwrap();
    assert_eq!(report.characters, ids.len(), "{:?}", report.warnings);
    let catalog: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("catalog.json")).unwrap()).unwrap();
    let (doc, _, _) = gltf::import(dir.join("anims/vol2/base.glb")).unwrap();
    let vol2_names: Vec<String> = doc
        .nodes()
        .skip(1)
        .map(|n| n.name().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(vol2_names.len(), 43);
    for id in ids {
        let c = catalog["characters"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == *id)
            .unwrap_or_else(|| panic!("{id} in the catalog"));
        assert_eq!(c["skeleton"], "vol2");
        assert!(c["portrait"].is_string(), "{id}");
        let set = catalog["animationSets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == *id)
            .unwrap();
        assert!(set["clips"]["stand"].is_string(), "{id}");
        assert!(dir.join(set["file"].as_str().unwrap()).exists());
        // Vol. 2's joints, names in order: every clip binds by name.
        let (verts, names) = rest_vertices(&dir.join(c["model"].as_str().unwrap()));
        assert_eq!(names, vol2_names, "{id}");
        // Standing on the floor, the crown at the model height.
        let lo = verts.iter().map(|v| v[1]).fold(f32::MAX, f32::min);
        let hi = verts.iter().map(|v| v[1]).fold(f32::MIN, f32::max);
        let want = c["fields"]["modelHeightIn"].as_f64().unwrap() as f32 * 0.0254;
        assert!(lo.abs() < 0.03, "{id}: soles at {lo}");
        assert!(
            (hi - lo - want).abs() < 0.03,
            "{id}: {} m, want {want}",
            hi - lo
        );
    }
    for (file, clip) in libraries {
        let (doc, _, _) = gltf::import(dir.join(file)).unwrap();
        let anim = doc
            .animations()
            .find(|a| a.name() == Some(clip))
            .unwrap_or_else(|| panic!("{file} has {clip}"));
        // 22 body rotations and the root's translation.
        assert_eq!(anim.channels().count(), 23, "{clip}");
    }
}

#[test]
fn v3_characters_share_vol2_joints_and_stand_on_the_floor() {
    check_disc(
        "STREET_V3_GC",
        &[
            "v3gc:mario",
            "v3gc:luigi",
            "v3gc:peach",
            "v3gc:shawn_marion",
        ],
        &[
            ("anims/v3gc/base.glb", "jumpshot_r_3"),
            ("anims/v3gc/nintendo.glb", "celeb_short_flex"),
        ],
    );
}

#[test]
fn v1_characters_share_vol2_joints_and_stand_on_the_floor() {
    check_disc(
        "STREET_V1_GC",
        &["v1gc:shaquille_o_neal", "v1gc:zoe_payne"],
        &[("anims/v1gc/base.glb", "JUMPSHOT_R_3")],
    );
}
