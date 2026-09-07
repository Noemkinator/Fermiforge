//! Group F: WASM endpoint tests in a real browser (PLAN.md § 8.6).
//! Run with `wasm-pack test --headless --chrome core` (CI wasm-tests job).

#![cfg(target_arch = "wasm32")]

use fermiforge_core::wasm::{
    decode_scene_fragment, encode_scene_fragment, solve_atom_levels, solve_simple_huckel,
};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn empty_scene_fragment_roundtrips_in_browser() {
    let json = r#"{"schema":3}"#;
    let fragment = encode_scene_fragment(json).expect("encode");
    assert!(fragment.starts_with("#/s=v3."));
    let back = decode_scene_fragment(&fragment).expect("decode");
    assert_eq!(back, json);
}

#[wasm_bindgen_test]
fn hostile_fragment_is_rejected_not_panicking() {
    assert!(decode_scene_fragment("#/s=v9.garbage").is_err());
    assert!(decode_scene_fragment("#/s=nonsense").is_err());
    assert!(decode_scene_fragment("").is_err());
}

#[wasm_bindgen_test]
fn benzene_solves_through_wasm_boundary() {
    let request = r#"{
        "nAtoms": 6,
        "bonds": [[0,1],[1,2],[2,3],[3,4],[4,5],[5,0]],
        "alpha": {"value": 0.0, "source": "test", "edition": "1"},
        "beta": {"value": -1.0, "source": "test", "edition": "1"},
        "electrons": 6
    }"#;
    let response: serde_json::Value =
        serde_json::from_str(&solve_simple_huckel(request).expect("solve")).unwrap();
    assert_eq!(response["homo"], 2);
    assert_eq!(response["lumo"], 3);
    assert!((response["gap"].as_f64().unwrap() - 2.0).abs() < 0.01);
    assert!((response["totalEnergy"].as_f64().unwrap() + 8.0).abs() < 0.02);
}

#[wasm_bindgen_test]
fn hydrogen_ground_state_through_wasm_matches_dirac() {
    let request = r#"{
        "z": 1,
        "mass": {"value": 0.51099895069, "source": "test", "edition": "1"},
        "nMax": 1
    }"#;
    let response: serde_json::Value =
        serde_json::from_str(&solve_atom_levels(request).expect("solve")).unwrap();
    let levels = response["levels"].as_array().unwrap();
    assert_eq!(levels.len(), 1);
    assert_eq!(levels[0]["kappa"], -1);
    let binding = levels[0]["binding"]["value"].as_f64().unwrap();
    // Dirac 1s binding energy of hydrogen: 13.6 eV
    assert!((binding - 1.36e-5).abs() < 1e-7, "binding {binding} MeV");
    assert_eq!(levels[0]["binding"]["source"], "derived");
}
