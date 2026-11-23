//! Curve kind, tenor-match, time-roll, and rate-binding specification types.

use super::validation::check_id;
#[cfg(doc)]
use crate::OperationSpec;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::math::Compounding;
use finstack_quant_core::types::CurveId;
use finstack_quant_statements::types::NodeId;
use serde::{Deserialize, Serialize};

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
