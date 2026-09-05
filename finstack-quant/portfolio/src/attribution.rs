//! Portfolio-level P&L attribution.
//!
//! Aggregates instrument-level attribution across all positions in a portfolio,
//! with currency conversion to portfolio base currency.

use crate::error::{Error, Result};
#[cfg(not(target_arch = "wasm32"))]
use crate::evaluation::POSITION_PARALLEL_MIN_POSITIONS;
use crate::evaluation::{EvaluationProfile, PortfolioEvaluationPlan, PositionExecution};
use crate::portfolio::Portfolio;
use crate::types::PositionId;
use crate::valuation::{
    PortfolioValuation, PortfolioValuationOptions, PositionValue, RequestedMetrics,
};
use finstack_quant_attribution::{
    attribute_pnl_metrics_based, default_attribution_metrics, AttributionRequest,
};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::{CompositeInstrument, Instrument, PricingOptions};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Re-export attribution types appearing in this module's public API so direct
/// portfolio consumers do not need a separate, version-sensitive dependency.
pub use finstack_quant_attribution::{
    AttributionFactor, AttributionMeta, AttributionMethod, CarryDetail, CorrelationsAttribution,
    CreditCarryByLevel, CreditCarryDecomposition, CreditCurvesAttribution, CreditFactorAttribution,
    CrossFactorDetail, ExecutionPolicy, FxAttribution, InflationCurvesAttribution, LevelCarry,
    LevelPnl, ModelParamsAttribution, PnlAttribution, RatesCurvesAttribution, ScalarsAttribution,
    SourceLine, TaylorAttributionConfig, VolAttribution,
};

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

    /// Aggregate rates curves detail (optional).
    pub rates_detail: Option<RatesCurvesAttribution>,

    /// Aggregate credit curves detail (optional).
    pub credit_detail: Option<CreditCurvesAttribution>,

    /// Aggregate inflation curves detail (optional).
    pub inflation_detail: Option<InflationCurvesAttribution>,

    /// Aggregate correlations detail (optional).
    pub correlations_detail: Option<CorrelationsAttribution>,

    /// Aggregate FX detail (optional).
    pub fx_detail: Option<FxAttribution>,

    /// Aggregate volatility detail (optional).
    pub vol_detail: Option<VolAttribution>,

    /// Aggregate scalars detail (optional).
    pub scalars_detail: Option<ScalarsAttribution>,

    /// True if any constituent position's attribution was flagged invalid
    /// (for example, a non-finite factor sensitivity — see
    /// [`PnlAttribution::result_invalid`]). When `true`, the portfolio
    /// aggregates and [`PortfolioAttribution::reconciliation_check`] are not
    /// trustworthy and must not be relied on for reporting.
    ///
    pub result_invalid: bool,
}

/// Report from reconciling position-level P&L attribution against portfolio totals.
///
/// Verifies that the sum of all factor P&L buckets plus FX translation equals `total_pnl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationReport {
    /// Total residual: `total_pnl - (sum of factor buckets + fx_translation_pnl)`.
    pub total_residual: f64,
    /// Whether the reconciliation passes within tolerance.
    pub is_reconciled: bool,
    /// Tolerance used for the check.
    pub tolerance: f64,
}

struct PositionAttributionData {
    position_id: PositionId,
    pos_attr: PnlAttribution,
    val_t0_native: Money,
    inst_currency: Currency,
}

#[derive(Copy, Clone)]
enum FactorBucket {
    TotalPnl = 0,
    Carry = 1,
    RatesCurvesPnl = 2,
    CreditCurvesPnl = 3,
    InflationCurvesPnl = 4,
    CorrelationsPnl = 5,
    FxPnl = 6,
    FxTranslationPnl = 7,
    CrossFactorPnl = 8,
    VolPnl = 9,
    ModelParamsPnl = 10,
    MarketScalarsPnl = 11,
    Residual = 12,
}

const N_BUCKETS: usize = 13;

/// Private helper that aggregates portfolio-level factor P&L buckets using
/// Neumaier summation. An enum-keyed array eliminates per-field duplication
/// and makes the bucket-to-output mapping a single point of truth.
struct FactorAccumulator {
    buckets: [NeumaierAccumulator; N_BUCKETS],
    /// Set once any folded-in position carried `result_invalid = true`.
    result_invalid: bool,
}

impl FactorAccumulator {
    fn new() -> Self {
        Self {
            buckets: [NeumaierAccumulator::new(); N_BUCKETS],
            result_invalid: false,
        }
    }

    fn add(&mut self, b: FactorBucket, x: f64) {
        self.buckets[b as usize].add(x);
    }

    fn total(&self, b: FactorBucket) -> f64 {
        self.buckets[b as usize].current()
    }

    fn add_converted(
        &mut self,
        pos_attr: &PnlAttribution,
        convert: &impl Fn(Money) -> Result<Money>,
    ) -> Result<()> {
        // A single invalid constituent makes the whole aggregate untrustworthy.
        self.result_invalid |= pos_attr.result_invalid;
        self.add(
            FactorBucket::TotalPnl,
            convert(pos_attr.total_pnl)?.amount(),
        );
        self.add(FactorBucket::Carry, convert(pos_attr.carry)?.amount());
        self.add(
            FactorBucket::RatesCurvesPnl,
            convert(pos_attr.rates_curves_pnl)?.amount(),
        );
        self.add(
            FactorBucket::CreditCurvesPnl,
            convert(pos_attr.credit_curves_pnl)?.amount(),
        );
        self.add(
            FactorBucket::InflationCurvesPnl,
            convert(pos_attr.inflation_curves_pnl)?.amount(),
        );
        self.add(
            FactorBucket::CorrelationsPnl,
            convert(pos_attr.correlations_pnl)?.amount(),
        );
        self.add(FactorBucket::FxPnl, convert(pos_attr.fx_pnl)?.amount());
        self.add(
            FactorBucket::CrossFactorPnl,
            convert(pos_attr.cross_factor_pnl)?.amount(),
        );
        self.add(FactorBucket::VolPnl, convert(pos_attr.vol_pnl)?.amount());
        self.add(
            FactorBucket::ModelParamsPnl,
            convert(pos_attr.model_params_pnl)?.amount(),
        );
        self.add(
            FactorBucket::MarketScalarsPnl,
            convert(pos_attr.market_scalars_pnl)?.amount(),
        );
        self.add(FactorBucket::Residual, convert(pos_attr.residual)?.amount());
        Ok(())
    }

    fn add_fx_translation(&mut self, amount: f64) {
        self.add(FactorBucket::FxTranslationPnl, amount);
        self.add(FactorBucket::TotalPnl, amount);
    }

    fn into_portfolio_attribution(
        self,
        base_currency: Currency,
        by_position: IndexMap<PositionId, PnlAttribution>,
    ) -> finstack_quant_core::Result<PortfolioAttribution> {
        Ok({
            PortfolioAttribution {
                total_pnl: Money::new(self.total(FactorBucket::TotalPnl), base_currency)?,
                carry: Money::new(self.total(FactorBucket::Carry), base_currency)?,
                rates_curves_pnl: Money::new(
                    self.total(FactorBucket::RatesCurvesPnl),
                    base_currency,
                )?,
                credit_curves_pnl: Money::new(
                    self.total(FactorBucket::CreditCurvesPnl),
                    base_currency,
                )?,
                inflation_curves_pnl: Money::new(
                    self.total(FactorBucket::InflationCurvesPnl),
                    base_currency,
                )?,
                correlations_pnl: Money::new(
                    self.total(FactorBucket::CorrelationsPnl),
                    base_currency,
                )?,
                fx_pnl: Money::new(self.total(FactorBucket::FxPnl), base_currency)?,
                fx_translation_pnl: Money::new(
                    self.total(FactorBucket::FxTranslationPnl),
                    base_currency,
                )?,
                cross_factor_pnl: Money::new(
                    self.total(FactorBucket::CrossFactorPnl),
                    base_currency,
                )?,
                vol_pnl: Money::new(self.total(FactorBucket::VolPnl), base_currency)?,
                model_params_pnl: Money::new(
                    self.total(FactorBucket::ModelParamsPnl),
                    base_currency,
                )?,
                market_scalars_pnl: Money::new(
                    self.total(FactorBucket::MarketScalarsPnl),
                    base_currency,
                )?,
                residual: Money::new(self.total(FactorBucket::Residual), base_currency)?,
                by_position,
                rates_detail: None,
                credit_detail: None,
                inflation_detail: None,
                correlations_detail: None,
                fx_detail: None,
                vol_detail: None,
                scalars_detail: None,
                result_invalid: self.result_invalid,
            }
        })
    }
}

struct MethodOwnedAttributionRequest<'a> {
    market_t0: &'a MarketContext,
    market_t1: &'a MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: &'a FinstackConfig,
    method: &'a AttributionMethod,
}

fn attribute_composite_primitives(
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
    aggregate.residual = Money::from((0_i64, reporting_currency));

    for exposure in composite.flatten_primitives().map_err(Error::Core)? {
        let definition = exposure.instrument_definition().ok_or_else(|| {
            Error::valuation(
                composite.id(),
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
        let mut primitive = if matches!(method, AttributionMethod::MetricsBased) {
            let metrics = default_attribution_metrics();
            let options = PricingOptions::default().with_config(config);
            let result_t0 = instrument
                .price_with_metrics(market_t0, as_of_t0, &metrics, options.clone())
                .map_err(|error| Error::Core(error.into()))?;
            let result_t1 = instrument
                .price_with_metrics(market_t1, as_of_t1, &metrics, options)
                .map_err(|error| Error::Core(error.into()))?;
            attribute_pnl_metrics_based(
                &instrument,
                market_t0,
                market_t1,
                &result_t0,
                &result_t1,
                as_of_t0,
                as_of_t1,
            )
            .map_err(Error::Core)?
        } else {
            finstack_quant_attribution::attribute_pnl(
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
            .map_err(Error::Core)?
        };
        primitive.scale(exposure.quantity);
        translate_primitive_attribution(
            &mut primitive,
            value_t0.amount() * exposure.quantity,
            market_t0,
            market_t1,
            as_of_t0,
            as_of_t1,
            reporting_currency,
        )?;
        add_primitive_attribution(&mut aggregate, primitive)?;
    }

    aggregate.meta.notes.push(
        "Composite attribution is the reporting-currency sum of frozen primitive paths; detailed leaf reports remain available through primitive decomposition"
            .to_string(),
    );
    aggregate.compute_residual().map_err(Error::Core)?;
    Ok(aggregate)
}

fn translate_primitive_attribution(
    attribution: &mut PnlAttribution,
    opening_value: f64,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    reporting_currency: Currency,
) -> Result<()> {
    let native_currency = attribution.total_pnl.currency();
    let rate_t0 = crate::fx::convert_to_base(
        Money::from((1_i64, native_currency)),
        as_of_t0,
        market_t0,
        reporting_currency,
    )?
    .amount();
    let rate_t1 = crate::fx::convert_to_base(
        Money::from((1_i64, native_currency)),
        as_of_t1,
        market_t1,
        reporting_currency,
    )?
    .amount();
    let principal_translation = opening_value * (rate_t1 - rate_t0);
    attribution.total_pnl = translated(attribution.total_pnl, rate_t1, reporting_currency)?;
    attribution.total_pnl = Money::new(
        attribution.total_pnl.amount() + principal_translation,
        reporting_currency,
    )?;
    attribution.mark_to_market_pnl = attribution
        .mark_to_market_pnl
        .map(|value| {
            Money::new(
                value.amount() * rate_t1 + principal_translation,
                reporting_currency,
            )
        })
        .transpose()?;
    attribution.carry = translated(attribution.carry, rate_t1, reporting_currency)?;
    attribution.rates_curves_pnl =
        translated(attribution.rates_curves_pnl, rate_t1, reporting_currency)?;
    attribution.credit_curves_pnl =
        translated(attribution.credit_curves_pnl, rate_t1, reporting_currency)?;
    attribution.inflation_curves_pnl = translated(
        attribution.inflation_curves_pnl,
        rate_t1,
        reporting_currency,
    )?;
    attribution.correlations_pnl =
        translated(attribution.correlations_pnl, rate_t1, reporting_currency)?;
    attribution.fx_pnl = translated(attribution.fx_pnl, rate_t1, reporting_currency)?;
    attribution.vol_pnl = translated(attribution.vol_pnl, rate_t1, reporting_currency)?;
    attribution.cross_factor_pnl =
        translated(attribution.cross_factor_pnl, rate_t1, reporting_currency)?;
    attribution.model_params_pnl =
        translated(attribution.model_params_pnl, rate_t1, reporting_currency)?;
    attribution.market_scalars_pnl =
        translated(attribution.market_scalars_pnl, rate_t1, reporting_currency)?;
    attribution.fx_translation_pnl = Money::new(
        attribution.fx_translation_pnl.amount() * rate_t1 + principal_translation,
        reporting_currency,
    )?;
    attribution.residual = translated(attribution.residual, rate_t1, reporting_currency)?;
    attribution.carry_detail = None;
    attribution.rates_detail = None;
    attribution.credit_detail = None;
    attribution.inflation_detail = None;
    attribution.correlations_detail = None;
    attribution.fx_detail = None;
    attribution.vol_detail = None;
    attribution.cross_factor_detail = None;
    attribution.model_params_detail = None;
    attribution.scalars_detail = None;
    attribution.credit_factor_detail = None;
    attribution.credit_carry_decomposition = None;
    Ok(())
}

fn translated(value: Money, rate: f64, currency: Currency) -> finstack_quant_core::Result<Money> {
    Money::new(value.amount() * rate, currency)
}

fn add_money(target: &mut Money, source: Money) -> finstack_quant_core::Result<()> {
    *target = Money::new(target.amount() + source.amount(), target.currency())?;
    Ok(())
}

fn add_primitive_attribution(
    aggregate: &mut PnlAttribution,
    primitive: PnlAttribution,
) -> finstack_quant_core::Result<()> {
    add_money(&mut aggregate.total_pnl, primitive.total_pnl)?;
    match (
        aggregate.mark_to_market_pnl.as_mut(),
        primitive.mark_to_market_pnl,
    ) {
        (Some(target), Some(source)) => add_money(target, source)?,
        _ => aggregate.mark_to_market_pnl = None,
    }
    add_money(&mut aggregate.carry, primitive.carry)?;
    add_money(&mut aggregate.rates_curves_pnl, primitive.rates_curves_pnl)?;
    add_money(
        &mut aggregate.credit_curves_pnl,
        primitive.credit_curves_pnl,
    )?;
    add_money(
        &mut aggregate.inflation_curves_pnl,
        primitive.inflation_curves_pnl,
    )?;
    add_money(&mut aggregate.correlations_pnl, primitive.correlations_pnl)?;
    add_money(&mut aggregate.fx_pnl, primitive.fx_pnl)?;
    add_money(&mut aggregate.vol_pnl, primitive.vol_pnl)?;
    add_money(&mut aggregate.cross_factor_pnl, primitive.cross_factor_pnl)?;
    add_money(&mut aggregate.model_params_pnl, primitive.model_params_pnl)?;
    add_money(
        &mut aggregate.market_scalars_pnl,
        primitive.market_scalars_pnl,
    )?;
    add_money(
        &mut aggregate.fx_translation_pnl,
        primitive.fx_translation_pnl,
    )?;
    aggregate.result_invalid |= primitive.result_invalid;
    aggregate.meta.num_repricings += primitive.meta.num_repricings;
    aggregate.meta.notes.extend(primitive.meta.notes);
    Ok(())
}

/// Exact canonical evaluation profile required by an attribution method.
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

/// Attribute one position through a method that owns its repricing workflow.
///
/// Metrics-based attribution is deliberately excluded: it consumes the
/// portfolio-level prepared endpoint valuations in
/// [`reduce_metrics_based_prepared`] so a portfolio call does not perform two
/// additional metric valuations per position.
fn attribute_single_position_method_owned(
    position: &crate::position::Position,
    request: &MethodOwnedAttributionRequest<'_>,
    val_t0_native: Money,
    val_t0: Money,
    val_t1: Money,
) -> Result<PositionAttributionData> {
    if let Some(composite) = position
        .instrument
        .as_any()
        .downcast_ref::<CompositeInstrument>()
    {
        let mut pos_attr = attribute_composite_primitives(
            composite,
            request.market_t0,
            request.market_t1,
            request.as_of_t0,
            request.as_of_t1,
            request.config,
            request.method,
        )?;
        pos_attr.scale(position.scale_factor());
        return Ok(PositionAttributionData {
            position_id: position.position_id.clone(),
            pos_attr,
            val_t0_native,
            inst_currency: composite.spec.reporting_currency,
        });
    }
    let mut pos_attr = finstack_quant_attribution::attribute_pnl(
        request.method,
        &AttributionRequest {
            strict_validation: false,
            prepared_endpoints: Some((val_t0, val_t1)),
            ..AttributionRequest::new(
                &position.instrument,
                request.market_t0,
                request.market_t1,
                request.as_of_t0,
                request.as_of_t1,
                request.config,
            )
        },
    )
    .map_err(|error| Error::ValuationError {
        position_id: position.position_id.clone(),
        message: format!("Attribution failed: {error}"),
    })?;

    pos_attr.scale(position.scale_factor());
    let inst_currency = pos_attr.total_pnl.currency();

    Ok(PositionAttributionData {
        position_id: position.position_id.clone(),
        pos_attr,
        val_t0_native,
        inst_currency,
    })
}

/// Prepare the exact ordinary endpoint values consumed by repricing methods.
///
/// Both endpoints enter through the canonical portfolio executor once. The
/// attribution methods then perform only their financially distinct carry,
/// factor-restoration, and sensitivity repricings.
fn prepare_ordinary_endpoints(
    portfolio: &Portfolio,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: &FinstackConfig,
) -> Result<(PortfolioValuation, PortfolioValuation)> {
    let profile = attribution_endpoint_profile(&AttributionMethod::Parallel);
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
fn prepared_valuation_result<'a>(
    position: &crate::position::Position,
    position_value: &'a PositionValue,
    as_of: Date,
    endpoint: &str,
    require_complete_metrics: bool,
) -> Result<&'a finstack_quant_valuations::results::ValuationResult> {
    if position_value.position_id != position.position_id {
        return Err(Error::valuation(
            position.position_id.clone(),
            format!(
                "Attribution {endpoint} prepared position ID '{}' does not match '{}'",
                position_value.position_id, position.position_id
            ),
        ));
    }

    if require_complete_metrics && !position_value.risk_metrics_complete {
        return Err(Error::valuation(
            position.position_id.clone(),
            format!(
                "Attribution {endpoint} valuation is incomplete and cannot satisfy a strict metrics-based attribution request"
            ),
        ));
    }

    let valuation_result = position_value.valuation_result.as_ref().ok_or_else(|| {
        Error::valuation(
            position.position_id.clone(),
            format!("Attribution {endpoint} valuation result is missing"),
        )
    })?;

    if valuation_result.instrument_id != position.instrument.id() {
        return Err(Error::valuation(
            position.position_id.clone(),
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
            format!(
                "Attribution {endpoint} valuation date stamp {} does not match {as_of}",
                valuation_result.as_of
            ),
        ));
    }

    Ok(valuation_result)
}

/// Reduce strict, prepared endpoint valuations into metrics-based attribution.
///
/// The endpoint results contain unit instrument valuations. Attribution is
/// therefore calculated before applying the position scale, while the T0
/// principal used for portfolio FX translation is taken from the prepared,
/// already-scaled `PositionValue`.
pub(crate) fn reduce_metrics_based_prepared(
    portfolio: &Portfolio,
    markets: (&MarketContext, &MarketContext),
    dates: (Date, Date),
    config: &FinstackConfig,
    prepared_t0: &PortfolioValuation,
    prepared_t1: &PortfolioValuation,
) -> Result<PortfolioAttribution> {
    let (market_t0, market_t1) = markets;
    let (as_of_t0, as_of_t1) = dates;
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
                        "Attribution T0 prepared position valuation is missing",
                    )
                })?;
            let position_t1 = prepared_t1
                .get_position_value(position.position_id.as_str())
                .ok_or_else(|| {
                    Error::valuation(
                        position.position_id.clone(),
                        "Attribution T1 prepared position valuation is missing",
                    )
                })?;

            let val_t0 = prepared_valuation_result(position, position_t0, as_of_t0, "T0", true)?;
            let val_t1 = prepared_valuation_result(position, position_t1, as_of_t1, "T1", true)?;
            let mut pos_attr = if let Some(composite) = position
                .instrument
                .as_any()
                .downcast_ref::<CompositeInstrument>()
            {
                attribute_composite_primitives(
                    composite,
                    market_t0,
                    market_t1,
                    as_of_t0,
                    as_of_t1,
                    config,
                    &AttributionMethod::MetricsBased,
                )?
            } else {
                attribute_pnl_metrics_based(
                    &position.instrument,
                    market_t0,
                    market_t1,
                    val_t0,
                    val_t1,
                    as_of_t0,
                    as_of_t1,
                )
                .map_err(|error| Error::ValuationError {
                    position_id: position.position_id.clone(),
                    message: format!("Attribution failed: {error}"),
                })?
            };

            pos_attr.scale(position.scale_factor());
            let inst_currency = pos_attr.total_pnl.currency();
            Ok(PositionAttributionData {
                position_id: position.position_id.clone(),
                pos_attr,
                val_t0_native: position_t0.value_native,
                inst_currency,
            })
        };

    #[cfg(not(target_arch = "wasm32"))]
    let position_results: Vec<Result<PositionAttributionData>> =
        if portfolio.positions.len() >= POSITION_PARALLEL_MIN_POSITIONS {
            use rayon::prelude::*;
            portfolio
                .positions
                .par_iter()
                .map(reduce_position)
                .collect()
        } else {
            portfolio.positions.iter().map(reduce_position).collect()
        };

    #[cfg(target_arch = "wasm32")]
    let position_results: Vec<Result<PositionAttributionData>> =
        portfolio.positions.iter().map(reduce_position).collect();

    let position_data = position_results.into_iter().collect::<Result<Vec<_>>>()?;

    aggregate_position_attributions(
        portfolio,
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        position_data,
    )
}

/// Reduce exact ordinary endpoint valuations through a repricing attribution
/// method without pricing either endpoint again.
#[allow(clippy::too_many_arguments)]
pub(crate) fn reduce_method_owned_prepared(
    portfolio: &Portfolio,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: &FinstackConfig,
    method: &AttributionMethod,
    prepared_t0: &PortfolioValuation,
    prepared_t1: &PortfolioValuation,
) -> Result<PortfolioAttribution> {
    if matches!(method, AttributionMethod::MetricsBased) {
        return Err(Error::invalid_input(
            "metrics-based attribution requires complete prepared valuation results",
        ));
    }
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

    let request = MethodOwnedAttributionRequest {
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        config,
        method,
    };

    let reduce_position =
        |position: &crate::position::Position| -> Result<PositionAttributionData> {
            let position_t0 = prepared_t0
                .get_position_value(position.position_id.as_str())
                .ok_or_else(|| {
                    Error::valuation(
                        position.position_id.clone(),
                        "Attribution T0 prepared position valuation is missing",
                    )
                })?;
            let position_t1 = prepared_t1
                .get_position_value(position.position_id.as_str())
                .ok_or_else(|| {
                    Error::valuation(
                        position.position_id.clone(),
                        "Attribution T1 prepared position valuation is missing",
                    )
                })?;
            let val_t0 = prepared_valuation_result(position, position_t0, as_of_t0, "T0", false)?;
            let val_t1 = prepared_valuation_result(position, position_t1, as_of_t1, "T1", false)?;
            attribute_single_position_method_owned(
                position,
                &request,
                position_t0.value_native,
                val_t0.value,
                val_t1.value,
            )
        };

    #[cfg(not(target_arch = "wasm32"))]
    let position_results: Vec<Result<PositionAttributionData>> =
        if portfolio.positions.len() >= POSITION_PARALLEL_MIN_POSITIONS {
            use rayon::prelude::*;
            portfolio
                .positions
                .par_iter()
                .map(reduce_position)
                .collect()
        } else {
            portfolio.positions.iter().map(reduce_position).collect()
        };

    #[cfg(target_arch = "wasm32")]
    let position_results: Vec<Result<PositionAttributionData>> =
        portfolio.positions.iter().map(reduce_position).collect();

    let position_data = position_results.into_iter().collect::<Result<Vec<_>>>()?;

    aggregate_position_attributions(
        portfolio,
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        position_data,
    )
}

/// Aggregate ordered, native-currency position attributions into the
/// portfolio base currency and apply opening-principal FX translation.
fn aggregate_position_attributions(
    portfolio: &Portfolio,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    position_data: Vec<PositionAttributionData>,
) -> Result<PortfolioAttribution> {
    let base_currency = portfolio.base_currency;
    let mut acc = FactorAccumulator::new();
    let mut by_position: IndexMap<PositionId, PnlAttribution> =
        IndexMap::with_capacity(position_data.len());

    // Hoisted out of the per-position loop: the closure captures `market_t1`,
    // `base_currency`, and `as_of_t1` by reference and is reused for every field of
    // every position. Delegates to the shared `crate::fx::convert_to_base`
    // helper so the FX lookup + error mapping stay consistent with the rest of
    // the portfolio crate.
    let convert = |money: Money| -> Result<Money> {
        crate::fx::convert_to_base(money, as_of_t1, market_t1, base_currency)
    };

    for data in position_data {
        let PositionAttributionData {
            position_id,
            pos_attr,
            val_t0_native,
            inst_currency,
        } = data;

        acc.add_converted(&pos_attr, &convert)?;

        if inst_currency != base_currency {
            let rate_t0 =
                crate::fx::spot_rate_to_base(inst_currency, as_of_t0, market_t0, base_currency)?;
            let rate_t1 =
                crate::fx::spot_rate_to_base(inst_currency, as_of_t1, market_t1, base_currency)?;
            let principal_translation = val_t0_native.amount() * (rate_t1 - rate_t0);
            acc.add_fx_translation(principal_translation);
        }

        by_position.insert(position_id, pos_attr);
    }

    Ok(acc.into_portfolio_attribution(base_currency, by_position)?)
}

/// Compile and reduce strict T0/T1 metric endpoint valuations for the public
/// metrics-based attribution entry point.
fn attribute_portfolio_pnl_metrics_prepared(
    portfolio: &Portfolio,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: &FinstackConfig,
) -> Result<PortfolioAttribution> {
    let profile = attribution_endpoint_profile(&AttributionMethod::MetricsBased);
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
    let outcome = plan.execute();

    let prepared_t0 = outcome
        .get(t0_job)
        .map_err(|error| attribution_endpoint_error(error, "T0"))?;
    let prepared_t1 = outcome
        .get(t1_job)
        .map_err(|error| attribution_endpoint_error(error, "T1"))?;

    reduce_metrics_based_prepared(
        portfolio,
        (market_t0, market_t1),
        (as_of_t0, as_of_t1),
        config,
        prepared_t0.as_ref(),
        prepared_t1.as_ref(),
    )
}

fn attribution_endpoint_error(error: Error, endpoint: &str) -> Error {
    match error {
        Error::ValuationError {
            position_id,
            message,
        } => Error::valuation(
            position_id,
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
    if matches!(method, AttributionMethod::MetricsBased) {
        return attribute_portfolio_pnl_metrics_prepared(
            portfolio, market_t0, market_t1, as_of_t0, as_of_t1, config,
        );
    }

    let (prepared_t0, prepared_t1) =
        prepare_ordinary_endpoints(portfolio, market_t0, market_t1, as_of_t0, as_of_t1, config)?;
    reduce_method_owned_prepared(
        portfolio,
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        config,
        &method,
        &prepared_t0,
        &prepared_t1,
    )
}

impl PortfolioAttribution {
    /// Generate explanation tree for portfolio attribution.
    pub fn explain(&self) -> String {
        let mut lines = Vec::new();

        let fmt = |amount: &Money, total: &Money| -> String {
            let pct = if total.amount().abs() > 1e-10 {
                (amount.amount() / total.amount()) * 100.0
            } else {
                0.0
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
        acc.add(self.carry.amount());
        acc.add(self.rates_curves_pnl.amount());
        acc.add(self.credit_curves_pnl.amount());
        acc.add(self.inflation_curves_pnl.amount());
        acc.add(self.correlations_pnl.amount());
        acc.add(self.fx_pnl.amount());
        acc.add(self.cross_factor_pnl.amount());
        acc.add(self.vol_pnl.amount());
        acc.add(self.model_params_pnl.amount());
        acc.add(self.market_scalars_pnl.amount());
        acc.add(self.residual.amount());
        acc.add(self.fx_translation_pnl.amount());

        let total_residual = self.total_pnl.amount() - acc.total();
        // A portfolio aggregated from an invalid constituent cannot be trusted
        // to reconcile, even if the (equally corrupted) buckets happen to net
        // to within tolerance — never report `is_reconciled` in that case.
        let is_reconciled = !self.result_invalid && total_residual.abs() <= tolerance;

        ReconciliationReport {
            total_residual,
            is_reconciled,
            tolerance,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{Position, PositionUnit};
    use crate::types::Entity;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_valuations::instruments::composite::{
        CompositeLegSpec, CompositeSpec, RebalanceRule, WeightingMethod,
    };
    use finstack_quant_valuations::instruments::{Attributes, Instrument, PricingOptions};
    use finstack_quant_valuations::metrics::MetricId;
    use finstack_quant_valuations::pricer::InstrumentType;
    use finstack_quant_valuations::results::ValuationResult;
    use std::any::Any;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use time::macros::date;

    #[test]
    fn composite_attribution_reconciles_to_frozen_primitive_pnl() -> Result<()> {
        let spec = CompositeSpec::new(
            "A-B",
            Currency::USD,
            Money::from((100_i64, Currency::USD)),
            vec![
                CompositeLegSpec::new(
                    "A",
                    finstack_quant_valuations::instruments::InstrumentJson::Equity(
                        finstack_quant_valuations::instruments::Equity::new(
                            "A",
                            "A",
                            Currency::USD,
                        ),
                    ),
                    1.0,
                ),
                CompositeLegSpec::new(
                    "B",
                    finstack_quant_valuations::instruments::InstrumentJson::Equity(
                        finstack_quant_valuations::instruments::Equity::new(
                            "B",
                            "B",
                            Currency::USD,
                        ),
                    ),
                    -1.0,
                ),
            ],
            WeightingMethod::FixedQuantity,
            RebalanceRule::Manual,
        );
        let composite = spec.initialize_fixed(date!(2025 - 01 - 01))?.instrument;
        let position = Position::new(
            "P-COMPOSITE",
            "ENTITY",
            "A-B",
            Arc::new(composite),
            2.0,
            PositionUnit::Units,
        )?;
        let portfolio = Portfolio::builder("PORTFOLIO")
            .base_currency(Currency::USD)
            .as_of(date!(2025 - 01 - 01))
            .entity(Entity::new("ENTITY"))
            .position(position)
            .build()?;
        let market_t0 = MarketContext::new()
            .insert_price("A", MarketScalar::Unitless(100.0))
            .insert_price("B", MarketScalar::Unitless(90.0));
        let market_t1 = MarketContext::new()
            .insert_price("A", MarketScalar::Unitless(110.0))
            .insert_price("B", MarketScalar::Unitless(95.0));

        let attribution = attribute_portfolio_pnl(
            &portfolio,
            &market_t0,
            &market_t1,
            date!(2025 - 01 - 01),
            date!(2025 - 01 - 02),
            &FinstackConfig::default(),
            AttributionMethod::Parallel,
        )?;
        assert_eq!(attribution.total_pnl.amount(), 10.0);
        assert!(attribution.reconciliation_check(1.0e-9).is_reconciled);
        let position = attribution
            .by_position
            .get("P-COMPOSITE")
            .ok_or_else(|| Error::validation("missing composite attribution"))?;
        assert_eq!(position.total_pnl.amount(), 10.0);
        assert!(position
            .meta
            .notes
            .iter()
            .any(|note| note.contains("frozen primitive")));
        Ok(())
    }

    #[derive(Clone)]
    struct ConfigRequiredInstrument {
        attributes: Attributes,
        pv_only_calls: Arc<AtomicUsize>,
        metric_calls: Arc<AtomicUsize>,
        base_value_calls: Arc<AtomicUsize>,
    }

    #[derive(Clone)]
    struct EndpointFailingInstrument {
        id: String,
        attributes: Attributes,
        fail_as_of: Date,
    }

    finstack_quant_valuations::impl_empty_cashflow_provider!(
        ConfigRequiredInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );
    finstack_quant_valuations::impl_empty_cashflow_provider!(
        EndpointFailingInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl Instrument for ConfigRequiredInstrument {
        /// Test mock: reads no market data.
        fn market_dependencies(
            &self,
        ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
        {
            Ok(finstack_quant_valuations::instruments::MarketDependencies::new())
        }

        fn id(&self) -> &str {
            "CONFIG_REQUIRED"
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::Basket
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn attributes(&self) -> &Attributes {
            &self.attributes
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attributes
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn base_value(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            self.base_value_calls.fetch_add(1, Ordering::SeqCst);
            Ok(Money::from((100_i64, Currency::USD)))
        }

        fn price_with_metrics(
            &self,
            _market: &MarketContext,
            as_of: Date,
            _metrics: &[MetricId],
            options: PricingOptions,
        ) -> finstack_quant_valuations::Result<ValuationResult> {
            if _metrics.is_empty() {
                self.pv_only_calls.fetch_add(1, Ordering::SeqCst);
            } else {
                self.metric_calls.fetch_add(1, Ordering::SeqCst);
            }
            let config = options.config.ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "attribution pricing did not receive FinstackConfig".to_string(),
                )
            })?;
            if config.rounding.output_scale.overrides.get(&Currency::USD) != Some(&4) {
                return Err(finstack_quant_core::Error::Validation(
                    "attribution pricing received the wrong FinstackConfig".to_string(),
                )
                .into());
            }
            Ok(ValuationResult::stamped_with_config(
                self.id(),
                as_of,
                Money::from((100_i64, Currency::USD)),
                config.as_ref(),
            ))
        }
    }

    impl Instrument for EndpointFailingInstrument {
        /// Test mock: reads no market data.
        fn market_dependencies(
            &self,
        ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
        {
            Ok(finstack_quant_valuations::instruments::MarketDependencies::new())
        }

        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::Basket
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn attributes(&self) -> &Attributes {
            &self.attributes
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attributes
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn base_value(
            &self,
            _market: &MarketContext,
            as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            if as_of == self.fail_as_of {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{} failed at the closing endpoint",
                    self.id
                )));
            }
            Ok(Money::from((100_i64, Currency::USD)))
        }

        fn price_with_metrics(
            &self,
            market: &MarketContext,
            as_of: Date,
            _metrics: &[MetricId],
            _options: PricingOptions,
        ) -> finstack_quant_valuations::Result<ValuationResult> {
            Ok(ValuationResult::stamped(
                self.id(),
                as_of,
                self.base_value(market, as_of)?,
            ))
        }
    }

    #[test]
    fn factor_bucket_indices_are_unique_and_cover_n_buckets() {
        let buckets = [
            FactorBucket::TotalPnl,
            FactorBucket::Carry,
            FactorBucket::RatesCurvesPnl,
            FactorBucket::CreditCurvesPnl,
            FactorBucket::InflationCurvesPnl,
            FactorBucket::CorrelationsPnl,
            FactorBucket::FxPnl,
            FactorBucket::FxTranslationPnl,
            FactorBucket::CrossFactorPnl,
            FactorBucket::VolPnl,
            FactorBucket::ModelParamsPnl,
            FactorBucket::MarketScalarsPnl,
            FactorBucket::Residual,
        ];
        assert_eq!(buckets.len(), N_BUCKETS);
        let mut seen = [false; N_BUCKETS];
        for b in buckets {
            let idx = b as usize;
            assert!(!seen[idx], "duplicate index {idx}");
            seen[idx] = true;
        }
        assert!(seen.iter().all(|&v| v), "gap in bucket indices");
    }

    fn sample_position_attr(
        position_id: &str,
        total: f64,
        carry: f64,
        residual: f64,
    ) -> PnlAttribution {
        let mut attr = PnlAttribution::new(
            Money::new(total, Currency::USD).expect("valid money fixture"),
            position_id,
            date!(2026 - 01 - 02),
            date!(2026 - 01 - 03),
            AttributionMethod::Parallel,
        );
        attr.carry = Money::new(carry, Currency::USD).expect("valid money fixture");
        attr.rates_curves_pnl =
            Money::new(total - carry - residual, Currency::USD).expect("valid money fixture");
        attr.residual = Money::new(residual, Currency::USD).expect("valid money fixture");
        attr
    }

    #[test]
    fn test_default_metrics_nonempty() {
        assert!(!default_attribution_metrics().is_empty());
    }

    #[test]
    fn metrics_based_attribution_forwards_caller_config() {
        let pv_only_calls = Arc::new(AtomicUsize::new(0));
        let metric_calls = Arc::new(AtomicUsize::new(0));
        let base_value_calls = Arc::new(AtomicUsize::new(0));
        let instrument = Arc::new(ConfigRequiredInstrument {
            attributes: Attributes::new(),
            pv_only_calls: Arc::clone(&pv_only_calls),
            metric_calls: Arc::clone(&metric_calls),
            base_value_calls: Arc::clone(&base_value_calls),
        });
        let position = crate::position::Position::new(
            "P_CONFIG",
            "E_CONFIG",
            "CONFIG_REQUIRED",
            instrument,
            1.0,
            crate::position::PositionUnit::Units,
        )
        .expect("position");
        let mut config = FinstackConfig::default();
        config
            .rounding
            .output_scale
            .overrides
            .insert(Currency::USD, 4);

        let portfolio = Portfolio::builder("CONFIG_PORTFOLIO")
            .base_currency(Currency::USD)
            .as_of(date!(2026 - 01 - 02))
            .entity(crate::types::Entity::new("E_CONFIG"))
            .position(position)
            .build()
            .expect("portfolio");

        attribute_portfolio_pnl(
            &portfolio,
            &MarketContext::new(),
            &MarketContext::new(),
            date!(2026 - 01 - 02),
            date!(2026 - 01 - 03),
            &config,
            AttributionMethod::MetricsBased,
        )
        .expect("metrics-based attribution must forward config");

        assert_eq!(
            metric_calls.load(Ordering::SeqCst),
            2,
            "prepared attribution must value each endpoint exactly once"
        );
        assert_eq!(pv_only_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            base_value_calls.load(Ordering::SeqCst),
            0,
            "prepared metrics attribution must not reprice T0 through value()"
        );
    }

    fn assert_same_financial_decomposition(actual: &PnlAttribution, expected: &PnlAttribution) {
        assert_eq!(actual.total_pnl, expected.total_pnl);
        assert_eq!(actual.carry, expected.carry);
        assert_eq!(actual.rates_curves_pnl, expected.rates_curves_pnl);
        assert_eq!(actual.credit_curves_pnl, expected.credit_curves_pnl);
        assert_eq!(actual.inflation_curves_pnl, expected.inflation_curves_pnl);
        assert_eq!(actual.correlations_pnl, expected.correlations_pnl);
        assert_eq!(actual.fx_pnl, expected.fx_pnl);
        assert_eq!(actual.cross_factor_pnl, expected.cross_factor_pnl);
        assert_eq!(actual.vol_pnl, expected.vol_pnl);
        assert_eq!(actual.model_params_pnl, expected.model_params_pnl);
        assert_eq!(actual.market_scalars_pnl, expected.market_scalars_pnl);
        assert_eq!(actual.residual, expected.residual);
        assert_eq!(actual.result_invalid, expected.result_invalid);
    }

    #[test]
    fn repricing_methods_prepare_endpoints_once_and_preserve_decomposition() {
        let as_of_t0 = date!(2026 - 01 - 02);
        let as_of_t1 = date!(2026 - 01 - 03);
        let methods = [
            AttributionMethod::Parallel,
            AttributionMethod::Waterfall(finstack_quant_attribution::default_waterfall_order()),
            AttributionMethod::Taylor(TaylorAttributionConfig::default()),
        ];

        for method in methods {
            let pv_only_calls = Arc::new(AtomicUsize::new(0));
            let metric_calls = Arc::new(AtomicUsize::new(0));
            let base_value_calls = Arc::new(AtomicUsize::new(0));
            let instrument: Arc<dyn Instrument> = Arc::new(ConfigRequiredInstrument {
                attributes: Attributes::new(),
                pv_only_calls: Arc::clone(&pv_only_calls),
                metric_calls,
                base_value_calls,
            });
            let position = crate::position::Position::new(
                "P_CONFIG",
                "E_CONFIG",
                "CONFIG_REQUIRED",
                Arc::clone(&instrument),
                1.0,
                crate::position::PositionUnit::Units,
            )
            .expect("position");
            let portfolio = Portfolio::builder("CONFIG_PORTFOLIO")
                .base_currency(Currency::USD)
                .as_of(as_of_t0)
                .entity(crate::types::Entity::new("E_CONFIG"))
                .position(position)
                .build()
                .expect("portfolio");
            let mut config = FinstackConfig::default();
            config
                .rounding
                .output_scale
                .overrides
                .insert(Currency::USD, 4);
            let market_t0 = MarketContext::new();
            let market_t1 = MarketContext::new();

            let actual = attribute_portfolio_pnl(
                &portfolio,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &config,
                method.clone(),
            )
            .expect("prepared portfolio attribution");
            assert_eq!(
                pv_only_calls.load(Ordering::SeqCst),
                2,
                "{method} must prepare each ordinary endpoint exactly once"
            );

            let expected = match &method {
                AttributionMethod::Parallel => finstack_quant_attribution::attribute_pnl(
                    &AttributionMethod::Parallel,
                    &finstack_quant_attribution::AttributionRequest {
                        execution_policy: ExecutionPolicy::Serial,
                        ..finstack_quant_attribution::AttributionRequest::new(
                            &instrument,
                            &market_t0,
                            &market_t1,
                            as_of_t0,
                            as_of_t1,
                            &config,
                        )
                    },
                ),
                AttributionMethod::Waterfall(order) => finstack_quant_attribution::attribute_pnl(
                    &AttributionMethod::Waterfall(order.clone()),
                    &finstack_quant_attribution::AttributionRequest {
                        strict_validation: false,
                        model_params_t0: None,
                        ..finstack_quant_attribution::AttributionRequest::new(
                            &instrument,
                            &market_t0,
                            &market_t1,
                            as_of_t0,
                            as_of_t1,
                            &config,
                        )
                    },
                ),
                AttributionMethod::Taylor(taylor_config) => {
                    finstack_quant_attribution::attribute_pnl(
                        &AttributionMethod::Taylor(taylor_config.clone()),
                        &finstack_quant_attribution::AttributionRequest {
                            execution_policy: ExecutionPolicy::Serial,
                            ..finstack_quant_attribution::AttributionRequest::new(
                                &instrument,
                                &market_t0,
                                &market_t1,
                                as_of_t0,
                                as_of_t1,
                                &finstack_quant_core::config::FinstackConfig::default(),
                            )
                        },
                    )
                }
                AttributionMethod::MetricsBased => {
                    unreachable!("test covers repricing methods only")
                }
            }
            .expect("standalone instrument attribution");
            let actual_position = actual
                .by_position
                .get("P_CONFIG")
                .expect("position attribution");
            assert_same_financial_decomposition(actual_position, &expected);
        }
    }

    #[test]
    fn method_owned_attribution_reports_position_errors_in_input_order() {
        let as_of_t0 = date!(2026 - 01 - 02);
        let as_of_t1 = date!(2026 - 01 - 03);
        let mut portfolio_builder = Portfolio::builder("ORDERED_ERRORS")
            .base_currency(Currency::USD)
            .as_of(as_of_t0)
            .entity(crate::types::Entity::new("E"));

        for (position_id, instrument_id) in [("P_FIRST", "I_FIRST"), ("P_SECOND", "I_SECOND")] {
            let instrument = Arc::new(EndpointFailingInstrument {
                id: instrument_id.to_string(),
                attributes: Attributes::new(),
                fail_as_of: as_of_t1,
            });
            let position = crate::position::Position::new(
                position_id,
                "E",
                instrument_id,
                instrument,
                1.0,
                crate::position::PositionUnit::Units,
            )
            .expect("position");
            portfolio_builder = portfolio_builder.position(position);
        }
        let portfolio = portfolio_builder.build().expect("portfolio");

        let error = attribute_portfolio_pnl(
            &portfolio,
            &MarketContext::new(),
            &MarketContext::new(),
            as_of_t0,
            as_of_t1,
            &FinstackConfig::default(),
            AttributionMethod::Parallel,
        )
        .expect_err("both positions fail at T1");
        let message = error.to_string();
        assert!(
            message.contains("P_FIRST"),
            "logical first position must win: {message}"
        );
        assert!(
            !message.contains("P_SECOND"),
            "later position error must not win: {message}"
        );
    }

    #[test]
    fn portfolio_attribution_rejects_missing_cross_factor_pnl() {
        let base_currency = Currency::EUR;
        let zero = Money::from((0_i64, base_currency));
        let attr = PortfolioAttribution {
            total_pnl: Money::from((100_i64, base_currency)),
            carry: zero,
            rates_curves_pnl: zero,
            credit_curves_pnl: zero,
            inflation_curves_pnl: zero,
            correlations_pnl: zero,
            fx_pnl: zero,
            fx_translation_pnl: zero,
            cross_factor_pnl: zero,
            vol_pnl: zero,
            model_params_pnl: zero,
            market_scalars_pnl: zero,
            residual: zero,
            by_position: IndexMap::new(),
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            scalars_detail: None,
            result_invalid: false,
        };
        let mut value = serde_json::to_value(&attr).expect("serialize attribution");
        value
            .as_object_mut()
            .expect("attribution serializes to object")
            .remove("cross_factor_pnl");

        let error = serde_json::from_value::<PortfolioAttribution>(value)
            .expect_err("cross_factor_pnl is required by the canonical contract");
        assert!(error.to_string().contains("cross_factor_pnl"));
    }

    #[test]
    fn test_explain_formats_percentages_and_zero_total_safely() {
        let zero = Money::from((0_i64, Currency::USD));
        let explained = PortfolioAttribution {
            total_pnl: Money::from((200_i64, Currency::USD)),
            carry: Money::from((20_i64, Currency::USD)),
            rates_curves_pnl: Money::from((100_i64, Currency::USD)),
            credit_curves_pnl: Money::from((10_i64, Currency::USD)),
            inflation_curves_pnl: Money::from((5_i64, Currency::USD)),
            correlations_pnl: Money::from((15_i64, Currency::USD)),
            fx_pnl: Money::from((25_i64, Currency::USD)),
            fx_translation_pnl: Money::from((10_i64, Currency::USD)),
            cross_factor_pnl: zero,
            vol_pnl: Money::from((5_i64, Currency::USD)),
            model_params_pnl: Money::from((5_i64, Currency::USD)),
            market_scalars_pnl: Money::from((3_i64, Currency::USD)),
            residual: Money::from((2_i64, Currency::USD)),
            by_position: IndexMap::new(),
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            scalars_detail: None,
            result_invalid: false,
        };
        let rendered = explained.explain();
        assert!(rendered.contains("Portfolio P&L: USD 200.00"));
        assert!(rendered.contains("Carry: USD 20.00 (10.0%)"));
        assert!(rendered.contains("FX Translation: USD 10.00 (5.0%)"));
        assert!(rendered.contains("Residual: USD 2.00 (1.0%)"));

        let zero_total = PortfolioAttribution {
            total_pnl: zero,
            carry: Money::from((5_i64, Currency::USD)),
            rates_curves_pnl: zero,
            credit_curves_pnl: zero,
            inflation_curves_pnl: zero,
            correlations_pnl: zero,
            fx_pnl: zero,
            fx_translation_pnl: zero,
            cross_factor_pnl: zero,
            vol_pnl: zero,
            model_params_pnl: zero,
            market_scalars_pnl: zero,
            residual: Money::from((-5_i64, Currency::USD)),
            by_position: IndexMap::new(),
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            scalars_detail: None,
            result_invalid: false,
        };
        let zero_rendered = zero_total.explain();
        assert!(zero_rendered.contains("Carry: USD 5.00 (0.0%)"));
        assert!(zero_rendered.contains("Residual: USD -5.00 (0.0%)"));
    }

    #[test]
    fn test_reconciliation_check_passes_for_consistent_attribution() {
        let base_currency = Currency::USD;
        let portfolio_attr = PortfolioAttribution {
            total_pnl: Money::from((200_i64, base_currency)),
            carry: Money::from((20_i64, base_currency)),
            rates_curves_pnl: Money::from((100_i64, base_currency)),
            credit_curves_pnl: Money::from((10_i64, base_currency)),
            inflation_curves_pnl: Money::from((5_i64, base_currency)),
            correlations_pnl: Money::from((15_i64, base_currency)),
            fx_pnl: Money::from((25_i64, base_currency)),
            fx_translation_pnl: Money::from((10_i64, base_currency)),
            cross_factor_pnl: Money::from((0_i64, base_currency)),
            vol_pnl: Money::from((5_i64, base_currency)),
            model_params_pnl: Money::from((5_i64, base_currency)),
            market_scalars_pnl: Money::from((3_i64, base_currency)),
            residual: Money::from((2_i64, base_currency)),
            by_position: IndexMap::new(),
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            scalars_detail: None,
            result_invalid: false,
        };

        let report = portfolio_attr.reconciliation_check(0.01);
        assert!(
            report.is_reconciled,
            "expected reconciliation to pass, residual = {}",
            report.total_residual
        );
        assert!(
            report.total_residual.abs() < 1e-10,
            "residual should be ~0, got {}",
            report.total_residual
        );
    }

    #[test]
    fn reconciliation_check_includes_cross_factor_pnl() {
        let base_currency = Currency::USD;
        let zero = Money::from((0_i64, base_currency));
        let portfolio_attr = PortfolioAttribution {
            total_pnl: Money::from((107_i64, base_currency)),
            carry: Money::from((20_i64, base_currency)),
            rates_curves_pnl: Money::from((80_i64, base_currency)),
            credit_curves_pnl: zero,
            inflation_curves_pnl: zero,
            correlations_pnl: zero,
            fx_pnl: zero,
            fx_translation_pnl: zero,
            vol_pnl: zero,
            cross_factor_pnl: Money::from((7_i64, base_currency)),
            model_params_pnl: zero,
            market_scalars_pnl: zero,
            residual: zero,
            by_position: IndexMap::new(),
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            scalars_detail: None,
            result_invalid: false,
        };

        let report = portfolio_attr.reconciliation_check(0.01);
        assert!(report.is_reconciled);
        assert!(report.total_residual.abs() < 1e-10);
    }

    #[test]
    fn test_reconciliation_check_fails_when_totals_mismatch() {
        let base_currency = Currency::USD;
        let zero = Money::from((0_i64, base_currency));
        // total_pnl deliberately mismatches the sum of factor buckets
        let portfolio_attr = PortfolioAttribution {
            total_pnl: Money::from((1000_i64, base_currency)),
            carry: Money::from((100_i64, base_currency)),
            rates_curves_pnl: Money::from((500_i64, base_currency)),
            credit_curves_pnl: zero,
            inflation_curves_pnl: zero,
            correlations_pnl: zero,
            fx_pnl: zero,
            fx_translation_pnl: zero,
            cross_factor_pnl: zero,
            vol_pnl: zero,
            model_params_pnl: zero,
            market_scalars_pnl: zero,
            residual: zero,
            by_position: IndexMap::new(),
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            scalars_detail: None,
            result_invalid: false,
        };

        let report = portfolio_attr.reconciliation_check(0.01);
        assert!(
            !report.is_reconciled,
            "expected reconciliation to fail for mismatched totals"
        );
        assert!(
            (report.total_residual - 400.0).abs() < 1e-10,
            "residual should be 400.0, got {}",
            report.total_residual
        );
    }

    /// Audit (Portfolio): a single invalid constituent must flag the whole
    /// aggregate invalid so downstream reporting can refuse to trust it.
    #[test]
    fn portfolio_aggregation_propagates_position_result_invalid() {
        let good = sample_position_attr("GOOD", 50.0, 5.0, 2.0);
        let mut bad = sample_position_attr("BAD", 100.0, 10.0, 5.0);
        bad.result_invalid = true;

        let identity = |m: Money| -> Result<Money> { Ok(m) };
        let mut acc = FactorAccumulator::new();
        acc.add_converted(&good, &identity)
            .expect("same-currency add");
        acc.add_converted(&bad, &identity)
            .expect("same-currency add");

        let portfolio = acc
            .into_portfolio_attribution(Currency::USD, IndexMap::new())
            .expect("valid into_portfolio_attribution fixture");
        assert!(
            portfolio.result_invalid,
            "one invalid position must flag the whole portfolio invalid"
        );
    }

    /// Audit (Portfolio): `reconciliation_check` must never report a result as
    /// reconciled when `result_invalid` is set, even if the corrupted buckets
    /// happen to net to within tolerance.
    #[test]
    fn reconciliation_check_fails_when_result_invalid_even_if_numbers_net() {
        let base = Currency::USD;
        let zero = Money::from((0_i64, base));
        let attr = PortfolioAttribution {
            total_pnl: Money::from((100_i64, base)),
            carry: Money::from((100_i64, base)),
            rates_curves_pnl: zero,
            credit_curves_pnl: zero,
            inflation_curves_pnl: zero,
            correlations_pnl: zero,
            fx_pnl: zero,
            fx_translation_pnl: zero,
            cross_factor_pnl: zero,
            vol_pnl: zero,
            model_params_pnl: zero,
            market_scalars_pnl: zero,
            residual: zero,
            by_position: IndexMap::new(),
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            scalars_detail: None,
            result_invalid: true,
        };
        // The buckets net exactly to total_pnl, so a numeric-only check would
        // pass — the result_invalid gate must still force is_reconciled false.
        let report = attr.reconciliation_check(0.01);
        assert!(
            !report.is_reconciled,
            "must not report reconciled when result_invalid is set"
        );
    }
}
