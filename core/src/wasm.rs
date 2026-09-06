//! WASM bindings for the web client (PLAN.md § 4: the web app is just
//! another client of the core). JSON strings cross the boundary so the
//! scene schema and provenance types stay the single source of truth.

use wasm_bindgen::prelude::*;

use crate::huckel;
use crate::state::Scene;
use crate::value::Value;
use serde::Deserialize;

fn to_js<E: std::fmt::Display>(e: E) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// Decode a shareable URL fragment (`#/s=v1....`) into canonical scene JSON.
#[wasm_bindgen]
pub fn decode_scene_fragment(fragment: &str) -> Result<String, JsValue> {
    let scene = Scene::from_url_fragment(fragment).map_err(to_js)?;
    scene.canonical_json().map_err(to_js)
}

/// Encode canonical scene JSON into a shareable URL fragment.
#[wasm_bindgen]
pub fn encode_scene_fragment(scene_json: &str) -> Result<String, JsValue> {
    let scene: Scene = serde_json::from_str(scene_json).map_err(to_js)?;
    scene.to_url_fragment().map_err(to_js)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HuckelRequest {
    n_atoms: usize,
    bonds: Vec<[usize; 2]>,
    alpha: Value,
    beta: Value,
    electrons: usize,
}

/// Solve a simple-Hückel pi system. Input:
/// `{ nAtoms, bonds: [[i, j]], alpha: Value, beta: Value, electrons }`.
/// Output: energies (provenance-tracked), S-orthonormal MO coefficients,
/// HOMO/LUMO indices, gap and total pi energy.
#[wasm_bindgen]
pub fn solve_simple_huckel(request_json: &str) -> Result<String, JsValue> {
    let req: HuckelRequest = serde_json::from_str(request_json).map_err(to_js)?;
    let bonds: Vec<(usize, usize)> = req.bonds.iter().map(|b| (b[0], b[1])).collect();
    let solution = huckel::simple_huckel(req.n_atoms, &bonds, &req.alpha, &req.beta, req.electrons)
        .map_err(to_js)?;
    let response = serde_json::json!({
        "energies": solution.energies,
        "coefficients": solution.coefficients,
        "electrons": solution.electrons,
        "homo": solution.homo,
        "lumo": solution.lumo,
        "gap": solution.gap(),
        "totalEnergy": solution.total_energy(),
    });
    serde_json::to_string(&response).map_err(to_js)
}
