//! File-driven adapter for the production integrator. No copied physics kernel.
//! Run explicitly with STREET_BALL_PARITY_INPUTS (semicolon-separated paths)
//! and STREET_BALL_PARITY_REPORT (an output path outside the repository).

use super::*;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

const POSITION_TOLERANCE: f64 = 0.0001;
const NATIVE_VELOCITY_TOLERANCE: f64 = 0.0001;
const MAX_ADJACENT_UPDATES: u64 = 3600;

fn clock_capture_enabled(document: &Value) -> bool {
    document["metadata"]["flight_clock_capture"] == true
        || document["metadata"]["flight_clock"] == true
        || document["metadata"]["flight_clock"]["enabled"] == true
        || document["samples"].as_array().is_some_and(|samples| {
            samples.iter().any(|sample| {
                sample.get("flight_clock").is_some()
                    || sample["ball"].get("progress").is_some()
                    || sample["ball"].get("timer_bypass").is_some()
            })
        })
}

fn timing_basis(clock_capture: bool) -> &'static str {
    if clock_capture {
        "flight-clock-qualified"
    } else {
        "timer-inferred"
    }
}

fn address(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| u64::from_str_radix(value.as_str()?.strip_prefix("0x")?, 16).ok())
}

fn clock_guards(before: &Value, after: &Value) -> Vec<String> {
    let mut reasons = Vec::new();
    for (side, sample) in [("before", before), ("after", after)] {
        let clock = &sample["flight_clock"];
        let first = &clock["guard_before"];
        let last = &clock["guard_after"];
        if !first.is_object() || !last.is_object() {
            reasons.push(format!("{side}.flight_clock brackets missing"));
        }
        if clock["guard_words_stable"].as_bool() != Some(true) || first != last {
            reasons.push(format!(
                "{side}.flight_clock brackets changed or not certified stable"
            ));
        }
        if clock["packet_matches_guards"].as_bool() != Some(true) {
            reasons.push(format!(
                "{side}.flight_clock packet_matches_guards must be true"
            ));
        }
        for (marker_name, marker) in [("guard_before", first), ("guard_after", last)] {
            for (field, expected) in [
                ("game", address(&sample["roots"]["game"])),
                ("ball", address(&sample["ball"]["address"])),
            ] {
                if expected.is_none() || marker[field].as_u64() != expected {
                    reasons.push(format!(
                        "{side}.flight_clock.{marker_name}.{field} identity mismatch or missing"
                    ));
                }
            }
            for field in [
                "progress",
                "flight_updates",
                "timer",
                "state",
                "timer_bypass",
            ] {
                let value = sample["ball"][field]
                    .as_u64()
                    .filter(|n| *n <= u32::MAX as u64);
                if value.is_none() || marker[field].as_u64() != value {
                    reasons.push(format!(
                        "{side}.flight_clock.{marker_name}.{field} packet mismatch or missing u32"
                    ));
                }
            }
        }
        if sample["ball"]["timer_bypass"].as_u64() != Some(0) {
            reasons.push(format!("{side}.timer_bypass must be 0"));
        }
        // The native saturation comparison is signed `slt`.
        for field in ["progress", "flight_updates"] {
            if !sample["ball"][field]
                .as_u64()
                .is_some_and(|n| n <= i32::MAX as u64)
            {
                reasons.push(format!(
                    "{side}.{field} must be a nonnegative signed native counter"
                ));
            }
        }
    }
    // +0x60 increments only below +0x64. Saturation removes an independent
    // cadence measurement, so retain those pairs as explicit exclusions.
    if !before["ball"]["progress"]
        .as_u64()
        .zip(before["ball"]["flight_updates"].as_u64())
        .is_some_and(|(progress, duration)| progress < duration)
    {
        reasons
            .push("before.progress must be below flight_updates (unsaturated flight clock)".into());
    }
    if !after["ball"]["progress"]
        .as_u64()
        .zip(after["ball"]["flight_updates"].as_u64())
        .is_some_and(|(progress, duration)| progress <= duration)
    {
        reasons.push("after.progress must be at most flight_updates".into());
    }
    let progress_delta = after["ball"]["progress"]
        .as_u64()
        .zip(before["ball"]["progress"].as_u64())
        .and_then(|(after, before)| after.checked_sub(before));
    let timer_delta = before["ball"]["timer"]
        .as_u64()
        .zip(after["ball"]["timer"].as_u64())
        .and_then(|(before, after)| before.checked_sub(after));
    if !progress_delta
        .zip(timer_delta)
        .is_some_and(|(progress, timer)| progress == timer && timer > 0)
    {
        reasons.push("progress delta must equal positive timer decrease".into());
    }
    reasons
}

fn hash_file(path: &Path) -> Value {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    json!({"path": path, "bytes": bytes.len(), "blake3": blake3::hash(&bytes).to_hex().to_string()})
}

fn hash_compiled_source(path: &Path, bytes: &[u8]) -> Value {
    json!({"path": path, "bytes": bytes.len(), "blake3": blake3::hash(bytes).to_hex().to_string(),
        "origin": "include_bytes embedded in this compiled consumer", "disk_file": hash_file(path)})
}

fn vector(ball: &Value, field: &str) -> Option<[f32; 3]> {
    let values = ball[field].as_array()?;
    if values.len() < 3 {
        return None;
    }
    let mut result = [0.0; 3];
    for axis in 0..3 {
        result[axis] = values[axis].as_f64()? as f32;
        if !result[axis].is_finite() {
            return None;
        }
    }
    Some(result)
}

/// These are input-domain guards, fixed before inspecting prediction errors.
/// In particular there is no contact, position residual, velocity residual,
/// or residual-based atomicity filter. Clock captures additionally require
/// stable input brackets; legacy captures retain timer-inferred cadence.
fn guards(before: &Value, after: &Value, clock_capture: bool) -> Vec<String> {
    let mut reasons = Vec::new();
    for (side, sample) in [("before", before), ("after", after)] {
        let ball = &sample["ball"];
        for (field, value) in [
            ("state", 4),
            ("gravity_enabled", 1),
            ("bounce", 0),
            ("holder", 0),
        ] {
            if ball[field].as_u64() != Some(value) {
                reasons.push(format!("{side}.{field} must be {value}"));
            }
        }
        if sample["globals"]["fps"].as_u64() != Some(60) {
            reasons.push(format!("{side}.fps must be 60"));
        }
        if sample["root_pointers_stable"].as_bool() != Some(true) {
            reasons.push(format!("{side}.root_pointers_stable must be true"));
        }
        for field in ["position", "velocity"] {
            if vector(ball, field).is_none() {
                reasons.push(format!("{side}.{field} missing or not finite f32 xyz"));
            }
        }
        for field in ["gravity", "radius"] {
            if !ball[field].as_f64().is_some_and(|x| (x as f32).is_finite()) {
                reasons.push(format!("{side}.{field} missing or not finite f32"));
            }
        }
    }
    for field in [
        "address",
        "previous_holder",
        "target_player",
        "target_position",
        "flight_updates",
        "launch_flag",
        "collision_flag",
        "special_drain",
        "gravity",
        "radius",
    ] {
        if before["ball"][field].is_null() || after["ball"][field].is_null() {
            reasons.push(format!("missing flight signature field {field}"));
        } else if before["ball"][field] != after["ball"][field] {
            reasons.push(format!("flight signature changed: {field}"));
        }
    }
    if before["roots"]["game"].is_null() || before["roots"]["game"] != after["roots"]["game"] {
        reasons.push("game identity missing or changed".into());
    }
    match (
        before["ball"]["timer"].as_u64(),
        after["ball"]["timer"].as_u64(),
    ) {
        (Some(a), Some(b)) if a > b && a - b <= MAX_ADJACENT_UPDATES => {}
        _ => reasons.push(format!(
            "timer must decrease by 1..={MAX_ADJACENT_UPDATES} updates"
        )),
    }
    // FUN_0014d1f8 state-4 timeout handling can halve velocity at timer 5.
    // +0x64 launch duration supplies the initial timeout of duration+5.
    if !after["ball"]["timer"].as_u64().is_some_and(|n| n > 5) {
        reasons.push("after.timer must exceed native tail-helper threshold 5".into());
    }
    if !before["ball"]["timer"]
        .as_u64()
        .zip(before["ball"]["flight_updates"].as_u64())
        .is_some_and(|(timer, duration)| {
            duration.checked_add(5).is_some_and(|limit| timer <= limit)
        })
    {
        reasons.push("before.timer must be at most flight_updates + 5".into());
    }
    if clock_capture {
        reasons.extend(clock_guards(before, after));
    }
    reasons
}

fn guard_fixture(progress: u64, timer: u64) -> Value {
    let mut sample = json!({
        "roots": {"game": "0x00000001"}, "root_pointers_stable": true,
        "globals": {"fps": 60},
        "ball": {"address": "0x00000002", "state": 4, "gravity_enabled": 1,
            "bounce": 0, "holder": 0, "previous_holder": 0, "target_player": 0,
            "target_position": [0.0, 0.0, 0.0], "flight_updates": 100,
            "launch_flag": 0, "collision_flag": 0, "special_drain": 0,
            "gravity": 0.01125, "radius": 0.46875, "position": [0.0, 10.0, 0.0],
            "velocity": [0.1, 0.2, 0.3], "timer": timer,
            "progress": progress, "timer_bypass": 0}
    });
    fixture_clock_brackets(&mut sample);
    sample
}

fn fixture_clock_brackets(sample: &mut Value) {
    let ball = &sample["ball"];
    let marker = json!({"game": 1, "ball": 2, "progress": ball["progress"],
        "flight_updates": ball["flight_updates"], "timer": ball["timer"],
        "state": ball["state"], "timer_bypass": ball["timer_bypass"]});
    sample["flight_clock"] = json!({"guard_before": marker, "guard_after": marker,
        "guard_words_stable": true, "packet_matches_guards": true});
}

#[test]
fn flight_clock_guard_accepts_stable_agreeing_cadence_and_saturation_endpoint() {
    assert!(guards(&guard_fixture(20, 85), &guard_fixture(23, 82), true).is_empty());
    assert!(guards(&guard_fixture(97, 85), &guard_fixture(100, 82), true).is_empty());
    let reasons = guards(&guard_fixture(100, 85), &guard_fixture(100, 82), true);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("unsaturated flight clock"))
    );
}

#[test]
fn flight_clock_guard_rejects_signed_counter_high_bits() {
    let mut before = guard_fixture(0x80000000, 85);
    let mut after = guard_fixture(0x80000001, 84);
    for sample in [&mut before, &mut after] {
        sample["ball"]["flight_updates"] = json!(0x80000010_u32);
        fixture_clock_brackets(sample);
    }
    assert!(
        guards(&before, &after, true)
            .iter()
            .any(|reason| reason.contains("nonnegative signed native counter"))
    );
}

#[test]
fn flight_clock_guard_rejects_timer_bypass() {
    let before = guard_fixture(20, 85);
    let mut after = guard_fixture(23, 82);
    after["ball"]["timer_bypass"] = json!(1);
    fixture_clock_brackets(&mut after);
    assert_eq!(
        guards(&before, &after, true),
        ["after.timer_bypass must be 0"]
    );
}

#[test]
fn flight_clock_guard_rejects_torn_brackets_and_packet_disagreement() {
    let before = guard_fixture(20, 85);
    let mut after = guard_fixture(23, 82);
    // Even a stale true certification cannot override the underlying words.
    after["flight_clock"]["guard_after"]["timer"] = json!(81);
    let reasons = guards(&before, &after, true);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("brackets changed"))
    );
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("guard_after.timer packet mismatch"))
    );
    let mut after = guard_fixture(23, 82);
    after["flight_clock"]["guard_before"]["progress"] = json!(24);
    after["flight_clock"]["guard_after"]["progress"] = json!(24);
    let reasons = guards(&before, &after, true);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("guard_before.progress packet mismatch"))
    );
}

#[test]
fn flight_clock_guard_rejects_progress_timer_disagreement() {
    assert_eq!(
        guards(&guard_fixture(20, 85), &guard_fixture(22, 82), true),
        ["progress delta must equal positive timer decrease"]
    );
}

#[test]
fn flight_clock_legacy_classification_retains_numeric_residuals() {
    let mut before = guard_fixture(20, 85);
    let mut after = guard_fixture(23, 82);
    for sample in [&mut before, &mut after] {
        sample.as_object_mut().unwrap().remove("flight_clock");
        let ball = sample["ball"].as_object_mut().unwrap();
        ball.remove("progress");
        ball.remove("timer_bypass");
    }
    after["ball"]["position"] = json!([1000.0, -1000.0, 1000.0]);
    after["ball"]["velocity"] = json!([-1000.0, 1000.0, -1000.0]);
    let document = json!({"metadata": {"serial": "SLUS-20651"}, "samples": [before, after]});
    assert!(!clock_capture_enabled(&document));
    assert_eq!(timing_basis(false), "timer-inferred");
    assert_eq!(timing_basis(true), "flight-clock-qualified");
    assert!(guards(&document["samples"][0], &document["samples"][1], false).is_empty());
}

#[test]
fn flight_clock_declared_or_mixed_capture_cannot_fall_back_to_timer_inference() {
    let legacy = json!({"ball": {"timer": 85}});
    for metadata in [
        json!({"flight_clock_capture": true}),
        json!({"flight_clock": {"enabled": true}}),
    ] {
        let document = json!({"metadata": metadata, "samples": [legacy, legacy]});
        assert!(clock_capture_enabled(&document));
        assert!(!clock_guards(&document["samples"][0], &document["samples"][1]).is_empty());
    }
    let document = json!({"samples": [guard_fixture(20, 85), legacy]});
    assert!(clock_capture_enabled(&document));
    assert!(
        clock_guards(&document["samples"][0], &document["samples"][1])
            .iter()
            .any(|reason| reason == "after.flight_clock brackets missing")
    );
    assert!(clock_capture_enabled(
        &json!({"samples": [{"flight_clock": null}]})
    ));
    assert!(clock_capture_enabled(
        &json!({"samples": [{"ball": {"progress": null}}]})
    ));
}

#[test]
#[ignore = "needs original emulator capture paths and an external report output path"]
fn original_capture_production_ball_flight() {
    assert_eq!(
        crate::TICK_HZ,
        60,
        "capture velocity conversion assumes 60 Hz"
    );
    let input_paths =
        std::env::var("STREET_BALL_PARITY_INPUTS").expect("set STREET_BALL_PARITY_INPUTS");
    let output = std::env::var("STREET_BALL_PARITY_REPORT").expect("set STREET_BALL_PARITY_REPORT");
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository = fs::canonicalize(manifest.join("../..")).unwrap();
    let output_parent = fs::canonicalize(Path::new(&output).parent().unwrap()).unwrap();
    assert!(
        !output_parent.starts_with(&repository),
        "capture evidence must remain outside Git"
    );
    let paths: Vec<_> = input_paths.split(';').filter(|p| !p.is_empty()).collect();
    assert!(!paths.is_empty());
    let mut datasets = Vec::new();
    let mut total_rows = 0;
    let mut total_matches = 0;
    let mut total_excluded = 0;
    for input in paths {
        let document: Value = serde_json::from_slice(&fs::read(input).unwrap()).unwrap();
        assert_eq!(
            document["metadata"]["serial"].as_str(),
            Some("SLUS-20651"),
            "capture executable must be SLUS-20651: {input}"
        );
        let clock_capture = clock_capture_enabled(&document);
        let basis = timing_basis(clock_capture);
        let samples = document["samples"]
            .as_array()
            .expect("capture samples array");
        assert!(samples.len() >= 2);
        let mut rows = Vec::new();
        let mut exclusions = Vec::new();
        let mut exclusion_counts = BTreeMap::<String, usize>::new();
        let mut matched = 0;
        let mut max_position_error = [0.0_f64; 3];
        let mut max_velocity_error = [0.0_f64; 3];
        let mut contact_risk_rows = 0;
        let mut cadence_counts = BTreeMap::<u64, usize>::new();
        let mut exact_matches = 0;
        for (index, pair) in samples.windows(2).enumerate() {
            let (before, after) = (&pair[0], &pair[1]);
            let reasons = guards(before, after, clock_capture);
            if !reasons.is_empty() {
                for reason in &reasons {
                    *exclusion_counts.entry(reason.clone()).or_default() += 1;
                }
                exclusions.push(json!({"before_index": index, "after_index": index + 1,
                    "timing_basis": basis, "input_guard_exclusions": reasons,
                    "before_flight_clock": before["flight_clock"], "after_flight_clock": after["flight_clock"],
                    "before_ball": before["ball"], "after_ball": after["ball"]}));
                continue;
            }
            let updates = before["ball"]["timer"].as_u64().unwrap()
                - after["ball"]["timer"].as_u64().unwrap();
            let position = vector(&before["ball"], "position").unwrap();
            let velocity_native = vector(&before["ball"], "velocity").unwrap();
            let observed_position = vector(&after["ball"], "position").unwrap();
            let observed_velocity_native = vector(&after["ball"], "velocity").unwrap();
            // Input-only geometric risk annotation for the native clamp.
            // It never rejects a row or changes the production prediction.
            let native_bounds_crossing_from_inputs = [(0, 42.0), (2, 26.5)].map(|(axis, limit)| {
                let endpoint =
                    position[axis] as f64 + velocity_native[axis] as f64 * updates as f64;
                position[axis].abs() as f64 > limit || endpoint.abs() > limit
            });
            if native_bounds_crossing_from_inputs.iter().any(|risk| *risk) {
                contact_risk_rows += 1;
            }
            *cadence_counts.entry(updates).or_default() += 1;
            let mut ball = Ball::loose_at(position);
            ball.state = BallState::Pass;
            ball.radius = before["ball"]["radius"].as_f64().unwrap() as f32;
            // Native velocity is distance per 60 Hz update. Production Ball
            // stores distance per second. The actual production method below
            // performs all stepping; the adapter only converts units.
            ball.velocity = velocity_native.map(|v| v * 60.0);
            for _ in 0..updates {
                ball.integrate();
            }
            let predicted_velocity_native = ball.velocity.map(|v| v / 60.0);
            let mut position_error = [0.0; 3];
            let mut velocity_error_native = [0.0; 3];
            let mut velocity_error_per_second = [0.0; 3];
            let mut exact_position = [false; 3];
            let mut exact_velocity_native = [false; 3];
            let mut within_tolerance = true;
            let mut finite_output = true;
            for axis in 0..3 {
                position_error[axis] = ball.position[axis] as f64 - observed_position[axis] as f64;
                velocity_error_native[axis] =
                    predicted_velocity_native[axis] as f64 - observed_velocity_native[axis] as f64;
                velocity_error_per_second[axis] =
                    ball.velocity[axis] as f64 - (observed_velocity_native[axis] * 60.0) as f64;
                exact_position[axis] =
                    ball.position[axis].to_bits() == observed_position[axis].to_bits();
                exact_velocity_native[axis] = predicted_velocity_native[axis].to_bits()
                    == observed_velocity_native[axis].to_bits();
                finite_output &=
                    position_error[axis].is_finite() && velocity_error_native[axis].is_finite();
                within_tolerance &= position_error[axis].abs() <= POSITION_TOLERANCE
                    && velocity_error_native[axis].abs() <= NATIVE_VELOCITY_TOLERANCE;
                max_position_error[axis] = max_position_error[axis].max(position_error[axis].abs());
                max_velocity_error[axis] =
                    max_velocity_error[axis].max(velocity_error_native[axis].abs());
            }
            if within_tolerance {
                matched += 1;
            }
            if exact_position.iter().all(|x| *x) && exact_velocity_native.iter().all(|x| *x) {
                exact_matches += 1;
            }
            rows.push(json!({"before_index": index, "after_index": index + 1, "updates": updates,
                "timing_basis": basis, "before_flight_clock": before["flight_clock"], "after_flight_clock": after["flight_clock"],
                "before_wall_time_ns": before["wall_time_ns"], "after_wall_time_ns": after["wall_time_ns"],
                "before_ball": before["ball"], "after_ball": after["ball"],
                "predicted_position": ball.position, "observed_position": observed_position,
                "predicted_velocity_native_per_update": predicted_velocity_native,
                "observed_velocity_native_per_update": observed_velocity_native,
                "predicted_velocity_per_second": ball.velocity,
                "observed_velocity_per_second": observed_velocity_native.map(|v| v * 60.0),
                "signed_position_error": position_error, "signed_velocity_error_native_per_update": velocity_error_native,
                "signed_velocity_error_per_second": velocity_error_per_second,
                "exact_position_f32_bits": exact_position, "exact_native_velocity_f32_bits": exact_velocity_native,
                "within_tolerance": within_tolerance, "finite_output": finite_output,
                "classification": if !finite_output { "nonfinite_production_output" } else if within_tolerance { "match" } else { "mismatch" },
                "mismatch_retained_without_cause_attribution": !within_tolerance,
                "native_bounds_crossing_from_inputs_xz": native_bounds_crossing_from_inputs,
                "input_guard_exclusions": []}));
        }
        assert_eq!(
            rows.len() + exclusions.len(),
            samples.len() - 1,
            "every adjacent pair must be accounted for"
        );
        total_rows += rows.len();
        total_matches += matched;
        total_excluded += exclusions.len();
        datasets.push(json!({"input": hash_file(Path::new(input)), "capture_metadata": document["metadata"],
            "timing_basis": basis, "flight_clock_capture_enabled": clock_capture,
            "sample_count": samples.len(), "adjacent_pairs": samples.len() - 1,
            "summary": {"compared": rows.len(), "matched": matched, "mismatched": rows.len() - matched,
                "excluded": exclusions.len(), "max_abs_position_error_xyz": max_position_error,
                "max_abs_velocity_error_native_per_update_xyz": max_velocity_error,
                "exact_all_six_f32_bits": exact_matches, "native_bounds_contact_risk_rows_retained": contact_risk_rows,
                "native_update_count_histogram": cadence_counts},
            "exclusion_reason_counts": exclusion_counts, "pairs": rows, "excluded_pairs": exclusions}));
    }
    assert!(
        total_rows > 0,
        "no eligible capture transitions; no parity result was produced"
    );
    let executable = std::env::current_exe().unwrap();
    let report = json!({"schema": "street-production-ball-flight-parity-v2",
        "scope": "Actual compiled Ball::integrate shared by fly, fly_pass and roll compared to native state-4 direct-self-flight numeric advancement. This is the shared free-flight core only, not ordinary native shot state 5, World/self-toss caller, release, arrival, collision response, camera, rendering or whole-game parity.",
        "implementation": "Repeated calls to production Ball::integrate; no copied arithmetic kernel; production physics unchanged.",
        "sampling_limitation": "Live memory samples are not paused. Legacy datasets lack +0x190 timer_bypass and +0x60 progress and remain timer-inferred. Flight-clock-qualified datasets require zero endpoint bypass, stable brackets matching packet clock words, unsaturated starting progress, and equal progress/timer deltas. These input brackets cannot prove interior branch continuity or atomicity of position/velocity reads. All input-qualified numeric mismatches remain compared; causes are not inferred from residuals.",
        "input_guard_policy": {"state": 4, "gravity_enabled": 1, "bounce": 0, "holder": 0,
            "stable_identity": "ball.address and roots.game; both root_pointers_stable true",
            "stable_signature_fields": ["previous_holder", "target_player", "target_position", "flight_updates", "launch_flag", "collision_flag", "special_drain", "gravity", "radius"],
            "cadence": "before.ball.timer minus after.ball.timer; per-row and per-dataset timing_basis distinguishes timer-inferred legacy from flight-clock-qualified inputs",
            "flight_clock": "when metadata.flight_clock_capture is true or clock fields are present: both sample brackets stable, all clock words and identities match ball packet, timer_bypass zero, before.progress < flight_updates, after.progress <= flight_updates, progress increase equals positive timer decrease",
            "maximum_adjacent_updates": MAX_ADJACENT_UPDATES, "finite_f32_xyz": true,
            "native_tail_helper_guard": "after.timer > 5 and before.timer <= before.flight_updates + 5; source FUN_0014d1f8 0014d790..0014d81c",
            "error_based_filters": false, "residual_based_atomicity_filters": false,
            "clock_input_bracket_filters": true, "contact_residual_filters": false},
        "units": {"capture_fps": 60, "native_velocity": "distance per update", "production_velocity": "distance per second",
            "input_conversion": "native velocity * 60", "comparison_conversion": "production velocity / 60",
            "production_gravity_per_second_squared": GRAVITY, "production_tick_seconds": TICK_SECONDS,
            "production_gravity_per_update_squared": GRAVITY * TICK_SECONDS * TICK_SECONDS},
        "tolerances_predeclared": {"position_xyz_absolute": POSITION_TOLERANCE,
            "velocity_native_per_update_xyz_absolute": NATIVE_VELOCITY_TOLERANCE,
            "exact_f32_bits_reported_separately": true},
        "consumer": hash_file(&executable),
        "sources": [hash_compiled_source(&manifest.join("src/ball.rs"), include_bytes!("ball.rs")),
            hash_compiled_source(&manifest.join("src/ball_capture_parity.rs"), include_bytes!("ball_capture_parity.rs")),
            hash_compiled_source(&manifest.join("src/lib.rs"), include_bytes!("lib.rs")),
            hash_file(&manifest.join("Cargo.toml")), hash_file(&repository.join("Cargo.lock"))],
        "reproduction": {"working_directory": repository,
            "command": "cargo test -p street-sim original_capture_production_ball_flight -- --ignored --nocapture",
            "STREET_BALL_PARITY_INPUTS": input_paths, "STREET_BALL_PARITY_REPORT": output},
        "summary": {"compared": total_rows, "matched": total_matches, "mismatched": total_rows - total_matches, "excluded": total_excluded},
        "datasets": datasets});
    fs::write(&output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    eprintln!(
        "Production ball capture report: {output}; compared {total_rows}, matched {total_matches}, mismatched {}, excluded {total_excluded}",
        total_rows - total_matches
    );
}
