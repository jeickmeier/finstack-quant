//! Draw option cost of a stochastic revolver: the value to the lender of
//! having committed to lend at the contractual margin when the path's fair
//! spread differs, one forward loan per simulated draw.

use finstack_quant_cashflows::builder::FloatingRateSpec;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, CreditSpreadProcessSpec, DrawRepaySpec, McConfig, RevolvingCredit,
    RevolvingCreditFees, RevolvingCreditPricer, StochasticUtilizationSpec, UtilizationProcess,
};
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

use crate::common::test_helpers::{flat_discount_curve, flat_forward_curve};

const AS_OF: Date = date!(2024 - 01 - 15);
const MATURITY: Date = date!(2027 - 01 - 15);
pub(crate) const HAZARD_ID: &str = "BORROWER-HZ";
const MARGIN_BP: i64 = 250;

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("valid money fixture")
}

pub(crate) fn market() -> MarketContext {
    let fixings: Vec<(Date, f64)> = (0..25)
        .map(|days| (AS_OF - time::Duration::days(days), 0.04))
        .collect();
    MarketContext::new()
        .insert(flat_discount_curve(0.03, AS_OF, "USD-OIS"))
        .insert(flat_forward_curve(0.04, AS_OF, "USD-SOFR-3M"))
        .insert_series(ScalarTimeSeries::new("FIXING:USD-SOFR-3M", fixings, None).expect("fixings"))
}

/// Borrower hazard curve whose term structure rises, so the market-anchored
/// spread process reverts above its starting level.
pub(crate) fn widening_hazard_curve() -> HazardCurve {
    HazardCurve::builder(HAZARD_ID)
        .base_date(AS_OF)
        .knots([(1.0, 0.04), (3.0, 0.10), (5.0, 0.14)])
        .recovery_rate(0.4)
        .build()
        .expect("hazard curve")
}

pub(crate) fn floating_spec() -> FloatingRateSpec {
    FloatingRateSpec {
        index_id: "USD-SOFR-3M".into(),
        spread_bp: rust_decimal::Decimal::from(MARGIN_BP),
        gearing: rust_decimal::Decimal::ONE,
        gearing_includes_spread: true,
        index_floor_bp: None,
        all_in_floor_bp: None,
        all_in_cap_bp: None,
        index_cap_bp: None,
        overnight_index_constraints: Default::default(),
        reset_frequency: Tenor::quarterly(),
        index_tenor: None,
        reset_lag_days: 2,
        fixing_calendar_id: None,
        overnight_compounding: None,
        overnight_basis: None,
        fallback: Default::default(),
    }
}

/// Floating revolver at `MARGIN_BP` over the term index with the given
/// utilization volatility and spread process; the hazard curve is attached
/// only for the market-anchored process.
pub(crate) fn revolver(
    id: &str,
    utilization_vol: f64,
    spread: CreditSpreadProcessSpec,
    num_paths: usize,
) -> RevolvingCredit {
    let anchored = matches!(spread, CreditSpreadProcessSpec::MarketAnchored { .. });
    let mut builder = RevolvingCredit::builder()
        .id(id.into())
        .commitment_amount(usd(50_000_000.0))
        .drawn_amount(usd(10_000_000.0))
        .commitment_date(AS_OF)
        .maturity(MATURITY)
        .base_rate_spec(BaseRateSpec::Floating(floating_spec()))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0).expect("fees"))
        .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
            StochasticUtilizationSpec {
                utilization_process: UtilizationProcess::MeanReverting {
                    target_rate: 0.6,
                    speed: 1.0,
                    volatility: utilization_vol,
                    spread_sensitivity: 0.0,
                },
                num_paths,
                seed: Some(42),
                antithetic: true,
                use_sobol_qmc: false,
                mc_config: Some(McConfig {
                    correlation_matrix: None,
                    recovery_rate: 0.4,
                    credit_spread_process: spread,
                    interest_rate_process: None,
                    util_credit_corr: Some(0.5),
                }),
            },
        )))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.4);
    if anchored {
        builder = builder.credit_curve_id(HAZARD_ID.into());
    }
    builder.build().expect("stochastic revolver")
}

/// A constant spread equal to the contractual margin prices every draw at
/// fair value: the option cost is exactly zero on every path, and so is the
/// metric.
#[test]
fn constant_spread_at_the_margin_has_zero_option_cost_on_every_path() {
    let facility = revolver(
        "RCF-FAIR",
        0.25,
        CreditSpreadProcessSpec::Constant(f64::from(MARGIN_BP as i32) / 10_000.0),
        16,
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
        64,
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
