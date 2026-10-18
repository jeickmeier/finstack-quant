//! Stochastic revolver fixtures: a floating facility at a fixed margin over
//! the term index with a mean-reverting utilisation process and a chosen
//! credit-spread process, plus the market it prices on. Shared by the
//! revolver draw-option tests and, by `#[path]` include, by the
//! structured-credit crate's loan-pool tests.

use finstack_quant_cashflows::builder::FloatingRateSpec;
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::instruments::InstrumentPricingOverrides;
use finstack_quant_valuations_fixed_income::revolving_credit::{
    CreditSpreadProcessSpec, DrawRepaySpec, McConfig, RevolvingCredit, RevolvingCreditFees,
    StochasticUtilizationSpec, UtilizationProcess,
};
use time::macros::date;

use finstack_quant_valuations_test_support::test_helpers::{
    flat_discount_curve, flat_forward_curve,
};

pub(crate) const AS_OF: Date = date!(2024 - 01 - 15);
const MATURITY: Date = date!(2027 - 01 - 15);
pub(crate) const HAZARD_ID: &str = "BORROWER-HZ";
pub(crate) const MARGIN_BP: i64 = 250;

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
        forward_curve_id: "USD-SOFR-3M".into(),
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
        compounding: None,
        overnight_basis: None,
        fallback: Default::default(),
    }
}

/// Floating revolver at `MARGIN_BP` over the term index with the given
/// utilization volatility and spread process; the hazard curve is attached
/// only for the market-anchored process. `num_paths` antithetic estimators
/// simulate `2 × num_paths` paths.
pub(crate) fn revolver(
    id: &str,
    utilization_vol: f64,
    spread: CreditSpreadProcessSpec,
    num_paths: usize,
) -> RevolvingCredit {
    let anchored = matches!(spread, CreditSpreadProcessSpec::MarketAnchored { .. });
    let mut builder = RevolvingCredit::builder()
        .id(id.into())
        .commitment(usd(50_000_000.0))
        .drawn(usd(10_000_000.0))
        .issue_date(AS_OF)
        .maturity(MATURITY)
        .rate(RateSpec::Floating(floating_spec()))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0).expect("fees"))
        .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
            StochasticUtilizationSpec {
                utilization_process: UtilizationProcess::MeanReverting {
                    theta: 0.6,
                    kappa: 1.0,
                    sigma: utilization_vol,
                    spread_sensitivity: 0.0,
                },
                mc_config: Some(McConfig {
                    correlation_matrix: None,
                    credit_spread_process: spread,
                    interest_rate_process: None,
                    util_credit_corr: Some(0.5),
                }),
            },
        )))
        .instrument_pricing_overrides(
            InstrumentPricingOverrides::default()
                .with_mc_paths(num_paths)
                .with_mc_antithetic(true),
        )
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.4);
    if anchored {
        builder = builder.credit_curve_id(HAZARD_ID.into());
    }
    builder.build().expect("stochastic revolver")
}
