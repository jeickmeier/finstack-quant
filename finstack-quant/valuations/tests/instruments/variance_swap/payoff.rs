//! Tests for variance swap payoff calculations.

use super::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::equity::variance_swap::PayReceive;

#[test]
fn test_payoff_at_the_money_is_zero() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let realized_var = swap.strike_variance;

    // Act
    let payoff = swap.payoff(realized_var);

    // Assert
    assert!(payoff.amount().abs() < EPSILON);
    assert_eq!(payoff.currency(), Currency::USD);
}

#[test]
fn test_payoff_receive_side_profits_when_realized_exceeds_strike() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let realized_var = swap.strike_variance + 0.01; // Higher realized variance

    // Act
    let payoff = swap.payoff(realized_var);

    // Assert
    assert!(payoff.amount() > 0.0);
    let expected = DEFAULT_NOTIONAL * 0.01;
    assert!((payoff.amount() - expected).abs() < EPSILON);
}

#[test]
fn test_payoff_receive_side_loses_when_realized_below_strike() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let realized_var = swap.strike_variance - 0.01; // Lower realized variance

    // Act
    let payoff = swap.payoff(realized_var);

    // Assert
    assert!(payoff.amount() < 0.0);
    let expected = -DEFAULT_NOTIONAL * 0.01;
    assert!((payoff.amount() - expected).abs() < EPSILON);
}

#[test]
fn test_payoff_pay_side_has_opposite_sign_to_receive() {
    // Arrange
    let receive = sample_swap(PayReceive::Receive);
    let pay = sample_swap(PayReceive::Pay);
    let realized_var = 0.06; // Above strike

    // Act
    let receive_payoff = receive.payoff(realized_var);
    let pay_payoff = pay.payoff(realized_var);

    // Assert
    assert!(receive_payoff.amount() > 0.0);
    assert!(pay_payoff.amount() < 0.0);
    assert!((receive_payoff.amount() + pay_payoff.amount()).abs() < EPSILON);
}

#[test]
fn test_payoff_scales_linearly_with_notional() {
    // Arrange
    let swap_1m = sample_swap(PayReceive::Receive);
    let mut swap_2m = sample_swap(PayReceive::Receive);
    swap_2m.notional = Money::new(2.0 * DEFAULT_NOTIONAL, Currency::USD);
    let realized_var = 0.05;

    // Act
    let payoff_1m = swap_1m.payoff(realized_var);
    let payoff_2m = swap_2m.payoff(realized_var);

    // Assert
    assert!((payoff_2m.amount() - 2.0 * payoff_1m.amount()).abs() < EPSILON);
}

#[test]
fn test_payoff_scales_linearly_with_variance_difference() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);
    let var_diff_small = 0.01;
    let var_diff_large = 0.02;

    // Act
    let payoff_small = swap.payoff(swap.strike_variance + var_diff_small);
    let payoff_large = swap.payoff(swap.strike_variance + var_diff_large);

    // Assert
    assert!((payoff_large.amount() - 2.0 * payoff_small.amount()).abs() < EPSILON);
}

#[test]
fn test_payoff_with_extreme_variance_values() {
    // Arrange
    let swap = sample_swap(PayReceive::Receive);

    // Act & Assert - Very high variance
    let high_var = 2.0; // 141% vol
    let payoff_high = swap.payoff(high_var);
    assert!(payoff_high.amount() > 0.0);
    assert!(payoff_high.amount().is_finite());

    // Act & Assert - Near zero variance
    let low_var = 0.0001;
    let payoff_low = swap.payoff(low_var);
    assert!(payoff_low.amount() < 0.0);
    assert!(payoff_low.amount().is_finite());
}

#[test]
fn test_payoff_preserves_currency() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.notional = Money::new(DEFAULT_NOTIONAL, Currency::EUR);
    let realized_var = 0.05;

    // Act
    let payoff = swap.payoff(realized_var);

    // Assert
    assert_eq!(payoff.currency(), Currency::EUR);
}

#[test]
fn test_payoff_calculation_matches_theoretical_formula() {
    // Arrange: Payoff = Notional * (RealizedVar - StrikeVar) * Side.sign()
    let swap = sample_swap(PayReceive::Receive);
    let realized_var = 0.0625; // 25% vol
    let strike_var = 0.04; // 20% vol
    let expected = DEFAULT_NOTIONAL * (realized_var - strike_var) * 1.0;

    // Act
    let payoff = swap.payoff(realized_var);

    // Assert
    assert!((payoff.amount() - expected).abs() < EPSILON);
}

#[test]
fn test_payoff_with_zero_notional_is_zero() {
    // Arrange
    let mut swap = sample_swap(PayReceive::Receive);
    swap.notional = Money::new(0.0, Currency::USD);
    let realized_var = 0.10;

    // Act
    let payoff = swap.payoff(realized_var);

    // Assert
    assert_eq!(payoff.amount(), 0.0);
}
