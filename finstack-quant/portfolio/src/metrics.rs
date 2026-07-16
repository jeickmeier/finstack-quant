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
//! `aggregate_metrics` performs this conversion automatically using the FX rate
//! implied by each position's native and base-currency valuations. Positions with
//! zero native PV fall back to the FX matrix in the
//! [`finstack_quant_core::market_data::context::MarketContext`].

use crate::error::{Error, Result};
use crate::types::{EntityId, PositionId};
use crate::valuation::PortfolioValuation;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::summation::neumaier_sum;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::{HashMap, HashSet};
use finstack_quant_valuations::metrics::MetricId;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Aggregated metric across the portfolio.
///
/// Contains portfolio-wide totals as well as breakdowns by entity.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortfolioMetrics {
    /// Aggregated metrics (summable only)
    pub aggregated: IndexMap<String, AggregatedMetric>,

    /// Raw metrics by position (all metrics), with explicit native currency context.
    pub by_position: IndexMap<PositionId, PositionMetrics>,

    /// Positions whose non-finite metric values were excluded from aggregation.
    ///
    /// Each entry records the position and metric that was skipped, allowing
    /// callers to detect incomplete aggregation (analogous to
    /// [`crate::valuation::PortfolioValuation::degraded_positions`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_metrics: Vec<SkippedMetric>,
}

/// Position-level metrics with explicit native currency context.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PositionMetrics {
    /// Native currency for this position's valuation and non-summable metrics.
    pub currency: Currency,
    /// Raw metric values for the position.
    pub metrics: IndexMap<String, f64>,
}

impl std::ops::Deref for PositionMetrics {
    type Target = IndexMap<String, f64>;

    fn deref(&self) -> &Self::Target {
        &self.metrics
    }
}

impl<'a> IntoIterator for &'a PositionMetrics {
    type Item = (&'a String, &'a f64);
    type IntoIter = indexmap::map::Iter<'a, String, f64>;

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
    /// The scalar aggregate stored directly under `base` is excluded. Malformed
    /// legacy escape markers remain literal, and decoded-coordinate collisions
    /// fall back to literal wire components so all aggregate entries survive.
    pub fn metric_series(&self, base: &MetricId) -> Vec<(Vec<String>, &AggregatedMetric)> {
        let metric_ids: Vec<MetricId> = self.aggregated.keys().map(MetricId::custom).collect();
        let components = MetricId::decode_series_components(base, &metric_ids);
        self.aggregated
            .values()
            .zip(components)
            .filter_map(|(aggregate, components)| {
                components.map(|components| (components, aggregate))
            })
            .collect()
    }
}

/// A metric value that was excluded from portfolio aggregation because it was
/// non-finite (NaN or ±Inf).
#[derive(Clone, Debug, Serialize, Deserialize)]
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
static SUMMABLE_SET: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "theta",
        "dv01",
        "cs01",
        "delta",
        "gamma",
        "vega",
        "rho",
        "pv01",
        "ir01",
        "hazard_cs01",
        "index_delta",
        "bucketed_dv01",
        "bucketed_cs01",
        "accrued_interest",
        "pv_fixed",
        "pv_float",
        "pv_primary",
        "pv_reference",
    ]
    .into_iter()
    .collect()
});

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
    if SUMMABLE_SET.contains(metric_id) {
        return true;
    }

    // Handle composite keys produced by `MetricId::composite`, which uses the
    // pattern `base::label[::sub_label...]`.
    if let Some((base, _rest)) = metric_id.split_once("::") {
        return SUMMABLE_SET.contains(base);
    }

    false
}

/// Aggregate metrics from portfolio valuation results.
///
/// This function:
/// 1. Collects all metrics from position valuations (in parallel if enabled)
/// 2. FX-converts summable metrics to portfolio base currency before aggregation
/// 3. Aggregates summable metrics by entity and portfolio total
/// 4. Stores non-summable metrics by position only (in native currency)
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
/// # Arguments
///
/// * `valuation` - Portfolio valuation containing per-position valuation results.
/// * `base_ccy` - Portfolio base currency for aggregation.
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
/// - Fixed-income risk conventions:
///   `docs/REFERENCES.md#tuckman-serrat-fixed-income`
/// - Numerically stable aggregation:
///   `docs/REFERENCES.md#kahan-1965`
pub fn aggregate_metrics(
    valuation: &PortfolioValuation,
    base_ccy: Currency,
    market: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
) -> Result<PortfolioMetrics> {
    use rayon::prelude::*;

    let valuation_base_ccy = valuation.total_base_ccy.currency();
    if base_ccy != valuation_base_ccy {
        return Err(Error::invalid_input(format!(
            "M-17: aggregate_metrics base_ccy {base_ccy} does not match valuation base currency {valuation_base_ccy}"
        )));
    }
    if as_of != valuation.as_of {
        return Err(Error::invalid_input(format!(
            "M-17: aggregate_metrics as_of {as_of} does not match valuation as_of {}",
            valuation.as_of
        )));
    }

    let position_entries: Vec<_> = valuation.position_values.iter().collect();

    let collected: Vec<PositionMetricData> = position_entries
        .par_iter()
        .filter_map(|(position_id, position_value)| {
            position_value.valuation_result.as_ref().map(|val_result| {
                let metrics: IndexMap<String, f64> = val_result
                    .measures
                    .iter()
                    .map(|(id, v)| {
                        let metric_id = id.as_str().to_string();
                        let scaled =
                            scale_position_metric(&metric_id, *v, position_value.metric_scale);
                        (metric_id, scaled)
                    })
                    .collect();
                let fx_rate = fx_rate_for_position(position_value, base_ccy, market, as_of)?;
                Ok(PositionMetricData {
                    position_id: (*position_id).clone(),
                    entity_id: position_value.entity_id.clone(),
                    currency: position_value.value_native.currency(),
                    metrics,
                    fx_rate,
                })
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(aggregate_collected_metrics(collected))
}

/// Compute the FX conversion factor from a position's native currency to base currency.
///
/// Attempts to derive the rate from the position's valuation (value_base / value_native)
/// when the native PV is large enough for a reliable ratio. Falls back to the FX matrix
/// for positions with near-zero PV to avoid amplifying numerical noise.
fn fx_rate_for_position(
    position_value: &crate::valuation::PositionValue,
    base_ccy: Currency,
    market: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
) -> Result<f64> {
    let native_ccy = position_value.value_native.currency();

    if native_ccy == base_ccy {
        return Ok(1.0);
    }

    // Use a conservative threshold to avoid distorted implied FX rates when
    // native PV is near zero (e.g., expired instruments).  Positions with
    // |native_amount| < 1e-6 fall through to the FX matrix lookup.
    let native_amount = position_value.value_native.amount();
    if native_amount.abs() > 1e-6 {
        let base_amount = position_value.value_base.amount();
        return Ok(base_amount / native_amount);
    }

    let fx_matrix = market.fx().ok_or_else(|| {
        crate::error::Error::MissingMarketData(
            "FX matrix not available for metric FX conversion".to_string(),
        )
    })?;
    let query = FxQuery::new(native_ccy, base_ccy, as_of);
    let rate_result =
        fx_matrix
            .rate(query)
            .map_err(|_| crate::error::Error::FxConversionFailed {
                from: native_ccy,
                to: base_ccy,
            })?;
    Ok(rate_result.rate)
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
    metrics: IndexMap<String, f64>,
    fx_rate: f64,
}

/// Aggregate collected position metric data into portfolio metrics.
///
/// This is the shared Phase 2+3 logic used by both serial and parallel implementations.
/// Takes ownership of `collected` to move metric maps instead of cloning them.
fn aggregate_collected_metrics(collected: Vec<PositionMetricData>) -> PortfolioMetrics {
    use std::sync::Arc;

    let n = collected.len();
    let mut by_position: IndexMap<PositionId, PositionMetrics> = IndexMap::with_capacity(n);

    // Intern metric IDs: each unique string is stored once as Arc<str>,
    // eliminating per-position String clones during accumulation. The
    // intern table's iteration order doesn't affect output, so HashMap
    // is fine here.
    let mut intern: HashMap<String, Arc<str>> = HashMap::default();

    // These two maps drive the serialization order of
    // `PortfolioMetrics.aggregated` and `AggregatedMetric.by_entity`.
    // `IndexMap` preserves insertion order so CSV/JSON snapshots of
    // portfolio metrics are reproducible run-to-run (a plain `HashMap`
    // would randomise iteration per-process under SipHash).
    let mut metric_values: IndexMap<Arc<str>, Vec<f64>> = IndexMap::new();
    let mut entity_values: IndexMap<Arc<str>, IndexMap<EntityId, Vec<f64>>> = IndexMap::new();
    let mut skipped_metrics: Vec<SkippedMetric> = Vec::new();

    for data in collected {
        let fx_rate = data.fx_rate;
        let entity_id = data.entity_id;

        for (metric_id, value) in &data.metrics {
            if is_summable(metric_id) {
                if !value.is_finite() {
                    tracing::warn!(
                        metric_id = %metric_id,
                        position_id = %data.position_id,
                        value,
                        "Skipping non-finite metric value"
                    );
                    skipped_metrics.push(SkippedMetric {
                        position_id: data.position_id.clone(),
                        metric_id: metric_id.clone(),
                        value: *value,
                    });
                    continue;
                }

                let key = Arc::clone(
                    intern
                        .entry(metric_id.clone())
                        .or_insert_with(|| Arc::from(metric_id.as_str())),
                );
                let value_base = *value * fx_rate;

                metric_values
                    .entry(Arc::clone(&key))
                    .or_default()
                    .push(value_base);

                entity_values
                    .entry(key)
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

    let mut aggregated: IndexMap<String, AggregatedMetric> =
        IndexMap::with_capacity(metric_values.len());

    for (metric_id, values) in metric_values {
        let total = neumaier_sum(values.into_iter());

        let by_entity: IndexMap<EntityId, f64> = entity_values
            .shift_remove(&metric_id)
            .into_iter()
            .flat_map(|entity_map| {
                entity_map
                    .into_iter()
                    .map(|(eid, vals)| (eid, neumaier_sum(vals.into_iter())))
            })
            .collect();

        let metric_string: String = (*metric_id).to_string();
        aggregated.insert(
            metric_string.clone(),
            AggregatedMetric {
                metric_id: metric_string,
                total,
                by_entity,
            },
        );
    }

    PortfolioMetrics {
        aggregated,
        by_position,
        skipped_metrics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::PortfolioBuilder;
    use crate::position::{Position, PositionUnit};
    use crate::test_utils::build_test_market;
    use crate::types::Entity;
    use crate::valuation::value_portfolio;
    use finstack_quant_core::config::FinstackConfig;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::rates::deposit::Deposit;
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn test_is_summable() {
        assert!(is_summable("dv01"));
        assert!(is_summable("cs01"));
        assert!(is_summable("delta"));
        assert!(!is_summable("ytm"));
        assert!(!is_summable("duration"));

        // Test bucketed/composite keys
        assert!(is_summable("bucketed_dv01::2y"));
        assert!(is_summable("bucketed_cs01::AAA::5y"));
        assert!(!is_summable("unknown::2y"));
    }

    #[test]
    fn portfolio_metric_series_decodes_components_in_aggregate_order() {
        let mut aggregated = IndexMap::new();
        for (metric_id, total) in [
            ("bucketed_dv01", -3.0),
            ("bucketed_dv01::USD_x2dOIS::10y", -1.0),
            ("bucketed_cs01::ACME", 8.0),
            ("bucketed_dv01::EUR_x2fUSD::_empty", -2.0),
        ] {
            aggregated.insert(
                metric_id.to_string(),
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
    fn portfolio_metric_series_preserves_legacy_collision_entries() {
        let mut aggregated = IndexMap::new();
        for (metric_id, total) in [
            ("bucketed_dv01::curve-ray", -1.0),
            ("bucketed_dv01::curve_x2dray", -2.0),
            ("bucketed_dv01::curve_xray", -3.0),
        ] {
            aggregated.insert(
                metric_id.to_string(),
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
            .notional(Money::new(1_000_000.0, Currency::USD))
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
            .base_ccy(Currency::USD)
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

        // Should have position-level metrics
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

    // =====================================================================
    // IndexMap determinism
    // =====================================================================

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
            metrics.insert("dv01".into(), 100.0);
            metrics.insert("cs01".into(), 50.0);
            metrics.insert("delta".into(), 25.0);
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
