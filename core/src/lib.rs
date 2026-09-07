//! Fermiforge physics core.
//!
//! All physics lives here: no DOM, no WebGL, no networking.
//! No concrete physical constant may appear in this crate outside of tests —
//! every value arrives from `data/` through the provenance-tracked
//! [`value::Value`] type (PLAN.md § 13.2–13.3).

pub mod dirac_atom;
pub mod huckel;
pub mod state;
pub mod value;

#[cfg(target_arch = "wasm32")]
pub mod wasm;
