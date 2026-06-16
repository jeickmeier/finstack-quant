//! Tests for variance swap construction and builder pattern.

use super::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::equity::variance_swap::{
    PayReceive, RealizedVarMethod, VarianceSwap,
};
use finstack_quant_valuations::instruments::{Attributes, Instrument};

#[test]
fn test_builder_creates_valid_swap_with_all_required_fields() {
    // Arrange
    let (start, end) = default_dates();

    // Act
    let swap = VarianceSwap::builder()
        .id(InstrumentId::new("VAR-TEST-001"))
        .underlying_ticker(UNDERLYING_ID.to_string())
        .notional(Money::new(DEFAULT_NOTIONAL, Currency::USD))
        .strike_variance(DEFAULT_STRIKE_VAR)
        .start_date(start)
        .maturity(end)
        .observation_freq(Tenor::daily())
        .realized_var_method(RealizedVarMethod::CloseToClose)
        .side(PayReceive::Receive)
        .discount_curve_id(CurveId::new(DISC_ID))
        .day_count(DayCount::Act365F)
        .attributes(Attributes::new())
        .build();

    // Assert
    assert!(swap.is_ok());
    let swap = swap.unwrap();
    assert_eq!(swap.id.as_str(), "VAR-TEST-001");
    assert_eq!(swap.underlying_ticker, UNDERLYING_ID);
    assert_eq!(swap.notional.amount(), DEFAULT_NOTIONAL);
    assert_eq!(swap.strike_variance, DEFAULT_STRIKE_VAR);
    assert_eq!(swap.start_date, start);
    assert_eq!(swap.maturity, end);
}

#[test]
fn test_builder_creates_receive_and_pay_sides_correctly() {
    // Act
    let receive = sample_swap(PayReceive::Receive);
    let pay = sample_swap(PayReceive::Pay);

    // Assert
    assert!(matches!(receive.side, PayReceive::Receive));
    assert!(matches!(pay.side, PayReceive::Pay));
    assert_eq!(receive.side.sign(), 1.0);
    assert_eq!(pay.side.sign(), -1.0);
}

#[test]
fn test_strike_variance_stores_variance_not_volatility() {
    // Arrange
    let vol = 0.20;
    let var = vol * vol;

    // Act
    let swap = sample_swap(PayReceive::Receive);

    // Assert
    assert!((swap.strike_variance - var).abs() < EPSILON);
    assert!((swap.strike_variance.sqrt() - vol).abs() < EPSILON);
}

#[test]
fn test_different_observation_frequencies_are_supported() {
    // Arrange & Act
    let frequencies = vec![
        Tenor::daily(),
        Tenor::weekly(),
        Tenor::monthly(),
        Tenor::quarterly(),
        Tenor::semi_annual(),
    ];

    // Assert
    for freq in frequencies {
        let mut swap = sample_swap(PayReceive::Receive);
        swap.observation_freq = freq;
        let dates = swap.observation_dates();
        assert!(!dates.is_empty());
        assert!(dates.len() >= 2); // At minimum start and end
    }
}

#[test]
fn test_different_realized_variance_methods_are_supported() {
    // Arrange & Act
    let methods = vec![
        RealizedVarMethod::CloseToClose,
        RealizedVarMethod::Parkinson,
        RealizedVarMethod::GarmanKlass,
        RealizedVarMethod::RogersSatchell,
        RealizedVarMethod::YangZhang,
    ];

    // Assert
    for method in methods {
        let mut swap = sample_swap(PayReceive::Receive);
        swap.realized_var_method = method;
        assert_eq!(swap.realized_var_method, method);
    }
}

#[test]
fn test_different_day_count_conventions_are_supported() {
    // Arrange
    let conventions = vec![
        DayCount::Act365F,
        DayCount::Act360,
        DayCount::ActActIsma,
        DayCount::Thirty360,
    ];

    // Act & Assert
    for dc in conventions {
        let mut swap = sample_swap(PayReceive::Receive);
        swap.day_count = dc;
        assert_eq!(swap.day_count, dc);
    }
}

#[test]
fn test_instrument_trait_returns_correct_id_and_type() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);

    // Act
    let id = swap.id();
    let key = swap.key();

    // Assert
    assert!(id.contains("VAR"));
    assert_eq!(
        key,
        finstack_quant_valuations::pricer::InstrumentType::VarianceSwap
    );
}

#[test]
fn test_attributes_can_be_accessed_and_modified() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);

    // Act
    let attrs = swap.attributes_mut();
    attrs
        .meta
        .insert("book".to_string(), "EQUITY_VOLS".to_string());

    // Assert
    assert_eq!(
        swap.attributes().meta.get("book"),
        Some(&"EQUITY_VOLS".to_string())
    );
}

#[test]
fn test_discount_curve_id_accessor_returns_correct_value() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);

    // Act
    let discount_curve_id = swap.discount_curve_id();

    // Assert
    assert_eq!(discount_curve_id.as_str(), DISC_ID);
}

#[test]
fn test_clone_produces_independent_copy() {
    // Arrange
    let original = sample_swap(PayReceive::Receive);

    // Act
    let mut cloned = original.clone();
    cloned.strike_variance = 0.09;

    // Assert
    assert_eq!(original.strike_variance, DEFAULT_STRIKE_VAR);
    assert_eq!(cloned.strike_variance, 0.09);
}

#[test]
fn test_pay_receive_display_formatting() {
    // Act
    let pay_str = format!("{}", PayReceive::Pay);
    let recv_str = format!("{}", PayReceive::Receive);

    // Assert
    assert_eq!(pay_str, "pay");
    assert_eq!(recv_str, "receive");
}

#[test]
fn test_pay_receive_from_str_parses_valid_inputs() {
    // Arrange
    use std::str::FromStr;

    // Act & Assert
    assert!(matches!(PayReceive::from_str("pay"), Ok(PayReceive::Pay)));
    assert!(matches!(PayReceive::from_str("payer"), Ok(PayReceive::Pay)));
    assert!(matches!(PayReceive::from_str("short"), Ok(PayReceive::Pay)));
    assert!(matches!(
        PayReceive::from_str("receive"),
        Ok(PayReceive::Receive)
    ));
    assert!(matches!(
        PayReceive::from_str("receiver"),
        Ok(PayReceive::Receive)
    ));
    assert!(matches!(
        PayReceive::from_str("long"),
        Ok(PayReceive::Receive)
    ));
    assert!(matches!(
        PayReceive::from_str("LONG"),
        Ok(PayReceive::Receive)
    ));
}

#[test]
fn test_pay_receive_from_str_rejects_invalid_inputs() {
    // Arrange
    use std::str::FromStr;

    // Act & Assert
    assert!(PayReceive::from_str("invalid").is_err());
    assert!(PayReceive::from_str("").is_err());
    assert!(PayReceive::from_str("nonsense").is_err());
}
