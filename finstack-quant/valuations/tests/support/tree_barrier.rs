//! Tests for the surrounding crate component and its documented behavior.
//!
use crate::models::trees::BinomialTree;
use crate::instruments::OptionMarketParams;

#[test]
fn test_up_and_out_call_reduces_value_with_low_barrier() {
    let market = OptionMarketParams::call(100.0, 100.0, 0.05, 0.20, 1.0);
    let tree = BinomialTree::crr(150);

    let vanilla = tree.price_european(&market).unwrap();
    // Low barrier near spot: likely to hit, so value should move toward rebate (0)
    let up_and_out = tree
        .price_barrier_out(&market, Some(101.0), None, 0.0)
        .unwrap();

    assert!(up_and_out <= vanilla);
}

#[test]
fn test_in_out_parity_single_barrier() {
    let market = OptionMarketParams::put(100.0, 95.0, 0.03, 0.25, 0.5);
    let tree = BinomialTree::crr(200);

    let vanilla = tree.price_european(&market).unwrap();
    let down_and_out = tree
        .price_barrier_out(&market, None, Some(99.0), 0.0)
        .unwrap();
    let down_and_in = tree
        .price_barrier_in(&market, None, Some(99.0), 0.0)
        .unwrap();

    let parity_diff = (vanilla - (down_and_out + down_and_in)).abs();
    assert!(parity_diff < 1e-6);
}

#[test]
fn test_american_knock_in_put_ge_european() {
    let market = OptionMarketParams::put(100.0, 105.0, 0.03, 0.25, 1.0);
    let tree = BinomialTree::crr(200);

    let european = tree
        .price_barrier_in(&market, None, Some(95.0), 0.0)
        .unwrap();
    let american = tree
        .price_barrier_in_american(&market, None, Some(95.0), 0.0)
        .unwrap();

    assert!(
        american + 1e-8 >= european,
        "American knock-in should not be less than European"
    );
}

#[test]
fn test_knock_in_requires_exactly_one_barrier_level() {
    let market = OptionMarketParams::call(100.0, 100.0, 0.05, 0.20, 1.0);
    let tree = BinomialTree::crr(100);

    let neither = tree.price_barrier_in(&market, None, None, 0.0);
    let both = tree.price_barrier_in(&market, Some(120.0), Some(80.0), 0.0);

    assert!(neither.is_err(), "knock-in without a barrier should fail");
    assert!(both.is_err(), "knock-in with two barriers should fail");
}

#[test]
fn test_bermudan_knock_in_lies_between_european_and_american() {
    let market = OptionMarketParams::put(100.0, 105.0, 0.03, 0.25, 1.0);
    let tree = BinomialTree::crr(200);
    let exercise_dates = vec![0.25, 0.5, 0.75, 1.0];

    let european = tree
        .price_barrier_in(&market, None, Some(95.0), 0.0)
        .unwrap();
    let bermudan = tree
        .price_barrier_in_bermudan(&market, None, Some(95.0), 0.0, &exercise_dates)
        .unwrap();
    let american = tree
        .price_barrier_in_american(&market, None, Some(95.0), 0.0)
        .unwrap();

    assert!(bermudan + 1e-8 >= european);
    assert!(american + 1e-8 >= bermudan);
}
