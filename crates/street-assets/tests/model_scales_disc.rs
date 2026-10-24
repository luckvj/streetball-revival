//! Original model-scale producer regressions. Optional guarded adapters are
//! generated before residual comparison by the retained external capture tool.
use street_assets::{db, iso::Iso};

fn disc() -> Option<Iso<std::io::BufReader<std::fs::File>>> {
    std::env::var("STREET_ISO")
        .ok()
        .map(|path| Iso::open(path).unwrap())
}

#[test]
fn original_guarded_model_scale_source_tuples_match_exact_cache_words() {
    let Some(mut iso) = disc() else { return };
    let players = db::players(&mut iso).unwrap();
    // Direct original appearance/skeleton cache captures, normal options.
    // Expected words are outputs; every formula input stays in the disc DB.
    for (record, height, weight, build, morphs, h, w, head) in [
        (
            55,
            85,
            338,
            2,
            [15, 15, 22],
            0x3fa76276,
            0x3fb78480,
            [0x3f8713e1, 0x3f8f4cbd, 0x3f8713e1],
        ),
        (
            56,
            78,
            220,
            0,
            [5, 5, 10],
            0x3f999999,
            0x3f9e628b,
            [0x3f866667, 0x3f8ccccd, 0x3f866667],
        ),
        (
            57,
            73,
            205,
            0,
            [0, 0, 3],
            0x3f8fc0fc,
            0x3f9e0a16,
            [0x3f88c463, 0x3f8cdec1, 0x3f88c463],
        ),
        (
            110,
            83,
            230,
            1,
            [8, 8, 18],
            0x3fa37237,
            0x3f9be834,
            [0x3f81e98b, 0x3f8df0f2, 0x3f81e98b],
        ),
        (
            111,
            80,
            228,
            1,
            [7, 7, 15],
            0x3f9d89d9,
            0x3f9e316f,
            [0x3f858936, 0x3f8f851e, 0x3f858936],
        ),
        (
            112,
            70,
            171,
            0,
            [0, 0, 3],
            0x3f89d89e,
            0x3f936685,
            [0x3f8ea0ea, 0x3f92e84c, 0x3f8ea0ea],
        ),
        (
            135,
            81,
            256,
            2,
            [10, 10, 20],
            0x3f9f81f8,
            0x3fa057a6,
            [0x3f8795ce, 0x3f93e93e, 0x3f8795ce],
        ),
        (
            136,
            73,
            175,
            0,
            [0, 0, 3],
            0x3f8fc0fc,
            0x3f9204b3,
            [0x3f88c463, 0x3f8cdec1, 0x3f88c463],
        ),
        (
            137,
            79,
            231,
            1,
            [6, 6, 20],
            0x3f9b91b9,
            0x3fa0cbe1,
            [0x3f85f668, 0x3f97a7db, 0x3f85f668],
        ),
    ] {
        let p = &players[record];
        assert_eq!(
            (p.model_height_in, p.model_weight_lb, p.build, p.body_scale),
            (height, weight, build, morphs)
        );
        assert_eq!(
            p.native_body_scale().unwrap().map(f32::to_bits),
            [h, w, w],
            "body record {record}"
        );
        assert_eq!(
            p.native_head_scale().unwrap().map(f32::to_bits),
            head,
            "head record {record}"
        );
    }
    // Earlier intro capture retains the basic radius branch. Its width and
    // roster reads were not bracketed; retain that narrower evidence bound.
    for (record, h, w) in [
        (65, 0x3fa17a18, 0x3fa0b38f),
        (66, 0x3f9f81f8, 0x3f9f81e1),
        (67, 0x3f999999, 0x3f97039d),
    ] {
        assert!(f32::from_bits(w) <= f32::from_bits(h));
        assert_eq!(
            players[record]
                .native_body_scale()
                .unwrap()
                .map(f32::to_bits),
            [h, w, w]
        );
    }
}

#[test]
fn compiled_production_matches_every_guarded_model_scale_cache_observation() {
    let Ok(path) = std::env::var("STREET_MODEL_SCALES_ADAPTER") else {
        return;
    };
    let mut iso = disc().expect("guarded model-scale comparison requires STREET_ISO");
    let players = db::players(&mut iso).unwrap();
    let adapter: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let input = adapter["admitted"].as_array().unwrap();
    assert!(
        !input.is_empty(),
        "guarded adapter has no admitted cache observations"
    );
    let mut rows = Vec::new();
    let mut exact_body = 0;
    let mut exact_head = 0;
    for c in input {
        let record = c["player_record"].as_u64().unwrap() as usize;
        let p = &players[record];
        let body = p.native_body_scale().unwrap().map(f32::to_bits);
        let head = p.native_head_scale().unwrap().map(f32::to_bits);
        let native_body = [
            c["native_cached_height_bits"].as_u64().unwrap() as u32,
            c["native_cached_width_bits"].as_u64().unwrap() as u32,
            c["native_cached_width_bits"].as_u64().unwrap() as u32,
        ];
        let raw = c["native_cached_head_native_bits"].as_array().unwrap();
        let native_head = [
            raw[1].as_u64().unwrap() as u32,
            raw[2].as_u64().unwrap() as u32,
            raw[0].as_u64().unwrap() as u32,
        ];
        exact_body += usize::from(body == native_body);
        exact_head += usize::from(head == native_head);
        rows.push(serde_json::json!({"sample":c["sample"],"actor_id":c["actor_id"],"player_record":record,
            "production_body_bits":body,"native_body_bits":native_body,"production_head_xyz_bits":head,"native_head_xyz_bits":native_head}));
    }
    let result = serde_json::json!({"observations":input.len(),"exact_body":exact_body,"exact_head":exact_head,"cases":rows});
    if let Ok(path) = std::env::var("STREET_MODEL_SCALES_REPORT") {
        std::fs::write(path, serde_json::to_string_pretty(&result).unwrap()).unwrap();
    }
    eprintln!(
        "original guarded model scales: observations={} body={exact_body} head={exact_head}",
        input.len()
    );
    assert_eq!(exact_body, input.len(), "guarded body cache mismatch");
    assert_eq!(exact_head, input.len(), "guarded head cache mismatch");
}
