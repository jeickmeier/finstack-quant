//! CMBS collateral terms: a balloon that partly extends instead of paying,
//! appraisal reductions (ASER) whose interest shortfall lands on the junior
//! classes, prepayment penalties that reach the trust as interest, the
//! special servicing fee accruing on specially serviced balances only, and
//! the pool DSCR built from loan-level NOI.

use super::instrument_pool_tests::market_with_curves;
use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation_with_diagnostics, AssetPool, AssetType, BalloonSpec, CmbsDscrCalculator,
    DealFees, DealType, PenaltyStep, PeriodDiagnostics, PoolAsset, PrepaymentPenalty,
    SpecialServicingSpec, StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority,
    TrancheStructure,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::{MetricCalculator, MetricContext};
use std::sync::Arc;
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("month"), day).expect("date")
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn close() -> Date {
    d(2024, 1, 1)
}

fn maturity() -> Date {
    d(2032, 1, 1)
}

fn market() -> MarketContext {
    let knots: Vec<(f64, f64)> = (0..=12)
        .map(|i| (f64::from(i), (-0.05 * f64::from(i)).exp()))
        .collect();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(close())
        .knots(knots)
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

/// A 10M interest-only commercial mortgage at 8%, 30/360: a
/// `CommercialMortgage` row whose interest-only window runs to maturity.
fn loan(id: &str, maturity: Date) -> PoolAsset {
    let mut loan =
        PoolAsset::fixed_rate_bond(id, usd(10_000_000.0), 0.08, maturity, DayCount::Thirty360);
    loan.asset_type = AssetType::CommercialMortgage { ltv: None };
    loan.io_months = Some(close().months_until(maturity));
    loan
}

/// CMBS on `loans`: A 70% at 6%, B 20% at 9%, E 10%; fee-free, no
/// prepayments or defaults unless a test says otherwise.
fn cmbs(loans: Vec<PoolAsset>) -> StructuredCredit {
    let total: f64 = loans.iter().map(|l| l.balance.amount()).sum();
    let mut pool = AssetPool::new("P", DealType::Cmbs, Currency::USD);
    pool.assets = loans;
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "A",
            0.0,
            70.0,
            TrancheSeniority::Senior,
            usd(total * 0.7),
            TrancheCoupon::Fixed { rate: 0.06 },
            maturity(),
        )
        .expect("A"),
        Tranche::new(
            "B",
            70.0,
            90.0,
            TrancheSeniority::Mezzanine,
            usd(total * 0.2),
            TrancheCoupon::Fixed { rate: 0.09 },
            maturity(),
        )
        .expect("B"),
        Tranche::new(
            "E",
            90.0,
            100.0,
            TrancheSeniority::Equity,
            usd(total * 0.1),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity(),
        )
        .expect("E"),
    ])
    .expect("structure");
    let mut deal =
        StructuredCredit::new_cmbs("CMBS-T14", pool, tranches, close(), maturity(), "USD-OIS")
            .with_payment_calendar("nyse");
    deal.fees = None;
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    deal
}

fn periods(deal: &StructuredCredit) -> Vec<PeriodDiagnostics> {
    run_simulation_with_diagnostics(deal, &market(), close())
        .expect("run")
        .diagnostics
        .periods
}

fn first_interest(deal: &StructuredCredit, tranche: &str) -> f64 {
    run_simulation_with_diagnostics(deal, &market(), close())
        .expect("run")
        .tranches[tranche]
        .interest_flows[0]
        .1
        .amount()
}

fn accrual(from: Date, to: Date) -> f64 {
    DayCount::Thirty360
        .year_fraction(from, to, DayCountContext::default())
        .expect("accrual")
}

fn fees(servicing_fee_bp: f64, special_servicer_fee_bp: Option<f64>) -> DealFees {
    DealFees {
        trustee_fee_annual: usd(0.0),
        senior_mgmt_fee_bp: 0.0,
        subordinated_mgmt_fee_bp: 0.0,
        servicing_fee_bp,
        master_servicer_fee_bp: None,
        special_servicer_fee_bp,
        workout_fee_pct: None,
        liquidation_fee_pct: None,
        incentive_fee: None,
    }
}

fn special_servicing_fees(
    workout_fee_pct: Option<f64>,
    liquidation_fee_pct: Option<f64>,
) -> DealFees {
    DealFees {
        workout_fee_pct,
        liquidation_fee_pct,
        ..fees(0.0, None)
    }
}

fn serviced(mut loan: PoolAsset) -> PoolAsset {
    loan.special_servicing = Some(SpecialServicingSpec {
        appraisal_reduction_pct: 0.0,
    });
    loan
}

/// Every loan defaults 12% CDR in `month` only (no recovery lag).
fn default_in_month(deal: &mut StructuredCredit, month: usize) {
    let mut cdr = vec![0.0; month];
    cdr[month - 1] = 0.12;
    cdr.push(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::vector(cdr);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.5, 0);
}

/// A 30% / 24-month balloon on a loan maturing at the start of 2026: 70% of
/// the balance pays at the balloon, the other 30% is extended two years at
/// the extension coupon and pays then.
#[test]
fn balloon_extends_thirty_percent_of_the_balance_by_twenty_four_months() {
    let loan_maturity = d(2026, 1, 1);
    let control = periods(&cmbs(vec![loan("L1", loan_maturity)]));
    let mut extended_loan = loan("L1", loan_maturity);
    extended_loan.balloon = Some(BalloonSpec {
        extension_prob: 0.3,
        extension_months: 24,
        extension_rate: Some(0.07),
        loss_prob: 0.0,
        severity_pct: 0.0,
        workout_months: 0,
    });
    let extended = periods(&cmbs(vec![extended_loan]));

    let k = control
        .iter()
        .position(|p| p.pool_balance.amount() == 0.0)
        .expect("the control pays its balloon");
    assert!(control[k].payment_date >= loan_maturity);
    assert!((control[k].principal_collections.amount() - 10_000_000.0).abs() < 1e-6);

    // The balloon date pays 70% and leaves 30% outstanding.
    assert!((extended[k].principal_collections.amount() - 7_000_000.0).abs() < 1e-6);
    assert!((extended[k].pool_balance.amount() - 3_000_000.0).abs() < 1e-6);

    // The extended balance accrues at the extension coupon ...
    let ext_interest = extended[k + 1].interest_collections.amount();
    let expected =
        3_000_000.0 * 0.07 * accrual(extended[k].payment_date, extended[k + 1].payment_date);
    assert!(
        (ext_interest - expected).abs() < 1e-6,
        "{ext_interest} vs {expected}"
    );

    // ... and pays 24 months after the original maturity.
    let extended_maturity = d(2028, 1, 1);
    let payoff = extended
        .iter()
        .position(|p| p.pool_balance.amount() == 0.0)
        .expect("the extension pays");
    assert!(payoff > k);
    assert!(extended[payoff].payment_date >= extended_maturity);
    assert!(extended[payoff - 1].payment_date < extended_maturity);
    assert!((extended[payoff].principal_collections.amount() - 3_000_000.0).abs() < 1e-6);
    assert!(extended[k + 1..payoff]
        .iter()
        .all(|p| (p.pool_balance.amount() - 3_000_000.0).abs() < 1e-6));
}

/// An 80% appraisal reduction on one of two loans cuts interest collections
/// to 60% of the unimpaired pool; the senior class is paid in full and the
/// shortfall lands on the mezzanine and the residual.
#[test]
fn appraisal_reduction_shortfall_hits_the_junior_classes_first() {
    let control = cmbs(vec![loan("L1", maturity()), loan("L2", maturity())]);
    let mut impaired_loan = loan("L2", maturity());
    impaired_loan.special_servicing = Some(SpecialServicingSpec {
        appraisal_reduction_pct: 80.0,
    });
    let impaired = cmbs(vec![loan("L1", maturity()), impaired_loan]);

    let control_periods = periods(&control);
    let impaired_periods = periods(&impaired);
    let control_interest = control_periods[0].interest_collections.amount();
    let impaired_interest = impaired_periods[0].interest_collections.amount();
    assert!(
        (impaired_interest - 0.6 * control_interest).abs() < 1e-6,
        "{impaired_interest} vs {}",
        0.6 * control_interest
    );

    let a_control = first_interest(&control, "A");
    let a_impaired = first_interest(&impaired, "A");
    assert!((a_impaired - a_control).abs() < 1e-6, "A is whole");

    let b_control = first_interest(&control, "B");
    let b_impaired = first_interest(&impaired, "B");
    assert!(b_impaired < b_control, "B carries the shortfall");
    assert!(
        (b_impaired - (impaired_interest - a_impaired)).abs() < 1e-6,
        "B receives everything left after A: {b_impaired} vs {}",
        impaired_interest - a_impaired
    );

    let run = run_simulation_with_diagnostics(&impaired, &market(), close()).expect("run");
    let e_first = run.tranches["E"]
        .cashflows
        .iter()
        .find(|(date, _)| *date == impaired_periods[0].payment_date)
        .map_or(0.0, |(_, amount)| amount.amount());
    assert_eq!(e_first, 0.0, "the residual gets nothing");
}

/// A fixed 3% penalty on prepaid principal is collected as interest until
/// the penalty window closes.
#[test]
fn fixed_prepayment_penalty_enters_interest_collections_inside_its_window() {
    let loan_maturity = d(2029, 1, 1);
    let mut control = cmbs(vec![loan("L1", loan_maturity)]);
    control.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.20);
    let mut penalised_loan = loan("L1", loan_maturity);
    penalised_loan.prepayment_penalty = Some(PrepaymentPenalty::Fixed {
        pct: 3.0,
        through: Some(d(2024, 12, 31)),
    });
    let mut penalised = control.clone();
    penalised.pool.assets = vec![penalised_loan];

    let without = periods(&control);
    let with = periods(&penalised);
    assert_eq!(without.len(), with.len());

    let prepaid = 10_000_000.0 - without[0].pool_balance.amount();
    assert!(prepaid > 0.0);
    assert_eq!(with[0].pool_balance, without[0].pool_balance);
    let premium = with[0].interest_collections.amount() - without[0].interest_collections.amount();
    assert!(
        (premium - 0.03 * prepaid).abs() < 1e-6,
        "{premium} vs {}",
        0.03 * prepaid
    );

    let after = with
        .iter()
        .zip(&without)
        .find(|(p, _)| p.payment_date > d(2024, 12, 31))
        .expect("a period after the window");
    assert_eq!(after.0.interest_collections, after.1.interest_collections);
}

/// Yield maintenance charges the coupon lost against the reinvestment rate
/// over the remaining term of the prepaid balance.
#[test]
fn yield_maintenance_premium_is_the_lost_coupon_over_the_remaining_term() {
    let loan_maturity = d(2029, 1, 1);
    let mut control = cmbs(vec![loan("L1", loan_maturity)]);
    control.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.20);
    let mut penalised_loan = loan("L1", loan_maturity);
    penalised_loan.prepayment_penalty = Some(PrepaymentPenalty::YieldMaintenance {
        discount_curve_id: None,
        reinvestment_rate: Some(0.05),
        floor_pct: None,
        through: None,
    });
    let mut penalised = control.clone();
    penalised.pool.assets = vec![penalised_loan];

    let without = periods(&control);
    let with = periods(&penalised);
    let prepaid = 10_000_000.0 - without[0].pool_balance.amount();
    let years = (loan_maturity - with[0].payment_date).whole_days() as f64 / 365.25;
    let expected = prepaid * (0.08 - 0.05) * years;
    let premium = with[0].interest_collections.amount() - without[0].interest_collections.amount();
    assert!((premium - expected).abs() < 1e-6, "{premium} vs {expected}");
}

/// The special servicing fee accrues on the specially serviced loans only:
/// 25bp on one of two equal loans is half of a 25bp servicing fee on the
/// pool, and nothing when no loan is in special servicing.
#[test]
fn special_servicing_fee_accrues_on_the_specially_serviced_balance_only() {
    let mut serviced_loan = loan("L2", maturity());
    serviced_loan.special_servicing = Some(SpecialServicingSpec {
        appraisal_reduction_pct: 0.0,
    });
    let mut special = cmbs(vec![loan("L1", maturity()), serviced_loan]);
    special.fees = Some(fees(0.0, Some(25.0)));
    let mut pool_wide = cmbs(vec![loan("L1", maturity()), loan("L2", maturity())]);
    pool_wide.fees = Some(fees(25.0, None));
    let mut nothing_serviced = cmbs(vec![loan("L1", maturity()), loan("L2", maturity())]);
    nothing_serviced.fees = Some(fees(0.0, Some(25.0)));

    let special_fee = periods(&special)[0].fees_paid.amount();
    let pool_fee = periods(&pool_wide)[0].fees_paid.amount();
    assert!(pool_fee > 0.0);
    assert!(
        (special_fee - 0.5 * pool_fee).abs() < 1e-6,
        "{special_fee} vs half of {pool_fee}"
    );
    assert_eq!(periods(&nothing_serviced)[0].fees_paid.amount(), 0.0);
}

/// Loan-level NOI drives the pool DSCR: interest-only debt service is the
/// coupon on the balance, level-pay debt service is twelve contractual
/// payments.
#[test]
fn dscr_comes_from_loan_level_noi_and_debt_service() {
    let mut io_loan = loan("L1", maturity());
    io_loan.noi = Some(usd(1_200_000.0));
    let mut level_loan = loan("L2", maturity());
    level_loan.io_months = None;
    level_loan.contractual_payment = Some(usd(100_000.0));
    level_loan.noi = Some(usd(1_466_666.666_666_666_7));
    let deal = cmbs(vec![io_loan, level_loan]);

    let mut context = MetricContext::new(
        Arc::new(deal),
        Arc::new(MarketContext::new()),
        close(),
        usd(0.0),
        MetricContext::default_config(),
    );
    let dscr = CmbsDscrCalculator::new()
        .calculate(&mut context)
        .expect("dscr");
    // (1.2M + 1.4667M) / (0.8M + 1.2M)
    assert!((dscr - 4.0 / 3.0).abs() < 1e-9, "{dscr}");

    let mut without_noi = MetricContext::new(
        Arc::new(cmbs(vec![loan("L1", maturity())])),
        Arc::new(MarketContext::new()),
        close(),
        usd(0.0),
        MetricContext::default_config(),
    );
    let err = CmbsDscrCalculator::new()
        .calculate(&mut without_noi)
        .expect_err("no NOI anywhere");
    assert!(err.to_string().contains("annual_noi"));
}

/// Malformed commercial-mortgage terms are rejected by instrument validation.
#[test]
fn cmbs_terms_are_validated() {
    let mut bad_balloon = loan("L1", maturity());
    bad_balloon.balloon = Some(BalloonSpec {
        extension_prob: 1.5,
        extension_months: 24,
        extension_rate: None,
        loss_prob: 0.0,
        severity_pct: 0.0,
        workout_months: 0,
    });
    assert!(cmbs(vec![bad_balloon]).validate_invariants().is_err());

    let mut bad_penalty = loan("L1", maturity());
    bad_penalty.prepayment_penalty = Some(PrepaymentPenalty::Fixed {
        pct: -1.0,
        through: None,
    });
    assert!(cmbs(vec![bad_penalty]).validate_invariants().is_err());

    let mut bad_appraisal = loan("L1", maturity());
    bad_appraisal.special_servicing = Some(SpecialServicingSpec {
        appraisal_reduction_pct: 120.0,
    });
    assert!(cmbs(vec![bad_appraisal]).validate_invariants().is_err());

    let mut foreign_noi = loan("L1", maturity());
    foreign_noi.noi = Some(Money::new(1.0, Currency::EUR).expect("money"));
    assert!(cmbs(vec![foreign_noi]).validate_invariants().is_err());

    assert!(cmbs(vec![loan("L1", maturity())])
        .validate_invariants()
        .is_ok());
}

fn balloon(extension_prob: f64, extension_rate: Option<f64>) -> BalloonSpec {
    BalloonSpec {
        extension_prob,
        extension_months: 24,
        extension_rate,
        loss_prob: 0.0,
        severity_pct: 0.0,
        workout_months: 0,
    }
}

/// A floating SOFR + 300 loan extended at a fixed 9%: the extended balance
/// pays 9%, not the index plus spread.
#[test]
fn a_fixed_extension_rate_replaces_the_index_and_spread() {
    let loan_maturity = d(2026, 1, 1);
    let mut floating = PoolAsset::floating_rate_loan(
        "F1",
        usd(10_000_000.0),
        "USD-SOFR-3M",
        300.0,
        loan_maturity,
        DayCount::Act360,
    );
    floating.asset_type = AssetType::CommercialMortgage { ltv: None };
    floating.io_months = Some(close().months_until(loan_maturity));
    floating.balloon = Some(balloon(0.3, Some(0.09)));
    let deal = cmbs(vec![floating]);
    let run =
        run_simulation_with_diagnostics(&deal, &market_with_curves(close()), close()).expect("run");
    let periods = &run.diagnostics.periods;
    let k = periods
        .iter()
        .position(|p| (p.pool_balance.amount() - 3_000_000.0).abs() < 1e-6)
        .expect("the extension leaves 3M outstanding");
    let accrual = DayCount::Act360
        .year_fraction(
            periods[k].payment_date,
            periods[k + 1].payment_date,
            DayCountContext::default(),
        )
        .expect("accrual");
    let interest = periods[k + 1].interest_collections.amount();
    assert!(
        (interest - 3_000_000.0 * 0.09 * accrual).abs() < 1.0,
        "the extended balance pays the 9% modification rate: {interest} vs {}",
        3_000_000.0 * 0.09 * accrual
    );
    // Before the extension the loan paid SOFR (4%) + 300 = 7%.
    let before = periods[k - 1].interest_collections.amount();
    let before_accrual = DayCount::Act360
        .year_fraction(
            periods[k - 2].payment_date,
            periods[k - 1].payment_date,
            DayCountContext::default(),
        )
        .expect("accrual");
    let floating_coupon = before / (10_000_000.0 * before_accrual);
    assert!(
        (floating_coupon - 0.07).abs() < 0.002,
        "before the extension the loan pays about SOFR + 300 = 7%: {floating_coupon}"
    );
}

/// A level-pay loan whose 30% extended slice keeps its schedule: the first
/// post-extension scheduled principal is 30% of the last pre-extension one.
#[test]
fn an_extended_level_pay_slice_keeps_its_schedule_scaled_by_the_extended_fraction() {
    let loan_maturity = d(2027, 1, 1);
    let mut amortizing = PoolAsset::fixed_rate_bond(
        "A1",
        usd(10_000_000.0),
        0.08,
        loan_maturity,
        DayCount::Thirty360,
    );
    amortizing.asset_type = AssetType::CommercialMortgage { ltv: None };
    amortizing.amortization_term_months = Some(360);
    amortizing.balloon = Some(balloon(0.3, None));
    let periods = periods(&cmbs(vec![amortizing]));
    let k = periods
        .iter()
        .position(|p| p.payment_date >= loan_maturity)
        .expect("maturity period");
    // The schedule's principal grows by (1 + r) a month; the maturity period
    // pays the balloon instead, so the first extended period continues the
    // schedule one step on from the last regular one, scaled to the slice.
    let before = periods[k - 1].principal_collections.amount();
    let after = periods[k + 1].principal_collections.amount();
    let expected = 0.3 * before * (1.0 + 0.08 / 12.0);
    assert!(
        (after - expected).abs() < 1.0,
        "the extended slice amortizes at 30% of the schedule: {after} vs {expected}"
    );
}

/// 35% of the balloon defaults at 40% severity: a 14% loss of the balloon
/// balance, with the 60% recovery released after the 18-month workout.
#[test]
fn a_balloon_default_books_the_loss_and_recovers_after_the_workout() {
    let loan_maturity = d(2026, 1, 1);
    let mut defaulting = loan("L1", loan_maturity);
    defaulting.balloon = Some(BalloonSpec {
        extension_prob: 0.0,
        extension_months: 24,
        extension_rate: None,
        loss_prob: 0.35,
        severity_pct: 40.0,
        workout_months: 18,
    });
    // A second loan keeps the deal alive through the workout.
    let periods = periods(&cmbs(vec![defaulting, loan("L2", maturity())]));
    let k = periods
        .iter()
        .position(|p| p.payment_date >= loan_maturity)
        .expect("maturity period");
    assert!(
        (periods[k].defaults.amount() - 3_500_000.0).abs() < 1e-6,
        "35% of the balloon defaults: {}",
        periods[k].defaults.amount()
    );
    assert!(
        (periods[k].principal_collections.amount() - 6_500_000.0).abs() < 1e-6,
        "the rest pays as the balloon: {}",
        periods[k].principal_collections.amount()
    );
    let release = periods
        .iter()
        .position(|p| p.recoveries.amount() > 0.0)
        .expect("the workout recovery is released");
    // The claim runs from the maturity payment date (calendar-adjusted) and
    // lands on the first payment date at or after 18 months later.
    let due = periods[k].payment_date.add_months(18);
    assert!(
        periods[release].payment_date >= due && periods[release].payment_date <= due.add_months(1),
        "released 18 months after maturity ({due}): {}",
        periods[release].payment_date
    );
    assert!(
        (periods[release].recoveries.amount() - 2_100_000.0).abs() < 1e-6,
        "60% recovery on the 3.5M default: {}",
        periods[release].recoveries.amount()
    );
    let total_recoveries: f64 = periods.iter().map(|p| p.recoveries.amount()).sum();
    assert!(
        (total_recoveries - 2_100_000.0).abs() < 1e-6,
        "loss of 1.4M = 14% of the balloon"
    );
}

/// DSCR on live terms: a floating SOFR + 300 loan at a flat 4.3% curve pays
/// about 7.3% (not its 3% spread alone), and a level-pay loan's debt service
/// is twelve level payments over its remaining schedule, resolved as of the
/// valuation date rather than from the closing coupon.
#[test]
fn dscr_uses_the_live_floating_coupon_and_the_amortization_schedule() {
    let mut floating = PoolAsset::floating_rate_loan(
        "F1",
        usd(100_000_000.0),
        "USD-SOFR-3M",
        300.0,
        maturity(),
        DayCount::Act360,
    );
    floating.asset_type = AssetType::CommercialMortgage { ltv: None };
    floating.io_months = Some(close().months_until(maturity()));
    floating.noi = Some(usd(8_000_000.0));
    let sofr = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(close())
        .day_count(DayCount::Act360)
        .reset_lag(2)
        .knots([(0.0, 0.043), (10.0, 0.043)])
        .build()
        .expect("forward curve");
    let market = MarketContext::new().insert(sofr);
    let mut context = MetricContext::new(
        Arc::new(cmbs(vec![floating])),
        Arc::new(market.clone()),
        close(),
        usd(0.0),
        MetricContext::default_config(),
    );
    let dscr = CmbsDscrCalculator::new()
        .calculate(&mut context)
        .expect("dscr");
    assert!(
        (dscr - 8.0 / 7.3).abs() < 0.01,
        "8M NOI over 7.3% on 100M: {dscr}, not the spread-only 2.667"
    );

    // A level-pay 8% loan on a 30-year schedule: twelve level payments.
    let mut amortizing = PoolAsset::fixed_rate_bond(
        "A1",
        usd(10_000_000.0),
        0.08,
        maturity(),
        DayCount::Thirty360,
    );
    amortizing.asset_type = AssetType::CommercialMortgage { ltv: None };
    amortizing.amortization_term_months = Some(360);
    amortizing.noi = Some(usd(1_200_000.0));
    let r: f64 = 0.08 / 12.0;
    let level_payment = 10_000_000.0 * r / (1.0 - (1.0 + r).powi(-360));
    let mut context = MetricContext::new(
        Arc::new(cmbs(vec![amortizing])),
        Arc::new(market),
        close(),
        usd(0.0),
        MetricContext::default_config(),
    );
    let dscr = CmbsDscrCalculator::new()
        .calculate(&mut context)
        .expect("dscr");
    assert!(
        (dscr - 1_200_000.0 / (12.0 * level_payment)).abs() < 1e-6,
        "NOI over twelve level payments: {dscr}"
    );
}

/// A loan that defaults enters special servicing from the next period: the
/// special servicer fee is zero before the month-6 default and, from month
/// 7, equals the fee on a pool whose loans were specially serviced from
/// closing.
#[test]
fn a_loan_entering_default_is_specially_serviced_from_the_next_period() {
    let mut dynamic = cmbs(vec![loan("L1", maturity()), loan("L2", maturity())]);
    dynamic.fees = Some(fees(0.0, Some(25.0)));
    default_in_month(&mut dynamic, 6);
    let mut from_closing = cmbs(vec![
        serviced(loan("L1", maturity())),
        serviced(loan("L2", maturity())),
    ]);
    from_closing.fees = Some(fees(0.0, Some(25.0)));
    default_in_month(&mut from_closing, 6);

    let dynamic = periods(&dynamic);
    let from_closing = periods(&from_closing);
    for period in &dynamic[..6] {
        assert_eq!(
            period.fees_paid.amount(),
            0.0,
            "no fee before the default: {}",
            period.payment_date
        );
    }
    assert!(from_closing[0].fees_paid.amount() > 0.0);
    for (late, reference) in dynamic[6..12].iter().zip(&from_closing[6..12]) {
        assert!(
            (late.fees_paid.amount() - reference.fees_paid.amount()).abs() < 1e-6,
            "{}: {} vs {}",
            late.payment_date,
            late.fees_paid.amount(),
            reference.fees_paid.amount()
        );
        assert!(late.fees_paid.amount() > 0.0);
    }
}

/// The workout fee is 1% of the P&I collected on a specially serviced loan,
/// taken off the top of its collections; the liquidation fee is 2% of the
/// proceeds of a defaulted loan, taken before the recovery reaches the
/// waterfall. Both are reported as fees paid.
#[test]
fn workout_and_liquidation_fees_come_off_collections_and_recoveries() {
    let free = cmbs(vec![
        loan("L1", maturity()),
        serviced(loan("L2", maturity())),
    ]);
    let mut workout = cmbs(vec![
        loan("L1", maturity()),
        serviced(loan("L2", maturity())),
    ]);
    workout.fees = Some(special_servicing_fees(Some(1.0), None));
    let free_periods = periods(&free);
    let workout_periods = periods(&workout);
    let l2_interest = 10_000_000.0 * 0.08 * accrual(close(), free_periods[0].payment_date);
    let fee = workout_periods[0].fees_paid.amount();
    assert!(
        (fee - 0.01 * l2_interest).abs() < 1e-6,
        "1% of L2's interest: {fee} vs {}",
        0.01 * l2_interest
    );
    assert!(
        (free_periods[0].interest_collections.amount()
            - workout_periods[0].interest_collections.amount()
            - fee)
            .abs()
            < 1e-6,
        "the fee comes out of the interest collections"
    );

    let mut free = cmbs(vec![loan("L1", maturity()), loan("L2", maturity())]);
    default_in_month(&mut free, 1);
    let mut liquidation = cmbs(vec![loan("L1", maturity()), loan("L2", maturity())]);
    default_in_month(&mut liquidation, 1);
    liquidation.fees = Some(special_servicing_fees(None, Some(2.0)));
    let free_recoveries: f64 = periods(&free)[..3]
        .iter()
        .map(|p| p.recoveries.amount())
        .sum();
    let liquidation_periods = periods(&liquidation);
    let net_recoveries: f64 = liquidation_periods[..3]
        .iter()
        .map(|p| p.recoveries.amount())
        .sum();
    let fees: f64 = liquidation_periods[..3]
        .iter()
        .map(|p| p.fees_paid.amount())
        .sum();
    assert!(free_recoveries > 0.0);
    assert!(
        (net_recoveries - 0.98 * free_recoveries).abs() < 1e-6,
        "2% of the proceeds: {net_recoveries} vs {}",
        0.98 * free_recoveries
    );
    assert!((fees - 0.02 * free_recoveries).abs() < 1e-6, "{fees}");
}

fn flat_curve(rate: f64) -> DiscountCurve {
    DiscountCurve::builder("USD-YM")
        .base_date(close())
        .knots((0..=10).map(|i| (f64::from(i), (-rate * f64::from(i)).exp())))
        .build()
        .expect("curve")
}

/// Discounting the annual lost coupons on a flat 5% curve over five
/// remaining years values the yield-maintenance premium at Σ e^{-0.05k}
/// = 4.33 years of lost coupon instead of 5: about 13.4% below the
/// undiscounted premium.
#[test]
fn yield_maintenance_discounts_the_lost_coupons_on_the_curve() {
    let date = close();
    let maturity = d(2029, 1, 1);
    let undiscounted = PrepaymentPenalty::YieldMaintenance {
        discount_curve_id: None,
        reinvestment_rate: Some(0.05),
        floor_pct: None,
        through: None,
    }
    .premium(1_000_000.0, 0.08, date, maturity, None)
    .expect("premium");
    let discounted = PrepaymentPenalty::YieldMaintenance {
        discount_curve_id: Some("USD-YM".into()),
        reinvestment_rate: Some(0.05),
        floor_pct: None,
        through: None,
    }
    .premium(1_000_000.0, 0.08, date, maturity, Some(&flat_curve(0.05)))
    .expect("premium");
    let years = (maturity - date).whole_days() as f64 / 365.25;
    assert!((undiscounted - 30_000.0 * years).abs() < 1e-6);
    let ratio = discounted / undiscounted;
    assert!(
        (ratio - 0.866).abs() < 0.005,
        "discounted premium is 13.4% below undiscounted: ratio {ratio}"
    );
    assert!(PrepaymentPenalty::YieldMaintenance {
        discount_curve_id: Some("USD-YM".into()),
        reinvestment_rate: Some(0.05),
        floor_pct: None,
        through: None,
    }
    .premium(1_000_000.0, 0.08, date, maturity, None)
    .is_err());
}

/// When rates rise above the coupon the lost coupon is zero and the 1%
/// floor is the premium; the reinvestment rate comes from the curve.
#[test]
fn yield_maintenance_floor_binds_when_rates_rise() {
    let penalty = PrepaymentPenalty::YieldMaintenance {
        discount_curve_id: Some("USD-YM".into()),
        reinvestment_rate: None,
        floor_pct: Some(1.0),
        through: None,
    };
    let premium = penalty
        .premium(
            1_000_000.0,
            0.08,
            close(),
            d(2029, 1, 1),
            Some(&flat_curve(0.09)),
        )
        .expect("premium");
    assert!(
        (premium - 10_000.0).abs() < 1e-9,
        "the 1% floor binds: {premium}"
    );
    let low_rates = penalty
        .premium(
            1_000_000.0,
            0.08,
            close(),
            d(2029, 1, 1),
            Some(&flat_curve(0.05)),
        )
        .expect("premium");
    assert!(
        low_rates > 10_000.0,
        "below the coupon the lost coupon exceeds the floor"
    );
}

/// A lockout stops the deal's prepayment speed on the locked loan until the
/// lockout ends; a step-down schedule charges the step in force.
#[test]
fn lockout_stops_the_deal_cpr_on_the_locked_loan() {
    let loan_maturity = d(2029, 1, 1);
    let mut control = cmbs(vec![loan("L1", loan_maturity)]);
    control.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.20);
    let mut locked_loan = loan("L1", loan_maturity);
    locked_loan.prepayment_penalty = Some(PrepaymentPenalty::Lockout {
        through: Some(d(2024, 12, 31)),
    });
    let mut locked = control.clone();
    locked.pool.assets = vec![locked_loan];

    let without = periods(&control);
    let with = periods(&locked);
    assert!(without[0].pool_balance.amount() < 10_000_000.0);
    for period in with
        .iter()
        .take_while(|p| p.payment_date <= d(2024, 12, 31))
    {
        assert!(
            (period.pool_balance.amount() - 10_000_000.0).abs() < 1e-6,
            "no prepayment inside the lockout: {}",
            period.payment_date
        );
    }
    let after = with
        .iter()
        .find(|p| p.payment_date > d(2024, 12, 31))
        .expect("a period after the lockout");
    assert!(
        after.pool_balance.amount() < 10_000_000.0,
        "prepayments resume"
    );

    let mut stepped_loan = loan("L1", loan_maturity);
    stepped_loan.prepayment_penalty = Some(PrepaymentPenalty::StepDown {
        schedule: vec![
            PenaltyStep {
                through: d(2024, 12, 31),
                pct: 5.0,
            },
            PenaltyStep {
                through: d(2025, 12, 31),
                pct: 3.0,
            },
        ],
    });
    let mut stepped = control;
    stepped.pool.assets = vec![stepped_loan];
    let stepped = periods(&stepped);
    let prepaid = 10_000_000.0 - without[0].pool_balance.amount();
    let premium =
        stepped[0].interest_collections.amount() - without[0].interest_collections.amount();
    assert!((premium - 0.05 * prepaid).abs() < 1e-6, "{premium}");
    let (later, later_control) = stepped
        .iter()
        .zip(&without)
        .find(|(p, _)| p.payment_date > d(2024, 12, 31))
        .expect("second step");
    let prepaid_later = later_control.pool_balance.amount();
    assert!(prepaid_later > 0.0);
    assert!(
        later.interest_collections.amount() > later_control.interest_collections.amount(),
        "the second step still charges"
    );
    let (last, last_control) = stepped
        .iter()
        .zip(&without)
        .find(|(p, _)| p.payment_date > d(2025, 12, 31))
        .expect("after the schedule");
    assert_eq!(last.interest_collections, last_control.interest_collections);
}
