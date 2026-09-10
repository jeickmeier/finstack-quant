//! Margin serialization tests for portfolio.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_margin::{
    ImMethodology, NettingSetId, SimmCreditSector, SimmRiskClass, SimmSensitivities,
};
use finstack_quant_portfolio::types::PositionId;
use finstack_quant_portfolio::{NettingSetMargin, PortfolioMarginResult};
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

/// Assemble a USD portfolio result from base-currency netting-set margins the
/// way `PortfolioMarginAggregator::calculate` does.
fn portfolio_result(
    as_of: time::Date,
    margins: impl IntoIterator<Item = NettingSetMargin>,
) -> PortfolioMarginResult {
    let mut total_initial_margin = 0.0;
    let mut total_variation_margin = 0.0;
    let mut total_margin = 0.0;
    let mut total_positions = 0;
    let mut by_netting_set = HashMap::default();
    for margin in margins {
        total_initial_margin += margin.initial_margin.amount();
        total_variation_margin += margin.variation_margin.amount();
        total_margin += margin.total_margin.amount();
        total_positions += margin.position_count;
        by_netting_set.insert(margin.netting_set_id.clone(), margin);
    }
    PortfolioMarginResult {
        as_of,
        base_currency: Currency::USD,
        total_initial_margin: Money::new(total_initial_margin, Currency::USD)
            .expect("valid money fixture"),
        total_variation_margin: Money::new(total_variation_margin, Currency::USD)
            .expect("valid money fixture"),
        total_margin: Money::new(total_margin, Currency::USD).expect("valid money fixture"),
        by_netting_set,
        by_csa: HashMap::default(),
        total_required_im_collateral: Money::from((0_i64, Currency::USD)),
        total_im_transfer: Money::from((0_i64, Currency::USD)),
        total_segregated_im: Money::from((0_i64, Currency::USD)),
        total_positions,
        positions_without_margin: 0,
        degraded_positions: Vec::new(),
    }
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
        .credit_non_qualifying_delta
        .insert(("RMBS_INDEX".to_string(), "3Y".to_string()), -1_100.0);
    sensitivities.equity_delta.insert("AAPL".to_string(), 800.0);
    sensitivities.equity_vega.insert("AAPL".to_string(), 125.0);
    sensitivities.fx_delta.insert(Currency::JPY, 2_200.0);
    sensitivities
        .fx_vega
        .insert((Currency::EUR, Currency::USD), 410.0);
    sensitivities
        .commodity_delta
        .insert("EuropeanPowerAndCarbon".to_string(), -95.0);
    // The three vega buckets the portfolio wire previously dropped on round-trip.
    sensitivities
        .commodity_vega
        .insert("EuropeanPowerAndCarbon".to_string(), 42.0);
    sensitivities
        .credit_non_qualifying_vega
        .insert(("RMBS_INDEX".to_string(), "3Y".to_string()), 61.0);
    sensitivities.credit_qualifying_vega.insert(
        (
            SimmCreditSector::Financial,
            "BANK_A".to_string(),
            "5Y".to_string(),
        ),
        77.0,
    );
    sensitivities.add_curvature(finstack_quant_margin::SimmCurvatureSensitivity {
        risk_class: SimmRiskClass::InterestRate,
        bucket: "USD".into(),
        factor: "OIS".into(),
        risk_tenor: Some("5Y".into()),
        expiry_tenor: "1Y".into(),
        volatility_weighted_vega: -75.0,
    });
    sensitivities.credit_qualifying_delta.insert(
        (
            SimmCreditSector::Financial,
            "BANK_A".to_string(),
            "5Y".to_string(),
        ),
        725.0,
    );
    sensitivities
}

#[test]
fn simm_sensitivities_serialize_deterministically_across_insertion_orders() {
    let mut first = sample_simm_sensitivities();
    first.credit_qualifying_delta.insert(
        (
            SimmCreditSector::Sovereign,
            "UST".to_string(),
            "10Y".to_string(),
        ),
        350.0,
    );

    let mut second = first.clone();
    second.ir_delta.clear();
    second
        .ir_delta
        .insert((Currency::EUR, "2Y".to_string()), -2_500.0);
    second
        .ir_delta
        .insert((Currency::USD, "5Y".to_string()), 12_500.0);
    second.credit_qualifying_delta.clear();
    second.credit_qualifying_delta.insert(
        (
            SimmCreditSector::Sovereign,
            "UST".to_string(),
            "10Y".to_string(),
        ),
        350.0,
    );
    second.credit_qualifying_delta.insert(
        (
            SimmCreditSector::Financial,
            "BANK_A".to_string(),
            "5Y".to_string(),
        ),
        725.0,
    );

    let make_margin = |sensitivities| {
        NettingSetMargin::new(
            NettingSetId::bilateral("BANK_A", "CSA_01"),
            date!(2025 - 01 - 15),
            Money::new(1_250_000.0, Currency::USD).expect("valid money fixture"),
            Money::new(150_000.0, Currency::USD).expect("valid money fixture"),
            4,
            ImMethodology::Simm,
        )
        .expect("valid new fixture")
        .with_simm_breakdown(sensitivities, Default::default())
    };

    let first_json = serde_json::to_vec(&make_margin(first)).expect("first order serializes");
    let second_json = serde_json::to_vec(&make_margin(second)).expect("second order serializes");
    assert_eq!(first_json, second_json);
}

#[test]
fn im_breakdown_serializes_in_sorted_order_across_reversed_insertions() {
    let make_margin = |reverse: bool| {
        let mut entries = [
            ("InterestRate", 875_000.0),
            ("CreditQualifying", 125_000.0),
            ("Equity", 50_000.0),
        ];
        if reverse {
            entries.reverse();
        }
        let mut breakdown = HashMap::default();
        for (name, amount) in entries {
            breakdown.insert(
                name.to_string(),
                Money::new(amount, Currency::USD).expect("valid money fixture"),
            );
        }
        NettingSetMargin::new(
            NettingSetId::bilateral("BANK_A", "CSA_01"),
            date!(2025 - 01 - 15),
            Money::new(1_250_000.0, Currency::USD).expect("valid money fixture"),
            Money::new(150_000.0, Currency::USD).expect("valid money fixture"),
            4,
            ImMethodology::Simm,
        )
        .expect("valid new fixture")
        .with_simm_breakdown(sample_simm_sensitivities(), breakdown)
    };

    let first_json = serde_json::to_vec(&make_margin(false)).expect("first order serializes");
    let second_json = serde_json::to_vec(&make_margin(true)).expect("reverse order serializes");
    assert_eq!(first_json, second_json);
    let text = String::from_utf8(first_json).expect("margin JSON is UTF-8");
    let breakdown = text
        .split_once("\"im_breakdown\":")
        .expect("IM breakdown exists")
        .1;
    let credit = breakdown
        .find("CreditQualifying")
        .expect("credit entry exists");
    let equity = breakdown.find("Equity").expect("equity entry exists");
    let rates = breakdown.find("InterestRate").expect("rates entry exists");
    assert!(credit < equity && equity < rates);
    let restored: NettingSetMargin =
        serde_json::from_str(&text).expect("sorted IM breakdown deserializes");
    assert_eq!(restored.im_breakdown.len(), 3);
    assert_eq!(
        restored.im_breakdown["CreditQualifying"],
        Money::new(125_000.0, Currency::USD).expect("valid money fixture")
    );
}

#[test]
fn simm_margin_with_scalar_credit_qualifying_delta_is_rejected() {
    // The old `(label, tenor, value)` CQ shape must fail closed because sector
    // assignment is mandatory for ISDA SIMM bucket aggregation.
    let err = serde_json::from_str::<NettingSetMargin>(include_str!(
        "data/simm_margin_without_bucketed_credit.json"
    ))
    .expect_err("the scalar credit-qualifying shape must be rejected");
    assert!(
        err.to_string().contains("unknown variant `CDX.NA.IG`"),
        "the reference label must be rejected in the required sector slot, got: {err}"
    );
}

#[test]
fn test_netting_set_margin_json_roundtrip() {
    let margin = NettingSetMargin::new(
        NettingSetId::bilateral("BANK_A", "CSA_01"),
        date!(2025 - 01 - 15),
        Money::new(1_250_000.0, Currency::USD).expect("valid money fixture"),
        Money::new(150_000.0, Currency::USD).expect("valid money fixture"),
        4,
        ImMethodology::Simm,
    )
    .expect("valid new fixture")
    .with_simm_breakdown(
        sample_simm_sensitivities(),
        std::iter::once((
            "InterestRate".to_string(),
            Money::new(875_000.0, Currency::USD).expect("valid money fixture"),
        ))
        .collect(),
    );

    assert_roundtrip_value(&margin);

    let json = serde_json::to_value(&margin).expect("margin should serialize");
    assert_eq!(
        json["sensitivities"],
        serde_json::json!({
            "base_currency": "USD",
            "ir_delta": [["USD", "5Y", 12_500.0], ["EUR", "2Y", -2_500.0]],
            "ir_vega": [["USD", "10Y", 3_250.0]],
            "credit_qualifying_delta": [["financial", "BANK_A", "5Y", 725.0]],
            "credit_qualifying_vega": [["financial", "BANK_A", "5Y", 77.0]],
            "credit_non_qualifying_delta": [["RMBS_INDEX", "3Y", -1_100.0]],
            "credit_non_qualifying_vega": [["RMBS_INDEX", "3Y", 61.0]],
            "equity_delta": [["AAPL", 800.0]],
            "equity_vega": [["AAPL", 125.0]],
            "fx_delta": [["JPY", 2_200.0]],
            "fx_vega": [["EUR", "USD", 410.0]],
            "commodity_delta": [["EuropeanPowerAndCarbon", -95.0]],
            "commodity_vega": [["EuropeanPowerAndCarbon", 42.0]],
            "curvature": [{"risk_class": "interest_rate", "bucket": "USD", "factor": "OIS", "risk_tenor": "5Y", "expiry_tenor": "1Y", "volatility_weighted_vega": -75.0}],
        }),
        "portfolio results must use the canonical margin sensitivity tuples"
    );
    let canonical = sample_simm_sensitivities()
        .to_json()
        .expect("standalone SIMM sensitivities serialize");
    assert_eq!(
        json["sensitivities"],
        serde_json::from_str::<serde_json::Value>(&canonical).expect("canonical JSON is valid")
    );
}

#[test]
fn test_portfolio_margin_result_json_roundtrip() {
    let usd_margin = NettingSetMargin::new(
        NettingSetId::cleared("LCH"),
        date!(2025 - 01 - 15),
        Money::new(900_000.0, Currency::USD).expect("valid money fixture"),
        Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
        5,
        ImMethodology::ClearingHouse,
    )
    .expect("valid new fixture");
    let eur_margin = NettingSetMargin::new(
        NettingSetId::bilateral("BANK_B", "CSA_EUR"),
        date!(2025 - 01 - 15),
        Money::new(825_000.0, Currency::USD).expect("valid money fixture"),
        Money::new(55_000.0, Currency::USD).expect("valid money fixture"),
        3,
        ImMethodology::Simm,
    )
    .expect("valid new fixture")
    .with_simm_breakdown(sample_simm_sensitivities(), Default::default());

    let result = PortfolioMarginResult {
        positions_without_margin: 2,
        degraded_positions: vec![(PositionId::new("POS_9"), "missing VM source".into())],
        ..portfolio_result(date!(2025 - 01 - 15), [usd_margin, eur_margin])
    };

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
        Money::new(1_250_000.0, Currency::USD).expect("valid money fixture"),
        Money::new(150_000.0, Currency::USD).expect("valid money fixture"),
        4,
        ImMethodology::Simm,
    )
    .expect("valid new fixture");
    let mut json = serde_json::to_value(&margin).expect("margin should serialize");
    json["total_margin"] =
        serde_json::to_value(Money::new(1.0, Currency::USD).expect("valid money fixture"))
            .expect("money should serialize");

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
        Money::new(900_000.0, Currency::USD).expect("valid money fixture"),
        Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
        5,
        ImMethodology::ClearingHouse,
    )
    .expect("valid new fixture");
    let result = portfolio_result(date!(2025 - 01 - 15), [margin]);
    let mut json = serde_json::to_value(&result).expect("result should serialize");
    json["total_margin"] =
        serde_json::to_value(Money::new(1.0, Currency::USD).expect("valid money fixture"))
            .expect("money should serialize");

    let err = serde_json::from_value::<PortfolioMarginResult>(json)
        .expect_err("minor 17: inconsistent portfolio totals must fail");
    assert!(
        err.to_string().contains("minor 17"),
        "unexpected error: {err}"
    );
}

/// Every sensitivity bucket must survive portfolio serialization, including
/// the credit and commodity vegas previously lost by the duplicate wire type.
/// Compare domain values because serializing both sides can hide dropped fields.
#[test]
fn portfolio_wire_round_trip_preserves_all_sensitivity_buckets() {
    let original = sample_simm_sensitivities();
    assert!(
        !original.credit_qualifying_vega.is_empty()
            && !original.credit_non_qualifying_vega.is_empty()
            && !original.commodity_vega.is_empty(),
        "fixture must actually carry the buckets under test"
    );

    let margin = NettingSetMargin::new(
        NettingSetId::bilateral("BANK_A", "CSA_01"),
        date!(2025 - 01 - 15),
        Money::new(1_250_000.0, Currency::USD).expect("valid money fixture"),
        Money::new(150_000.0, Currency::USD).expect("valid money fixture"),
        4,
        ImMethodology::Simm,
    )
    .expect("valid new fixture")
    .with_simm_breakdown(original.clone(), HashMap::default());

    let text = serde_json::to_string(&margin).expect("netting set serializes");
    let restored: NettingSetMargin = serde_json::from_str(&text).expect("netting set deserializes");
    let restored = restored
        .sensitivities
        .expect("sensitivities survive the round-trip");

    assert_eq!(restored, original, "every SIMM sensitivity must survive");
}
