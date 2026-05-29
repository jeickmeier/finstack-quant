//! Serializable scenario specifications (serde-stable).
//!
//! This module defines the data contract that callers use to describe market,
//! instrument, statement, and time-roll shocks. Types here are intentionally
//! lightweight and focus on configuration rather than behaviour so they can be
//! shared across binaries, configuration files, and JSON/YAML APIs.
//!
//! Start with [`ScenarioSpec`] and [`OperationSpec`]. Use [`RateBindingSpec`]
//! when statement nodes should follow curve-driven rates, and use
//! [`HierarchyTarget`] together with [`ResolutionMode`](finstack_core::market_data::hierarchy::ResolutionMode)
//! for hierarchy-expanded shocks.
//!
//! # Conventions
//!
//! - Percent shocks use percentage-point inputs (`5.0 = +5%`).
//! - Curve and spread shocks quoted in `bp` use additive basis points (1 bp = 1e-4).
//! - Time-roll operations distinguish business-day-aware, calendar-day, and
//!   approximate day-count semantics via [`TimeRollMode`].
//! - Hierarchy-targeted operations expand to direct market-data identifiers at
//!   execution time; [`ScenarioSpec::resolution_mode`] determines whether
//!   overlapping hierarchy matches accumulate or collapse to the most specific
//!   match.
//! - Volatility-index curves use a separate variant
//!   ([`OperationSpec::VolIndexParallelPts`] / [`OperationSpec::VolIndexNodePts`])
//!   so that "points" semantics never collide with "basis points" semantics on
//!   rate curves.

use finstack_core::market_data::hierarchy::ResolutionMode;
use finstack_core::types::CurveId;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Re-export [`HierarchyTarget`] for hierarchy-targeted operations.
pub use finstack_core::market_data::hierarchy::HierarchyTarget;

/// Re-export [`InstrumentType`] for
/// type-safe instrument shock operations.
pub use finstack_valuations::pricer::InstrumentType;

/// Re-export [`NodeId`] for statement node identification.
pub use finstack_statements::types::NodeId;

/// A complete scenario specification with metadata and ordered operations.
///
/// # Fields
/// - `id`: Stable identifier used for persistence and reporting.
/// - `name`: Optional display name for UI or logs.
/// - `description`: Optional text describing the intent of the scenario.
/// - `operations`: Ordered list of [`OperationSpec`] values to execute.
/// - `priority`: Used by [`ScenarioEngine::try_compose`](crate::engine::ScenarioEngine::try_compose)
///   to determine merge ordering (lower numbers run first).
/// - `resolution_mode`: Controls how hierarchy-targeted shocks at multiple tree
///   levels combine for a single curve. Defaults to [`ResolutionMode::MostSpecificWins`].
///
/// # Examples
///
/// ```rust
/// use finstack_scenarios::{ScenarioSpec, OperationSpec, CurveKind};
/// use finstack_core::market_data::hierarchy::ResolutionMode;
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Create a [`crate::templates::ScenarioSpecBuilder`] for constructing a scenario.
    ///
    /// This is the preferred entry point when the scenario identifier is known
    /// up front and the ordered operations will be assembled fluently.
    #[must_use]
    pub fn builder(id: impl Into<String>) -> crate::templates::ScenarioSpecBuilder {
        crate::templates::ScenarioSpecBuilder::new(id)
    }
}

/// Individual operation within a scenario.
///
/// Each variant represents a specific type of shock or adjustment that can be
/// applied to market data, instruments, statements, or the valuation horizon.
/// Units are encoded in the variant name and field docs:
/// - `Pct` fields use percentage points (`5.0 = +5%`)
/// - `Bp` fields use additive basis points (1 bp = 1e-4)
/// - `Pts` fields use absolute correlation or volatility points in decimal form
///
/// Hierarchy-targeted variants are resolved into direct identifiers during
/// [`crate::engine::ScenarioEngine::apply`] using the market hierarchy attached
/// to the execution context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperationSpec {
    /// FX rate percent shift.
    ///
    /// # Example
    /// ```rust
    /// use finstack_scenarios::OperationSpec;
    /// use finstack_core::currency::Currency;
    ///
    /// let op = OperationSpec::MarketFxPct {
    ///     base: Currency::USD,
    ///     quote: Currency::EUR,
    ///     pct: 5.0, // +5% means USD strengthens
    /// };
    /// ```
    MarketFxPct {
        /// Base currency in the FX pair.
        base: finstack_core::currency::Currency,
        /// Quote currency in the FX pair.
        quote: finstack_core::currency::Currency,
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
    /// use finstack_scenarios::OperationSpec;
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
    /// use finstack_scenarios::OperationSpec;
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

    /// Parallel shift to a curve (additive in basis points).
    ///
    /// For rate-style curves ([`CurveKind::Discount`], [`CurveKind::Forward`],
    /// [`CurveKind::ParCDS`], [`CurveKind::Inflation`], [`CurveKind::Commodity`]),
    /// `bp` uses the standard fixed-income convention where `1 bp = 0.0001` in
    /// fractional rate space.
    ///
    /// **Volatility index curves use a separate variant.** Use
    /// [`OperationSpec::VolIndexParallelPts`] for VIX/VSTOXX-style curves where
    /// shocks are quoted in absolute index points.
    ///
    /// # Example
    /// ```rust
    /// use finstack_scenarios::{OperationSpec, CurveKind};
    ///
    /// let op = OperationSpec::CurveParallelBp {
    ///     curve_kind: CurveKind::Discount,
    ///     curve_id: "USD_SOFR".into(),
    ///     discount_curve_id: None,
    ///     bp: 50.0, // +50bp parallel shift
    /// };
    /// ```
    CurveParallelBp {
        /// Type of curve (Discount, Forward, ParCDS, Inflation, Commodity).
        curve_kind: CurveKind,
        /// Curve identifier.
        curve_id: CurveId,
        /// Optional explicit discount curve identifier when an operation must pair
        /// `curve_id` with a discounting curve (some forward/hazard bumps need one).
        /// If `None`, adapters apply engine default resolution; set this when multiple
        /// curves could match or you need deterministic selection.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        discount_curve_id: Option<CurveId>,
        /// Basis point shift (additive).
        bp: f64,
    },

    /// Node-specific basis point shifts for curve shaping.
    ///
    /// For rate-style curves, `bp` values use the standard fixed-income
    /// convention where `1 bp = 0.0001` in fractional rate space. Use
    /// [`OperationSpec::VolIndexNodePts`] for vol-index curves.
    ///
    /// # Example
    /// ```rust
    /// use finstack_scenarios::{OperationSpec, CurveKind, TenorMatchMode};
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
        /// Type of curve (Discount, Forward, ParCDS, Inflation, Commodity).
        curve_kind: CurveKind,
        /// Curve identifier.
        curve_id: CurveId,
        /// Optional explicit discount curve identifier when an operation must pair
        /// `curve_id` with a discounting curve (some forward/hazard bumps need one).
        /// If `None`, adapters apply engine default resolution; set this when multiple
        /// curves could match or you need deterministic selection.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        discount_curve_id: Option<CurveId>,
        /// Vector of (tenor, basis_point_shift) pairs.
        nodes: Vec<(String, f64)>,
        /// How to handle tenors not in the curve.
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
    /// use finstack_scenarios::OperationSpec;
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
    /// use finstack_scenarios::OperationSpec;
    ///
    /// let op = OperationSpec::BaseCorrParallelPts {
    ///     surface_id: "CDX_IG".into(),
    ///     points: 0.05, // +5 percentage points
    /// };
    /// ```
    BaseCorrParallelPts {
        /// Surface identifier.
        surface_id: CurveId,
        /// Absolute shift in correlation points.
        points: f64,
    },

    /// Bucket-specific base correlation shifts by detachment.
    ///
    /// The current base-correlation representation is one-dimensional by
    /// detachment point. Maturity filters are accepted in the wire format for
    /// forward compatibility, but non-empty maturity filters are rejected during
    /// application until term-structured base-correlation surfaces are supported.
    ///
    /// # Example
    /// ```rust
    /// use finstack_scenarios::OperationSpec;
    ///
    /// let op = OperationSpec::BaseCorrBucketPts {
    ///     surface_id: "CDX_IG".into(),
    ///     detachment_bps: Some(vec![300, 700]), // 3% and 7% detachment
    ///     maturities: None, // required today: base correlation is detachment-only
    ///     points: 0.03,
    /// };
    /// ```
    BaseCorrBucketPts {
        /// Surface identifier.
        surface_id: CurveId,
        /// Optional detachment points in basis points (e.g., 300 for 3%).
        detachment_bps: Option<Vec<i32>>,
        /// Reserved maturity filters for future term-structured base correlation.
        ///
        /// Use `None` or an empty vector today. Non-empty maturity filters are
        /// rejected because `BaseCorrelationCurve` has no maturity dimension.
        maturities: Option<Vec<String>>,
        /// Absolute shift in correlation points.
        points: f64,
    },

    /// Parallel percent shift to volatility surface.
    ///
    /// `pct` may be `<= -100`: vol-to-zero is sometimes used in degenerate
    /// stress (post-shock arbitrage detection emits a warning).
    ///
    /// # Example
    /// ```rust
    /// use finstack_scenarios::{OperationSpec, VolSurfaceKind};
    ///
    /// let op = OperationSpec::VolSurfaceParallelPct {
    ///     surface_kind: VolSurfaceKind::Equity,
    ///     surface_id: "SPX".into(),
    ///     pct: 10.0, // +10% volatility increase
    /// };
    /// ```
    VolSurfaceParallelPct {
        /// Type of volatility surface (Equity, FX, IR, etc.).
        surface_kind: VolSurfaceKind,
        /// Surface identifier.
        surface_id: CurveId,
        /// Percentage change in volatility.
        pct: f64,
    },

    /// Bucketed volatility surface shock.
    ///
    /// # Example
    /// ```rust
    /// use finstack_scenarios::{OperationSpec, VolSurfaceKind};
    ///
    /// let op = OperationSpec::VolSurfaceBucketPct {
    ///     surface_kind: VolSurfaceKind::Equity,
    ///     surface_id: "SPX".into(),
    ///     tenors: Some(vec!["1M".into(), "3M".into()]),
    ///     strikes: Some(vec![90.0, 95.0, 100.0]),
    ///     pct: 15.0,
    /// };
    /// ```
    VolSurfaceBucketPct {
        /// Type of volatility surface (Equity, FX, IR, etc.).
        surface_kind: VolSurfaceKind,
        /// Surface identifier.
        surface_id: CurveId,
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
    /// use finstack_scenarios::OperationSpec;
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
    /// use finstack_scenarios::OperationSpec;
    ///
    /// let op = OperationSpec::StmtForecastAssign {
    ///     node_id: "Revenue".into(),
    ///     value: 1_000_000.0,
    /// };
    /// ```
    StmtForecastAssign {
        /// Statement node identifier.
        node_id: NodeId,
        /// Absolute value to assign.
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
    /// use finstack_scenarios::{OperationSpec, RateBindingSpec};
    /// use finstack_scenarios::spec::Compounding;
    ///
    /// let op = OperationSpec::RateBinding {
    ///     binding: RateBindingSpec {
    ///         node_id: "InterestExpense".into(),
    ///         curve_id: "USD-OIS".into(),
    ///         tenor: "1Y".into(),
    ///         compounding: Compounding::Annual,
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
    /// use finstack_scenarios::OperationSpec;
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
    /// use finstack_scenarios::{OperationSpec, InstrumentType};
    ///
    /// let op = OperationSpec::InstrumentPricePctByType {
    ///     instrument_types: vec![InstrumentType::Bond, InstrumentType::Loan],
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
    /// use finstack_scenarios::{OperationSpec, InstrumentType};
    ///
    /// let op = OperationSpec::InstrumentSpreadBpByType {
    ///     instrument_types: vec![InstrumentType::CDS],
    ///     bp: 100.0,
    /// };
    /// ```
    InstrumentSpreadBpByType {
        /// Instrument types to shock.
        instrument_types: Vec<InstrumentType>,
        /// Basis point shift to apply.
        bp: f64,
    },

    // ========================================================================
    // Structured Credit Correlation Operations
    // ========================================================================
    /// Shock asset correlation for structured credit instruments.
    ///
    /// Applies a parallel shift to asset correlation parameters. For flat
    /// correlation structures, this bumps the single asset correlation. For
    /// sectored structures, it bumps both intra-sector and inter-sector
    /// correlations (inter-sector at half the rate).
    ///
    /// # Arguments
    /// - `delta_pts`: Additive shock in correlation points (e.g., 0.05 for +5%)
    ///
    /// # Clamping
    /// Asset correlation is clamped to [0, 0.99] after the shock.
    ///
    /// # Example
    /// ```rust
    /// use finstack_scenarios::OperationSpec;
    ///
    /// let op = OperationSpec::AssetCorrelationPts {
    ///     delta_pts: 0.05, // +5 percentage points
    /// };
    /// ```
    AssetCorrelationPts {
        /// Additive shock in correlation points.
        delta_pts: f64,
    },

    /// Shock prepay-default correlation for structured credit instruments.
    ///
    /// Applies a parallel shift to the correlation between prepayment and
    /// default rates. This is typically negative (higher prepayment = lower
    /// defaults due to selection effects).
    ///
    /// # Arguments
    /// - `delta_pts`: Additive shock in correlation points
    ///
    /// # Clamping
    /// Prepay-default correlation is clamped to [-0.99, 0.99].
    ///
    /// # Example
    /// ```rust
    /// use finstack_scenarios::OperationSpec;
    ///
    /// let op = OperationSpec::PrepayDefaultCorrelationPts {
    ///     delta_pts: 0.10, // +10 percentage points (less negative)
    /// };
    /// ```
    PrepayDefaultCorrelationPts {
        /// Additive shock in correlation points.
        delta_pts: f64,
    },

    // ========================================================================
    // Hierarchy-targeted operations (expanded to direct ops at execution time)
    // ========================================================================
    /// Hierarchy-targeted parallel curve shift.
    ///
    /// During [`crate::engine::ScenarioEngine::apply`], the engine resolves
    /// `target` against the market-data hierarchy and expands this into one
    /// [`OperationSpec::CurveParallelBp`] per matched curve. When multiple
    /// hierarchy operations reach the same curve, [`ScenarioSpec::resolution_mode`]
    /// determines whether they accumulate or only the most specific match survives.
    HierarchyCurveParallelBp {
        /// Type of curve (Discount, Forward, ParCDS, Inflation, Commodity).
        curve_kind: CurveKind,
        /// Hierarchy target to resolve to curves.
        target: HierarchyTarget,
        /// Basis point shift (additive).
        bp: f64,
        /// Optional discount curve for hazard-curve recalibration.
        ///
        /// Propagated to each expanded [`OperationSpec::CurveParallelBp`].
        /// If `None`, the adapter applies heuristic resolution.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        discount_curve_id: Option<CurveId>,
    },

    /// Hierarchy-targeted vol-surface parallel shift.
    ///
    /// Resolution follows the same hierarchy-expansion rules as
    /// [`OperationSpec::HierarchyCurveParallelBp`], but targets volatility
    /// surfaces of the requested [`VolSurfaceKind`].
    HierarchyVolSurfaceParallelPct {
        /// Type of volatility surface.
        surface_kind: VolSurfaceKind,
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
        /// Absolute shift in correlation points.
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
    /// use finstack_scenarios::{OperationSpec, TimeRollMode};
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

fn default_true() -> bool {
    true
}

/// Identifies which family of curve an operation targets.
///
/// These variants map to the market data collections exposed by
/// `finstack_core::market_data::context::MarketContext`.
/// They also determine which quoting and interpolation conventions apply when
/// downstream helpers extract rates or apply node shocks.
///
/// # Examples
/// ```rust
/// use finstack_scenarios::CurveKind;
///
/// let kind = CurveKind::Discount;
/// assert_eq!(format!("{:?}", kind), "Discount");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// Commodity forward curve.
    Commodity,
}

/// Labels which category of volatility surface an operation represents.
///
/// The current market context stores volatility surfaces in a single collection,
/// so scenario adapters look up surfaces by `surface_id`; `VolSurfaceKind` is
/// retained as explicit metadata for serde contracts, template discovery, and
/// future validation.
///
/// # Examples
/// ```rust
/// use finstack_scenarios::VolSurfaceKind;
///
/// assert!(matches!(VolSurfaceKind::Swaption, VolSurfaceKind::Swaption));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolSurfaceKind {
    /// Equity volatility surface.
    Equity,
    /// Credit volatility surface.
    Credit,
    /// Swaption volatility surface.
    Swaption,
}

/// Strategy for aligning requested tenor bumps with curve pillars.
///
/// [`TenorMatchMode::Exact`] requires the requested tenor to coincide with an
/// existing pillar. [`TenorMatchMode::Interpolate`] instead distributes the
/// bump across adjacent knots using the interpolation policy implemented by the
/// adapter.
///
/// # Examples
/// ```rust
/// use finstack_scenarios::TenorMatchMode;
///
/// let mode = TenorMatchMode::Interpolate;
/// assert_eq!(format!("{:?}", mode), "Interpolate");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
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
/// approximations aligned with [`crate::utils::parse_period_to_days`].
///
/// # Non-additivity of `Approximate`
///
/// [`TimeRollMode::Approximate`] is **not additive across composed rolls**.
/// Month periods use [`crate::utils::parse_period_to_days`], which rounds
/// `months * 365 / 12` independently for each operation. For example, `6M`
/// resolves to 183 days, so two `6M` approximate rolls produce 366 days while
/// one `12M` or `1Y` roll resolves to 365 days. For chained horizons or
/// scenario composition prefer
/// [`TimeRollMode::BusinessDays`] or [`TimeRollMode::CalendarDays`], which
/// both resolve the target date via [`finstack_core::dates::Tenor`] and are
/// additive modulo the chosen business-day convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TimeRollMode {
    /// Calendar-aware roll that adjusts to business days when a calendar is supplied.
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
/// count when converting `tenor` into a year fraction.
///
/// Supported `day_count` override strings include `act/360`,
/// `act/365f`, `act/act`, `30/360`, and `30e/360`.
///
/// # Examples
/// ```rust
/// use finstack_scenarios::spec::RateBindingSpec;
/// use finstack_scenarios::spec::Compounding;
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
#[serde(deny_unknown_fields)]
pub struct RateBindingSpec {
    /// Statement node ID to receive the rate.
    pub node_id: NodeId,

    /// Curve ID to extract rate from.
    pub curve_id: CurveId,

    /// Tenor for rate extraction (e.g., "3M", "1Y").
    pub tenor: String,

    /// Compounding convention for rate conversion.
    #[serde(default)]
    pub compounding: Compounding,

    /// Day count convention override. If None, uses curve's convention.
    #[serde(default)]
    pub day_count: Option<String>,
}

/// Compounding convention for rate conversions.
///
/// Used when extracting rates from curves to convert between
/// different quoting conventions (for example from continuous zeros to annual
/// or simple statement rates). The output remains a decimal annualized rate;
/// only the compounding basis changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Compounding {
    /// Simple interest (no compounding).
    Simple,
    /// Continuous compounding (default).
    #[default]
    Continuous,
    /// Annual compounding (1 per year).
    Annual,
    /// Semi-annual compounding (2 per year).
    SemiAnnual,
    /// Quarterly compounding (4 per year).
    Quarterly,
    /// Monthly compounding (12 per year).
    Monthly,
}

impl ScenarioSpec {
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
        if self.id.trim().is_empty() {
            return Err(crate::error::Error::Validation(
                "Scenario ID cannot be empty".into(),
            ));
        }
        let time_roll_count = self
            .operations
            .iter()
            .filter(|op| matches!(op, OperationSpec::TimeRollForward { .. }))
            .count();
        if time_roll_count > 1 {
            return Err(crate::error::Error::Validation(format!(
                "Scenario contains {time_roll_count} TimeRollForward operations; at most one is allowed"
            )));
        }
        for (i, op) in self.operations.iter().enumerate() {
            op.validate()
                .map_err(|e| crate::error::Error::Validation(format!("Operation {i}: {e}")))?;
        }
        Ok(())
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
    /// structural rules. FX shocks additionally reject `pct <= -100` because
    /// driving a spot rate to zero would propagate NaNs through triangulation;
    /// other percent shocks (equity, instrument price, vol) accept any finite
    /// `pct` value.
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
            }
            OperationSpec::CurveParallelBp {
                curve_id,
                discount_curve_id,
                bp,
                ..
            } => {
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
                ..
            } => {
                check_id(curve_id.as_str(), "curve_id")?;
                if let Some(discount_curve_id) = discount_curve_id {
                    check_id(discount_curve_id.as_str(), "discount_curve_id")?;
                }
                check_nodes(nodes)?;
            }
            OperationSpec::VolIndexParallelPts { curve_id, points } => {
                check_id(curve_id.as_str(), "curve_id")?;
                check_finite(*points, "points")?;
            }
            OperationSpec::VolIndexNodePts {
                curve_id, nodes, ..
            } => {
                check_id(curve_id.as_str(), "curve_id")?;
                check_nodes(nodes)?;
            }
            OperationSpec::BaseCorrParallelPts { surface_id, points } => {
                check_id(surface_id.as_str(), "surface_id")?;
                check_corr_delta(*points, "points")?;
            }
            OperationSpec::BaseCorrBucketPts {
                surface_id, points, ..
            } => {
                check_id(surface_id.as_str(), "surface_id")?;
                check_corr_delta(*points, "points")?;
            }
            OperationSpec::VolSurfaceParallelPct {
                surface_id, pct, ..
            } => {
                check_id(surface_id.as_str(), "surface_id")?;
                check_finite(*pct, "pct")?;
            }
            OperationSpec::VolSurfaceBucketPct {
                surface_id, pct, ..
            } => {
                check_id(surface_id.as_str(), "surface_id")?;
                check_finite(*pct, "pct")?;
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
                check_id(binding.node_id.as_str(), "node_id")?;
                check_id(binding.curve_id.as_str(), "curve_id")?;
                if binding.tenor.trim().is_empty() {
                    return Err(crate::error::Error::Validation(
                        "RateBinding tenor cannot be empty".into(),
                    ));
                }
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
            }
            OperationSpec::InstrumentSpreadBpByType { bp, .. } => {
                check_finite(*bp, "bp")?;
            }
            OperationSpec::AssetCorrelationPts { delta_pts } => {
                check_corr_delta(*delta_pts, "delta_pts")?;
            }
            OperationSpec::PrepayDefaultCorrelationPts { delta_pts } => {
                check_corr_delta(*delta_pts, "delta_pts")?;
            }
            OperationSpec::HierarchyCurveParallelBp {
                target,
                bp,
                discount_curve_id,
                ..
            } => {
                if target.path.is_empty() {
                    return Err(crate::error::Error::Validation(
                        "Hierarchy target path cannot be empty".into(),
                    ));
                }
                check_finite(*bp, "bp")?;
                if let Some(dcid) = discount_curve_id {
                    check_id(dcid.as_str(), "discount_curve_id")?;
                }
            }
            OperationSpec::HierarchyVolSurfaceParallelPct { target, pct, .. } => {
                if target.path.is_empty() {
                    return Err(crate::error::Error::Validation(
                        "Hierarchy target path cannot be empty".into(),
                    ));
                }
                check_finite(*pct, "pct")?;
            }
            OperationSpec::HierarchyEquityPricePct { target, pct } => {
                if target.path.is_empty() {
                    return Err(crate::error::Error::Validation(
                        "Hierarchy target path cannot be empty".into(),
                    ));
                }
                check_finite(*pct, "pct")?;
            }
            OperationSpec::HierarchyBaseCorrParallelPts { target, points } => {
                if target.path.is_empty() {
                    return Err(crate::error::Error::Validation(
                        "Hierarchy target path cannot be empty".into(),
                    ));
                }
                check_corr_delta(*points, "points")?;
            }
            OperationSpec::TimeRollForward { period, .. } => {
                if period.trim().is_empty() {
                    return Err(crate::error::Error::Validation(
                        "Time roll period cannot be empty".into(),
                    ));
                }
            }
        }
        Ok(())
    }
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
