//! Numeric precision, rounding rules, and IO metadata for `finstack-core`.
//!
//! FinStack opts into *explicit* configuration: there is no global state and all
//! helpers in this module expect a `FinstackConfig` supplied by the caller. The
//! config captures per-currency rounding scales, the active numeric engine, and
//! helper utilities for stamping outputs.
//!
//! # When to use
//! - to determine ingest/output decimal scales for [`crate::money::Money`]
//! - to stamp result envelopes with the numeric configuration via `results_meta`
//! - to snapshot the rounding policy that produced an output
//!
//! # Examples
//! ```rust
//! use finstack_core::config::{FinstackConfig, RoundingMode};
//! use finstack_core::currency::Currency;
//!
//! let mut cfg = FinstackConfig::default();
//! cfg.rounding.mode = RoundingMode::AwayFromZero;
//! cfg.rounding.output_scale.overrides.insert(Currency::JPY, 0);
//! cfg.rounding.ingest_scale.overrides.insert(Currency::JPY, 0);
//!
//! let usd_scale = cfg.output_scale(Currency::USD);
//! let jpy_ingest = cfg.ingest_scale(Currency::JPY);
//!
//! assert_eq!(usd_scale, 2);
//! assert_eq!(jpy_ingest, 0);
//! ```
//!
//! ## Numeric mode
//!
//! The engine currently operates in a single numeric mode: `NumericMode::F64`.
//! To make this explicit and avoid unnecessary function calls, the active mode
//! is exposed as a constant: `NUMERIC_MODE`. Future releases may introduce
//! additional modes (e.g., alternative numeric strategies) or feature-gated switching; in that case
//! the constant will remain stable and reflect the compile-time choice.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use std::collections::BTreeMap;

/// Rounding modes supported by the library.
///
/// The variants mirror the most common conventions found in pricing engines.
///
/// # Examples
/// ```rust
/// use finstack_core::config::{FinstackConfig, RoundingMode};
///
/// let mut cfg = FinstackConfig::default();
/// cfg.rounding.mode = RoundingMode::TowardZero;
/// assert!(matches!(cfg.rounding.mode, RoundingMode::TowardZero));
/// ```
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum RoundingMode {
    /// Banker's rounding (ties to even).
    #[default]
    Bankers,
    /// Round halves away from zero.
    AwayFromZero,
    /// Round toward zero (truncate).
    TowardZero,
    /// Round toward negative infinity.
    Floor,
    /// Round toward positive infinity.
    Ceil,
}

// Default derived above

// ---------------------------------------------------------------------------
// Display + FromStr
// ---------------------------------------------------------------------------

impl std::fmt::Display for RoundingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            RoundingMode::Bankers => "bankers",
            RoundingMode::AwayFromZero => "away_from_zero",
            RoundingMode::TowardZero => "toward_zero",
            RoundingMode::Floor => "floor",
            RoundingMode::Ceil => "ceil",
        };
        f.write_str(label)
    }
}

impl crate::parse::NormalizedEnum for RoundingMode {
    const VARIANTS: &'static [(&'static str, Self)] = &[
        ("bankers", Self::Bankers),
        ("banker", Self::Bankers),
        ("away_from_zero", Self::AwayFromZero),
        ("awayfromzero", Self::AwayFromZero),
        ("toward_zero", Self::TowardZero),
        ("towards_zero", Self::TowardZero),
        ("floor", Self::Floor),
        ("ceil", Self::Ceil),
        ("ceiling", Self::Ceil),
    ];
}

impl std::str::FromStr for RoundingMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::parse::parse_normalized_enum(s)
    }
}

/// Global numeric configuration supplied to valuation components.
///
/// The configuration owns two [`CurrencyScalePolicy`] maps (ingest/output) and
/// the active [`RoundingMode`]. It can be customised at startup and threaded
/// through pricing calculations.
///
/// # Examples
/// ```rust
/// use finstack_core::config::{FinstackConfig, RoundingMode};
/// use finstack_core::currency::Currency;
///
/// let mut cfg = FinstackConfig::default();
/// cfg.rounding.mode = RoundingMode::Bankers;
/// cfg.rounding.output_scale.overrides.insert(Currency::CHF, 2);
/// assert_eq!(cfg.output_scale(Currency::CHF), 2);
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FinstackConfig {
    /// Detailed rounding policy (ingest/output scales by currency).
    pub rounding: RoundingPolicy,
    /// Numerical tolerance settings for floating-point comparisons.
    #[serde(default)]
    pub tolerances: ToleranceConfig,
    /// Optional module-specific configuration sections (versioned, namespaced keys).
    ///
    /// Keys follow `{crate}.{domain}.v{N}`, e.g., `valuations.calibration.v2`.
    /// Values are validated by the owning crate's strict serde schema.
    #[serde(default, skip_serializing_if = "ConfigExtensions::is_empty")]
    pub extensions: ConfigExtensions,
}
// Default derived above

/// Versioned, namespaced extension map carried alongside the core config.
///
/// Keys must match the `{crate}.{domain}.v{N}` pattern (one or more
/// dot-separated `snake_case` segments followed by a `v{N}` version segment,
/// e.g. `valuations.calibration.v2` or
/// `valuations.structured_credit.ytm.v1`). Deserialization rejects keys that
/// do not match, so obvious typos fail loudly instead of being silently
/// carried along .
#[derive(Clone, Debug, Default, Serialize)]
pub struct ConfigExtensions {
    #[serde(flatten)]
    pub(crate) inner: BTreeMap<String, JsonValue>,
}

/// Returns true if `key` matches the `{crate}.{domain}.v{N}` extension-key
/// pattern: at least two leading `snake_case` segments and a trailing
/// `v{digits}` version segment.
fn is_valid_extension_key(key: &str) -> bool {
    fn is_ident(s: &str) -> bool {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    }
    let segments: Vec<&str> = key.split('.').collect();
    let Some((version, idents)) = segments.split_last() else {
        return false;
    };
    if idents.len() < 2 {
        return false;
    }
    let version_ok = version
        .strip_prefix('v')
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    version_ok && idents.iter().all(|s| is_ident(s))
}

impl<'de> Deserialize<'de> for ConfigExtensions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let inner = BTreeMap::<String, JsonValue>::deserialize(deserializer)?;
        if let Some(bad) = inner.keys().find(|k| !is_valid_extension_key(k)) {
            return Err(serde::de::Error::custom(format!(
                "invalid config extension key '{bad}': keys must match '{{crate}}.{{domain}}.v{{N}}' (e.g. 'valuations.calibration.v2')"
            )));
        }
        Ok(Self { inner })
    }
}

impl ConfigExtensions {
    /// Returns true if no extension sections are present.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get a section by key.
    #[inline]
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        self.inner.get(key)
    }

    /// Iterate over configured extension keys.
    #[inline]
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.inner.keys().map(String::as_str)
    }

    /// Insert or replace a section by key.
    #[inline]
    pub fn insert(&mut self, key: impl Into<String>, value: JsonValue) -> Option<JsonValue> {
        self.inner.insert(key.into(), value)
    }

    /// Remove a section by key.
    #[inline]
    pub fn remove(&mut self, key: &str) -> Option<JsonValue> {
        self.inner.remove(key)
    }
}

/// Policy mapping that determines decimal places for each currency at ingest/output.
///
/// The policy stores currency-specific overrides only.
///
/// # Examples
/// ```rust
/// use finstack_core::config::{CurrencyScalePolicy, FinstackConfig};
/// use finstack_core::currency::Currency;
/// use std::collections::BTreeMap;
///
/// let mut cfg = FinstackConfig::default();
/// cfg.rounding.output_scale = CurrencyScalePolicy {
///     overrides: BTreeMap::from([(Currency::KWD, 3)]),
/// };
///
/// assert_eq!(cfg.output_scale(Currency::KWD), 3);
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CurrencyScalePolicy {
    /// Explicit currency overrides for scale.
    pub overrides: BTreeMap<crate::currency::Currency, u32>,
}

/// Full rounding policy used at IO boundaries and normalization steps.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RoundingPolicy {
    /// Rounding behaviour to apply when mapping fractional values to a scale.
    pub mode: RoundingMode,
    /// Decimal places applied when normalizing inbound values by currency.
    pub ingest_scale: CurrencyScalePolicy,
    /// Decimal places used at output/serialization for each currency.
    pub output_scale: CurrencyScalePolicy,
}

/// Numerical tolerance configuration for floating-point comparisons.
///
/// Provides configurable epsilon values for zero-checks in rate calculations
/// and generic floating-point comparisons. These defaults are chosen to balance
/// numerical stability with practical precision requirements.
///
/// # Examples
/// ```rust
/// use finstack_core::config::ToleranceConfig;
///
/// let mut tol = ToleranceConfig::default();
/// assert_eq!(tol.rate_epsilon, 1e-12);
///
/// // Customize for stricter rate comparisons
/// tol.rate_epsilon = 1e-14;
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ToleranceConfig {
    /// Epsilon for rate comparisons (default: 1e-12).
    ///
    /// Used when comparing interest rates, yields, and other small ratios.
    #[serde(default = "default_rate_epsilon")]
    pub rate_epsilon: f64,
    /// Epsilon for generic floating-point comparisons (default: 1e-10).
    ///
    /// Used for general numerical comparisons where higher tolerance is acceptable.
    #[serde(default = "default_generic_epsilon")]
    pub generic_epsilon: f64,
}

/// Default epsilon for rate comparisons: 1e-12.
///
/// # Rationale
///
/// This value is chosen to be:
/// - Small enough to distinguish meaningful rate differences (1 bp = 0.0001)
/// - Large enough to absorb floating-point accumulation errors in typical
///   pricing calculations
/// - Approximately sqrt(machine_epsilon) × typical_rate_magnitude, a standard
///   choice for numerical tolerance (machine_epsilon ≈ 2.2e-16 for f64)
///
/// A rate difference of 1e-12 corresponds to ~0.0001 basis points, which is
/// economically insignificant in all practical applications.
fn default_rate_epsilon() -> f64 {
    1e-12
}

/// Default epsilon for generic floating-point comparisons: 1e-10.
///
/// # Rationale
///
/// This is more permissive than [`default_rate_epsilon`] because:
/// - Generic calculations may accumulate more rounding error
/// - Used for quantities like prices, notionals, PVs where precision
///   requirements are less strict than for rates
/// - Still tight enough to catch actual numerical errors vs. expected values
///
/// At 1e-10, a $1M notional would need to match within $0.0001 (0.01 cents),
/// which is below any practical settlement precision.
fn default_generic_epsilon() -> f64 {
    1e-10
}

impl Default for ToleranceConfig {
    fn default() -> Self {
        Self {
            rate_epsilon: default_rate_epsilon(),
            generic_epsilon: default_generic_epsilon(),
        }
    }
}

/// Snapshot of active rounding settings used for result stamping.
///
/// Instances are typically produced via [`rounding_context_from`] and persisted
/// alongside valuation results.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RoundingContext {
    /// Active rounding mode.
    pub mode: RoundingMode,
    /// Ingest scale map snapshot by currency code.
    pub ingest_scale_by_ccy: BTreeMap<crate::currency::Currency, u32>,
    /// Output scale map snapshot by currency code.
    pub output_scale_by_ccy: BTreeMap<crate::currency::Currency, u32>,
    /// Tolerance settings snapshot for floating-point comparisons.
    #[serde(default)]
    pub tolerances: ToleranceConfig,
    /// Schema version for forward compatibility.
    pub version: u32,
}

/// Zero-kind classification for tolerance checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ZeroKind {
    /// Money-like magnitudes (use currency scale).
    Money(crate::currency::Currency),
    /// Interest rates or small numeric ratios.
    Rate,
    /// Generic floating-point comparisons.
    Generic,
}

impl Default for RoundingContext {
    fn default() -> Self {
        rounding_context_from(&FinstackConfig::default())
    }
}

impl RoundingContext {
    /// Effective output scale for the provided currency within this context.
    #[inline]
    pub fn output_scale(&self, ccy: crate::currency::Currency) -> u32 {
        if let Some(&s) = self.output_scale_by_ccy.get(&ccy) {
            return s;
        }
        ccy.decimals() as u32
    }

    /// Money epsilon derived from the currency output scale (half ULP at that scale).
    #[inline]
    pub fn money_epsilon(&self, ccy: crate::currency::Currency) -> f64 {
        let scale = self.output_scale(ccy) as i32;
        // Half of one unit in the last place at the configured scale.
        0.5 * 10f64.powi(-scale)
    }

    /// Returns true if a money amount (in the given currency) is effectively zero under this context.
    #[inline]
    pub fn is_effectively_zero_money(&self, amount: f64, ccy: crate::currency::Currency) -> bool {
        amount.abs() <= self.money_epsilon(ccy)
    }

    /// Returns true if a floating value is effectively zero for the specified kind.
    ///
    /// Uses tolerance values from the context's [`ToleranceConfig`]:
    /// - Rate: uses [`ToleranceConfig::rate_epsilon`] (default: 1e-12)
    /// - Generic: uses [`ToleranceConfig::generic_epsilon`] (default: 1e-10)
    /// - Money: uses [`Self::money_epsilon`] (derived from currency scale)
    #[inline]
    pub fn is_effectively_zero(&self, x: f64, kind: ZeroKind) -> bool {
        match kind {
            ZeroKind::Money(ccy) => self.is_effectively_zero_money(x, ccy),
            ZeroKind::Rate => x.abs() <= self.tolerances.rate_epsilon,
            ZeroKind::Generic => x.abs() <= self.tolerances.generic_epsilon,
        }
    }
}

/// Numeric engine mode compiled into the crate.
///
/// Currently single-variant (F64); exists as `#[non_exhaustive]` for forward-compatible extension.
///
/// # What the stamp asserts (and what it does not)
///
/// `F64` asserts that **curve, rate, and analytics arithmetic** (discounting,
/// interpolation, risk metrics, scenario math, FX rates as scalars) runs on
/// 64-bit IEEE 754 floating point. It does **not** describe the [`Money`]
/// layer: monetary amounts are stored and combined on a Decimal-backed
/// representation (`rust_decimal::Decimal`) regardless of this stamp, with
/// f64 appearing only at ingest/accessor boundaries. Auditors reading
/// `numeric_mode: F64` in [`ResultsMeta`] should therefore interpret it as
/// "non-monetary numerics are f64", not "monetary amounts were accumulated
/// in floating point". (Clarified .)
///
/// [`Money`]: crate::money::Money
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[non_exhaustive]
pub enum NumericMode {
    /// Floating-point f64 engine for curve/rate/analytics arithmetic.
    /// Monetary amounts remain Decimal-backed independent of this variant.
    F64,
}

/// Metadata bundle that accompanies valuation outputs.
///
/// The metadata is intentionally small so it can be attached to reports and
/// downstream data stores for reproducibility and audit trails.
///
/// # Examples
/// ```rust
/// use finstack_core::config::{results_meta, FinstackConfig, NumericMode};
///
/// let meta = results_meta(&FinstackConfig::default());
/// assert_eq!(meta.numeric_mode, NumericMode::F64);
/// assert!(meta.timestamp.is_none()); // deterministic by default
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResultsMeta {
    /// Numeric engine mode used to produce the results.
    pub numeric_mode: NumericMode,
    /// Rounding context snapshot applied to IO boundaries.
    pub rounding: RoundingContext,
    /// Optional FX policy applied by the computing layer (human-readable key).
    #[serde(default)]
    pub fx_policy_applied: Option<String>,
    /// Timestamp when result was computed (ISO 8601 format).
    /// Useful for audit trails and reproducibility.
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        with = "time::serde::iso8601::option"
    )]
    #[schemars(with = "Option<String>")]
    pub timestamp: Option<time::OffsetDateTime>,
    /// Finstack library version used to produce the result.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub version: Option<String>,
}

impl Default for ResultsMeta {
    fn default() -> Self {
        results_meta(&FinstackConfig::default())
    }
}

impl FinstackConfig {
    /// Effective output scale for a currency. Falls back to ISO-4217 `Currency::decimals()`.
    pub fn output_scale(&self, ccy: crate::currency::Currency) -> u32 {
        if let Some(&s) = self.rounding.output_scale.overrides.get(&ccy) {
            return s;
        }
        ccy.decimals() as u32
    }

    /// Effective ingest scale for a currency. Falls back to a high precision default
    /// to preserve inputs unless explicitly overridden (min of 6 decimals).
    pub fn ingest_scale(&self, ccy: crate::currency::Currency) -> u32 {
        if let Some(&s) = self.rounding.ingest_scale.overrides.get(&ccy) {
            return s;
        }
        core::cmp::max(6, ccy.decimals() as u32)
    }
}

/// Build a snapshot of the current rounding context from a config.
///
/// The snapshot captures the concrete overrides used during valuation and can
/// later be embedded in [`ResultsMeta`].
///
/// # Examples
/// ```rust
/// use finstack_core::config::{rounding_context_from, FinstackConfig};
///
/// let ctx = rounding_context_from(&FinstackConfig::default());
/// assert_eq!(ctx.version, 1);
/// ```
pub fn rounding_context_from(cfg: &FinstackConfig) -> RoundingContext {
    RoundingContext {
        mode: cfg.rounding.mode,
        ingest_scale_by_ccy: cfg.rounding.ingest_scale.overrides.clone(),
        output_scale_by_ccy: cfg.rounding.output_scale.overrides.clone(),
        tolerances: cfg.tolerances,
        version: 1,
    }
}

/// Active numeric mode used by the engine.
///
/// This compile-time constant indicates the numeric representation used for
/// **curve, rate, and analytics arithmetic** in Finstack. Currently fixed to
/// [`NumericMode::F64`] (64-bit IEEE 754 floating point).
///
/// # Scope of the stamp
///
/// The constant — and the `numeric_mode` field it stamps into
/// [`ResultsMeta`] — covers non-monetary numerics only. The
/// [`Money`](crate::money::Money) layer is Decimal-backed
/// (`rust_decimal::Decimal`) independent of this stamp: monetary amounts are
/// ingested, rounded, and combined in Decimal, with f64 used only at the
/// `Money::amount()` accessor and f64-input constructors. See
/// [`NumericMode`] for the full audit-stamp semantics (clarified per the
/// ).
///
/// # Rationale
///
/// F64 provides:
/// - ~15-17 significant decimal digits of precision
/// - Sufficient for curve/rate/analytics calculations
/// - Hardware-accelerated operations on modern CPUs
/// - Wide ecosystem compatibility (Python, JavaScript, databases)
///
/// # Future Considerations
///
/// This constant exists to support potential future modes (e.g., decimal arithmetic
/// for regulatory compliance). Code that needs to be mode-aware should reference
/// this constant rather than assuming F64.
///
/// # Example
///
/// ```rust
/// use finstack_core::config::{NUMERIC_MODE, NumericMode};
///
/// assert_eq!(NUMERIC_MODE, NumericMode::F64);
/// ```
pub const NUMERIC_MODE: NumericMode = NumericMode::F64;

/// Construct a [`ResultsMeta`] snapshot for stamping into result envelopes.
///
/// Convenience wrapper that combines [`NUMERIC_MODE`] and
/// [`rounding_context_from`], without a timestamp (deterministic).
///
/// # Examples
/// ```rust
/// use finstack_core::config::{results_meta, FinstackConfig, NumericMode};
///
/// let cfg = FinstackConfig::default();
/// let meta = results_meta(&cfg);
/// assert_eq!(meta.numeric_mode, NumericMode::F64);
/// assert!(meta.timestamp.is_none());
/// ```
pub fn results_meta(cfg: &FinstackConfig) -> ResultsMeta {
    results_meta_with_timestamp(cfg, None)
}

/// Construct a [`ResultsMeta`] snapshot and stamp a timestamp of "now".
///
/// Use this at user-facing IO boundaries and audit trails. For deterministic outputs
/// (golden tests, reproducible snapshots), prefer [`results_meta`].
pub fn results_meta_now(cfg: &FinstackConfig) -> ResultsMeta {
    // With `wasm-bindgen` feature enabled in `time` crate, `now_utc()` works on WASM too.
    results_meta_with_timestamp(cfg, Some(time::OffsetDateTime::now_utc()))
}

/// Construct a [`ResultsMeta`] snapshot with an explicitly provided timestamp.
///
/// This is useful for deterministic injection (tests) or when a higher-level layer
/// controls timestamping.
pub fn results_meta_with_timestamp(
    cfg: &FinstackConfig,
    timestamp: Option<time::OffsetDateTime>,
) -> ResultsMeta {
    ResultsMeta {
        numeric_mode: NUMERIC_MODE,
        rounding: rounding_context_from(cfg),
        fx_policy_applied: None,
        timestamp,
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::RoundingMode;

    #[test]
    fn config_extensions_accept_valid_versioned_keys() {
        // ): extension keys are validated
        // against `{crate}.{domain}.v{N}` at deserialization.
        for key in [
            "valuations.calibration.v2",
            "core.dummy_registry.v1",
            "valuations.structured_credit.ytm.v1",
        ] {
            let json = format!(
                r#"{{"rounding":{{"mode":"Bankers","ingest_scale":{{"overrides":{{}}}},"output_scale":{{"overrides":{{}}}}}},"extensions":{{"{key}":{{}}}}}}"#
            );
            let cfg: super::FinstackConfig =
                serde_json::from_str(&json).unwrap_or_else(|e| panic!("{key} rejected: {e}"));
            assert!(cfg.extensions.get(key).is_some());
        }
    }

    #[test]
    fn config_extensions_reject_malformed_keys() {
        for key in [
            "valuationscalibration",     // no namespacing at all
            "valuations.v1",             // missing domain segment
            "valuations.calibration",    // missing version segment
            "valuations.calibration.2",  // version missing the `v` prefix
            "valuations.calibration.vX", // non-numeric version
            "Valuations.Calibration.v1", // not snake_case
        ] {
            let json = format!(
                r#"{{"rounding":{{"mode":"Bankers","ingest_scale":{{"overrides":{{}}}},"output_scale":{{"overrides":{{}}}}}},"extensions":{{"{key}":{{}}}}}}"#
            );
            assert!(
                serde_json::from_str::<super::FinstackConfig>(&json).is_err(),
                "malformed extension key '{key}' must be rejected"
            );
        }
    }

    fn assert_parses_to(label: &str, expected: RoundingMode) {
        assert!(matches!(label.parse::<RoundingMode>(), Ok(value) if value == expected));
    }

    #[test]
    fn rounding_mode_display_roundtrip() {
        let all = [
            RoundingMode::Bankers,
            RoundingMode::AwayFromZero,
            RoundingMode::TowardZero,
            RoundingMode::Floor,
            RoundingMode::Ceil,
        ];

        for mode in &all {
            let label = mode.to_string();
            assert!(
                matches!(label.parse::<RoundingMode>(), Ok(value) if value == *mode),
                "roundtrip failed for {label}"
            );
        }
    }

    #[test]
    fn rounding_mode_from_str_aliases() {
        assert_parses_to("banker", RoundingMode::Bankers);
        assert_parses_to("awayfromzero", RoundingMode::AwayFromZero);
        assert_parses_to("towards_zero", RoundingMode::TowardZero);
        assert_parses_to("ceiling", RoundingMode::Ceil);
    }

    #[test]
    fn rounding_mode_from_str_unknown() {
        assert!("garbage".parse::<RoundingMode>().is_err());
    }
}
