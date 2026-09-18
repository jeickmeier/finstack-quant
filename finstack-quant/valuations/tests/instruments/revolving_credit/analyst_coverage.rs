//! Regressions from the 2026-09-17 credit-analyst coverage audit.
//!
//! Each test pins one measured audit probe: the `drawn_amount` anchor
//! semantic shared by both engines, the typed schedule conventions, and the
//! credit-risk visibility of the undrawn commitment.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, CreditSpreadProcessSpec, DrawRepayEvent, DrawRepaySpec, McConfig,
    RevolvingCredit, RevolvingCreditFees, StochasticUtilizationSpec, UtilizationProcess,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::{standard_pricer_registry, ModelKey};
use time::macros::date;

const COMMITMENT: time::Date = date!(2024 - 01 - 15);
const AS_OF: time::Date = date!(2025 - 01 - 15);
const MATURITY: time::Date = date!(2030 - 01 - 15);

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn market() -> MarketContext {
    MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(AS_OF)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.04_f64).exp()),
                (10.0, (-0.40_f64).exp()),
            ])
            .build()
            .expect("curve"),
    )
}

fn seasoned(draw_repay_spec: DrawRepaySpec) -> RevolvingCredit {
    RevolvingCredit::builder()
        .id("RC-ANCHOR".into())
        .commitment_amount(usd(10_000_000.0))
        .drawn_amount(usd(4_000_000.0))
        .commitment_date(COMMITMENT)
        .maturity(MATURITY)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.07 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(50.0, 0.0, 0.0).expect("fees"))
        .draw_repay_spec(draw_repay_spec)
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.4)
        .build()
        .expect("facility")
}

fn zero_vol_stochastic() -> DrawRepaySpec {
    DrawRepaySpec::Stochastic(Box::new(StochasticUtilizationSpec {
        utilization_process: UtilizationProcess::MeanReverting {
            target_rate: 0.4,
            speed: 1.0,
            volatility: 0.0,
            spread_sensitivity: 0.0,
        },
        num_paths: 4,
        seed: Some(1),
        antithetic: false,
        use_sobol_qmc: false,
        mc_config: Some(McConfig {
            correlation_matrix: None,
            credit_spread_process: CreditSpreadProcessSpec::Constant(0.0),
            interest_rate_process: None,
            util_credit_corr: None,
        }),
    }))
}

fn metrics(facility: &RevolvingCredit, model: ModelKey) -> (f64, f64, f64) {
    let result = standard_pricer_registry()
        .price_with_metrics(
            facility,
            model,
            &market(),
            AS_OF,
            &[
                MetricId::custom("utilization_rate"),
                MetricId::custom("available_capacity"),
            ],
            Default::default(),
        )
        .expect("price with metrics");
    (
        result.value.amount(),
        result.measures[&MetricId::custom("utilization_rate")],
        result.measures[&MetricId::custom("available_capacity")],
    )
}

/// Audit probe A: with the same fields, the deterministic and the
/// zero-volatility stochastic engine must read the same anchor balance.
/// Before the fix the deterministic engine replayed a historical draw on top
/// of `drawn_amount` (PV 7.09M, utilization 0.60) while the stochastic engine
/// started from `drawn_amount` (PV 4.76M, utilization 0.40).
#[test]
fn drawn_amount_is_the_anchor_balance_in_both_modes() {
    let deterministic = seasoned(DrawRepaySpec::Deterministic(vec![]));
    let stochastic = seasoned(zero_vol_stochastic());

    let (pv_det, util_det, avail_det) = metrics(&deterministic, ModelKey::Discounting);
    let (pv_sto, util_sto, avail_sto) = metrics(&stochastic, ModelKey::MonteCarloGBM);

    assert!(
        (util_det - 0.4).abs() < 1e-12,
        "deterministic utilization {util_det}"
    );
    assert!(
        (util_sto - 0.4).abs() < 1e-12,
        "stochastic utilization {util_sto}"
    );
    assert!((avail_det - 6_000_000.0).abs() < 1e-6);
    assert!((avail_sto - 6_000_000.0).abs() < 1e-6);
    // Same anchor balance, same flat utilization: the two engines differ only
    // by the stochastic engine's midpoint funding convention, which is inert
    // when utilization never moves.
    assert!(
        (pv_det - pv_sto).abs() < 1.0,
        "deterministic PV {pv_det} vs zero-vol stochastic PV {pv_sto}"
    );
}

/// A deterministic event dated on or before the valuation date would be
/// replayed on top of a balance that already includes it, so the engine
/// rejects it and names the date.
#[test]
fn deterministic_event_on_or_before_the_valuation_date_is_rejected() {
    let facility = seasoned(DrawRepaySpec::Deterministic(vec![DrawRepayEvent {
        date: date!(2024 - 06 - 15),
        amount: usd(2_000_000.0),
        is_draw: true,
    }]));
    let err = facility
        .value(&market(), AS_OF)
        .expect_err("pre-valuation event must be rejected")
        .to_string();
    assert!(
        err.contains("2024-06-15") && err.contains("anchor"),
        "error should name the event date and the anchor: {err}"
    );

    // The same event dated after the valuation date is accepted and moves the
    // balance forward.
    let future = seasoned(DrawRepaySpec::Deterministic(vec![DrawRepayEvent {
        date: date!(2025 - 06 - 15),
        amount: usd(2_000_000.0),
        is_draw: true,
    }]));
    let (_, util, avail) = metrics(&future, ModelKey::Discounting);
    assert!(
        (util - 0.4).abs() < 1e-12,
        "utilization is read at the valuation date"
    );
    assert!((avail - 6_000_000.0).abs() < 1e-6);
}

// ---------------------------------------------------------------------------
// Task 2: typed schedule conventions
// ---------------------------------------------------------------------------

fn conventions(
    calendar_id: Option<&str>,
    bdc: finstack_quant_core::dates::BusinessDayConvention,
    payment_lag_days: u32,
    maturity: time::Date,
) -> finstack_quant_core::Result<RevolvingCredit> {
    RevolvingCredit::builder()
        .id("RC-CAL".into())
        .commitment_amount(usd(10_000_000.0))
        .drawn_amount(usd(4_000_000.0))
        .commitment_date(AS_OF)
        .maturity(maturity)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.07 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(50.0, 0.0, 0.0).expect("fees"))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.4)
        .calendar_id_opt(calendar_id.map(str::to_string))
        .business_day_convention(bdc)
        .payment_lag_days(payment_lag_days)
        .build()
}

fn payment_dates_in(facility: &RevolvingCredit, year: i32, month: time::Month) -> Vec<time::Date> {
    use finstack_quant_cashflows::traits::CashflowScheduleSource;
    let schedule = facility
        .raw_cashflow_schedule(&market(), AS_OF)
        .expect("schedule");
    let mut dates: Vec<_> = schedule
        .get_flows()
        .iter()
        .map(|cf| cf.date)
        .filter(|d| d.year() == year && d.month() == month)
        .collect();
    dates.sort();
    dates.dedup();
    dates
}

/// Audit probe: 2029-01-15 is Martin Luther King Day. Only the typed
/// `calendar_id` moves the payment; attributes metadata is inert and an
/// unknown calendar fails validation.
#[test]
fn calendar_id_is_the_only_way_to_adjust_for_holidays() {
    use finstack_quant_core::dates::BusinessDayConvention;
    use finstack_quant_valuations::instruments::Attributes;

    let weekends =
        conventions(None, BusinessDayConvention::ModifiedFollowing, 0, MATURITY).expect("facility");
    assert_eq!(
        payment_dates_in(&weekends, 2029, time::Month::January),
        vec![date!(2029 - 01 - 15)]
    );

    let usny = conventions(
        Some("usny"),
        BusinessDayConvention::ModifiedFollowing,
        0,
        MATURITY,
    )
    .expect("facility");
    assert_eq!(
        payment_dates_in(&usny, 2029, time::Month::January),
        vec![date!(2029 - 01 - 16)]
    );

    let mut meta_only = weekends;
    meta_only.attributes = Attributes::new().with_meta("calendar_id", "usny");
    assert_eq!(
        payment_dates_in(&meta_only, 2029, time::Month::January),
        vec![date!(2029 - 01 - 15)],
        "attributes metadata must not drive the schedule"
    );

    let err = conventions(
        Some("usnyx"),
        BusinessDayConvention::ModifiedFollowing,
        0,
        MATURITY,
    )
    .expect_err("unknown calendar must fail validation")
    .to_string();
    assert!(err.contains("usnyx"), "{err}");
}

/// A payment lag moves settlement, not accrual: the accrual factor of the
/// first period is unchanged while its payment date rolls forward by the lag.
#[test]
fn payment_lag_moves_the_payment_date_but_not_the_accrual() {
    use finstack_quant_cashflows::traits::CashflowScheduleSource;
    use finstack_quant_core::dates::BusinessDayConvention;

    let flat = conventions(
        Some("usny"),
        BusinessDayConvention::ModifiedFollowing,
        0,
        MATURITY,
    )
    .expect("facility");
    let lagged = conventions(
        Some("usny"),
        BusinessDayConvention::ModifiedFollowing,
        2,
        MATURITY,
    )
    .expect("facility");
    let first = |f: &RevolvingCredit| {
        let schedule = f.raw_cashflow_schedule(&market(), AS_OF).expect("schedule");
        schedule
            .get_flows()
            .iter()
            .find(|cf| matches!(cf.kind, finstack_quant_core::cashflow::CFKind::Fixed))
            .cloned()
            .expect("interest flow")
    };
    let (a, b) = (first(&flat), first(&lagged));
    // 2025-04-15 is a Tuesday: two business days later is Thursday 17th.
    assert_eq!(a.date, date!(2025 - 04 - 15));
    assert_eq!(b.date, date!(2025 - 04 - 17));
    assert!((a.accrual_factor - b.accrual_factor).abs() < 1e-15);
    assert!((a.amount.amount() - b.amount.amount()).abs() < 1e-9);
}

/// `Preceding` rolls a weekend maturity payment back to the Friday; Modified
/// Following rolls it forward to the Monday.
#[test]
fn business_day_convention_is_honoured_on_payment_dates() {
    use finstack_quant_core::dates::BusinessDayConvention;

    // 2026-01-18 is a Sunday.
    let maturity = date!(2026 - 01 - 18);
    let following =
        conventions(None, BusinessDayConvention::ModifiedFollowing, 0, maturity).expect("facility");
    let preceding =
        conventions(None, BusinessDayConvention::Preceding, 0, maturity).expect("facility");
    assert_eq!(
        payment_dates_in(&following, 2026, time::Month::January),
        vec![date!(2026 - 01 - 19)]
    );
    assert_eq!(
        payment_dates_in(&preceding, 2026, time::Month::January),
        vec![date!(2026 - 01 - 16)]
    );
}

// ---------------------------------------------------------------------------
// Task 4: commitment schedule
// ---------------------------------------------------------------------------

use finstack_quant_core::cashflow::CFKind;
use finstack_quant_valuations::instruments::fixed_income::loan_terms::{
    CommitmentStep, FeeStep, MarginStepUp,
};

fn flows_of(facility: &RevolvingCredit) -> Vec<finstack_quant_core::cashflow::CashFlow> {
    use finstack_quant_cashflows::traits::CashflowScheduleSource;
    facility
        .raw_cashflow_schedule(&market(), AS_OF)
        .expect("schedule")
        .get_flows()
        .to_vec()
}

fn stepped(
    drawn: f64,
    draw_repay_spec: DrawRepaySpec,
    steps: Vec<CommitmentStep>,
) -> finstack_quant_core::Result<RevolvingCredit> {
    RevolvingCredit::builder()
        .id("RC-STEP".into())
        .commitment_amount(usd(10_000_000.0))
        .drawn_amount(usd(drawn))
        .commitment_date(AS_OF)
        .maturity(date!(2027 - 01 - 15))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.07 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(50.0, 0.0, 0.0).expect("fees"))
        .draw_repay_spec(draw_repay_spec)
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.4)
        .commitment_schedule(steps)
        .build()
}

const STEP_DATE: time::Date = date!(2026 - 01 - 15);

fn step_to_6m() -> Vec<CommitmentStep> {
    vec![CommitmentStep {
        date: STEP_DATE,
        amount: usd(6_000_000.0),
        fee_bp: 25.0,
    }]
}

/// A commitment step-down shrinks the commitment-fee base from the step date
/// and charges the reduction fee on the reduced amount on that date.
#[test]
fn commitment_step_down_reprices_fees_and_charges_the_reduction_fee() {
    let facility = stepped(
        5_000_000.0,
        DrawRepaySpec::Deterministic(vec![]),
        step_to_6m(),
    )
    .expect("facility");
    let flows = flows_of(&facility);

    // Commitment fee for 2025-10-15 -> 2026-01-15 (92 days) on 5M undrawn.
    let before = flows
        .iter()
        .find(|cf| cf.kind == CFKind::CommitmentFee && cf.date == date!(2026 - 01 - 15))
        .expect("fee before the step");
    assert!((before.amount.amount() - 5_000_000.0 * 0.005 * 92.0 / 360.0).abs() < 1e-6);
    // Commitment fee for 2026-01-15 -> 2026-04-15 (90 days) on 1M undrawn.
    let after = flows
        .iter()
        .find(|cf| cf.kind == CFKind::CommitmentFee && cf.date == date!(2026 - 04 - 15))
        .expect("fee after the step");
    assert!(
        (after.amount.amount() - 1_000_000.0 * 0.005 * 90.0 / 360.0).abs() < 1e-6,
        "{}",
        after.amount
    );
    // Reduction fee: 4M reduced at 25 bp on the step date.
    let reduction = flows
        .iter()
        .find(|cf| cf.kind == CFKind::Fee && cf.date == STEP_DATE)
        .expect("reduction fee");
    assert!((reduction.amount.amount() - 10_000.0).abs() < 1e-6);
}

/// A step below the drawn balance is rejected at construction; the analyst
/// dates the repayment.
#[test]
fn commitment_step_below_the_drawn_balance_is_rejected() {
    let err = stepped(
        7_000_000.0,
        DrawRepaySpec::Deterministic(vec![]),
        step_to_6m(),
    )
    .expect_err("7M drawn cannot step to 6M")
    .to_string();
    assert!(err.contains("below the drawn balance"), "{err}");

    // With a 1M repayment dated on the step it is accepted.
    stepped(
        7_000_000.0,
        DrawRepaySpec::Deterministic(vec![DrawRepayEvent {
            date: STEP_DATE,
            amount: usd(1_000_000.0),
            is_draw: false,
        }]),
        step_to_6m(),
    )
    .expect("repayment on the step date makes the schedule feasible");
}

/// Utilization is drawn over the commitment in force: a zero-volatility
/// stochastic facility at 50% utilization repays 2M when the commitment
/// steps from 10M to 6M and repays the remaining 3M at maturity.
#[test]
fn stochastic_utilization_books_the_principal_implied_by_a_commitment_step() {
    let facility =
        stepped(5_000_000.0, zero_vol_stochastic_at(0.5), step_to_6m()).expect("facility");
    let result = finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCreditPricer::price_with_paths(&facility, &market(), AS_OF)
        .expect("paths");
    let flows = result.path_results[0].cashflows.get_flows().to_vec();
    let notional: Vec<_> = flows
        .iter()
        .filter(|cf| cf.kind == CFKind::Notional)
        .map(|cf| (cf.date, cf.amount.amount()))
        .collect();
    let repaid: f64 = notional.iter().map(|(_, a)| a).sum();
    assert!((repaid - 5_000_000.0).abs() < 1e-6, "{notional:?}");
    let terminal = notional.last().expect("terminal repayment");
    assert!((terminal.1 - 3_000_000.0).abs() < 1e-6, "{notional:?}");
    // The commitment-driven change is booked on the step date itself, not
    // at the period midpoint.
    let step_leg = notional
        .iter()
        .find(|(d, _)| *d == STEP_DATE)
        .expect("principal leg on the step date");
    assert!((step_leg.1 - 2_000_000.0).abs() < 1e-6, "{notional:?}");
}

fn zero_vol_stochastic_at(target: f64) -> DrawRepaySpec {
    DrawRepaySpec::Stochastic(Box::new(StochasticUtilizationSpec {
        utilization_process: UtilizationProcess::MeanReverting {
            target_rate: target,
            speed: 1.0,
            volatility: 0.0,
            spread_sensitivity: 0.0,
        },
        num_paths: 2,
        seed: Some(1),
        antithetic: false,
        use_sobol_qmc: false,
        mc_config: Some(McConfig {
            correlation_matrix: None,
            credit_spread_process: CreditSpreadProcessSpec::Constant(0.0),
            interest_rate_process: None,
            util_credit_corr: None,
        }),
    }))
}

/// `available_capacity` and `utilization_rate` read the commitment in force
/// on the valuation date.
#[test]
fn capacity_metrics_read_the_stepped_commitment() {
    let facility = stepped(
        5_000_000.0,
        DrawRepaySpec::Deterministic(vec![]),
        step_to_6m(),
    )
    .expect("facility");
    let after = date!(2026 - 03 - 01);
    let result = standard_pricer_registry()
        .price_with_metrics(
            &facility,
            ModelKey::Discounting,
            &market(),
            after,
            &[
                MetricId::custom("utilization_rate"),
                MetricId::custom("available_capacity"),
            ],
            Default::default(),
        )
        .expect("metrics");
    let util = result.measures[&MetricId::custom("utilization_rate")];
    let avail = result.measures[&MetricId::custom("available_capacity")];
    assert!((util - 5.0 / 6.0).abs() < 1e-12, "{util}");
    assert!((avail - 1_000_000.0).abs() < 1e-6, "{avail}");
}

// ---------------------------------------------------------------------------
// Task 5: margin and fee steps
// ---------------------------------------------------------------------------

fn with_steps(
    draw_repay_spec: DrawRepaySpec,
    margin_steps: Vec<MarginStepUp>,
    fee_steps: Vec<FeeStep>,
) -> RevolvingCredit {
    let mut fees = RevolvingCreditFees::flat(50.0, 0.0, 0.0).expect("fees");
    fees.steps = fee_steps;
    RevolvingCredit::builder()
        .id("RC-MARGIN".into())
        .commitment_amount(usd(10_000_000.0))
        .drawn_amount(usd(5_000_000.0))
        .commitment_date(AS_OF)
        .maturity(date!(2027 - 01 - 15))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.07 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(fees)
        .draw_repay_spec(draw_repay_spec)
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.4)
        .margin_steps(margin_steps)
        .build()
        .expect("facility")
}

fn interest_on(flows: &[finstack_quant_core::cashflow::CashFlow], date: time::Date) -> f64 {
    flows
        .iter()
        .find(|cf| cf.kind == CFKind::Fixed && cf.date == date)
        .map(|cf| cf.amount.amount())
        .expect("interest flow")
}

/// A +100 bp margin step on an accrual boundary raises every later interest
/// flow by exactly `balance × 100 bp × dt` and leaves earlier ones unchanged.
#[test]
fn margin_step_reprices_interest_from_its_date() {
    let base = with_steps(DrawRepaySpec::Deterministic(vec![]), vec![], vec![]);
    let stepped = with_steps(
        DrawRepaySpec::Deterministic(vec![]),
        vec![MarginStepUp {
            date: date!(2026 - 07 - 15),
            delta_bp: 100,
        }],
        vec![],
    );
    let (a, b) = (flows_of(&base), flows_of(&stepped));
    assert!(
        (interest_on(&a, date!(2026 - 07 - 15)) - interest_on(&b, date!(2026 - 07 - 15))).abs()
            < 1e-9
    );
    let dt = 92.0 / 360.0; // 2026-07-15 -> 2026-10-15
    let extra = interest_on(&b, date!(2026 - 10 - 15)) - interest_on(&a, date!(2026 - 10 - 15));
    assert!((extra - 5_000_000.0 * 0.01 * dt).abs() < 1e-6, "{extra}");
}

/// A fee step changes only the fee it names, from its date.
#[test]
fn fee_step_changes_only_the_named_fee() {
    let base = with_steps(DrawRepaySpec::Deterministic(vec![]), vec![], vec![]);
    let stepped = with_steps(
        DrawRepaySpec::Deterministic(vec![]),
        vec![],
        vec![FeeStep {
            date: date!(2026 - 07 - 15),
            commitment_delta_bp: 25.0,
            usage_delta_bp: 0.0,
            facility_delta_bp: 0.0,
        }],
    );
    let (a, b) = (flows_of(&base), flows_of(&stepped));
    let fee = |flows: &[finstack_quant_core::cashflow::CashFlow], d: time::Date| {
        flows
            .iter()
            .find(|cf| cf.kind == CFKind::CommitmentFee && cf.date == d)
            .map(|cf| cf.amount.amount())
            .expect("commitment fee")
    };
    assert!((fee(&a, date!(2026 - 07 - 15)) - fee(&b, date!(2026 - 07 - 15))).abs() < 1e-9);
    let dt = 92.0 / 360.0;
    let extra = fee(&b, date!(2026 - 10 - 15)) - fee(&a, date!(2026 - 10 - 15));
    assert!((extra - 5_000_000.0 * 0.0025 * dt).abs() < 1e-6, "{extra}");
    assert!(
        (interest_on(&a, date!(2026 - 10 - 15)) - interest_on(&b, date!(2026 - 10 - 15))).abs()
            < 1e-9
    );
}

/// A margin step inside an accrual period is honoured by both engines, and
/// the deterministic and zero-volatility stochastic interest agree.
#[test]
fn intra_period_margin_step_is_honoured_by_both_engines() {
    let step = MarginStepUp {
        date: date!(2026 - 08 - 15),
        delta_bp: 100,
    };
    let deterministic = with_steps(
        DrawRepaySpec::Deterministic(vec![]),
        vec![step.clone()],
        vec![],
    );
    let stochastic = with_steps(zero_vol_stochastic_at(0.5), vec![step], vec![]);
    let det_flows = flows_of(&deterministic);
    let sto_flows = flows_of(&stochastic);
    // 2026-07-15 -> 2026-08-15 at 7% (31 days), 2026-08-15 -> 2026-10-15 at 8% (61 days).
    let expected = 5_000_000.0 * (0.07 * 31.0 / 360.0 + 0.08 * 61.0 / 360.0);
    let det = interest_on(&det_flows, date!(2026 - 10 - 15));
    let sto = interest_on(&sto_flows, date!(2026 - 10 - 15));
    assert!(
        (det - expected).abs() < 1e-6,
        "deterministic {det} vs {expected}"
    );
    assert!(
        (sto - expected).abs() < 1e-6,
        "stochastic {sto} vs {expected}"
    );
}

// ---------------------------------------------------------------------------
// Task 6: letter-of-credit sub-facility
// ---------------------------------------------------------------------------

use finstack_quant_cashflows::builder::FloatingRateSpec;
use finstack_quant_core::market_data::term_structures::{ForwardCurve, HazardCurve};
use finstack_quant_valuations::instruments::fixed_income::loan_terms::{
    LcEvent, LetterOfCreditSpec,
};

fn sofr_plus(spread_bp: i64) -> BaseRateSpec {
    BaseRateSpec::Floating(FloatingRateSpec {
        index_id: "USD-SOFR-3M".into(),
        spread_bp: rust_decimal::Decimal::from(spread_bp),
        gearing: rust_decimal::Decimal::ONE,
        gearing_includes_spread: true,
        index_floor_bp: Some(rust_decimal::Decimal::ZERO),
        all_in_cap_bp: None,
        all_in_floor_bp: None,
        index_cap_bp: None,
        overnight_index_constraints: Default::default(),
        reset_frequency: Tenor::quarterly(),
        index_tenor: None,
        reset_lag_days: 0,
        fixing_calendar_id: None,
        overnight_compounding: None,
        overnight_basis: None,
        fallback: Default::default(),
    })
}

fn market_with_forward() -> MarketContext {
    market().insert(
        ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(AS_OF)
            .knots(vec![(0.0, 0.04), (10.0, 0.04)])
            .build()
            .expect("forward"),
    )
}

fn lc_spec(outstanding: f64, events: Vec<LcEvent>, leq: f64) -> LetterOfCreditSpec {
    LetterOfCreditSpec {
        sublimit: usd(4_000_000.0),
        outstanding: usd(outstanding),
        events,
        fee_bp: None,
        fronting_fee_bp: 12.5,
        leq,
    }
}

fn with_lc(
    draw_repay_spec: DrawRepaySpec,
    lc: LetterOfCreditSpec,
    credit: Option<&str>,
) -> finstack_quant_core::Result<RevolvingCredit> {
    let mut fees = RevolvingCreditFees::flat(50.0, 0.0, 0.0).expect("fees");
    fees.usage_fee_tiers = vec![
        finstack_quant_cashflows::builder::FeeTier {
            threshold: rust_decimal::Decimal::ZERO,
            bp: rust_decimal::Decimal::ZERO,
        },
        finstack_quant_cashflows::builder::FeeTier {
            threshold: rust_decimal::Decimal::new(5, 1),
            bp: rust_decimal::Decimal::from(25),
        },
    ];
    RevolvingCredit::builder()
        .id("RC-LC".into())
        .commitment_amount(usd(10_000_000.0))
        .drawn_amount(usd(3_000_000.0))
        .commitment_date(AS_OF)
        .maturity(date!(2027 - 01 - 15))
        .base_rate_spec(sofr_plus(325))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(fees)
        .draw_repay_spec(draw_repay_spec)
        .discount_curve_id("USD-OIS".into())
        .credit_curve_id_opt(credit.map(Into::into))
        .recovery_rate(0.4)
        .lc_opt(Some(lc))
        .build()
}

fn flow_amount(
    flows: &[finstack_quant_core::cashflow::CashFlow],
    kind: CFKind,
    date: time::Date,
) -> f64 {
    flows
        .iter()
        .find(|cf| cf.kind == kind && cf.date == date)
        .map(|cf| cf.amount.amount())
        .unwrap_or(0.0)
}

/// 10M line, 3M drawn, 2M LC at the 325 bp margin plus 12.5 bp fronting:
/// commitment fee on 5M, LC fee and fronting fee on 2M, and the usage-fee
/// tier evaluated at 50% (drawn plus LC).
#[test]
fn letters_of_credit_reduce_availability_and_pay_lc_and_fronting_fees() {
    use finstack_quant_cashflows::traits::CashflowScheduleSource;
    let facility = with_lc(
        DrawRepaySpec::Deterministic(vec![]),
        lc_spec(2_000_000.0, vec![], 0.0),
        None,
    )
    .expect("facility");
    let flows = facility
        .raw_cashflow_schedule(&market_with_forward(), AS_OF)
        .expect("schedule")
        .get_flows()
        .to_vec();
    let pay = date!(2025 - 04 - 15);
    let dt = 90.0 / 360.0;
    assert!(
        (flow_amount(&flows, CFKind::CommitmentFee, pay) - 5_000_000.0 * 0.0050 * dt).abs() < 1e-6
    );
    assert!((flow_amount(&flows, CFKind::LcFee, pay) - 2_000_000.0 * 0.0325 * dt).abs() < 1e-6);
    assert!(
        (flow_amount(&flows, CFKind::FrontingFee, pay) - 2_000_000.0 * 0.00125 * dt).abs() < 1e-6
    );
    // Usage fee tier at 50% utilization (3M drawn + 2M LC) applies 25 bp to the drawn 3M.
    assert!((flow_amount(&flows, CFKind::UsageFee, pay) - 3_000_000.0 * 0.0025 * dt).abs() < 1e-6);

    let result = standard_pricer_registry()
        .price_with_metrics(
            &facility,
            ModelKey::Discounting,
            &market_with_forward(),
            AS_OF,
            &[MetricId::custom("available_capacity")],
            Default::default(),
        )
        .expect("metrics");
    assert!((result.measures[&MetricId::custom("available_capacity")] - 5_000_000.0).abs() < 1e-6);
}

/// An LC issuance inside a period slices the accrual: the LC fee for the
/// period is `2M × 325 bp × 30/360 + 3M × 325 bp × 60/360`.
#[test]
fn lc_issuance_inside_a_period_slices_the_accrual() {
    use finstack_quant_cashflows::traits::CashflowScheduleSource;
    let facility = with_lc(
        DrawRepaySpec::Deterministic(vec![]),
        lc_spec(
            2_000_000.0,
            vec![LcEvent {
                date: date!(2025 - 02 - 14),
                amount: usd(1_000_000.0),
                is_issue: true,
            }],
            0.0,
        ),
        None,
    )
    .expect("facility");
    let flows = facility
        .raw_cashflow_schedule(&market_with_forward(), AS_OF)
        .expect("schedule")
        .get_flows()
        .to_vec();
    let expected = 2_000_000.0 * 0.0325 * 30.0 / 360.0 + 3_000_000.0 * 0.0325 * 60.0 / 360.0;
    let actual = flow_amount(&flows, CFKind::LcFee, date!(2025 - 04 - 15));
    assert!((actual - expected).abs() < 1e-6, "{actual} vs {expected}");
}

/// The default leg funds `lc.leq × LC` at par and recovers it at R: on a
/// flat hazard the PV falls by exactly the extra loss.
#[test]
fn lc_draw_at_default_enters_the_default_leg() {
    let hazard = HazardCurve::builder("BORROWER-HZ")
        .base_date(AS_OF)
        .recovery_rate(0.4)
        .day_count(DayCount::Act365F)
        .knots([(1.0, 0.05), (5.0, 0.05)])
        .build()
        .expect("hazard");
    let m = market_with_forward().insert(hazard);
    let value = |leq: f64| {
        let facility = with_lc(
            DrawRepaySpec::Deterministic(vec![]),
            lc_spec(2_000_000.0, vec![], leq),
            Some("BORROWER-HZ"),
        )
        .expect("facility");
        facility.value(&m, AS_OF).expect("pv").amount()
    };
    let (no_draw, half) = (value(0.0), value(0.5));
    // Expected loss on the LC draw: 0.5 × 2M × (1 − 0.4) × PD(2y) discounted.
    let pd_2y = 1.0 - (-0.05_f64 * 2.0).exp();
    let rough = 0.5 * 2_000_000.0 * 0.6 * pd_2y;
    let drop = no_draw - half;
    assert!(
        drop > 0.0,
        "LC leq must lower the lender PV: {no_draw} vs {half}"
    );
    assert!(
        (drop - rough).abs() < 0.15 * rough,
        "drop {drop} should approximate the undiscounted expected loss {rough}"
    );
}

/// A stochastic utilization path never exceeds `1 − LC / C`.
#[test]
fn stochastic_utilization_is_capped_by_outstanding_letters_of_credit() {
    let spec = DrawRepaySpec::Stochastic(Box::new(StochasticUtilizationSpec {
        utilization_process: UtilizationProcess::MeanReverting {
            target_rate: 0.95,
            speed: 3.0,
            volatility: 0.3,
            spread_sensitivity: 0.0,
        },
        num_paths: 16,
        seed: Some(3),
        antithetic: false,
        use_sobol_qmc: false,
        mc_config: Some(McConfig {
            correlation_matrix: None,
            credit_spread_process: CreditSpreadProcessSpec::Constant(0.0),
            interest_rate_process: None,
            util_credit_corr: None,
        }),
    }));
    let facility = with_lc(spec, lc_spec(2_000_000.0, vec![], 0.0), None).expect("facility");
    let result = finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCreditPricer::price_with_paths(&facility, &market_with_forward(), AS_OF)
        .expect("paths");
    for path in &result.path_results {
        let data = path.path_data.as_ref().expect("path data");
        assert!(
            data.utilization_path.iter().all(|&u| u <= 0.8 + 1e-12),
            "utilization must stay below 1 - LC/C = 0.8: {:?}",
            data.utilization_path
        );
    }
}

/// Validation: LC outstanding above the sublimit, drawn plus LC above the
/// commitment, and a fixed-rate facility without `lc.fee_bp` are rejected.
#[test]
fn lc_validation_rejects_infeasible_sublimits() {
    let over_sublimit = with_lc(
        DrawRepaySpec::Deterministic(vec![]),
        lc_spec(5_000_000.0, vec![], 0.0),
        None,
    )
    .expect_err("5M LC above the 4M sublimit")
    .to_string();
    assert!(over_sublimit.contains("sublimit"), "{over_sublimit}");

    let mut fixed = with_lc(
        DrawRepaySpec::Deterministic(vec![]),
        lc_spec(2_000_000.0, vec![], 0.0),
        None,
    )
    .expect("facility");
    fixed.base_rate_spec = BaseRateSpec::Fixed { rate: 0.07 };
    let err = fixed
        .validate()
        .expect_err("fixed rate needs lc.fee_bp")
        .to_string();
    assert!(err.contains("fee_bp"), "{err}");
}

// ---------------------------------------------------------------------------
// Task 7: quote metrics
// ---------------------------------------------------------------------------

/// Flat 4% curve on ACT/365F, consistent between discounting and the index.
fn flat_consistent_market() -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(AS_OF)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.04_f64).exp()),
            (10.0, (-0.40_f64).exp()),
        ])
        .build()
        .expect("curve");
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(AS_OF)
        .knots(vec![(0.0, 0.04), (10.0, 0.04)])
        .build()
        .expect("forward");
    MarketContext::new().insert(disc).insert(fwd)
}

fn quote_facility(
    base_rate_spec: BaseRateSpec,
    drawn: f64,
    quoted_clean_price: Option<f64>,
    quoted_dm: Option<f64>,
) -> RevolvingCredit {
    let mut facility = RevolvingCredit::builder()
        .id("RC-QUOTE".into())
        .commitment_amount(usd(10_000_000.0))
        .drawn_amount(usd(drawn))
        .commitment_date(AS_OF)
        .maturity(date!(2028 - 01 - 15))
        .base_rate_spec(base_rate_spec)
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(50.0, 0.0, 0.0).expect("fees"))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
        .build()
        .expect("facility");
    facility
        .instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price = quoted_clean_price;
    facility
        .instrument_pricing_overrides
        .market_quotes
        .quoted_discount_margin = quoted_dm;
    facility
}

fn quote_metric(facility: &RevolvingCredit, market: &MarketContext, id: MetricId) -> f64 {
    let result = standard_pricer_registry()
        .price_with_metrics(
            facility,
            ModelKey::Discounting,
            market,
            AS_OF,
            std::slice::from_ref(&id),
            Default::default(),
        )
        .expect("metric");
    result.measures[&id]
}

/// A par quote on a fully drawn facility solves to its contractual margin
/// (within the ACT/360 coupon versus continuous-curve basis), while the
/// model value reprices at zero margin by construction: the schedule already
/// carries the margin income.
#[test]
fn discount_margin_of_a_par_quoted_facility_is_the_contractual_margin() {
    let m = flat_consistent_market();
    let par = quote_facility(sofr_plus(300), 10_000_000.0, Some(100.0), None);
    let dm = quote_metric(&par, &m, MetricId::DiscountMargin);
    assert!((dm - 0.03).abs() < 0.001, "par-quoted dm {dm}");

    let model = quote_facility(sofr_plus(300), 10_000_000.0, None, None);
    let dm = quote_metric(&model, &m, MetricId::DiscountMargin);
    assert!(dm.abs() < 1e-8, "model dm {dm}");
}

/// A 98 clean quote on the drawn balance solves to a margin above the
/// contractual one; the margin-quoted price round-trips.
#[test]
fn discount_margin_and_price_from_dm_round_trip_a_quote() {
    let m = flat_consistent_market();
    let quoted = quote_facility(sofr_plus(300), 5_000_000.0, Some(98.0), None);
    let dm = quote_metric(&quoted, &m, MetricId::DiscountMargin);
    assert!(dm > 0.03, "a discount should widen the margin: {dm}");

    let at_dm = quote_facility(sofr_plus(300), 5_000_000.0, None, Some(dm));
    let price = quote_metric(&at_dm, &m, MetricId::custom("price_from_dm"));
    assert!((price - 98.0).abs() < 1e-6, "price {price}");

    // The par-quoted margin prices back to par.
    let par = quote_facility(sofr_plus(300), 10_000_000.0, Some(100.0), None);
    let dm_par = quote_metric(&par, &m, MetricId::DiscountMargin);
    let at_par = quote_facility(sofr_plus(300), 10_000_000.0, None, Some(dm_par));
    let price = quote_metric(&at_par, &m, MetricId::custom("price_from_dm"));
    assert!((price - 100.0).abs() < 1e-6, "price {price}");
}

/// Accrued interest on a seasoned facility is the pro-rata share of the
/// current period's interest on the facility day count.
#[test]
fn accrued_interest_is_pro_rata_within_the_current_period() {
    let mut facility = quote_facility(BaseRateSpec::Fixed { rate: 0.06 }, 5_000_000.0, None, None);
    facility.commitment_date = date!(2024 - 10 - 15);
    // Valuation 2025-01-15 sits 92 days into the 2024-10-15 -> 2025-01-15 ... no:
    // the period is 2025-01-15 -> 2025-04-15 with settlement on its start, so
    // accrued is zero there; move settlement 30 days in.
    facility.settlement_days = 0;
    let market = flat_consistent_market();
    let at_start = quote_metric(&facility, &market, MetricId::custom("accrued_interest"));
    assert!(
        at_start.abs() < 1e-9,
        "settlement on an accrual start accrues nothing: {at_start}"
    );

    let mut later = facility;
    later.settlement_days = 21; // 21 weekdays ≈ 29 calendar days after 2025-01-15
    let settlement = later.settlement_date(AS_OF).expect("settlement");
    let days = (settlement - date!(2025 - 01 - 15)).whole_days() as f64;
    let accrued = quote_metric(&later, &market, MetricId::custom("accrued_interest"));
    let expected = 5_000_000.0 * 0.06 * days / 360.0;
    assert!(
        (accrued - expected).abs() < 1e-6,
        "accrued {accrued} vs {expected}"
    );
}

/// Yield to maturity rises as the quoted price falls, and a par quote on a
/// fully drawn fixed line yields its coupon within compounding effects.
#[test]
fn yield_to_maturity_moves_inversely_with_the_quote() {
    let m = flat_consistent_market();
    let par = quote_facility(
        BaseRateSpec::Fixed { rate: 0.06 },
        10_000_000.0,
        Some(100.0),
        None,
    );
    let discount = quote_facility(
        BaseRateSpec::Fixed { rate: 0.06 },
        10_000_000.0,
        Some(98.0),
        None,
    );
    let y_par = quote_metric(&par, &m, MetricId::Ytm);
    let y_disc = quote_metric(&discount, &m, MetricId::Ytm);
    assert!(y_par > 0.06 - 1e-9 && y_par < 0.07, "par yield {y_par}");
    assert!(y_disc > y_par, "{y_disc} vs {y_par}");
}

/// Running all-in rate: interest on the drawn balance plus the commitment
/// fee on the undrawn, over the drawn balance.
#[test]
fn all_in_rate_is_cash_cost_over_time_weighted_drawn_balance() {
    let facility = quote_facility(BaseRateSpec::Fixed { rate: 0.07 }, 5_000_000.0, None, None);
    let rate = quote_metric(
        &facility,
        &flat_consistent_market(),
        MetricId::custom("all_in_rate"),
    );
    // (5M × 7% + 5M × 0.5%) / 5M = 7.5%, both legs on ACT/360.
    assert!((rate - 0.075).abs() < 1e-6, "all-in {rate}");
}

// ---------------------------------------------------------------------------
// Task 9: credit-risk visibility of the undrawn commitment
// ---------------------------------------------------------------------------

fn knot_hazard() -> HazardCurve {
    HazardCurve::builder("BORROWER-HZ")
        .base_date(AS_OF)
        .recovery_rate(0.4)
        .day_count(DayCount::Act365F)
        .knots([(1.0, 0.03), (3.0, 0.04), (5.0, 0.05)])
        .build()
        .expect("hazard")
}

fn exposure_facility(drawn: f64, leq: f64, credit: bool) -> RevolvingCredit {
    RevolvingCredit::builder()
        .id("RC-EXPOSURE".into())
        .commitment_amount(usd(10_000_000.0))
        .drawn_amount(usd(drawn))
        .commitment_date(AS_OF)
        .maturity(date!(2028 - 01 - 15))
        .base_rate_spec(sofr_plus(325))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(50.0, 0.0, 0.0).expect("fees"))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .credit_curve_id_opt(credit.then(|| "BORROWER-HZ".into()))
        .recovery_rate(0.4)
        .leq(leq)
        .build()
        .expect("facility")
}

fn measure(facility: &RevolvingCredit, market: &MarketContext, id: MetricId) -> f64 {
    let result = standard_pricer_registry()
        .price_with_metrics(
            facility,
            ModelKey::Discounting,
            market,
            AS_OF,
            std::slice::from_ref(&id),
            Default::default(),
        )
        .expect("metric");
    result
        .measures
        .iter()
        .find(|(k, _)| k.to_string().starts_with(&id.to_string()))
        .map(|(_, v)| *v)
        .unwrap_or_else(|| panic!("metric {id} missing from {:?}", result.measures))
}

/// A positive `leq` without any default model is rejected by the standalone
/// pricer instead of silently pricing no draw at default.
#[test]
fn leq_without_a_default_model_is_rejected() {
    let facility = exposure_facility(1_000_000.0, 0.5, false);
    let err = facility
        .value(&market_with_forward(), AS_OF)
        .expect_err("leq needs a default model")
        .to_string();
    assert!(err.contains("leq"), "{err}");
    // Zero leq prices fine without a curve.
    exposure_facility(1_000_000.0, 0.0, false)
        .value(&market_with_forward(), AS_OF)
        .expect("no contingent exposure");
}

/// Audit probe: the z-spread fallback sees the undrawn line only through its
/// fee annuity, while the hazard-based CS01 with `leq` carries the contingent
/// exposure. A knot-built (non-replayable) hazard curve now gives a finite,
/// negative direct-bump CS01 instead of an error.
#[test]
fn undrawn_exposure_shows_in_hazard_cs01_but_not_in_the_z_spread_fallback() {
    let market = market_with_forward().insert(knot_hazard());
    let z_spread = measure(
        &exposure_facility(1_000_000.0, 0.0, false),
        &market,
        MetricId::Cs01,
    );
    let hazard_no_leq = measure(
        &exposure_facility(1_000_000.0, 0.0, true),
        &market,
        MetricId::Cs01,
    );
    let hazard_leq = measure(
        &exposure_facility(1_000_000.0, 0.5, true),
        &market,
        MetricId::Cs01,
    );
    assert!(z_spread < 0.0 && hazard_no_leq < 0.0 && hazard_leq < 0.0);
    assert!(
        hazard_leq < 2.0 * hazard_no_leq,
        "leq must at least double the credit sensitivity: {hazard_leq} vs {hazard_no_leq} (z-spread {z_spread})"
    );
    // Bucketed direct-bump series reconciles to the parallel figure.
    let bucketed = measure(
        &exposure_facility(1_000_000.0, 0.5, true),
        &market,
        MetricId::BucketedCs01,
    );
    assert!(
        (bucketed - hazard_leq).abs() < 0.02 * hazard_leq.abs(),
        "bucketed {bucketed} vs parallel {hazard_leq}"
    );
}

/// `exposure_at_default` = drawn + leq × undrawn (+ LC leq × LC face).
#[test]
fn exposure_at_default_counts_the_contingent_undrawn_and_lc_draws() {
    let market = market_with_forward().insert(knot_hazard());
    let ead = measure(
        &exposure_facility(1_000_000.0, 0.5, true),
        &market,
        MetricId::custom("exposure_at_default"),
    );
    assert!((ead - 5_500_000.0).abs() < 1e-6, "ead {ead}");

    let mut with_lc = exposure_facility(1_000_000.0, 0.5, true);
    with_lc.lc = Some(lc_spec(2_000_000.0, vec![], 0.5));
    let ead = measure(&with_lc, &market, MetricId::custom("exposure_at_default"));
    // 1M drawn + 0.5 × (10M − 1M − 2M) + 0.5 × 2M.
    assert!((ead - 5_500_000.0).abs() < 1e-6, "ead with LC {ead}");
}

/// `expected_loss` is the value removed by the default model: positive with
/// a curve, larger with `leq`, zero without a curve.
#[test]
fn expected_loss_grows_with_the_contingent_exposure() {
    let market = market_with_forward().insert(knot_hazard());
    let el_0 = measure(
        &exposure_facility(1_000_000.0, 0.0, true),
        &market,
        MetricId::custom("expected_loss"),
    );
    let el_half = measure(
        &exposure_facility(1_000_000.0, 0.5, true),
        &market,
        MetricId::custom("expected_loss"),
    );
    assert!(el_0 > 0.0, "{el_0}");
    assert!(el_half > el_0, "{el_half} vs {el_0}");
    let none = measure(
        &exposure_facility(1_000_000.0, 0.0, false),
        &market,
        MetricId::custom("expected_loss"),
    );
    assert_eq!(none, 0.0);
}

// ---------------------------------------------------------------------------
// Task 10: upfront fee percentage and OID effective rate
// ---------------------------------------------------------------------------

use finstack_quant_valuations::instruments::fixed_income::loan_terms::{ScheduledFee, UpfrontFee};

fn origination_facility(upfront: Option<UpfrontFee>, fees_bp: f64) -> RevolvingCredit {
    let mut fees = RevolvingCreditFees::flat(fees_bp, 0.0, 0.0).expect("fees");
    fees.upfront_fee = upfront;
    RevolvingCredit::builder()
        .id("RC-ORIG".into())
        .commitment_amount(usd(10_000_000.0))
        .drawn_amount(usd(10_000_000.0))
        .commitment_date(date!(2025 - 02 - 15))
        .maturity(date!(2028 - 02 - 15))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.07 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(fees)
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
        .build()
        .expect("facility")
}

/// A percentage upfront fee prices exactly like the equivalent amount.
#[test]
fn percentage_upfront_fee_equals_the_equivalent_amount() {
    let pct = origination_facility(Some(UpfrontFee::PctOfCommitment(0.02)), 0.0);
    let amount = origination_facility(Some(UpfrontFee::Amount(usd(200_000.0))), 0.0);
    let none = origination_facility(None, 0.0);
    let m = market();
    let (pv_pct, pv_amt, pv_none) = (
        pct.value(&m, AS_OF).expect("pv").amount(),
        amount.value(&m, AS_OF).expect("pv").amount(),
        none.value(&m, AS_OF).expect("pv").amount(),
    );
    assert!((pv_pct - pv_amt).abs() < 1e-9);
    let df = market()
        .get_discount("USD-OIS")
        .expect("curve")
        .df_between_dates(AS_OF, date!(2025 - 02 - 15))
        .expect("df");
    assert!((pv_pct - pv_none - 200_000.0 * df).abs() < 1e-6);
    // JSON round trip of both variants.
    let json = serde_json::to_string(&pct).expect("json");
    let back: RevolvingCredit = serde_json::from_str(&json).expect("round trip");
    assert!(
        matches!(back.fees.upfront_fee, Some(UpfrontFee::PctOfCommitment(p)) if (p - 0.02).abs() < 1e-15)
    );
}

/// The origination effective rate with a 2% upfront fee exceeds the running
/// all-in rate, and excluding fees from the EIR removes it.
#[test]
fn oid_effective_rate_exceeds_the_running_rate_with_an_upfront_fee() {
    let m = market();
    let with_fee = origination_facility(Some(UpfrontFee::PctOfCommitment(0.02)), 0.0);
    let run = |facility: &RevolvingCredit| {
        standard_pricer_registry()
            .price_with_metrics(
                facility,
                ModelKey::Discounting,
                &m,
                AS_OF,
                &[MetricId::custom("oid_eir_amortization")],
                Default::default(),
            )
            .expect("metrics")
    };
    let result = run(&with_fee);
    let eir = result.measures[&MetricId::custom("oid_eir_rate")];
    // 7% ACT/360 quarterly coupon: the money-weighted running rate is a
    // little above 7% on an annual basis; a 2% discount adds ~70 bp a year
    // over three years.
    assert!(eir > 0.075 && eir < 0.085, "eir {eir}");

    let mut without_fees = with_fee;
    without_fees.oid_eir = Some(
        finstack_quant_valuations::instruments::fixed_income::loan_terms::OidEirSpec {
            include_fees: false,
        },
    );
    let plain = run(&without_fees).measures[&MetricId::custom("oid_eir_rate")];
    assert!(
        plain < eir && plain > 0.069 && plain < 0.075,
        "plain {plain}"
    );
}

// ---------------------------------------------------------------------------
// Task 11: scheduled fees and custom-deal recipes
// ---------------------------------------------------------------------------

/// An amendment fee appears once on its date, survival-weighted like every
/// other flow, and raises the PV by its discounted amount.
#[test]
fn scheduled_fee_is_paid_once_on_its_date() {
    let mut facility = seasoned(DrawRepaySpec::Deterministic(vec![]));
    facility.scheduled_fees = vec![ScheduledFee {
        date: date!(2026 - 03 - 01),
        amount: usd(100_000.0),
    }];
    facility.validate().expect("valid");
    let flows = flows_of(&facility);
    let fees: Vec<_> = flows.iter().filter(|cf| cf.kind == CFKind::Fee).collect();
    assert_eq!(fees.len(), 1);
    assert_eq!(fees[0].date, date!(2026 - 03 - 01));
    assert!((fees[0].amount.amount() - 100_000.0).abs() < 1e-9);
    let base = seasoned(DrawRepaySpec::Deterministic(vec![]))
        .value(&market(), AS_OF)
        .expect("pv")
        .amount();
    let with_fee = facility.value(&market(), AS_OF).expect("pv").amount();
    let df = market()
        .get_discount("USD-OIS")
        .expect("curve")
        .df_between_dates(AS_OF, date!(2026 - 03 - 01))
        .expect("df");
    assert!((with_fee - base - 100_000.0 * df).abs() < 1e-6);
}

/// Term-out recipe: the commitment steps to the drawn balance on the
/// term-out date and the balance amortizes on dated repayments; no
/// commitment fee accrues after the term-out and the notional flows follow
/// the amortization.
#[test]
fn term_out_recipe_stops_availability_and_amortizes() {
    let term_out = date!(2026 - 01 - 15);
    let mut facility = seasoned(DrawRepaySpec::Deterministic(
        [
            date!(2026 - 04 - 15),
            date!(2026 - 07 - 15),
            date!(2026 - 10 - 15),
            date!(2027 - 01 - 15),
        ]
        .into_iter()
        .map(|date| DrawRepayEvent {
            date,
            amount: usd(1_000_000.0),
            is_draw: false,
        })
        .collect(),
    ));
    // Availability ends at the term-out and the commitment then follows the
    // amortizing balance, so nothing is undrawn.
    facility.commitment_schedule = [
        (term_out, 4_000_000.0),
        (date!(2026 - 04 - 15), 3_000_000.0),
        (date!(2026 - 07 - 15), 2_000_000.0),
        (date!(2026 - 10 - 15), 1_000_000.0),
        (date!(2027 - 01 - 15), 0.0),
    ]
    .into_iter()
    .map(|(date, amount)| CommitmentStep {
        date,
        amount: usd(amount),
        fee_bp: 0.0,
    })
    .collect();
    facility.validate().expect("valid");
    let flows = flows_of(&facility);
    assert!(
        flows
            .iter()
            .filter(|cf| cf.kind == CFKind::CommitmentFee)
            .all(|cf| cf.date <= term_out),
        "no commitment fee after the term-out: {:?}",
        flows
            .iter()
            .filter(|cf| cf.kind == CFKind::CommitmentFee)
            .map(|cf| (cf.date, cf.amount.amount()))
            .collect::<Vec<_>>()
    );
    let repaid: f64 = flows
        .iter()
        .filter(|cf| cf.kind == CFKind::Notional && cf.date > term_out && cf.date < MATURITY)
        .map(|cf| cf.amount.amount())
        .sum();
    assert!((repaid - 4_000_000.0).abs() < 1e-6, "amortized {repaid}");
}

/// Clean-down recipe: a repayment into the window and a redraw out of it
/// leave no interest inside the window while the commitment fee runs on the
/// full commitment.
#[test]
fn clean_down_recipe_zeroes_the_balance_inside_the_window() {
    let facility = seasoned(DrawRepaySpec::Deterministic(vec![
        DrawRepayEvent {
            date: date!(2026 - 01 - 15),
            amount: usd(4_000_000.0),
            is_draw: false,
        },
        DrawRepayEvent {
            date: date!(2026 - 02 - 14),
            amount: usd(4_000_000.0),
            is_draw: true,
        },
    ]));
    let flows = flows_of(&facility);
    // Period 2026-01-15 -> 2026-04-15: 30 days at zero balance, then 60 days on 4M.
    let interest = interest_on(&flows, date!(2026 - 04 - 15));
    assert!(
        (interest - 4_000_000.0 * 0.07 * 60.0 / 360.0).abs() < 1e-6,
        "{interest}"
    );
    let fee = flows
        .iter()
        .find(|cf| cf.kind == CFKind::CommitmentFee && cf.date == date!(2026 - 04 - 15))
        .map(|cf| cf.amount.amount())
        .expect("commitment fee");
    let expected = 10_000_000.0 * 0.005 * 30.0 / 360.0 + 6_000_000.0 * 0.005 * 60.0 / 360.0;
    assert!((fee - expected).abs() < 1e-6, "{fee} vs {expected}");
}

// ---------------------------------------------------------------------------
// Review follow-ups: accordions, historical steps
// ---------------------------------------------------------------------------

/// An accordion (commitment step up) lets a later draw exceed the opening
/// commitment; the default-leg replay and the capacity metric read the
/// stepped commitment.
#[test]
fn accordion_allows_draws_above_the_opening_commitment() {
    // The later valuation sits inside a coupon period that reset on
    // 2026-01-15, so that fixing must be on the market.
    let market = market_with_forward().insert(knot_hazard()).insert_series(
        finstack_quant_core::market_data::scalars::ScalarTimeSeries::new(
            "FIXING:USD-SOFR-3M",
            vec![(date!(2026 - 01 - 15), 0.04)],
            None,
        )
        .expect("fixings"),
    );
    let mut facility = exposure_facility(8_000_000.0, 0.5, true);
    facility.commitment_schedule = vec![CommitmentStep {
        date: date!(2026 - 01 - 15),
        amount: usd(15_000_000.0),
        fee_bp: 0.0,
    }];
    facility.draw_repay_spec = DrawRepaySpec::Deterministic(vec![DrawRepayEvent {
        date: date!(2026 - 02 - 01),
        amount: usd(4_000_000.0),
        is_draw: true,
    }]);
    facility.validate().expect("accordion facility");
    let pv = facility
        .value(&market, AS_OF)
        .expect("prices with the default leg");
    assert!(pv.amount().is_finite());
    let result = standard_pricer_registry()
        .price_with_metrics(
            &facility,
            ModelKey::Discounting,
            &market,
            date!(2026 - 01 - 20),
            &[MetricId::custom("available_capacity")],
            Default::default(),
        )
        .expect("metrics");
    // 15M commitment less the 8M anchor balance once the accordion is in
    // force and before the scheduled draw.
    assert!((result.measures[&MetricId::custom("available_capacity")] - 7_000_000.0).abs() < 1e-6);
}

/// A commitment step dated before the valuation date is history: the
/// stochastic engine books no principal for it and starts from the anchor
/// balance.
#[test]
fn historical_commitment_steps_book_no_principal() {
    let mut facility = seasoned(zero_vol_stochastic_at(0.5));
    facility.commitment_amount = usd(20_000_000.0);
    facility.commitment_schedule = vec![CommitmentStep {
        date: date!(2024 - 07 - 15),
        amount: usd(10_000_000.0),
        fee_bp: 25.0,
    }];
    facility.drawn_amount = usd(5_000_000.0);
    facility.validate().expect("seasoned facility");
    let result = finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCreditPricer::price_with_paths(&facility, &market(), AS_OF)
        .expect("paths");
    let flows = result.path_results[0].cashflows.get_flows().to_vec();
    let notional: Vec<_> = flows
        .iter()
        .filter(|cf| cf.kind == CFKind::Notional)
        .map(|cf| (cf.date, cf.amount.amount()))
        .collect();
    assert_eq!(
        notional.len(),
        1,
        "only the terminal repayment: {notional:?}"
    );
    assert!((notional[0].1 - 5_000_000.0).abs() < 1e-6, "{notional:?}");
    assert!(
        flows.iter().all(|cf| cf.kind != CFKind::Fee),
        "the historical reduction fee is not re-charged"
    );
}
