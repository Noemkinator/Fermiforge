//! Bond perception tests: distance -> order, valence clamp, implicit H
//! (PLAN.md § 8.6 group B).

use std::collections::HashMap;

use fermiforge_core::bonding::{BondLength, derive_bonded, implicit_h, index_lengths};
use fermiforge_core::state::{Atom, Bond};

fn refs() -> Vec<BondLength> {
    serde_json::from_str(
        r#"[
        {"a":"C","b":"C","single":1.54,"double":1.34,"triple":1.20,"aromatic":1.39},
        {"a":"C","b":"O","single":1.43,"double":1.23},
        {"a":"C","b":"H","single":1.09},
        {"a":"C","b":"Cl","single":1.77},
        {"a":"H","b":"H","single":0.74}
    ]"#,
    )
    .expect("refs json")
}

fn valences() -> HashMap<String, f64> {
    serde_json::from_str(r#"{"C":4,"H":1,"O":2,"Cl":1}"#).expect("valences json")
}

fn atom(symbol: &str, x: f64, y: f64) -> Atom {
    Atom {
        symbol: symbol.into(),
        x,
        y,
        z: 0.0,
    }
}

fn orders(atoms: &[Atom], table: &std::collections::HashMap<String, BondLength>) -> Vec<f64> {
    derive_bonded(atoms, table, &valences(), 1.6)
        .iter()
        .map(|b| b.order)
        .collect()
}

#[test]
fn single_double_triple_from_distance() {
    let table = index_lengths(&refs());
    let ethane = [atom("C", 0.0, 0.0), atom("C", 1.54, 0.0)];
    assert_eq!(orders(&ethane, &table), vec![1.0]);
    let ethene = [atom("C", 0.0, 0.0), atom("C", 1.34, 0.0)];
    assert_eq!(orders(&ethene, &table), vec![2.0]);
    let ethyne = [atom("C", 0.0, 0.0), atom("C", 1.20, 0.0)];
    assert_eq!(orders(&ethyne, &table), vec![3.0]);
}

#[test]
fn benzene_ring_is_aromatic_with_implicit_hydrogens() {
    let atoms: Vec<Atom> = (0..6)
        .map(|k| {
            let a = std::f64::consts::PI / 3.0 * k as f64;
            atom(
                "C",
                (1.39 * a.cos() * 100.0).round() / 100.0,
                (1.39 * a.sin() * 100.0).round() / 100.0,
            )
        })
        .collect();
    let table = index_lengths(&refs());
    let bonds = derive_bonded(&atoms, &table, &valences(), 1.6);
    assert_eq!(bonds.len(), 6);
    assert!(
        bonds.iter().all(|b| b.order == 1.5),
        "all aromatic: {bonds:?}"
    );
    assert_eq!(implicit_h(&atoms, &bonds, &valences()), vec![1; 6]);
}

#[test]
fn valence_clamp_repairs_hypervalent_carbon() {
    // central C at double-bond distance from three carbons: 2+2+2 > 4
    let atoms = [
        atom("C", 0.0, 0.0),
        atom("C", 1.34, 0.0),
        atom("C", -0.67, 1.16),
        atom("C", -0.67, -1.16),
    ];
    let table = index_lengths(&refs());
    let bonds = derive_bonded(&atoms, &table, &valences(), 1.6);
    let sum: f64 = bonds
        .iter()
        .filter(|b| b.a == 0 || b.b == 0)
        .map(|b| b.order)
        .sum();
    assert!(
        sum <= 4.0 + 1e-9,
        "central carbon valence respected: {bonds:?}"
    );
    assert!(
        bonds.iter().any(|b| b.order > 1.0),
        "pi bonding kept where geometry allows"
    );
}

#[test]
fn pair_specific_cutoff_catches_long_bonds() {
    // C-Cl at 1.77 A must bond although the global fallback cutoff is 1.6
    let atoms = [atom("C", 0.0, 0.0), atom("Cl", 1.77, 0.0)];
    let table = index_lengths(&refs());
    let bonds = derive_bonded(&atoms, &table, &valences(), 1.6);
    assert_eq!(bonds.len(), 1);
    assert_eq!(bonds[0].order, 1.0);
    assert_eq!(implicit_h(&atoms, &bonds, &valences()), vec![3, 0]);
}

#[test]
fn unknown_symbols_use_fallback_and_no_valence_clamp() {
    let atoms = [atom("Xx", 0.0, 0.0), atom("Xx", 1.50, 0.0)];
    let table = index_lengths(&refs());
    let bonds = derive_bonded(&atoms, &table, &valences(), 1.6);
    assert_eq!(bonds.len(), 1);
    assert_eq!(bonds[0].order, 1.0);
    assert_eq!(implicit_h(&atoms, &bonds, &valences()), vec![0, 0]);
}

#[test]
fn isolated_atom_is_a_radical_not_a_hydride() {
    let atoms = vec![
        Atom {
            symbol: "C".into(),
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        Atom {
            symbol: "C".into(),
            x: 1.54,
            y: 0.0,
            z: 0.0,
        },
        Atom {
            symbol: "C".into(),
            x: 0.0,
            y: 9.0,
            z: 0.0,
        },
    ];
    let bonds = vec![Bond::single(0, 1)];
    let mut valences = HashMap::new();
    valences.insert("C".to_string(), 4.0);
    assert_eq!(implicit_h(&atoms, &bonds, &valences), vec![3, 3, 0]);
}

#[test]
fn five_single_bonds_on_carbon_break_one() {
    let table = index_lengths(&refs());
    let center = atom("C", 0.0, 0.0);
    let neighbors: Vec<Atom> = (0..5)
        .map(|i| {
            let a = std::f64::consts::PI * 2.0 * (i as f64) / 5.0;
            atom("C", a.cos() * 1.54, a.sin() * 1.54)
        })
        .collect();
    let mut atoms = vec![center];
    atoms.extend(neighbors);
    let bonds = derive_bonded(&atoms, &table, &valences(), 1.6);
    let sum: f64 = bonds
        .iter()
        .filter(|b| b.a == 0 || b.b == 0)
        .map(|b| b.order)
        .sum();
    assert!(
        sum <= 4.0 + 1e-9,
        "central carbon sum {sum} exceeds valence 4"
    );
    assert_eq!(bonds.len(), 4, "one of the five spokes must break");
}
