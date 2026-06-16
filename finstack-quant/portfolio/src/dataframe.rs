//! Tabular exports for portfolio results.
//!
//! Functions in this module turn portfolio valuations and metrics into
//! [`finstack_quant_core::table::TableEnvelope`] values that can be serialized or
//! consumed by downstream bindings without taking a Rust DataFrame dependency.
//!
//! # Conventions
//!
//! - Position values are exported with both native-currency and base-currency
//!   columns.
//! - Metric exports use long format so bucketed and custom keys remain easy to
//!   filter and pivot downstream.

use crate::metrics::PortfolioMetrics;
use crate::valuation::PortfolioValuation;
use finstack_quant_core::table::{TableColumn, TableColumnData, TableColumnRole, TableEnvelope};
use finstack_quant_core::Result;
use indexmap::IndexMap;
use serde_json::json;

fn table_metadata(source: &str) -> IndexMap<String, serde_json::Value> {
    let mut metadata = IndexMap::new();
    metadata.insert("layout".to_string(), json!("long"));
    metadata.insert("source".to_string(), json!(source));
    metadata
}

/// Export position values to a table envelope.
///
/// Columns produced:
/// `position_id`, `entity_id`, `value_native`, `value_base`, `currency_native`, `currency_base`.
/// The instrument identifier can be re-joined by consumers if required.
///
/// # Arguments
///
/// * `valuation` - Portfolio valuation containing per-position monetary values.
///
/// # Returns
///
/// A [`Result`] wrapping the generated [`TableEnvelope`].
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_portfolio::positions_to_table;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// # let valuation: finstack_quant_portfolio::PortfolioValuation = unimplemented!("Provide a valuation");
/// let table = positions_to_table(&valuation)?;
/// assert!(table.column("position_id").is_some());
/// # Ok(())
/// # }
/// ```
pub fn positions_to_table(valuation: &PortfolioValuation) -> Result<TableEnvelope> {
    let n = valuation.position_values.len();
    let mut position_ids: Vec<String> = Vec::with_capacity(n);
    let mut entity_ids: Vec<String> = Vec::with_capacity(n);
    let mut values_native: Vec<f64> = Vec::with_capacity(n);
    let mut values_base: Vec<f64> = Vec::with_capacity(n);
    let mut currencies_native: Vec<String> = Vec::with_capacity(n);
    let mut currencies_base: Vec<String> = Vec::with_capacity(n);

    for (position_id, position_value) in &valuation.position_values {
        position_ids.push(position_id.to_string());
        entity_ids.push(position_value.entity_id.to_string());
        values_native.push(position_value.value_native.amount());
        values_base.push(position_value.value_base.amount());
        currencies_native.push(position_value.value_native.currency().to_string());
        currencies_base.push(position_value.value_base.currency().to_string());
    }

    TableEnvelope::new_with_metadata(
        vec![
            TableColumn::new("position_id", TableColumnData::String(position_ids))
                .with_role(TableColumnRole::Dimension),
            TableColumn::new("entity_id", TableColumnData::String(entity_ids))
                .with_role(TableColumnRole::Dimension),
            TableColumn::new("value_native", TableColumnData::Float64(values_native))
                .with_role(TableColumnRole::Measure),
            TableColumn::new("value_base", TableColumnData::Float64(values_base))
                .with_role(TableColumnRole::Measure),
            TableColumn::new(
                "currency_native",
                TableColumnData::String(currencies_native),
            )
            .with_role(TableColumnRole::Attribute),
            TableColumn::new("currency_base", TableColumnData::String(currencies_base))
                .with_role(TableColumnRole::Attribute),
        ],
        table_metadata("portfolio_positions"),
    )
}

/// Export entity-level aggregates to a table envelope.
///
/// Columns produced: `entity_id`, `total_value`, `currency`.
///
/// # Arguments
///
/// * `valuation` - Portfolio valuation containing aggregated entity values.
///
/// # Returns
///
/// A [`Result`] containing the generated [`TableEnvelope`].
pub fn entities_to_table(valuation: &PortfolioValuation) -> Result<TableEnvelope> {
    let n = valuation.by_entity.len();
    let mut entity_ids: Vec<String> = Vec::with_capacity(n);
    let mut total_values: Vec<f64> = Vec::with_capacity(n);
    let mut currencies: Vec<String> = Vec::with_capacity(n);

    for (entity_id, money) in &valuation.by_entity {
        entity_ids.push(entity_id.to_string());
        total_values.push(money.amount());
        currencies.push(money.currency().to_string());
    }

    TableEnvelope::new_with_metadata(
        vec![
            TableColumn::new("entity_id", TableColumnData::String(entity_ids))
                .with_role(TableColumnRole::Dimension),
            TableColumn::new("total_value", TableColumnData::Float64(total_values))
                .with_role(TableColumnRole::Measure),
            TableColumn::new("currency", TableColumnData::String(currencies))
                .with_role(TableColumnRole::Attribute),
        ],
        table_metadata("portfolio_entities"),
    )
}

/// Export metrics to a table envelope in long format.
///
/// Columns produced: `metric_id`, `position_id`, `currency`, `value`.
///
/// # Arguments
///
/// * `metrics` - Aggregated and per-position portfolio metrics.
///
/// # Returns
///
/// A [`Result`] containing the generated [`TableEnvelope`].
pub fn metrics_to_table(metrics: &PortfolioMetrics) -> Result<TableEnvelope> {
    let row_count: usize = metrics
        .by_position
        .values()
        .map(|pm| pm.metrics.len())
        .sum();
    let mut metric_ids: Vec<String> = Vec::with_capacity(row_count);
    let mut position_ids: Vec<String> = Vec::with_capacity(row_count);
    let mut currencies: Vec<String> = Vec::with_capacity(row_count);
    let mut values: Vec<f64> = Vec::with_capacity(row_count);

    for (position_id, position_metrics) in &metrics.by_position {
        for (metric_id, value) in &position_metrics.metrics {
            metric_ids.push(metric_id.clone());
            position_ids.push(position_id.to_string());
            currencies.push(position_metrics.currency.to_string());
            values.push(*value);
        }
    }

    TableEnvelope::new_with_metadata(
        vec![
            TableColumn::new("metric_id", TableColumnData::String(metric_ids))
                .with_role(TableColumnRole::Dimension),
            TableColumn::new("position_id", TableColumnData::String(position_ids))
                .with_role(TableColumnRole::Dimension),
            TableColumn::new("currency", TableColumnData::String(currencies))
                .with_role(TableColumnRole::Attribute),
            TableColumn::new("value", TableColumnData::Float64(values))
                .with_role(TableColumnRole::Measure),
        ],
        table_metadata("portfolio_metrics"),
    )
}

/// Export aggregated metrics to a table envelope.
///
/// Columns produced: `metric_id`, `total`, where `total` is the summation across positions.
///
/// # Arguments
///
/// * `metrics` - Portfolio metrics containing aggregate totals.
///
/// # Returns
///
/// A [`Result`] containing the generated [`TableEnvelope`].
pub fn aggregated_metrics_to_table(metrics: &PortfolioMetrics) -> Result<TableEnvelope> {
    let n = metrics.aggregated.len();
    let mut metric_ids: Vec<String> = Vec::with_capacity(n);
    let mut totals: Vec<f64> = Vec::with_capacity(n);

    for (metric_id, agg_metric) in &metrics.aggregated {
        metric_ids.push(metric_id.clone());
        totals.push(agg_metric.total);
    }

    TableEnvelope::new_with_metadata(
        vec![
            TableColumn::new("metric_id", TableColumnData::String(metric_ids))
                .with_role(TableColumnRole::Dimension),
            TableColumn::new("total", TableColumnData::Float64(totals))
                .with_role(TableColumnRole::Measure),
        ],
        table_metadata("portfolio_aggregated_metrics"),
    )
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
    fn test_positions_to_table() {
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
        let table = positions_to_table(&valuation).expect("test should succeed");

        assert_eq!(table.row_count, 1);
        assert!(table.column("position_id").is_some());
        assert!(table.column("value_base").is_some());
    }

    #[test]
    fn test_entities_to_table() {
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
        let table = entities_to_table(&valuation).expect("test should succeed");

        assert_eq!(table.row_count, 1);
        assert!(table.column("entity_id").is_some());
        assert!(table.column("total_value").is_some());
    }
}
