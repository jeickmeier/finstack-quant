//! `recovery01` and `jump_to_default` on hazard-priced bonds and loans.
//!
//! Every curve here is hand-built, so `recovery01` is the frozen-curve bump: the
//! hazard rates stay put and only the recovery moves. The recalibrated case needs
//! the calibration crate and lives in the umbrella crate's bond metric tests.

use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, Rate};
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_core::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_core::pricer::ModelKey;
use finstack_quant_valuations_fixed_income::bond::{Bond, CallPut, CallPutSchedule};
use finstack_quant_valuations_fixed_income::revolving_credit::{
    DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use finstack_quant_valuations_fixed_income::term_loan::TermLoan;
use time::macros::date;

const AS_OF: Date = date!(2025 - 01 - 01);
const RATE: f64 = 0.03;
const HAZARD: f64 = 0.02;
const RECOVERY: f64 = 0.4;
const NOTIONAL: f64 = 1_000_000.0;

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn hazard_curve(recovery: f64) -> HazardCurve {
    HazardCurve::builder("USD-CREDIT")
        .base_date(AS_OF)
        .day_count(DayCount::Act365F)
        .recovery_rate(recovery)
        .knots([(0.0, HAZARD), (10.0, HAZARD)])
        .build()
        .expect("hazard curve")
}

fn market_at(recovery: f64) -> MarketContext {
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(AS_OF)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (10.0, (-RATE * 10.0).exp())])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("discount curve");
    MarketContext::new()
        .insert(discount)
        .insert(hazard_curve(recovery))
}

fn market() -> MarketContext {
    market_at(RECOVERY)
}

fn bond_with_coupon(coupon: f64) -> Bond {
    let mut bond = Bond::fixed(
        "DEFAULT-RISK",
        usd(NOTIONAL),
        Rate::from_decimal(coupon).expect("rate"),
        AS_OF,
        date!(2030 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.settlement_convention = None;
    bond.credit_curve_id = Some(CurveId::new("USD-CREDIT"));
    bond
}

fn zero_coupon() -> Bond {
    let mut bond = Bond::zero_coupon(
        "DEFAULT-RISK-ZERO",
        usd(NOTIONAL),
        AS_OF,
        date!(2030 - 01 - 01),
        finstack_quant_core::math::Compounding::Periodic(std::num::NonZeroU32::MIN),
        "USD-OIS",
    )
    .expect("zero");
    bond.settlement_convention = None;
    bond.credit_curve_id = Some(CurveId::new("USD-CREDIT"));
    bond
}

fn term_loan() -> TermLoan {
    let mut loan = TermLoan::example().expect("loan");
    loan.issue_date = AS_OF;
    loan.maturity = date!(2030 - 01 - 01);
    loan.credit_curve_id = Some(CurveId::new("USD-CREDIT"));
    loan
}

fn revolver() -> RevolvingCredit {
    RevolvingCredit::builder()
        .id("DEFAULT-RISK-RCF".into())
        .commitment(usd(10_000_000.0))
        .drawn(usd(4_000_000.0))
        .issue_date(AS_OF)
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .frequency(Tenor::quarterly())
        .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .credit_curve_id_opt(Some("USD-CREDIT".into()))
        .recovery_rate(RECOVERY)
        .leq(0.5)
        .build()
        .expect("revolver")
}

fn measure(instrument: &dyn Instrument, market: &MarketContext, id: MetricId) -> f64 {
    instrument
        .price_with_metrics(
            market,
            AS_OF,
            std::slice::from_ref(&id),
            PricingOptions::default(),
        )
        .expect("metric")
        .measures[id.as_str()]
}

fn error_of(instrument: &dyn Instrument, market: &MarketContext, id: MetricId) -> String {
    instrument
        .price_with_metrics(market, AS_OF, &[id], PricingOptions::default())
        .expect_err("not hazard-priced")
        .to_string()
}

/// Closed form of a zero-coupon bond under flat `r`, flat `λ` and recovery `R`.
///
/// Survival to `t` is `e^(−λt)` and default in `dt` has density `λ e^(−λt)`, so
/// discounting the promised par at `T` and `R·N` paid at default gives
///
/// ```text
/// PV = N e^(−(r+λ)T) + R N ∫₀ᵀ λ e^(−(r+λ)t) dt
///    = N e^(−(r+λ)T) + R N λ/(r+λ) (1 − e^(−(r+λ)T))
/// ```
///
/// The PV is linear in `R` on a frozen curve, so
/// `recovery01 = 0.01 N λ/(r+λ) (1 − e^(−(r+λ)T))`, and an immediate default pays
/// `R N`, so `jump_to_default = R N − PV`.
fn zero_coupon_closed_form() -> (f64, f64, f64) {
    let t = (date!(2030 - 01 - 01) - AS_OF).whole_days() as f64 / 365.0;
    let total = RATE + HAZARD;
    let default_leg = NOTIONAL * HAZARD / total * (1.0 - (-total * t).exp());
    let pv = NOTIONAL * (-total * t).exp() + RECOVERY * default_leg;
    (pv, 0.01 * default_leg, RECOVERY * NOTIONAL - pv)
}

#[test]
fn zero_coupon_matches_the_closed_form() {
    let (pv, recovery01, jump) = zero_coupon_closed_form();
    let bond = zero_coupon();
    let market = market();
    assert!((bond.value_raw(&market, AS_OF).expect("pv") - pv).abs() < 1e-6);
    let got = measure(&bond, &market, MetricId::Recovery01);
    assert!((got - recovery01).abs() < 1e-6, "{got} vs {recovery01}");
    let got = measure(&bond, &market, MetricId::JumpToDefault);
    assert!((got - jump).abs() < 1e-6, "{got} vs {jump}");
    // Inputs: 5y, r 3%, λ 2%, R 40%, N 1mm.
    assert!((recovery01 - 885.223_578).abs() < 1e-3, "{recovery01}");
    assert!((jump + 414_103.048_485).abs() < 1e-3, "{jump}");
}

#[test]
fn recovery01_is_the_central_difference_of_the_full_pv() {
    for coupon in [0.0, 0.05] {
        let bond = bond_with_coupon(coupon);
        let up = bond
            .value_raw(&market_at(RECOVERY + 0.01), AS_OF)
            .expect("up");
        let down = bond
            .value_raw(&market_at(RECOVERY - 0.01), AS_OF)
            .expect("down");
        let expected = (up - down) / 2.0;
        let got = measure(&bond, &market(), MetricId::Recovery01);
        assert!((got - expected).abs() < 1e-9, "{got} vs {expected}");
        assert!(got > 0.0, "a frozen curve gains from recovery: {got}");
    }
}

#[test]
fn coupon_bond_jump_is_recovery_of_par_with_no_accrued_less_value() {
    let bond = bond_with_coupon(0.05);
    let market = market();
    let seasoned = date!(2025 - 03 - 15);
    let pv = bond.value_raw(&market, seasoned).expect("pv");
    let result = bond
        .price_with_metrics(
            &market,
            seasoned,
            &[MetricId::JumpToDefault],
            PricingOptions::default(),
        )
        .expect("jump");
    let got = result.measures["jump_to_default"];
    assert!((got - (RECOVERY * NOTIONAL - pv)).abs() < 1e-6, "{got}");
    assert!(got < 0.0, "a long holder loses on default: {got}");
}

#[test]
fn matured_bond_has_no_claim_and_no_value() {
    let bond = bond_with_coupon(0.05);
    let result = bond
        .price_with_metrics(
            &market(),
            date!(2030 - 06 - 01),
            &[MetricId::JumpToDefault],
            PricingOptions::default(),
        )
        .expect("jump");
    assert_eq!(result.measures["jump_to_default"], 0.0);
}

#[test]
fn recovery_at_zero_takes_the_one_sided_difference() {
    let market = market_at(0.0);
    let bond = zero_coupon();
    let base = bond.value_raw(&market, AS_OF).expect("base");
    let up = bond.value_raw(&market_at(0.01), AS_OF).expect("up");
    let got = measure(&bond, &market, MetricId::Recovery01);
    assert!((got - (up - base)).abs() < 1e-6, "{got}");
}

#[test]
fn quoted_bond_keeps_a_recovery01_and_jumps_from_the_quote() {
    let mut bond = bond_with_coupon(0.05);
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price_pct(95.0);
    let market = market();
    let recovery01 = measure(&bond, &market, MetricId::Recovery01);
    assert!(recovery01 > 0.0, "the quote must not pin it to zero");
    let jump = measure(&bond, &market, MetricId::JumpToDefault);
    let quoted_value = bond.value_raw(&market, AS_OF).expect("pv");
    assert!((quoted_value - 950_000.0).abs() < 1e-6, "{quoted_value}");
    assert!((jump - (RECOVERY * NOTIONAL - quoted_value)).abs() < 1e-6);
}

#[test]
fn rates_credit_callable_is_hazard_priced() {
    let mut bond = bond_with_coupon(0.05);
    // One year, because a daily rates-credit tree over five takes seconds a price.
    bond.maturity = date!(2026 - 01 - 01);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: date!(2025 - 07 - 01),
            end: date!(2025 - 07 - 01),
            price_pct_of_par: 101.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond.instrument_pricing_overrides = InstrumentPricingOverrides::default()
        .with_hw1f_sigma(0.01)
        .with_hw1f_mean_reversion(0.03);
    let market = market();
    let pv = bond.value_raw(&market, AS_OF).expect("pv");
    let up = bond.value_raw(&market_at(0.41), AS_OF).expect("up");
    let down = bond.value_raw(&market_at(0.39), AS_OF).expect("down");
    let got = measure(&bond, &market, MetricId::Recovery01);
    assert!((got - (up - down) / 2.0).abs() < 1e-9, "{got}");
    assert!(got > 0.0, "{got}");
    let jump = measure(&bond, &market, MetricId::JumpToDefault);
    assert!((jump - (RECOVERY * NOTIONAL - pv)).abs() < 1e-6, "{jump}");
}

#[test]
fn a_bond_not_on_a_hazard_curve_is_an_error_and_its_other_metrics_stand() {
    let market = market();
    let mut risk_free = bond_with_coupon(0.05);
    risk_free.credit_curve_id = None;
    for id in [MetricId::Recovery01, MetricId::JumpToDefault] {
        assert!(error_of(&risk_free, &market, id).contains("requires hazard pricing"));
    }
    assert!(measure(&risk_free, &market, MetricId::Dv01) != 0.0);

    // A hazard curve named, with the discounting model selected, is not priced on it.
    let bond = bond_with_coupon(0.05);
    let error = bond
        .price_with_metrics(
            &market,
            AS_OF,
            &[MetricId::Recovery01],
            PricingOptions::default().with_model(ModelKey::Discounting),
        )
        .expect_err("discounting ignores the curve")
        .to_string();
    assert!(error.contains("requires hazard pricing"), "{error}");
}

#[test]
fn notional_cannot_be_negative_so_a_short_is_a_position() {
    let mut bond = bond_with_coupon(0.05);
    bond.notional = usd(-NOTIONAL);
    assert!(bond.value_raw(&market(), AS_OF).is_err());
    let mut loan = term_loan();
    loan.notional_limit = usd(-NOTIONAL);
    assert!(loan.value_raw(&market(), AS_OF).is_err());
}

#[test]
fn term_loan_recovery01_is_the_central_difference_and_jump_is_recovery_of_balance() {
    let loan = term_loan();
    let market = market();
    let up = loan.value_raw(&market_at(0.41), AS_OF).expect("up");
    let down = loan.value_raw(&market_at(0.39), AS_OF).expect("down");
    let got = measure(&loan, &market, MetricId::Recovery01);
    assert!((got - (up - down) / 2.0).abs() < 1e-9, "{got}");
    assert!(got > 0.0, "{got}");

    let pv = loan.value_raw(&market, AS_OF).expect("pv");
    let jump = measure(&loan, &market, MetricId::JumpToDefault);
    let balance = loan.notional_limit.amount();
    assert!((jump - (RECOVERY * balance - pv)).abs() < 1e-6, "{jump}");
    assert!(jump < 0.0, "{jump}");
}

#[test]
fn term_loan_without_a_credit_curve_is_an_error() {
    let mut loan = term_loan();
    loan.credit_curve_id = None;
    let market = market();
    for id in [MetricId::Recovery01, MetricId::JumpToDefault] {
        assert!(error_of(&loan, &market, id).contains("requires hazard pricing"));
    }
    assert!(measure(&loan, &market, MetricId::Ytm).is_finite());
}

#[test]
fn revolver_bumps_its_own_recovery_rate_not_the_curve() {
    let facility = revolver();
    // The curve's recovery differs from the facility's, and moves no PV.
    let market = market_at(0.25);
    assert_eq!(
        facility.value_raw(&market, AS_OF).expect("pv"),
        facility.value_raw(&market_at(0.75), AS_OF).expect("pv")
    );
    let pv_at = |recovery: f64| {
        let mut bumped = facility.clone();
        bumped.recovery_rate = recovery;
        bumped.value_raw(&market, AS_OF).expect("pv")
    };
    let expected = (pv_at(RECOVERY + 0.01) - pv_at(RECOVERY - 0.01)) / 2.0;
    let got = measure(&facility, &market, MetricId::Recovery01);
    assert!((got - expected).abs() < 1e-9, "{got} vs {expected}");
    assert!(got > 0.0, "{got}");
}

#[test]
fn revolver_jump_recovers_the_drawn_balance_and_loses_on_the_draw_at_default() {
    let facility = revolver();
    let market = market();
    let pv = facility.value_raw(&market, AS_OF).expect("pv");
    // R × 4mm drawn, and (R − 1) × 50% × 6mm undrawn drawn at default.
    let on_default = 0.4 * 4_000_000.0 - 0.6 * 0.5 * 6_000_000.0;
    let jump = measure(&facility, &market, MetricId::JumpToDefault);
    assert!((jump - (on_default - pv)).abs() < 1e-6, "{jump}");
    assert!((on_default + 200_000.0).abs() < 1e-6);
}

#[test]
fn revolver_without_a_credit_curve_is_an_error() {
    let mut facility = revolver();
    facility.credit_curve_id = None;
    // Validation rejects a draw at default with no default model.
    facility.leq = 0.0;
    let market = market();
    for id in [MetricId::Recovery01, MetricId::JumpToDefault] {
        let error = error_of(&facility, &market, id);
        assert!(error.contains("requires hazard pricing"), "{error}");
    }
}
