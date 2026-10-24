//! Actual production generator versus original instruction and runtime state evidence.
use super::Rng;
use serde_json::{Value, json};
use std::{fs, path::Path};

const MAX_DRAWS: u32 = 4096;

fn words(value: &Value) -> Option<[u32; 6]> {
    let array = value.as_array()?;
    if array.len() != 6 {
        return None;
    }
    let mut result = [0; 6];
    for (dst, value) in result.iter_mut().zip(array) {
        *dst = u32::try_from(value.as_u64()?).ok()?;
    }
    Some(result)
}

fn hash_file(path: &Path) -> Value {
    let bytes = fs::read(path).unwrap();
    json!({"path":path,"blake3":blake3::hash(&bytes).to_hex().to_string()})
}

#[test]
#[ignore = "requires original instruction evidence and native capture paths"]
fn original_rng_instructions_and_captured_state_advances() {
    let reference_path = std::env::var("STREET_RNG_ORIGINAL_REFERENCE").unwrap();
    let captures = std::env::var("STREET_RNG_NATIVE_CAPTURES").unwrap();
    let output = std::env::var("STREET_RNG_PARITY_REPORT").unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    assert!(
        !Path::new(&output)
            .parent()
            .unwrap()
            .canonicalize()
            .unwrap()
            .starts_with(&root)
    );
    let reference: Value = serde_json::from_slice(&fs::read(&reference_path).unwrap()).unwrap();
    let mut instruction_rows = Vec::new();
    for case in reference["seeded_cases"].as_array().unwrap() {
        let seed = u32::try_from(case["seed"].as_u64().unwrap()).unwrap();
        let mut rng = Rng::new(seed);
        let expected = words(&case["seed_execution"]["state"]).unwrap();
        instruction_rows.push(json!({"seed":seed,"phase":"seed","actual":rng.state(),
            "expected":expected,"matched":rng.state()==expected}));
        for (index, draw) in case["draws"].as_array().unwrap().iter().enumerate() {
            let returned = rng.next_u32();
            let expected = words(&draw["state"]).unwrap();
            let expected_return = u32::try_from(draw["return"].as_u64().unwrap()).unwrap();
            instruction_rows.push(json!({"seed":seed,"phase":"draw","index":index,
                "actual_state":rng.state(),"expected_state":expected,"actual_return":returned,
                "expected_return":expected_return,"matched":rng.state()==expected && returned==expected_return}));
        }
    }
    for case in reference["carry_cases"].as_array().unwrap() {
        let input = words(&case["input"]).unwrap();
        let mut rng = Rng::from_state(input);
        let returned = rng.next_u32();
        let expected = words(&case["execution"]["state"]).unwrap();
        let expected_return = u32::try_from(case["execution"]["return"].as_u64().unwrap()).unwrap();
        instruction_rows.push(
            json!({"phase":"carry","input":input,"actual_state":rng.state(),
            "expected_state":expected,"actual_return":returned,"expected_return":expected_return,
            "matched":rng.state()==expected && returned==expected_return}),
        );
    }
    let mut datasets = Vec::new();
    let mut total_compared = 0;
    let mut total_matched = 0;
    let mut total_excluded = 0;
    for path in captures.split(';').filter(|path| !path.is_empty()) {
        let document: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(document["metadata"]["serial"], "SLUS-20651");
        let samples = document["samples"].as_array().unwrap();
        let mut rows = Vec::new();
        let mut excluded = Vec::new();
        for (index, pair) in samples.windows(2).enumerate() {
            let (before, after) = (&pair[0], &pair[1]);
            let mut reasons = Vec::new();
            let states = words(&before["native_rng_words"]).zip(words(&after["native_rng_words"]));
            if states.is_none() {
                reasons.push("missing six unsigned native RNG words");
            }
            if before["roots"]["game"].is_null()
                || before["roots"]["game"] != after["roots"]["game"]
            {
                reasons.push("game identity missing or changed");
            }
            if before["root_pointers_stable"] != true || after["root_pointers_stable"] != true {
                reasons.push("root pointer bookends unstable");
            }
            let draws = states.map(|(a, b)| b[5].wrapping_sub(a[5]));
            if !draws.is_some_and(|draws| draws <= MAX_DRAWS) {
                reasons.push("counter delta exceeds predeclared 4096-draw bound");
            }
            if !reasons.is_empty() {
                excluded.push(json!({"before_index":index,"after_index":index+1,"reasons":reasons,
                    "before_words":before["native_rng_words"],"after_words":after["native_rng_words"]}));
                continue;
            }
            let (input, expected) = states.unwrap();
            let draws = draws.unwrap();
            let mut rng = Rng::from_state(input);
            for _ in 0..draws {
                rng.next_u32();
            }
            rows.push(
                json!({"before_index":index,"after_index":index+1,"input":input,
                "native_draw_counter_delta":draws,"expected":expected,"actual":rng.state(),
                "matched":rng.state()==expected,"zero_draw_pair":draws==0}),
            );
        }
        let matched = rows.iter().filter(|row| row["matched"] == true).count();
        total_compared += rows.len();
        total_matched += matched;
        total_excluded += excluded.len();
        assert_eq!(rows.len() + excluded.len(), samples.len().saturating_sub(1));
        datasets.push(
            json!({"input":hash_file(Path::new(path)),"compared":rows.len(),"matched":matched,
            "excluded":excluded.len(),"pairs":rows,"excluded_pairs":excluded}),
        );
    }
    assert!(total_compared > 0);
    let source = include_bytes!("rng.rs");
    let compiled_adapter = include_bytes!("rng_capture_parity.rs");
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        source.as_slice(),
        fs::read(manifest.join("src/rng.rs")).unwrap()
    );
    assert_eq!(
        compiled_adapter.as_slice(),
        fs::read(manifest.join("src/rng_capture_parity.rs")).unwrap()
    );
    let report = json!({"scope":"Actual production RNG state transition, not original gameplay caller draw ordering",
        "reference":hash_file(Path::new(&reference_path)),"instruction_comparisons":instruction_rows.len(),
        "instruction_matches":instruction_rows.iter().filter(|row|row["matched"]==true).count(),
        "instruction_rows":instruction_rows,"runtime_compared":total_compared,"runtime_matched":total_matched,
        "runtime_excluded":total_excluded,"datasets":datasets,
        "cadence":"Native word+0x14 increments once per generator draw; wrapping endpoint delta supplies count, bounded at4096 before comparing other words.",
        "limitations":"Original live six-word reads are not atomic. Counter deltas do not prove no interior reseed. Every guard-qualified mismatch is retained; no residual filtering. Five other words verify the measured draw count; this does not establish which gameplay/audio/AI callers consumed draws.",
        "compiled_rng_source_blake3":blake3::hash(source).to_hex().to_string(),
        "source":hash_file(&manifest.join("src/rng.rs")),"consumer":hash_file(&std::env::current_exe().unwrap()),
        "adapter":hash_file(&manifest.join("src/rng_capture_parity.rs")),
        "compiled_adapter_blake3":blake3::hash(compiled_adapter).to_hex().to_string(),
        "cargo_lock":hash_file(&root.join("Cargo.lock"))});
    fs::write(output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    assert_eq!(
        report["instruction_matches"], report["instruction_comparisons"],
        "original fixed instruction comparisons failed; all residuals are retained in the report"
    );
    eprintln!(
        "RNG comparisons: instruction {}, runtime {total_matched}/{total_compared}, excluded {total_excluded}",
        report["instruction_matches"]
    );
}
