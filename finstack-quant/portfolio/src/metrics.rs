//! Portfolio metrics aggregation.
//!
//! Provides utilities to determine which valuation metrics are summable and
//! to consolidate per-position measures into portfolio-level analytics.
//!
//! # FX Conversion
//!
//! When aggregating risk metrics across a multi-currency portfolio, position-level
//! sensitivities must be converted to the portfolio's base currency before summation.
//! For example, a EUR position's DV01 (reported in EUR) and a USD position's DV01
//! (reported in USD) cannot be meaningfully summed without FX conversion.
//!
//! `aggregate_metrics` converts each summable metric using the market
//! FX-matrix spot at `as_of` when the pair is available. If the matrix (or
//! the pair) is missing and native PV is large enough, it falls back to the
//! rate implied by `value_base / value_native`. Positions with near-zero
//! native PV and no matrix quote fail closed.

use crate::error::{Error, Result};
#[cfg(not(target_arch = "wasm32"))]
use crate::evaluation::POSITION_PARALLEL_MIN_POSITIONS;
use crate::types::{EntityId, PositionId};
use crate::valuation::PortfolioValuation;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::summation::neumaier_sum;
use finstack_quant_core::HashSet;
use finstack_quant_valuations::metrics::MetricId;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Aggregated metric across the portfolio.
///
/// Contains portfolio-wide totals as well as breakdowns by entity.
///
/// # Additive versus non-additive metrics
///
/// Summing currency-denominated rate and credit sensitivities across
/// positions is standard desk practice: `dv01`, `cs01`, `pv01` and their
/// bucketed series all measure a change per basis point of the *same* kind of
/// risk factor, and their sum is a portfolio-level number a risk manager can
/// hedge with (Tuckman & Serrat, *Fixed Income Securities*). `fx_delta` and
/// `index_delta` are likewise currency-denominated and linear in size.
///
/// Unqualified scalar Greeks (`delta`, `gamma`, `vega`, `vanna`, `volga`)
/// are not additive because their risk factors may differ across positions.
/// Portfolio totals require qualified keys such as `delta::<underlying>` or
/// `bucketed_vega::<surface>::<expiry>::<strike>`.
/// Instrument-specific measures such as yield and duration remain in
/// [`PortfolioMetrics::by_position`] and are listed on
/// [`PortfolioMetrics::unaggregated_metrics`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct AggregatedMetric {
    /// Metric identifier
    pub metric_id: String,

    /// Total value across all positions (for summable metrics)
    pub total: f64,

    /// Aggregated values by entity
    pub by_entity: IndexMap<EntityId, f64>,
}

/// Complete portfolio metrics results.
///
/// Holds both aggregated metrics and per-position values returned
/// by `aggregate_metrics`.
///
/// # Completeness of the aggregated totals
///
/// A total in [`aggregated`](Self::aggregated) is not necessarily a sum over
/// every position in the portfolio. Three fields make each omission visible,
/// and consumers that report totals should surface all three:
///
/// - [`degraded_positions`](Self::degraded_positions) — positions that fell
///   back to a PV-only valuation, so they carry **no** risk measures and
///   contribute zero to every total.
/// - [`skipped_metrics`](Self::skipped_metrics) — individual non-finite
///   values excluded from a total.
/// - [`unaggregated_metrics`](Self::unaggregated_metrics) — metrics that
///   exist per position but are not portfolio-summable, so they have no
///   total at all.
///
/// A metric narrowed away because a position's instrument type has no
/// calculator for it is *not* one of these omissions: the model gives that
/// instrument type no such exposure, so the total is complete without it. Such
/// narrowing is reported per position on
/// [`PositionValue::inapplicable_metrics`](crate::valuation::PositionValue::inapplicable_metrics).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct PortfolioMetrics {
    /// Aggregated metrics (summable only)
    pub aggregated: IndexMap<MetricId, AggregatedMetric>,

    /// Raw metrics by position (all metrics), with explicit native currency context.
    pub by_position: IndexMap<PositionId, PositionMetrics>,

    /// Positions whose non-finite metric values were excluded from aggregation.
    ///
    /// Each entry records the position and metric that was skipped, allowing
    /// callers to detect incomplete aggregation (analogous to
    /// [`crate::valuation::PortfolioValuation::degraded_positions`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_metrics: Vec<SkippedMetric>,

    /// Positions that carried no risk measures at all because their valuation
    /// fell back to PV-only.
    ///
    /// Mirrored from
    /// [`PortfolioValuation::degraded_positions`](crate::valuation::PortfolioValuation::degraded_positions)
    /// (positions with `risk_metrics_complete == false`). Such positions
    /// contribute zero to every total without producing a
    /// [`SkippedMetric`] entry — that field only records non-finite values —
    /// so this list is the only signal that the aggregate is partial.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub degraded_positions: Vec<PositionId>,

    /// Metric identifiers present in [`by_position`](Self::by_position) that
    /// were not aggregated into a portfolio total because they are not
    /// summable across positions (for example `ytm`, `duration`, or any
    /// metric outside the additive allowlist).
    ///
    /// Sorted and de-duplicated. The per-position values remain available in
    /// `by_position` in their native currency.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unaggregated_metrics: Vec<String>,
}

/// Position-level metrics with explicit native currency context.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct PositionMetrics {
    /// Native currency for this position's valuation and non-summable metrics.
    pub currency: Currency,
    /// Raw metric values for the position.
    pub metrics: IndexMap<MetricId, f64>,
}

impl std::ops::Deref for PositionMetrics {
    type Target = IndexMap<MetricId, f64>;

    fn deref(&self) -> &Self::Target {
        &self.metrics
    }
}

impl<'a> IntoIterator for &'a PositionMetrics {
    type Item = (&'a MetricId, &'a f64);
    type IntoIter = indexmap::map::Iter<'a, MetricId, f64>;

    fn into_iter(self) -> Self::IntoIter {
        self.metrics.iter()
    }
}

impl PortfolioMetrics {
    /// Get an aggregated metric by identifier.
    ///
    /// # Arguments
    ///
    /// * `metric_id` - Identifier of the metric to look up.
    pub fn get_metric(&self, metric_id: &str) -> Option<&AggregatedMetric> {
        self.aggregated.get(metric_id)
    }

    /// Get metrics for a specific position.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Identifier of the position to query.
    pub fn get_position_metrics(&self, position_id: &str) -> Option<&PositionMetrics> {
        self.by_position.get(position_id)
    }

    /// Get the total value of a specific metric across the portfolio.
    ///
    /// # Arguments
    ///
    /// * `metric_id` - Identifier of the metric.
    pub fn get_total(&self, metric_id: &str) -> Option<f64> {
        self.aggregated.get(metric_id).map(|m| m.total)
    }

    /// Return decoded components and aggregate payloads for a composite metric.
    ///
    /// Entries retain the deterministic insertion order of [`Self::aggregated`].
    /// The scalar aggregate stored directly under `base` is excluded. Typed
    /// canonical metric keys prevent aliases between distinct coordinates.
    ///
    /// # Arguments
    ///
    /// * `base` - Composite metric identifier whose decoded series components
    ///   should be collected from [`Self::aggregated`].
    pub fn metric_series(&self, base: &MetricId) -> Vec<(Vec<String>, &AggregatedMetric)> {
        self.aggregated
            .iter()
            .filter_map(|(key, aggregate)| {
                key.decode_components(base)
                    .map(|components| (components, aggregate))
            })
            .collect()
    }
}

/// A metric value that was excluded from portfolio aggregation because it was
/// non-finite (NaN or ±Inf).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct SkippedMetric {
    /// Position that produced the non-finite value.
    pub position_id: PositionId,
    /// Metric identifier that was skipped.
    pub metric_id: String,
    /// The non-finite value that was encountered.
    pub value: f64,
}

impl Default for PortfolioMetrics {
    fn default() -> Self {
        Self {
            aggregated: IndexMap::new(),
            by_position: IndexMap::new(),
            skipped_metrics: Vec::new(),
            degraded_positions: Vec::new(),
            unaggregated_metrics: Vec::new(),
        }
    }
}

/// Metrics that can be meaningfully summed across positions.
///
/// These metrics scale linearly with position size and can be aggregated.
///
/// Bucketed metrics (e.g. key-rate DV01) are stored in `ValuationResult::measures`
/// using composite keys of the form:
///
/// - `bucketed_dv01::2y`
/// - `bucketed_cs01::5y`
///
/// To support portfolio-level aggregation of these series, `is_summable` performs
/// a prefix match on the base metric ID rather than requiring an exact key match.
///
/// Membership is an explicit allowlist rather than a registry-driven property:
/// a metric is listed only when it is currency-denominated and linear in
/// position size, so that FX conversion plus summation yields a number with a
/// portfolio-level meaning. Metrics that appear per position but are absent
/// here are reported on
/// [`PortfolioMetrics::unaggregated_metrics`] so the omission is visible.
/// Check if a metric can be summed across positions.
///
/// This treats both base IDs (e.g. `bucketed_dv01`) and structured
/// composite keys (e.g. `bucketed_dv01::2y`) as summable so that
/// key-rate / bucketed series aggregate correctly.
///
/// # Arguments
///
/// * `metric_id` - Metric identifier to test.
///
/// # Returns
///
/// `true` when the metric is safe to sum across positions after any required
/// FX conversion.
pub(crate) fn is_summable(metric_id: &str) -> bool {
    metric_id
        .parse::<MetricId>()
        .is_ok_and(|id| finstack_quant_valuations::metrics::is_additive_metric(&id))
}

/// Aggregate metrics from portfolio valuation results.
///
/// Collects position metrics (in parallel when enabled), FX-converts summable
/// metrics to the portfolio base currency, rolls those up by entity and
/// portfolio total, and stores non-summable metrics per position in native
/// currency.
///
/// # FX Conversion
///
/// Position-level risk sensitivities are typically denominated in the instrument's
/// native currency. Before portfolio-level summation, each metric value is
/// multiplied by the implied FX rate from the position's native currency to the
/// portfolio base currency:
///
/// ```text
/// metric_base = metric_native × (value_base / value_native)
/// ```
///
/// For positions where the native PV is zero (or the position is already in
/// base currency), no conversion is applied.
///
/// # Warning: scalar spot and vol Greeks are not summed
///
/// `dv01` / `cs01` / `pv01` / `fx_delta` / `index_delta` totals are
/// portfolio-level numbers. Scalar `delta`, `gamma` and `vega` are excluded
/// from those totals because they mix unrelated underlyings and volatility
/// surfaces; read them per position or via the bucketed series. See
/// [`AggregatedMetric`] for the full discussion.
///
/// # Completeness
///
/// Totals may cover fewer than all positions. Degraded positions (PV-only
/// fallback) are listed on [`PortfolioMetrics::degraded_positions`],
/// individual non-finite values on [`PortfolioMetrics::skipped_metrics`], and
/// non-summable metric names on
/// [`PortfolioMetrics::unaggregated_metrics`].
///
/// # Arguments
///
/// * `valuation` - Portfolio valuation containing per-position valuation results.
/// * `base_currency` - Portfolio base currency for aggregation.
/// * `market` - Market context providing FX rates for zero-PV positions.
/// * `as_of` - Valuation date used for FX rate lookups.
///
/// # Returns
///
/// [`Result`] with a populated [`PortfolioMetrics`] structure.
///
/// # Parallelism
///
/// Per-position metrics are collected in parallel using rayon and then
/// deterministically aggregated with a serial Neumaier fold to ensure
/// consistency across runs.
///
/// # References
///
/// - Fixed-income risk conventions: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
///
/// - Numerically stable aggregation: `docs/REFERENCES.md#kahan-1965`
///
///
/// # Errors
///
/// Returns an error when `base_currency` or `as_of` differs from the valuation,
/// or when an out-of-base position with near-zero native PV cannot obtain an
/// FX conversion rate from `market`. Non-finite summable metric values are
/// recorded as skipped in the result rather than failing aggregation.
pub fn aggregate_metrics(
    valuation: &PortfolioValuation,
    base_currency: Currency,
    market: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
) -> Result<PortfolioMetrics> {
    let valuation_base_currency = valuation.total_base_currency.currency();
    if base_currency != valuation_base_currency {
        return Err(Error::invalid_input(format!(
            "M-17: aggregate_metrics base_currency {base_currency} does not match valuation base currency {valuation_base_currency}"
        )));
    }
    if as_of != valuation.as_of {
        return Err(Error::invalid_input(format!(
            "M-17: aggregate_metrics as_of {as_of} does not match valuation as_of {}",
            valuation.as_of
        )));
    }

    let position_entries: Vec<_> = valuation.position_values.iter().collect();

    let collect_position =
        |(position_id, position_value): (&PositionId, &crate::valuation::PositionValue)| {
            position_value.valuation_result.as_ref().map(|val_result| {
                let metrics: IndexMap<MetricId, f64> = val_result
                    .measures
                    .iter()
                    .map(|(id, v)| {
                        let scaled =
                            scale_position_metric(id.as_str(), *v, position_value.metric_scale);
                        (id.clone(), scaled)
                    })
                    .collect();
                let fx_rate = fx_rate_for_position(position_value, base_currency, market, as_of)?;
                Ok(PositionMetricData {
                    position_id: (*position_id).clone(),
                    entity_id: position_value.entity_id.clone(),
                    currency: position_value.value_native.currency(),
                    metrics,
                    fx_rate,
                })
            })
        };

    #[cfg(not(target_arch = "wasm32"))]
    let collected_results: Vec<Result<PositionMetricData>> =
        if position_entries.len() >= POSITION_PARALLEL_MIN_POSITIONS {
            use rayon::prelude::*;
            position_entries
                .par_iter()
                .filter_map(|(position_id, position_value)| {
                    collect_position((position_id, position_value))
                })
                .collect()
        } else {
            position_entries
                .iter()
                .filter_map(|(position_id, position_value)| {
                    collect_position((position_id, position_value))
                })
                .collect()
        };

    #[cfg(target_arch = "wasm32")]
    let collected_results: Vec<Result<PositionMetricData>> = position_entries
        .iter()
        .filter_map(|(position_id, position_value)| collect_position((position_id, position_value)))
        .collect();

    let collected = collected_results
        .into_iter()
        .collect::<Result<Vec<PositionMetricData>>>()?;

    let mut metrics = aggregate_collected_metrics(collected);
    metrics
        .degraded_positions
        .clone_from(&valuation.degraded_positions);
    Ok(metrics)
}

/// Compute the FX conversion factor from a position's native currency to base currency.
///
/// Uses the market FX-matrix spot when available; falls back to the rate implied
/// by the position's valuation (value_base / value_native) when the matrix or the
/// pair is missing and the native PV is large enough for a reliable ratio.
fn fx_rate_for_position(
    position_value: &crate::valuation::PositionValue,
    base_currency: Currency,
    market: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
) -> Result<f64> {
    let native_currency = position_value.value_native.currency();

    if native_currency == base_currency {
        return Ok(1.0);
    }

    // Prefer the market spot: the PV-implied ratio `value_base / value_native`
    // carries base-currency quantization noise from the valuation step
    // (value_base was rounded to currency decimals), and that noise would
    // scale every summable risk metric (DV01, CS01, deltas).
    if let Ok(rate) = crate::fx::spot_rate_to_base(native_currency, as_of, market, base_currency) {
        return Ok(rate);
    }

    // Fallback: the PV-implied ratio, so aggregation still works from
    // valuation output alone when the matrix (or the pair) is unavailable.
    // A conservative threshold avoids distorted implied rates when native PV
    // is near zero (e.g., expired instruments).
    let native_amount = position_value.value_native.amount();
    if native_amount.abs() > 1e-6 {
        let base_amount = position_value.value_base.amount();
        return Ok(base_amount / native_amount);
    }

    Err(crate::error::Error::FxConversionFailed {
        from: native_currency,
        to: base_currency,
    })
}

fn scale_position_metric(metric_id: &str, value: f64, metric_scale: f64) -> f64 {
    if is_summable(metric_id) {
        value * metric_scale
    } else {
        value
    }
}

/// Collected per-position metric data ready for aggregation.
#[derive(Clone)]
struct PositionMetricData {
    position_id: PositionId,
    entity_id: EntityId,
    currency: Currency,
    metrics: IndexMap<MetricId, f64>,
    fx_rate: f64,
}

/// Aggregate collected position metric data into portfolio metrics.
fn aggregate_collected_metrics(collected: Vec<PositionMetricData>) -> PortfolioMetrics {
    let n = collected.len();
    let mut by_position: IndexMap<PositionId, PositionMetrics> = IndexMap::with_capacity(n);

    // IndexMap keeps aggregated/by_entity insertion order stable for snapshots.
    let mut metric_values: IndexMap<MetricId, Vec<f64>> = IndexMap::new();
    let mut entity_values: IndexMap<MetricId, IndexMap<EntityId, Vec<f64>>> = IndexMap::new();
    let mut skipped_metrics: Vec<SkippedMetric> = Vec::new();
    let mut unaggregated: HashSet<String> = HashSet::default();

    for data in collected {
        let fx_rate = data.fx_rate;
        let entity_id = data.entity_id;

        for (metric_id, value) in &data.metrics {
            let metric_name = metric_id.as_str();
            if !is_summable(metric_name) {
                unaggregated.insert(metric_name.to_string());
            } else {
                if !value.is_finite() {
                    tracing::warn!(
                        metric_id = %metric_name,
                        position_id = %data.position_id,
                        value,
                        "Skipping non-finite metric value"
                    );
                    skipped_metrics.push(SkippedMetric {
                        position_id: data.position_id.clone(),
                        metric_id: metric_name.to_string(),
                        value: *value,
                    });
                    continue;
                }

                let value_base = *value * fx_rate;

                metric_values
                    .entry(metric_id.clone())
                    .or_default()
                    .push(value_base);

                entity_values
                    .entry(metric_id.clone())
                    .or_default()
                    .entry(entity_id.clone())
                    .or_default()
                    .push(value_base);
            }
        }

        by_position.insert(
            data.position_id,
            PositionMetrics {
                currency: data.currency,
                metrics: data.metrics,
            },
        );
    }

    let mut aggregated: IndexMap<MetricId, AggregatedMetric> =
        IndexMap::with_capacity(metric_values.len());

    for (metric_id, values) in metric_values {
        let total = neumaier_sum(values);

        let by_entity: IndexMap<EntityId, f64> = entity_values
            .shift_remove(&metric_id)
            .into_iter()
            .flat_map(|entity_map| {
                entity_map
                    .into_iter()
                    .map(|(eid, vals)| (eid, neumaier_sum(vals)))
            })
            .collect();

        let metric_string = metric_id.to_string();
        aggregated.insert(
            metric_id,
            AggregatedMetric {
                metric_id: metric_string,
                total,
                by_entity,
            },
        );
    }

    let mut unaggregated_metrics: Vec<String> = unaggregated.into_iter().collect();
    unaggregated_metrics.sort_unstable();

    PortfolioMetrics {
        aggregated,
        by_position,
        skipped_metrics,
        degraded_positions: Vec::new(),
        unaggregated_metrics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::PortfolioBuilder;
    use crate::position::{Position, PositionUnit};
    use crate::test_utils::build_test_market;
    use crate::types::Entity;
    use crate::valuation::{value_portfolio, PositionValue};
    use finstack_quant_core::config::FinstackConfig;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, SimpleFxProvider};
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::rates::deposit::Deposit;
    use finstack_quant_valuations::results::ValuationResult;
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn portfolio_metric_maps_reject_noncanonical_wire_keys() {
        let invalid = "bucketed_dv01::USD_x2dOIS::10y";
        let aggregate = serde_json::json!({
            "aggregated": {invalid: {"metric_id": invalid, "total": 1.0, "by_entity": {}}},
            "by_position": {}
        });
        assert!(serde_json::from_value::<PortfolioMetrics>(aggregate).is_err());
        let position = serde_json::json!({
            "aggregated": {},
            "by_position": {"POS": {"currency": "USD", "metrics": {invalid: 1.0}}}
        });
        assert!(serde_json::from_value::<PortfolioMetrics>(position).is_err());
    }

    #[test]
    fn test_is_summable() {
        assert!(is_summable("dv01"));
        assert!(is_summable("cs01"));
        assert!(is_summable("fx_delta"));
        assert!(is_summable("index_delta"));
        assert!(!is_summable("delta"));
        assert!(!is_summable("gamma"));
        assert!(!is_summable("vega"));
        assert!(is_summable("delta::AAPL"));
        assert!(is_summable("gamma::SPX"));
        assert!(is_summable("vega::SPX_VOL"));
        assert!(!is_summable("ytm"));
        assert!(!is_summable("duration"));

        assert!(is_summable("bucketed_dv01::2y"));
        assert!(is_summable("bucketed_cs01::AAA::5y"));
        assert!(is_summable("bucketed_vega::SURFACE::1y"));
        assert!(!is_summable("unknown::2y"));
    }

    #[test]
    fn portfolio_metric_series_decodes_components_in_aggregate_order() {
        let mut aggregated = IndexMap::new();
        for (metric_id, total) in [
            ("bucketed_dv01", -3.0),
            ("bucketed_dv01::USD-OIS::10y", -1.0),
            ("bucketed_cs01::ACME", 8.0),
            ("bucketed_dv01::EUR/USD::", -2.0),
        ] {
            aggregated.insert(
                MetricId::custom(metric_id),
                AggregatedMetric {
                    metric_id: metric_id.to_string(),
                    total,
                    by_entity: IndexMap::new(),
                },
            );
        }
        let metrics = PortfolioMetrics {
            aggregated,
            ..Default::default()
        };

        let series = metrics.metric_series(&MetricId::BucketedDv01);
        assert_eq!(series.len(), 2);
        assert_eq!(series[0].0, vec!["USD-OIS", "10y"]);
        assert_eq!(series[0].1.total, -1.0);
        assert_eq!(series[1].0, vec!["EUR/USD", ""]);
        assert_eq!(series[1].1.total, -2.0);
    }

    #[test]
    fn portfolio_metric_series_preserves_literal_escape_coordinates() {
        let mut aggregated = IndexMap::new();
        for (metric_id, total) in [
            ("bucketed_dv01::curve-ray", -1.0),
            ("bucketed_dv01::curve_x2dray", -2.0),
            ("bucketed_dv01::curve_xray", -3.0),
        ] {
            aggregated.insert(
                MetricId::custom(metric_id),
                AggregatedMetric {
                    metric_id: metric_id.to_string(),
                    total,
                    by_entity: IndexMap::new(),
                },
            );
        }
        let metrics = PortfolioMetrics {
            aggregated,
            ..Default::default()
        };

        assert_eq!(
            metrics
                .metric_series(&MetricId::BucketedDv01)
                .into_iter()
                .map(|(components, metric)| (components, metric.total))
                .collect::<Vec<_>>(),
            vec![
                (vec!["curve-ray".to_string()], -1.0),
                (vec!["curve_x2dray".to_string()], -2.0),
                (vec!["curve_xray".to_string()], -3.0),
            ]
        );
    }

    #[test]
    fn test_aggregate_metrics_basic() {
        let as_of = date!(2024 - 01 - 01);

        let deposit = Deposit::builder()
            .id("DEP_1M".into())
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .start_date(as_of)
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test should succeed");

        let position = Position::new(
            "POS_001",
            "ENTITY_A",
            "DEP_1M",
            Arc::new(deposit),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");

        let portfolio = PortfolioBuilder::new("TEST")
            .base_currency(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");

        let market = build_test_market();
        let config = FinstackConfig::default();

        let valuation = value_portfolio(&portfolio, &market, &config, &Default::default())
            .expect("test should succeed");
        let metrics = aggregate_metrics(&valuation, Currency::USD, &market, as_of)
            .expect("test should succeed");

        assert_eq!(valuation.position_values.len(), 1);
        assert!(
            !metrics.by_position.is_empty(),
            "Should have position metrics"
        );
        let position_metrics = metrics
            .get_position_metrics("POS_001")
            .expect("position metrics should be present");
        assert_eq!(position_metrics.currency, Currency::USD);
    }

    /// Risk metrics must be converted at the market spot, not at the
    /// PV-implied ratio `value_base / value_native`: `value_base` was
    /// quantized to base-currency decimals when the valuation ran, so the
    /// implied ratio carries rounding noise that would scale every summable
    /// metric (DV01, CS01, deltas). Here the implied ratio is
    /// 3.60 / 3.33 ≈ 1.0811 while the true spot is 1.08.
    #[test]
    fn fx_rate_for_position_prefers_matrix_spot_over_pv_implied_ratio() {
        let as_of = date!(2024 - 01 - 01);
        let position_value = PositionValue {
            position_id: PositionId::from("EUR_POS".to_string()),
            entity_id: EntityId::from("ENTITY".to_string()),
            value_native: Money::new(3.33, Currency::EUR).expect("valid money fixture"),
            // Quantized during valuation: 3.33 * 1.08 = 3.5964 -> 3.60 USD.
            value_base: Money::new(3.60, Currency::USD).expect("valid money fixture"),
            metric_scale: 1.0,
            risk_metrics_complete: true,
            risk_error: None,
            inapplicable_metrics: Vec::new(),
            valuation_result: None,
        };

        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quotes(&[(Currency::EUR, Currency::USD, 1.08)])
            .expect("quote should set");
        let market = MarketContext::new().insert_fx(FxMatrix::new(provider));

        let rate = fx_rate_for_position(&position_value, Currency::USD, &market, as_of)
            .expect("rate should resolve");
        assert!(
            (rate - 1.08).abs() < 1e-12,
            "expected the matrix spot 1.08, got the implied ratio {rate}"
        );
    }

    /// Without an FX matrix the PV-implied ratio remains the fallback so
    /// metric aggregation still works on valuation output alone.
    #[test]
    fn fx_rate_for_position_falls_back_to_implied_ratio_without_matrix() {
        let as_of = date!(2024 - 01 - 01);
        let position_value = PositionValue {
            position_id: PositionId::from("EUR_POS".to_string()),
            entity_id: EntityId::from("ENTITY".to_string()),
            value_native: Money::from((100_i64, Currency::EUR)),
            value_base: Money::from((108_i64, Currency::USD)),
            metric_scale: 1.0,
            risk_metrics_complete: true,
            risk_error: None,
            inapplicable_metrics: Vec::new(),
            valuation_result: None,
        };

        let market = MarketContext::new();
        let rate = fx_rate_for_position(&position_value, Currency::USD, &market, as_of)
            .expect("implied fallback should resolve");
        assert!(
            (rate - 1.08).abs() < 1e-12,
            "implied ratio expected: {rate}"
        );
    }

    #[test]
    fn aggregate_metrics_reports_fx_errors_in_position_order() {
        let as_of = date!(2024 - 01 - 01);
        let entity_id = EntityId::from("ENTITY".to_string());
        let mut position_values = IndexMap::new();
        for (position_id, currency) in [("FIRST_EUR", Currency::EUR), ("SECOND_GBP", Currency::GBP)]
        {
            let position_id = PositionId::from(position_id.to_string());
            let measures = IndexMap::from([(MetricId::Dv01, 1.0)]);
            let valuation_result = ValuationResult::stamped(
                position_id.as_str(),
                as_of,
                Money::from((0_i64, currency)),
            )
            .with_measures(measures);
            position_values.insert(
                position_id.clone(),
                PositionValue {
                    position_id,
                    entity_id: entity_id.clone(),
                    value_native: Money::from((0_i64, currency)),
                    value_base: Money::from((0_i64, Currency::USD)),
                    metric_scale: 1.0,
                    risk_metrics_complete: true,
                    risk_error: None,
                    inapplicable_metrics: Vec::new(),
                    valuation_result: Some(valuation_result),
                },
            );
        }
        let valuation = PortfolioValuation {
            as_of,
            position_values,
            total_base_currency: Money::from((0_i64, Currency::USD)),
            by_entity: IndexMap::new(),
            degraded_positions: Vec::new(),
            fx_collapse_policy: FxConversionPolicy::CashflowDate,
            provenance: None,
        };
        let market =
            MarketContext::new().insert_fx(FxMatrix::new(Arc::new(SimpleFxProvider::new())));

        let error = aggregate_metrics(&valuation, Currency::USD, &market, as_of)
            .expect_err("both positions have missing FX quotes");
        let message = error.to_string();
        assert!(
            message.contains("EUR"),
            "logical first position must win: {message}"
        );
        assert!(
            !message.contains("GBP"),
            "later position error must not win: {message}"
        );
    }

    // IndexMap determinism

    /// `aggregate_collected_metrics` uses `IndexMap` so the iteration
    /// order of `PortfolioMetrics.aggregated` and
    /// `AggregatedMetric.by_entity` matches the insertion order of
    /// metric IDs and entity IDs across repeated calls with the same
    /// input. This guarantees deterministic CSV / JSON snapshots for
    /// downstream tooling; a `HashMap`-backed implementation would
    /// produce different orderings across CI runs.
    #[test]
    fn aggregate_metrics_output_order_is_deterministic_within_a_run() {
        fn make_data(name: &str, entity: &str) -> PositionMetricData {
            let mut metrics = indexmap::IndexMap::new();
            metrics.insert(MetricId::Dv01, 100.0);
            metrics.insert(MetricId::Cs01, 50.0);
            metrics.insert(MetricId::FxDelta, 25.0);
            PositionMetricData {
                position_id: PositionId::from(name.to_string()),
                entity_id: EntityId::from(entity.to_string()),
                currency: Currency::USD,
                fx_rate: 1.0,
                metrics,
            }
        }

        let collected = vec![
            make_data("POS_A", "ENTITY_X"),
            make_data("POS_B", "ENTITY_Y"),
            make_data("POS_C", "ENTITY_X"),
        ];
        let a = aggregate_collected_metrics(collected.clone());
        let b = aggregate_collected_metrics(collected);

        let a_order: Vec<_> = a.aggregated.keys().cloned().collect();
        let b_order: Vec<_> = b.aggregated.keys().cloned().collect();
        assert_eq!(
            a_order, b_order,
            "aggregated metric order must be deterministic across calls"
        );

        // Also verify by_entity ordering is deterministic.
        for (key, agg) in &a.aggregated {
            let a_entities: Vec<_> = agg.by_entity.keys().cloned().collect();
            let b_entities: Vec<_> = b
                .aggregated
                .get(key)
                .expect("same key")
                .by_entity
                .keys()
                .cloned()
                .collect();
            assert_eq!(
                a_entities, b_entities,
                "by_entity order for {key} must be deterministic"
            );
        }
    }
}
