//! Leg specifications, rebalance policy, weighting methods, resolved state,
//! and shared numeric helpers for composite instruments.

use crate::instruments::InstrumentJson;
use crate::metrics::MetricId;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::calendar::{adjust, calendar_by_id_strict};
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DateExt, HolidayCalendar, Tenor, TenorUnit,
};
use finstack_quant_core::expr::Expr;
use finstack_quant_core::market_data::context::{MarketContext, MarketContextState};
use finstack_quant_core::math::summation::neumaier_sum;
use finstack_quant_core::math::variance;
use finstack_quant_core::money::{fx::FxQuery, Money};
use finstack_quant_core::types::InstrumentId;
use finstack_quant_core::{Error, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Maximum nesting depth of composite-in-composite trees, counting the root.
pub const MAX_COMPOSITE_DEPTH: usize = 8;
/// Maximum number of leg nodes permitted across one composite tree.
pub const MAX_COMPOSITE_LEGS: usize = 64;
pub(super) const MIN_ABS_INPUT: f64 = 1.0e-12;

/// One self-contained leg in a composite specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CompositeLegSpec {
    /// Stable identifier of the embedded instrument; must equal `instrument.id()`.
    pub instrument_id: InstrumentId,
    /// Canonical typed definition of the underlying instrument.
    pub instrument: Box<InstrumentJson>,
    /// Signed leg score: the resolved unit quantity under
    /// [`WeightingMethod::FixedQuantity`] and a scale-free signed target score
    /// that every dynamic method normalizes.
    pub score: f64,
}

impl CompositeLegSpec {
    /// Construct a self-contained composite leg.
    ///
    /// `score` is the resolved quantity under [`WeightingMethod::FixedQuantity`]
    /// and the signed target score for every dynamic method. Validation requires
    /// a finite value with absolute magnitude greater than `1e-12`.
    ///
    /// # Arguments
    ///
    /// * `instrument_id` - Identifier that must equal `instrument.id()` after
    ///   the embedded definition is boxed.
    /// * `instrument` - Canonical typed underlying instrument; nested
    ///   composites are permitted within [`MAX_COMPOSITE_DEPTH`].
    /// * `score` - Signed fixed quantity or dynamic target score; must be
    ///   finite and non-zero.
    #[must_use]
    pub fn new(
        instrument_id: impl Into<InstrumentId>,
        instrument: InstrumentJson,
        score: f64,
    ) -> Self {
        Self {
            instrument_id: instrument_id.into(),
            instrument: Box::new(instrument),
            score,
        }
    }
}

/// Rule controlling when dynamic quantities may be explicitly recalculated.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RebalanceRule {
    /// Rebalance only when the caller explicitly invokes `rebalance`.
    Manual,
    /// Rebalance on the supplied adjusted dates.
    Dates {
        /// Strictly increasing dates on which the new state becomes eligible.
        #[serde(with = "finstack_quant_core::wire::dates")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
        )]
        dates: Vec<Date>,
    },
    /// Generate dates from a calendar-aware cadence.
    Calendar {
        /// Unadjusted schedule start.
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        start: Date,
        /// Optional final unadjusted schedule date.
        #[serde(default, with = "finstack_quant_core::wire::optional_date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "Option<finstack_quant_core::wire::DateWire>")
        )]
        end: Option<Date>,
        /// Rebalance cadence as a tenor (for example `1D`, `1W`, `1M`, `3M`).
        frequency: Tenor,
        /// Registered holiday-calendar identifier.
        calendar_id: String,
        /// Business-day adjustment applied to each generated date.
        business_day_convention: BusinessDayConvention,
    },
}

impl RebalanceRule {
    /// Validate ordering, calendar, and schedule bounds.
    ///
    /// Explicit dates must be strictly increasing. Calendar rules must have
    /// `end >= start` when an end date is supplied (`end == start` is a
    /// single rebalance date; `end = None` is an open-ended cadence), and the
    /// named holiday calendar must resolve.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate or unordered explicit dates, an end
    /// before the start, unknown calendar identifiers, or a bounded cadence
    /// whose dates cannot be generated or business-day adjusted.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Manual => Ok(()),
            Self::Dates { dates } => {
                if dates.windows(2).any(|pair| pair[0] >= pair[1]) {
                    return Err(Error::Validation(
                        "composite rebalance dates must be strictly increasing".to_string(),
                    ));
                }
                Ok(())
            }
            Self::Calendar {
                start,
                end,
                frequency,
                calendar_id,
                business_day_convention,
            } => {
                if end.is_some_and(|end| end < *start) {
                    return Err(Error::Validation(
                        "composite rebalance calendar end precedes start".to_string(),
                    ));
                }
                let calendar = calendar_by_id_strict(calendar_id)?;
                if let Some(end) = end {
                    cadence_dates(*start, *end, *frequency, calendar, *business_day_convention)?;
                }
                Ok(())
            }
        }
    }

    /// Return eligible rebalance dates up to a supplied horizon.
    ///
    /// [`Self::Manual`] yields an empty list. Explicit dates are filtered to
    /// `date <= horizon`. Calendar dates are the cadence `start + k * frequency`
    /// (`k = 0, 1, ...`, each anchored on `start`) whose unadjusted date is on
    /// or before `min(end, horizon)` (or `horizon` when `end` is omitted),
    /// business-day adjusted and kept when the adjusted date is on or before
    /// `horizon`. The cut-off is not itself a rebalance date unless it falls
    /// on the cadence.
    ///
    /// # Arguments
    ///
    /// * `horizon` - Latest date, inclusive, to include after business-day
    ///   adjustment; dates after this cutoff are omitted.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured calendar is unknown or a cadence
    /// date cannot be generated or adjusted.
    pub fn dates_through(&self, horizon: Date) -> Result<Vec<Date>> {
        match self {
            Self::Manual => Ok(Vec::new()),
            Self::Dates { dates } => Ok(dates
                .iter()
                .copied()
                .take_while(|date| *date <= horizon)
                .collect()),
            Self::Calendar {
                start,
                end,
                frequency,
                calendar_id,
                business_day_convention,
            } => {
                let calendar = calendar_by_id_strict(calendar_id)?;
                let cutoff = end.map_or(horizon, |end| end.min(horizon));
                let mut dates = cadence_dates(
                    *start,
                    cutoff,
                    *frequency,
                    calendar,
                    *business_day_convention,
                )?;
                dates.retain(|date| *date <= horizon);
                Ok(dates)
            }
        }
    }
}

/// Business-day-adjusted cadence `start + k * frequency` for every `k >= 0`
/// whose unadjusted date is on or before `cutoff`.
///
/// Each date is anchored on `start` (not stepped from the previous date), so
/// month-end starts do not drift. Adjacent dates that adjust onto the same
/// business day are reported once. An empty list is returned when `cutoff`
/// precedes `start`.
fn cadence_dates(
    start: Date,
    cutoff: Date,
    frequency: Tenor,
    calendar: &dyn HolidayCalendar,
    business_day_convention: BusinessDayConvention,
) -> Result<Vec<Date>> {
    let overflow = || Error::Validation("composite rebalance cadence overflows".to_string());
    let mut dates: Vec<Date> = Vec::new();
    for k in 0_i64.. {
        let steps = i64::from(frequency.count())
            .checked_mul(k)
            .ok_or_else(overflow)?;
        let raw = match frequency.unit() {
            TenorUnit::Days => start.add_days(steps)?,
            TenorUnit::Weeks => start.add_days(steps.checked_mul(7).ok_or_else(overflow)?)?,
            TenorUnit::Months => start.add_months(i32::try_from(steps).map_err(|_| overflow())?)?,
            TenorUnit::Years => start.add_months(
                i32::try_from(steps.checked_mul(12).ok_or_else(overflow)?)
                    .map_err(|_| overflow())?,
            )?,
        };
        if raw > cutoff {
            break;
        }
        let adjusted = adjust(raw, business_day_convention, calendar)?;
        if dates.last() != Some(&adjusted) {
            dates.push(adjusted);
        }
    }
    Ok(dates)
}

/// Policy used to resolve signed leg quantities at initialization or rebalance.
///
/// Each variant consumes the signed `score` on [`CompositeLegSpec`] as either
/// the quantity itself or a target score. Resolution happens only in
/// [`CompositeSpec::initialize`] / [`CompositeSpec::initialize_fixed`] or
/// [`CompositeInstrument::rebalance`].
///
/// [`CompositeSpec::initialize`]: crate::instruments::composite::CompositeSpec::initialize
/// [`CompositeSpec::initialize_fixed`]: crate::instruments::composite::CompositeSpec::initialize_fixed
/// [`CompositeInstrument::rebalance`]: crate::instruments::composite::CompositeInstrument::rebalance
///
/// # Formulas
///
/// Let `w_i` be the leg score, `G` a positive reporting-currency gross
/// notional, `N_i` the absolute unit notional, `m_i` the unit metric, `s_i`
/// the (optionally neutralized) score, and `σ_i` annualized unit-P&L
/// volatility. With an anchor quantity `q_a`:
///
/// ```text
/// FixedQuantity:     q_i = w_i
/// NotionalWeighted:  q_i = sign(w_i) · G · |w_i| / Σ|w| / N_i
/// MetricWeighted:    q_i = (s_i / s_a) · (q_a · m_a) / m_i
/// VolatilityWeighted:q_i = (w_i / w_a) · q_a · σ_a / σ_i
/// ```
///
/// Neutralization rescales positive scores to sum to `+1` and negative scores
/// to sum to `-1` before metric weighting. User-defined expressions replace
/// these closed forms and must return a finite non-zero quantity per leg.
///
/// # References
///
/// - DV01-neutral and duration-weighted curve trades:
///   `docs/REFERENCES.md#tuckman-serrat-fixed-income`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WeightingMethod {
    /// Use each leg's signed `score` directly as its resolved quantity.
    FixedQuantity,
    /// Allocate signed scores across a fixed gross reporting-currency notional.
    NotionalWeighted {
        /// Positive gross notional in the composite reporting currency.
        gross_notional: Money,
    },
    /// Target relative per-leg contributions to a valuation metric.
    MetricWeighted {
        /// Metric used as the weighting measure, such as `dv01` or `delta`.
        metric: MetricId,
        /// Leg whose resolved quantity fixes the overall scale.
        anchor_leg_id: InstrumentId,
        /// Non-zero signed quantity assigned to the anchor leg.
        anchor_quantity: f64,
        /// Whether positive and negative scores are normalized to `+1` and `-1`.
        neutralize: bool,
    },
    /// Inverse annualized unit-P&L volatility weighting.
    VolatilityWeighted {
        /// Leg whose resolved quantity fixes the overall scale.
        anchor_leg_id: InstrumentId,
        /// Non-zero signed quantity assigned to the anchor leg.
        anchor_quantity: f64,
        /// Maximum number of most-recent P&L observations used.
        lookback: usize,
        /// Minimum P&L observations required for every active leg.
        min_observations: usize,
        /// Positive periods-per-year factor used to annualize sample volatility.
        annualization_factor: f64,
    },
    /// Deterministic expression-defined quantities.
    UserDefined {
        /// Metrics populated into the expression context for every leg.
        required_metrics: Vec<MetricId>,
        /// One scalar expression per leg, keyed by instrument identifier.
        ///
        /// Available columns: `as_of_days`, `leg.{id}.score`,
        /// `leg.{id}.value`, `leg.{id}.fx_rate`, optional `leg.{id}.notional`,
        /// `leg.{id}.metric.{metric}` for each required metric, and
        /// `leg.{id}.volatility` when history has at least three observations
        /// (annualized with `sqrt(annualization_factor)`). `leg.{id}.notional`
        /// is the signed reporting-currency notional.
        quantity_expressions: IndexMap<String, Expr>,
        /// Positive periods-per-year factor used to annualize the
        /// `leg.{id}.volatility` column (for example `252.0` for daily history).
        annualization_factor: f64,
    },
}

impl WeightingMethod {
    /// Construct parallel-DV01-neutral weighting.
    ///
    /// Equivalent to [`Self::MetricWeighted`] with `metric = dv01` and
    /// `neutralize = true`. Positive and negative score groups are normalized
    /// separately so a steepener or butterfly can split wing risk.
    ///
    /// # Arguments
    ///
    /// * `anchor_leg_id` - Existing leg whose signed quantity fixes overall
    ///   scale; must be present on the specification.
    /// * `anchor_quantity` - Finite non-zero signed quantity assigned to the
    ///   anchor; `q_a · m_a` must have the same sign as the anchor score.
    #[must_use]
    pub fn dv01_neutral(anchor_leg_id: impl Into<InstrumentId>, anchor_quantity: f64) -> Self {
        Self::MetricWeighted {
            metric: MetricId::Dv01,
            anchor_leg_id: anchor_leg_id.into(),
            anchor_quantity,
            neutralize: true,
        }
    }

    /// Construct delta-neutral weighting.
    ///
    /// Equivalent to [`Self::MetricWeighted`] with `metric = delta` and
    /// `neutralize = true`.
    ///
    /// # Arguments
    ///
    /// * `anchor_leg_id` - Existing delta-bearing leg whose signed quantity
    ///   fixes overall scale.
    /// * `anchor_quantity` - Finite non-zero signed quantity assigned to the
    ///   anchor; `q_a · Δ_a` must have the same sign as the anchor score.
    #[must_use]
    pub fn delta_neutral(anchor_leg_id: impl Into<InstrumentId>, anchor_quantity: f64) -> Self {
        Self::MetricWeighted {
            metric: MetricId::Delta,
            anchor_leg_id: anchor_leg_id.into(),
            anchor_quantity,
            neutralize: true,
        }
    }

    /// Construct modified-duration weighting without neutrality normalization.
    ///
    /// Equivalent to [`Self::MetricWeighted`] with `metric = duration_mod` and
    /// `neutralize = false`. Scores are used as raw relative weights.
    ///
    /// # Arguments
    ///
    /// * `anchor_leg_id` - Existing duration-bearing leg whose signed quantity
    ///   fixes overall scale.
    /// * `anchor_quantity` - Finite non-zero signed quantity assigned to the
    ///   anchor; `q_a · D_a` must have the same sign as the anchor score.
    #[must_use]
    pub fn duration_weighted(anchor_leg_id: impl Into<InstrumentId>, anchor_quantity: f64) -> Self {
        Self::MetricWeighted {
            metric: MetricId::DurationMod,
            anchor_leg_id: anchor_leg_id.into(),
            anchor_quantity,
            neutralize: false,
        }
    }

    /// Construct inverse unit-P&L-volatility weighting.
    ///
    /// Unit P&L is value change plus signed cashflows on `(t_{k-1}, t_k]`,
    /// converted to the composite reporting currency. Sample volatility uses
    /// Bessel's correction (`n - 1`) and is annualized by
    /// `sqrt(annualization_factor)`. History supplied to `initialize` or
    /// `rebalance` must be strictly increasing and must end on the effective
    /// date.
    ///
    /// # Arguments
    ///
    /// * `anchor_leg_id` - Existing leg whose signed quantity fixes overall
    ///   scale.
    /// * `anchor_quantity` - Finite non-zero signed quantity assigned to the
    ///   anchor; sign must match the anchor score.
    /// * `lookback` - Maximum number of most-recent unit-P&L observations
    ///   retained; must be at least `min_observations`.
    /// * `min_observations` - Minimum finite P&L observations required for
    ///   every active leg; must satisfy `lookback >= min_observations >= 2`.
    /// * `annualization_factor` - Positive periods-per-year factor, such as
    ///   `252.0` for daily observations.
    #[must_use]
    pub fn volatility_weighted(
        anchor_leg_id: impl Into<InstrumentId>,
        anchor_quantity: f64,
        lookback: usize,
        min_observations: usize,
        annualization_factor: f64,
    ) -> Self {
        Self::VolatilityWeighted {
            anchor_leg_id: anchor_leg_id.into(),
            anchor_quantity,
            lookback,
            min_observations,
            annualization_factor,
        }
    }
}

/// Immutable resolved quantity for one top-level composite leg.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ResolvedCompositeLeg {
    /// Identifier of the corresponding leg specification.
    pub instrument_id: InstrumentId,
    /// Signed quantity held from the state's effective date until the next rebalance.
    pub quantity: f64,
}

/// Immutable holdings state used by pricing, risk, scenarios, and history.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CompositeState {
    /// Date from which the resolved quantities are effective.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub effective_date: Date,
    /// Resolved top-level quantities in specification order.
    pub resolved_legs: Vec<ResolvedCompositeLeg>,
    /// Finite scalar inputs retained for rebalance audit and reproducibility.
    pub weighting_inputs: IndexMap<String, f64>,
}

/// One dated, complete market snapshot used by dynamic weighting and history.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CompositeMarketObservation {
    /// Observation date; must be strictly increasing within a supplied history.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Complete materialized market state for the observation date.
    pub state: MarketContextState,
}

impl CompositeMarketObservation {
    /// Capture a market context as a dated immutable observation.
    ///
    /// The snapshot stores the complete [`MarketContextState`]. Dynamic
    /// weighting and history restore it rather than mutating the live market.
    ///
    /// # Arguments
    ///
    /// * `date` - Observation date; sequences must be strictly increasing and
    ///   volatility history must end on the rebalance date.
    /// * `market` - Complete market context whose curves, prices, and FX
    ///   matrix are materialized for this date.
    ///
    /// # Errors
    ///
    /// Returns an error when the FX provider cannot supply a coherent snapshot.
    pub fn new(date: Date, market: &MarketContext) -> Result<Self> {
        Ok(Self {
            date,
            state: MarketContextState::try_from(market)?,
        })
    }

    pub(crate) fn restore(&self) -> Result<MarketContext> {
        MarketContext::try_from(self.state.clone())
    }
}

pub(super) fn normalized_scores(legs: &[CompositeLegSpec], neutralize: bool) -> Result<Vec<f64>> {
    if !neutralize {
        return Ok(legs.iter().map(|leg| leg.score).collect());
    }
    let positive = legs
        .iter()
        .filter(|leg| leg.score > 0.0)
        .map(|leg| leg.score)
        .sum::<f64>();
    let negative = legs
        .iter()
        .filter(|leg| leg.score < 0.0)
        .map(|leg| leg.score.abs())
        .sum::<f64>();
    if !positive.is_finite()
        || !negative.is_finite()
        || positive <= MIN_ABS_INPUT
        || negative <= MIN_ABS_INPUT
    {
        return Err(Error::Validation(
            "neutral weighting requires non-zero positive and negative score totals".to_string(),
        ));
    }
    Ok(legs
        .iter()
        .map(|leg| {
            if leg.score > 0.0 {
                leg.score / positive
            } else {
                leg.score / negative
            }
        })
        .collect())
}

pub(super) fn leg_index(legs: &[CompositeLegSpec], id: &InstrumentId) -> Result<usize> {
    legs.iter()
        .position(|leg| leg.instrument_id == *id)
        .ok_or_else(|| Error::Validation(format!("composite anchor leg '{}' is not present", id)))
}

pub(super) fn convert_amount(
    market: &MarketContext,
    amount: f64,
    from: Currency,
    to: Currency,
    as_of: Date,
) -> Result<f64> {
    if from == to {
        return Ok(amount);
    }
    let rate = market.fx_required()?.rate(FxQuery::new(from, to, as_of))?;
    Ok(amount * rate.rate)
}

pub(crate) fn validate_history(
    history: &[CompositeMarketObservation],
    latest_allowed: Option<Date>,
) -> Result<()> {
    if history.windows(2).any(|pair| pair[0].date >= pair[1].date) {
        return Err(Error::Validation(
            "composite market observations must be strictly increasing".to_string(),
        ));
    }
    if latest_allowed.is_some_and(|latest| {
        history
            .last()
            .is_some_and(|observation| observation.date > latest)
    }) {
        return Err(Error::Validation(
            "composite history contains observations after the rebalance date".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn unit_pnl_series(
    instrument_json: &InstrumentJson,
    reporting_currency: Currency,
    history: &[CompositeMarketObservation],
) -> Result<Vec<f64>> {
    validate_history(history, None)?;
    if history.len() < 2 {
        return Ok(Vec::new());
    }
    let instrument = instrument_json.clone().into_boxed()?;
    let mut values = Vec::with_capacity(history.len());
    let mut markets = Vec::with_capacity(history.len());
    for observation in history {
        let market = observation.restore()?;
        let value = instrument.value(&market, observation.date)?;
        values.push(convert_amount(
            &market,
            value.amount(),
            value.currency(),
            reporting_currency,
            observation.date,
        )?);
        markets.push(market);
    }
    let mut pnl = Vec::with_capacity(history.len() - 1);
    for index in 1..history.len() {
        let start = history[index - 1].date;
        let end = history[index].date;
        let cashflows = instrument.dated_cashflows(&markets[index], start)?;
        let cashflow_total = cashflows
            .into_iter()
            .filter(|(date, _)| *date > start && *date <= end)
            .map(|(date, amount)| {
                convert_amount(
                    &markets[index],
                    amount.amount(),
                    amount.currency(),
                    reporting_currency,
                    date,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        pnl.push(values[index] - values[index - 1] + neumaier_sum(cashflow_total));
    }
    Ok(pnl)
}

pub(crate) fn cashflows_between(
    instrument_json: &InstrumentJson,
    quantity: f64,
    reporting_currency: Currency,
    market: &MarketContext,
    start: Date,
    end: Date,
) -> Result<f64> {
    let instrument = instrument_json.clone().into_boxed()?;
    let cashflows = instrument.dated_cashflows(market, start)?;
    let amounts = cashflows
        .into_iter()
        .filter(|(date, _)| *date > start && *date <= end)
        .map(|(date, amount)| {
            convert_amount(
                market,
                amount.amount(),
                amount.currency(),
                reporting_currency,
                date,
            )
            .map(|converted| converted * quantity)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(neumaier_sum(amounts))
}

/// Sample (n-1) standard deviation of unit P&L observations.
///
/// Rejects fewer than two observations and any non-finite value instead of
/// returning the zero or NaN that [`variance`] would produce for them.
pub(super) fn sample_std_dev(values: &[f64]) -> Result<f64> {
    if values.len() < 2 || values.iter().any(|value| !value.is_finite()) {
        return Err(Error::Validation(
            "unit-P&L volatility requires at least two finite observations".to_string(),
        ));
    }
    Ok(variance(values).sqrt())
}
