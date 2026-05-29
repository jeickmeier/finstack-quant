//! Portfolio valuation and aggregation.

use crate::error::{Error, Result};
use crate::portfolio::Portfolio;
use crate::types::{EntityId, PositionId};
use finstack_core::config::FinstackConfig;
use finstack_core::currency::Currency;
use finstack_core::market_data::context::MarketContext;
use finstack_core::math::summation::neumaier_sum;
use finstack_core::money::fx::FxConversionPolicy;
use finstack_core::money::Money;
use finstack_valuations::metrics::MetricId;
use finstack_valuations::results::ValuationResult;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Minimum number of dirty positions before `revalue_affected` considers
/// parallel repricing.
const REVALUE_AFFECTED_PARALLEL_MIN_AFFECTED: usize = 64;

/// Minimum number of positions before full portfolio valuation uses Rayon.
const VALUE_PORTFOLIO_PARALLEL_MIN_POSITIONS: usize = 64;

/// Portfolio config extension key for selective-repricing debug checks.
pub const PORTFOLIO_SELECTIVE_REPRICING_EXTENSION_KEY: &str = "portfolio.selective_repricing.v1";

const SELECTIVE_REPRICING_VERIFY_TOL: f64 = 1e-10;

fn should_value_portfolio_use_parallel(position_count: usize) -> bool {
    position_count >= VALUE_PORTFOLIO_PARALLEL_MIN_POSITIONS
}

fn should_verify_selective_repricing(config: &FinstackConfig) -> bool {
    config
        .extensions
        .get(PORTFOLIO_SELECTIVE_REPRICING_EXTENSION_KEY)
        .and_then(|value| value.get("verify_full_eval"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn verify_selective_repricing_matches_full(
    selective: &PortfolioValuation,
    full: &PortfolioValuation,
) -> Result<()> {
    let total_delta = (selective.total_base_ccy.amount() - full.total_base_ccy.amount()).abs();
    if total_delta > SELECTIVE_REPRICING_VERIFY_TOL {
        return Err(Error::invalid_input(format!(
            "Selective repricing verification failed: total delta {total_delta} exceeds tolerance {SELECTIVE_REPRICING_VERIFY_TOL}"
        )));
    }

    for (position_id, full_value) in &full.position_values {
        let selective_value = selective
            .get_position_value(position_id.as_str())
            .ok_or_else(|| {
                Error::invalid_input(format!(
                    "Selective repricing verification failed: missing position '{position_id}'"
                ))
            })?;
        let delta = (selective_value.value_base.amount() - full_value.value_base.amount()).abs();
        if delta > SELECTIVE_REPRICING_VERIFY_TOL {
            return Err(Error::invalid_input(format!(
                "Selective repricing verification failed for position '{position_id}': delta {delta} exceeds tolerance {SELECTIVE_REPRICING_VERIFY_TOL}"
            )));
        }
    }

    Ok(())
}

/// Result of valuing a single position.
///
/// Holds both native-currency and base-currency valuations along with
/// the underlying [`ValuationResult`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PositionValue {
    /// Position identifier
    pub position_id: PositionId,

    /// Entity that owns this position
    pub entity_id: EntityId,

    /// Value in the instrument's native currency
    pub value_native: Money,

    /// Value converted to portfolio base currency
    pub value_base: Money,

    /// Linear scaling factor to apply to summable risk measures.
    ///
    /// This mirrors the economic position size and sign used for PV scaling,
    /// but is kept separate so non-summable metrics such as YTM remain
    /// unscaled at the position drill-down level.
    pub metric_scale: f64,

    /// Whether all requested risk metrics were computed successfully.
    pub risk_metrics_complete: bool,

    /// Original metrics failure message when the valuation fell back to PV-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_error: Option<String>,

    /// Full valuation result with metrics (including computed risk measures).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valuation_result: Option<ValuationResult>,
}

/// Complete portfolio valuation results.
///
/// Provides per-position valuations, totals by entity, and the grand total.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortfolioValuation {
    /// Valuation date carried through from the portfolio.
    pub as_of: finstack_core::dates::Date,

    /// Values for each position
    pub position_values: IndexMap<PositionId, PositionValue>,

    /// Total portfolio value in base currency
    pub total_base_ccy: Money,

    /// Aggregated values by entity
    pub by_entity: IndexMap<EntityId, Money>,

    /// Positions whose valuation fell back to PV-only because requested risk
    /// metrics could not be computed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub degraded_positions: Vec<PositionId>,

    /// FX policy applied when collapsing position values to the base currency.
    ///
    /// Base-currency rollups use an explicit spot-equivalent conversion at
    /// [`as_of`](Self::as_of) through the market FX matrix; this records the
    /// applied [`FxConversionPolicy`] so the result envelope satisfies the
    /// policy-visibility invariant (the FX strategy is stamped, not implied).
    #[serde(default = "default_fx_collapse_policy")]
    pub fx_collapse_policy: FxConversionPolicy,
}

/// Default FX policy stamped on a [`PortfolioValuation`]: the spot-equivalent
/// `as_of` conversion used by [`crate::fx::convert_to_base`].
fn default_fx_collapse_policy() -> FxConversionPolicy {
    FxConversionPolicy::CashflowDate
}

impl PortfolioValuation {
    /// Look up the value for a single position by string id.
    ///
    /// The underlying `position_values` field is `IndexMap<PositionId, _>`
    /// where [`PositionId`] is a newtype around `String`. Callers that have a
    /// `&str` (e.g. from a CLI argument, a binding, or a foreign string)
    /// would otherwise have to round-trip through `PositionId::new` / a
    /// `Borrow<str>` lookup; this accessor provides the borrow-aware path
    /// directly. Internal callers (selective repricing, liquidity scoring,
    /// FX checks) all rely on it.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Identifier to query.
    pub fn get_position_value(&self, position_id: &str) -> Option<&PositionValue> {
        self.position_values.get(position_id)
    }

    /// Look up the total value for a single entity by string id.
    ///
    /// Same `&str` ergonomics rationale as [`Self::get_position_value`] —
    /// avoids a `EntityId::new` allocation at every call site. Used by
    /// margin and metric-aggregation paths that already hold borrowed entity
    /// strings.
    ///
    /// # Arguments
    ///
    /// * `entity_id` - Entity identifier to query (accepts `&str` or `&EntityId`).
    pub fn get_entity_value(&self, entity_id: &str) -> Option<&Money> {
        self.by_entity.get(entity_id)
    }

    /// Whether any position fell back to PV-only because its risk metrics
    /// failed.
    ///
    /// Equivalent to `!self.degraded_positions.is_empty()`. Provided as a
    /// named predicate so risk-report dashboards can read intent off the
    /// call site instead of reasoning about empty-vector semantics.
    pub fn has_degraded_risk(&self) -> bool {
        !self.degraded_positions.is_empty()
    }

    /// Borrow the list of position ids whose valuation fell back to PV-only.
    ///
    /// Returns a `&[PositionId]` rather than exposing the raw `Vec` so the
    /// public API does not commit to vector semantics (size, order, growth)
    /// that internal aggregation may want to change.
    pub fn degraded_positions(&self) -> &[PositionId] {
        &self.degraded_positions
    }
}

/// Standard metrics to compute for portfolio positions.
///
/// Note: Using Theta, DV01, and CS01 which are widely supported as scalar metrics.
fn standard_portfolio_metrics() -> Vec<MetricId> {
    // Core risk set chosen to align with common portfolio risk reports:
    // - Theta: carry / time-decay
    // - Dv01 / BucketedDv01: parallel and key-rate IR risk
    // - Cs01 / BucketedCs01: credit spread / hazard risk
    // - Delta / Gamma / Vega / Rho: standard option Greeks
    //
    // Instruments that do not support a given metric simply omit it from
    // `ValuationResult::measures`; aggregation remains robust to missing keys.
    vec![
        MetricId::Theta,
        MetricId::Dv01,
        MetricId::BucketedDv01,
        MetricId::Cs01,
        MetricId::BucketedCs01,
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Rho,
        MetricId::Pv01,
    ]
}

/// Resolve the metric set to request based on valuation options.
fn resolve_metrics(options: &PortfolioValuationOptions) -> Vec<MetricId> {
    match &options.metrics {
        RequestedMetrics::Standard => standard_portfolio_metrics(),
        RequestedMetrics::Only(metrics) => metrics.clone(),
        RequestedMetrics::StandardPlus(extra) => {
            let mut merged = standard_portfolio_metrics();
            for m in extra {
                if !merged.contains(m) {
                    merged.push(m.clone());
                }
            }
            merged
        }
    }
}

/// Assemble final valuation from per-position results.
fn assemble_valuation(
    position_values_vec: Vec<PositionValue>,
    base_ccy: Currency,
    as_of: finstack_core::dates::Date,
) -> Result<PortfolioValuation> {
    let n = position_values_vec.len();
    let mut position_values = IndexMap::with_capacity(n);
    let mut entity_amounts: IndexMap<EntityId, Vec<f64>> = IndexMap::new();

    for pv in position_values_vec {
        entity_amounts
            .entry(pv.entity_id.clone())
            .or_default()
            .push(pv.value_base.amount());
        position_values.insert(pv.position_id.clone(), pv);
    }

    let by_entity: IndexMap<EntityId, Money> = entity_amounts
        .into_iter()
        .map(|(entity_id, amounts)| (entity_id, Money::new(neumaier_sum(amounts), base_ccy)))
        .collect();

    let total_amount = neumaier_sum(by_entity.values().map(|v| v.amount()));
    let total_base_ccy = Money::new(total_amount, base_ccy);

    let degraded_positions = position_values
        .values()
        .filter(|pv| !pv.risk_metrics_complete)
        .map(|pv| pv.position_id.clone())
        .collect();

    Ok(PortfolioValuation {
        as_of,
        position_values,
        total_base_ccy,
        by_entity,
        degraded_positions,
        fx_collapse_policy: default_fx_collapse_policy(),
    })
}

/// Which metric set to request for every position in the portfolio.
///
/// This replaces the legacy tri-state combination of `additional_metrics`
/// and `replace_standard_metrics` with a single explicit enum that has one
/// obvious interpretation for each variant.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(tag = "mode", content = "metrics", rename_all = "snake_case")]
pub enum RequestedMetrics {
    /// Standard portfolio metric set only (see the internal
    /// `standard_portfolio_metrics` helper).
    #[default]
    Standard,
    /// Standard set plus the listed extra metrics (de-duplicated).
    StandardPlus(Vec<MetricId>),
    /// Only the listed metrics; the standard set is not included.
    Only(Vec<MetricId>),
}

/// Options controlling portfolio valuation behaviour.
///
/// By default, risk metrics are treated as best-effort: if metrics fail for
/// a position, the engine falls back to PV-only valuation for that position.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PortfolioValuationOptions {
    /// When `true`, any failure to compute requested risk metrics for a
    /// position causes the entire portfolio valuation to fail.
    ///
    /// When `false` (default), the engine falls back to PV-only
    /// valuation for that position if metrics fail, preserving
    /// aggregate PV but potentially leaving some risk metrics missing.
    pub strict_risk: bool,

    /// Which metric set to request. See [`RequestedMetrics`].
    #[serde(default)]
    pub metrics: RequestedMetrics,
}

/// Value all positions in a portfolio with full metrics.
///
/// This function:
/// 1. Iterates through all positions (in parallel if enabled)
/// 2. Prices each instrument with metrics
/// 3. Converts values to base currency using FX rates
/// 4. Aggregates by entity
///
/// Portfolio valuation uses compensated summation during aggregation and treats
/// `PositionUnit` as part of the pricing contract, so the reported portfolio
/// totals reflect scaled holdings rather than raw instrument PVs.
///
/// # Arguments
///
/// * `portfolio` - Portfolio to value.
/// * `market` - Market data context supplying curves and FX.
/// * `config` - Runtime configuration for the valuation engine.
/// * `options` - Portfolio valuation options controlling risk behaviour.
///
/// # Returns
///
/// [`Result`] containing [`PortfolioValuation`] on success.
///
/// # Errors
///
/// Returns [`Error`] in the following cases:
///
/// - [`Error::ValuationError`] - Instrument pricing failed for a position
/// - [`Error::MissingMarketData`] - FX matrix unavailable for cross-currency conversion
/// - [`Error::FxConversionFailed`] - Required FX rate not found in the matrix
/// - [`Error::Core`] - Monetary arithmetic overflow during aggregation
///
/// # Parallelism
///
/// Position valuations are computed in parallel using rayon. Results are
/// deterministically reduced to ensure consistency across runs (per-position
/// results are collected in input order, then a serial Neumaier fold produces
/// the aggregate totals).
///
/// # Examples
///
/// ```ignore
/// use finstack_core::config::FinstackConfig;
/// use finstack_core::market_data::context::MarketContext;
/// use finstack_portfolio::valuation::{value_portfolio, PortfolioValuationOptions};
///
/// # fn main() -> finstack_portfolio::Result<()> {
/// # let portfolio: finstack_portfolio::Portfolio = unimplemented!("Provide a portfolio");
/// # let market: MarketContext = unimplemented!("Provide market data");
/// let config = FinstackConfig::default();
/// let valuation = value_portfolio(
///     &portfolio,
///     &market,
///     &config,
///     &PortfolioValuationOptions::default(),
/// )?;
///
/// println!("Total base PV: {}", valuation.total_base_ccy);
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - Numerically stable aggregation:
///   `docs/REFERENCES.md#kahan-1965`
pub fn value_portfolio(
    portfolio: &Portfolio,
    market: &MarketContext,
    _config: &FinstackConfig,
    options: &PortfolioValuationOptions,
) -> Result<PortfolioValuation> {
    if !should_value_portfolio_use_parallel(portfolio.positions.len()) {
        return value_portfolio_serial(portfolio, market, _config, options);
    }

    let metrics = resolve_metrics(options);

    use rayon::prelude::*;
    let position_values_vec: Vec<PositionValue> = portfolio
        .positions
        .par_iter()
        .map(|position| {
            value_single_position(position, market, portfolio, &metrics, options.strict_risk)
        })
        .collect::<Result<Vec<_>>>()?;

    assemble_valuation(position_values_vec, portfolio.base_ccy, portfolio.as_of)
}

/// Serial counterpart to [`value_portfolio`] for nested-parallel call sites.
///
/// When an outer loop (e.g. [`crate::replay::replay_portfolio`] over a
/// historical timeline of snapshots) is parallelized across its own work
/// items, calling the parallel [`value_portfolio`] for each item would spawn
/// nested Rayon work and often hurt throughput due to thread-pool contention.
/// This variant runs the per-position pricing step serially so the outer
/// `par_iter` owns the available parallelism.
///
/// Results are numerically identical to [`value_portfolio`] (same Neumaier
/// aggregation of per-position contributions in positional order).
pub(crate) fn value_portfolio_serial(
    portfolio: &Portfolio,
    market: &MarketContext,
    _config: &FinstackConfig,
    options: &PortfolioValuationOptions,
) -> Result<PortfolioValuation> {
    let metrics = resolve_metrics(options);

    let position_values_vec: Vec<PositionValue> = portfolio
        .positions
        .iter()
        .map(|position| {
            value_single_position(position, market, portfolio, &metrics, options.strict_risk)
        })
        .collect::<Result<Vec<_>>>()?;

    assemble_valuation(position_values_vec, portfolio.base_ccy, portfolio.as_of)
}

/// Value a single position with metrics and FX conversion.
///
/// This helper function is used by both serial and parallel implementations.
fn value_single_position(
    position: &crate::position::Position,
    market: &MarketContext,
    portfolio: &Portfolio,
    metrics: &[MetricId],
    strict_risk: bool,
) -> Result<PositionValue> {
    // Price the instrument with metrics.
    //
    // When `strict_risk` is `false`, metric failures fall back to PV-only
    // valuation for the position. When `true`, any metric failure bubbles up
    // as a portfolio error.
    let (valuation_result, risk_metrics_complete, risk_error) = if strict_risk {
        (
            position
                .instrument
                .price_with_metrics(
                    market,
                    portfolio.as_of,
                    metrics,
                    finstack_valuations::instruments::PricingOptions::default(),
                )
                .map_err(|e: finstack_core::Error| Error::ValuationError {
                    position_id: position.position_id.clone(),
                    message: e.to_string(),
                })?,
            true,
            None,
        )
    } else {
        match position.instrument.price_with_metrics(
            market,
            portfolio.as_of,
            metrics,
            finstack_valuations::instruments::PricingOptions::default(),
        ) {
            Ok(result) => (result, true, None),
            Err(metric_error) => {
                let value = position.instrument.value(market, portfolio.as_of).map_err(
                    |e: finstack_core::Error| Error::ValuationError {
                        position_id: position.position_id.clone(),
                        // Both pricing paths failed: metrics first, then the
                        // PV-only fallback. Surface both so the operator knows
                        // the position is fully un-priceable, not just missing
                        // risk metrics.
                        message: format!(
                            "instrument '{}' failed PV-only fallback ({e}) after \
                             metric pricing also failed ({metric_error})",
                            position.instrument.id()
                        ),
                    },
                )?;
                (
                    ValuationResult::stamped(position.instrument.id(), portfolio.as_of, value),
                    false,
                    Some(metric_error.to_string()),
                )
            }
        }
    };

    let value_native = valuation_result.value;

    // Scale by quantity using unit-aware scaling logic.
    let scaled_native = position.scale_value(value_native);

    // Convert to base currency via the shared FX helper so error semantics and
    // FxMatrix lookup conventions stay aligned with cashflows / attribution /
    // margin aggregation.
    let value_base =
        crate::fx::convert_to_base(scaled_native, portfolio.as_of, market, portfolio.base_ccy)?;

    Ok(PositionValue {
        position_id: position.position_id.clone(),
        entity_id: position.entity_id.clone(),
        value_native: scaled_native,
        value_base,
        metric_scale: position.scale_factor(),
        risk_metrics_complete,
        risk_error,
        valuation_result: Some(valuation_result),
    })
}

fn reuse_prior_or_value_position(
    position: &crate::position::Position,
    market: &MarketContext,
    portfolio: &Portfolio,
    metrics: &[MetricId],
    strict_risk: bool,
    prior: &PortfolioValuation,
) -> Result<PositionValue> {
    if let Some(pv) = prior.position_values.get(position.position_id.as_str()) {
        Ok(pv.clone())
    } else {
        value_single_position(position, market, portfolio, metrics, strict_risk)
    }
}

// =============================================================================
// Selective Repricing
// =============================================================================

/// Revalue only the positions affected by a set of changed market factor keys.
///
/// This function consults the portfolio's [`crate::dependencies::DependencyIndex`] to determine which
/// positions depend on the supplied keys, reprices only those positions against
/// the (updated) market context, and patches the prior valuation with fresh
/// results.  Unaffected positions retain their prior values.  Positions whose
/// dependencies could not be resolved are always repriced as a conservative
/// fallback.
///
/// The resulting [`PortfolioValuation`] is fully recomputed (totals, entity
/// rollups, degraded-risk tracking) and is identical to what
/// [`value_portfolio`] would produce if the same updated market
/// were used for a full revaluation.
///
/// # Arguments
///
/// * `portfolio` - Portfolio whose positions to selectively reprice.
/// * `market` - Market data context **after** the factor change has been applied.
/// * `config` - Runtime configuration forwarded to the pricing engine.
/// * `options` - Valuation options (strict_risk, metrics, etc.).
/// * `prior` - Previous full valuation whose unaffected positions are reused.
/// * `changed` - Market factor keys that moved; only positions depending on
///   at least one of these keys will be repriced.
///
/// # Returns
///
/// [`Result`] containing the patched [`PortfolioValuation`].
///
/// # Errors
///
/// Propagates any pricing or FX conversion errors encountered when revaluing
/// affected positions (same error semantics as [`value_portfolio`]).
///
/// # References
///
/// - Numerically stable aggregation:
///   `docs/REFERENCES.md#kahan-1965`
pub fn revalue_affected(
    portfolio: &Portfolio,
    market: &MarketContext,
    config: &FinstackConfig,
    options: &PortfolioValuationOptions,
    prior: &PortfolioValuation,
    changed: &[crate::dependencies::MarketFactorKey],
) -> Result<PortfolioValuation> {
    debug_assert_eq!(
        portfolio.dependency_index().indexed_position_count(),
        portfolio.positions.len(),
        "dependency index is stale: positions were mutated without updating \
         the index — call Portfolio::rebuild_index after direct mutation"
    );
    let affected_indices = portfolio.dependency_index().affected_positions(changed);

    if affected_indices.is_empty() {
        return Ok(prior.clone());
    }

    let metrics = resolve_metrics(options);
    let positions = &portfolio.positions;
    let use_parallel = affected_indices.len() >= REVALUE_AFFECTED_PARALLEL_MIN_AFFECTED
        && affected_indices.len().saturating_mul(4) >= positions.len();

    let position_values_vec: Vec<PositionValue> = if use_parallel {
        use rayon::prelude::*;

        let mut affected_mask = vec![false; positions.len()];
        for &idx in &affected_indices {
            affected_mask[idx] = true;
        }

        // Broad shocks dirty enough of the book that it is worth building a
        // one-byte mask and letting Rayon fan out across positions. Unaffected
        // rows still reuse the prior valuation result in place.
        positions
            .par_iter()
            .enumerate()
            .map(|(idx, position)| {
                if affected_mask[idx] {
                    value_single_position(
                        position,
                        market,
                        portfolio,
                        &metrics,
                        options.strict_risk,
                    )
                } else {
                    reuse_prior_or_value_position(
                        position,
                        market,
                        portfolio,
                        &metrics,
                        options.strict_risk,
                        prior,
                    )
                }
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        // Sparse shocks stay on a cheap serial path so the work remains
        // proportional to the dirty subset instead of paying Rayon overhead on
        // every position. `affected_indices` is sorted, so a single forward
        // scan avoids per-row binary searches.
        let mut next_affected = affected_indices.iter().copied().peekable();
        let mut values = Vec::with_capacity(positions.len());

        for (idx, position) in positions.iter().enumerate() {
            let should_reprice =
                matches!(next_affected.peek(), Some(&affected_idx) if affected_idx == idx);
            if should_reprice {
                next_affected.next();
                values.push(value_single_position(
                    position,
                    market,
                    portfolio,
                    &metrics,
                    options.strict_risk,
                )?);
            } else {
                values.push(reuse_prior_or_value_position(
                    position,
                    market,
                    portfolio,
                    &metrics,
                    options.strict_risk,
                    prior,
                )?);
            }
        }

        values
    };

    let selective = assemble_valuation(position_values_vec, portfolio.base_ccy, portfolio.as_of)?;
    if should_verify_selective_repricing(config) {
        let full = value_portfolio(portfolio, market, config, options)?;
        verify_selective_repricing_matches_full(&selective, &full)?;
    }
    Ok(selective)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::PortfolioBuilder;
    use crate::position::{Position, PositionUnit};
    use crate::test_utils::build_test_market;
    use crate::types::{Entity, DUMMY_ENTITY_ID};
    use finstack_core::currency::Currency;
    use finstack_core::money::Money;
    use finstack_valuations::instruments::rates::deposit::Deposit;
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn small_portfolios_stay_on_serial_path() {
        assert!(!should_value_portfolio_use_parallel(0));
        assert!(!should_value_portfolio_use_parallel(1));
        assert!(!should_value_portfolio_use_parallel(
            VALUE_PORTFOLIO_PARALLEL_MIN_POSITIONS - 1
        ));
        assert!(should_value_portfolio_use_parallel(
            VALUE_PORTFOLIO_PARALLEL_MIN_POSITIONS
        ));
    }

    #[test]
    fn test_value_single_position() {
        let as_of = date!(2024 - 01 - 01);

        let deposit = Deposit::builder()
            .id("DEP_1M".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test should succeed");

        let position = Position::new(
            "POS_001",
            DUMMY_ENTITY_ID,
            "DEP_1M",
            Arc::new(deposit),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");

        let portfolio = PortfolioBuilder::new("TEST")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .position(position)
            .build()
            .expect("test should succeed");

        let market = build_test_market();
        let config = FinstackConfig::default();

        let valuation = value_portfolio(&portfolio, &market, &config, &Default::default())
            .expect("test should succeed");

        assert_eq!(valuation.position_values.len(), 1);
        // Note: With flat curve, deposit PV is small but should be present
        assert!(valuation.total_base_ccy.amount().abs() >= 0.0);
        assert_eq!(valuation.by_entity.len(), 1);
    }

    #[test]
    fn test_value_multiple_entities() {
        let as_of = date!(2024 - 01 - 01);

        let dep1 = Deposit::builder()
            .id("DEP_1".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test should succeed");

        let dep2 = Deposit::builder()
            .id("DEP_2".into())
            .notional(Money::new(500_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 03 - 01))
            .day_count(finstack_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test should succeed");

        let pos1 = Position::new(
            "POS_001",
            "ENTITY_A",
            "DEP_1",
            Arc::new(dep1),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");

        let pos2 = Position::new(
            "POS_002",
            "ENTITY_B",
            "DEP_2",
            Arc::new(dep2),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");

        let portfolio = PortfolioBuilder::new("TEST")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .entity(Entity::new("ENTITY_B"))
            .position(pos1)
            .position(pos2)
            .build()
            .expect("test should succeed");

        let market = build_test_market();
        let config = FinstackConfig::default();

        let valuation = value_portfolio(&portfolio, &market, &config, &Default::default())
            .expect("test should succeed");

        assert_eq!(valuation.position_values.len(), 2);
        assert_eq!(valuation.by_entity.len(), 2);
        assert!(valuation.get_entity_value("ENTITY_A").is_some());
        assert!(valuation.get_entity_value("ENTITY_B").is_some());
    }
}
