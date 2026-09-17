//! CMBS collateral terms: a balloon that partly extends instead of paying,
//! appraisal reductions (ASER) whose interest shortfall lands on the junior
//! classes, prepayment penalties that reach the trust as interest, the
//! special servicing fee accruing on specially serviced balances only, and
//! the pool DSCR built from loan-level NOI.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation_with_diagnostics, AssetPool, BalloonSpec, CmbsDscrCalculator, DealFees,
    DealType, PeriodDiagnostics, PoolAsset, PrepaymentPenalty, SpecialServicingSpec,
    StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
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

/// A 10M interest-only commercial mortgage at 8%, 30/360.
fn loan(id: &str, maturity: Date) -> PoolAsset {
    PoolAsset::fixed_rate_bond(id, usd(10_000_000.0), 0.08, maturity, DayCount::Thirty360)
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
        incentive_fee: None,
    }
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
        default_prob: 0.3,
        extension_months: 24,
        extension_rate: Some(0.07),
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
        reinvestment_rate: 0.05,
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
        default_prob: 1.5,
        extension_months: 24,
        extension_rate: None,
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
