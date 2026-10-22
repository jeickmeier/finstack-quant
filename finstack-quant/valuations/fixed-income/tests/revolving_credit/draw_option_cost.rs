//! Draw option cost of a stochastic revolver: the value to the lender of
//! having committed to lend at the contractual margin when the path's fair
//! spread differs, one forward loan per simulated draw.

use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_valuations_core::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_fixed_income::revolving_credit::CreditSpreadProcessSpec;
use finstack_quant_valuations_fixed_income::revolving_credit::RevolvingCreditPricer;

use crate::support::revolver_draws::{
    market, revolver, widening_hazard_curve, AS_OF, HAZARD_ID, MARGIN_BP,
};

/// A constant spread process, at any level, prices every draw at fair value:
/// the fair spread is anchored to the margin, so the option cost is exactly
/// zero on every path, and so is the metric.
#[test]
fn constant_spread_at_any_level_has_zero_option_cost_on_every_path() {
    let facility = revolver(
        "RCF-FAIR",
        0.25,
        CreditSpreadProcessSpec::Constant(0.045),
        8,
    );
    let market = market();
    let result = RevolvingCreditPricer::price_with_paths(&facility, &market, AS_OF)
        .expect("stochastic pricing");
    assert_eq!(result.path_results.len(), 16);
    let mut drew = false;
    for path in &result.path_results {
        assert_eq!(path.draw_option_cost.amount(), 0.0);
        let utilization = &path.path_data.as_ref().expect("path data").utilization_path;
        drew |= utilization.windows(2).any(|w| w[1] > w[0] + 1e-9);
    }
    assert!(drew, "the paths must carry genuine draws");
    assert_eq!(result.draw_option_cost.mean.amount(), 0.0);
    assert_eq!(result.draw_option_cost.stderr, 0.0);

    let priced = facility
        .price_with_metrics(
            &market,
            AS_OF,
            &[MetricId::custom("draw_option_cost")],
            PricingOptions::default(),
        )
        .expect("metric");
    assert_eq!(priced.measures.get("draw_option_cost"), Some(&0.0));
}

/// On a market-anchored path that widens, draws are made at a margin below
/// the fair spread, so the lender's option cost is negative.
#[test]
fn widening_market_anchored_spread_gives_a_negative_option_cost() {
    let facility = revolver(
        "RCF-WIDEN",
        0.25,
        CreditSpreadProcessSpec::MarketAnchored {
            credit_curve_id: HAZARD_ID.into(),
            kappa: 0.5,
            implied_vol: 0.4,
            tenor_years: None,
        },
        32,
    );
    let market = market().insert(widening_hazard_curve());
    let result = RevolvingCreditPricer::price_with_paths(&facility, &market, AS_OF)
        .expect("stochastic pricing");
    let cost = result.draw_option_cost.mean.amount();
    assert!(cost < 0.0, "expected a negative option cost, got {cost}");
    assert!(
        result.draw_option_cost.ci_95.1.amount() < 0.0,
        "the whole confidence interval must be negative: {:?}",
        result.draw_option_cost.ci_95
    );
    assert_eq!(result.draw_option_cost.num_paths, 32);
    assert!(result
        .path_results
        .iter()
        .all(|p| p.draw_option_cost.amount().is_finite()));

    let priced = facility
        .price_with_metrics(
            &market,
            AS_OF,
            &[MetricId::custom("draw_option_cost")],
            PricingOptions::default(),
        )
        .expect("metric");
    let metric = priced
        .measures
        .get("draw_option_cost")
        .copied()
        .expect("metric present");
    assert!(
        (metric - cost).abs() < 1e-9,
        "metric {metric} vs result {cost}"
    );
}

/// The option cost depends on spread changes since the valuation date, not
/// on the margin level: two facilities differing only in their margin carry
/// identical draw option costs on every path.
#[test]
fn draw_option_cost_is_independent_of_the_margin_level() {
    let market = market().insert(widening_hazard_curve());
    let spread = || CreditSpreadProcessSpec::MarketAnchored {
        credit_curve_id: HAZARD_ID.into(),
        kappa: 0.5,
        implied_vol: 0.4,
        tenor_years: None,
    };
    // One id for both, so they share the derived seed (common random numbers).
    let tight = revolver("RCF-MARGIN", 0.25, spread(), 16);
    let mut wide = revolver("RCF-MARGIN", 0.25, spread(), 16);
    if let RateSpec::Floating(spec) = &mut wide.rate {
        spec.spread_bp = rust_decimal::Decimal::from(MARGIN_BP + 400);
    }
    let a = RevolvingCreditPricer::price_with_paths(&tight, &market, AS_OF).expect("tight");
    let b = RevolvingCreditPricer::price_with_paths(&wide, &market, AS_OF).expect("wide");
    for (pa, pb) in a.path_results.iter().zip(&b.path_results) {
        assert!(
            (pa.draw_option_cost.amount() - pb.draw_option_cost.amount()).abs() < 1e-6,
            "{} vs {}",
            pa.draw_option_cost,
            pb.draw_option_cost
        );
    }
    assert!(a.draw_option_cost.mean.amount() < 0.0);
}
