//! Smoke test: the shared builders compose into a priceable market context.

use finstack_quant_valuations_test_support::builders::TestMarketBuilder;
use finstack_quant_valuations_test_support::fixtures::{base_date, usd_discount_curve};
use finstack_quant_valuations_test_support::test_helpers::flat_discount_curve;
use finstack_quant_valuations_test_support::{assert_parity, parity::ParityConfig};

#[test]
fn builders_and_fixtures_agree_on_the_discount_factor() {
    let as_of = base_date();
    let market = TestMarketBuilder::new(as_of).rate(0.05).build();
    let from_builder = market
        .get_discount("USD-OIS")
        .expect("builder writes USD-OIS")
        .df(1.0);
    let from_helper = flat_discount_curve(0.05, as_of, "USD-OIS").df(1.0);
    assert_parity!(
        from_builder,
        from_helper,
        ParityConfig::default(),
        "flat 5% one-year DF"
    );
    assert!(usd_discount_curve(as_of, "USD-OIS").df(1.0) < 1.0);
}
