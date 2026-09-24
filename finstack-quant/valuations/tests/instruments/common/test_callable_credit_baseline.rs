//! Behavioral baselines for callable credit-risky pricing on the explicit
//! rates-credit path.
//!
//! These pin the PV/OAS numbers produced by the two-factor rates-credit
//! model for a callable bond and a callable term loan. They exist so the
//! public `hazard_volatility` / `rate_credit_correlation` inputs, the shared
//! resolver, and the deterministic seed remain wired consistently.
//!
//! The pinned regime is the one the engines used implicitly before the
//! refactor — short-rate vol `0.01`, hazard vol `0.20`, no mean reversion,
//! zero correlation — stated explicitly through `ModelConfig`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve, ParInterp};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CallPut, CallPutSchedule};
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, LoanCall, LoanCallSchedule, LoanCallType, RateSpec, TermLoan,
    TermLoanTreePricer,
};
use time::macros::date;

/// Baseline regime: the volatilities the engines applied implicitly before
/// the configuration refactor made them public inputs.
const BASELINE_RATE_VOL: f64 = 0.01;
const BASELINE_HAZARD_VOL: f64 = 0.20;

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
        .insert(hazard_curve())
}

fn replayable_market() -> MarketContext {
    let source = MarketContext::new().insert(discount_curve());
    let hazard = crate::test_support::credit::calibrated_hazard_curve(
        &source,
        as_of(),
        "USD-HAZ",
        "CALLABLE-CREDIT-ENTITY",
        "USD-OIS",
    )
    .expect("hazard calibration should succeed");
    source.insert(hazard)
}

/// Fixed-rate callable bond routed onto the rates-credit tree by an explicit
/// `credit_curve_id`.
fn callable_credit_bond() -> Bond {
    let mut bond = Bond::fixed(
        "CALLABLE-CREDIT-BASELINE",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.06).expect("valid rate fixture"),
        as_of(),
        date!(2030 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();
    bond.credit_curve_id = Some(CurveId::from("USD-HAZ"));
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2027 - 01 - 01),
            end_date: date!(2029 - 01 - 01),
            price_pct_of_par: 102.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond
}

fn callable_credit_loan() -> TermLoan {
    let mut loan = TermLoan::builder()
        .id(InstrumentId::new("TL-CALLABLE-CREDIT-BASELINE"))
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .issue_date(as_of())
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 600 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .business_day_convention(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .credit_curve_id_opt(Some(CurveId::from("USD-HAZ")))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .build()
        .unwrap();
    loan.call_schedule = Some(LoanCallSchedule {
        calls: vec![LoanCall {
            date: date!(2027 - 01 - 01),
            price_pct_of_par: 102.0,
            call_type: LoanCallType::Hard,
        }],
    });
    apply_baseline_regime(&mut loan.instrument_pricing_overrides);
    loan
}

/// Apply the baseline regime explicitly through the public model config.
///
/// The native bond path infers `rates_credit` when these joint-factor inputs
/// are present.
fn apply_baseline_regime(
    overrides: &mut finstack_quant_valuations::instruments::InstrumentPricingOverrides,
) {
    overrides.model_config.hw1f_sigma = Some(BASELINE_RATE_VOL);
    overrides.model_config.hazard_volatility = Some(BASELINE_HAZARD_VOL);
    // These tests pin routing and qualitative model behavior. A small,
    // deterministic sample keeps the integration suite fast; production uses
    // the full default budget unless callers override `mc_paths`.
    overrides.model_config.mc_paths = Some(128);
}

/// PV of the callable credit-risky bond under the baseline regime.
///
/// # Why this moved from the pre-refactor number
///
/// The callable stochastic bond now uses the pathwise LSMC exercise engine.
/// The pinned value uses the deterministic instrument-derived seed and the
/// test-only 128-estimator budget set by [`apply_baseline_regime`].
///
/// Re-blessed for the exercise-candidate grid (W3.1): the call window is now
/// exercisable at its ends, schedule dates and month-ends instead of every
/// calendar day. The PV moved from 986_735.637_112_065 by -19.79 on 1MM face
/// (-0.002 per 100), inside the 0.01-per-100 gate pinned by
/// tests/instruments/bond/exercise_grid.rs.
const BASELINE_BOND_PV: f64 = 986_715.843_219_591;
/// PV of the callable credit-risky term loan under the same regime after
/// dirty-call settlement and survival-on-continuation rollback. Accrued
/// coupons remain payable through adjusted payment dates, including calls
/// between an unadjusted coupon end and its delayed settlement.
const BASELINE_LOAN_PV: f64 = 9_862_676.817_499_82;
/// OAS (bp) recovered by re-solving at the term loan's own model price.
/// Non-zero only by the clean/dirty accrued conversion inside the solve.
const BASELINE_LOAN_OAS_BP: f64 = -1.191_277_146_1;

#[test]
fn callable_credit_bond_pv_baseline() {
    use finstack_quant_valuations::instruments::Instrument;

    let mut bond = callable_credit_bond();
    apply_baseline_regime(&mut bond.instrument_pricing_overrides);

    let pv = bond.value(&market(), as_of()).unwrap().amount();

    assert!(
        (pv - BASELINE_BOND_PV).abs() < 1e-6,
        "callable credit-risky bond PV moved: actual={pv:.10}, \
         baseline={BASELINE_BOND_PV:.10}. The rates-credit lattice or the \
         bond routing changed — not just configuration wiring."
    );
}

#[test]
fn callable_credit_term_loan_pv_and_oas_baseline() {
    let loan = callable_credit_loan();
    let market = market();
    let pricer = TermLoanTreePricer::new();

    let pv = pricer
        .price_callable(&loan, &market, as_of())
        .unwrap()
        .amount();
    assert!(
        (pv - BASELINE_LOAN_PV).abs() < 1e-6,
        "callable credit-risky term-loan PV moved: actual={pv:.10}, \
         baseline={BASELINE_LOAN_PV:.10}"
    );

    // OAS re-solved at the instrument's own model price: the round trip must
    // land on the same point it did before the refactor, which also pins that
    // direct PV and the OAS objective share one calibrated tree.
    let clean_px = 100.0 * pv / loan.notional_limit.amount();
    let oas_bp = pricer
        .calculate_oas(&loan, &market, as_of(), clean_px)
        .unwrap();
    assert!(
        (oas_bp - BASELINE_LOAN_OAS_BP).abs() < 1e-6,
        "callable credit-risky term-loan OAS moved: actual={oas_bp:.10}, \
         baseline={BASELINE_LOAN_OAS_BP:.10}"
    );
}

/// The joint rates-credit model reaches all four factor-volatility regimes by
/// changing only `ModelConfig`.
#[test]
fn all_four_regimes_are_selected_by_model_config_alone() {
    use finstack_quant_valuations::instruments::Instrument;

    let market = market();
    let price_in = |sigma: Option<f64>, hazard_vol: Option<f64>, rho: Option<f64>| -> f64 {
        let mut bond = callable_credit_bond();
        bond.instrument_pricing_overrides.model_config.hw1f_sigma = sigma;
        bond.instrument_pricing_overrides
            .model_config
            .hazard_volatility = hazard_vol;
        bond.instrument_pricing_overrides
            .model_config
            .rate_credit_correlation = rho;
        bond.instrument_pricing_overrides.model_config.mc_paths = Some(128);
        bond.value(&market, as_of()).unwrap().amount()
    };

    let deterministic = price_in(None, None, None);
    let stochastic_rates = price_in(Some(0.01), None, None);
    let stochastic_credit = price_in(None, Some(0.02), None);
    let correlated = price_in(Some(0.01), Some(0.02), Some(-0.4));

    for (label, pv) in [
        ("zero/zero", deterministic),
        ("nonzero/zero", stochastic_rates),
        ("zero/nonzero", stochastic_credit),
        ("nonzero/nonzero", correlated),
    ] {
        assert!(
            pv.is_finite() && pv > 0.0,
            "{label} regime must price: {pv}"
        );
    }

    // Each stochastic factor must actually bite — a regime that silently
    // collapsed onto another would show up as an identical price.
    assert!(
        (stochastic_rates - deterministic).abs() > 1e-6,
        "rate volatility must change the callable value: \
         stochastic={stochastic_rates:.6}, deterministic={deterministic:.6}"
    );
    assert!(
        (stochastic_credit - deterministic).abs() > 1e-6,
        "hazard volatility must change the callable value: \
         stochastic={stochastic_credit:.6}, deterministic={deterministic:.6}"
    );
    assert!(
        (correlated - price_in(Some(0.01), Some(0.02), None)).abs() > 1e-9,
        "correlation must change the correlated regime's value"
    );
}

/// Hazard-model inputs on an instrument with no credit curve are rejected,
/// not silently ignored: that instrument prices on the short-rate tree, which
/// has no hazard factor at all.
#[test]
fn hazard_inputs_without_a_credit_curve_fail_validation() {
    use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
    use finstack_quant_valuations::pricer::ModelKey;

    let market = market();
    for (label, apply) in [
        (
            "hazard_volatility",
            Box::new(|b: &mut Bond| {
                b.instrument_pricing_overrides
                    .model_config
                    .hazard_volatility = Some(0.02);
            }) as Box<dyn Fn(&mut Bond)>,
        ),
        (
            "rate_credit_correlation",
            Box::new(|b: &mut Bond| {
                b.instrument_pricing_overrides
                    .model_config
                    .rate_credit_correlation = Some(0.3);
            }),
        ),
    ] {
        let mut bond = callable_credit_bond();
        bond.credit_curve_id = None;
        apply(&mut bond);
        let err = bond
            .value(&market, as_of())
            .expect_err("hazard input without a credit curve must fail");
        let msg = err.to_string();
        assert!(
            msg.contains(label) && msg.contains("credit_curve_id"),
            "error must name the inert field and the missing curve: {msg}"
        );

        let default_registry_error = bond
            .price_with_metrics(&market, as_of(), &[], PricingOptions::default())
            .expect_err("the registry's native default must reject the same orphan credit input");
        let default_registry_message = default_registry_error.to_string();
        assert!(
            default_registry_message.contains(label)
                && default_registry_message.contains("credit_curve_id"),
            "default registry error must name the inert field and missing curve: {default_registry_message}"
        );

        let explicit_tree = bond
            .price_with_metrics(
                &market,
                as_of(),
                &[],
                PricingOptions::default().with_model(ModelKey::Tree),
            )
            .expect("an explicit Tree selection must ignore credit-only model inputs");
        assert!(explicit_tree.value.amount().is_finite());
    }
}

/// The legacy `implied_volatility` channel fails loudly on the credit path
/// rather than silently flipping the regime to deterministic rates.
#[test]
fn legacy_vol_channel_is_rejected_on_the_credit_path() {
    use finstack_quant_valuations::instruments::Instrument;

    let mut bond = callable_credit_bond();
    bond.instrument_pricing_overrides.model_config.hw1f_sigma = None;
    // The legacy channel, deliberately populated: it must be rejected on this
    // path rather than quietly standing in for hw1f_sigma.
    bond.instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.01);

    let err = bond
        .value(&market(), as_of())
        .expect_err("implied_volatility without hw1f_sigma must fail on this path");
    let msg = err.to_string();
    assert!(
        msg.contains("hw1f_sigma") && msg.contains("implied_volatility"),
        "error must name both the canonical and the legacy field: {msg}"
    );
}

/// Every risk path reads the same resolved model inputs.
///
/// Rate vega in particular has to bump the volatility channel the instrument's
/// routing actually consumes. A credit-risky callable reads `hw1f_sigma`, so a
/// vega that bumped `implied_volatility` would report identically zero — a
/// silently inert risk number rather than a visible failure.
#[test]
fn risk_metrics_share_the_resolved_model_inputs() {
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::metrics::MetricId;

    let market = replayable_market();
    let mut bond = callable_credit_bond();
    apply_baseline_regime(&mut bond.instrument_pricing_overrides);

    let result = bond
        .price_with_metrics(
            &market,
            as_of(),
            &[
                MetricId::Vega,
                MetricId::Cs01,
                MetricId::EmbeddedOptionValue,
            ],
            crate::test_support::credit::pricing_options(),
        )
        .expect("credit-risky callable must produce risk metrics");

    let vega = *result.measures.get("vega").expect("vega measure");
    assert!(
        vega.is_finite() && vega.abs() > 1e-9,
        "rate vega must be live on the rates-credit path — a zero here means \
         the bump moved a channel the model no longer reads: {vega}"
    );

    let cs01 = *result.measures.get("cs01").expect("cs01 measure");
    assert!(
        cs01.is_finite() && cs01.abs() > 1e-9,
        "CS01 must be live on the rates-credit path: {cs01}"
    );

    let option_value = *result
        .measures
        .get("embedded_option_value")
        .expect("embedded option value");
    assert!(
        option_value.is_finite(),
        "embedded option value must resolve through the same tree inputs"
    );
}

/// Removing the call schedule recovers the straight credit-risky value under
/// the same resolved model inputs.
#[test]
fn removing_calls_recovers_the_straight_credit_risky_value() {
    use finstack_quant_valuations::instruments::Instrument;

    let market = market();
    let mut callable = callable_credit_bond();
    apply_baseline_regime(&mut callable.instrument_pricing_overrides);
    let mut straight = callable.clone();
    straight.call_put = None;

    let callable_pv = callable.value(&market, as_of()).unwrap().amount();
    let straight_pv = straight.value(&market, as_of()).unwrap().amount();
    assert!(
        straight_pv >= callable_pv - 1e-9,
        "the issuer's call cannot raise the holder's value: \
         straight={straight_pv:.6}, callable={callable_pv:.6}"
    );
}

/// Recovery, call friction, and amortization all still move the callable
/// credit-risky term loan in the documented direction. These are the
/// qualitative baselines the refactor must not invert.
#[test]
fn callable_credit_term_loan_structural_baselines() {
    let market = market();
    let pricer = TermLoanTreePricer::new();
    let base = callable_credit_loan();
    let base_pv = pricer
        .price_callable(&base, &market, as_of())
        .unwrap()
        .amount();

    // Call friction raises the exercise threshold, so the borrower's option is
    // worth less to them and the lender's claim is worth at least as much.
    let mut frictional = callable_credit_loan();
    frictional
        .instrument_pricing_overrides
        .model_config
        .call_friction_cents = Some(500.0);
    let frictional_pv = pricer
        .price_callable(&frictional, &market, as_of())
        .unwrap()
        .amount();
    assert!(
        frictional_pv >= base_pv - 1e-9,
        "call friction must not lower the lender's callable value: \
         base={base_pv:.6}, frictional={frictional_pv:.6}"
    );

    // Removing the call schedule recovers the straight credit-risky value,
    // which must dominate the callable one.
    let mut straight = callable_credit_loan();
    straight.call_schedule = None;
    let straight_pv = pricer
        .price_callable(&straight, &market, as_of())
        .unwrap()
        .amount();
    assert!(
        straight_pv >= base_pv - 1e-9,
        "straight credit-risky value must dominate the callable value: \
         straight={straight_pv:.6}, callable={base_pv:.6}"
    );

    // A higher hazard curve lowers PV.
    let high_hazard = HazardCurve::builder("USD-HAZ")
        .base_date(as_of())
        .recovery_rate(0.4)
        .knots([(0.0, 0.08), (5.0, 0.08), (10.0, 0.08)])
        .par_interp(ParInterp::Linear)
        .build()
        .unwrap();
    let stressed = MarketContext::new()
        .insert(discount_curve())
        .insert(high_hazard);
    let stressed_pv = pricer
        .price_callable(&base, &stressed, as_of())
        .unwrap()
        .amount();
    assert!(
        stressed_pv < base_pv,
        "higher hazard must lower PV: stressed={stressed_pv:.6}, base={base_pv:.6}"
    );
}
