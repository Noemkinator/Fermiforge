//! Group F: WASM endpoint tests in a real browser (PLAN.md § 8.6).
//! Run with `wasm-pack test --headless --chrome core` (CI wasm-tests job).

#![cfg(target_arch = "wasm32")]

use fermiforge_core::wasm::{
    atom_orbital, decode_scene_fragment, encode_scene_fragment, solve_atom_levels,
    solve_simple_huckel,
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

/// Finite 2pF nucleus lifts the muonic-lead 1s level (binding reduced):
/// first-order shift is positive and 2..50 % of the point binding, matching
/// the core-level `heavy_muonic_finite_nucleus_reduces_binding` anchor
/// (Angeli & Marinova rms 5.5009 fm).
#[wasm_bindgen_test]
fn finite_nucleus_shifts_muonic_lead_1s() {
    let request = r#"{
        "z": 82,
        "mass": {"value": 105.6583755, "source": "test", "edition": "1"},
        "nMax": 1,
        "numeric": true,
        "nucleus": "finite",
        "rms": {"value": 5.5009, "source": "test", "edition": "1"}
    }"#;
    let response: serde_json::Value =
        serde_json::from_str(&solve_atom_levels(request).expect("solve")).unwrap();
    let levels = response["levels"].as_array().unwrap();
    let binding = levels[0]["binding"]["value"].as_f64().unwrap();
    let shift = levels[0]["shift"]["value"].as_f64().unwrap();
    assert!(
        shift > 0.0,
        "finite nucleus must reduce binding, shift {shift}"
    );
    // fraction of the POINT binding (binding + shift = m - E_point)
    let frac = shift / (binding + shift);
    assert!((0.02..0.6).contains(&frac), "shift fraction {frac}");
    assert_eq!(response["nucleus"]["shape"], "2pf");
    assert!(response["method"].as_str().unwrap().contains("+2pf"));
}

/// Reduced-mass hydrogen Lyman-alpha (2p1/2 -> 1s1/2) at the NIST value
/// 121.567 nm; the effective mass must sit below the electron rest mass.
#[wasm_bindgen_test]
fn reduced_mass_hydrogen_lyman_alpha_matches_nist() {
    let request = r#"{
        "z": 1,
        "mass": {"value": 0.51099895069, "source": "test", "edition": "1"},
        "nMax": 2,
        "reducedMass": true,
        "massNumber": 1,
        "proton": {"value": 938.27208816, "source": "test", "edition": "1"},
        "neutron": {"value": 939.56542194, "source": "test", "edition": "1"}
    }"#;
    let response: serde_json::Value =
        serde_json::from_str(&solve_atom_levels(request).expect("solve")).unwrap();
    let m_eff = response["effectiveMass"]["value"].as_f64().unwrap();
    // mu = m_e / (1 + m_e/m_p) = 0.51072080 MeV
    assert!((m_eff - 0.510_720_8).abs() < 1e-6, "mu={m_eff} MeV");
    let lines = response["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 1);
    let lambda = lines[0]["lambdaNm"]["value"].as_f64().unwrap();
    // NIST ASD: H I Ly-alpha 1215.6701 Å = 121.56701 nm
    assert!((lambda - 121.567).abs() < 0.01, "lambda {lambda} nm");
    assert!(
        response["method"]
            .as_str()
            .unwrap()
            .contains("+reduced-mass")
    );
}

/// Uehling validation through the boundary: electronic-hydrogen 2S shift
/// against the published -1.122e-7 eV (Wikipedia / Greiner & Reinhardt),
/// plus input validation for the flag.
#[wasm_bindgen_test]
fn uehling_shift_and_validation_through_wasm() {
    let request = r#"{
        "z": 1,
        "mass": {"value": 0.51099895069, "source": "test", "edition": "1"},
        "nMax": 2,
        "uehling": true,
        "electronMass": {"value": 0.51099895069, "source": "test", "edition": "1"}
    }"#;
    let response: serde_json::Value =
        serde_json::from_str(&solve_atom_levels(request).expect("solve")).unwrap();
    let levels = response["levels"].as_array().unwrap();
    // nMax=2 list order: (1,-1), (2,-1), (2,1) — the 2S (kappa=-1) row:
    let s2 = &levels[1];
    assert_eq!(s2["n"], 2);
    assert_eq!(s2["kappa"], -1);
    let shift = s2["shift"]["value"].as_f64().unwrap();
    // -1.122e-7 eV = -1.122e-13 MeV, 3 % (core anchor)
    assert!(
        (shift - (-1.122e-13)).abs() < 3.4e-15,
        "Uehling 2S shift {shift} MeV"
    );
    // point nucleus reports exact zero shift
    let point = r#"{
        "z": 1,
        "mass": {"value": 0.51099895069, "source": "test", "edition": "1"},
        "nMax": 1
    }"#;
    let point: serde_json::Value =
        serde_json::from_str(&solve_atom_levels(point).expect("solve")).unwrap();
    assert_eq!(point["levels"][0]["shift"]["value"], 0.0);
    // uehling without electronMass is an error, not a panic
    let bad = r#"{
        "z": 1,
        "mass": {"value": 0.51099895069, "source": "test", "edition": "1"},
        "nMax": 1,
        "uehling": true
    }"#;
    assert!(solve_atom_levels(bad).is_err());
}

/// Orbital density endpoint: hydrogen 1s peak density at the nucleus equals
/// the analytic (1/a)^3/pi, and the 2p most-probable radius is 4a
/// (Bethe & Salpeter 1957 §3.1-3.2).
#[wasm_bindgen_test]
fn orbital_density_matches_analytic_anchors() {
    let request = r#"{
        "z": 1,
        "mass": {"value": 0.51099895069, "source": "test", "edition": "1"},
        "n": 1,
        "kappa": -1,
        "size": 64
    }"#;
    let res: serde_json::Value =
        serde_json::from_str(&atom_orbital(request).expect("orbital")).unwrap();
    assert_eq!(res["l"], 0);
    let a = res["aFm"]["value"].as_f64().unwrap();
    assert!((a - 52_918.0).abs() / 52_918.0 < 1e-4);
    let density = res["density"].as_array().unwrap();
    assert_eq!(density.len(), 64 * 64);
    let max = density
        .iter()
        .map(|v| v.as_f64().unwrap())
        .fold(0.0f64, f64::max);
    let exact = 1.0 / (a * a * a * std::f64::consts::PI);
    // grid half-width is extent = 10a, cell centres at (i+0.5)·20a/64; the
    // nearest one sits r = sqrt(2)·10a/64 = 0.221a off the nucleus and the
    // 1s density falls as e^-2r/a (Bethe & Salpeter 1957 §3.1)
    let expected = exact * (-2.0 * (2.0f64).sqrt() * 10.0 / 64.0).exp();
    assert!(
        max < exact && (max - expected).abs() / expected < 0.05,
        "max {max} expected {expected} exact {exact}"
    );

    let p2 = r#"{
        "z": 1,
        "mass": {"value": 0.51099895069, "source": "test", "edition": "1"},
        "n": 2,
        "kappa": 1,
        "size": 32
    }"#;
    let p2: serde_json::Value = serde_json::from_str(&atom_orbital(p2).expect("orbital")).unwrap();
    assert_eq!(p2["l"], 1);
    let peak = p2["peakRfm"]["value"].as_f64().unwrap();
    assert!(
        (peak - 4.0 * a).abs() / a < 0.02,
        "2p peak {peak} vs {}",
        4.0 * a
    );
    // bad kappa is an error, not a panic
    let bad = r#"{
        "z": 1,
        "mass": {"value": 0.51099895069, "source": "test", "edition": "1"},
        "n": 1,
        "kappa": 1
    }"#;
    assert!(atom_orbital(bad).is_err());
}
