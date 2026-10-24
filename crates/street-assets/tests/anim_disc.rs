//! Animation checks against a real USA disc image of the original game.
//! Skipped unless STREET_ISO points at the image.

use street_assets::{anim, iso::Iso};

fn disc() -> Option<Iso<std::io::BufReader<std::fs::File>>> {
    let path = std::env::var_os("STREET_ISO")?;
    Some(Iso::open(path).expect("STREET_ISO is not a readable ISO9660 image"))
}

fn angle(a: [f32; 4], b: [f32; 4]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    2.0 * dot.abs().min(1.0).acos()
}

#[test]
fn native_script69_frame21_reconstructs_observed_callback_target() {
    let Some(mut iso) = disc() else {
        return;
    };
    let Some(capture_path) = std::env::var_os("STREET_NATIVE_CAPTURE") else {
        return;
    };
    let capture: serde_json::Value =
        serde_json::from_slice(&std::fs::read(capture_path).unwrap()).unwrap();
    assert_eq!(capture["metadata"]["serial"], "SLUS-20651");
    let samples = capture["samples"].as_array().unwrap();
    let release = (1..samples.len())
        .find(|&i| {
            let s = &samples[i];
            let pc = s["channel0"]["script_pc"].as_u64().unwrap_or(0);
            (0x44e365..0x44e3da).contains(&pc)
                && s["ball"]["state"].as_u64() == Some(4)
                && samples[i - 1]["ball"]["state"].as_u64() == Some(3)
                && s["ball"]["previous_holder"] == s["ball"]["target_player"]
                && s["channel0"]["animation"].as_u64() == Some(296)
        })
        .expect("capture needs an observed direct script69 row296 release");
    let before = &samples[release - 1];
    let released = &samples[release];
    assert_eq!(released["planner"].as_i64(), Some(-2));
    assert_eq!(released["queue_state"].as_u64(), Some(0));
    assert_eq!(released["self_toss_duration"].as_u64(), Some(28));
    assert_eq!(released["self_toss_bounce"].as_u64(), Some(0));
    assert_eq!(released["ball"]["flight_updates"].as_u64(), Some(28));
    assert_eq!(released["ball"]["bounce"].as_u64(), Some(0));
    assert_eq!(released["player"]["selected_hand"].as_u64(), Some(1));
    let v3 = |value: &serde_json::Value| -> [f32; 3] {
        std::array::from_fn(|i| value[i].as_f64().unwrap() as f32)
    };
    let v4 = |value: &serde_json::Value| -> [f32; 4] {
        std::array::from_fn(|i| value[i].as_f64().unwrap() as f32)
    };
    let bank = anim::load(&mut iso, "/ANIM/ANIBIG.BIN").unwrap();
    let name = "DRIBBLE_FREESTYLE_12_2_B_R";
    let native = bank.native_rotations(name).unwrap().unwrap();
    assert_eq!(bank.states(name).unwrap().unwrap()[21] & 0xc, 8);
    let root =
        bank.root_motion(name).unwrap().unwrap().frames[21].map(|v| v * anim::TRANSLATION_SCALE);
    let skeleton = street_assets::player::skeleton(&mut iso).unwrap();
    // Guest state stays in the external read-only capture, not source files.
    let actor_q = v4(&before["player"]["orientation"]);
    let root_world = v3(&before["skeleton"]["root_translation"]);
    let compensation = v3(&before["player"]["root_compensation"]);
    let sx = before["skeleton"]["body_scale"][0].as_f64().unwrap() as f32;
    let sy = before["skeleton"]["body_scale"][1].as_f64().unwrap() as f32;
    let radius = released["ball"]["radius"].as_f64().unwrap() as f32;
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut rotations: Vec<[f32; 4]> = Vec::new();
    for (index, bone) in skeleton.bones.iter().enumerate().take(20) {
        let channel = native
            .bones
            .iter()
            .position(|&b| usize::from(b) == index)
            .unwrap();
        let local_q = native.frames[21][channel];
        let (position, rotation) = if let Some(parent) = bone.parent {
            let mut scales = [sx; 3];
            if sy > sx {
                if matches!(index, 13 | 17) {
                    scales[0] = sy;
                }
                if matches!(index, 1 | 5) {
                    scales[2] = sy;
                }
            }
            let translated = native_rotate(
                rotations[parent],
                std::array::from_fn(|k| bone.translation[k] * scales[k]),
            );
            (
                std::array::from_fn(|k| positions[parent][k] + translated[k]),
                native_multiply(rotations[parent], local_q),
            )
        } else {
            (root_world, native_multiply(actor_q, local_q))
        };
        positions.push(position);
        rotations.push(rotation);
    }
    let offset = native_rotate(rotations[19], [0.4 * sx, 0.0, radius]);
    let root_correction = native_rotate(actor_q, root.map(|v| v * sx));
    let target: [f32; 3] = std::array::from_fn(|k| {
        positions[19][k] + offset[k] + root_correction[k] - compensation[k]
    });
    let observed = v3(&released["self_toss_target"]);
    let error = target
        .iter()
        .zip(observed)
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f32>()
        .sqrt();
    assert!(
        error < 0.0001,
        "predicted {target:?}, observed {observed:?}, error {error}"
    );
    let renderer = bank.rotations(name).unwrap().unwrap();
    assert!(
        renderer.frames[21]
            .iter()
            .all(|q| (q.iter().map(|v| v * v).sum::<f32>() - 1.0).abs() < 1e-4)
    );
    assert!(
        native.frames[21]
            .iter()
            .any(|q| (q.iter().map(|v| v * v).sum::<f32>() - 1.0).abs() > 0.001)
    );
}

#[test]
fn calculate_self_toss_targets_for_all_scripts() {
    let Some(mut iso) = disc() else {
        return;
    };
    let mut banks = Vec::new();
    for path in anim::BANKS {
        banks.push(anim::load(&mut iso, path).unwrap());
    }
    let skeleton = street_assets::player::skeleton(&mut iso).unwrap();
    let clips = [
        (21, "DRIBBLE_FREESTYLE_03_1_B_R", 70),
        (23, "DRIBBLE_FREESTYLE_03_6_L", 30),
        (36, "DRIBBLE_FREESTYLE_10_5_L", 32),
        (60, "DRIBBLE_FREESTYLE_05_2_A_R", 50),
        (61, "DRIBBLE_FREESTYLE_06_1_B_R", 34),
        (69, "DRIBBLE_FREESTYLE_12_2_B_R", 21),
    ];
    let sx = 1.0f32;
    let radius = 0.4005f32;
    let actor_q = [0.0, 0.0, 0.0, 1.0];
    let root_world = [0.0, 0.0, 0.0];

    for (script_id, name, frame) in clips {
        let (bank, actual_name) = banks
            .iter()
            .find_map(|b| {
                b.animations()
                    .find(|a| a.eq_ignore_ascii_case(name))
                    .map(|a| (b, a))
            })
            .unwrap_or_else(|| panic!("animation {name} not found in any bank"));
        let native = bank.native_rotations(actual_name).unwrap().unwrap();
        let state_val = bank.states(actual_name).unwrap().unwrap()[frame];
        let flags = state_val & 0xc;
        let hand_mode = if flags == 8 { 1 } else { 2 };
        let hand_bone = if hand_mode == 1 { 19 } else { 15 };
        let root = bank.root_motion(actual_name).unwrap().unwrap().frames[frame]
            .map(|v| v * anim::TRANSLATION_SCALE);

        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut rotations: Vec<[f32; 4]> = Vec::new();
        for (index, bone) in skeleton.bones.iter().enumerate().take(20) {
            let channel = native
                .bones
                .iter()
                .position(|&b| usize::from(b) == index)
                .unwrap();
            let local_q = native.frames[frame][channel];
            let (position, rotation) = if let Some(parent) = bone.parent {
                let scales = [sx; 3];
                let translated = native_rotate(
                    rotations[parent],
                    std::array::from_fn(|k| bone.translation[k] * scales[k]),
                );
                (
                    std::array::from_fn(|k| positions[parent][k] + translated[k]),
                    native_multiply(rotations[parent], local_q),
                )
            } else {
                (root_world, native_multiply(actor_q, local_q))
            };
            positions.push(position);
            rotations.push(rotation);
        }
        let z_offset = if hand_mode == 1 { radius } else { -radius };
        let offset = native_rotate(rotations[hand_bone], [0.4 * sx, 0.0, z_offset]);
        let root_correction = native_rotate(actor_q, root.map(|v| v * sx));
        let target: [f32; 3] =
            std::array::from_fn(|k| positions[hand_bone][k] + offset[k] + root_correction[k]);
        assert!(
            target.iter().all(|c| c.is_finite()),
            "script {script_id} target must be finite"
        );
    }
}

fn native_multiply(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [x, y, z, w] = a;
    let [xx, yy, zz, ww] = b;
    [
        w * xx + x * ww + y * zz - z * yy,
        w * yy - x * zz + y * ww + z * xx,
        w * zz + x * yy - y * xx + z * ww,
        w * ww - x * xx - y * yy - z * zz,
    ]
}

fn native_rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let result = native_multiply(
        native_multiply(q, [v[0], v[1], v[2], 0.0]),
        [-q[0], -q[1], -q[2], q[3]],
    );
    [result[0], result[1], result[2]]
}

#[test]
fn every_animation_state_track_decodes_with_the_original_frame_count() {
    let Some(mut iso) = disc() else { return };
    let mut tracks = 0;
    for path in anim::BANKS {
        let bank = anim::load(&mut iso, path).unwrap();
        for name in bank.animations() {
            let states = bank
                .states(name)
                .unwrap_or_else(|error| panic!("{path} {name}: {error}"))
                .unwrap_or_else(|| panic!("{path} {name}: undecoded state track"));
            let rotations = bank.rotations(name).unwrap().unwrap();
            assert_eq!(states.len(), rotations.frames.len(), "{path} {name}");
            tracks += 1;
        }
    }
    assert_eq!(tracks, 646);
    // Independent raw-channel keys from ANIBIG: left hand at time 0,
    // both hands at frame 12, right hand at frame 13.
    let bank = anim::load(&mut iso, "/ANIM/ANIBIG.BIN").unwrap();
    let states = bank
        .states("1footjoglayup06_upunder_notra_l")
        .unwrap()
        .unwrap();
    assert_eq!(states[0] & 0xc, 4);
    assert_eq!(states[12] & 0xc, 12);
    assert_eq!(states[13] & 0xc, 8);
}

/// A wrong reading of the delta-compressed data shows up as jumps between
/// frames; real motion at 30 frames a second turns a bone only a little.
#[test]
fn decoded_animations_are_continuous() {
    let Some(mut iso) = disc() else {
        eprintln!("STREET_ISO not set; skipping");
        return;
    };
    let (mut steps, mut jumps, mut tracks) = (0usize, 0usize, 0usize);
    for path in anim::BANKS {
        let bank = anim::load(&mut iso, path).unwrap();
        for name in bank.animations() {
            let r = bank
                .rotations(name)
                .unwrap()
                .unwrap_or_else(|| panic!("{path}: {name} rotation track was not decoded"));
            tracks += 1;
            assert!(
                r.bones.iter().all(|&b| b < 43),
                "{name}: bone outside the skeleton"
            );
            assert!(!r.frames.is_empty(), "{name}: no frames");
            for frame in &r.frames {
                assert_eq!(frame.len(), r.bones.len(), "{name}: missing bone rotations");
                for q in frame {
                    let norm: f32 = q.iter().map(|v| v * v).sum();
                    assert!(
                        (norm - 1.0).abs() < 1e-4,
                        "{name}: invalid quaternion {q:?}"
                    );
                }
            }
            for pair in r.frames.windows(2) {
                for (a, b) in pair[0].iter().zip(&pair[1]) {
                    steps += 1;
                    jumps += usize::from(angle(*a, *b) > 45f32.to_radians());
                }
            }
            let root = bank.root_motion(name).unwrap().expect("root motion");
            assert_eq!(root.frames.len(), r.frames.len(), "{name}: track lengths");
        }
    }
    assert_eq!(tracks, 646, "USA disc rotation track coverage");
    // Fast hand and arm moves (behind-the-head passes, the broken-ankles
    // reaction) do turn a bone more than 45 degrees in a frame: about 0.09% of
    // steps, spread evenly over the frames of a block rather than at keys, so
    // they are motion, not decoding drift.
    assert!(jumps * 500 < steps, "{jumps} jumps in {steps} bone steps");
}

/// Anchors independently read from the USA disc and the original readers
/// (0x395508 for lowcomp, 0x392c00 for nocomp). The final lowcomp samples
/// catch accidental key resets in these streams, which carry only frame-0 keys.
#[test]
fn lowcomp_and_raw_tracks_match_original_reader_samples() {
    let Some(mut iso) = disc() else { return };
    let bank = anim::load(&mut iso, "/ANIM/ANIBIG.BIN").unwrap();
    let samples: [(&str, usize, usize, [f64; 4]); 6] = [
        (
            "DIVEFORBALL_SUCCESSFUL_03_8_lowcomp",
            44,
            43,
            [0.54038698, 0.45220261, 0.59519825, 0.38630784],
        ),
        (
            "DIVEPASS_PHARMACIST_01_3_R_lowcomp",
            47,
            46,
            [0.41545006, 0.38156183, 0.63862570, 0.52342052],
        ),
        (
            "DIVEPASS_PHARMACIST_02_2_R_lowcomp",
            47,
            46,
            [-0.46978467, -0.40009964, 0.55990794, 0.55292472],
        ),
        (
            "DIVEPASS_PHARMACIST_02_6_lowcomp",
            45,
            44,
            [0.49465862, 0.52574329, 0.51652847, 0.46054879],
        ),
        (
            "STANDDRIB_BALL_ON_FOOT_3_L_nocomp",
            41,
            1,
            [-0.47387439, -0.55622774, 0.47470099, 0.49062493],
        ),
        (
            "STANDDRIB_DANCE_6_L_nocomp",
            28,
            1,
            [-0.34621587, -0.45217645, 0.50628000, 0.64757377],
        ),
    ];
    for (name, frames, frame, expected) in samples {
        let rotations = bank.rotations(name).unwrap().expect(name);
        assert_eq!(rotations.bones, (0..22).collect::<Vec<_>>(), "{name}");
        assert_eq!(rotations.frames.len(), frames, "{name}");
        for (actual, expected) in rotations.frames[frame][0].into_iter().zip(expected) {
            assert!(
                (f64::from(actual) - expected).abs() < 2e-6,
                "{name} frame {frame}: {actual} differs from {expected}"
            );
        }
    }
}

#[test]
fn lowcomp_native_tracks_retain_accumulated_norms() {
    let Some(mut iso) = disc() else { return };
    let bank = anim::load(&mut iso, "/ANIM/ANIBIG.BIN").unwrap();
    let mut tracks = 0;
    let mut differences = 0;
    for name in bank.animations().filter(|name| name.ends_with("_lowcomp")) {
        let native = bank.native_rotations(name).unwrap().expect(name);
        let display = bank.rotations(name).unwrap().unwrap();
        assert_eq!(native.bones, display.bones);
        assert_eq!(native.frames.len(), display.frames.len());
        for (raw, normalized) in native
            .frames
            .iter()
            .flatten()
            .zip(display.frames.iter().flatten())
        {
            assert!(raw.iter().all(|value| value.is_finite()));
            let norm = raw.iter().map(|value| value * value).sum::<f32>().sqrt();
            assert!(norm > 0.0);
            for (&value, &unit) in raw.iter().zip(normalized) {
                assert!((value / norm - unit).abs() < 1e-6);
            }
            differences += usize::from(raw != normalized);
        }
        tracks += 1;
    }
    assert_eq!(tracks, 4);
    assert!(
        differences > 0,
        "native values must retain disc norm deviations"
    );
}

#[test]
fn standing_idle_holds_the_hips_at_standing_height() {
    let Some(mut iso) = disc() else { return };
    let bank = anim::load(&mut iso, "/ANIM/ANIBFE.BIN").unwrap();
    let root = bank.root_motion("createplayer_stand").unwrap().unwrap();
    for t in &root.frames {
        // 4.05 is the skeleton's hip height above the feet, in skeleton units.
        let hips = t[1] * anim::TRANSLATION_SCALE;
        assert!((3.9..4.2).contains(&hips), "hips at {hips}");
    }
}
