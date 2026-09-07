//! Bond perception: element-pair reference lengths -> bond orders, then a
//! valence clamp so hypervalent geometry is repaired (PLAN.md § 8.6 group B).
//!
//! Orders: 1.0 single, 1.5 aromatic, 2.0 double, 3.0 triple. The order is
//! inferred from the interatomic distance by nearest reference length; an
//! atom's total bond order never exceeds its standard valence.

use std::collections::HashMap;

use crate::state::{Atom, Bond};

/// Reference bond lengths (angstrom) for one unordered element pair.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BondLength {
    pub a: String,
    pub b: String,
    pub single: f64,
    #[serde(default)]
    pub double: Option<f64>,
    #[serde(default)]
    pub triple: Option<f64>,
    #[serde(default)]
    pub aromatic: Option<f64>,
}

pub type LengthTable = HashMap<String, BondLength>;

#[must_use]
pub fn pair_key(x: &str, y: &str) -> String {
    if x <= y {
        format!("{x}-{y}")
    } else {
        format!("{y}-{x}")
    }
}

#[must_use]
pub fn index_lengths(refs: &[BondLength]) -> LengthTable {
    refs.iter()
        .map(|r| (pair_key(&r.a, &r.b), r.clone()))
        .collect()
}

impl BondLength {
    fn reference(&self, order: f64) -> Option<f64> {
        if order == 1.0 {
            Some(self.single)
        } else if order == 1.5 {
            self.aromatic
        } else if order == 2.0 {
            self.double
        } else if order == 3.0 {
            self.triple
        } else {
            None
        }
    }

    fn orders(&self) -> [f64; 4] {
        [1.0, 1.5, 2.0, 3.0]
    }

    fn infer(&self, d: f64) -> f64 {
        let mut best = (1.0, (d - self.single).abs());
        for order in self.orders() {
            if let Some(len) = self.reference(order) {
                let err = (d - len).abs();
                if err < best.1 {
                    best = (order, err);
                }
            }
        }
        best.0
    }

    fn next_lower(&self, order: f64) -> Option<f64> {
        self.orders()
            .iter()
            .rev()
            .find(|o| **o < order - 1e-9 && self.reference(**o).is_some())
            .copied()
    }
}

fn distance(a: &Atom, b: &Atom) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

/// Derive bonds with orders from geometry: pair-aware cutoff (element-pair
/// single length x 1.12, else `fallback_single` x 1.12), nearest-reference
/// order inference, then valence clamping.
#[must_use]
pub fn derive_bonded(
    atoms: &[Atom],
    table: &LengthTable,
    valences: &HashMap<String, f64>,
    fallback_single: f64,
) -> Vec<Bond> {
    let mut bonds = Vec::new();
    for i in 0..atoms.len() {
        for j in (i + 1)..atoms.len() {
            let d = distance(&atoms[i], &atoms[j]);
            if d <= 1e-9 {
                continue;
            }
            let r = table.get(&pair_key(&atoms[i].symbol, &atoms[j].symbol));
            let single = r.map_or(fallback_single, |r| r.single);
            if d <= single * 1.12 + 1e-9 {
                let order = r.map_or(1.0, |r| r.infer(d));
                bonds.push(Bond {
                    a: i as u16,
                    b: j as u16,
                    order,
                });
            }
        }
    }
    clamp_valence(atoms, &mut bonds, table, valences);
    bonds
}

/// Downgrade bonds (least distance-penalty first) until every atom's summed
/// bond order fits its standard valence. Unknown symbols are unconstrained.
pub fn clamp_valence(
    atoms: &[Atom],
    bonds: &mut [Bond],
    table: &LengthTable,
    valences: &HashMap<String, f64>,
) {
    for _ in 0..128 {
        let mut sums = vec![0.0f64; atoms.len()];
        for b in bonds.iter() {
            sums[b.a as usize] += b.order;
            sums[b.b as usize] += b.order;
        }
        let Some(violator) = atoms
            .iter()
            .enumerate()
            .find(|(i, a)| valences.get(&a.symbol).is_some_and(|v| sums[*i] > v + 1e-9))
        else {
            return;
        };
        let ai = violator.0;
        let mut best: Option<(usize, f64)> = None;
        for (k, b) in bonds.iter().enumerate() {
            if (b.a as usize) != ai && (b.b as usize) != ai {
                continue;
            }
            let Some(r) = table.get(&pair_key(
                &atoms[b.a as usize].symbol,
                &atoms[b.b as usize].symbol,
            )) else {
                continue;
            };
            let Some(next) = r.next_lower(b.order) else {
                continue;
            };
            let d = distance(&atoms[b.a as usize], &atoms[b.b as usize]);
            let cur = r.reference(b.order).unwrap_or(f64::MAX);
            let penalty = (d - next).abs() - (d - cur).abs();
            if best.is_none_or(|(_, p)| penalty < p) {
                best = Some((k, penalty));
            }
        }
        let Some((k, _)) = best else {
            return; // cannot repair further (should not happen with single refs)
        };
        let b = bonds[k];
        let r = table
            .get(&pair_key(
                &atoms[b.a as usize].symbol,
                &atoms[b.b as usize].symbol,
            ))
            .expect("checked above");
        bonds[k].order = r.next_lower(b.order).expect("checked above");
    }
}

/// Implicit hydrogens per atom: valence minus summed bond order. Atoms
/// without any bond are treated as bare radicals (no implicit H) — a lone
/// carbon is C, not methane.
#[must_use]
pub fn implicit_h(atoms: &[Atom], bonds: &[Bond], valences: &HashMap<String, f64>) -> Vec<u32> {
    let mut sums = vec![0.0f64; atoms.len()];
    for b in bonds {
        sums[b.a as usize] += b.order;
        sums[b.b as usize] += b.order;
    }
    atoms
        .iter()
        .enumerate()
        .map(|(i, a)| {
            if sums[i] == 0.0 {
                return 0;
            }
            valences
                .get(&a.symbol)
                .map_or(0.0, |v| (v - sums[i]).max(0.0))
                .round() as u32
        })
        .collect()
}
