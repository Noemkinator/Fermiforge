//! Provenance-tracked physical values (PLAN.md § 7.1).
//!
//! Every physical number entering or leaving the core is a [`Value`]:
//! a float plus its uncertainty and full provenance (source, edition,
//! fetch date, URL, id). The only ways to build one are
//! [`Value::from_source`] (raw data), [`Value::derive`] (computed) and
//! [`Value::from_user`] (user overlay). A bare float in the public core
//! API is a bug (PLAN.md § 13.3).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors produced by value construction and derivation.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ValueError {
    /// A physical value without source/edition provenance is rejected
    /// ("no bare numbers", PLAN.md § 7.1).
    #[error("missing provenance: {0}")]
    MissingProvenance(String),
    /// Non-finite physical values are never allowed.
    #[error("non-finite value: {0}")]
    NonFinite(String),
    /// Derivation produced a non-finite result.
    #[error("derivation produced non-finite result (method: {method})")]
    NonFiniteResult { method: String },
}

/// A physical value with uncertainty and provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Value {
    /// The numeric value in the units implied by its `id`/source.
    pub value: f64,
    /// One-standard-deviation absolute uncertainty, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncertainty: Option<f64>,
    /// Data source, e.g. `"PDG"`, `"CODATA"`, `"user"`, `"derived"`.
    pub source: String,
    /// Source edition, e.g. `"2024"`.
    pub edition: String,
    /// Date the data were fetched, ISO 8601.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched: Option<String>,
    /// Resolvable URL of the original record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Canonical id, e.g. `"particles/up/mass_MeV"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Derivation method label (required for derived values),
    /// e.g. `"linear-propagation"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Ids (or `source/edition` labels) of the inputs this value derives from.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_from: Vec<String>,
    /// For user overlays: id of the original value the edit is based on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
}

impl Value {
    /// The only path for raw input data (PLAN.md § 7.1).
    ///
    /// Rejects missing provenance and non-finite numbers ("no bare numbers").
    #[allow(clippy::too_many_arguments)]
    pub fn from_source(
        value: f64,
        uncertainty: Option<f64>,
        source: &str,
        edition: &str,
        fetched: Option<&str>,
        url: Option<&str>,
        id: Option<&str>,
    ) -> Result<Self, ValueError> {
        if source.trim().is_empty() {
            return Err(ValueError::MissingProvenance("source".into()));
        }
        if edition.trim().is_empty() {
            return Err(ValueError::MissingProvenance("edition".into()));
        }
        if !value.is_finite() {
            return Err(ValueError::NonFinite("value".into()));
        }
        if uncertainty.is_some_and(|u| !u.is_finite() || u < 0.0) {
            return Err(ValueError::NonFinite("uncertainty".into()));
        }
        Ok(Self {
            value,
            uncertainty,
            source: source.to_string(),
            edition: edition.to_string(),
            fetched: fetched.map(str::to_string),
            url: url.map(str::to_string),
            id: id.map(str::to_string),
            method: None,
            derived_from: Vec::new(),
            based_on: None,
        })
    }

    /// The only path for derived values (PLAN.md § 7.1).
    ///
    /// Computes `f(inputs)`, propagates uncertainties by first-order linear
    /// propagation (numerical central-difference gradient), merges input
    /// provenance and records `derived_from` plus the `method` label.
    ///
    /// Uncertainty model: independent inputs, first-order Taylor expansion —
    /// `u_f^2 = sum_i (df/dx_i)^2 * u_i^2`. Inputs without uncertainty are
    /// treated as exact. Valid for mildly non-linear functions; strongly
    /// non-linear derivations should document the limitation in `method`.
    pub fn derive(
        f: &dyn Fn(&[f64]) -> f64,
        inputs: &[&Value],
        method: &str,
    ) -> Result<Self, ValueError> {
        if method.trim().is_empty() {
            return Err(ValueError::MissingProvenance("method".into()));
        }
        if inputs.is_empty() {
            return Err(ValueError::MissingProvenance(
                "derivation requires at least one input".into(),
            ));
        }
        let xs: Vec<f64> = inputs.iter().map(|v| v.value).collect();
        let value = f(&xs);
        if !value.is_finite() {
            return Err(ValueError::NonFiniteResult {
                method: method.to_string(),
            });
        }

        let uncertainty = propagate_uncertainty(f, &xs, inputs);

        let mut derived_from: Vec<String> = inputs
            .iter()
            .map(|v| {
                v.id.clone()
                    .unwrap_or_else(|| format!("{}/{}", v.source, v.edition))
            })
            .collect();
        derived_from.sort();
        derived_from.dedup();

        let edition = inputs
            .iter()
            .map(|v| v.edition.as_str())
            .max()
            .unwrap_or("unknown")
            .to_string();

        Ok(Self {
            value,
            uncertainty,
            source: "derived".to_string(),
            edition,
            fetched: None,
            url: None,
            id: None,
            method: Some(method.to_string()),
            derived_from,
            based_on: None,
        })
    }

    /// User overlay edit (PLAN.md § 7.1, D7): same machinery, tagged
    /// `source: "user"` with `based_on` pointing at the original value.
    pub fn from_user(value: f64, based_on: &str) -> Result<Self, ValueError> {
        if based_on.trim().is_empty() {
            return Err(ValueError::MissingProvenance("based_on".into()));
        }
        if !value.is_finite() {
            return Err(ValueError::NonFinite("value".into()));
        }
        Ok(Self {
            value,
            uncertainty: None,
            source: "user".to_string(),
            edition: "user".to_string(),
            fetched: None,
            url: None,
            id: None,
            method: None,
            derived_from: Vec::new(),
            based_on: Some(based_on.to_string()),
        })
    }

    /// Label used in provenance chains: the id when present, else
    /// `source/edition`.
    pub fn label(&self) -> String {
        self.id
            .clone()
            .unwrap_or_else(|| format!("{}/{}", self.source, self.edition))
    }
}

/// First-order uncertainty propagation with a central-difference gradient.
fn propagate_uncertainty(f: &dyn Fn(&[f64]) -> f64, xs: &[f64], inputs: &[&Value]) -> Option<f64> {
    if inputs.iter().all(|v| v.uncertainty.is_none()) {
        return None;
    }
    let mut variance = 0.0;
    for i in 0..xs.len() {
        let Some(u) = inputs[i].uncertainty else {
            continue;
        };
        let step = xs[i].abs().max(1.0) * 1e-6;
        let mut up = xs.to_vec();
        let mut down = xs.to_vec();
        up[i] += step;
        down[i] -= step;
        let derivative = (f(&up) - f(&down)) / (2.0 * step);
        variance += (derivative * u) * (derivative * u);
    }
    Some(variance.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(v: f64, u: Option<f64>, id: &str) -> Value {
        Value::from_source(
            v,
            u,
            "PDG",
            "2024",
            Some("2026-02-14"),
            Some("https://example.test"),
            Some(id),
        )
        .expect("valid source value")
    }

    #[test]
    fn from_source_keeps_full_provenance() {
        let v = src(2.16, Some(0.09), "particles/up/mass_MeV");
        assert_eq!(v.source, "PDG");
        assert_eq!(v.edition, "2024");
        assert_eq!(v.fetched.as_deref(), Some("2026-02-14"));
        assert_eq!(v.id.as_deref(), Some("particles/up/mass_MeV"));
    }

    #[test]
    fn bare_numbers_are_rejected() {
        assert_eq!(
            Value::from_source(1.0, None, "", "2024", None, None, None),
            Err(ValueError::MissingProvenance("source".into()))
        );
        assert_eq!(
            Value::from_source(1.0, None, "PDG", "", None, None, None),
            Err(ValueError::MissingProvenance("edition".into()))
        );
        assert!(matches!(
            Value::from_source(f64::NAN, None, "PDG", "2024", None, None, None),
            Err(ValueError::NonFinite(_))
        ));
    }

    #[test]
    fn derive_propagates_linear_uncertainty() {
        // f = a + b with u_a = 3, u_b = 4 must give u_f = 5.
        let a = src(10.0, Some(3.0), "a");
        let b = src(20.0, Some(4.0), "b");
        let sum = Value::derive(&|x| x[0] + x[1], &[&a, &b], "sum").expect("derive");
        assert!((sum.value - 30.0).abs() < 1e-12);
        let u = sum.uncertainty.expect("propagated uncertainty");
        assert!((u - 5.0).abs() < 1e-6);
    }

    #[test]
    fn derive_merges_provenance_and_records_method() {
        let a = src(2.0, None, "alpha");
        let b = src(3.0, None, "beta");
        let r = Value::derive(&|x| x[0] * x[1], &[&a, &b], "product").expect("derive");
        assert_eq!(r.source, "derived");
        assert_eq!(r.method.as_deref(), Some("product"));
        assert_eq!(
            r.derived_from,
            vec!["alpha".to_string(), "beta".to_string()]
        );
        assert_eq!(r.value, 6.0);
        assert!(r.uncertainty.is_none());
    }

    #[test]
    fn derive_without_method_is_rejected() {
        let a = src(1.0, None, "a");
        assert!(matches!(
            Value::derive(&|x| x[0], &[&a], ""),
            Err(ValueError::MissingProvenance(_))
        ));
    }

    #[test]
    fn user_overlay_is_tagged_and_linked() {
        let v = Value::from_user(0.5, "constants/alpha").expect("user value");
        assert_eq!(v.source, "user");
        assert_eq!(v.based_on.as_deref(), Some("constants/alpha"));
        assert!(matches!(
            Value::from_user(0.5, ""),
            Err(ValueError::MissingProvenance(_))
        ));
    }

    #[test]
    fn serde_roundtrip_keeps_all_fields() {
        let v = src(1.23, Some(0.01), "x");
        let json = serde_json::to_string(&v).expect("serialize");
        let back: Value = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(v, back);
    }
}
