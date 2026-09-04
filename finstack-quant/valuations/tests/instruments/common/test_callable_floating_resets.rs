//! Acceptance tests for path-dependent floating resets under the rates-credit
//! model.
//!
//! Stochastic factors are sampled with a deterministic seed so paired cases
//! share random numbers. Two identities anchor the tests:
//!
//! - **FRN invariance**: for an option-free floater with zero rate/credit
//!   correlation, rate volatility does not change the value of a forward-set
//!   floating leg beyond Monte Carlo tolerance.
//! - **Optionality breaks the invariance the right way**: an all-in cap is
//!   a short caplet strip (PV falls as rate vol rises), an all-in floor is
//!   a long floorlet strip (PV rises), and a non-zero rate/credit
//!   correlation moves PV through the coupon/survival-discount covariance
//!   even without caps or floors.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_cashflows::builder::FloatingRateSpec;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, ForwardCurve, HazardCurve, ParInterp,
};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::bond::{
    Bond, CallPut, CallPutSchedule, CashflowSpec,
};
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, LoanCall, LoanCallSchedule, LoanCallType, RateSpec, TermLoan,
    TermLoanTreePricer,
};
use finstack_quant_valuations::pricer::ModelKey;
use finstack_quant_valuations::results::ValuationDetails;
use rust_decimal::Decimal;
use time::macros::date;

fn as_of() -> Date {
    date!(2025 - 01 - 01)
}

fn discount_curve() -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(as_of())
        .knots([
            (0.0, 1.0),
            (1.0, (-0.04_f64).exp()),
            (5.0, (-0.20_f64).exp()),
            (10.0, (-0.40_f64).exp()),
        ])
        .build()
        .unwrap()
}

fn index_curve() -> ForwardCurve {
    ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of())
        .knots([(0.0, 0.04), (10.0, 0.04)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

fn hazard_curve() -> HazardCurve {
    HazardCurve::builder("USD-HAZ")
        .base_date(as_of())
        .recovery_rate(0.4)
        .knots([(0.0, 0.03), (5.0, 0.03), (10.0, 0.03)])
        .par_interp(ParInterp::Linear)
        .build()
        .unwrap()
}

fn market() -> MarketContext {
    MarketContext::new()
        .insert(discount_curve())
        .insert(index_curve())
        .insert(hazard_curve())
}

/// Quarterly USD-SOFR-3M + 200 bp floater on the rates-credit path.
///
/// Zero reset lag keeps the first (current) period's observation exactly on
/// the valuation date — a known fixing that must stay deterministic — while
/// every later reset is a future observation.
fn floating_credit_bond() -> Bond {
    let mut bond = Bond::fixed(
        "FRN-CREDIT-RESETS",
        Money::new(1_000_000.0, Currency::USD),
        finstack_quant_core::types::Rate::from_decimal(0.06),
        as_of(),
        date!(2030 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();
    bond.cashflow_spec = CashflowSpec::floating_with_reset_lag(
        CurveId::from("USD-SOFR-3M"),
        200.0,
        Tenor::quarterly(),
        DayCount::Act360,
        0,
    )
    .unwrap();
    bond.credit_curve_id = Some(CurveId::from("USD-HAZ"));
    bond
}

fn with_all_in_cap(mut bond: Bond, cap_bp: i64) -> Bond {
    match &mut bond.cashflow_spec {
        CashflowSpec::Floating(fc) => {
            fc.rate_spec.all_in_cap_bp = Some(Decimal::from(cap_bp));
        }
        other => panic!("expected floating spec, got {other:?}"),
    }
    bond
}

fn with_all_in_floor(mut bond: Bond, floor_bp: i64) -> Bond {
    match &mut bond.cashflow_spec {
        CashflowSpec::Floating(fc) => {
            fc.rate_spec.all_in_floor_bp = Some(Decimal::from(floor_bp));
        }
        other => panic!("expected floating spec, got {other:?}"),
    }
    bond
}

fn price(bond: &Bond, sigma: Option<f64>, hazard_vol: Option<f64>, rho: Option<f64>) -> f64 {
    price_estimate(bond, sigma, hazard_vol, rho).0
}

fn price_estimate(
    bond: &Bond,
    sigma: Option<f64>,
    hazard_vol: Option<f64>,
    rho: Option<f64>,
) -> (f64, f64) {
    let mut bond = bond.clone();
    bond.instrument_pricing_overrides.model_config.hw1f_sigma = sigma;
    bond.instrument_pricing_overrides
        .model_config
        .hazard_volatility = hazard_vol;
    bond.instrument_pricing_overrides
        .model_config
        .rate_credit_correlation = rho;
    bond.instrument_pricing_overrides.model_config.mc_paths = Some(512);
    let result = finstack_quant_valuations::pricer::standard_pricer_registry()
        .price_with_metrics(
            &bond,
            ModelKey::RatesCredit,
            &market(),
            as_of(),
            &[],
            Default::default(),
        )
        .unwrap();
    let standard_error = match &result.details {
        Some(ValuationDetails::MonteCarlo(details)) => details.standard_error,
        _ => 0.0,
    };
    (result.value.amount(), standard_error)
}

fn monte_carlo_tolerance(left_standard_error: f64, right_standard_error: f64) -> f64 {
    let combined = left_standard_error.hypot(right_standard_error);
    (4.0 * combined).max(0.05)
}

/// Option-free floater, zero correlation: expected PV is invariant to the
/// short-rate volatility, with and without a stochastic hazard factor. The
/// stochastic bullet path is sampled, so the comparison uses the reported
/// sampling error rather than requiring exact pathwise equality.
#[test]
fn frn_pv_invariant_to_rate_vol_without_correlation() {
    let bond = floating_credit_bond();

    let deterministic = price(&bond, Some(0.0), None, None);
    let (stochastic_rates, stochastic_rates_se) = price_estimate(&bond, Some(0.012), None, None);
    let rates_tolerance = monte_carlo_tolerance(stochastic_rates_se, 0.0);
    assert!(
        (stochastic_rates - deterministic).abs() <= rates_tolerance,
        "option-free FRN PV must not move with rate vol at rho = 0: \
         sigma=0 -> {deterministic:.6}, sigma=0.012 -> {stochastic_rates:.6}, \
         MC standard error -> {stochastic_rates_se:.6}, \
         tolerance -> {rates_tolerance:.6}"
    );

    let (both_factors, both_factors_se) = price_estimate(&bond, Some(0.012), Some(0.02), None);
    let (credit_only, credit_only_se) = price_estimate(&bond, Some(0.0), Some(0.02), None);
    let independent_factors_tolerance = monte_carlo_tolerance(both_factors_se, credit_only_se);
    assert!(
        (both_factors - credit_only).abs() <= independent_factors_tolerance,
        "with independent factors the rate vol still must not move an \
         option-free FRN: credit-only -> {credit_only:.6}, \
         both -> {both_factors:.6}, combined MC tolerance -> \
         {independent_factors_tolerance:.6}"
    );
}

/// An all-in cap makes the holder short a caplet strip: rate volatility
/// must strictly cheapen the bond. This is the coupon-level optionality the
/// deterministic projection cannot see.
#[test]
fn capped_frn_loses_value_under_rate_vol() {
    let bond = with_all_in_cap(floating_credit_bond(), 650);
    let flat = price(&bond, Some(0.0), None, None);
    let vol = price(&bond, Some(0.012), None, None);
    assert!(
        vol < flat - 50.0,
        "near-the-money all-in cap must cheapen the floater under rate vol: \
         sigma=0 -> {flat:.4}, sigma=0.012 -> {vol:.4}"
    );
}

/// An all-in floor is a long floorlet strip: rate volatility must strictly
/// richen the bond.
#[test]
fn floored_frn_gains_value_under_rate_vol() {
    let bond = with_all_in_floor(floating_credit_bond(), 600);
    let flat = price(&bond, Some(0.0), None, None);
    let vol = price(&bond, Some(0.012), None, None);
    assert!(
        vol > flat + 50.0,
        "near-the-money all-in floor must richen the floater under rate vol: \
         sigma=0 -> {flat:.4}, sigma=0.012 -> {vol:.4}"
    );
}

/// With both factors stochastic and no caps or floors, the rate/credit
/// correlation moves the floater through two channels: the risky-discount
/// covariance `E[S·D]` on every booked cashflow (present for fixed bonds
/// too, positive correlation richens), and the coupon-specific covariance
/// carried by the folded increments (negative for positive correlation;
/// its sign is pinned in isolation by the lattice test
/// `pure_forward_increment_folds_to_zero_without_correlation`). The total
/// must move in both correlation directions — the discount-node channel
/// alone cancels for an option-free floater (see the invariance test), so
/// a flat response would mean the covariance machinery is dead.
#[test]
fn correlation_moves_floating_pv() {
    let bond = floating_credit_bond();
    let independent = price(&bond, Some(0.012), Some(0.02), None);
    let positive = price(&bond, Some(0.012), Some(0.02), Some(0.4));
    let negative = price(&bond, Some(0.012), Some(0.02), Some(-0.4));

    assert!(
        (positive - independent).abs() > 1.0,
        "rho = 0.4 must move the floater: rho=0 -> {independent:.4}, \
         rho=0.4 -> {positive:.4}"
    );
    assert!(
        (negative - independent).abs() > 1.0,
        "rho = -0.4 must move the floater: rho=0 -> {independent:.4}, \
         rho=-0.4 -> {negative:.4}"
    );
    assert!(
        (positive - independent) * (negative - independent) < 0.0,
        "opposite correlations must move the floater in opposite directions: \
         rho=-0.4 -> {negative:.4}, rho=0 -> {independent:.4}, \
         rho=0.4 -> {positive:.4}"
    );
}

/// A call date strictly inside a future floating coupon period retains the
/// simulated fixing state and is exercisable under stochastic rates.
#[test]
fn mid_period_call_prices_with_stochastic_rates() {
    let mut bond = floating_credit_bond();
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2027 - 02 - 15),
            end_date: date!(2027 - 02 - 15),
            price_pct_of_par: 95.0,
            make_whole: None,
        }],
        puts: vec![],
    });

    // Deterministic rates: today's behavior, prices fine.
    let deterministic = price(&bond, Some(0.0), Some(0.02), None);
    assert!(deterministic.is_finite() && deterministic > 0.0);

    let uncallable = price(&floating_credit_bond(), Some(0.012), Some(0.02), None);
    let stochastic = price(&bond, Some(0.012), Some(0.02), None);
    assert!(
        stochastic.is_finite() && stochastic > 0.0,
        "between-reset stochastic call must produce a finite positive value: {stochastic}"
    );
    assert!(
        stochastic < uncallable - 1_000.0,
        "a binding issuer call must reduce holder value: callable={stochastic}, bullet={uncallable}"
    );
}

/// Quarterly floating term loan (SOFR + 400 bp) on the rates-credit path.
fn floating_credit_loan(coupon_type: CouponType) -> TermLoan {
    TermLoan::builder()
        .id(InstrumentId::new("TL-FLOATING-CREDIT-RESETS"))
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(as_of())
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Floating(FloatingRateSpec {
            index_id: CurveId::new("USD-SOFR-3M"),
            spread_bp: Decimal::new(400, 0),
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        }))
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .business_day_convention(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .credit_curve_id_opt(Some(CurveId::from("USD-HAZ")))
        .amortization(AmortizationSpec::None)
        .coupon_type(coupon_type)
        .build()
        .unwrap()
}

fn price_loan(loan: &TermLoan, sigma: Option<f64>) -> finstack_quant_core::Result<f64> {
    let mut loan = loan.clone();
    loan.instrument_pricing_overrides.model_config.hw1f_sigma = sigma;
    TermLoanTreePricer::new()
        .price_callable(&loan, &market(), as_of())
        .map(|m| m.amount())
}

/// The FRN invariance holds for the term-loan engine too: an option-free
/// floating loan with zero rate/credit correlation must not move with the
/// short-rate volatility.
#[test]
fn floating_loan_pv_invariant_to_rate_vol_without_correlation() {
    let loan = floating_credit_loan(CouponType::Cash);
    let deterministic = price_loan(&loan, None).unwrap();
    let stochastic = price_loan(&loan, Some(0.012)).unwrap();
    assert!(
        (stochastic - deterministic).abs() < 0.5,
        "option-free floating loan PV must not move with rate vol at rho = 0: \
         sigma=0 -> {deterministic:.6}, sigma=0.012 -> {stochastic:.6}"
    );
}

/// A standing (effective-dated) loan call provision stays priceable under
/// stochastic rates: exercise is discretized to the floating periods'
/// reset/payment slices, and the call can only cheapen the loan relative
/// to its non-callable twin.
#[test]
fn floating_loan_standing_call_prices_under_stochastic_rates() {
    let mut callable = floating_credit_loan(CouponType::Cash);
    callable.call_schedule = Some(LoanCallSchedule {
        calls: vec![LoanCall {
            date: date!(2027 - 01 - 01),
            price_pct_of_par: 100.0,
            call_type: LoanCallType::Hard,
        }],
    });
    let non_callable = floating_credit_loan(CouponType::Cash);

    let callable_pv = price_loan(&callable, Some(0.012)).unwrap();
    let non_callable_pv = price_loan(&non_callable, Some(0.012)).unwrap();
    assert!(
        callable_pv.is_finite() && callable_pv > 0.0,
        "callable floating loan must price under stochastic rates"
    );
    assert!(
        callable_pv <= non_callable_pv + 1e-6,
        "the borrower's call cannot make the loan worth more to the holder: \
         callable={callable_pv:.4}, non-callable={non_callable_pv:.4}"
    );
}

/// A future floating PIK coupon capitalizes a node-dependent amount into
/// principal — path-dependent outstanding the lattice cannot carry. It must
/// fail loudly in stochastic-rate mode while deterministic-rate PIK keeps
/// pricing exactly as today.
#[test]
fn floating_pik_loan_rejected_only_under_stochastic_rates() {
    let loan = floating_credit_loan(CouponType::Pik);

    let deterministic = price_loan(&loan, None).unwrap();
    assert!(
        deterministic.is_finite() && deterministic > 0.0,
        "deterministic-rate floating PIK must keep pricing"
    );

    let err = price_loan(&loan, Some(0.012)).expect_err("stochastic-rate floating PIK");
    let msg = err.to_string();
    assert!(
        msg.contains("PIK") && msg.contains("path-dependent"),
        "error must name the PIK path-dependence: {msg}"
    );
}

/// The OAS solve runs the fold on every objective evaluation; solving at
/// the loan's own model price must land near zero (the residual is the
/// clean/dirty accrued conversion, same as the fixed-rate baseline).
#[test]
fn floating_loan_oas_solves_under_stochastic_rates() {
    let mut loan = floating_credit_loan(CouponType::Cash);
    loan.instrument_pricing_overrides.model_config.hw1f_sigma = Some(0.012);
    let market = market();
    let pricer = TermLoanTreePricer::new();

    let pv = pricer
        .price_callable(&loan, &market, as_of())
        .unwrap()
        .amount();
    let clean_px = 100.0 * pv / loan.notional_limit.amount();
    let oas_bp = pricer
        .calculate_oas(&loan, &market, as_of(), clean_px)
        .unwrap();
    assert!(
        oas_bp.is_finite() && oas_bp.abs() < 50.0,
        "OAS re-solved at the model's own price must be near zero, got {oas_bp:.4} bp"
    );
}

/// Every volatility regime prices the callable floater finitely through the
/// public config alone — including the correlated regime, whose coupons,
/// discounting, and survival all interact on the lattice.
#[test]
fn callable_floater_prices_in_all_regimes() {
    let mut bond = floating_credit_bond();
    // A live call struck at par-ish, on coupon dates only.
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2027 - 01 - 01),
            end_date: date!(2029 - 01 - 01),
            price_pct_of_par: 101.0,
            make_whole: None,
        }],
        puts: vec![],
    });

    for (label, sigma, hazard_vol, rho) in [
        ("zero/zero", None, None, None),
        ("nonzero/zero", Some(0.012), None, None),
        ("zero/nonzero", None, Some(0.02), None),
        ("correlated", Some(0.012), Some(0.02), Some(-0.3)),
    ] {
        let pv = price(&bond, sigma, hazard_vol, rho);
        assert!(
            pv.is_finite() && pv > 0.0,
            "{label}: callable floater must price, got {pv}"
        );
    }
}
