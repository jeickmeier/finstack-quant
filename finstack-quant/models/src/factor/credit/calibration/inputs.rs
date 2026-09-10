use std::collections::BTreeMap;

use finstack_quant_core::dates::Date;
use finstack_quant_core::types::IssuerId;
use serde::{Deserialize, Serialize};

use crate::factor::credit::hierarchy::{GenericFactorSpec, IssuerTags};

/// Issuer-spread history aligned to a complete regular date grid.
///
/// `dates` is the sorted observation grid. `spreads[issuer]` has length
/// `dates.len()`. Every entry must be `Some(decimal_spread)` — gaps and
/// `None` are rejected at calibration. Callers pass **decimal** spreads
/// (`0.01` = 100 bp).
///
/// Every spread must lie in the open decimal band `(-0.5, 2.0)` — i.e.
/// below 20,000 bp. Deeply distressed quotes at or above 200% running-spread
/// equivalents are rejected as looking like basis points; such names must be
/// excluded from the calibration universe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HistoryPanel {
    /// Observation dates (sorted ascending).
    #[serde(with = "finstack_quant_core::wire::dates")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub dates: Vec<Date>,
    /// Per-issuer decimal spread series aligned with [`dates`][Self::dates].
    ///
    /// Each vector must be fully observed (`Some` at every date). Values are
    /// decimal (`0.01` = 100 bp), converted to bp at calibrate entry.
    pub spreads: BTreeMap<IssuerId, Vec<Option<f64>>>,
}

/// Point-in-time issuer tags at the calibration `as_of`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct IssuerTagPanel {
    /// Tag map keyed by issuer.
    pub tags: BTreeMap<IssuerId, IssuerTags>,
}

/// Generic (PC) factor reference and aligned values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GenericFactorSeries {
    /// Reference (name + series_id) embedded into the artifact.
    pub spec: GenericFactorSpec,
    /// Generic factor values aligned with [`HistoryPanel::dates`].
    ///
    /// Decimal units (`0.01` = 100 bp), same convention as issuer spreads.
    pub values: Vec<f64>,
}

/// All inputs the calibrator needs for a single calibration run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreditCalibrationInputs {
    /// Complete regular issuer-spread history in decimal units.
    pub history_panel: HistoryPanel,
    /// Per-issuer hierarchy tags (point-in-time).
    pub issuer_tags: IssuerTagPanel,
    /// Generic factor series + spec.
    pub generic_factor: GenericFactorSeries,
    /// Calibration anchor date (must appear in `history_panel.dates`).
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,
    /// Issuer spreads at `as_of` in decimal units (level space).
    pub as_of_spreads: BTreeMap<IssuerId, f64>,
    /// Optional annualized idiosyncratic volatility overrides, in decimal
    /// spread per square-root year (`0.001` means 10 bp per square-root year).
    ///
    /// Caller-supplied values take precedence over history, peer-proxy, and
    /// global-default adder-vol estimates. Values must be finite and
    /// non-negative; calibration converts them to the model's bp units.
    pub idiosyncratic_overrides: BTreeMap<IssuerId, f64>,
    /// Option-adjusted spread duration in **years** (`> 0`) per issuer.
    ///
    /// Required when
    /// [`CreditCalibrationConfig::bucket_weighting`][super::config::CreditCalibrationConfig::bucket_weighting]
    /// is [`BucketWeighting::Dts`][super::config::BucketWeighting::Dts].
    /// The historical peel weights each date by contemporaneous DTS
    /// (`SD × panel spread_bp` at that date); the anchor uses as-of DTS.
    /// Persisted on each
    /// [`IssuerBetaRow`][crate::factor::credit::hierarchy::IssuerBetaRow] so
    /// decompose can rebuild DTS from the current spread. The duration is a
    /// single value across the calibration window (no per-date duration
    /// series).
    #[serde(default)]
    pub spread_durations: BTreeMap<IssuerId, f64>,
}
