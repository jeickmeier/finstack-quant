//! Portfolio-level P&L attribution aggregation, reconciliation, and reporting.

use crate::Portfolio;
use crate::error::{Error, Result};
use crate::evaluation::try_map_positions;
use crate::evaluation::{EvaluationProfile, PortfolioEvaluationPlan, PositionExecution};
use crate::types::PositionId;
use crate::valuation::{
    PortfolioValuation, PortfolioValuationOptions, PositionValue, RequestedMetrics,
};
use finstack_quant_attribution::{
    AttributionMethod, AttributionRequest, PnlAttribution, PnlBucket, PnlBucketShare,
    attribute_pnl_metrics_based, default_attribution_metrics, translate_to_target_currency,
};
use finstack_quant_core::config::{FinstackConfig, RoundingContext, ZeroKind};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::composite::CompositeInstrument;
use finstack_quant_valuations::instruments::json_loader::instrument_json_of;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Portfolio-level P&L attribution result.
///
/// Aggregates P&L attribution across all positions with currency conversion
/// to portfolio base currency.
///
/// # FX Translation Effects
///
/// For positions denominated in currencies other than the portfolio's base currency,
/// the attribution includes FX translation effects. The `total_pnl` field represents
/// the **all-in P&L** including both:
///
/// 1. **Instrument-level P&L** converted to base currency at T₁ FX rates
/// 2. **FX translation P&L** from the revaluation of opening principal
///
/// The decomposition is:
///
/// ```text
/// total_pnl = sum(factor_pnl_at_T1_FX) + fx_translation_pnl + residual
/// ```
///
/// Where each factor bucket (carry, rates, credit, vol, etc.) is converted to
/// base currency using the T₁ FX rate. This means the implicit FX translation
/// of the P&L *flow* (i.e., `PnL_native × (FX_T1 - FX_T0)`) is absorbed into
/// each factor bucket rather than isolated in `fx_translation_pnl`.
///
/// `fx_translation_pnl` captures **only** the revaluation of the opening
/// principal:
///
/// ```text
/// fx_translation_pnl = Val_T0_native × (FX_T1 - FX_T0)
/// ```
///
/// This convention is consistent with systems that convert factor P&L at
/// closing rates and report principal revaluation separately.
///
/// # Note on by_position Attribution
///
/// The `by_position` map contains instrument-currency attribution before FX
/// translation effects are applied. To reconcile with `total_pnl`, apply the
/// FX rates and add the principal revaluation effect.
///
/// # Conventions
///
/// The portfolio-level aggregates are reported in portfolio base currency,
/// while `by_position` remains in each instrument's native currency so callers
/// can inspect raw instrument attribution before FX translation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PortfolioAttribution {
    /// Total portfolio P&L in base currency.
    ///
    /// This is the **all-in P&L** that includes:
    /// - All factor attributions converted to base currency at T₁ rates
    /// - FX translation effects from opening principal revaluation
    ///
    /// Note: This differs from a simple sum of factor attributions because
    /// cross-currency positions include FX translation P&L on the principal.
    pub total_pnl: Money,

    /// Carry P&L (theta + accruals) in base currency.
    pub carry: Money,

    /// Interest rate curves P&L in base currency.
    pub rates_curves_pnl: Money,

    /// Credit hazard curves P&L in base currency.
    pub credit_curves_pnl: Money,

    /// Inflation curves P&L in base currency.
    pub inflation_curves_pnl: Money,

    /// Base correlation curves P&L in base currency.
    pub correlations_pnl: Money,

    /// FX rate changes P&L in base currency.
    ///
    /// This captures FX exposure within instruments (e.g., cross-currency swaps),
    /// not the translation effect from converting instrument P&L to base currency.
    pub fx_pnl: Money,

    /// FX translation P&L from revaluing opening principal to base currency.
    ///
    /// For cross-currency positions, this captures the effect of FX rate changes
    /// on the T₀ position value:
    ///
    /// ```text
    /// fx_translation_pnl = Val_T0_native × (FX_T1 - FX_T0)
    /// ```
    ///
    /// Note: The implicit FX translation of each factor's P&L flow
    /// (converting native-currency factor P&L at T₁ FX rather than T₀ FX) is
    /// absorbed into the respective factor buckets (carry, rates, etc.) and is
    /// **not** included here.
    ///
    /// This is separate from `fx_pnl` which captures FX exposure within instruments.
    pub fx_translation_pnl: Money,

    /// Cross-factor interaction P&L in base currency.
    ///
    /// Aggregated from each position's native-currency `cross_factor_pnl`
    /// after conversion to portfolio base currency.
    pub cross_factor_pnl: Money,

    /// Implied volatility changes P&L in base currency.
    pub vol_pnl: Money,

    /// Model parameters P&L in base currency.
    pub model_params_pnl: Money,

    /// Market scalars P&L in base currency.
    pub market_scalars_pnl: Money,

    /// Residual P&L (unexplained) in base currency.
    pub residual: Money,

    /// Attribution by position in instrument-native currency.
    ///
    /// Note: These values are in each instrument's native currency and do not
    /// include FX translation effects. Use the portfolio-level aggregates for
    /// base-currency totals.
    pub by_position: IndexMap<PositionId, PnlAttribution>,

    /// True if any constituent position's attribution was flagged invalid
    /// (for example, a non-finite factor sensitivity — see
    /// [`PnlAttribution::result_invalid`]). When `true`, the portfolio
    /// aggregates and [`PortfolioAttribution::reconciliation_check`] are not
    /// trustworthy and must not be relied on for reporting.
    ///
    pub result_invalid: bool,
}

/// Report from reconciling an attribution's effect buckets against its total.
///
/// Returned by [`PortfolioAttribution::reconciliation_check`] (factor P&L
/// buckets plus FX translation against `total_pnl`) and by
/// [`crate::fi_attribution::FiAttributionResult::reconciliation_check`] (the
/// five Campisi effect totals against the active return).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct ReconciliationReport {
    /// Total minus the sum of its effect buckets: `total_pnl - (factor buckets
    /// + fx_translation_pnl)` for P&L attribution, `active_return - (allocation
    /// + carry + treasury + spread + selection)` for Campisi attribution.
    pub total_residual: f64,
    /// Whether the reconciliation passes within tolerance.
    pub is_reconciled: bool,
    /// Tolerance used for the check.
    pub tolerance: f64,
}

pub(super) struct PositionAttributionData {
    pub(super) position_id: PositionId,
    pub(super) pos_attr: PnlAttribution,
    pub(super) val_t0_native: Money,
}

impl PortfolioAttribution {
    /// The portfolio result for a base-currency aggregate of its positions.
    pub(super) fn from_aggregate(
        aggregate: PnlAttribution,
        by_position: IndexMap<PositionId, PnlAttribution>,
    ) -> Self {
        Self {
            total_pnl: aggregate.total_pnl,
            carry: aggregate.carry,
            rates_curves_pnl: aggregate.rates_curves_pnl,
            credit_curves_pnl: aggregate.credit_curves_pnl,
            inflation_curves_pnl: aggregate.inflation_curves_pnl,
            correlations_pnl: aggregate.correlations_pnl,
            fx_pnl: aggregate.fx_pnl,
            fx_translation_pnl: aggregate.fx_translation_pnl,
            cross_factor_pnl: aggregate.cross_factor_pnl,
            vol_pnl: aggregate.vol_pnl,
            model_params_pnl: aggregate.model_params_pnl,
            market_scalars_pnl: aggregate.market_scalars_pnl,
            residual: aggregate.residual,
            by_position,
            result_invalid: aggregate.result_invalid,
        }
    }
}

pub(super) fn attribute_composite_primitives(
    composite: &CompositeInstrument,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: &FinstackConfig,
    method: &AttributionMethod,
) -> Result<PnlAttribution> {
    let reporting_currency = composite.spec.reporting_currency;
    let mut aggregate = PnlAttribution::new(
        Money::from((0_i64, reporting_currency)),
        composite.id(),
        as_of_t0,
        as_of_t1,
        method.clone(),
    );
    aggregate.mark_to_market_pnl = Some(Money::from((0_i64, reporting_currency)));
    aggregate.pv_t0 = Some(Money::from((0_i64, reporting_currency)));
    aggregate.pv_t1 = Some(Money::from((0_i64, reporting_currency)));
    aggregate.residual = Money::from((0_i64, reporting_currency));

    for exposure in composite.flatten_primitives().map_err(Error::Core)? {
        let definition = exposure
            .instrument()
            .and_then(instrument_json_of)
            .ok_or_else(|| {
                Error::valuation(
                    composite.id(),
                    finstack_quant_core::ErrorKind::Computation,
                    format!(
                        "primitive path '{}' lost its embedded definition",
                        exposure.path.join("/")
                    ),
                )
            })?;
        let instrument: Arc<dyn finstack_quant_valuations::instruments::Instrument> =
            Arc::from(definition.clone().into_boxed().map_err(Error::Core)?);
        let value_t0 = instrument.value(market_t0, as_of_t0).map_err(Error::Core)?;
        let value_t1 = instrument.value(market_t1, as_of_t1).map_err(Error::Core)?;
        let mut primitive = finstack_quant_attribution::attribute_pnl(
            method,
            &AttributionRequest {
                strict_validation: false,
                prepared_endpoints: Some((value_t0, value_t1)),
                ..AttributionRequest::new(
                    &instrument,
                    market_t0,
                    market_t1,
                    as_of_t0,
                    as_of_t1,
                    config,
                )
            },
        )
        .map_err(Error::Core)?;
        primitive.scale(exposure.quantity)?;
        translate_to_target_currency(
            &mut primitive,
            value_t0.checked_mul_f64(exposure.quantity)?,
            reporting_currency,
            market_t0,
            market_t1,
            as_of_t0,
            as_of_t1,
        )?;
        // `accumulate` sums amounts only; the composite records the FX
        // policy its translated legs applied.
        if aggregate.meta.fx_policy.is_none() {
            aggregate.meta.fx_policy = primitive.meta.fx_policy.clone();
        }
        aggregate.accumulate(primitive)?;
    }

    aggregate.meta.notes.push(
        "Composite attribution is the reporting-currency sum of frozen primitive paths; detailed leaf reports remain available through primitive decomposition"
            .to_string(),
    );
    aggregate.compute_residual().map_err(Error::Core)?;
    Ok(aggregate)
}

/// Exact canonical evaluation profile required by an attribution method.
///
/// `MetricsBased` needs a cross-instrument menu of sensitivities, so each
/// position requests the part its own instrument type supports. Every other
/// method reprices directly and needs PV alone.
pub(crate) fn attribution_endpoint_profile(method: &AttributionMethod) -> EvaluationProfile {
    if matches!(method, AttributionMethod::MetricsBased) {
        EvaluationProfile::strict_metrics(&default_attribution_metrics())
    } else {
        EvaluationProfile::from_options(&PortfolioValuationOptions {
            strict_risk: false,
            metrics: RequestedMetrics::Only(Vec::new()),
        })
    }
}

/// Prepare the exact ordinary endpoint values consumed by repricing methods.
///
/// Both endpoints enter through the canonical portfolio executor once. The
/// attribution methods then perform only their financially distinct carry,
/// factor-restoration, and sensitivity repricings.
pub(super) fn prepare_endpoints(
    portfolio: &Portfolio,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: &FinstackConfig,
    method: &AttributionMethod,
) -> Result<(PortfolioValuation, PortfolioValuation)> {
    let profile = attribution_endpoint_profile(method);
    let mut plan = PortfolioEvaluationPlan::new(config);
    let portfolio_state = plan.register_portfolio(portfolio);
    let market_t0_state = plan.register_market(market_t0, as_of_t0);
    let market_t1_state = plan.register_market(market_t1, as_of_t1);
    let t0_job = plan.register_evaluation(
        market_t0_state,
        portfolio_state,
        profile.clone(),
        PositionExecution::Parallel,
    )?;
    let t1_job = plan.register_evaluation(
        market_t1_state,
        portfolio_state,
        profile,
        PositionExecution::Parallel,
    )?;
    let mut outcome = plan.execute();
    let prepared_t0 = outcome
        .take_valuation(t0_job)
        .map_err(|error| attribution_endpoint_error(error, "T0"))?;
    let prepared_t1 = outcome
        .take_valuation(t1_job)
        .map_err(|error| attribution_endpoint_error(error, "T1"))?;
    Ok((prepared_t0, prepared_t1))
}

/// Validate and return the unscaled valuation result prepared for a position.
pub(super) fn prepared_valuation_result<'a>(
    position: &crate::position::Position,
    position_value: &'a PositionValue,
    as_of: Date,
    endpoint: &str,
    require_complete_metrics: bool,
) -> Result<&'a finstack_quant_valuations::results::ValuationResult> {
    if position_value.position_id != position.position_id {
        return Err(Error::valuation(
            position.position_id.clone(),
            finstack_quant_core::ErrorKind::Validation,
            format!(
                "Attribution {endpoint} prepared position ID '{}' does not match '{}'",
                position_value.position_id, position.position_id
            ),
        ));
    }

    if require_complete_metrics && !position_value.risk_metrics_complete {
        return Err(Error::valuation(
            position.position_id.clone(),
            finstack_quant_core::ErrorKind::Validation,
            format!(
                "Attribution {endpoint} valuation is incomplete and cannot satisfy a strict metrics-based attribution request"
            ),
        ));
    }

    let valuation_result = position_value.valuation_result.as_ref().ok_or_else(|| {
        Error::valuation(
            position.position_id.clone(),
            finstack_quant_core::ErrorKind::NotFound,
            format!("Attribution {endpoint} valuation result is missing"),
        )
    })?;

    if valuation_result.instrument_id != position.instrument.id() {
        return Err(Error::valuation(
            position.position_id.clone(),
            finstack_quant_core::ErrorKind::Validation,
            format!(
                "Attribution {endpoint} valuation instrument stamp '{}' does not match '{}'",
                valuation_result.instrument_id,
                position.instrument.id()
            ),
        ));
    }

    if valuation_result.as_of != as_of {
        return Err(Error::valuation(
            position.position_id.clone(),
            finstack_quant_core::ErrorKind::Validation,
            format!(
                "Attribution {endpoint} valuation date stamp {} does not match {as_of}",
                valuation_result.as_of
            ),
        ));
    }

    Ok(valuation_result)
}
/// Reduce exact ordinary endpoint valuations through a repricing attribution
/// method without pricing either endpoint again.
pub(crate) fn reduce_prepared(
    portfolio: &Portfolio,
    markets: (&MarketContext, &MarketContext),
    dates: (Date, Date),
    config: &FinstackConfig,
    method: &AttributionMethod,
    prepared_t0: &PortfolioValuation,
    prepared_t1: &PortfolioValuation,
) -> Result<PortfolioAttribution> {
    let (market_t0, market_t1) = markets;
    let (as_of_t0, as_of_t1) = dates;
    let require_complete_metrics = matches!(method, AttributionMethod::MetricsBased);
    if prepared_t0.as_of != as_of_t0 {
        return Err(Error::invalid_input(format!(
            "prepared attribution T0 portfolio valuation is stamped {} instead of {as_of_t0}",
            prepared_t0.as_of
        )));
    }
    if prepared_t1.as_of != as_of_t1 {
        return Err(Error::invalid_input(format!(
            "prepared attribution T1 portfolio valuation is stamped {} instead of {as_of_t1}",
            prepared_t1.as_of
        )));
    }

    let reduce_position =
        |position: &crate::position::Position| -> Result<PositionAttributionData> {
            let position_t0 = prepared_t0
                .get_position_value(position.position_id.as_str())
                .ok_or_else(|| {
                    Error::valuation(
                        position.position_id.clone(),
                        finstack_quant_core::ErrorKind::NotFound,
                        "Attribution T0 prepared position valuation is missing",
                    )
                })?;
            let position_t1 = prepared_t1
                .get_position_value(position.position_id.as_str())
                .ok_or_else(|| {
                    Error::valuation(
                        position.position_id.clone(),
                        finstack_quant_core::ErrorKind::NotFound,
                        "Attribution T1 prepared position valuation is missing",
                    )
                })?;
            let val_t0 = prepared_valuation_result(
                position,
                position_t0,
                as_of_t0,
                "T0",
                require_complete_metrics,
            )?;
            let val_t1 = prepared_valuation_result(
                position,
                position_t1,
                as_of_t1,
                "T1",
                require_complete_metrics,
            )?;
            let attribution_failed = |error: finstack_quant_core::Error| Error::ValuationError {
                position_id: position.position_id.clone(),
                message: format!("Attribution failed: {error}"),
                kind: error.kind(),
            };
            let mut pos_attr = if let Some(composite) = position
                .instrument
                .as_any()
                .downcast_ref::<CompositeInstrument>()
            {
                attribute_composite_primitives(
                    composite, market_t0, market_t1, as_of_t0, as_of_t1, config, method,
                )?
            } else if require_complete_metrics {
                attribute_pnl_metrics_based(
                    &position.instrument,
                    market_t0,
                    market_t1,
                    val_t0,
                    val_t1,
                    as_of_t0,
                    as_of_t1,
                )
                .map_err(attribution_failed)?
            } else {
                finstack_quant_attribution::attribute_pnl(
                    method,
                    &AttributionRequest {
                        strict_validation: false,
                        prepared_endpoints: Some((val_t0.value, val_t1.value)),
                        ..AttributionRequest::new(
                            &position.instrument,
                            market_t0,
                            market_t1,
                            as_of_t0,
                            as_of_t1,
                            config,
                        )
                    },
                )
                .map_err(attribution_failed)?
            };

            pos_attr
                .scale(position.scale_factor())
                .map_err(|error| Error::ValuationError {
                    position_id: position.position_id.clone(),
                    message: format!("Attribution scaling failed: {error}"),
                    kind: error.kind(),
                })?;
            Ok(PositionAttributionData {
                position_id: position.position_id.clone(),
                pos_attr,
                val_t0_native: position_t0.value_native,
            })
        };

    let position_data = try_map_positions(&portfolio.positions, reduce_position)?;

    aggregate_position_attributions(
        portfolio,
        method,
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        position_data,
    )
}

/// Aggregate ordered, native-currency position attributions into the
/// portfolio base currency and apply opening-principal FX translation.
pub(super) fn aggregate_position_attributions(
    portfolio: &Portfolio,
    method: &AttributionMethod,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    position_data: Vec<PositionAttributionData>,
) -> Result<PortfolioAttribution> {
    let mut aggregate = PnlAttribution::new(
        Money::from((0_i64, portfolio.base_currency)),
        portfolio.id.as_str(),
        as_of_t0,
        as_of_t1,
        method.clone(),
    );
    let mut by_position: IndexMap<PositionId, PnlAttribution> =
        IndexMap::with_capacity(position_data.len());

    for PositionAttributionData {
        position_id,
        pos_attr,
        val_t0_native,
    } in position_data
    {
        // Factors at T1 FX; the opening principal's FX move lands in
        // fx_translation_pnl. A base-currency position is unchanged.
        let mut in_base = pos_attr.clone();
        translate_to_target_currency(
            &mut in_base,
            val_t0_native,
            portfolio.base_currency,
            market_t0,
            market_t1,
            as_of_t0,
            as_of_t1,
        )?;
        aggregate.accumulate(in_base)?;
        by_position.insert(position_id, pos_attr);
    }

    Ok(PortfolioAttribution::from_aggregate(aggregate, by_position))
}

pub(super) fn attribution_endpoint_error(error: Error, endpoint: &str) -> Error {
    match error {
        Error::ValuationError {
            position_id,
            message,
            kind,
        } => Error::valuation(
            position_id,
            kind,
            format!("Attribution {endpoint} valuation failed: {message}"),
        ),
        other => other,
    }
}

/// Perform P&L attribution for an entire portfolio.
///
/// Attributes each position's P&L and aggregates to portfolio base currency.
/// Each position is attributed using the specified method (Parallel, Waterfall,
/// or MetricsBased), and the results are converted to the portfolio's base
/// currency with explicit FX translation P&L tracking.
///
/// # Arguments
///
/// * `portfolio` - Portfolio to attribute
/// * `market_t0` - Market context at T₀
/// * `market_t1` - Market context at T₁
/// * `as_of_t0` - Valuation date at T₀ (typically yesterday for day-over-day)
/// * `as_of_t1` - Valuation date at T₁ (typically today for day-over-day)
/// * `config` - Finstack configuration
/// * `method` - Attribution methodology (Parallel, Waterfall, or MetricsBased)
///
/// # Returns
///
/// Portfolio-level attribution with per-position breakdown.
///
/// # Examples
///
/// ```no_run
/// use finstack_quant_portfolio::attribution::attribute_portfolio_pnl;
/// use finstack_quant_attribution::AttributionMethod;
/// use finstack_quant_core::config::FinstackConfig;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_portfolio::Portfolio;
/// use time::macros::date;
///
/// # fn main() -> finstack_quant_portfolio::Result<()> {
/// let as_of_t0 = date!(2025-11-20);  // Yesterday
/// let as_of_t1 = date!(2025-11-21);  // Today
///
/// # let portfolio: Portfolio = unimplemented!("Provide your portfolio");
/// # let market_t0: MarketContext = unimplemented!("Provide market at t0");
/// # let market_t1: MarketContext = unimplemented!("Provide market at t1");
/// # let config: FinstackConfig = unimplemented!("Provide finstack config");
/// let attribution = attribute_portfolio_pnl(
///     &portfolio,
///     &market_t0,
///     &market_t1,
///     as_of_t0,
///     as_of_t1,
///     &config,
///     AttributionMethod::Parallel,
/// )?;
///
/// println!("Portfolio P&L: {}", attribution.total_pnl);
/// println!("Total Carry: {}", attribution.carry);
/// println!("FX Translation: {}", attribution.fx_translation_pnl);
///
/// // Drill down to specific position
/// if let Some(pos_attr) = attribution.by_position.get("POS_001") {
///     println!("Position POS_001 P&L: {}", pos_attr.total_pnl);
/// }
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - Parametric risk-reporting background: `docs/REFERENCES.md#jpmorgan1996RiskMetrics`
///
///
/// # Errors
///
/// Propagates per-position valuation and attribution errors. For a position
/// outside the base currency, both market contexts must provide an FX matrix
/// and a conversion rate to the base currency at their respective as-of dates;
/// otherwise the call returns missing-market-data or FX-conversion errors.
pub fn attribute_portfolio_pnl(
    portfolio: &Portfolio,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: &FinstackConfig,
    method: AttributionMethod,
) -> Result<PortfolioAttribution> {
    let (prepared_t0, prepared_t1) = prepare_endpoints(
        portfolio, market_t0, market_t1, as_of_t0, as_of_t1, config, &method,
    )?;
    reduce_prepared(
        portfolio,
        (market_t0, market_t1),
        (as_of_t0, as_of_t1),
        config,
        &method,
        &prepared_t0,
        &prepared_t1,
    )
}

impl PortfolioAttribution {
    /// Whether `total_pnl` is effectively zero under the default
    /// [`RoundingContext`] (half a unit of the base currency's output scale),
    /// so no bucket has a defined share of it. [`Self::explain`] and
    /// [`Self::pct_of_total_by_bucket`] share this rule.
    fn total_is_effectively_zero(&self) -> bool {
        RoundingContext::default().is_effectively_zero(
            self.total_pnl.amount(),
            ZeroKind::Money(self.total_pnl.currency()),
        )
    }

    /// Generate explanation tree for portfolio attribution.
    ///
    /// Each bucket line shows its amount and its share of `total_pnl`; the
    /// share prints as `0.0%` when `total_pnl` is effectively zero under the
    /// same rule as [`Self::pct_of_total_by_bucket`], which returns `None`
    /// shares there.
    pub fn explain(&self) -> String {
        let mut lines = Vec::new();
        let total_is_zero = self.total_is_effectively_zero();

        let fmt = |amount: &Money, total: &Money| -> String {
            let pct = if total_is_zero {
                0.0
            } else {
                (amount.amount() / total.amount()) * 100.0
            };
            format!("{} ({:.1}%)", amount, pct)
        };

        lines.push(format!("Portfolio P&L: {}", self.total_pnl));
        lines.push(format!("  ├─ Carry: {}", fmt(&self.carry, &self.total_pnl)));
        lines.push(format!(
            "  ├─ Rates Curves: {}",
            fmt(&self.rates_curves_pnl, &self.total_pnl)
        ));
        lines.push(format!(
            "  ├─ Credit Curves: {}",
            fmt(&self.credit_curves_pnl, &self.total_pnl)
        ));
        lines.push(format!(
            "  ├─ Inflation: {}",
            fmt(&self.inflation_curves_pnl, &self.total_pnl)
        ));
        lines.push(format!(
            "  ├─ Correlations: {}",
            fmt(&self.correlations_pnl, &self.total_pnl)
        ));
        lines.push(format!("  ├─ FX: {}", fmt(&self.fx_pnl, &self.total_pnl)));
        lines.push(format!(
            "  ├─ FX Translation: {}",
            fmt(&self.fx_translation_pnl, &self.total_pnl)
        ));
        lines.push(format!(
            "  ├─ Cross Factor: {}",
            fmt(&self.cross_factor_pnl, &self.total_pnl)
        ));
        lines.push(format!("  ├─ Vol: {}", fmt(&self.vol_pnl, &self.total_pnl)));
        lines.push(format!(
            "  ├─ Model Params: {}",
            fmt(&self.model_params_pnl, &self.total_pnl)
        ));
        lines.push(format!(
            "  ├─ Market Scalars: {}",
            fmt(&self.market_scalars_pnl, &self.total_pnl)
        ));
        lines.push(format!(
            "  └─ Residual: {}",
            fmt(&self.residual, &self.total_pnl)
        ));

        lines.join("\n")
    }

    /// Every base-currency money bucket with its share of total P&L, in the
    /// struct's field order.
    ///
    /// Lists the eleven factor buckets and `residual`, the same twelve
    /// amounts [`Self::reconciliation_check`] adds up, so a report reads the
    /// buckets from Rust instead of naming them and a bucket added to this
    /// struct cannot silently go missing. `total_pnl` is the headline value,
    /// not a bucket; `by_position` holds native-currency detail and is not
    /// listed. Each share is `amount / total_pnl * 100`.
    ///
    /// # Returns
    ///
    /// Twelve [`PnlBucketShare`] records whose `field` is this struct's wire
    /// field name; `pct_of_total` is `None` on every record when `total_pnl`
    /// is effectively zero under the default [`RoundingContext`] (half a unit
    /// of the base currency's output scale), because a share of zero is
    /// undefined.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_attribution::PnlBucket;
    /// use finstack_quant_core::{currency::Currency, money::Money};
    /// use finstack_quant_portfolio::attribution::PortfolioAttribution;
    /// use indexmap::IndexMap;
    ///
    /// let usd = |amount: i64| Money::from((amount, Currency::USD));
    /// let attribution = PortfolioAttribution {
    ///     total_pnl: usd(200),
    ///     carry: usd(50),
    ///     rates_curves_pnl: usd(150),
    ///     credit_curves_pnl: usd(0),
    ///     inflation_curves_pnl: usd(0),
    ///     correlations_pnl: usd(0),
    ///     fx_pnl: usd(0),
    ///     fx_translation_pnl: usd(0),
    ///     cross_factor_pnl: usd(0),
    ///     vol_pnl: usd(0),
    ///     model_params_pnl: usd(0),
    ///     market_scalars_pnl: usd(0),
    ///     residual: usd(0),
    ///     by_position: IndexMap::new(),
    ///     result_invalid: false,
    /// };
    /// let buckets = attribution.pct_of_total_by_bucket();
    /// assert_eq!(buckets[0].field, PnlBucket::Carry);
    /// assert_eq!(buckets[0].pct_of_total, Some(25.0));
    /// ```
    #[must_use]
    pub fn pct_of_total_by_bucket(&self) -> Vec<PnlBucketShare> {
        let total = self.total_pnl.amount();
        let total_is_zero = self.total_is_effectively_zero();
        [
            (PnlBucket::Carry, &self.carry),
            (PnlBucket::RatesCurvesPnl, &self.rates_curves_pnl),
            (PnlBucket::CreditCurvesPnl, &self.credit_curves_pnl),
            (PnlBucket::InflationCurvesPnl, &self.inflation_curves_pnl),
            (PnlBucket::CorrelationsPnl, &self.correlations_pnl),
            (PnlBucket::FxPnl, &self.fx_pnl),
            (PnlBucket::FxTranslationPnl, &self.fx_translation_pnl),
            (PnlBucket::CrossFactorPnl, &self.cross_factor_pnl),
            (PnlBucket::VolPnl, &self.vol_pnl),
            (PnlBucket::ModelParamsPnl, &self.model_params_pnl),
            (PnlBucket::MarketScalarsPnl, &self.market_scalars_pnl),
            (PnlBucket::Residual, &self.residual),
        ]
        .into_iter()
        .map(|(field, amount)| PnlBucketShare {
            field,
            amount: *amount,
            pct_of_total: (!total_is_zero).then(|| amount.amount() / total * 100.0),
        })
        .collect()
    }

    /// Check that the sum of all factor P&L buckets plus FX translation
    /// reconciles against `total_pnl` within the given tolerance.
    ///
    /// This uses the portfolio-level (base-currency) aggregates, so no
    /// additional FX conversion is needed.
    ///
    /// If [`PortfolioAttribution::result_invalid`] is set, `is_reconciled` is
    /// forced to `false` regardless of the numeric residual: an aggregate built
    /// from a corrupted constituent must never be reported as reconciled.
    ///
    /// # Arguments
    ///
    /// * `tolerance` - Absolute tolerance in base-currency units (e.g. 0.01
    ///   for one-cent precision).
    pub fn reconciliation_check(&self, tolerance: f64) -> ReconciliationReport {
        let mut acc = NeumaierAccumulator::new();
        let mut valid = true;
        for term in [
            self.carry.amount(),
            self.rates_curves_pnl.amount(),
            self.credit_curves_pnl.amount(),
            self.inflation_curves_pnl.amount(),
            self.correlations_pnl.amount(),
            self.fx_pnl.amount(),
            self.cross_factor_pnl.amount(),
            self.vol_pnl.amount(),
            self.model_params_pnl.amount(),
            self.market_scalars_pnl.amount(),
            self.residual.amount(),
            self.fx_translation_pnl.amount(),
        ] {
            if acc.add(term).is_err() {
                valid = false;
            }
        }

        let explained = if valid {
            acc.get_total().unwrap_or(f64::NAN)
        } else {
            f64::NAN
        };
        let total_residual = self.total_pnl.amount() - explained;
        // A portfolio aggregated from an invalid constituent cannot be trusted
        // to reconcile, even if the (equally corrupted) buckets happen to net
        // to within tolerance — never report `is_reconciled` in that case.
        let is_reconciled =
            !self.result_invalid && total_residual.is_finite() && total_residual.abs() <= tolerance;

        ReconciliationReport {
            total_residual,
            is_reconciled,
            tolerance,
        }
    }
}
