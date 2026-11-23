//! Atomic [`OperationSpec`] market, statement, and instrument shock enum.

use super::types::*;
use super::validation::{check_finite, check_rate_curve_kind};
#[cfg(doc)]
use crate::ScenarioSpec;
use finstack_quant_core::market_data::hierarchy::HierarchyTarget;
use finstack_quant_core::types::CurveId;
use finstack_quant_statements::types::NodeId;
use finstack_quant_valuations::pricer::InstrumentType;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Individual operation within a scenario.
///
/// Each variant represents a specific type of shock or adjustment that can be
/// applied to market data, instruments, statements, or the valuation horizon.
/// Units are encoded in the variant name and field docs:
/// - `Pct` fields use percentage points (`5.0 = +5%`)
/// - `Bp` fields use additive rate basis points (1 bp = 1e-4)
/// - Vol-index `Pts` are **index points** (`1.0` on 18.5 → 19.5)
/// - Correlation / base-corr `Pts` are **decimal correlation** (`0.02` = +0.02)
///
/// Hierarchy-targeted variants are resolved into direct identifiers during
/// [`crate::ScenarioEngine::apply`] using the market hierarchy attached
/// to the execution context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, strum::IntoStaticStr)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[strum(serialize_all = "snake_case")]
pub enum OperationSpec {
    /// FX rate percent shift.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::OperationSpec;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let op = OperationSpec::MarketFxPct {
    ///     base: Currency::USD,
    ///     quote: Currency::EUR,
    ///     pct: 5.0, // +5% means USD strengthens
    /// };
    /// ```
    MarketFxPct {
        /// Base currency in the FX pair.
        base: finstack_quant_core::currency::Currency,
        /// Quote currency in the FX pair.
        quote: finstack_quant_core::currency::Currency,
        /// Percentage change (positive means base strengthens).
        pct: f64,
    },

    /// Equity price percent shock.
    ///
    /// `pct` may be `<= -100`: stocks can be stressed to zero or below in
    /// tail-risk scenarios, since downstream pricers handle non-positive prices
    /// (default events, fair-value floors).
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::OperationSpec;
    ///
    /// let op = OperationSpec::EquityPricePct {
    ///     ids: vec!["SPY".into(), "QQQ".into()],
    ///     pct: -10.0, // -10% price drop
    /// };
    /// ```
    EquityPricePct {
        /// Equity identifiers to shock.
        ids: Vec<String>,
        /// Percentage price change.
        pct: f64,
    },

    /// Instrument price shock by exact attribute match.
    ///
    /// `pct` may be `<= -100`: legitimate write-down stress.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::OperationSpec;
    /// use indexmap::IndexMap;
    ///
    /// let mut attrs = IndexMap::new();
    /// attrs.insert("sector".into(), "Energy".into());
    /// attrs.insert("rating".into(), "BBB".into());
    ///
    /// let op = OperationSpec::InstrumentPricePctByAttr {
    ///     attrs,
    ///     pct: -3.0,
    /// };
    /// ```
    ///
    /// Matching semantics:
    /// - Only instrument metadata key/value pairs are considered (the `meta` map on
    ///   `Attributes`); tag sets are ignored.
    /// - Attribute keys/values compared case-insensitively
    /// - AND semantics (all provided pairs must match metadata)
    /// - An empty attribute map is rejected at validation time. Use
    ///   [`OperationSpec::InstrumentPricePctByType`] with an explicit list of
    ///   instrument types to apply a broad shock.
    InstrumentPricePctByAttr {
        /// Attributes to match. Matching uses AND semantics with
        /// case-insensitive key/value comparison.
        attrs: IndexMap<String, String>,
        /// Percentage price change.
        pct: f64,
    },

    /// Parallel additive shift in rate basis points for a discount, forward,
    /// par CDS or inflation curve. One basis point is 0.0001 in decimal rate space.
    /// Commodity curves use [`OperationSpec::CommodityCurveParallelPct`].
    /// Volatility index curves use [`OperationSpec::VolIndexParallelPts`].
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::{OperationSpec, CurveKind};
    ///
    /// let op = OperationSpec::CurveParallelBp {
    ///     curve_kind: CurveKind::Discount,
    ///     curve_id: "USD_SOFR".into(),
    ///     discount_curve_id: None,
    ///     bp: 50.0, // +50bp parallel shift
    /// };
    /// ```
    CurveParallelBp {
        /// Rate curve family (Discount, Forward, ParCDS or Inflation).
        curve_kind: CurveKind,
        /// Curve identifier.
        curve_id: CurveId,
        /// Optional explicit discount curve identifier when an operation must pair
        /// `curve_id` with a discounting curve (some forward/hazard bumps need one).
        /// If `None`, adapters apply engine default resolution; set this when multiple
        /// curves could match or you need deterministic selection.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        discount_curve_id: Option<CurveId>,
        /// Additive rate shift in basis points.
        bp: f64,
    },

    /// Node-specific additive rate shifts in basis points.
    ///
    /// One basis point is 0.0001 in decimal rate space.
    /// Commodity curves use [`OperationSpec::CommodityCurveNodePct`].
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::{OperationSpec, CurveKind, TenorMatchMode};
    ///
    /// let op = OperationSpec::CurveNodeBp {
    ///     curve_kind: CurveKind::Discount,
    ///     curve_id: "USD_SOFR".into(),
    ///     discount_curve_id: None,
    ///     nodes: vec![
    ///         ("2Y".into(), 25.0),
    ///         ("10Y".into(), -10.0),
    ///     ],
    ///     match_mode: TenorMatchMode::Interpolate,
    /// };
    /// ```
    CurveNodeBp {
        /// Rate curve family (Discount, Forward, ParCDS or Inflation).
        curve_kind: CurveKind,
        /// Curve identifier.
        curve_id: CurveId,
        /// Optional explicit discount curve identifier when an operation must pair
        /// `curve_id` with a discounting curve (some forward/hazard bumps need one).
        /// If `None`, adapters apply engine default resolution; set this when multiple
        /// curves could match or you need deterministic selection.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        discount_curve_id: Option<CurveId>,
        /// Vector of `(tenor, shift)` pairs where shift is in rate basis points.
        nodes: Vec<(String, f64)>,
        /// How to handle tenors not in the curve.
        #[serde(default)]
        match_mode: TenorMatchMode,
    },

    /// Parallel relative price shock to a commodity forward curve.
    CommodityCurveParallelPct {
        /// Identifier of the commodity price curve.
        curve_id: CurveId,
        /// Percentage change, such that `5.0` raises prices by five percent.
        pct: f64,
    },

    /// Node-specific relative price shocks to a commodity forward curve.
    CommodityCurveNodePct {
        /// Identifier of the commodity price curve.
        curve_id: CurveId,
        /// Pairs of tenor and percentage price change; `5.0` raises price by five percent.
        nodes: Vec<(String, f64)>,
        /// Policy for requested tenors between curve pillars.
        #[serde(default)]
        match_mode: TenorMatchMode,
    },

    /// Parallel shock to a volatility-index curve in absolute index points.
    ///
    /// `points` is in absolute vol-index points (e.g. `+0.5` lifts every knot
    /// of a VIX curve by 0.5 vol points). This is intentionally distinct from
    /// the basis-point semantics used by [`OperationSpec::CurveParallelBp`].
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::OperationSpec;
    ///
    /// let op = OperationSpec::VolIndexParallelPts {
    ///     curve_id: "VIX".into(),
    ///     points: 1.0, // +1.0 vol point on every knot
    /// };
    /// ```
    VolIndexParallelPts {
        /// Curve identifier.
        curve_id: CurveId,
        /// Absolute index-point shift.
        points: f64,
    },

    /// Node-specific shock to a volatility-index curve in absolute index points.
    ///
    /// Each `(tenor, points)` entry shifts the matching knot by `points`
    /// absolute vol-index points.
    VolIndexNodePts {
        /// Curve identifier.
        curve_id: CurveId,
        /// Vector of (tenor, points) pairs in absolute vol-index points.
        nodes: Vec<(String, f64)>,
        /// How to handle tenors not in the curve.
        #[serde(default)]
        match_mode: TenorMatchMode,
    },

    /// Parallel shift to base correlation surface (absolute points).
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::OperationSpec;
    ///
    /// let op = OperationSpec::BaseCorrParallelPts {
    ///     surface_id: "CDX_IG".into(),
    ///     points: 0.02, // +0.02 decimal correlation
    /// };
    /// ```
    BaseCorrParallelPts {
        /// Surface identifier.
        surface_id: CurveId,
        /// Absolute shift in decimal correlation (`0.02` = +0.02).
        points: f64,
    },

    /// Bucket-specific base correlation shifts by detachment.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::OperationSpec;
    ///
    /// let op = OperationSpec::BaseCorrBucketPts {
    ///     surface_id: "CDX_IG".into(),
    ///     detachment_bp: Some(vec![300, 700]), // 3% and 7% detachment
    ///     points: 0.02, // +0.02 decimal correlation
    /// };
    /// ```
    BaseCorrBucketPts {
        /// Surface identifier.
        surface_id: CurveId,
        /// Optional detachment points in basis points (e.g., 300 for 3%).
        detachment_bp: Option<Vec<i32>>,
        /// Absolute shift in decimal correlation (`0.02` = +0.02).
        points: f64,
    },

    /// Parallel percent shift to a volatility surface.
    ///
    /// Shocks are **sticky-strike** multiplicative grid moves: each stored
    /// `(expiry, secondary_value)` quote is multiplied by `(100 + pct) / 100`.
    /// Run surface diagnostics separately on the shocked artifact.
    ///
    /// `pct` must satisfy `pct >= -100` (`-100%` collapses vols to `0.0`;
    /// `pct < -100` fails validation).
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::OperationSpec;
    ///
    /// let op = OperationSpec::VolSurfaceParallelPct {
    ///     vol_surface_id: "SPX".into(),
    ///     pct: 10.0, // +10% volatility increase
    /// };
    /// ```
    VolSurfaceParallelPct {
        /// Volatility-surface identifier.
        vol_surface_id: CurveId,
        /// Percentage change in volatility.
        pct: f64,
    },

    /// Bucketed volatility surface shock.
    ///
    /// Same sticky-strike multiplicative convention as
    /// [`OperationSpec::VolSurfaceParallelPct`], restricted to the selected
    /// expiry/strike buckets (`pct >= -100`). Tenor selection requires an
    /// explicit `expiry_clock` on the target surface anchored at the execution
    /// date; strike filtering requires a `Strike` secondary axis.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::OperationSpec;
    ///
    /// let op = OperationSpec::VolSurfaceBucketPct {
    ///     vol_surface_id: "SPX".into(),
    ///     tenors: Some(vec!["1M".into(), "3M".into()]),
    ///     strikes: Some(vec![90.0, 95.0, 100.0]),
    ///     pct: 15.0,
    /// };
    /// ```
    VolSurfaceBucketPct {
        /// Volatility-surface identifier.
        vol_surface_id: CurveId,
        /// Optional tenor strings (e.g., "1M", "3M").
        tenors: Option<Vec<String>>,
        /// Optional strike levels.
        strikes: Option<Vec<f64>>,
        /// Percentage change in volatility.
        pct: f64,
    },

    /// Statement forecast percent change.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::OperationSpec;
    ///
    /// let op = OperationSpec::StmtForecastPercent {
    ///     node_id: "Revenue".into(),
    ///     pct: -5.0, // -5% revenue decrease
    /// };
    /// ```
    StmtForecastPercent {
        /// Statement node identifier.
        node_id: NodeId,
        /// Percentage change to apply.
        pct: f64,
    },

    /// Statement forecast value assignment.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::OperationSpec;
    ///
    /// let op = OperationSpec::StmtForecastAssign {
    ///     node_id: "Revenue".into(),
    ///     value: 1_000_000.0,
    /// };
    /// ```
    StmtForecastAssign {
        /// Statement node identifier.
        node_id: NodeId,
        /// Finite absolute value in the node's units; monetary nodes retain
        /// their currency and interpret this as major currency units.
        value: f64,
    },

    /// Bind a statement rate node to a market curve for this scenario.
    ///
    /// The rate at the specified tenor is extracted from the curve and applied
    /// to the node, overriding any static value. If
    /// [`ExecutionContext::rate_bindings`](crate::engine::ExecutionContext::rate_bindings)
    /// is initialized with `Some(_)`, the binding is also registered there so
    /// later scenario applications can keep the statement node synchronized with
    /// market curve rolls and shocks. When the context field is `None`, this
    /// operation performs a one-shot update and does not persist the binding.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::{OperationSpec, RateBindingSpec};
    /// use finstack_quant_core::math::Compounding;
    ///
    /// let op = OperationSpec::RateBinding {
    ///     binding: RateBindingSpec {
    ///         node_id: "InterestExpense".into(),
    ///         curve_id: "USD-OIS".into(),
    ///         tenor: "1Y".into(),
    ///         compounding: Compounding::Periodic(std::num::NonZeroU32::MIN),
    ///         day_count: None,
    ///     },
    /// };
    /// ```
    RateBinding {
        /// The rate binding specification to apply.
        binding: RateBindingSpec,
    },

    /// Instrument spread shock by exact attribute match (FI only).
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::OperationSpec;
    /// use indexmap::IndexMap;
    ///
    /// let mut attrs = IndexMap::new();
    /// attrs.insert("rating".into(), "CCC".into());
    ///
    /// let op = OperationSpec::InstrumentSpreadBpByAttr {
    ///     attrs,
    ///     bp: 50.0,
    /// };
    /// ```
    ///
    /// Matching semantics:
    /// - Only instrument metadata key/value pairs are considered (the `meta` map on
    ///   `Attributes`); tag sets are ignored.
    /// - Attribute keys/values compared case-insensitively
    /// - AND semantics (all provided pairs must match metadata)
    /// - An empty attribute map is rejected at validation time. Use
    ///   [`OperationSpec::InstrumentSpreadBpByType`] with an explicit list of
    ///   instrument types to apply a broad shock.
    InstrumentSpreadBpByAttr {
        /// Attributes to match. Matching uses AND semantics with
        /// case-insensitive key/value comparison.
        attrs: IndexMap<String, String>,
        /// Basis point shift to apply.
        bp: f64,
    },

    /// Instrument price shock by type.
    ///
    /// `pct` may be `<= -100`: legitimate write-down stress.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::OperationSpec;
    /// use finstack_quant_valuations::pricer::InstrumentType;
    ///
    /// let op = OperationSpec::InstrumentPricePctByType {
    ///     instrument_types: vec![InstrumentType::Bond, InstrumentType::TermLoan],
    ///     pct: -5.0,
    /// };
    /// ```
    InstrumentPricePctByType {
        /// Instrument types to shock.
        instrument_types: Vec<InstrumentType>,
        /// Percentage price change.
        pct: f64,
    },

    /// Instrument spread shock by type.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::OperationSpec;
    /// use finstack_quant_valuations::pricer::InstrumentType;
    ///
    /// let op = OperationSpec::InstrumentSpreadBpByType {
    ///     instrument_types: vec![InstrumentType::Cds],
    ///     bp: 100.0,
    /// };
    /// ```
    InstrumentSpreadBpByType {
        /// Instrument types to shock.
        instrument_types: Vec<InstrumentType>,
        /// Basis point shift to apply.
        bp: f64,
    },

    // Structured Credit Correlation Operations
    /// Shock asset correlation for structured credit instruments.
    ///
    /// Applies a parallel shift to asset correlation parameters. For flat
    /// correlation structures, this bumps the single asset correlation. For
    /// sectored structures, it bumps both intra-sector and inter-sector
    /// correlations (inter-sector at half the rate).
    ///
    /// # Arguments
    /// - `delta_pts`: Additive shock in decimal correlation (`0.02` = +0.02)
    ///
    /// # Clamping
    /// Asset correlation is clamped to [0, 0.99] after the shock.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::OperationSpec;
    ///
    /// let op = OperationSpec::AssetCorrelationPts {
    ///     delta_pts: 0.02, // +0.02 decimal correlation
    /// };
    /// ```
    AssetCorrelationPts {
        /// Additive shock in decimal correlation (`0.02` = +0.02).
        delta_pts: f64,
    },

    /// Shock prepay-default correlation for structured credit instruments.
    ///
    /// Applies a parallel shift to the correlation between prepayment and
    /// default rates. This is typically negative (higher prepayment = lower
    /// defaults due to selection effects).
    ///
    /// # Arguments
    /// - `delta_pts`: Additive shock in decimal correlation (`0.02` = +0.02)
    ///
    /// # Clamping
    /// Prepay-default correlation is clamped to [-0.99, 0.99].
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::OperationSpec;
    ///
    /// let op = OperationSpec::PrepayDefaultCorrelationPts {
    ///     delta_pts: 0.10, // +0.10 decimal correlation (less negative)
    /// };
    /// ```
    PrepayDefaultCorrelationPts {
        /// Additive shock in decimal correlation (`0.02` = +0.02).
        delta_pts: f64,
    },

    // Hierarchy-targeted operations (expanded to direct ops at execution time)
    /// Hierarchy-targeted parallel curve shift.
    ///
    /// During [`crate::ScenarioEngine::apply`], the engine resolves
    /// `target` against the market-data hierarchy and expands this into one
    /// [`OperationSpec::CurveParallelBp`] per matched curve. When multiple
    /// hierarchy operations reach the same curve, [`ScenarioSpec::resolution_mode`]
    /// determines whether they accumulate or only the most specific match survives.
    HierarchyCurveParallelBp {
        /// Rate curve family (Discount, Forward, ParCDS or Inflation).
        curve_kind: CurveKind,
        /// Hierarchy target to resolve to curves.
        target: HierarchyTarget,
        /// Additive rate shift in basis points.
        bp: f64,
        /// Optional discount curve for hazard-curve recalibration.
        ///
        /// Propagated to each expanded [`OperationSpec::CurveParallelBp`].
        /// If `None`, the adapter applies heuristic resolution.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        discount_curve_id: Option<CurveId>,
    },

    /// Apply a relative price shock to every commodity curve matching the hierarchy target.
    HierarchyCommodityCurveParallelPct {
        /// Predicate identifying the target commodity curves.
        target: HierarchyTarget,
        /// Percentage change, such that `5.0` raises prices by five percent.
        pct: f64,
    },

    /// Hierarchy-targeted vol-surface parallel shift.
    ///
    /// Resolution follows the same hierarchy-expansion rules as
    /// [`OperationSpec::HierarchyCurveParallelBp`], expanding into
    /// [`OperationSpec::VolSurfaceParallelPct`] per matched surface (`pct >= -100`).
    HierarchyVolSurfaceParallelPct {
        /// Hierarchy target to resolve to surfaces.
        target: HierarchyTarget,
        /// Percentage change in volatility.
        pct: f64,
    },

    /// Hierarchy-targeted equity price shift.
    ///
    /// Resolution expands the hierarchy target into one direct equity-price
    /// shock per matched identifier. Percent inputs use percentage points.
    HierarchyEquityPricePct {
        /// Hierarchy target to resolve to equity IDs.
        target: HierarchyTarget,
        /// Percentage price change.
        pct: f64,
    },

    /// Hierarchy-targeted base-correlation parallel shift.
    ///
    /// Resolution expands the hierarchy target into one direct
    /// [`OperationSpec::BaseCorrParallelPts`] per matched surface.
    HierarchyBaseCorrParallelPts {
        /// Hierarchy target to resolve to surfaces.
        target: HierarchyTarget,
        /// Absolute shift in decimal correlation (`0.02` = +0.02).
        points: f64,
    },

    /// Roll the valuation horizon forward before any later scenario operations run.
    ///
    /// `period` uses tenor-style strings such as `"1D"`, `"1W"`, `"1M"`, and
    /// `"1Y"`. [`TimeRollMode`] controls whether the roll uses business-day-aware
    /// date arithmetic, pure calendar arithmetic, or fixed approximate day counts.
    ///
    /// This operation is handled in phase 0 of
    /// [`crate::ScenarioEngine::apply`], before market or statement
    /// shocks. If `apply_shocks` is `false`, the engine stops after the time roll
    /// and returns an [`crate::engine::ApplicationReport`] without applying the
    /// remaining operations in the scenario.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_scenarios::{OperationSpec, TimeRollMode};
    ///
    /// let op = OperationSpec::TimeRollForward {
    ///     period: "1M".into(),
    ///     apply_shocks: true,
    ///     roll_mode: TimeRollMode::BusinessDays,
    /// };
    /// ```
    TimeRollForward {
        /// Period to roll forward (e.g., "1D", "1W", "1M", "1Y")
        period: String,
        /// Whether to continue applying the remaining scenario operations after the roll.
        ///
        /// When `false`, [`crate::ScenarioEngine::apply`] returns
        /// immediately after the roll-forward step.
        #[serde(default = "default_true")]
        apply_shocks: bool,
        /// Roll mode controlling calendar vs business-day behaviour.
        ///
        /// Defaults to `BusinessDays`, which respects calendars when provided.
        #[serde(default)]
        roll_mode: TimeRollMode,
    },
}

impl OperationSpec {
    /// Build a [`OperationSpec::TimeRollForward`] with the wire defaults for
    /// omitted fields.
    ///
    /// The host bindings call this so an omitted `apply_shocks` / `roll_mode`
    /// resolves exactly as it does when the field is absent from JSON.
    ///
    /// # Arguments
    ///
    /// * `period` - Tenor-style roll period such as `"1D"`, `"1W"`, `"1M"` or
    ///   `"1Y"`. Stored verbatim; [`OperationSpec::validate`] parses it.
    /// * `apply_shocks` - Whether the engine applies the scenario's remaining
    ///   operations after the roll. `None` uses the serde default, `true`.
    /// * `roll_mode` - Calendar-vs-business-day semantics of the roll. `None`
    ///   uses [`TimeRollMode::default`] ([`TimeRollMode::BusinessDays`]).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_scenarios::{OperationSpec, TimeRollMode};
    ///
    /// let op = OperationSpec::time_roll_forward("1M", None, None);
    /// assert_eq!(
    ///     op,
    ///     OperationSpec::TimeRollForward {
    ///         period: "1M".into(),
    ///         apply_shocks: true,
    ///         roll_mode: TimeRollMode::BusinessDays,
    ///     }
    /// );
    /// ```
    #[must_use]
    pub fn time_roll_forward(
        period: impl Into<String>,
        apply_shocks: Option<bool>,
        roll_mode: Option<TimeRollMode>,
    ) -> Self {
        Self::TimeRollForward {
            period: period.into(),
            apply_shocks: apply_shocks.unwrap_or_else(default_true),
            roll_mode: roll_mode.unwrap_or_default(),
        }
    }

    /// Expand one parallel basis-point shift into one
    /// [`OperationSpec::CurveParallelBp`] per curve identifier.
    ///
    /// # Arguments
    ///
    /// * `curve_kind` - Curve family every generated operation targets.
    /// * `curve_ids` - Curve identifiers, one operation each, in the given order.
    /// * `bp` - Additive rate shift in basis points (1 bp is 0.0001).
    /// * `discount_curve_id` - Optional discount curve used when re-bootstrapping
    ///   ParCDS quotes; copied onto every generated operation.
    ///
    /// # Errors
    /// Rejects a commodity curve kind or another invalid operation input.
    pub fn parallel_bp_many<I, S>(
        curve_kind: CurveKind,
        curve_ids: I,
        bp: f64,
        discount_curve_id: Option<CurveId>,
    ) -> crate::error::Result<Vec<Self>>
    where
        I: IntoIterator<Item = S>,
        S: Into<CurveId>,
    {
        check_rate_curve_kind(curve_kind)?;
        check_finite(bp, "bp")?;
        curve_ids
            .into_iter()
            .map(|curve_id| {
                let op = Self::CurveParallelBp {
                    curve_kind,
                    curve_id: curve_id.into(),
                    discount_curve_id: discount_curve_id.clone(),
                    bp,
                };
                op.validate()?;
                Ok(op)
            })
            .collect()
    }

    /// Wire name of this operation: the serde `kind` tag.
    ///
    /// # Returns
    ///
    /// The snake-case variant name, e.g. `"curve_parallel_bp"` for
    /// [`OperationSpec::CurveParallelBp`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_scenarios::OperationSpec;
    ///
    /// let op = OperationSpec::time_roll_forward("1M", None, None);
    /// assert_eq!(op.kind(), "time_roll_forward");
    /// ```
    #[must_use]
    pub fn kind(&self) -> &'static str {
        self.into()
    }

    /// Whether this operation needs instrument access during application.
    ///
    /// # Returns
    ///
    /// `true` for instrument-mutating operations and time rolls.
    pub fn requires_instruments(&self) -> bool {
        self.mutates_instruments() || matches!(self, Self::TimeRollForward { .. })
    }

    /// Whether this operation targets the market-data hierarchy and is expanded
    /// into direct operations before application.
    pub(crate) fn is_hierarchy(&self) -> bool {
        matches!(
            self,
            Self::HierarchyCurveParallelBp { .. }
                | Self::HierarchyCommodityCurveParallelPct { .. }
                | Self::HierarchyVolSurfaceParallelPct { .. }
                | Self::HierarchyEquityPricePct { .. }
                | Self::HierarchyBaseCorrParallelPts { .. }
        )
    }

    /// Whether this operation can replace or mutate portfolio instruments.
    ///
    /// # Returns
    ///
    /// `true` for price, spread, and structured-credit correlation shocks.
    pub fn mutates_instruments(&self) -> bool {
        matches!(
            self,
            Self::InstrumentPricePctByAttr { .. }
                | Self::InstrumentSpreadBpByAttr { .. }
                | Self::InstrumentPricePctByType { .. }
                | Self::InstrumentSpreadBpByType { .. }
                | Self::AssetCorrelationPts { .. }
                | Self::PrepayDefaultCorrelationPts { .. }
        )
    }
}

fn default_true() -> bool {
    true
}
