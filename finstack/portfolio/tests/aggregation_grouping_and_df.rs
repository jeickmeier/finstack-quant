mod common;

use common::*;
use finstack_core::config::FinstackConfig;
use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_portfolio::grouping::{
    aggregate_by_attribute, aggregate_by_multiple_attributes, group_by_attribute,
};
use finstack_portfolio::position::{Position, PositionUnit};
use finstack_portfolio::types::Entity;
use finstack_portfolio::PortfolioBuilder;
use finstack_valuations::instruments::rates::deposit::Deposit;
use std::sync::Arc;
use time::Duration;

#[test]
fn grouping_and_multi_attribute_aggregation() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    let dep1 = Deposit::builder()
        .id("D1".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .unwrap();
    let dep2 = Deposit::builder()
        .id("D2".into())
        .notional(Money::new(500_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .unwrap();

    let p1 = Position::new("P1", "E", "D1", Arc::new(dep1), 1.0, PositionUnit::Units)
        .unwrap()
        .with_text_attribute("rating", "AAA")
        .with_text_attribute("sector", "Banking");
    let p2 = Position::new("P2", "E", "D2", Arc::new(dep2), 1.0, PositionUnit::Units)
        .unwrap()
        .with_text_attribute("rating", "AA");

    let portfolio = PortfolioBuilder::new("P")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E"))
        .positions(vec![p1, p2])
        .build()
        .unwrap();

    let market = market_with_usd();
    let config = FinstackConfig::default();
    let valuation = finstack_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();

    let groups = group_by_attribute(portfolio.positions(), "rating");
    assert!(groups.contains_key("AAA") && groups.contains_key("AA"));
    assert!(!groups.contains_key("_untagged"));

    let agg =
        aggregate_by_attribute(&valuation, portfolio.positions(), "rating", Currency::USD).unwrap();
    assert!(agg.contains_key("AAA") && agg.contains_key("AA"));

    let agg2 = aggregate_by_multiple_attributes(
        &valuation,
        portfolio.positions(),
        &["rating", "sector"],
        Currency::USD,
    )
    .unwrap();
    assert!(!agg2.is_empty());
}

#[test]
fn dataframe_exports_have_expected_columns() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    let dep = Deposit::builder()
        .id("D".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .unwrap();

    let p = Position::new("P", "E", "D", Arc::new(dep), 1.0, PositionUnit::Units).unwrap();

    let portfolio = PortfolioBuilder::new("P")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E"))
        .position(p)
        .build()
        .unwrap();

    let market = market_with_usd();
    let config = FinstackConfig::default();
    let valuation = finstack_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();
    let metrics =
        finstack_portfolio::metrics::aggregate_metrics(&valuation, Currency::USD, &market, as_of)
            .unwrap();

    let table_pos = finstack_portfolio::positions_to_table(&valuation).unwrap();
    let table_ent = finstack_portfolio::entities_to_table(&valuation).unwrap();
    let table_m = finstack_portfolio::metrics_to_table(&metrics).unwrap();
    let table_ma = finstack_portfolio::aggregated_metrics_to_table(&metrics).unwrap();

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
