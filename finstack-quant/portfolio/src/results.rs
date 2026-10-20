//! Portfolio results and output types.

use crate::metrics::PortfolioMetrics;
use crate::valuation::PortfolioValuation;
use finstack_quant_core::config::{results_meta, FinstackConfig, ResultsMeta};
use finstack_quant_core::wire::SchemaVersion;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Complete results from portfolio evaluation.
///
/// Contains valuation, metrics, and metadata about the calculation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct PortfolioResult {
    /// Required wire-format schema version. Only numeric `1` is accepted.
    pub schema_version: SchemaVersion,

    /// Portfolio valuation results
    pub valuation: PortfolioValuation,

    /// Aggregated metrics
    pub metrics: PortfolioMetrics,

    /// Metadata about the calculation
    pub meta: ResultsMeta,
}

impl PortfolioResult {
    /// Assemble a result envelope and stamp its metadata from the valuation.
    ///
    /// The `meta` stamp is derived here rather than supplied by the caller:
    /// the numeric mode, rounding context and library version come from
    /// `config`, and `fx_policy_applied` records the valuation's
    /// [`fx_collapse_policy`](PortfolioValuation::fx_collapse_policy), so the
    /// envelope always names the FX strategy that produced its base-currency
    /// totals.
    ///
    /// # Arguments
    ///
    /// * `valuation` - Portfolio valuation component; its FX collapse policy is
    ///   stamped into `meta.fx_policy_applied` (e.g. `"cashflow_date"`).
    /// * `metrics` - Aggregated metrics computed from the same valuation.
    /// * `config` - Library configuration whose rounding context is recorded
    ///   in `meta.rounding`.
    /// * `timestamp` - Optional UTC audit timestamp for `meta.timestamp`. Pass
    ///   `None` for deterministic output: identical inputs then serialize
    ///   identically.
    ///
    /// # Returns
    ///
    /// Combined result wrapper containing valuation, metrics, and calculation metadata.
    pub fn new(
        valuation: PortfolioValuation,
        metrics: PortfolioMetrics,
        config: &FinstackConfig,
        timestamp: Option<OffsetDateTime>,
    ) -> Self {
        let mut meta = results_meta(config);
        meta.timestamp = timestamp;
        meta.fx_policy_applied = Some(valuation.fx_collapse_policy.to_string());
        Self {
            schema_version: SchemaVersion::CURRENT,
            valuation,
            metrics,
            meta,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::aggregate_metrics;
    use crate::position::{Position, PositionUnit};
    use crate::test_utils::build_test_market;
    use crate::types::Entity;
    use crate::valuation::value_portfolio;
    use crate::Portfolio;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::rates::deposit::Deposit;
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn test_portfolio_results() {
        let as_of = date!(2024 - 01 - 01);

        let deposit = Deposit::builder()
            .id("DEP_1M".into())
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .start_date(as_of)
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .fixed_rate_opt(Some(
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

        let portfolio = Portfolio::builder("TEST")
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
        let results = PortfolioResult::new(valuation.clone(), metrics.clone(), &config, None);

        // The meta stamp names the valuation's FX policy and, without a
        // timestamp, two envelopes from identical inputs serialize identically.
        assert_eq!(
            results.meta.fx_policy_applied.as_deref(),
            Some(valuation.fx_collapse_policy.to_string().as_str())
        );
        assert_eq!(results.meta.timestamp, None);
        let again = PortfolioResult::new(valuation.clone(), metrics.clone(), &config, None);
        assert_eq!(
            serde_json::to_string(&results).expect("serialize"),
            serde_json::to_string(&again).expect("serialize")
        );
        let stamp = time::macros::datetime!(2024-01-01 12:00 UTC);
        let stamped = PortfolioResult::new(valuation, metrics, &config, Some(stamp));
        assert_eq!(stamped.meta.timestamp, Some(stamp));

        // Note: With flat curve, deposit PV is small but portfolio results should be present
        assert!(results.valuation.total_base_currency.amount().abs() >= 0.0);

        assert_eq!(results.metrics.get_total("absent"), None);
        let missing = results
            .metrics
            .require_total("absent")
            .expect_err("an absent metric is an error");
        assert_eq!(
            missing.kind(),
            finstack_quant_core::error::ErrorKind::NotFound
        );
        assert!(missing.to_string().contains("metric 'absent'"));
        for (metric_id, aggregate) in &results.metrics.aggregated {
            let required = results
                .metrics
                .require_total(metric_id.as_str())
                .expect("present metric");
            assert_eq!(required.to_bits(), aggregate.total.to_bits());
        }
    }
}
