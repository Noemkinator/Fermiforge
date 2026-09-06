//! Hückel validation against analytical solutions (PLAN.md § 8.1).

use fermiforge_core::huckel::{HuckelError, solve_generalized};
use fermiforge_core::value::Value;

fn alpha() -> Value {
    Value::from_source(
        0.0,
        Some(0.05),
        "test",
        "1",
        None,
        None,
        Some("huckel/alpha"),
    )
    .unwrap()
}

fn beta() -> Value {
    Value::from_source(
        -1.0,
        Some(0.1),
        "test",
        "1",
        None,
        None,
        Some("huckel/beta"),
    )
    .unwrap()
}

fn benzene_bonds() -> Vec<(usize, usize)> {
    (0..6).map(|i| (i, (i + 1) % 6)).collect()
}

/// § 8.1: benzene spectrum vs analytic alpha + k*beta, k in {2,1,1,-1,-1,-2},
/// tolerance 0.01 |beta|.
#[test]
fn benzene_spectrum_matches_analytic() {
    let sol = fermiforge_core::huckel::simple_huckel(6, &benzene_bonds(), &alpha(), &beta(), 6)
        .expect("benzene");
    let expected = [2.0, 1.0, 1.0, -1.0, -1.0, -2.0]; // k values, ascending energy with beta<0
    for (e, k) in sol.energies.iter().zip(expected) {
        let analytic = -k;
        assert!(
            (e.value - analytic).abs() < 0.01,
            "energy {e:?} vs analytic {analytic}"
        );
    }
}

#[test]
fn benzene_homo_lumo_and_gap() {
    let sol = fermiforge_core::huckel::simple_huckel(6, &benzene_bonds(), &alpha(), &beta(), 6)
        .expect("benzene");
    assert_eq!(sol.homo, Some(2)); // degenerate pair indices 1..=2
    assert_eq!(sol.lumo, Some(3));
    assert!((sol.gap().unwrap() - 2.0).abs() < 0.01); // 2 |beta|
    assert!((sol.energies[1].value - sol.energies[2].value).abs() < 1e-9); // degeneracy
}

/// § 8.1 aromaticity: total pi energy of benzene is 8 alpha + 8 beta;
/// delocalization energy vs three isolated ethylenes is exactly 2 |beta|.
#[test]
fn benzene_aromaticity_delocalization_energy() {
    let (a, b) = (alpha(), beta());
    let benzene = fermiforge_core::huckel::simple_huckel(6, &benzene_bonds(), &a, &b, 6).unwrap();
    assert!((benzene.total_energy() - 8.0 * b.value).abs() < 0.02); // 8 alpha + 8 beta

    let ethylene = fermiforge_core::huckel::simple_huckel(2, &[(0, 1)], &a, &b, 2).unwrap();
    let three_ethylenes = 3.0 * ethylene.total_energy();
    let delocalization = benzene.total_energy() - three_ethylenes;
    assert!(
        (delocalization - 2.0 * b.value).abs() < 0.02,
        "resonance energy {delocalization} expected 2 beta"
    );
}

/// § 8.1 MO symmetry: the lowest MO of benzene is the uniform 1/sqrt(6)
/// combination; the total pi density is 1.0 electron per carbon (aromatic).
#[test]
fn benzene_mo_symmetry() {
    let sol = fermiforge_core::huckel::simple_huckel(6, &benzene_bonds(), &alpha(), &beta(), 6)
        .expect("benzene");
    let lowest = &sol.coefficients[0];
    let uniform = 1.0 / 6.0_f64.sqrt();
    for &c in lowest {
        assert!(
            (c.abs() - uniform).abs() < 1e-6,
            "lowest MO not uniform: {lowest:?}"
        );
    }
    // Total pi density must be exactly 1.0 electron per carbon (aromatic);
    // unlike individual coefficients this is invariant to the basis chosen
    // in the degenerate HOMO pair.
    let homo = sol.homo.expect("homo");
    for atom in 0..6 {
        let density: f64 = (0..=homo)
            .map(|mo| 2.0 * sol.coefficients[mo][atom].powi(2))
            .sum();
        assert!(
            (density - 1.0).abs() < 1e-6,
            "pi density on carbon {atom} = {density}, expected 1.0"
        );
    }
}

/// Generalized solver against the analytic 2x2 result eps+- = (a +- b)/(1 +- s).
#[test]
fn generalized_solver_matches_analytic_2x2() {
    let (a, b, s) = (3.0, 1.0, 0.25);
    let h = vec![vec![a, b], vec![b, a]];
    let sm = vec![vec![1.0, s], vec![s, 1.0]];
    let provenance =
        |e: f64| Value::from_source(e, None, "test", "1", None, None, Some("gen/eps")).unwrap();
    let sol = solve_generalized(&h, &sm, 2, &provenance).expect("2x2");
    // (1,-1) eigenvector: eps = (a-b)/(1-s); (1,1): eps = (a+b)/(1+s)
    let e_minus = (a - b) / (1.0 - s);
    let e_plus = (a + b) / (1.0 + s);
    assert!((sol.energies[0].value - e_minus).abs() < 1e-9);
    assert!((sol.energies[1].value - e_plus).abs() < 1e-9);
    // S-orthonormality of the occupied MO: c^T S c = 1
    let c = &sol.coefficients[0];
    let norm = c[0] * c[0] + c[1] * c[1] + 2.0 * s * c[0] * c[1];
    assert!((norm - 1.0).abs() < 1e-9);
}

#[test]
fn non_positive_definite_overlap_is_rejected() {
    let h = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
    let s = vec![vec![1.0, 1.0], vec![1.0, 1.0]]; // singular
    let provenance = |e: f64| Value::from_source(e, None, "t", "1", None, None, None).unwrap();
    assert_eq!(
        solve_generalized(&h, &s, 2, &provenance).err(),
        Some(HuckelError::NotPositiveDefinite)
    );
}

/// § 13.3: energies are provenance-tracked derivations of alpha and beta.
#[test]
fn energies_carry_provenance_from_parameters() {
    let sol = fermiforge_core::huckel::simple_huckel(6, &benzene_bonds(), &alpha(), &beta(), 6)
        .expect("benzene");
    let e0 = &sol.energies[0];
    assert_eq!(e0.source, "derived");
    assert_eq!(
        e0.method.as_deref(),
        Some("simple-huckel/linear-in-alpha-beta")
    );
    assert_eq!(
        e0.derived_from,
        vec!["huckel/alpha".to_string(), "huckel/beta".to_string()]
    );
    // u(eps)^2 = u(alpha)^2 + k^2 u(beta)^2 ; for k = 2: sqrt(0.05^2 + 4*0.1^2)
    let expected_u = (0.05_f64.powi(2) + 4.0 * 0.1_f64.powi(2)).sqrt();
    let u = e0.uncertainty.expect("propagated");
    assert!((u - expected_u).abs() < 1e-3);
}

#[test]
fn too_many_electrons_is_rejected() {
    assert_eq!(
        fermiforge_core::huckel::simple_huckel(2, &[(0, 1)], &alpha(), &beta(), 5).err(),
        Some(HuckelError::InvalidElectronCount {
            electrons: 5,
            orbitals: 2
        })
    );
}

#[test]
fn determinism_same_input_same_output() {
    let run = || {
        fermiforge_core::huckel::simple_huckel(6, &benzene_bonds(), &alpha(), &beta(), 6)
            .unwrap()
            .energies
            .iter()
            .map(|e| e.value.to_bits())
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
}
