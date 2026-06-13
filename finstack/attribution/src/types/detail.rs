//! Factor-specific detail types for P&L attribution breakdowns.
//!
//! This module holds the per-factor attribution detail structs: carry
//! decomposition, curve-level breakdowns, FX pair attribution, volatility
//! surface attribution, cross-factor interactions, model parameters, market
//! scalars, and credit factor hierarchy decomposition.

use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, IssuerId};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Per-(curve, tenor) P&L map: keyed by `(curve_id, tenor_label)`.
type CurveTenorPnlMap = IndexMap<(CurveId, String), Money>;

/// Serde representation for `IndexMap<(CurveId, String), Money>` keyed maps.
///
/// JSON object keys must be strings, so the `(curve_id, tenor)` tuple is
/// encoded as `"{curve_id}|{tenor}"` (e.g. `"USD-OIS|5Y"`). Decoding splits
/// on the LAST `'|'` so a curve id containing `'|'` still round-trips
/// (tenor labels never contain one). Plain derived `Serialize` on a
/// tuple-keyed map fails at runtime with "key must be a string" (quant
/// review M10), so these maps must go through this module.
mod curve_tenor_key_map {
    use super::*;
    use serde::de::Error as _;
    use serde::ser::SerializeMap;

    pub fn serialize<S>(map: &CurveTenorPnlMap, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut out = serializer.serialize_map(Some(map.len()))?;
        for ((curve_id, tenor), value) in map {
            out.serialize_entry(&format!("{}|{}", curve_id.as_str(), tenor), value)?;
        }
        out.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<CurveTenorPnlMap, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw: IndexMap<String, Money> = IndexMap::deserialize(deserializer)?;
        raw.into_iter()
            .map(|(key, value)| {
                let (curve, tenor) = key.rsplit_once('|').ok_or_else(|| {
                    D::Error::custom(format!("expected 'curve_id|tenor' key, got {key:?}"))
                })?;
                Ok(((CurveId::new(curve), tenor.to_string()), value))
            })
            .collect()
    }
}

/// Serde representation for `Option<IndexMap<(CurveId, String), Money>>`.
mod opt_curve_tenor_key_map {
    use super::*;

    pub fn serialize<S>(map: &Option<CurveTenorPnlMap>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match map {
            Some(inner) => {
                #[derive(Serialize)]
                struct Wrapper<'a>(
                    #[serde(with = "super::curve_tenor_key_map")]
                    &'a IndexMap<(CurveId, String), Money>,
                );
                serializer.serialize_some(&Wrapper(inner))
            }
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<CurveTenorPnlMap>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper(
            #[serde(with = "super::curve_tenor_key_map")] IndexMap<(CurveId, String), Money>,
        );
        let opt: Option<Wrapper> = Option::deserialize(deserializer)?;
        Ok(opt.map(|w| w.0))
    }
}

/// Serde representation for `IndexMap<(Currency, Currency), Money>` keyed
/// maps: the pair is encoded as `"{from}/{to}"` (e.g. `"EUR/USD"`).
mod currency_pair_key_map {
    use super::*;
    use serde::de::Error as _;
    use serde::ser::SerializeMap;
    use std::str::FromStr;

    pub fn serialize<S>(
        map: &IndexMap<(Currency, Currency), Money>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut out = serializer.serialize_map(Some(map.len()))?;
        for ((from, to), value) in map {
            out.serialize_entry(&format!("{from}/{to}"), value)?;
        }
        out.end()
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<IndexMap<(Currency, Currency), Money>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw: IndexMap<String, Money> = IndexMap::deserialize(deserializer)?;
        raw.into_iter()
            .map(|(key, value)| {
                let (from, to) = key.split_once('/').ok_or_else(|| {
                    D::Error::custom(format!("expected 'FROM/TO' currency pair key, got {key:?}"))
                })?;
                let from = Currency::from_str(from)
                    .map_err(|e| D::Error::custom(format!("bad currency {from:?}: {e}")))?;
                let to = Currency::from_str(to)
                    .map_err(|e| D::Error::custom(format!("bad currency {to:?}: {e}")))?;
                Ok(((from, to), value))
            })
            .collect()
    }
}

/// Hierarchy-level decomposition of credit P&L, opt-in via
/// `AttributionSpec.credit_factor_model`.
///
/// The reconciliation invariant
///
/// ```text
/// generic_pnl + Σ_levels(level.total) + adder_pnl_total + curve_shape_pnl ≡ credit_curves_pnl
/// ```
///
/// holds at absolute tolerance `1e-8` for both metrics-based and Taylor methods.
/// `curve_shape_pnl` is the non-parallel hazard-curve residual (audit item #1);
/// for a purely parallel credit move it is zero and the invariant reduces to
/// the historical `generic + Σ levels + adder` form.
///
/// **Single-instrument scope**: when produced via the valuations-layer
/// per-instrument attribution wire (`metrics_based`, `taylor`), each call
/// processes a single instrument. Therefore `LevelPnl.by_bucket` will contain
/// at most one entry per call (the issuer's bucket at that level).
/// Portfolio-level multi-bucket aggregation is provided at the portfolio layer
/// (PR-8 onward).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreditFactorAttribution {
    /// Deterministic traceability ID for the calibrated model. Format:
    /// `format!("{}/{:016x}", model.as_of, fnv1a64(serde_json::to_string(model)))`.
    pub model_id: String,
    /// P&L attributed to the generic (PC) credit factor:
    /// `Σ_i CS01_i × β_i^PC × ΔF_PC` (canonical signed CS01 = ∂PV/∂s,
    /// negative for long credit — no extra negation).
    pub generic_pnl: Money,
    /// One entry per [`finstack_factor_model::credit::hierarchy::HierarchyDimension`]
    /// in the spec order recorded by the model's hierarchy.
    pub levels: Vec<LevelPnl>,
    /// Total adder P&L: `Σ_i CS01_i × Δadder_i` (canonical signed CS01).
    /// This is the **parallel** issuer-idiosyncratic move only; non-parallel
    /// curve-shape risk is reported separately in [`Self::curve_shape_pnl`].
    ///
    /// **Degenerate (no factor observations) semantics**: when the market
    /// carries no scalar series for any credit factor, nothing about the
    /// issuer's spread move is identifiably systematic, so the entire
    /// parallel ΔS is routed here (generic and level components are zero).
    pub adder_pnl_total: Money,
    /// P&L attributed to the **non-parallel** part of the hazard-curve move
    /// (steepening / twist / term-structure roll) — the curve-shape residual.
    ///
    /// On the linear (MetricsBased / Taylor) wire the parallel steps are
    /// single-CS01 × Δbp products, so this closing residual also absorbs
    /// **spread convexity** of large parallel moves — read it as
    /// "non-parallel + higher-order", not pure curve shape, on that path.
    /// The reprice-based parallel/waterfall cascades distribute convexity
    /// into the steps via cumulative bumps.
    ///
    /// Audit item #1: previously this residual was absorbed into
    /// `adder_pnl_total`, which mislabeled curve-shape risk as
    /// issuer-idiosyncratic. It is now a distinct component. The reconciliation
    /// invariant is correspondingly:
    /// `generic + Σ levels + adder + curve_shape ≡ credit_curves_pnl`.
    ///
    /// `#[serde(default)]` keeps backward compatibility: older serialized
    /// attributions (no `curve_shape_pnl`) deserialize with a zero value.
    #[serde(default = "super::zero_money_usd")]
    pub curve_shape_pnl: Money,
    /// Optional per-issuer adder breakdown (gated by
    /// `CreditFactorDetailOptions.include_per_issuer_adder`, default off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<BTreeMap<String, Money>>")]
    pub adder_pnl_by_issuer: Option<BTreeMap<IssuerId, Money>>,
    /// Diagnostic: absolute magnitude of the per-issuer adder step P&L
    /// (`|adder_pnl_total|`). Surfaced so downstream consumers can detect
    /// when the adder is absorbing significant non-parallel curve moves.
    /// A `tracing::warn!` is also emitted by the cascade builder when the
    /// adder magnitude exceeds `credit_cascade::ADDER_MAGNITUDE_WARN_RATIO`
    /// of total credit P&L.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adder_magnitude: Option<Money>,
}

/// P&L contribution from a single hierarchy level.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LevelPnl {
    /// Human-readable level name (e.g. `"rating"`, `"region"`, `"sector"`,
    /// or a custom dimension key).
    pub level_name: String,
    /// Aggregate P&L for this level across all buckets.
    pub total: Money,
    /// Optional per-bucket breakdown keyed by dotted bucket path
    /// (e.g. `"IG.EU.FIN"`). Empty when
    /// `CreditFactorDetailOptions.include_per_bucket_breakdown == false`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub by_bucket: BTreeMap<String, Money>,
}

/// Detailed attribution for interest rate curves.
///
/// Provides aggregate and per-curve/per-tenor breakdown for discount
/// and forward curves.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RatesCurvesAttribution {
    /// P&L by curve ID.
    pub by_curve: IndexMap<CurveId, Money>,

    /// P&L by (curve_id, tenor), serialized with `"{curve_id}|{tenor}"` keys.
    #[serde(with = "curve_tenor_key_map", default)]
    #[schemars(with = "IndexMap<String, Money>")]
    pub by_tenor: IndexMap<(CurveId, String), Money>,

    /// Total discount curves P&L.
    pub discount_total: Money,

    /// Total forward curves P&L.
    pub forward_total: Money,
}

/// Detailed attribution for credit hazard curves.
///
/// Provides per-curve and per-tenor breakdown for credit spread risk.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreditCurvesAttribution {
    /// P&L by curve ID.
    pub by_curve: IndexMap<CurveId, Money>,

    /// P&L by (curve_id, tenor), serialized with `"{curve_id}|{tenor}"` keys.
    #[serde(with = "curve_tenor_key_map", default)]
    #[schemars(with = "IndexMap<String, Money>")]
    pub by_tenor: IndexMap<(CurveId, String), Money>,
}

/// Detailed attribution for inflation curves.
///
/// Provides per-curve breakdown with optional tenor detail for
/// term-structured inflation curves.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InflationCurvesAttribution {
    /// P&L by curve ID.
    pub by_curve: IndexMap<CurveId, Money>,

    /// P&L by (curve_id, tenor) for term-structured inflation curves,
    /// serialized with `"{curve_id}|{tenor}"` keys.
    #[serde(with = "opt_curve_tenor_key_map", default)]
    #[schemars(with = "Option<IndexMap<String, Money>>")]
    pub by_tenor: Option<IndexMap<(CurveId, String), Money>>,
}

/// Detailed attribution for base correlation curves.
///
/// Used for structured credit products (CDO tranches, synthetic credit).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CorrelationsAttribution {
    /// P&L by correlation curve ID.
    pub by_curve: IndexMap<CurveId, Money>,
}

/// Detailed attribution for FX rate changes.
///
/// Provides per-currency-pair breakdown.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FxAttribution {
    /// P&L by (from_currency, to_currency) pair, serialized with
    /// `"{FROM}/{TO}"` keys (e.g. `"EUR/USD"`).
    #[serde(with = "currency_pair_key_map", default)]
    #[schemars(with = "IndexMap<String, Money>")]
    pub by_pair: IndexMap<(Currency, Currency), Money>,
}

/// Detailed attribution for implied volatility changes.
///
/// Provides per-surface breakdown.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VolAttribution {
    /// P&L by volatility surface ID.
    pub by_surface: IndexMap<CurveId, Money>,
}

/// Detailed attribution for cross-factor interaction terms.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CrossFactorDetail {
    /// Total cross-factor P&L across all populated pairs.
    pub total: Money,

    /// P&L by human-readable factor-pair label.
    #[serde(default)]
    pub by_pair: IndexMap<String, Money>,
}

/// Detailed attribution for model-specific parameters.
///
/// Extensible structure for instrument-specific model parameters
/// (prepayment speeds, default rates, recovery rates, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModelParamsAttribution {
    /// Prepayment speed changes (for MBS/ABS).
    pub prepayment: Option<Money>,

    /// Default rate changes (for structured credit).
    pub default_rate: Option<Money>,

    /// Recovery rate changes (for credit instruments).
    pub recovery_rate: Option<Money>,

    /// Conversion ratio changes (for convertible bonds).
    pub conversion_ratio: Option<Money>,

    /// Other model-specific parameters.
    #[serde(default)]
    pub other: IndexMap<String, Money>,
}

/// One source-line of carry, optionally split into rates / credit components.
///
/// Used for `CarryDetail.coupon_income` and `CarryDetail.roll_down`. When no
/// `CreditFactorModel` is supplied to attribution, `rates_part` and
/// `credit_part` are both `None` and `total` carries the scalar value (legacy
/// behavior). When a model is supplied, the two parts sum to `total` at
/// 1e-8 absolute tolerance (PR-8b §7.1, §7.4 invariants 1 & 2).
///
/// # JSON backward-compat
///
/// Pre-PR-8b JSON encoded `coupon_income` / `roll_down` as a bare `Money`
/// object (`{"amount": ..., "currency": ...}`). The custom `Deserialize`
/// implementation below accepts either:
///   - a legacy `Money` shape → `SourceLine { total: <money>, rates_part: None, credit_part: None }`
///   - the full `SourceLine` shape (`total` + optional `rates_part` / `credit_part`)
///
/// New JSON serialization always uses the `SourceLine` shape.
#[derive(Debug, Clone, Serialize, PartialEq, schemars::JsonSchema)]
pub struct SourceLine {
    /// Total signed amount for this line (always populated).
    pub total: Money,
    /// Rates-only contribution (populated only when a credit factor model
    /// drove the split).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rates_part: Option<Money>,
    /// Credit-only contribution (populated only when a credit factor model
    /// drove the split).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credit_part: Option<Money>,
}

impl SourceLine {
    /// Build a scalar (no-model) source line: only `total` is populated.
    pub fn scalar(total: Money) -> Self {
        Self {
            total,
            rates_part: None,
            credit_part: None,
        }
    }

    /// Build a split source line. The caller is responsible for ensuring
    /// `rates_part + credit_part == total` to within 1e-8.
    pub fn split(total: Money, rates_part: Money, credit_part: Money) -> Self {
        Self {
            total,
            rates_part: Some(rates_part),
            credit_part: Some(credit_part),
        }
    }
}

// Custom Deserialize that accepts either the legacy `Money` shape (object
// with `amount` and `currency`) or the new `SourceLine` shape (object with
// `total` and optional `rates_part` / `credit_part`). Disambiguation is by
// presence of the `total` key — `Money` has no `total` field.
impl<'de> Deserialize<'de> for SourceLine {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Read into a JSON Value first so we can route on shape. This is
        // fine for an `Option<SourceLine>` field: serde only invokes us when
        // a value is present.
        let v = serde_json::Value::deserialize(deserializer)?;
        if let Some(obj) = v.as_object() {
            if obj.contains_key("total") {
                // New shape. `deny_unknown_fields` keeps this inbound surface
                // as strict as the rest of the crate (quant review minor: a
                // typo'd key was previously dropped silently).
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Helper {
                    total: Money,
                    #[serde(default)]
                    rates_part: Option<Money>,
                    #[serde(default)]
                    credit_part: Option<Money>,
                }
                let h: Helper = serde_json::from_value(v).map_err(serde::de::Error::custom)?;
                return Ok(SourceLine {
                    total: h.total,
                    rates_part: h.rates_part,
                    credit_part: h.credit_part,
                });
            }
            if obj.contains_key("amount") && obj.contains_key("currency") {
                // Legacy bare-Money shape.
                let m: Money = serde_json::from_value(v).map_err(serde::de::Error::custom)?;
                return Ok(SourceLine::scalar(m));
            }
        }
        Err(serde::de::Error::custom(
            "expected SourceLine ({total, rates_part?, credit_part?}) or legacy Money ({amount, currency})",
        ))
    }
}

/// Detailed carry decomposition.
///
/// When available, breaks carry into sub-components:
/// - **coupon_income**: Net cashflows (coupons, interest) received during the period
/// - **pull_to_par**: PV convergence toward par (time effect at flat yield)
/// - **roll_down**: Curve shape benefit from aging along a sloped curve
/// - **funding_cost**: Cost of financing the position
/// - **theta**: Total pre-funding carry (before decomposition into sub-components)
///
/// In metrics-based attribution, these fields are populated from pre-computed
/// carry decomposition metrics when available. In repricing-based attribution
/// methods, only a partial breakdown may be available.
///
/// `coupon_income` and `roll_down` are typed as [`SourceLine`] so that
/// callers with a `CreditFactorModel` may further split them into rates and
/// credit components (PR-8b §7.1).
///
/// # Reference
///
/// Bloomberg PORT decomposes carry into Carry (coupon/funding), Curve Roll-Down,
/// and Shift as distinct P&L components.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CarryDetail {
    /// Total carry P&L. Equals `theta + coupon_income` on the repricing-based
    /// paths, or the `CarryTotal` metric on the metrics-based path.
    ///
    /// NOTE: this is **not** the sum of every field below. `theta` is the
    /// pre-decomposition aggregate that already contains `pull_to_par` and
    /// `roll_down`, so summing `theta` together with those sub-lines would
    /// double-count. See each field's own doc for its role in the breakdown.
    pub total: Money,

    /// Coupon/interest income received during the period (with optional
    /// rates / credit split).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coupon_income: Option<SourceLine>,

    /// PV convergence toward par (time effect at flat yield). Unsplit (v1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pull_to_par: Option<Money>,

    /// Curve shape benefit from aging along a sloped curve, with optional
    /// rates / credit split.
    ///
    /// This field includes slide/rolldown effects separate from pure pull-to-par.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roll_down: Option<SourceLine>,

    /// Cost of financing the position. Pure rates, never split.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funding_cost: Option<Money>,

    /// Total pre-funding carry (before decomposition into sub-components).
    /// Residual catch-all, unsplit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theta: Option<Money>,
}

/// Factor-cut decomposition of carry under a calibrated `CreditFactorModel`
/// (PR-8b §7.2). Populated only when an `AttributionSpec.credit_factor_model`
/// was supplied. Purely additive — does not modify any existing field.
///
/// # Reconciliation invariants (§7.4, all at 1e-8 absolute tolerance)
///
/// - `credit_carry_total ≡ Σ_lines SourceLine.credit_part` (lines = coupon + roll)
/// - `credit_carry_total ≡ generic + Σ_levels(level.total) + adder_total`
/// - `rates_carry_total ≡ Σ_lines SourceLine.rates_part − funding_cost`
///
/// # Attribution method coverage
///
/// All four methods populate `carry_detail`: Parallel, Waterfall and Taylor
/// via `apply_total_return_carry` (theta + coupon_income; pull-to-par and
/// funding lines stay `None` on those paths), MetricsBased from the carry
/// decomposition metrics. `credit_carry_decomposition` is therefore emitted
/// on any path whose `carry_detail.coupon_income` is populated when a
/// `CreditFactorModel` is supplied — the decomposition logic is
/// method-agnostic.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreditCarryDecomposition {
    /// Deterministic traceability id of the model used (matches
    /// `CreditFactorAttribution.model_id`).
    pub model_id: String,
    /// Sum of rates components across split source lines, minus funding cost.
    pub rates_carry_total: Money,
    /// Sum of credit components across split source lines.
    pub credit_carry_total: Money,
    /// Per-factor breakdown of `credit_carry_total`.
    pub credit_by_level: CreditCarryByLevel,
}

/// Per-factor breakdown of credit carry (PR-8b §7.2).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreditCarryByLevel {
    /// Generic (PC) factor contribution to credit carry.
    pub generic: Money,
    /// One entry per hierarchy level, in spec order.
    pub levels: Vec<LevelCarry>,
    /// Sum of issuer-specific adder carry contributions.
    pub adder_total: Money,
    /// Optional per-issuer adder breakdown (gated by
    /// `CreditFactorDetailOptions.include_per_issuer_adder`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<BTreeMap<String, Money>>")]
    pub adder_by_issuer: Option<BTreeMap<IssuerId, Money>>,
}

/// Carry contribution from a single hierarchy level.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LevelCarry {
    /// Human-readable level name (e.g. `"rating"`, `"region"`).
    pub level_name: String,
    /// Aggregate carry for this level across all buckets.
    pub total: Money,
    /// Optional per-bucket breakdown keyed by dotted bucket path.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub by_bucket: BTreeMap<String, Money>,
}

/// Detailed attribution for market scalars.
///
/// Includes dividends, equity/commodity prices, inflation indices, etc.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ScalarsAttribution {
    /// Dividend changes by equity ID.
    #[serde(default)]
    pub dividends: IndexMap<CurveId, Money>,

    /// Inflation index changes.
    #[serde(default)]
    pub inflation: IndexMap<CurveId, Money>,

    /// Equity price changes.
    #[serde(default)]
    pub equity_prices: IndexMap<CurveId, Money>,

    /// Commodity price changes.
    #[serde(default)]
    pub commodity_prices: IndexMap<CurveId, Money>,
}
