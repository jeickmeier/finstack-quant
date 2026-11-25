//! Registry-level tests for the Tsiveriotis-Fernandes convertible model.
//!
//! The lattice itself is unit-tested beside the engine. These tests check
//! what only the registry can show: that selecting the model reaches the
//! pricer and every convertible metric, and that an unsupported model is
//! rejected.

use super::fixtures::*;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::ModelKey;

const TREE_STEPS: usize = 100;

fn options(model: ModelKey) -> finstack_quant_valuations::instruments::PricingOptions {
    crate::instruments::test_support::credit::pricing_options().with_model(model)
}

fn calibrated_credit_market() -> finstack_quant_core::market_data::context::MarketContext {
    let market = create_market_context();
    let hazard = crate::instruments::test_support::credit::calibrated_hazard_curve(
        &market,
        dates::base_date(),
        "USD-CREDIT",
        "ISSUER",
        "USD-OIS",
    )
    .expect("replayable credit curve");
    market.try_insert(hazard).expect("insert market curve")
}

fn recovering_bond()
-> finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond {
    let mut bond = with_tree_steps(&create_convertible_with_credit(), TREE_STEPS);
    bond.recovery_rate = Some(0.4);
    bond
}

#[test]
fn selecting_the_model_changes_the_price_and_every_credit_dependent_metric() {
    let bond = recovering_bond();
    let market = calibrated_credit_market();
    let metrics = [
        MetricId::BondFloor,
        MetricId::Delta,
        MetricId::Vega,
        MetricId::Dv01,
        MetricId::Cs01,
    ];
    let price = |model| {
        bond.price_with_metrics(&market, dates::base_date(), &metrics, options(model))
            .expect("priced with metrics")
    };
    let tree = price(ModelKey::Tree);
    let tf = price(ModelKey::TsiveriotisFernandes);

    assert!(
        (tree.value.amount() - tf.value.amount()).abs() > 1e-6 * tree.value.amount(),
        "the two credit models must not give the same price"
    );
    for metric in ["bond_floor", "delta", "vega", "dv01", "cs01"] {
        let (tree_value, tf_value) = (tree.measures[metric], tf.measures[metric]);
        assert!(tf_value.is_finite(), "{metric} under tsiveriotis_fernandes");
        assert!(
            (tree_value - tf_value).abs() > 1e-9 * tree_value.abs(),
            "{metric} did not follow the selected model: {tree_value} for both"
        );
    }
    // With 40% recovery the spread is 60% of the hazard, and the default
    // model also pays recovery, so the two floors differ; both stay below
    // the price.
    assert!(tf.measures["bond_floor"] < tf.value.amount());
    // A wider spread lowers the value of the cash component.
    assert!(tf.measures["cs01"] < 0.0);
}

#[test]
fn an_unsupported_model_is_rejected() {
    let bond = recovering_bond();
    let market = calibrated_credit_market();
    let error = bond
        .price_with_metrics(
            &market,
            dates::base_date(),
            &[],
            options(ModelKey::HullWhite1F),
        )
        .expect_err("convertibles have no Hull-White pricer");
    assert!(!error.to_string().is_empty());
}
