//! `ConvertibleBond.credit_curve_id` names the issuer `HazardCurve`.
//!
//! The Tsiveriotis-Zhang cash component is discounted at the zero-recovery
//! risky forward `rf_fwd × S(t_{i+1}) / S(t_i)`, blended with the risk-free
//! forward by the instrument's `recovery_rate`. These tests pin that the
//! hazard curve reproduces the former risky-discount-curve prices, that full
//! recovery removes all credit effect, and that a deep out-of-the-money
//! convertible converges to the QuantLib hazard-rate bond.

use super::fixtures::*;
use finstack_quant_cashflows::builder::specs::{CouponType, FixedCouponSpec, RollRule};
use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    price_convertible_bond, ConvertibleBond, ConvertibleTreeType,
};
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
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

/// Prices and CS01s captured on the pre-change commit (ed2d9f2f3), where
/// `USD-CREDIT` was the log-linear risky discount curve `exp(-(r + λ) t)` on
/// the same knots. Entries: (spot, recovery, PV bits, CS01 bits).
const RISKY_DISCOUNT_PINS: [(f64, f64, u64, u64); 6] = [
    (150.0, 0.0, 0x409961bfaa1a0fa9, 0xbfc5fb00f0e04000),
    (150.0, 0.4, 0x409999b765dfd3ad, 0xbfbb565c4ab16000),
    (150.0, 1.0, 0x4099f18e9684bc08, 0x0000000000000000),
    (50.0, 0.0, 0x408ffea224ea8184, 0xbfdaf1a67f862c00),
    (50.0, 0.4, 0x409088b0e581080c, 0xbfd0ca86b880a800),
    (50.0, 1.0, 0x409160c62350f9e5, 0x0000000000000000),
];

#[test]
fn convertible_hazard_equals_equivalent_risky_curve() {
    for (spot, recovery, pv_bits, cs01_bits) in RISKY_DISCOUNT_PINS {
        let bond = credit_bond(recovery);
        let market = hazard_market(spot);
        let pv = price_convertible_bond(
            &bond,
            &market,
            ConvertibleTreeType::Binomial,
            dates::base_date(),
        )
        .unwrap()
        .amount();
        // The per-step risky forwards agree to float rounding (a product of
        // two exponentials vs one), so the lattice value agrees to ~1e-14
        // relative; 1e-10 relative on a ~1e3 PV leaves ample room without
        // hiding a curve-semantics difference (~1e-3 relative per 10 bp).
        let pinned_pv = f64::from_bits(pv_bits);
        assert!(
            (pv - pinned_pv).abs() <= 1e-10 * pinned_pv.abs(),
            "spot={spot} R={recovery}: {pv} vs pinned {pinned_pv}"
        );

        // CS01 now shocks the hazard curve by 1 bp of spread; with a
        // zero-recovery curve that is the same 1 bp continuous shift the
        // former risky discount curve took.
        let cs01 = *bond
            .price_with_metrics(
                &market,
                dates::base_date(),
                &[MetricId::Cs01],
                PricingOptions::default(),
            )
            .unwrap()
            .measures
            .get("cs01")
            .unwrap();
        let pinned_cs01 = f64::from_bits(cs01_bits);
        // Central difference of two ~1e3 PVs: absolute noise ~1e-12.
        assert!(
            (cs01 - pinned_cs01).abs() <= 1e-9,
            "spot={spot} R={recovery}: CS01 {cs01} vs pinned {pinned_cs01}"
        );
    }
}

#[test]
fn full_recovery_convertible_equals_risk_free() {
    for spot in [150.0, 50.0] {
        let market = hazard_market(spot);
        let with_credit = price_convertible_bond(
            &credit_bond(1.0),
            &market,
            ConvertibleTreeType::Binomial,
            dates::base_date(),
        )
        .unwrap()
        .amount();
        let mut risk_free = credit_bond(0.0);
        risk_free.credit_curve_id = None;
        risk_free.recovery_rate = None;
        let without_credit = price_convertible_bond(
            &risk_free,
            &market,
            ConvertibleTreeType::Binomial,
            dates::base_date(),
        )
        .unwrap()
        .amount();
        // R = 1 blends to exactly the risk-free forward at every step.
        assert!(
            (with_credit - without_credit).abs() <= 1e-12 * without_credit.abs(),
            "spot={spot}: {with_credit} vs {without_credit}"
        );
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
        let pv = price_convertible_bond(
            &deep_otm_convertible(steps),
            &market,
            ConvertibleTreeType::Binomial,
            as_of,
        )
        .unwrap()
        .amount();
        assert!(
            (pv - QUANTLIB_HAZARD_BOND_NPV).abs() <= tolerance,
            "steps={steps}: {pv} vs QuantLib {QUANTLIB_HAZARD_BOND_NPV}"
        );
    }
}
