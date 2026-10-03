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

impl CreditCalibrationInputs {
    /// Build calibration inputs anchored at the end of a history panel.
    ///
    /// The calibrator only accepts an anchor equal to the last panel date
    /// (an earlier `as_of` would leak post-anchor history into the fit), so
    /// this constructor sets `as_of` to the last entry of
    /// `history_panel.dates` and `as_of_spreads` to each issuer's spread on
    /// that date. Issuers whose last observation is `None` are left out of
    /// `as_of_spreads`; calibration then rejects the gap.
    /// `idiosyncratic_overrides` starts empty.
    ///
    /// # Arguments
    ///
    /// * `history_panel` - Sorted, regular issuer-spread history in decimal
    ///   units (`0.01` = 100 bp); its last date becomes the calibration
    ///   anchor and its last row the anchor cross-section.
    /// * `issuer_tags` - Point-in-time hierarchy tags per issuer, keyed by
    ///   the canonical dimension keys (`"rating"`, `"region"`, ...).
    /// * `generic_factor` - Generic (PC) factor reference and values aligned
    ///   with `history_panel.dates`, in decimal units. The caller names the
    ///   series explicitly; nothing is defaulted.
    /// * `spread_durations` - Option-adjusted spread duration in years per
    ///   issuer; may be empty unless DTS bucket weighting is configured.
    ///
    /// # Returns
    ///
    /// Inputs ready for [`super::CreditCalibrator::calibrate`].
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when
    /// `history_panel.dates` is empty. All other checks run at calibration.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::collections::BTreeMap;
    /// use finstack_quant_core::dates::Date;
    /// use finstack_quant_core::types::IssuerId;
    /// use finstack_quant_models::factor::credit::calibration::{
    ///     CreditCalibrationInputs, GenericFactorSeries, HistoryPanel, IssuerTagPanel,
    /// };
    /// use finstack_quant_models::factor::credit::hierarchy::GenericFactorSpec;
    /// use time::Month;
    ///
    /// let dates = vec![
    ///     Date::from_calendar_date(2024, Month::January, 31)?,
    ///     Date::from_calendar_date(2024, Month::February, 29)?,
    /// ];
    /// let panel = HistoryPanel {
    ///     dates: dates.clone(),
    ///     spreads: BTreeMap::from([(IssuerId::new("A"), vec![Some(0.010), Some(0.012)])]),
    /// };
    /// let generic = GenericFactorSeries {
    ///     spec: GenericFactorSpec { name: "CDX IG 5Y".into(), series_id: "cdx.ig.5y".into() },
    ///     values: vec![0.006, 0.007],
    /// };
    /// let inputs = CreditCalibrationInputs::from_panel(
    ///     panel,
    ///     IssuerTagPanel { tags: BTreeMap::new() },
    ///     generic,
    ///     BTreeMap::new(),
    /// )?;
    /// assert_eq!(inputs.as_of, dates[1]);
    /// assert_eq!(inputs.as_of_spreads[&IssuerId::new("A")], 0.012);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_panel(
        history_panel: HistoryPanel,
        issuer_tags: IssuerTagPanel,
        generic_factor: GenericFactorSeries,
        spread_durations: BTreeMap<IssuerId, f64>,
    ) -> finstack_quant_core::Result<Self> {
        let Some(&as_of) = history_panel.dates.last() else {
            return Err(finstack_quant_core::Error::Validation(
                "CreditCalibrationInputs::from_panel: history_panel.dates is empty".into(),
            ));
        };
        let last = history_panel.dates.len() - 1;
        let as_of_spreads = history_panel
            .spreads
            .iter()
            .filter_map(|(issuer, values)| {
                values
                    .get(last)
                    .copied()
                    .flatten()
                    .map(|v| (issuer.clone(), v))
            })
            .collect();
        Ok(Self {
            history_panel,
            issuer_tags,
            generic_factor,
            as_of,
            as_of_spreads,
            idiosyncratic_overrides: BTreeMap::new(),
            spread_durations,
        })
    }
}
