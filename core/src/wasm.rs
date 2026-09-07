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
struct AtomLevelsRequest {
    z: u32,
    mass: Value,
    n_max: u32,
    #[serde(default)]
    numeric: bool,
    /// "point" (default) or "finite" (symmetric two-parameter Fermi, a = 0.52 fm;
    /// Hofstadter RMP 28 (1956) 214, Fricke et al. ADT 43 (1995) 71).
    #[serde(default)]
    nucleus: Option<String>,
    /// Diffuseness a (fm) override for the 2pF fold.
    #[serde(default)]
    a: Option<Value>,
    /// RMS charge radius (fm) — c is solved from it (solve_two_pf).
    #[serde(default)]
    rms: Option<Value>,
    /// Fallback uniform-sphere radius parameter r0 (fm): R = r0·A^(1/3)
    /// (Crane-style estimate; used only when `rms` is absent).
    #[serde(default)]
    r0: Option<Value>,
    #[serde(default)]
    reduced_mass: bool,
    #[serde(default)]
    proton: Option<Value>,
    #[serde(default)]
    neutron: Option<Value>,
    /// Electron mass (MeV) for the Uehling term (required when `uehling`).
    #[serde(default)]
    electron_mass: Option<Value>,
    /// Add the Uehling vacuum-polarization potential
    /// (Uehling, Phys. Rev. 48 (1935) 55).
    #[serde(default)]
    uehling: bool,
    /// Nucleon number A for the nucleus mass; defaults to round(2.02·Z).
    #[serde(default)]
    mass_number: Option<u32>,
}

/// hc in MeV·nm: h·c = 1239.8419843320026 eV·nm is EXACT in the SI-2019
/// unit system (h, c, e all exact); /1e6 for MeV. CODATA 2022.
const HC_MEV_NM: f64 = 1_239.841_984_332_002_6e-6;

/// Solve hydrogen-like Dirac levels with optional finite nuclear size,
/// Uehling vacuum polarization and reduced mass. Input
/// `{ z, mass: Value, nMax, numeric?, nucleus?, a?, rms?, r0?, reducedMass?,
/// proton?, neutron?, electronMass?, uehling? }` (masses in MeV, incl. rest).
/// Output `{ levels: [{ n, kappa, e, binding, shift }], lines: [{ from, to,
/// dE, lambdaNm }], nucleus: { shape, c, a, rms }, effectiveMass, method }`.
/// `shift` is the first-order perturbative energy shift from the
/// finite-size + Uehling terms; `lambdaNm` = hc/dE (emission wavelength).
#[wasm_bindgen]
pub fn solve_atom_levels(request_json: &str) -> Result<String, JsValue> {
    let req: AtomLevelsRequest = serde_json::from_str(request_json).map_err(to_js)?;
    if req.z == 0 || req.z > 137 {
        return Err(to_js("Z must be 1..=137 for a point nucleus"));
    }
    if req.mass.value <= 0.0 {
        return Err(to_js("lepton mass must be positive"));
    }
    let finite = match req.nucleus.as_deref() {
        None | Some("point") => false,
        Some("finite") => true,
        Some(other) => return Err(to_js(format!("unknown nucleus model '{other}'"))),
    };
    if req.uehling && req.electron_mass.is_none() {
        return Err(to_js("uehling requires electronMass"));
    }
    if req.reduced_mass && (req.proton.is_none() || req.neutron.is_none()) {
        return Err(to_js("reducedMass requires proton and neutron masses"));
    }

    // --- effective lepton mass (reduced mass of the two-body system) ------
    // Nucleus mass M = Z·m_p + N·m_n; binding energy (< 0.1 % for the
    // isotopes we ship) is neglected — documented approximation, no data.
    // Default A: natural hydrogen is 99.98 % H-1; elsewhere the valley of
    // stability A ≈ round(2.02·Z).
    let mass_number = req.mass_number.unwrap_or_else(|| {
        if req.z == 1 {
            1
        } else {
            (req.z as f64 * 2.02).round() as u32
        }
    });
    if mass_number < req.z {
        return Err(to_js("mass number A must be >= Z"));
    }
    let mut mass_method = String::from("lepton rest mass");
    let mut effective_mass = req.mass.clone();
    if req.reduced_mass {
        let a = mass_number;
        let n_nucleon = a - req.z;
        let mp = req.proton.as_ref().unwrap();
        let mn = req.neutron.as_ref().unwrap();
        let m_nucleus = Value::derive(
            &|x: &[f64]| req.z as f64 * x[0] + n_nucleon as f64 * x[1],
            &[mp, mn],
            &format!("nucleus mass Z·mp+N·mn, A={a} (binding neglected)"),
        )
        .map_err(to_js)?;
        effective_mass = Value::derive(
            &|x: &[f64]| crate::dirac_atom::reduced_mass(x[0], x[1]),
            &[&req.mass, &m_nucleus],
            "reduced mass m·M/(m+M) (no-recoil correction, Bransden & Joachain)",
        )
        .map_err(to_js)?;
        mass_method = format!("reduced-mass A={a}");
    }
    let mass = effective_mass.value;

    // --- charge distribution ----------------------------------------------
    // 2pF from the RMS radius when given (solve_two_pf bisection), else a
    // uniform-sphere estimate rms = sqrt(3/5)·r0·A^(1/3) (Bethe & Salpeter
    // 1957 §53) folded back into a 2pF half-density radius.
    let mut nucleus_json = serde_json::json!({ "shape": "point" });
    let mut dist = crate::dirac_atom::ChargeDist::Point;
    let mut method = String::from("dirac-coulomb");
    if finite {
        let a = match &req.a {
            Some(a) => a.clone(),
            None => Value::from_user(
                crate::dirac_atom::TWO_PF_A_FM,
                "2pF diffuseness default (Hofstadter 1956 / Fricke 1995)",
            )
            .map_err(to_js)?,
        };
        let a_val = a.value;
        if !(0.2..=1.5).contains(&a_val) {
            return Err(to_js("diffuseness a must be 0.2..=1.5 fm"));
        }
        let (c_val, rms_value, shape) = match &req.rms {
            Some(rms) => {
                if !(0.1..=20.0).contains(&rms.value) {
                    return Err(to_js("rms radius must be 0.1..=20 fm"));
                }
                let c = Value::derive(
                    &|x: &[f64]| crate::dirac_atom::solve_two_pf(x[0], a_val),
                    &[rms],
                    &format!("2pF c from rms, a={a_val} fm (Fricke 1995 shape)"),
                )
                .map_err(to_js)?;
                (c.value, rms.clone(), "2pf")
            }
            None => {
                let r0 = req
                    .r0
                    .clone()
                    .ok_or_else(|| to_js("finite nucleus needs rms or r0"))?;
                let a_rms = mass_number as f64;
                let r_rms = Value::derive(
                    &|x: &[f64]| (1.5f64 / 5.0f64).sqrt() * x[0] * a_rms.powf(1.0 / 3.0),
                    &[&r0],
                    &format!("uniform-sphere rms sqrt(3/5)·r0·A^(1/3), A={a_rms:.0}"),
                )
                .map_err(to_js)?;
                let c = Value::derive(
                    &|x: &[f64]| crate::dirac_atom::solve_two_pf(x[0], a_val),
                    &[&r_rms],
                    &format!("2pF c from rms, a={a_val} fm (Fricke 1995 shape)"),
                )
                .map_err(to_js)?;
                (c.value, r_rms, "2pf (r0 estimate)")
            }
        };
        if !(0.05..=20.0).contains(&c_val) {
            return Err(to_js("solved 2pF half-density c out of range"));
        }
        dist = crate::dirac_atom::ChargeDist::Fermi2p {
            c_fermi: c_val,
            a_fermi: a_val,
        };
        nucleus_json = serde_json::json!({
            "shape": shape, "c": c_val, "a": a, "rms": rms_value,
        });
        method.push_str("+2pf");
    }
    if req.uehling {
        method.push_str("+uehling");
    }
    if req.reduced_mass {
        method.push_str("+reduced-mass");
    }

    // --- potential table (only when needed) --------------------------------
    let cap = if req.numeric || finite { 3 } else { 6 };
    let n_max = req.n_max.clamp(1, cap);
    let ueh_m = if req.uehling {
        Some(req.electron_mass.as_ref().unwrap().value)
    } else {
        None
    };
    let pot = if finite || req.uehling {
        let za = req.z as f64 * crate::dirac_atom::ALPHA;
        // outer turning point of the n-th Bohr orbit with margin (see
        // dirac_atom::r_out rationale)
        let r_out =
            (crate::dirac_atom::HBARC / (za * mass) * (n_max * n_max) as f64 * 51.0).max(30.0);
        Some(crate::dirac_atom::build_potential(
            req.z, dist, ueh_m, r_out,
        ))
    } else {
        None
    };
    if pot.is_none() && req.numeric {
        method.push_str("-shooting");
    }

    // --- levels -------------------------------------------------------------
    // Shooting with the modified potential only for the finite nucleus, whose
    // shift can reach ~10 % of the binding (muonic Pb). A Uehling-only
    // correction is <= 0.1 meV against eV spacings: first-order perturbation
    // theory (expectation_shift) is exact to many digits and far more stable
    // than moving the shooting root.
    let solve = |m: f64, n: u32, kappa: i32| -> f64 {
        if finite {
            crate::dirac_atom::solve_level_opt(req.z, m, n, kappa, 120_000, pot.as_ref())
        } else if req.numeric {
            crate::dirac_atom::solve_level(req.z, m, n, kappa)
        } else {
            crate::dirac_atom::hydrogenic_energy(req.z, m, n, kappa)
        }
    };
    let levels: Vec<serde_json::Value> = crate::dirac_atom::level_list(n_max)
        .into_iter()
        .map(|(n, kappa)| -> Result<serde_json::Value, JsValue> {
            let e = solve(mass, n, kappa);
            // Dirac-Coulomb energies are exactly linear in the lepton mass,
            // so propagation needs no re-integration of the ODE. The 2pF /
            // Uehling corrections enter through `shift` (first-order PT)
            // instead — they are not linear in mass.
            let ratio = e / mass;
            let e_value = Value::derive(
                &|x: &[f64]| x[0] * ratio,
                &[&effective_mass],
                &format!("{method}/n={n},kappa={kappa}"),
            )
            .map_err(to_js)?;
            let binding = Value::derive(
                &|x: &[f64]| x[0] * (1.0 - ratio),
                &[&effective_mass],
                &format!("rest-mass-minus-{method}/n={n},kappa={kappa}"),
            )
            .map_err(to_js)?;
            // Shift relative to the point nucleus. For the finite case the
            // full re-solve is used (Z*alpha ~ 0.6 for muonic Pb invalidates
            // first-order PT: the wavefunction itself relaxes); for a
            // Uehling-only correction (<= 0.1 meV against eV spacings) the
            // first-order expectation value is exact to many digits.
            let shift = match (&pot, finite) {
                (Some(p), false) => {
                    let s = crate::dirac_atom::expectation_shift(req.z, mass, n, kappa, p, 60_000);
                    Value::derive(
                        &|x: &[f64]| x[0] * (s / mass),
                        &[&effective_mass],
                        &format!("first-order shift, {mass_method}"),
                    )
                    .map_err(to_js)?
                }
                (Some(_), true) => {
                    let s = e - crate::dirac_atom::hydrogenic_energy(req.z, mass, n, kappa);
                    Value::derive(
                        &|x: &[f64]| x[0] * (s / mass),
                        &[&effective_mass],
                        &format!("shift from full 2pF re-solve, {mass_method}"),
                    )
                    .map_err(to_js)?
                }
                (None, _) => {
                    Value::from_user(0.0, "point nucleus, no vacuum polarization").map_err(to_js)?
                }
            };
            Ok(serde_json::json!({
                "n": n, "kappa": kappa, "e": e_value, "binding": binding, "shift": shift,
            }))
        })
        .collect::<Result<Vec<_>, JsValue>>()?;

    // --- emission lines (n+1 -> n, kappa = -1 -> +1 members) ----------------
    // Wavelength lambda = hc/dE with hc EXACT in SI-2019 (CODATA 2022).
    // E1-allowed sharp-series members (n p1/2 -> n s1/2 etc., kappa -1 -> +1
    // is the allowed j-preserving branch); Dirac point energies are
    // l-degenerate so only the s1/2 lower level enters the difference.
    let mut lines: Vec<serde_json::Value> = Vec::new();
    for n_lo in 1..n_max {
        let e_lo = solve(mass, n_lo, -1);
        let e_hi = solve(mass, n_lo + 1, 1);
        let d_e = e_hi - e_lo;
        if d_e <= 0.0 {
            continue;
        }
        let d_e_value = Value::derive(
            &|x: &[f64]| x[0] * (d_e / mass),
            &[&effective_mass],
            &format!("transition {n_lo}..{n_lo}+1, {method}"),
        )
        .map_err(to_js)?;
        let lambda = Value::derive(
            &|x: &[f64]| HC_MEV_NM / x[0],
            &[&d_e_value],
            "lambda = hc/dE, hc exact SI-2019",
        )
        .map_err(to_js)?;
        lines.push(serde_json::json!({
            "from": { "n": n_lo + 1, "kappa": 1 },
            "to": { "n": n_lo, "kappa": -1 },
            "dE": d_e_value, "lambdaNm": lambda,
        }));
    }

    serde_json::to_string(&serde_json::json!({
        "levels": levels,
        "lines": lines,
        "nucleus": nucleus_json,
        "effectiveMass": effective_mass,
        "method": method,
    }))
    .map_err(to_js)
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
    let solution =
        huckel::simple_huckel_weighted(req.n_atoms, &bonds, &req.alpha, &req.beta, req.electrons)
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
