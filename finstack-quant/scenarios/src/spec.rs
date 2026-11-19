//! Serializable scenario specifications (serde-stable).
//!
//! This module defines the data contract that callers use to describe market,
//! instrument, statement, and time-roll shocks. Types here are intentionally
//! lightweight and focus on configuration rather than behaviour so they can be
//! shared across binaries, configuration files, and JSON/YAML APIs.
//!
//! Start with [`ScenarioSpec`] and [`OperationSpec`]. Use [`RateBindingSpec`]
//! when statement nodes should follow curve-driven rates, and use
//! [`HierarchyTarget`](finstack_quant_core::market_data::hierarchy::HierarchyTarget) together with [`ResolutionMode`](finstack_quant_core::market_data::hierarchy::ResolutionMode)
//! for hierarchy-expanded shocks.
//!
//! # Conventions
//!
//! - Percent shocks use percentage-point inputs (`5.0 = +5%`).
//! - Curve and spread shocks quoted in `bp` use additive basis points
//!   (1 bp = 1e-4). Commodity shocks quoted in `pct` scale forward prices.
//! - Vol-index `Pts` are index points (`1.0` on 18.5 → 19.5).
//! - Correlation / base-corr `Pts` are decimal correlation (`0.02` = +0.02).
//! - Time-roll operations distinguish business-day-aware, calendar-day, and
//!   approximate day-count semantics via [`TimeRollMode`].
//! - Hierarchy-targeted operations expand to direct market-data identifiers at
//!   execution time; [`ScenarioSpec::resolution_mode`] determines whether
//!   overlapping hierarchy matches accumulate or collapse to the most specific
//!   match.

use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::hierarchy::HierarchyTarget;
use finstack_quant_core::market_data::hierarchy::ResolutionMode;
use finstack_quant_core::math::Compounding;
use finstack_quant_core::types::CurveId;
use finstack_quant_statements::types::NodeId;
use finstack_quant_valuations::pricer::InstrumentType;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// A complete scenario specification with metadata and ordered operations.
///
/// # Fields
/// - `id`: Stable identifier used for persistence and reporting.
/// - `name`: Optional display name for UI or logs.
/// - `description`: Optional text describing the intent of the scenario.
/// - `operations`: Ordered list of [`OperationSpec`] values to execute.
/// - `priority`: Used by [`ScenarioSpec::compose`](crate::spec::ScenarioSpec::compose)
///   to determine merge ordering (lower numbers run first).
/// - `resolution_mode`: Controls how hierarchy-targeted shocks at multiple tree
///   levels combine for a single curve. Defaults to [`ResolutionMode::MostSpecificWins`].
///
/// # Examples
///
/// ```rust
/// use finstack_quant_scenarios::{ScenarioSpec, OperationSpec, CurveKind};
/// use finstack_quant_core::market_data::hierarchy::ResolutionMode;
///
/// let scenario = ScenarioSpec {
///     id: "stress_test".into(),
///     name: Some("Q1 Stress".into()),
///     description: Some("Rate shock scenario".into()),
///     operations: vec![
///         OperationSpec::CurveParallelBp {
///             curve_kind: CurveKind::Discount,
///             curve_id: "USD_SOFR".into(),
///             discount_curve_id: None,
///             bp: 50.0,
///         },
///     ],
///     priority: 0,
///     resolution_mode: ResolutionMode::default(),
/// };
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ScenarioSpec {
    /// Unique identifier for this scenario.
    pub id: String,

    /// Optional human-readable name.
    #[serde(default)]
    pub name: Option<String>,

    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,

    /// List of operations to apply.
    pub operations: Vec<OperationSpec>,

    /// Priority for composition (lower value = higher priority).
    #[serde(default)]
    pub priority: i32,

    /// Resolution mode for hierarchy-targeted operations.
    ///
    /// Only relevant when operations use hierarchy targeting.
    /// Default: [`ResolutionMode::MostSpecificWins`].
    #[serde(default)]
    pub resolution_mode: ResolutionMode,
}

impl ScenarioSpec {
    /// Strict composition: merge scenarios into a single spec, sorted by
    /// ascending priority, returning an error at compose time when the
    /// concatenated operations would be rejected at apply time.
    ///
    /// Every input is validated first (see [`ScenarioSpec::validate`]), so
    /// the composed spec is built only from valid scenarios. Composition then
    /// rejects scenarios that together contain more than one
    /// [`OperationSpec::TimeRollForward`].
    ///
    /// # Merge rules
    ///
    /// Inputs are stably sorted by ascending `priority` (equal priorities keep
    /// input order); every rule below follows that execution order.
    ///
    /// | Field | Composed value |
    /// |-------|----------------|
    /// | `id` | Input ids joined with `+` (`"composed"` when empty) |
    /// | `name` | Input names (the id where a name is absent) joined with `" + "` |
    /// | `description` | Present input descriptions joined with `" + "`; `None` when no input has one |
    /// | `operations` | Input operations concatenated in execution order |
    /// | `priority` | The lowest input priority, so the composed spec sorts where its first-running input did (`0` when empty) |
    /// | `resolution_mode` | The shared mode (inputs must agree; [`ResolutionMode::default`] when empty) |
    ///
    /// # Arguments
    ///
    /// * `scenarios` - Scenario specifications to merge in ascending priority
    ///   order. Every non-empty input must use the same `resolution_mode`;
    ///   conflicting modes are rejected before composition.
    ///
    /// # Errors
    ///
    /// Returns a validation error, prefixed with the offending scenario id,
    /// when any input fails [`ScenarioSpec::validate`] (blank id, invalid or
    /// non-finite operation fields, more than one time roll), and a validation
    /// error when `scenarios` contain conflicting `resolution_mode` values or
    /// together contain more than one time-roll operation.
    pub fn compose(mut scenarios: Vec<ScenarioSpec>) -> crate::Result<Self> {
        for scenario in &scenarios {
            scenario.validate().map_err(|error| {
                crate::error::Error::validation(format!(
                    "Cannot compose scenario '{}': {error}",
                    scenario.id
                ))
            })?;
        }
        if let Some(first) = scenarios.first()
            && let Some(conflicting) = scenarios
                .iter()
                .skip(1)
                .find(|scenario| scenario.resolution_mode != first.resolution_mode)
        {
            return Err(crate::error::Error::validation(format!(
                "Cannot compose scenarios '{}' (resolution_mode '{}') and '{}' \
                     (resolution_mode '{}'): all scenarios must use the same resolution_mode.",
                first.id,
                finstack_quant_core::wire::serde_label(&first.resolution_mode)?,
                conflicting.id,
                finstack_quant_core::wire::serde_label(&conflicting.resolution_mode)?,
            )));
        }
        // Stable so equal-priority scenarios keep input order.
        scenarios.sort_by_key(|s| s.priority);

        let composed_id = if scenarios.is_empty() {
            "composed".to_string()
        } else {
            scenarios
                .iter()
                .map(|scenario| scenario.id.as_str())
                .collect::<Vec<_>>()
                .join("+")
        };
        let composed_name = if scenarios.is_empty() {
            Some("Composed Scenario".to_string())
        } else {
            Some(
                scenarios
                    .iter()
                    .map(|scenario| scenario.name.as_deref().unwrap_or(scenario.id.as_str()))
                    .collect::<Vec<_>>()
                    .join(" + "),
            )
        };
        let resolution_mode = scenarios
            .first()
            .map_or_else(ResolutionMode::default, |scenario| scenario.resolution_mode);
        let priority = scenarios.first().map_or(0, |scenario| scenario.priority);
        let descriptions: Vec<&str> = scenarios
            .iter()
            .filter_map(|scenario| scenario.description.as_deref())
            .collect();
        let description = (!descriptions.is_empty()).then(|| descriptions.join(" + "));

        let mut all_operations = Vec::new();
        for scenario in scenarios {
            all_operations.extend(scenario.operations);
        }

        let time_roll_count = all_operations
            .iter()
            .filter(|op| matches!(op, OperationSpec::TimeRollForward { .. }))
            .count();
        if time_roll_count > 1 {
            return Err(crate::error::Error::validation(format!(
                "Compose would produce {time_roll_count} TimeRollForward operations; only \
                 one is allowed per composed scenario. Merge the roll periods into a single \
                 `TimeRollForward` (preferred) or remove the duplicates before calling compose."
            )));
        }

        Ok(Self {
            id: composed_id,
            name: composed_name,
            description,
            operations: all_operations,
            priority,
            resolution_mode,
        })
    }

    /// Whether applying this scenario needs the portfolio's instruments.
    ///
    /// Instrument-mutating shocks and time rolls need instrument access.
    /// Market-only, statement-only, and rate-binding operations do not.
    ///
    /// # Returns
    ///
    /// `true` when the execution context must include instruments.
    pub fn requires_instruments(&self) -> bool {
        self.operations
            .iter()
            .any(OperationSpec::requires_instruments)
    }

    /// Whether applying this scenario can replace or mutate instruments.
    ///
    /// # Returns
    ///
    /// `true` for instrument price, spread, or correlation shocks. A time roll
    /// reads instruments for carry but does not mutate them and therefore
    /// returns `false`.
    pub fn mutates_instruments(&self) -> bool {
        self.operations
            .iter()
            .any(OperationSpec::mutates_instruments)
    }
}

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
/// [`crate::engine::ScenarioEngine::apply`] using the market hierarchy attached
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
    /// `(expiry, secondary_value)` vol is multiplied by `1 + pct/100`. This is
    /// not a sticky-delta re-mark. The post-shock fixed-strike calendar-spread
    /// check applies only to `Strike`-axis `BlackLognormal` surfaces and is a
    /// heuristic screen, not a no-arbitrage certificate.
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
    /// date; strike filtering requires a `Strike` secondary axis. The
    /// calendar-spread check applies only to `Strike`-axis `BlackLognormal`
    /// surfaces and is not a no-arbitrage certificate.
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
    /// During [`crate::engine::ScenarioEngine::apply`], the engine resolves
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
    /// [`crate::engine::ScenarioEngine::apply`], before market or statement
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
        /// When `false`, [`crate::engine::ScenarioEngine::apply`] returns
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

/// Identifies which family of curve an operation targets.
///
/// These variants map to the market data collections exposed by
/// `finstack_quant_core::market_data::context::MarketContext`.
/// They also determine which quoting and interpolation conventions apply when
/// downstream helpers extract rates or apply node shocks.
///
/// # Examples
/// ```rust
/// use finstack_quant_scenarios::CurveKind;
///
/// let kind = CurveKind::Discount;
/// assert_eq!(format!("{:?}", kind), "Discount");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CurveKind {
    /// Discount factor curve.
    Discount,
    /// Forward rate curve.
    #[serde(rename = "forward")]
    Forward,
    /// Credit Par CDS curve (bumping spreads)
    #[serde(rename = "par_cds")]
    ParCDS,
    /// Inflation index curve.
    Inflation,
    /// Commodity forward [`PriceCurve`](finstack_quant_core::market_data::term_structures::PriceCurve).
    ///
    /// Price shocks use [`OperationSpec::CommodityCurveParallelPct`] or
    /// [`OperationSpec::CommodityCurveNodePct`].
    Commodity,
}

/// Strategy for aligning requested tenor bumps with curve pillars.
///
/// [`TenorMatchMode::Exact`] requires the requested tenor to coincide with an
/// existing pillar. [`TenorMatchMode::Interpolate`] distributes the bump
/// across the two adjacent knots, then **calibrates those pillar deltas so
/// the curve's native interpolant hits the requested shock at the requested
/// tenor** (discount zeros, inflation implied annual rates, forward rates,
/// and commodity price forwards). The initial guess is the minimum-norm
/// `1/Σw²` split; a few scale/Newton iterations adjust it when the live
/// interpolant is not linear-on-rate. On-pillar requests reduce to a single
/// full-size pillar bump. Delivery is first-order only for par-CDS
/// solve-to-par recalibration, which still emits
/// [`Warning::InterpolatedNodeBumpFirstOrder`](crate::warning::Warning::InterpolatedNodeBumpFirstOrder).
///
/// # Examples
/// ```rust
/// use finstack_quant_scenarios::TenorMatchMode;
///
/// let mode = TenorMatchMode::Interpolate;
/// assert_eq!(format!("{:?}", mode), "Interpolate");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TenorMatchMode {
    /// Match exact pillar only (errors if not found).
    Exact,
    /// Use key-rate bump at interpolated time.
    #[default]
    Interpolate,
}

/// Controls how time roll-forward periods are interpreted.
///
/// Use [`TimeRollMode::BusinessDays`] when the scenario should respect holiday
/// calendars and business-day adjustment rules. Use
/// [`TimeRollMode::CalendarDays`] when the tenor should be added without a
/// business-day adjustment. [`TimeRollMode::Approximate`] uses fixed day-count
/// approximations aligned with the internal period-to-day conversion.
///
/// # Reported day counts are always calendar days
///
/// Every mode reports `RollForwardReport::days` (and `HorizonResult::horizon_days`)
/// as the *calendar-day* span between the old and new as-of dates. `BusinessDays`
/// adjusts the **target date**, not the unit of the count: a `1M` business-day
/// roll whose target is carried from a Saturday to the following Monday reports
/// 33 calendar days, not 31 and not a business-day tally. Downstream ACT/365F
/// annualization depends on this.
///
/// # Non-additivity of `Approximate`
///
/// [`TimeRollMode::Approximate`] is **not additive across composed rolls**.
/// Month periods round `months * 365 / 12` independently for each operation.
/// For example, `6M`
/// resolves to 183 days, so two `6M` approximate rolls produce 366 days while
/// one `12M` or `1Y` roll resolves to 365 days. For chained horizons or
/// scenario composition prefer
/// [`TimeRollMode::BusinessDays`] or [`TimeRollMode::CalendarDays`], which
/// both resolve the target date via [`finstack_quant_core::dates::Tenor`] and are
/// additive modulo the chosen business-day convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TimeRollMode {
    /// Business-day-adjusted roll (ModifiedFollowing).
    ///
    /// The tenor is added to the as-of date and the result is adjusted with
    /// [`BusinessDayConvention::ModifiedFollowing`]. When the execution context
    /// carries no calendar, [`finstack_quant_core::dates::WEEKENDS_ONLY`] is
    /// used, so this mode always avoids weekends; supply a calendar for holiday
    /// awareness.
    ///
    /// [`BusinessDayConvention::ModifiedFollowing`]: finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing
    #[default]
    BusinessDays,
    /// Pure calendar-day addition (no business-day adjustment even if a calendar exists).
    CalendarDays,
    /// Explicit approximate mode using fixed day counts and rounded `365 / 12` months.
    ///
    /// See the enum-level "Non-additivity" note: composed rolls in this mode
    /// do not commute with calendar arithmetic (`6M + 6M = 366d != 1Y`).
    Approximate,
}

/// Configuration for rate binding between curves and statement nodes.
///
/// Specifies how to extract a rate from a market curve and link it to a
/// statement forecast node. This enables automatic propagation of market
/// rate shocks to financial statement projections.
///
/// The extracted rate is written into the statement model as a decimal scalar
/// (for example `0.0525` for 5.25%). `compounding` controls the output quote
/// convention, while `day_count` optionally overrides the curve's native day
/// count when converting `tenor` into a year fraction. Monetary target nodes
/// are rejected during application because a rate is dimensionless.
///
/// Persisted `day_count` override values are the canonical snake_case
/// [`DayCount`] labels: `act_360`, `act_365f`, `act_365l`, `nl_365`, `30_360`,
/// `30e_360`, `30e_360_isda`, `act_act`, `act_act_isma`, and `bus_252`.
///
/// # Examples
/// ```rust
/// use finstack_quant_scenarios::RateBindingSpec;
/// use finstack_quant_core::math::Compounding;
///
/// let binding = RateBindingSpec {
///     node_id: "InterestRate".into(),
///     curve_id: "USD_SOFR".into(),
///     tenor: "1Y".into(),
///     compounding: Compounding::Continuous,
///     day_count: None, // Use curve's day count
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RateBindingSpec {
    /// Statement node ID to receive the rate.
    pub node_id: NodeId,

    /// Curve ID to extract rate from.
    pub curve_id: CurveId,

    /// Tenor for rate extraction (e.g., "3M", "1Y").
    pub tenor: String,

    /// Compounding convention for rate conversion.
    #[serde(default = "default_continuous_compounding")]
    pub compounding: Compounding,

    /// Output quote day-count convention. If `None`, uses the curve's convention.
    /// Changes the annualized rate while preserving maturity dates and the
    /// curve-implied accumulation factor; native curve query times are unchanged.
    #[serde(default)]
    pub day_count: Option<DayCount>,
}

const fn default_continuous_compounding() -> Compounding {
    Compounding::Continuous
}

impl RateBindingSpec {
    /// Validate identifiers and eagerly parse the tenor.
    ///
    /// This standalone check is also used by [`OperationSpec::validate`] so
    /// callers validating a binding before embedding it receive identical
    /// tenor and day-count diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Validation`] when `node_id` or
    /// `curve_id` is blank, `tenor` is not accepted by
    /// [`finstack_quant_core::dates::Tenor::parse`]. Invalid persisted
    /// `day_count` values are rejected directly by serde.
    pub fn validate(&self) -> crate::error::Result<()> {
        check_id(self.node_id.as_str(), "node_id")?;
        check_id(self.curve_id.as_str(), "curve_id")?;
        if self.tenor.trim().is_empty() {
            return Err(crate::error::Error::Validation(
                "RateBinding tenor cannot be empty".into(),
            ));
        }
        finstack_quant_core::dates::Tenor::parse(self.tenor.trim())
            .map_err(|error| crate::error::Error::InvalidTenor(error.to_string()))?;
        Ok(())
    }
}

/// Structural validation diagnostics for a scenario specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ScenarioValidationReport {
    /// Whether the specification passed all structural checks.
    pub valid: bool,
    /// Failures in specification and operation order.
    pub errors: Vec<String>,
    /// Advisory diagnostics associated with the specification.
    pub warnings: Vec<String>,
    /// Whether at least one operation needs an instrument inventory.
    pub requires_instruments: bool,
    /// Whether applying the scenario can modify instrument state.
    pub mutates_instruments: bool,
    /// Number of operations in the submitted specification.
    pub operation_count: usize,
}

/// Check a scenario's structure and report diagnostics without applying shocks.
///
/// # Arguments
/// * `spec` - Scenario whose identifiers, timing and operations are checked.
///   Market and instrument lookups occur only when the scenario is applied.
#[must_use]
pub fn validate_scenario_spec(spec: &ScenarioSpec) -> ScenarioValidationReport {
    let errors: Vec<_> = spec
        .validation_errors()
        .map(|error| error.to_string())
        .collect();
    ScenarioValidationReport {
        valid: errors.is_empty(),
        errors,
        warnings: Vec::new(),
        requires_instruments: spec.requires_instruments(),
        mutates_instruments: spec.mutates_instruments(),
        operation_count: spec.operations.len(),
    }
}

impl ScenarioSpec {
    /// Parse a scenario specification from JSON and validate it.
    ///
    /// The single ingest path for host bindings: strict serde decoding
    /// (unknown fields are rejected) followed by [`ScenarioSpec::validate`].
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical `ScenarioSpec` JSON document.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`](crate::Error::Validation) prefixed with
    /// `Failed to parse ScenarioSpec JSON:` when `json` does not decode, or
    /// the error [`ScenarioSpec::validate`] returns for an inconsistent spec.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_scenarios::ScenarioSpec;
    ///
    /// let spec = ScenarioSpec::from_json(r#"{"id":"base","operations":[]}"#)?;
    /// assert_eq!(spec.id, "base");
    /// # Ok::<(), finstack_quant_scenarios::Error>(())
    /// ```
    pub fn from_json(json: &str) -> crate::error::Result<Self> {
        let spec: Self = serde_json::from_str(json).map_err(|error| {
            crate::error::Error::Validation(format!("Failed to parse ScenarioSpec JSON: {error}"))
        })?;
        spec.validate()?;
        Ok(spec)
    }

    /// Validate the scenario specification for consistency.
    ///
    /// Checks for:
    /// - Non-empty ID
    /// - Valid operations (recursively)
    ///
    /// # Returns
    ///
    /// `Ok(())` when the scenario has a non-empty identifier, contains at most
    /// one [`OperationSpec::TimeRollForward`], and every contained operation
    /// passes [`OperationSpec::validate`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Validation`] if the scenario ID is empty,
    /// more than one time-roll operation is present, or any contained operation
    /// is invalid.
    pub fn validate(&self) -> crate::error::Result<()> {
        self.validation_errors().next().map_or(Ok(()), Err)
    }

    fn validation_errors(&self) -> impl Iterator<Item = crate::error::Error> + '_ {
        use crate::error::Error;

        let id_error = self
            .id
            .trim()
            .is_empty()
            .then(|| Error::Validation("Scenario ID cannot be empty".into()));
        let time_roll_count = self
            .operations
            .iter()
            .filter(|op| matches!(op, OperationSpec::TimeRollForward { .. }))
            .count();
        let roll_error = (time_roll_count > 1).then(|| {
            Error::Validation(format!(
                "Scenario contains {time_roll_count} TimeRollForward operations; at most one is allowed"
            ))
        });
        let operation_errors = self.operations.iter().enumerate().filter_map(|(i, op)| {
            op.validate()
                .err()
                .map(|error| Error::Validation(format!("Operation {i}: {error}")))
        });
        id_error
            .into_iter()
            .chain(roll_error)
            .chain(operation_errors)
    }
}

impl OperationSpec {
    /// Validate the operation specification.
    ///
    /// Checks for:
    /// - Non-NaN numeric values
    /// - Non-empty identifiers
    /// - Logical consistency (e.g. valid currency pairs)
    ///
    /// # Returns
    ///
    /// `Ok(())` when the operation's identifiers, numeric inputs, and variant-
    /// specific invariants are valid.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Validation`] if any identifier is empty,
    /// any numeric field is non-finite, or the variant violates its own
    /// structural rules. FX shocks reject `pct <= -100` because driving a spot
    /// rate to zero would propagate NaNs through triangulation. Equity and
    /// instrument price shocks reject `pct < -100`; an exact -100% wipeout is
    /// allowed, but a negative post-shock price is not.
    pub fn validate(&self) -> crate::error::Result<()> {
        match self {
            OperationSpec::MarketFxPct { base, quote, pct } => {
                if base == quote {
                    return Err(crate::error::Error::Validation(
                        "Base and quote currencies must be different".into(),
                    ));
                }
                check_finite(*pct, "pct")?;
                // FX cannot be driven to zero or negative; a -100% shock would
                // collapse the spot rate and propagate NaNs through triangulation
                // and valuation downstream.
                check_pct_floor(*pct, "pct")?;
            }
            OperationSpec::EquityPricePct { ids, pct } => {
                if ids.is_empty() {
                    return Err(crate::error::Error::Validation(
                        "Equity IDs cannot be empty".into(),
                    ));
                }
                check_finite(*pct, "pct")?;
                check_price_pct_floor(*pct, "pct")?;
            }
            OperationSpec::InstrumentPricePctByAttr { attrs, pct } => {
                if attrs.is_empty() {
                    return Err(crate::error::Error::Validation(
                        "InstrumentPricePctByAttr.attrs must not be empty; an empty map would \
                         silently apply the shock to every instrument. Use \
                         InstrumentPricePctByType to target by instrument type."
                            .into(),
                    ));
                }
                check_finite(*pct, "pct")?;
                check_price_pct_floor(*pct, "pct")?;
            }
            OperationSpec::CurveParallelBp {
                curve_id,
                discount_curve_id,
                bp,
                curve_kind,
            } => {
                check_rate_curve_kind(*curve_kind)?;
                check_id(curve_id.as_str(), "curve_id")?;
                if let Some(discount_curve_id) = discount_curve_id {
                    check_id(discount_curve_id.as_str(), "discount_curve_id")?;
                }
                check_finite(*bp, "bp")?;
            }
            OperationSpec::CurveNodeBp {
                curve_id,
                discount_curve_id,
                nodes,
                curve_kind,
                ..
            } => {
                check_rate_curve_kind(*curve_kind)?;
                check_id(curve_id.as_str(), "curve_id")?;
                if let Some(discount_curve_id) = discount_curve_id {
                    check_id(discount_curve_id.as_str(), "discount_curve_id")?;
                }
                check_nodes(nodes)?;
            }
            OperationSpec::CommodityCurveParallelPct { curve_id, pct } => {
                check_id(curve_id.as_str(), "curve_id")?;
                check_finite(*pct, "pct")?;
            }
            OperationSpec::VolIndexParallelPts { curve_id, points } => {
                check_id(curve_id.as_str(), "curve_id")?;
                check_finite(*points, "points")?;
            }
            OperationSpec::VolIndexNodePts {
                curve_id, nodes, ..
            }
            | OperationSpec::CommodityCurveNodePct {
                curve_id, nodes, ..
            } => {
                check_id(curve_id.as_str(), "curve_id")?;
                check_nodes(nodes)?;
            }
            OperationSpec::BaseCorrParallelPts { surface_id, points }
            | OperationSpec::BaseCorrBucketPts {
                surface_id, points, ..
            } => {
                check_id(surface_id.as_str(), "surface_id")?;
                check_corr_delta(*points, "points")?;
            }
            OperationSpec::VolSurfaceParallelPct {
                vol_surface_id,
                pct,
                ..
            }
            | OperationSpec::VolSurfaceBucketPct {
                vol_surface_id,
                pct,
                ..
            } => {
                check_id(vol_surface_id.as_str(), "vol_surface_id")?;
                check_finite(*pct, "pct")?;
                check_price_pct_floor(*pct, "pct")?;
            }
            OperationSpec::StmtForecastPercent { node_id, pct } => {
                check_id(node_id.as_str(), "node_id")?;
                check_finite(*pct, "pct")?;
            }
            OperationSpec::StmtForecastAssign { node_id, value } => {
                check_id(node_id.as_str(), "node_id")?;
                check_finite(*value, "value")?;
            }
            OperationSpec::RateBinding { binding } => {
                binding.validate()?;
            }
            OperationSpec::InstrumentSpreadBpByAttr { attrs, bp } => {
                if attrs.is_empty() {
                    return Err(crate::error::Error::Validation(
                        "InstrumentSpreadBpByAttr.attrs must not be empty; an empty map would \
                         silently apply the shock to every instrument. Use \
                         InstrumentSpreadBpByType to target by instrument type."
                            .into(),
                    ));
                }
                check_finite(*bp, "bp")?;
            }
            OperationSpec::InstrumentPricePctByType { pct, .. } => {
                check_finite(*pct, "pct")?;
                check_price_pct_floor(*pct, "pct")?;
            }
            OperationSpec::InstrumentSpreadBpByType { bp, .. } => {
                check_finite(*bp, "bp")?;
            }
            OperationSpec::AssetCorrelationPts { delta_pts }
            | OperationSpec::PrepayDefaultCorrelationPts { delta_pts } => {
                check_corr_delta(*delta_pts, "delta_pts")?;
            }
            OperationSpec::HierarchyCurveParallelBp {
                target,
                bp,
                discount_curve_id,
                curve_kind,
            } => {
                check_rate_curve_kind(*curve_kind)?;
                target
                    .validate()
                    .map_err(|err| crate::error::Error::Validation(err.to_string()))?;
                check_finite(*bp, "bp")?;
                if let Some(dcid) = discount_curve_id {
                    check_id(dcid.as_str(), "discount_curve_id")?;
                }
            }
            OperationSpec::HierarchyCommodityCurveParallelPct { target, pct } => {
                target
                    .validate()
                    .map_err(|err| crate::error::Error::Validation(err.to_string()))?;
                check_finite(*pct, "pct")?;
            }
            OperationSpec::HierarchyVolSurfaceParallelPct { target, pct, .. }
            | OperationSpec::HierarchyEquityPricePct { target, pct } => {
                target
                    .validate()
                    .map_err(|err| crate::error::Error::Validation(err.to_string()))?;
                check_finite(*pct, "pct")?;
                check_price_pct_floor(*pct, "pct")?;
            }
            OperationSpec::HierarchyBaseCorrParallelPts { target, points } => {
                target
                    .validate()
                    .map_err(|err| crate::error::Error::Validation(err.to_string()))?;
                check_corr_delta(*points, "points")?;
            }
            OperationSpec::TimeRollForward { period, .. } => {
                if period.trim().is_empty() {
                    return Err(crate::error::Error::Validation(
                        "Time roll period cannot be empty".into(),
                    ));
                }
                finstack_quant_core::dates::Tenor::parse(period.trim()).map_err(|error| {
                    crate::error::Error::InvalidPeriod(format!(
                        "Time roll period {period:?} is not a valid tenor: {error}"
                    ))
                })?;
            }
        }
        Ok(())
    }
}

fn check_rate_curve_kind(kind: CurveKind) -> crate::error::Result<()> {
    if kind == CurveKind::Commodity {
        return Err(crate::error::Error::Validation(
            "Commodity price curves use commodity_curve_parallel_pct, commodity_curve_node_pct or hierarchy_commodity_curve_parallel_pct".into(),
        ));
    }
    Ok(())
}

fn check_finite(val: f64, name: &str) -> crate::error::Result<()> {
    if !val.is_finite() {
        Err(crate::error::Error::Validation(format!(
            "Value '{name}' must be finite"
        )))
    } else {
        Ok(())
    }
}

fn check_id(id: &str, name: &str) -> crate::error::Result<()> {
    if id.trim().is_empty() {
        Err(crate::error::Error::Validation(format!(
            "Identifier '{name}' cannot be empty"
        )))
    } else {
        Ok(())
    }
}

fn check_pct_floor(val: f64, name: &str) -> crate::error::Result<()> {
    if val <= -100.0 {
        Err(crate::error::Error::Validation(format!(
            "Percentage '{name}' must be greater than -100% (got {val:.1}%)"
        )))
    } else {
        Ok(())
    }
}

fn check_price_pct_floor(val: f64, name: &str) -> crate::error::Result<()> {
    if val < -100.0 {
        Err(crate::error::Error::Validation(format!(
            "Price percentage '{name}' must be at least -100% (got {val:.1}%)"
        )))
    } else {
        Ok(())
    }
}

fn check_nodes(nodes: &[(String, f64)]) -> crate::error::Result<()> {
    if nodes.is_empty() {
        return Err(crate::error::Error::Validation(
            "Curve nodes cannot be empty".into(),
        ));
    }
    for (tenor, bp) in nodes {
        if tenor.trim().is_empty() {
            return Err(crate::error::Error::Validation(
                "Curve node tenor cannot be empty".into(),
            ));
        }
        check_finite(*bp, "bp")?;
    }
    Ok(())
}

/// Correlation shocks are quoted as additive points (1.0 = +100 pts). A Δρ
/// outside `[-2, 2]` cannot produce a valid correlation regardless of base
/// level (since ρ ∈ [-1, 1] implies |Δρ| ≤ 2). Reject such inputs at
/// validation time to surface obvious unit-confusion (e.g. 25 meaning
/// "0.25") before the adapter silently clamps the outcome.
fn check_corr_delta(val: f64, name: &str) -> crate::error::Result<()> {
    check_finite(val, name)?;
    if !(-2.0..=2.0).contains(&val) {
        return Err(crate::error::Error::Validation(format!(
            "Correlation delta '{name}' must lie in [-2, 2] points (got {val:.4}); \
             inputs are additive correlation points where 1.0 = +100 pts"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    /// One value of every [`OperationSpec`] variant.
    fn one_of_each() -> Vec<OperationSpec> {
        let target = HierarchyTarget {
            path: vec!["Rates".into()],
            tag_filter: None,
        };
        let attrs: IndexMap<String, String> = [("sector".to_string(), "energy".to_string())]
            .into_iter()
            .collect();
        let nodes = vec![("5Y".to_string(), 10.0)];
        vec![
            OperationSpec::MarketFxPct {
                base: Currency::EUR,
                quote: Currency::USD,
                pct: 1.0,
            },
            OperationSpec::EquityPricePct {
                ids: vec!["SPX".into()],
                pct: 1.0,
            },
            OperationSpec::InstrumentPricePctByAttr {
                attrs: attrs.clone(),
                pct: 1.0,
            },
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD-OIS".into(),
                discount_curve_id: None,
                bp: 1.0,
            },
            OperationSpec::CurveNodeBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD-OIS".into(),
                discount_curve_id: None,
                nodes: nodes.clone(),
                match_mode: TenorMatchMode::Exact,
            },
            OperationSpec::VolIndexParallelPts {
                curve_id: "VIX".into(),
                points: 1.0,
            },
            OperationSpec::VolIndexNodePts {
                curve_id: "VIX".into(),
                nodes,
                match_mode: TenorMatchMode::Exact,
            },
            OperationSpec::BaseCorrParallelPts {
                surface_id: "CDX".into(),
                points: 0.01,
            },
            OperationSpec::BaseCorrBucketPts {
                surface_id: "CDX".into(),
                detachment_bp: None,
                points: 0.01,
            },
            OperationSpec::VolSurfaceParallelPct {
                vol_surface_id: "SPX".into(),
                pct: 1.0,
            },
            OperationSpec::VolSurfaceBucketPct {
                vol_surface_id: "SPX".into(),
                tenors: None,
                strikes: None,
                pct: 1.0,
            },
            OperationSpec::StmtForecastPercent {
                node_id: "Revenue".into(),
                pct: 1.0,
            },
            OperationSpec::StmtForecastAssign {
                node_id: "Revenue".into(),
                value: 1.0,
            },
            OperationSpec::RateBinding {
                binding: RateBindingSpec {
                    node_id: "Rate".into(),
                    curve_id: "USD-OIS".into(),
                    tenor: "1Y".into(),
                    compounding: Compounding::Periodic(std::num::NonZeroU32::MIN),
                    day_count: None,
                },
            },
            OperationSpec::InstrumentSpreadBpByAttr { attrs, bp: 1.0 },
            OperationSpec::InstrumentPricePctByType {
                instrument_types: vec![InstrumentType::Bond],
                pct: 1.0,
            },
            OperationSpec::InstrumentSpreadBpByType {
                instrument_types: vec![InstrumentType::Bond],
                bp: 1.0,
            },
            OperationSpec::AssetCorrelationPts { delta_pts: 0.01 },
            OperationSpec::PrepayDefaultCorrelationPts { delta_pts: 0.01 },
            OperationSpec::HierarchyCurveParallelBp {
                curve_kind: CurveKind::Discount,
                target: target.clone(),
                bp: 1.0,
                discount_curve_id: None,
            },
            OperationSpec::HierarchyVolSurfaceParallelPct {
                target: target.clone(),
                pct: 1.0,
            },
            OperationSpec::HierarchyEquityPricePct {
                target: target.clone(),
                pct: 1.0,
            },
            OperationSpec::HierarchyBaseCorrParallelPts {
                target,
                points: 0.01,
            },
            OperationSpec::time_roll_forward("1M", None, None),
        ]
    }

    /// `kind()` is the serde `kind` tag for every variant, so hosts and
    /// diagnostics can name an operation without serializing it.
    #[test]
    fn kind_is_the_serde_tag_for_every_variant() {
        let operations = one_of_each();
        assert_eq!(operations.len(), 24);
        for operation in operations {
            let wire = serde_json::to_value(&operation).expect("operation serializes");
            assert_eq!(Some(operation.kind()), wire["kind"].as_str(), "{wire}");
        }
    }
}
