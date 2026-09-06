//! Scene state round-trip and robustness tests (PLAN.md § 8.6).
//! Groups A (basic round-trip), B (deterministic serialization), D (robustness).

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use fermiforge_core::state::{Atom, MAX_ATOMS, Mode, Override, Scene, StateError};

fn craft(json: &str) -> String {
    let deflated = miniz_oxide::deflate::compress_to_vec(json.as_bytes(), 9);
    format!("v1.{}", URL_SAFE_NO_PAD.encode(&deflated))
}

fn benzene() -> Scene {
    let atoms = (0..6)
        .map(|k| {
            let angle = std::f64::consts::FRAC_PI_3 * k as f64;
            Atom {
                symbol: "C".to_string(),
                x: 1.39 * angle.cos(),
                y: 1.39 * angle.sin(),
                z: 0.0,
            }
        })
        .collect();
    Scene {
        atoms,
        ..Scene::default()
    }
}

fn muonic_lead() -> Scene {
    Scene {
        mode: Mode::Atom,
        nucleus_z: Some(82),
        lepton: Some("muon".to_string()),
        overrides: vec![Override {
            key: "constants/alpha".to_string(),
            value: 0.007_297_352_56,
        }],
        ..Scene::default()
    }
}

// ---------- Group A: basic round-trip ----------

#[test]
fn roundtrip_empty_scene() {
    let scene = Scene::default();
    let encoded = scene.encode().expect("encode");
    let decoded = Scene::from_url_fragment(&encoded).expect("decode");
    assert_eq!(scene, decoded);
}

#[test]
fn roundtrip_huckel_scene() {
    let scene = benzene();
    let encoded = scene.to_url_fragment().expect("encode");
    let decoded = Scene::from_url_fragment(&encoded).expect("decode");
    assert_eq!(scene.quantized(), decoded);
}

#[test]
fn roundtrip_atom_scene() {
    let scene = muonic_lead();
    let encoded = scene.encode().expect("encode");
    let decoded = Scene::from_url_fragment(&encoded).expect("decode");
    assert_eq!(scene, decoded);
}

#[test]
fn quantization_is_stable_under_roundtrip() {
    let scene = Scene {
        atoms: vec![Atom {
            symbol: "Pb".to_string(),
            x: 1.234_567_89,
            y: -0.005,
            z: 9.999_999,
        }],
        ..Scene::default()
    };
    let once = Scene::from_url_fragment(&scene.encode().unwrap()).unwrap();
    let twice = Scene::from_url_fragment(&once.encode().unwrap()).unwrap();
    assert_eq!(once, twice);
}

#[test]
fn float_override_roundtrips_bit_exact() {
    // Regression: serde_json float parsing is off by 1 ulp on some values
    // (e.g. the shortest repr "1.3900000000000001"); the exact_float codec
    // must keep such values bit-identical (PLAN.md § 13.10).
    let tricky: f64 = "1.3900000000000001".parse().expect("parses");
    let scene = Scene {
        overrides: vec![Override {
            key: "constants/alpha".to_string(),
            value: tricky,
        }],
        ..Scene::default()
    };
    let decoded = Scene::from_url_fragment(&scene.encode().unwrap()).expect("decode");
    assert_eq!(
        decoded.overrides[0].value.to_bits(),
        tricky.to_bits(),
        "override value changed bits"
    );
}

// ---------- Group B: deterministic serialization ----------

#[test]
fn serialization_is_deterministic() {
    let scene = benzene();
    assert_eq!(scene.encode().unwrap(), scene.encode().unwrap());
    assert_eq!(
        scene.canonical_json().unwrap(),
        scene.canonical_json().unwrap()
    );
}

#[test]
fn key_order_is_canonical() {
    let scene = Scene {
        atoms: benzene().atoms,
        charge: 1,
        ..muonic_lead()
    };
    let json = scene.canonical_json().expect("canonical json");
    let keys = [
        "\"atoms\"",
        "\"charge\"",
        "\"lepton\"",
        "\"mode\"",
        "\"nucleus_z\"",
        "\"overrides\"",
        "\"schema\"",
    ];
    let mut last = 0usize;
    for key in keys {
        let at = json
            .find(key)
            .unwrap_or_else(|| panic!("missing {key} in {json}"));
        assert!(at > last, "keys not sorted in {json}");
        last = at;
    }
}

#[test]
fn defaults_are_omitted() {
    let json = Scene::default().canonical_json().expect("canonical json");
    assert!(
        json.len() < 80,
        "default scene JSON too long ({})",
        json.len()
    );
}

// ---------- Group D: robustness and limits ----------

#[test]
fn rejects_unknown_version() {
    assert_eq!(
        Scene::from_url_fragment("v2.abc"),
        Err(StateError::UnknownVersion("v2".into()))
    );
    assert_eq!(
        Scene::from_url_fragment("vx.abc"),
        Err(StateError::UnknownVersion("vx".into()))
    );
    assert_eq!(
        Scene::from_url_fragment("no-dot-here"),
        Err(StateError::Corrupted("missing version prefix".into()))
    );
}

#[test]
fn rejects_corrupted_payload() {
    for input in [
        "",
        "v1.",
        "v1.!!!not-base64!!!",
        "v1.YWJjZA", // valid base64, not deflate
        &craft("not json at all"),
        &craft("{}"),                           // missing schema
        &craft(r#"{"schema":1,"surprise":1}"#), // unknown field
        &craft(r#"{"schema":1,"atoms":[{"symbol":"C","x":null}]}"#),
    ] {
        let result = Scene::from_url_fragment(input);
        assert!(
            matches!(result, Err(StateError::Corrupted(_))),
            "expected Corrupted for {input:?}, got {result:?}"
        );
    }
}

#[test]
fn rejects_oversized_state() {
    let mut scene = Scene::default();
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((seed >> 33) as f64 / (1u64 << 31) as f64) * 200.0 - 100.0
    };
    scene.atoms = (0..MAX_ATOMS)
        .map(|i| Atom {
            symbol: ["H", "C", "N", "O", "Si", "Cl", "Br", "Fe"][i % 8].to_string(),
            x: next(),
            y: next(),
            z: next(),
        })
        .collect();
    match scene.encode() {
        Err(StateError::TooLarge { bytes, limit }) => assert!(bytes > limit),
        other => panic!("expected TooLarge, got {other:?}"),
    }
}

#[test]
fn url_input_is_validated_against_limits() {
    // Hostile scene with far more atoms than allowed, bypassing encode limits.
    let mut json = String::from(r#"{"schema":1,"atoms":["#);
    for i in 0..99_999 {
        if i > 0 {
            json.push(',');
        }
        json.push_str(r#"{"symbol":"H","x":0,"y":0,"z":0}"#);
    }
    json.push_str("]}");
    let result = Scene::from_url_fragment(&craft(&json));
    assert!(
        matches!(result, Err(StateError::TooLarge { .. })),
        "oversized decompressed input must be rejected, got {result:?}"
    );

    // Just above the atom limit but under the JSON budget: limit error.
    let mut json = String::from(r#"{"schema":1,"atoms":["#);
    for i in 0..MAX_ATOMS + 1 {
        if i > 0 {
            json.push(',');
        }
        json.push_str(r#"{"symbol":"H","x":0,"y":0,"z":0}"#);
    }
    json.push_str("]}");
    let result = Scene::from_url_fragment(&craft(&json));
    assert_eq!(
        result.err(),
        Some(StateError::LimitExceeded {
            field: "atoms",
            count: MAX_ATOMS + 1,
            max: MAX_ATOMS
        })
    );
}

#[test]
fn rejects_invalid_fields() {
    for json in [
        r#"{"schema":1,"atoms":[{"symbol":"Xx1","x":0,"y":0,"z":0}]}"#,
        r#"{"schema":1,"atoms":[{"symbol":"","x":0,"y":0,"z":0}]}"#,
        r#"{"schema":1,"atoms":[{"symbol":"C","x":200000000,"y":0,"z":0}]}"#,
        r#"{"schema":1,"lepton":"hadron!"}"#,
        r#"{"schema":1,"overrides":[{"key":"bad key!","value":"1.0"}]}"#,
    ] {
        let result = Scene::from_url_fragment(&craft(json));
        assert!(
            matches!(result, Err(StateError::InvalidField { .. })),
            "expected InvalidField for {json}, got {result:?}"
        );
    }
}

#[test]
fn readable_form_is_rejected_until_m3() {
    assert_eq!(
        Scene::from_url_fragment("#/atom?Z=82&lepton=muon"),
        Err(StateError::Unsupported(
            "readable query form (#/atom?...) is scheduled for M3"
        ))
    );
}
