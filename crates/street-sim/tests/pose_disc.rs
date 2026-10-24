//! Opt-in test against the user's disc (`STREET_ISO`) and bones read from
//! PCSX2 save states (`tools/pose_capture.py`; `STREET_POSE_SET`, else
//! `street-pose-set.json` under `STREET_EVIDENCE` or the temp directory):
//! `street_sim::pose` poses each actor's channel-0 clip at its time and
//! should land every bone where the original's model has it.

use street_assets::{iso::Iso, pose::PoseData};
use street_sim::pose::{self, Body, ClipData, Frame, Place, Poses, SkeletonBone};

fn poses() -> Option<Poses> {
    let path = std::env::var_os("STREET_ISO")?;
    let mut iso = Iso::open(path).unwrap();
    let data = PoseData::read(&mut iso).unwrap();
    let skeleton: Vec<SkeletonBone> = data
        .skeleton
        .bones
        .iter()
        .map(|b| SkeletonBone {
            parent: b.parent.map(|p| p as u8),
            translation: b.translation,
            rotation: b.rotation,
        })
        .collect();
    let clips = data
        .clips
        .into_iter()
        .map(|c| ClipData {
            raw_pose: c.raw_pose,
            bones: c.bones,
            rotations: c.rotations,
            root: c.root,
            states: c.states,
        })
        .collect();
    Poses::new(&skeleton, clips, data.records)
}

fn evidence() -> Option<serde_json::Value> {
    let path = std::env::var_os("STREET_POSE_SET")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            let dir = std::env::var_os("STREET_EVIDENCE").unwrap_or(std::env::temp_dir().into());
            Some(std::path::PathBuf::from(dir).join("street-pose-set.json"))
        })?;
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn f(v: &serde_json::Value) -> f32 {
    v.as_f64().unwrap_or(0.0) as f32
}

fn v3(v: &serde_json::Value) -> [f32; 3] {
    [f(&v[0]), f(&v[1]), f(&v[2])]
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn the_posed_bones_land_where_the_originals_are() {
    let Some(poses) = poses() else {
        return;
    };
    let Some(set) = evidence() else {
        return;
    };
    let (mut close, mut actors) = (0, 0);
    let mut holders = 0;
    for state in set.as_array().unwrap() {
        let holder = state["ball_holder"].as_i64().unwrap_or(-1);
        for (i, a) in state["actors"].as_array().unwrap().iter().enumerate() {
            let ch = &a["channels"][0];
            let record = ch["clip"].as_i64().unwrap();
            if !(0..1144).contains(&record) || !poses.has(record as u16) {
                continue;
            }
            let scale = v3(&a["scale"]);
            let body = Body {
                height: scale[0],
                width: scale[1],
                head: f(&a["model_head"][1]),
            };
            let place = Place {
                position: v3(&a["position"]),
                word: a["heading"].as_i64().unwrap() as i32,
            };
            let frame = Frame {
                record: record as u16,
                time: f(&ch["time"]),
                looping: false,
            };
            let pose = poses.pose(frame, body, place).unwrap();
            let worst = (0..pose::BONES)
                .map(|b| dist(pose.joints[b].position, v3(&a["bones"][b]["p"])))
                .fold(0.0, f32::max);
            println!(
                "state {} actor {i} record {record} t {:.2} worst {worst:.3}",
                state["state"],
                f(&ch["time"])
            );
            actors += 1;
            if worst < 0.1 {
                close += 1;
                if holder == i as i64 {
                    // The held ball is the hand's point less the ball's
                    // radius (`FUN_0014da20` flag 0).
                    let hand = if ch["flags14"].as_u64().unwrap_or(0) & 8 != 0 {
                        1
                    } else {
                        2
                    };
                    let at = pose::hand_point(
                        &pose,
                        record as i32,
                        ch["frame"].as_i64().unwrap() as i32,
                        hand,
                        body,
                        Some(f(&state["ball_radius"])),
                    );
                    let d = dist(at, v3(&state["ball"]));
                    println!("  holder's ball {d:.3} off");
                    assert!(d < 0.1, "{d}");
                    holders += 1;
                }
            }
        }
    }
    println!("{close} of {actors} actors within 0.1 on every bone, {holders} holders checked");
    // The three save states of 2026-10-16: the six actors of the first
    // (intro, celebration, pickup and walk clips) and the second's block
    // jump in the air match, and the first's holder's ball; the second's
    // moving actors and every actor of the third (a restart's walk) are
    // posed by more than channel 0 (the original's blends and overlays,
    // not traced), 0.4 to 4.7 off.
    assert!(close >= 7 && holders >= 1, "{close} of {actors}, {holders}");
}
