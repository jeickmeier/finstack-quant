//! Regression tests for the 2026-09 revolving-credit review fixes:
//!
//! 1. Seasoned stochastic facilities price (the Monte Carlo grid used to be
//!    rejected for any valuation date after the commitment date) and the
//!    simulated funding leg conserves principal for every valuation date.
//! 2. A facility credit curve requires a market-anchored spread process, so
//!    hazard CS01 can never silently report zero.
//! 3. Reset frequencies shorter than the payment frequency re-fix the coupon
//!    inside the period, in both engines, and the two engines agree.
//! 4. Stochastic (Hull-White) rates keep the index-over-OIS basis.
//! 5. Loan-equivalent exposure (`leq`) adds the cost of draws at default.

use finstack_quant_cashflows::builder::FloatingRateSpec;
use finstack_quant_cashflows::traits::CashflowScheduleSource;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor, TenorUnit};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, CreditSpreadProcessSpec, DrawRepaySpec, InterestRateProcessSpec, McConfig,
    RevolvingCredit, RevolvingCreditFees, RevolvingCreditPricer, StochasticUtilizationSpec,
    UtilizationProcess,
};
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

use crate::common::test_helpers::flat_discount_curve;

const COMMITMENT: Date = date!(2025 - 01 - 01);
const MATURITY: Date = date!(2027 - 01 - 01);

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("valid money fixture")
}

fn term_spec(reset: Tenor) -> FloatingRateSpec {
    FloatingRateSpec {
        index_id: "USD-SOFR-3M".into(),
        spread_bp: rust_decimal::Decimal::ZERO,
        gearing: rust_decimal::Decimal::ONE,
        gearing_includes_spread: true,
        index_floor_bp: None,
        all_in_floor_bp: None,
        all_in_cap_bp: None,
        index_cap_bp: None,
        overnight_index_constraints: Default::default(),
        reset_frequency: reset,
        index_tenor: None,
        reset_lag_days: 0,
        fixing_calendar_id: None,
        overnight_compounding: None,
        overnight_basis: None,
        fallback: Default::default(),
    }
}

/// Zero-volatility utilization that still drifts toward `target`, so the
/// simulated path carries genuine draws/repayments without Monte Carlo noise.
/// Stochastic utilization spec. Zero volatility freezes utilization (parity
/// mode); a positive volatility produces genuine simulated draws/repayments.
fn stochastic(
    target: f64,
    volatility: f64,
    num_paths: usize,
    mc_config: Option<McConfig>,
) -> DrawRepaySpec {
    DrawRepaySpec::Stochastic(Box::new(StochasticUtilizationSpec {
        utilization_process: UtilizationProcess::MeanReverting {
            target_rate: target,
            speed: 1.0,
            volatility,
            spread_sensitivity: 0.0,
        },
        num_paths,
        seed: Some(42),
        antithetic: false,
        use_sobol_qmc: false,
        mc_config,
    }))
}

fn drifting_stochastic(target: f64, mc_config: Option<McConfig>) -> DrawRepaySpec {
    stochastic(target, 0.0, 2, mc_config)
}

fn no_credit_config() -> McConfig {
    McConfig {
        correlation_matrix: None,
        credit_spread_process: CreditSpreadProcessSpec::Constant(0.0),
        interest_rate_process: None,
        util_credit_corr: None,
    }
}

fn facility(
    id: &str,
    drawn: f64,
    base: BaseRateSpec,
    draw_repay: DrawRepaySpec,
    recovery_rate: f64,
) -> RevolvingCredit {
    RevolvingCredit::builder()
        .id(id.into())
        .commitment_amount(usd(10_000_000.0))
        .drawn_amount(usd(drawn))
        .commitment_date(COMMITMENT)
        .maturity(MATURITY)
        .base_rate_spec(base)
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(draw_repay)
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(recovery_rate)
        .build()
        .expect("facility fixture")
}

fn notional_sum(schedule: &finstack_quant_cashflows::builder::CashFlowSchedule) -> f64 {
    schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Notional)
        .map(|cf| cf.amount.amount())
        .sum()
}

/// Fix 1: the Monte Carlo grid is validated on the anchor-relative axis, so a
/// stochastic facility prices on every date of its life, and the simulated
/// funding leg always nets to the balance outstanding at the valuation date.
#[test]
fn seasoned_stochastic_facility_prices_and_conserves_principal() {
    let market = MarketContext::new().insert(flat_discount_curve(0.03, COMMITMENT, "USD-OIS"));
    let drawn = 3_000_000.0;

    // Valuation dates before, on, and after the first accrual period's midpoint,
    // on an accrual boundary, and deep into the life.
    for as_of in [
        date!(2025 - 01 - 02),
        date!(2025 - 02 - 15),
        date!(2025 - 03 - 01),
        date!(2025 - 04 - 01),
        date!(2026 - 05 - 20),
    ] {
        // Genuine utilization volatility so every path carries simulated
        // draws and repayments after the valuation date.
        let f = facility(
            "RC-SEASONED",
            drawn,
            BaseRateSpec::Fixed { rate: 0.05 },
            stochastic(0.6, 0.25, 8, Some(no_credit_config())),
            0.4,
        );
        let result = RevolvingCreditPricer::price_with_paths(&f, &market, as_of)
            .unwrap_or_else(|e| panic!("seasoned stochastic pricing failed at {as_of}: {e}"));
        assert_eq!(result.path_results.len(), 8);
        for (i, path) in result.path_results.iter().enumerate() {
            let sum = notional_sum(&path.cashflows);
            assert!(
                (sum - drawn).abs() < 1e-6,
                "path {i} at {as_of}: notional flows sum to {sum}, drawn = {drawn}"
            );
            // Every period after as_of books its utilization change, so the
            // path must carry interim principal flows besides the terminal
            // repayment.
            let interim = path
                .cashflows
                .get_flows()
                .iter()
                .filter(|cf| cf.kind == CFKind::Notional && cf.date > as_of && cf.date < MATURITY)
                .count();
            assert!(
                interim >= 1,
                "path {i} at {as_of}: expected simulated draws/repayments"
            );
            assert!(path.pv.amount().is_finite());
        }
    }

    // Theta rolls the valuation date forward one day, which used to fail for
    // every stochastic facility.
    let f = facility(
        "RC-SEASONED-THETA",
        drawn,
        BaseRateSpec::Fixed { rate: 0.05 },
        drifting_stochastic(0.6, Some(no_credit_config())),
        0.4,
    );
    let priced = f
        .price_with_metrics(
            &market,
            COMMITMENT,
            &[MetricId::Theta],
            PricingOptions::default(),
        )
        .expect("theta on a stochastic facility");
    let theta = priced.measures.get("theta").expect("theta measure");
    assert!(theta.is_finite(), "theta must be finite, got {theta}");
}

/// Fix 2: a facility credit curve is the hazard CS01 bump target, so the
/// stochastic spread process must anchor to that same curve.
#[test]
fn stochastic_facility_with_credit_curve_requires_market_anchored_process() {
    let explicit_cir = McConfig {
        correlation_matrix: None,
        credit_spread_process: CreditSpreadProcessSpec::Cir {
            kappa: 0.5,
            theta: 0.02,
            sigma: 0.05,
            initial: 0.02,
        },
        interest_rate_process: None,
        util_credit_corr: None,
    };
    let err = RevolvingCredit::builder()
        .id("RC-CS01-CIR".into())
        .commitment_amount(usd(10_000_000.0))
        .drawn_amount(usd(3_000_000.0))
        .commitment_date(COMMITMENT)
        .maturity(MATURITY)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(drifting_stochastic(0.6, Some(explicit_cir)))
        .discount_curve_id("USD-OIS".into())
        .credit_curve_id("BORROWER-HZ".into())
        .recovery_rate(0.4)
        .build()
        .expect_err("explicit CIR with a facility credit curve must be rejected");
    assert!(
        err.to_string().contains("MarketAnchored") && err.to_string().contains("cir"),
        "unexpected error: {err}"
    );

    let other_curve = McConfig {
        correlation_matrix: None,
        credit_spread_process: CreditSpreadProcessSpec::MarketAnchored {
            credit_curve_id: "OTHER-HZ".into(),
            kappa: 0.1,
            implied_vol: 0.3,
            tenor_years: None,
        },
        interest_rate_process: None,
        util_credit_corr: None,
    };
    let err = RevolvingCredit::builder()
        .id("RC-CS01-OTHER".into())
        .commitment_amount(usd(10_000_000.0))
        .drawn_amount(usd(3_000_000.0))
        .commitment_date(COMMITMENT)
        .maturity(MATURITY)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(drifting_stochastic(0.6, Some(other_curve)))
        .discount_curve_id("USD-OIS".into())
        .credit_curve_id("BORROWER-HZ".into())
        .recovery_rate(0.4)
        .build()
        .expect_err("market-anchored process on a different curve must be rejected");
    assert!(
        err.to_string().contains("must equal"),
        "unexpected error: {err}"
    );
}

/// Fix 3: monthly resets on a quarterly-pay facility re-fix the coupon at each
/// reset in the deterministic engine, and the zero-volatility stochastic engine
/// reproduces the same schedule.
#[test]
fn intra_period_resets_refix_the_coupon_in_both_engines() {
    let as_of = COMMITMENT;
    // Knots placed exactly on the Act/360 year fractions of the monthly reset
    // dates so the expected coupon is exact.
    let t_feb = 31.0 / 360.0;
    let t_mar = 59.0 / 360.0;
    let t_apr = 90.0 / 360.0;
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 0.02),
            (t_feb, 0.04),
            (t_mar, 0.06),
            (t_apr, 0.08),
            (3.0, 0.08),
        ])
        .build()
        .expect("forward curve");
    let market = MarketContext::new()
        .insert(flat_discount_curve(0.03, as_of, "USD-OIS"))
        .insert(fwd);
    let monthly = Tenor::new(1, TenorUnit::Months).expect("tenor");

    let deterministic = RevolvingCredit::builder()
        .id("RC-RESET-DET".into())
        .commitment_amount(usd(10_000_000.0))
        .drawn_amount(usd(10_000_000.0))
        .commitment_date(COMMITMENT)
        .maturity(date!(2026 - 01 - 01))
        .base_rate_spec(BaseRateSpec::Floating(term_spec(monthly)))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
        .build()
        .expect("deterministic facility");
    let schedule = deterministic
        .raw_cashflow_schedule(&market, as_of)
        .expect("schedule");
    let first = schedule
        .get_flows()
        .iter()
        .find(|cf| cf.kind == CFKind::FloatReset)
        .expect("first coupon");
    let expected = 10_000_000.0 * (0.02 * t_feb + 0.04 * (t_mar - t_feb) + 0.06 * (t_apr - t_mar));
    let period_start_only = 10_000_000.0 * 0.02 * t_apr;
    assert!(
        (first.amount.amount() - expected).abs() < 1e-6,
        "first coupon {} must re-fix monthly (expected {expected}, period-start-only would be \
         {period_start_only})",
        first.amount.amount()
    );

    // Zero-volatility stochastic twin at 100% utilization must reproduce the
    // deterministic schedule coupon for coupon.
    let stochastic = RevolvingCredit::builder()
        .id("RC-RESET-STOCH".into())
        .commitment_amount(usd(10_000_000.0))
        .drawn_amount(usd(10_000_000.0))
        .commitment_date(COMMITMENT)
        .maturity(date!(2026 - 01 - 01))
        .base_rate_spec(BaseRateSpec::Floating(term_spec(monthly)))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(drifting_stochastic(1.0, Some(no_credit_config())))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
        .build()
        .expect("stochastic facility");
    let result = RevolvingCreditPricer::price_with_paths(&stochastic, &market, as_of)
        .expect("stochastic pricing");
    let path_coupons: Vec<f64> = result.path_results[0]
        .cashflows
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .map(|cf| cf.amount.amount())
        .collect();
    let det_coupons: Vec<f64> = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .map(|cf| cf.amount.amount())
        .collect();
    assert_eq!(path_coupons.len(), det_coupons.len());
    for (i, (p, d)) in path_coupons.iter().zip(&det_coupons).enumerate() {
        assert!(
            (p - d).abs() < 1e-6,
            "coupon {i}: stochastic {p} vs deterministic {d}"
        );
    }
    let pv_det = deterministic.value(&market, as_of).expect("pv").amount();
    let pv_stoch = result.mc_result.estimate.mean.amount();
    assert!(
        ((pv_det - pv_stoch) / pv_det).abs() < 1e-9,
        "monthly-reset parity: det {pv_det} vs stoch {pv_stoch}"
    );
}

/// Fix 4: with a stochastic Hull-White short rate the index fixing is
/// `F_index(t) + (r_t − f_OIS(t))`, so a 200 bp OIS/index basis survives and
/// the σ → 0 limit matches the deterministic-forward valuation.
#[test]
fn stochastic_hull_white_rates_keep_the_index_basis() {
    let as_of = COMMITMENT;
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (1.0, 0.05), (5.0, 0.05)])
        .build()
        .expect("forward curve");
    let market = MarketContext::new()
        .insert(flat_discount_curve(0.03, as_of, "USD-OIS"))
        .insert(fwd);
    let hull_white = McConfig {
        correlation_matrix: None,
        credit_spread_process: CreditSpreadProcessSpec::Constant(0.0),
        interest_rate_process: Some(InterestRateProcessSpec::HullWhite1F {
            kappa: 0.05,
            sigma: 1e-6,
            initial: 0.05,
            theta: 0.05,
        }),
        util_credit_corr: None,
    };

    let forward_mode = facility(
        "RC-BASIS-FWD",
        3_000_000.0,
        BaseRateSpec::Floating(term_spec(Tenor::quarterly())),
        drifting_stochastic(0.6, Some(no_credit_config())),
        0.0,
    );
    let hw_mode = facility(
        "RC-BASIS-HW",
        3_000_000.0,
        BaseRateSpec::Floating(term_spec(Tenor::quarterly())),
        drifting_stochastic(0.6, Some(hull_white)),
        0.0,
    );
    let pv_fwd = RevolvingCreditPricer::price_with_paths(&forward_mode, &market, as_of)
        .expect("forward-mode pricing");
    let pv_hw =
        RevolvingCreditPricer::price_with_paths(&hw_mode, &market, as_of).expect("HW pricing");

    let first_rate = |r: &finstack_quant_valuations::instruments::fixed_income::revolving_credit::EnhancedMonteCarloResult| {
        r.path_results[0]
            .cashflows
            .get_flows()
            .iter()
            .find(|cf| cf.kind == CFKind::FloatReset)
            .and_then(|cf| cf.rate)
            .expect("first coupon rate")
    };
    let rate_hw = first_rate(&pv_hw);
    assert!(
        (rate_hw - 0.05).abs() < 1e-5,
        "HW-mode first coupon must carry the index forward (5%), got {rate_hw}"
    );
    let a = pv_fwd.mc_result.estimate.mean.amount();
    let b = pv_hw.mc_result.estimate.mean.amount();
    // Flat OIS curve, exact θ(t) fit and a trapezoid over a constant short
    // rate make the pathwise bank account equal the static curve, so the two
    // modes agree to solver precision rather than a loose tolerance.
    assert!(
        ((a - b) / a).abs() < 1e-5,
        "σ → 0 Hull-White PV {b} must match the deterministic-forward PV {a}"
    );
}

/// Fix 5: `leq` adds the cost of the undrawn commitment drawn at default. For
/// a zero-coupon facility with flat DF = 1 and flat hazard the risky PV is
/// `D·SP + (1 − SP)·[R·D + LEQ·U·(R − 1)]`.
#[test]
fn leq_prices_the_draw_at_default() {
    let start = COMMITMENT;
    let end = date!(2026 - 01 - 01);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(start)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 1.0), (5.0, 1.0)])
        .build()
        .expect("flat unit curve");
    let hazard = HazardCurve::builder("USD-HZ")
        .base_date(start)
        .knots([(1.0, 0.20), (5.0, 0.20)])
        .recovery_rate(0.40)
        .build()
        .expect("hazard");
    let market = MarketContext::new().insert(disc).insert(hazard);

    let build = |id: &str, leq: f64| {
        RevolvingCredit::builder()
            .id(id.into())
            .commitment_amount(usd(1_000_000.0))
            .drawn_amount(usd(400_000.0))
            .commitment_date(start)
            .maturity(end)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.0 })
            .day_count(DayCount::Act365F)
            .frequency(Tenor::annual())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .credit_curve_id("USD-HZ".into())
            .recovery_rate(0.4)
            .leq(leq)
            .build()
            .expect("facility")
    };

    let d = 400_000.0;
    let u = 600_000.0;
    let r = 0.4;
    let sp = (-0.20_f64).exp();
    let plain = d * sp + r * d * (1.0 - sp);

    let pv_zero = build("RC-LEQ-0", 0.0)
        .value(&market, start)
        .expect("price")
        .amount();
    assert!(
        (pv_zero - plain).abs() < 1.0,
        "leq = 0 must reproduce the plain recovery leg: {pv_zero} vs {plain}"
    );

    let leq = 0.5;
    let expected = plain + (1.0 - sp) * leq * u * (r - 1.0);
    let pv_leq = build("RC-LEQ-50", leq)
        .value(&market, start)
        .expect("price")
        .amount();
    assert!(
        (pv_leq - expected).abs() < 1.0,
        "leq = 0.5 PV {pv_leq} must equal {expected}"
    );
    assert!(pv_leq < pv_zero, "a draw at default must cost the lender");

    // Default is zero and out-of-range values are rejected.
    let default_leq = RevolvingCredit::example().expect("example").leq;
    assert_eq!(default_leq, 0.0);
    let mut bad = build("RC-LEQ-BAD", 0.0);
    bad.leq = 1.5;
    assert!(bad
        .validate()
        .expect_err("leq > 1 must fail")
        .to_string()
        .contains("leq"));
}

/// Draws smaller than the old booking threshold are still funded: a
/// utilization mean-reverting (speed 1, volatility 1e-8) from 30% toward
/// 30.0002% moves the balance by at most 10M × 2e-6 × (1 − e^-0.25) ≈ 4.4
/// dollars per quarter (below 1e-6 of the commitment, which used to be
/// dropped) and about 17 dollars over two years, yet the principal leg must still
/// net to the 3M drawn at the valuation date (lender view: funding outflows
/// plus the terminal repayment).
#[test]
fn stochastic_funding_leg_conserves_cash_for_small_draws() {
    let market = MarketContext::new().insert(flat_discount_curve(0.03, COMMITMENT, "USD-OIS"));
    let drawn = 3_000_000.0;
    let f = facility(
        "RC-SMALL-DRAWS",
        drawn,
        BaseRateSpec::Fixed { rate: 0.05 },
        stochastic(0.300_002, 1e-8, 2, Some(no_credit_config())),
        0.4,
    );
    let result = RevolvingCreditPricer::price_with_paths(&f, &market, COMMITMENT).expect("price");
    for (i, path) in result.path_results.iter().enumerate() {
        let terminal: f64 = path
            .cashflows
            .get_flows()
            .iter()
            .filter(|cf| cf.kind == CFKind::Notional && cf.date >= MATURITY)
            .map(|cf| cf.amount.amount())
            .sum();
        assert!(
            terminal > drawn + 10.0,
            "the balance drifted up: {terminal}"
        );
        let sum = notional_sum(&path.cashflows);
        assert!(
            (sum - drawn).abs() < 1e-6,
            "path {i}: notional flows sum to {sum}, drawn = {drawn}"
        );
    }
}
