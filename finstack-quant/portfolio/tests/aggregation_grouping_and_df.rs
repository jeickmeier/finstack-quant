//! Aggregation grouping and df tests for portfolio.

mod common;

use common::*;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::PortfolioBuilder;
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use std::sync::Arc;
use time::Duration;

#[test]
fn dataframe_exports_have_expected_columns() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    let dep = Deposit::builder()
        .id("D".into())
        .notional(Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .unwrap();

    let p = Position::new("P", "E", "D", Arc::new(dep), 1.0, PositionUnit::Units).unwrap();

    let portfolio = PortfolioBuilder::new("P")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E"))
        .position(p)
        .build()
        .unwrap();

    let market = market_with_usd();
    let config = FinstackConfig::default();
    let valuation = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();
    let metrics = finstack_quant_portfolio::metrics::aggregate_metrics(
        &valuation,
        Currency::USD,
        &market,
        as_of,
    )
    .unwrap();

    let table_pos = finstack_quant_portfolio::positions_to_table(&valuation).unwrap();
    let table_ent = finstack_quant_portfolio::entities_to_table(&valuation).unwrap();
    let table_m = finstack_quant_portfolio::metrics_to_table(&metrics).unwrap();
    let table_ma = finstack_quant_portfolio::aggregated_metrics_to_table(&metrics).unwrap();

    let pos_cols: Vec<&str> = table_pos
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect();
    assert!(pos_cols.contains(&"position_id") && pos_cols.contains(&"value_base"));
    let ent_cols: Vec<&str> = table_ent
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect();
    assert!(ent_cols.contains(&"entity_id") && ent_cols.contains(&"total_value"));
    let m_cols: Vec<&str> = table_m
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect();
    assert!(
        m_cols.contains(&"metric_id")
            && m_cols.contains(&"position_id")
            && m_cols.contains(&"value")
    );
    let ma_cols: Vec<&str> = table_ma
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect();
    assert!(ma_cols.contains(&"metric_id") && ma_cols.contains(&"total"));
}
