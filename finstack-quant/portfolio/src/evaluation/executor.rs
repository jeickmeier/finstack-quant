//! Canonical position-evaluation kernel.

use super::{EvaluationMetricProfile, EvaluationProfile, RiskFailurePolicy};
use crate::error::{Error, Result};
use crate::position::Position;
use crate::types::{EntityId, PositionId};
use crate::valuation::{PortfolioValuation, PositionValue};
use crate::Portfolio;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::summation::neumaier_sum;
use finstack_quant_core::money::fx::FxConversionPolicy;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::results::ValuationResult;
use indexmap::IndexMap;

pub(crate) const POSITION_PARALLEL_MIN_POSITIONS: usize = 64;
const SELECTIVE_PARALLEL_MIN_REPRICES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PositionExecution {
    Auto,
    Serial,
    Parallel,
}

#[derive(Clone, Copy)]
pub(crate) struct SelectiveSeed<'a> {
    pub(crate) prior: &'a PortfolioValuation,
    pub(crate) reprice_indices: &'a [usize],
    pub(crate) refresh_base_currency: bool,
}

pub(crate) struct EvaluationInput<'a> {
    pub(crate) portfolio: &'a Portfolio,
    pub(crate) market: &'a MarketContext,
    pub(crate) as_of: Date,
    pub(crate) config: &'a FinstackConfig,
    pub(crate) profile: &'a EvaluationProfile,
    pub(crate) pricing_options: &'a PricingOptions,
    pub(crate) execution: PositionExecution,
    pub(crate) seed: Option<SelectiveSeed<'a>>,
}

#[derive(Clone, Copy)]
pub(crate) struct RawPositionEndpoint {
    pub(crate) amount: f64,
}

pub(crate) struct RawPortfolioEvaluation {
    endpoints: Vec<RawPositionEndpoint>,
}

impl RawPortfolioEvaluation {
    pub(crate) fn endpoint(&self, index: usize) -> Option<RawPositionEndpoint> {
        self.endpoints.get(index).copied()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct RawSelectiveSeed<'a> {
    pub(crate) prior: &'a RawPortfolioEvaluation,
    pub(crate) reprice_indices: &'a [usize],
}

pub(crate) struct RawEvaluationInput<'a> {
    pub(crate) portfolio: &'a Portfolio,
    pub(crate) market: &'a MarketContext,
    pub(crate) as_of: Date,
    pub(crate) execution: PositionExecution,
    pub(crate) seed: Option<RawSelectiveSeed<'a>>,
}

pub(crate) fn evaluate_portfolio(input: EvaluationInput<'_>) -> Result<PortfolioValuation> {
    let position_count = input.portfolio.positions.len();
    let selective_work = input
        .seed
        .map(|seed| (seed.reprice_indices.len(), seed.refresh_base_currency));
    let execution = resolve_execution(input.execution, position_count, selective_work);

    let results = match execution {
        PositionExecution::Auto | PositionExecution::Serial => evaluate_serial(&input),
        #[cfg(not(target_arch = "wasm32"))]
        PositionExecution::Parallel => evaluate_parallel(&input),
        #[cfg(target_arch = "wasm32")]
        PositionExecution::Parallel => evaluate_serial(&input),
    };
    let position_values = collect_in_logical_order(results)?;
    assemble_valuation(position_values, input.portfolio, input.profile, input.as_of)
}

/// Evaluate unscaled factor-stress endpoints in the portfolio base currency.
///
/// Each instrument is priced native via `value_raw_with_currency`, then
/// converted with the portfolio spot FX helper on **this** market at
/// `as_of`. Callers multiply the stressed-minus-base difference by
/// [`Position::scale_factor`]. This intentionally returns a dedicated `f64`
/// rather than forcing sensitivity endpoints through `Money` or
/// `PortfolioValuation`. It shares the executor's position-axis policy,
/// deterministic ordering, and selective endpoint reuse.
pub(crate) fn evaluate_raw_portfolio(
    input: RawEvaluationInput<'_>,
) -> Result<RawPortfolioEvaluation> {
    let position_count = input.portfolio.positions.len();
    let selective_work = input.seed.map(|seed| (seed.reprice_indices.len(), false));
    let execution = resolve_execution(input.execution, position_count, selective_work);

    #[cfg(target_arch = "wasm32")]
    let _ = execution;

    #[cfg(not(target_arch = "wasm32"))]
    let results = match execution {
        PositionExecution::Auto | PositionExecution::Serial => {
            evaluate_raw_serial(&input, position_count)
        }
        PositionExecution::Parallel => evaluate_raw_parallel(&input, position_count),
    };

    #[cfg(target_arch = "wasm32")]
    let results = evaluate_raw_serial(&input, position_count);

    Ok(RawPortfolioEvaluation {
        endpoints: collect_in_logical_order(results)?,
    })
}

fn resolve_execution(
    requested: PositionExecution,
    position_count: usize,
    selective_work: Option<(usize, bool)>,
) -> PositionExecution {
    if requested != PositionExecution::Auto {
        return requested;
    }

    if let Some((reprice_count, refresh_base_currency)) = selective_work {
        let work_count = if refresh_base_currency {
            position_count
        } else {
            reprice_count
        };
        if work_count >= SELECTIVE_PARALLEL_MIN_REPRICES
            && work_count.saturating_mul(4) >= position_count
        {
            PositionExecution::Parallel
        } else {
            PositionExecution::Serial
        }
    } else if position_count >= POSITION_PARALLEL_MIN_POSITIONS {
        PositionExecution::Parallel
    } else {
        PositionExecution::Serial
    }
}

fn evaluate_raw_serial(
    input: &RawEvaluationInput<'_>,
    position_count: usize,
) -> Vec<Result<RawPositionEndpoint>> {
    let mut next_reprice = input
        .seed
        .map(|seed| seed.reprice_indices.iter().copied().peekable());
    let mut endpoints = Vec::with_capacity(position_count);
    for (index, position) in input.portfolio.positions.iter().enumerate() {
        let should_reprice = next_reprice
            .as_mut()
            .is_none_or(|indices| matches!(indices.peek(), Some(&dirty) if dirty == index));
        if should_reprice {
            if let Some(indices) = next_reprice.as_mut() {
                indices.next();
            }
            endpoints.push(raw_position_endpoint(input, position));
        } else {
            endpoints.push(reuse_raw_position(input, index, position));
        }
    }
    endpoints
}

#[cfg(not(target_arch = "wasm32"))]
fn evaluate_raw_parallel(
    input: &RawEvaluationInput<'_>,
    position_count: usize,
) -> Vec<Result<RawPositionEndpoint>> {
    use rayon::prelude::*;

    let dirty_mask = input.seed.map(|seed| {
        let mut mask = vec![false; position_count];
        for index in seed.reprice_indices {
            if let Some(entry) = mask.get_mut(*index) {
                *entry = true;
            }
        }
        mask
    });
    input
        .portfolio
        .positions
        .par_iter()
        .enumerate()
        .map(|(index, position)| {
            if dirty_mask
                .as_ref()
                .is_none_or(|mask| mask.get(index).copied().unwrap_or(true))
            {
                raw_position_endpoint(input, position)
            } else {
                reuse_raw_position(input, index, position)
            }
        })
        .collect()
}

fn evaluate_serial(input: &EvaluationInput<'_>) -> Vec<Result<PositionValue>> {
    let mut next_reprice = input
        .seed
        .map(|seed| seed.reprice_indices.iter().copied().peekable());
    let mut values = Vec::with_capacity(input.portfolio.positions.len());

    for (index, position) in input.portfolio.positions.iter().enumerate() {
        let should_reprice = next_reprice
            .as_mut()
            .is_none_or(|indices| matches!(indices.peek(), Some(&dirty) if dirty == index));
        if should_reprice {
            if let Some(indices) = next_reprice.as_mut() {
                indices.next();
            }
            values.push(value_position(input, position));
        } else {
            values.push(reuse_position(input, index, position));
        }
    }

    values
}

#[cfg(not(target_arch = "wasm32"))]
fn evaluate_parallel(input: &EvaluationInput<'_>) -> Vec<Result<PositionValue>> {
    use rayon::prelude::*;

    let dirty_mask = input.seed.map(|seed| {
        let mut mask = vec![false; input.portfolio.positions.len()];
        for index in seed.reprice_indices {
            if let Some(entry) = mask.get_mut(*index) {
                *entry = true;
            }
        }
        mask
    });

    input
        .portfolio
        .positions
        .par_iter()
        .enumerate()
        .map(|(index, position)| {
            if dirty_mask
                .as_ref()
                .is_none_or(|mask| mask.get(index).copied().unwrap_or(true))
            {
                value_position(input, position)
            } else {
                reuse_position(input, index, position)
            }
        })
        .collect()
}

fn collect_in_logical_order<T>(results: Vec<Result<T>>) -> Result<Vec<T>> {
    let mut values = Vec::with_capacity(results.len());
    for result in results {
        values.push(result?);
    }
    Ok(values)
}

/// Value one position under the evaluation profile's metric and risk policy.
///
/// The returned `(risk_metrics_complete, risk_error)` pair distinguishes two
/// different fallbacks:
///
/// - **Metrics fallback** (metrics were requested and failed):
///   `risk_metrics_complete = false` and `risk_error = Some(..)`. The position
///   is degraded and is reported on
///   [`PortfolioValuation::degraded_positions`](crate::valuation::PortfolioValuation::degraded_positions).
/// - **PV-only fallback** (no metrics were requested and the canonical pricer
///   failed): `risk_metrics_complete = true` — nothing risk-related is
///   missing — but `risk_error = Some(..)` records the canonical pricing
///   failure so the switch of pricing path is auditable.
///
/// Neither covers the profile's metric menu being narrowed to what this
/// position's instrument type supports. That is structural, not a failure, and
/// is reported on
/// [`PositionValue::inapplicable_metrics`](crate::valuation::PositionValue::inapplicable_metrics).
fn value_position(input: &EvaluationInput<'_>, position: &Position) -> Result<PositionValue> {
    let PositionMetricRequest {
        requested,
        inapplicable_metrics,
    } = requested_metrics(input, position);
    let metrics_requested = !requested.is_empty();
    let (valuation_result, risk_metrics_complete, risk_error) =
        match position.instrument.price_with_metrics(
            input.market,
            input.as_of,
            &requested,
            input.pricing_options.clone(),
        ) {
            Ok(result) => (result, true, None),
            Err(error) if input.profile.risk_policy == RiskFailurePolicy::Strict => {
                return Err(Error::ValuationError {
                    position_id: position.position_id.clone(),
                    message: error.to_string(),
                });
            }
            Err(pricing_error) => {
                let failed_path = if metrics_requested {
                    "metric pricing"
                } else {
                    "canonical pricing"
                };
                let value = position
                    .instrument
                    .value(input.market, input.as_of)
                    .map_err(|error| Error::ValuationError {
                        position_id: position.position_id.clone(),
                        message: format!(
                            "instrument '{}' failed PV-only fallback ({error}) after \
                         {failed_path} also failed ({pricing_error})",
                            position.instrument.id()
                        ),
                    })?;
                // When metrics were requested, the position is degraded: something
                // risk-related is missing. When none were, nothing risk-related is
                // missing, but the position did silently switch pricing paths, so
                // the canonical failure is recorded to keep the switch auditable.
                (
                    ValuationResult::stamped_with_config(
                        position.instrument.id(),
                        input.as_of,
                        value,
                        input.config,
                    ),
                    !metrics_requested,
                    Some(pricing_error.to_string()),
                )
            }
        };

    let value_native = position.scale_value(valuation_result.value)?;
    let value_base = collapse_to_base(input, value_native)?;

    Ok(PositionValue {
        position_id: position.position_id.clone(),
        entity_id: position.entity_id.clone(),
        value_native,
        value_base,
        metric_scale: position.scale_factor(),
        risk_metrics_complete,
        risk_error,
        inapplicable_metrics,
        valuation_result: Some(valuation_result),
    })
}

/// One position's share of an [`EvaluationProfile`]'s metric menu.
struct PositionMetricRequest {
    /// Menu entries this position's instrument type supports, in menu order.
    requested: Vec<MetricId>,
    /// Menu entries it does not support, in menu order. Surfaced on
    /// [`PositionValue::inapplicable_metrics`] so the narrowing is reported
    /// rather than silent.
    inapplicable_metrics: Vec<MetricId>,
}

/// Split an evaluation profile's metric menu for one position.
///
/// A portfolio metric list spans a heterogeneous book, so it is a menu: each
/// position asks for the part its own instrument type has a calculator for.
/// Narrowing portfolio-wide instead would drop metrics that other positions do
/// support, and requesting the whole menu from every position would fail the
/// valuation on the first instrument type that lacks one.
///
/// # Arguments
///
/// * `input` - Evaluation input supplying the profile whose metric menu is
///   split and the pricing options whose `metric_registry` (or the shared
///   standard registry when absent) decides applicability.
/// * `position` - Position whose instrument type decides the split; only its
///   instrument's [`Instrument::key`] is consulted.
///
/// # Returns
///
/// The supported and unsupported halves of the menu, both in menu order. A
/// [`EvaluationMetricProfile::PvOnly`] profile yields two empty lists.
fn requested_metrics(input: &EvaluationInput<'_>, position: &Position) -> PositionMetricRequest {
    let EvaluationMetricProfile::Menu(menu) = &input.profile.metrics else {
        return PositionMetricRequest {
            requested: Vec::new(),
            inapplicable_metrics: Vec::new(),
        };
    };
    let requested = supported_metrics(position.instrument.as_ref(), menu, input.pricing_options);
    let inapplicable_metrics = menu
        .iter()
        .filter(|metric| !requested.contains(metric))
        .cloned()
        .collect();
    PositionMetricRequest {
        requested,
        inapplicable_metrics,
    }
}

/// Narrow a metric menu to the entries one instrument supports.
///
/// Pricing rejects any requested metric that has no calculator for the
/// instrument type, which is correct for a list chosen against one instrument
/// and wrong for a cross-instrument menu. Callers holding such a menu narrow
/// here, so dropping the rest is a visible decision at the call site.
///
/// # Arguments
///
/// * `instrument` - Instrument whose registered metric calculators decide the
///   subset; only its [`Instrument::key`] instrument type is consulted.
/// * `menu` - Candidate identifiers, in menu order; the returned subset
///   preserves that order.
/// * `options` - Pricing options whose `metric_registry`, when present,
///   decides applicability; otherwise the shared standard registry is used.
pub(crate) fn supported_metrics(
    instrument: &dyn Instrument,
    menu: &[MetricId],
    options: &PricingOptions,
) -> Vec<MetricId> {
    let registry = match options.metric_registry.as_deref() {
        Some(registry) => registry,
        None => finstack_quant_valuations::metrics::standard_registry(),
    };
    registry.applicable_subset(menu, instrument.key())
}

fn raw_position_endpoint(
    input: &RawEvaluationInput<'_>,
    position: &Position,
) -> Result<RawPositionEndpoint> {
    // Preserve the historical factor-stress error taxonomy: raw pricing
    // failures remain their originating core/input/calibration variants.
    let (amount, currency) = position
        .instrument
        .value_raw_with_currency(input.market, input.as_of)?;
    if !amount.is_finite() {
        return Err(Error::validation(format!(
            "M-1: non-finite factor-stress PV for position '{}' ({amount})",
            position.position_id
        )));
    }
    let amount = crate::fx::convert_to_base(
        Money::new(amount, currency)?,
        input.as_of,
        input.market,
        input.portfolio.base_currency,
    )?
    .amount();
    Ok(RawPositionEndpoint { amount })
}

fn reuse_raw_position(
    input: &RawEvaluationInput<'_>,
    index: usize,
    position: &Position,
) -> Result<RawPositionEndpoint> {
    input
        .seed
        .and_then(|seed| seed.prior.endpoint(index))
        .map_or_else(|| raw_position_endpoint(input, position), Ok)
}

fn reuse_position(
    input: &EvaluationInput<'_>,
    index: usize,
    position: &Position,
) -> Result<PositionValue> {
    let Some(seed) = input.seed else {
        return value_position(input, position);
    };
    let Some((prior_id, prior)) = seed.prior.position_values.get_index(index) else {
        return value_position(input, position);
    };
    if prior_id != &position.position_id || prior.position_id != position.position_id {
        return value_position(input, position);
    }

    let mut reused = prior.clone();
    if seed.refresh_base_currency {
        reused.value_base = collapse_to_base(input, reused.value_native)?;
    }
    Ok(reused)
}

fn collapse_to_base(input: &EvaluationInput<'_>, value_native: Money) -> Result<Money> {
    crate::fx::convert_to_base(
        value_native,
        input.as_of,
        input.market,
        input.portfolio.base_currency,
    )
}

fn assemble_valuation(
    position_values_vec: Vec<PositionValue>,
    portfolio: &Portfolio,
    profile: &EvaluationProfile,
    as_of: Date,
) -> Result<PortfolioValuation> {
    let base_currency = portfolio.base_currency;
    let mut position_values = IndexMap::with_capacity(position_values_vec.len());
    let mut entity_amounts: IndexMap<EntityId, Vec<f64>> = IndexMap::new();

    for value in position_values_vec {
        entity_amounts
            .entry(value.entity_id.clone())
            .or_default()
            .push(value.value_base.amount());
        position_values.insert(value.position_id.clone(), value);
    }

    let by_entity: IndexMap<EntityId, Money> = entity_amounts
        .into_iter()
        .map(|(entity_id, amounts)| {
            Money::new(neumaier_sum(amounts), base_currency).map(|money| (entity_id, money))
        })
        .collect::<finstack_quant_core::Result<_>>()?;
    let total_base_currency = Money::new(
        neumaier_sum(by_entity.values().map(Money::amount)),
        base_currency,
    )?;
    let degraded_positions: Vec<PositionId> = position_values
        .values()
        .filter(|value| !value.risk_metrics_complete)
        .map(|value| value.position_id.clone())
        .collect();

    Ok(PortfolioValuation {
        as_of,
        position_values,
        total_base_currency,
        by_entity,
        degraded_positions,
        fx_collapse_policy: FxConversionPolicy::CashflowDate,
        provenance: Some(super::EvaluationProvenance {
            profile: profile.clone(),
            portfolio_state_id: portfolio.evaluation_state_id,
            base_currency,
        }),
    })
}
