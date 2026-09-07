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
#[serde(deny_unknown_fields)]
struct BondsRequest {
    atoms: Vec<crate::state::Atom>,
    cutoff: Value,
}

/// Derive bonds from geometry. Input `{ atoms: [Atom], cutoff: Value }`,
/// output `[[i, j], ...]`.
#[wasm_bindgen]
pub fn derive_bonds(request_json: &str) -> Result<String, JsValue> {
    let req: BondsRequest = serde_json::from_str(request_json).map_err(to_js)?;
    let bonds = huckel::derive_bonds(&req.atoms, &req.cutoff);
    let response: Vec<[usize; 2]> = bonds.into_iter().map(|(i, j)| [i, j]).collect();
    serde_json::to_string(&response).map_err(to_js)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BondedRequest {
    atoms: Vec<crate::state::Atom>,
    refs: Vec<crate::bonding::BondLength>,
    valences: std::collections::HashMap<String, f64>,
    #[serde(default)]
    fallback_single: Option<f64>,
}

/// Derive bonds with orders from geometry. Input
/// `{ atoms, refs: [BondLength], valences: {symbol: n}, fallback_single? }`,
/// output `[[a, b]] | [[a, b, "order"]]`.
#[wasm_bindgen]
pub fn derive_bonded(request_json: &str) -> Result<String, JsValue> {
    let req: BondedRequest = serde_json::from_str(request_json).map_err(to_js)?;
    let table = crate::bonding::index_lengths(&req.refs);
    let bonds = crate::bonding::derive_bonded(
        &req.atoms,
        &table,
        &req.valences,
        req.fallback_single.unwrap_or(1.6),
    );
    serde_json::to_string(&bonds).map_err(to_js)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HuckelRequest {
    n_atoms: usize,
    bonds: Vec<[usize; 2]>,
    #[serde(default)]
    orders: Vec<String>,
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
    let bonds: Vec<(usize, usize, f64)> = if req.orders.is_empty() {
        req.bonds.iter().map(|b| (b[0], b[1], 1.0)).collect()
    } else {
        if req.orders.len() != req.bonds.len() {
            return Err(to_js(format!(
                "orders length {} != bonds length {}",
                req.orders.len(),
                req.bonds.len()
            )));
        }
        req.bonds
            .iter()
            .zip(&req.orders)
            .map(|(b, o)| {
                o.parse::<f64>()
                    .map(|w| (b[0], b[1], w))
                    .map_err(|e| to_js(format!("invalid bond order {o:?}: {e}")))
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    let solution = huckel::simple_huckel_weighted(req.n_atoms, &bonds, &req.alpha, &req.beta, req.electrons)
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
