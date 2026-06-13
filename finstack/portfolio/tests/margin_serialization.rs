//! Margin serialization tests for portfolio.

use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_margin::{ImMethodology, NettingSetId, SimmRiskClass, SimmSensitivities};
use finstack_portfolio::types::PositionId;
use finstack_portfolio::{CurrencyMismatchError, NettingSetMargin, PortfolioMarginResult};
use time::macros::date;

fn roundtrip_json<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json = serde_json::to_string(value).expect("serialization should succeed");
    serde_json::from_str(&json).expect("deserialization should succeed")
}

fn assert_roundtrip_value<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let restored = roundtrip_json(value);
    assert_eq!(
        serde_json::to_value(value).expect("value serialization should succeed"),
        serde_json::to_value(&restored).expect("value reserialization should succeed")
    );
}

fn sample_simm_sensitivities() -> SimmSensitivities {
    let mut sensitivities = SimmSensitivities::new(Currency::USD);
    sensitivities
        .ir_delta
        .insert((Currency::USD, "5Y".to_string()), 12_500.0);
    sensitivities
        .ir_delta
        .insert((Currency::EUR, "2Y".to_string()), -2_500.0);
    sensitivities
        .ir_vega
        .insert((Currency::USD, "10Y".to_string()), 3_250.0);
    sensitivities
        .credit_qualifying_delta
        .insert(("CDX.NA.IG".to_string(), "5Y".to_string()), 4_500.0);
    sensitivities
        .credit_non_qualifying_delta
        .insert(("HY_INDEX".to_string(), "3Y".to_string()), -1_100.0);
    sensitivities.equity_delta.insert("AAPL".to_string(), 800.0);
    sensitivities.equity_vega.insert("AAPL".to_string(), 125.0);
    sensitivities.fx_delta.insert(Currency::JPY, 2_200.0);
    sensitivities
        .fx_vega
        .insert((Currency::EUR, Currency::USD), 410.0);
    sensitivities
        .commodity_delta
        .insert("Power".to_string(), -95.0);
    sensitivities
        .curvature
        .insert(SimmRiskClass::InterestRate, -75.0);
    sensitivities
}

#[test]
fn test_currency_mismatch_error_roundtrip() {
    assert_roundtrip_value(&CurrencyMismatchError {
        netting_set_id: NettingSetId::bilateral("BANK_A", "CSA_01"),
        netting_set_currency: Currency::EUR,
        base_currency: Currency::USD,
    });
}

#[test]
fn test_netting_set_margin_json_roundtrip() {
    let margin = NettingSetMargin::new(
        NettingSetId::bilateral("BANK_A", "CSA_01"),
        date!(2025 - 01 - 15),
        Money::new(1_250_000.0, Currency::USD),
        Money::new(150_000.0, Currency::USD),
        4,
        ImMethodology::Simm,
    )
    .with_simm_breakdown(
        sample_simm_sensitivities(),
        std::iter::once((
            "InterestRate".to_string(),
            Money::new(875_000.0, Currency::USD),
        ))
        .collect(),
    );

    assert_roundtrip_value(&margin);

    let json = serde_json::to_value(&margin).expect("margin should serialize");
    assert!(json.get("sensitivities").is_some());
    assert!(json["sensitivities"]["ir_delta"].is_array());
    assert!(json["sensitivities"]["fx_vega"].is_array());
}

#[test]
fn test_portfolio_margin_result_json_roundtrip() {
    let usd_margin = NettingSetMargin::new(
        NettingSetId::cleared("LCH"),
        date!(2025 - 01 - 15),
        Money::new(900_000.0, Currency::USD),
        Money::new(100_000.0, Currency::USD),
        5,
        ImMethodology::ClearingHouse,
    );
    let eur_margin = NettingSetMargin::new(
        NettingSetId::bilateral("BANK_B", "CSA_EUR"),
        date!(2025 - 01 - 15),
        Money::new(750_000.0, Currency::EUR),
        Money::new(50_000.0, Currency::EUR),
        3,
        ImMethodology::Simm,
    )
    .with_simm_breakdown(sample_simm_sensitivities(), Default::default());

    let mut result = PortfolioMarginResult::new(date!(2025 - 01 - 15), Currency::USD);
    result
        .add_netting_set(usd_margin)
        .expect("same-currency netting set should aggregate");
    result
        .add_netting_set_with_fx(eur_margin, 1.1)
        .expect("valid FX rate should aggregate EUR netting set");
    result.positions_without_margin = 2;
    result.add_degraded_position(PositionId::new("POS_9"), "missing VM source");

    assert_roundtrip_value(&result);

    let json = serde_json::to_value(&result).expect("portfolio margin result should serialize");
    assert!(json.get("netting_sets").is_some());
    assert!(json.get("by_netting_set").is_none());
    assert!(json["netting_sets"].is_array());
    assert!(json["degraded_positions"].is_array());
}

#[test]
fn minor17_netting_set_deserialize_rejects_inconsistent_total() {
    let margin = NettingSetMargin::new(
        NettingSetId::bilateral("BANK_A", "CSA_01"),
        date!(2025 - 01 - 15),
        Money::new(1_250_000.0, Currency::USD),
        Money::new(150_000.0, Currency::USD),
        4,
        ImMethodology::Simm,
    );
    let mut json = serde_json::to_value(&margin).expect("margin should serialize");
    json["total_margin"] =
        serde_json::to_value(Money::new(1.0, Currency::USD)).expect("money should serialize");

    let err = serde_json::from_value::<NettingSetMargin>(json)
        .expect_err("minor 17: inconsistent netting-set total must fail");
    assert!(
        err.to_string().contains("minor 17"),
        "unexpected error: {err}"
    );
}

#[test]
fn minor17_portfolio_margin_deserialize_rejects_inconsistent_totals() {
    let margin = NettingSetMargin::new(
        NettingSetId::cleared("LCH"),
        date!(2025 - 01 - 15),
        Money::new(900_000.0, Currency::USD),
        Money::new(100_000.0, Currency::USD),
        5,
        ImMethodology::ClearingHouse,
    );
    let mut result = PortfolioMarginResult::new(date!(2025 - 01 - 15), Currency::USD);
    result
        .add_netting_set(margin)
        .expect("same-currency netting set should aggregate");
    let mut json = serde_json::to_value(&result).expect("result should serialize");
    json["total_margin"] =
        serde_json::to_value(Money::new(1.0, Currency::USD)).expect("money should serialize");

    let err = serde_json::from_value::<PortfolioMarginResult>(json)
        .expect_err("minor 17: inconsistent portfolio totals must fail");
    assert!(
        err.to_string().contains("minor 17"),
        "unexpected error: {err}"
    );
}
