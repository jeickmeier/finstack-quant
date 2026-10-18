//! Credit-factor attribution detail configuration and model traceability.

use finstack_quant_factor::credit::hierarchy::CreditFactorModel;
use serde::{Deserialize, Serialize};

/// Options controlling the credit-factor detail emitted by attribution.
///
/// Defaults: per-issuer adder breakdown OFF (large-portfolio payload control);
/// per-bucket breakdown ON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, default)]
pub struct CreditFactorDetailOptions {
    /// When true, populate `CreditFactorAttribution.adder_pnl_by_issuer`.
    /// Defaults to `false` to keep payload small for big portfolios.
    pub include_per_issuer_adder: bool,
    /// When true, populate `LevelPnl.by_bucket` for every level. When false,
    /// only `LevelPnl.total` is populated. Defaults to `true`.
    pub include_per_bucket_breakdown: bool,
}

impl Default for CreditFactorDetailOptions {
    fn default() -> Self {
        Self {
            include_per_issuer_adder: false,
            include_per_bucket_breakdown: true,
        }
    }
}

/// Stable, deterministic identifier for a [`CreditFactorModel`].
///
/// Defined as `"{as_of}/{fnv1a64(serde_json::to_string(model))}"` (16-char
/// lowercase hex). The model is serialized via `serde_json` (which uses
/// `BTreeMap`-stable order) so two byte-identical models produce the same id.
///
/// FNV-1a is used to avoid a new external crypto dependency; the id is for
/// traceability, not security.
///
/// # Arguments
///
/// * `model` - Calibrated credit-factor model whose valuation date and stable
///   serialized contents determine the traceability identifier.
#[allow(clippy::expect_used)] // CreditFactorModel has no non-serializable fields
pub(crate) fn credit_factor_model_id(model: &CreditFactorModel) -> String {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let json = serde_json::to_string(model).expect("CreditFactorModel is always serializable");
    let mut hash: u64 = FNV_OFFSET;
    for b in json.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{}/{:016x}", model.as_of, hash)
}
