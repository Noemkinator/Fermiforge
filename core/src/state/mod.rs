//! Scene state: schema, canonical serialization and URL codec (PLAN.md § 7.5).
//!
//! A scene contains only inputs (geometry, parameters, overrides) — never
//! computed data such as orbitals or grids (PLAN.md § 13.9). Serialization is
//! deterministic: canonical JSON (sorted keys, omitted defaults) → DEFLATE →
//! base64url without padding, prefixed with a format version (`v1.`).
//!
//! URL input is untrusted: everything decoded is validated against hard
//! limits before use.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current scene schema version (serialized in every payload).
/// v2: added explicit `bonds` (v1 scenes migrate to empty = derive from geometry).
pub const SCHEMA_VERSION: u32 = 2;
/// URL format version prefix (PLAN.md § 7.5).
pub const URL_VERSION_PREFIX: &str = "v2.";
/// Fragment prefix used by the web app: `#/s=<payload>`.
pub const URL_FRAGMENT_PREFIX: &str = "#/s=";

/// Hard limits for untrusted scene input (PLAN.md § 7.5).
pub const MAX_ATOMS: usize = 1_000;
/// Maximum number of user overrides.
pub const MAX_OVERRIDES: usize = 50;
/// Maximum encoded payload size (6 kB compressed, PLAN.md § 7.5).
pub const MAX_ENCODED_LEN: usize = 6_144;
/// Maximum canonical JSON size before compression (decompression-bomb guard).
pub const MAX_JSON_LEN: usize = 65_536;
/// Position quantization step in angstrom (PLAN.md § 8.6 group A).
pub const POSITION_STEP: f64 = 0.01;
/// Sanity bound on |coordinate| in angstrom (limit, not a physical constant).
pub const MAX_COORD_ABS: f64 = 1.0e6;

/// Errors produced by scene encoding/decoding.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum StateError {
    /// Encoded state exceeds the URL size budget.
    #[error("state too large: {bytes} bytes encoded, limit {limit}")]
    TooLarge { bytes: usize, limit: usize },
    /// Unknown or unparsable URL format/schema version.
    #[error("unknown scene version: {0:?}")]
    UnknownVersion(String),
    /// Payload cannot be decoded (bad base64, deflate or JSON).
    #[error("corrupted payload: {0}")]
    Corrupted(String),
    /// A scene limit (atoms, overrides, coordinate magnitude) is exceeded.
    #[error("limit exceeded: {field} = {count}, max {max}")]
    LimitExceeded {
        field: &'static str,
        count: usize,
        max: usize,
    },
    /// A field has an invalid shape or value.
    #[error("invalid field {field}: {reason}")]
    InvalidField { field: &'static str, reason: String },
    /// Feature deliberately not implemented in this phase.
    #[error("unsupported scene form: {0}")]
    Unsupported(&'static str),
}

/// Rendering/interaction mode of the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Hückel molecular layer.
    #[default]
    Huckel,
    /// Dirac atomic layer.
    Atom,
}

/// A single atom: element symbol plus quantized position in angstrom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Atom {
    /// Element symbol as in `data/` (e.g. `"C"`), never a hardcoded Z.
    pub symbol: String,
    #[serde(with = "coord")]
    pub x: f64,
    #[serde(with = "coord")]
    pub y: f64,
    #[serde(with = "coord")]
    pub z: f64,
}

/// Coordinates are serialized as integer multiples of [`POSITION_STEP`]
/// (fixed-point, PLAN.md § 8.6 group A). Integers round-trip bit-exactly,
/// unlike JSON floats — which is what makes `assert_eq!` on scenes reliable.
mod coord {
    use super::POSITION_STEP;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i64((v / POSITION_STEP).round() as i64)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        let steps = i64::deserialize(d)?;
        Ok(steps as f64 * POSITION_STEP)
    }
}

/// Floats that must survive the round-trip bit-exactly are serialized as
/// strings of their shortest round-trip representation (Rust `Display`) and
/// parsed with the correctly-rounded native parser. serde_json's float
/// parsing is off by 1 ulp on some values, which would break determinism
/// (PLAN.md § 13.10).
mod exact_float {
    use serde::{Deserialize, Deserializer, Serializer, de};

    pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        let text = String::deserialize(d)?;
        text.parse::<f64>()
            .map_err(|e| de::Error::custom(format!("invalid float {text:?}: {e}")))
    }
}

impl Default for Atom {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

/// A user override of a named numeric parameter (constant, mass, ...).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Override {
    pub key: String,
    #[serde(with = "exact_float")]
    pub value: f64,
}

impl Default for Override {
    fn default() -> Self {
        Self {
            key: String::new(),
            value: 0.0,
        }
    }
}

/// Full scene description: inputs only, never computed data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Scene {
    /// Schema version, always serialized.
    pub schema: u32,
    #[serde(skip_serializing_if = "Mode::is_default")]
    pub mode: Mode,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub atoms: Vec<Atom>,
    /// Explicit pi-system bonds as atom index pairs; empty means "derive
    /// from geometry" (added in schema v2).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bonds: Vec<[u16; 2]>,
    #[serde(skip_serializing_if = "is_zero_i32")]
    pub charge: i32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<Override>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lepton: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nucleus_z: Option<u32>,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            schema: SCHEMA_VERSION,
            mode: Mode::default(),
            atoms: Vec::new(),
            bonds: Vec::new(),
            charge: 0,
            overrides: Vec::new(),
            lepton: None,
            nucleus_z: None,
        }
    }
}

impl Mode {
    fn is_default(&self) -> bool {
        *self == Mode::default()
    }
}

fn is_zero_i32(v: &i32) -> bool {
    *v == 0
}

/// Quantize one coordinate to the fixed-point grid (PLAN.md § 8.6 group A).
pub fn quantize(x: f64) -> f64 {
    ((x / POSITION_STEP).round() * POSITION_STEP) + 0.0
}

impl Scene {
    /// Copy with all positions on the quantization grid. Round-trip
    /// comparisons use this so `assert_eq!` on floats is reliable.
    #[must_use]
    pub fn quantized(&self) -> Scene {
        let mut out = self.clone();
        for atom in &mut out.atoms {
            atom.x = quantize(atom.x);
            atom.y = quantize(atom.y);
            atom.z = quantize(atom.z);
        }
        out
    }

    /// Canonical form used for encoding: quantized positions and overrides
    /// sorted by key, so that scenes differing only in override order
    /// produce identical URLs (PLAN.md § 8.6 group C).
    #[must_use]
    pub fn canonicalized(&self) -> Scene {
        let mut out = self.quantized();
        out.overrides.sort_by(|a, b| {
            a.key.cmp(&b.key).then_with(|| {
                a.value
                    .partial_cmp(&b.value)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        out
    }

    /// Validate against limits for untrusted input (PLAN.md § 7.5).
    pub fn validate(&self) -> Result<(), StateError> {
        if self.atoms.len() > MAX_ATOMS {
            return Err(StateError::LimitExceeded {
                field: "atoms",
                count: self.atoms.len(),
                max: MAX_ATOMS,
            });
        }
        if self.overrides.len() > MAX_OVERRIDES {
            return Err(StateError::LimitExceeded {
                field: "overrides",
                count: self.overrides.len(),
                max: MAX_OVERRIDES,
            });
        }
        let n_atoms = self.atoms.len();
        for bond in &self.bonds {
            if bond[0] as usize >= n_atoms || bond[1] as usize >= n_atoms {
                return Err(StateError::InvalidField {
                    field: "bond",
                    reason: format!("{bond:?} references atom outside 0..{n_atoms}"),
                });
            }
        }
        for atom in &self.atoms {
            let bytes = atom.symbol.as_bytes();
            if bytes.is_empty() || bytes.len() > 3 || !bytes.iter().all(u8::is_ascii_alphabetic) {
                return Err(StateError::InvalidField {
                    field: "atom.symbol",
                    reason: format!("{:?} is not 1-3 ASCII letters", atom.symbol),
                });
            }
            for (name, v) in [("x", atom.x), ("y", atom.y), ("z", atom.z)] {
                if !v.is_finite() || v.abs() > MAX_COORD_ABS {
                    return Err(StateError::InvalidField {
                        field: "atom.coordinate",
                        reason: format!("{name}={v} outside ±{MAX_COORD_ABS} Å"),
                    });
                }
            }
        }
        for o in &self.overrides {
            if o.key.is_empty()
                || o.key.len() > 64
                || !o
                    .key
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/'))
            {
                return Err(StateError::InvalidField {
                    field: "override.key",
                    reason: format!("{:?} is not a valid key", o.key),
                });
            }
            if !o.value.is_finite() {
                return Err(StateError::InvalidField {
                    field: "override.value",
                    reason: format!("{} is not finite", o.value),
                });
            }
        }
        if let Some(lepton) = &self.lepton {
            if lepton.is_empty()
                || lepton.len() > 16
                || !lepton
                    .bytes()
                    .all(|b| b.is_ascii_alphabetic() || b.is_ascii_digit())
            {
                return Err(StateError::InvalidField {
                    field: "lepton",
                    reason: format!("{lepton:?} is not a valid lepton tag"),
                });
            }
        }
        Ok(())
    }

    /// Canonical JSON: sorted keys, omitted defaults (PLAN.md § 8.6 group B).
    pub fn canonical_json(&self) -> Result<String, StateError> {
        let value = serde_json::to_value(self)
            .map_err(|e| StateError::Corrupted(format!("serialize: {e}")))?;
        serde_json::to_string(&value).map_err(|e| StateError::Corrupted(format!("serialize: {e}")))
    }

    /// Encode to the compact URL payload (`v1.<base64url>`), without fragment.
    pub fn encode(&self) -> Result<String, StateError> {
        self.validate()?;
        let json = self.canonicalized().canonical_json()?;
        if json.len() > MAX_JSON_LEN {
            return Err(StateError::TooLarge {
                bytes: json.len(),
                limit: MAX_JSON_LEN,
            });
        }
        let deflated = miniz_oxide::deflate::compress_to_vec(json.as_bytes(), 9);
        let payload = URL_SAFE_NO_PAD.encode(&deflated);
        let total = URL_VERSION_PREFIX.len() + payload.len();
        if total > MAX_ENCODED_LEN {
            return Err(StateError::TooLarge {
                bytes: total,
                limit: MAX_ENCODED_LEN,
            });
        }
        Ok(format!("{URL_VERSION_PREFIX}{payload}"))
    }

    /// Full shareable fragment: `#/s=v1.<base64url>`.
    pub fn to_url_fragment(&self) -> Result<String, StateError> {
        Ok(format!("{}{}", URL_FRAGMENT_PREFIX, self.encode()?))
    }

    /// Decode a URL fragment or bare payload. Untrusted input: every limit is
    /// enforced and no malformed input may panic (PLAN.md § 8.6 group D).
    pub fn from_url_fragment(input: &str) -> Result<Scene, StateError> {
        let rest = input
            .strip_prefix(URL_FRAGMENT_PREFIX)
            .or_else(|| input.strip_prefix("#s="))
            .or_else(|| input.strip_prefix("s="))
            .unwrap_or(input);
        if rest.contains('?') {
            return Err(StateError::Unsupported(
                "readable query form (#/atom?...) is scheduled for M3",
            ));
        }
        let (version, payload) = rest
            .split_once('.')
            .ok_or_else(|| StateError::Corrupted("missing version prefix".into()))?;
        let number = version
            .strip_prefix('v')
            .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|n| n.parse::<u32>().ok())
            .ok_or_else(|| StateError::UnknownVersion(version.to_string()))?;
        if number > SCHEMA_VERSION {
            return Err(StateError::UnknownVersion(version.to_string()));
        }

        let raw = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|e| StateError::Corrupted(format!("base64: {e}")))?;
        if raw.len() > MAX_ENCODED_LEN {
            return Err(StateError::TooLarge {
                bytes: raw.len(),
                limit: MAX_ENCODED_LEN,
            });
        }
        let json = miniz_oxide::inflate::decompress_to_vec(&raw)
            .map_err(|e| StateError::Corrupted(format!("deflate: {e:?}")))?;
        if json.len() > MAX_JSON_LEN {
            return Err(StateError::TooLarge {
                bytes: json.len(),
                limit: MAX_JSON_LEN,
            });
        }
        let text =
            String::from_utf8(json).map_err(|e| StateError::Corrupted(format!("utf8: {e}")))?;

        let mut value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| StateError::Corrupted(format!("json: {e}")))?;
        let found = value
            .as_object()
            .and_then(|map| map.get("schema"))
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| StateError::Corrupted("missing schema field".into()))?;
        let mut version_of_value = found as u32;
        while version_of_value < SCHEMA_VERSION {
            value = migrate_step(version_of_value, value)?;
            version_of_value = value
                .as_object()
                .and_then(|map| map.get("schema"))
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| StateError::Corrupted("migration dropped schema".into()))?
                as u32;
        }
        let scene: Scene = serde_json::from_value(value)
            .map_err(|e| StateError::Corrupted(format!("scene: {e}")))?;
        scene.validate()?;
        Ok(scene)
    }
}

/// One schema migration step `from -> from + 1`.
///
/// Rule (PLAN.md § 7.5, § 8.6 group E): every future schema bump adds a step
/// here plus a stored fixture of an old-version serialization.
fn migrate_step(from: u32, value: serde_json::Value) -> Result<serde_json::Value, StateError> {
    match from {
        // v1 -> v2: explicit bonds added; v1 scenes derive them from geometry
        1 => {
            let mut map = match value {
                serde_json::Value::Object(map) => map,
                _ => return Err(StateError::Corrupted("migration on non-object".into())),
            };
            map.insert("bonds".into(), serde_json::json!([]));
            map.insert("schema".into(), serde_json::json!(2));
            Ok(serde_json::Value::Object(map))
        }
        _ => Err(StateError::UnknownVersion(format!(
            "no migration step defined for schema {from}"
        ))),
    }
}
