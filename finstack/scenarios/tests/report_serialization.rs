use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_scenarios::{ArbitrageViolation, InterpolationResult, RollForwardReport};
use indexmap::IndexMap;
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

#[test]
fn test_scenarios_report_and_diagnostics_roundtrip() {
    let mut total_carry = IndexMap::new();
    total_carry.insert(Currency::USD, Money::new(1250.0, Currency::USD));

    let mut instrument_carry = IndexMap::new();
    instrument_carry.insert(Currency::USD, Money::new(500.0, Currency::USD));

    assert_roundtrip_value(&RollForwardReport {
        old_date: date!(2025 - 01 - 01),
        new_date: date!(2025 - 02 - 01),
        days: 31,
        instrument_carry: vec![("BOND_A".to_string(), instrument_carry)],
        total_carry,
        failed_instruments: vec![("LOAN_B".to_string(), "missing carry inputs".to_string())],
    });

    assert_roundtrip_value(&ArbitrageViolation::CalendarSpread {
        strike: 100.0,
        expiry: 2.0,
        prev_variance: 0.09,
        curr_variance: 0.08,
    });

    assert_roundtrip_value(&InterpolationResult {
        weights: vec![(2, 0.25), (3, 0.75)],
        is_extrapolation: false,
        extrapolation_distance: None,
    });
}
