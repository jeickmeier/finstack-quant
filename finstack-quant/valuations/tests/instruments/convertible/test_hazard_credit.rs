//! `ConvertibleBond.credit_curve_id` names the issuer `HazardCurve`.
//!
//! Default pays recovery times outstanding principal. Surviving equity has
//! drift `r - q + lambda`; default extinguishes the equity and debt.

use super::fixtures::*;
use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_cashflows::builder::{CouponType, FixedCouponSpec, RollRule};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    price_convertible_bond, ConvertibleBond,
};
use time::macros::date;

const RF: f64 = 0.03;
const LAMBDA: f64 = 0.02;
const KNOTS: [f64; 5] = [0.0, 1.0, 3.0, 5.0, 10.0];

/// Flat log-linear risk-free curve plus a flat zero-recovery issuer hazard
/// curve. Both curves use Act/365F, so `rf_df(t) × S(t) = exp(-(r + λ) t)`
/// equals the former log-linear risky discount curve on the same knots.
fn hazard_market(spot: f64) -> MarketContext {
    let rf = DiscountCurve::builder("USD-OIS")
        .base_date(dates::base_date())
        .knots(KNOTS.map(|t| (t, (-RF * t).exp())))
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    MarketContext::new()
        .insert(rf)
        .insert(HazardCurve::flat("USD-CREDIT", dates::base_date(), LAMBDA, 0.0).unwrap())
        .insert_price("AAPL", MarketScalar::Unitless(spot))
        .insert_price(
            "AAPL-VOL",
            MarketScalar::Unitless(market_params::VOL_STANDARD),
        )
        .insert_price(
            "AAPL-DIVYIELD",
            MarketScalar::Unitless(market_params::DIV_YIELD),
        )
}

fn credit_bond(recovery: f64) -> ConvertibleBond {
    let mut bond = with_tree_steps(&create_convertible_with_credit(), 100);
    bond.recovery_rate = Some(recovery);
    bond
}

/// With conversion only at maturity and no coupons the independent solution
/// is a survival-discounted zero plus a Black call with rate r+lambda and
/// continuous recovery of face. This tests the equity compensator and cash
/// recovery together, without pinning a previous implementation's output.
#[test]
fn convertible_frp_matches_european_closed_form() {
    use finstack_quant_core::math::norm_cdf;
    use finstack_quant_valuations::instruments::ConversionPolicy;
    for spot in [50.0, 100.0, 150.0] {
        for recovery in [0.0, 0.4, 1.0] {
            let mut bond = credit_bond(recovery);
            bond.cashflow_spec =
                finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec::default();
            bond.conversion.policy = ConversionPolicy::Window {
                start: bond.maturity,
                end: bond.maturity,
            };
            bond.instrument_pricing_overrides.model_config.tree_steps = Some(2000);
            let market = hazard_market(spot);
            let time = (bond.maturity - dates::base_date()).whole_days() as f64 / 365.0;
            let rate = RF + LAMBDA;
            let face = bond.notional.amount();
            let ratio = bond.conversion.ratio.expect("ratio");
            let strike = face / ratio;
            let vol = market_params::VOL_STANDARD;
            let dividend = market_params::DIV_YIELD;
            let d1 = ((spot / strike).ln() + (rate - dividend + 0.5 * vol * vol) * time)
                / (vol * time.sqrt());
            let d2 = d1 - vol * time.sqrt();
            let call = ratio
                * (spot * (-dividend * time).exp() * norm_cdf(d1)
                    - strike * (-rate * time).exp() * norm_cdf(d2));
            let expected = face * (-rate * time).exp()
                + call
                + recovery * face * LAMBDA / rate * (1.0 - (-rate * time).exp());
            let actual = price_convertible_bond(&bond, &market, dates::base_date())
                .expect("FRP price")
                .amount();
            assert!(
                (actual - expected).abs() < 0.15,
                "spot={spot}, R={recovery}: {actual} vs {expected}"
            );
        }
    }
}

/// QuantLib 1.43 `usd_fixed_5y_hazard` (FixedRateBond, 5% semiannual 30/360,
/// zero recovery, flat 2% hazard, flat 30/360 log-linear OIS) NPV per 100.
const QUANTLIB_HAZARD_BOND_NPV: f64 = 95.35267772630202;

fn deep_otm_convertible(steps: usize) -> ConvertibleBond {
    let mut bond = create_standard_convertible();
    bond.id = "CB-DEEP-OTM".into();
    bond.notional = Money::new(100.0, Currency::USD).unwrap();
    bond.issue_date = date!(2026 - 04 - 30);
    bond.maturity = date!(2031 - 04 - 30);
    bond.settlement_days = Some(2);
    // One millionth of a share per bond at spot 1: the equity option is
    // worthless and the convertible is its bond floor.
    bond.conversion.ratio = Some(1e-6);
    bond.credit_curve_id = Some("USD-CREDIT".into());
    bond.recovery_rate = Some(0.0);
    bond.cashflow_spec =
        finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec::Fixed(
            FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: rust_decimal::Decimal::new(5, 2),
                schedule: ScheduleParams {
                    frequency: Tenor::semi_annual(),
                    day_count: DayCount::Thirty360,
                    business_day_convention: BusinessDayConvention::Following,
                    calendar_id: "weekends_only".into(),
                    stub: StubKind::ShortFront,
                    end_of_month: false,
                    payment_lag_days: 0,
                    adjust_accrual_dates: true,
                    roll_rule: RollRule::None,
                },
            },
        );
    with_tree_steps(&bond, steps)
}

fn quantlib_hazard_market(as_of: Date) -> MarketContext {
    let ois = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Thirty360)
        .knots([(0.0, 1.0), (30.0, 0.30119421191220214)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    let hazard = HazardCurve::builder("USD-CREDIT")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 0.02), (30.0, 0.02)])
        .recovery_rate(0.0)
        .build()
        .unwrap();
    MarketContext::new()
        .insert(ois)
        .insert(hazard)
        .insert_price("AAPL", MarketScalar::Unitless(1.0))
        .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.0))
}

#[test]
fn deep_otm_zero_recovery_convertible_matches_hazard_bond() {
    let as_of = date!(2026 - 04 - 30);
    let market = quantlib_hazard_market(as_of);
    // Tolerances from measured tree convergence against QuantLib: first order
    // in the step count, with coupon dates snapped to the lattice. Measured
    // errors: 2.25e-3 (250 steps), 5.36e-4 (1000 steps) and 2.3e-12 (2000
    // steps, when every coupon date lands on a node).
    for (steps, tolerance) in [(250_usize, 5e-3), (1000, 1e-3), (2000, 1e-9)] {
        let pv = price_convertible_bond(&deep_otm_convertible(steps), &market, as_of)
            .unwrap()
            .amount();
        assert!(
            (pv - QUANTLIB_HAZARD_BOND_NPV).abs() <= tolerance,
            "steps={steps}: {pv} vs QuantLib {QUANTLIB_HAZARD_BOND_NPV}"
        );
    }
}
