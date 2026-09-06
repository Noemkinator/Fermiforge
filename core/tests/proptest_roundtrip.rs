//! Property-based round-trip tests (PLAN.md § 8.6 group C).
//! Ignored in daily CI; the nightly workflow runs them with PROPTEST_CASES=100000.

#![cfg(not(target_arch = "wasm32"))]

use fermiforge_core::state::{Atom, Mode, Override, Scene};
use proptest::option;
use proptest::prelude::*;
use proptest::sample::select;

fn arb_scene() -> impl Strategy<Value = Scene> {
    let symbol = select(vec!["H", "C", "N", "O", "Si", "Cl", "Br", "Fe"]);
    fn coord() -> impl Strategy<Value = f64> {
        -1000.0f64..1000.0
    }
    let atom = (symbol, coord(), coord(), coord()).prop_map(|(symbol, x, y, z)| Atom {
        symbol: symbol.to_string(),
        x,
        y,
        z,
    });
    let key = "[a-z][a-z0-9]{0,12}(/[a-z][a-z0-9]{0,12}){0,3}";
    let override_ = (key, -1.0e6f64..1.0e6).prop_map(|(key, value)| Override { key, value });
    (
        prop::collection::vec(atom, 0..60),
        prop::collection::vec(override_, 0..50),
        any::<bool>(),
        -5i32..5,
        option::of(select(vec!["electron", "muon"])),
        option::of(1u32..=118),
    )
        .prop_map(
            |(atoms, overrides, atom_mode, charge, lepton, nucleus_z)| Scene {
                schema: fermiforge_core::state::SCHEMA_VERSION,
                mode: if atom_mode { Mode::Atom } else { Mode::Huckel },
                atoms,
                bonds: Vec::new(),
                charge,
                overrides,
                lepton: lepton.map(str::to_string),
                nucleus_z,
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    #[ignore] // nightly: cargo test -- --ignored (PLAN.md § 10.1)
    fn roundtrip_random_scenes(scene in arb_scene()) {
        let encoded = scene.encode()?;
        let decoded = Scene::from_url_fragment(&encoded)?;
        prop_assert_eq!(scene.canonicalized(), decoded);
    }

    #[test]
    #[ignore]
    fn roundtrip_symmetric_in_overrides(mut scene in arb_scene()) {
        scene.overrides.reverse();
        let a = scene.encode()?;
        scene.overrides.sort_by(|x, y| x.key.cmp(&y.key));
        let b = scene.encode()?;
        prop_assert_eq!(a, b);
    }
}
