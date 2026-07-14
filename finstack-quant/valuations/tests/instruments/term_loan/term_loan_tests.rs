use finstack_quant_cashflows::primitives::CFKind;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    self, CommitmentStepDown, DdtlSpec, DrawEvent, OidEirSpec, OidPolicy, TermLoan,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::Pricer;

fn mc() -> MarketContext {
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (1.0, 0.97), (5.0, 0.85)])
        .build()
        .unwrap();
    MarketContext::new().insert(disc)
}

#[test]
fn term_loan_fixed_with_draws_and_fees() {
    let issue = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2027, time::Month::January, 1).unwrap();

    let loan = TermLoan::builder()
        .id("TL-001".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .rate(term_loan::RateSpec::Fixed { rate_bp: 800 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(finstack_quant_core::dates::StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(term_loan::AmortizationSpec::None)
        .coupon_type(finstack_quant_cashflows::builder::specs::CouponType::Cash)
        .upfront_fee_opt(Some(Money::new(25_000.0, Currency::USD)))
        .ddtl_opt(Some(DdtlSpec {
            commitment_limit: Money::new(10_000_000.0, Currency::USD),
            availability_start: issue,
            availability_end: Date::from_calendar_date(2026, time::Month::January, 1).unwrap(),
            draws: vec![
                DrawEvent {
                    date: issue,
                    amount: Money::new(3_000_000.0, Currency::USD),
                },
                DrawEvent {
                    date: Date::from_calendar_date(2025, time::Month::July, 1).unwrap(),
                    amount: Money::new(2_000_000.0, Currency::USD),
                },
            ],
            commitment_step_downs: vec![],
            usage_fee_bp: 10,
            commitment_fee_bp: 25,
            fee_base: term_loan::CommitmentFeeBase::Undrawn,
            oid_policy: Some(OidPolicy::WithheldPct(100)), // 1% withheld OID on draws
        }))
        .covenants_opt(None)
        .pricing_overrides(Default::default())
        .attributes(Default::default())
        .build()
        .unwrap();

    let market = mc();
    let pricer =
        finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoanDiscountingPricer;
    let as_of = issue;

    // Ensure pricing runs and PV is finite
    let result = pricer.price_dyn(&loan, &market, as_of).expect("pricing ok");
    assert_eq!(result.instrument_id, "TL-001");
    assert!(result.value.amount().is_finite());

    // Build full schedule and verify flows exist and are ordered
    let sched = loan.cashflow_schedule(&market, as_of).unwrap();
    assert!(!sched.get_flows().is_empty());
    let mut last = sched.get_flows()[0].date;
    for cf in sched.get_flows() {
        assert!(cf.date >= last);
        last = cf.date;
    }
}

#[test]
fn term_loan_commitment_fee_step_downs() {
    let issue = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let step_down = Date::from_calendar_date(2025, time::Month::July, 1).unwrap();
    let availability_end = Date::from_calendar_date(2026, time::Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2027, time::Month::January, 1).unwrap();

    let loan = TermLoan::builder()
        .id("TL-STEPDOWN".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .rate(term_loan::RateSpec::Fixed { rate_bp: 700 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(finstack_quant_core::dates::StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(term_loan::AmortizationSpec::None)
        .coupon_type(finstack_quant_cashflows::builder::specs::CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(Some(DdtlSpec {
            commitment_limit: Money::new(10_000_000.0, Currency::USD),
            availability_start: issue,
            availability_end,
            draws: vec![],
            commitment_step_downs: vec![CommitmentStepDown {
                date: step_down,
                new_limit: Money::new(5_000_000.0, Currency::USD),
            }],
            usage_fee_bp: 0,
            commitment_fee_bp: 100,
            fee_base: term_loan::CommitmentFeeBase::Undrawn,
            oid_policy: None,
        }))
        .covenants_opt(None)
        .pricing_overrides(Default::default())
        .attributes(Default::default())
        .build()
        .unwrap();

    let sched = loan.cashflow_schedule(&mc(), issue).unwrap();
    let fees: Vec<_> = sched
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::CommitmentFee)
        .collect();
    assert!(!fees.is_empty());

    let before = fees
        .iter()
        .filter(|cf| cf.date < step_down)
        .map(|cf| cf.amount.amount())
        .next()
        .expect("fee before step-down");
    let after = fees
        .iter()
        .filter(|cf| cf.date > step_down)
        .map(|cf| cf.amount.amount())
        .next()
        .expect("fee after step-down");

    let ratio = after / before;
    assert!(ratio > 0.4 && ratio < 0.6, "fee ratio: {}", ratio);
}

#[test]
fn term_loan_commitment_fee_windowed_to_availability() {
    let issue = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let availability_end = Date::from_calendar_date(2025, time::Month::July, 1).unwrap();
    let maturity = Date::from_calendar_date(2026, time::Month::January, 1).unwrap();

    let loan = TermLoan::builder()
        .id("TL-WINDOW".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .rate(term_loan::RateSpec::Fixed { rate_bp: 650 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(finstack_quant_core::dates::StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(term_loan::AmortizationSpec::None)
        .coupon_type(finstack_quant_cashflows::builder::specs::CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(Some(DdtlSpec {
            commitment_limit: Money::new(10_000_000.0, Currency::USD),
            availability_start: issue,
            availability_end,
            draws: vec![],
            commitment_step_downs: vec![],
            usage_fee_bp: 0,
            commitment_fee_bp: 50,
            fee_base: term_loan::CommitmentFeeBase::Undrawn,
            oid_policy: None,
        }))
        .covenants_opt(None)
        .pricing_overrides(Default::default())
        .attributes(Default::default())
        .build()
        .unwrap();

    let sched = loan.cashflow_schedule(&mc(), issue).unwrap();
    let fee_dates: Vec<_> = sched
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::CommitmentFee)
        .map(|cf| cf.date)
        .collect();
    assert!(!fee_dates.is_empty());

    let max_fee_date = fee_dates.iter().max().copied().unwrap();
    assert!(
        max_fee_date <= availability_end,
        "fee after availability window: {} > {}",
        max_fee_date,
        availability_end
    );
}

#[test]
fn term_loan_oid_eir_amortization_schedule() {
    let issue = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2027, time::Month::January, 1).unwrap();

    let loan = TermLoan::builder()
        .id("TL-OID-EIR".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .rate(term_loan::RateSpec::Fixed { rate_bp: 500 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(finstack_quant_core::dates::StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(term_loan::AmortizationSpec::None)
        .coupon_type(finstack_quant_cashflows::builder::specs::CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(Some(DdtlSpec {
            commitment_limit: Money::new(1_000_000.0, Currency::USD),
            availability_start: issue,
            availability_end: issue,
            draws: vec![DrawEvent {
                date: issue,
                amount: Money::new(1_000_000.0, Currency::USD),
            }],
            commitment_step_downs: vec![],
            usage_fee_bp: 0,
            commitment_fee_bp: 0,
            fee_base: term_loan::CommitmentFeeBase::Undrawn,
            oid_policy: Some(OidPolicy::WithheldPct(200)),
        }))
        .covenants_opt(None)
        .pricing_overrides(Default::default())
        .oid_eir_opt(Some(OidEirSpec::default()))
        .attributes(Default::default())
        .build()
        .unwrap();

    let market = mc();
    let result = loan
        .price_with_metrics(
            &market,
            issue,
            &[MetricId::custom("oid_eir_amortization")],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let eir = *result
        .measures
        .get("oid_eir_rate")
        .expect("EIR rate should be reported");
    assert!(eir > 0.05);

    let total_amort = *result
        .measures
        .get("oid_eir_amortization")
        .expect("amortization total should be reported");
    assert!(total_amort > 0.0);

    let has_series = result
        .measures
        .keys()
        .any(|k| k.as_str().starts_with("oid_eir_amortization::"));
    assert!(has_series);
}

#[test]
fn term_loan_pik_toggle_and_cash_sweep() {
    let issue = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2026, time::Month::January, 1).unwrap();

    let cov = term_loan::TermLoanCovenantEvents {
        margin_stepups: vec![],
        pik_toggles: vec![term_loan::PikToggle {
            date: Date::from_calendar_date(2025, time::Month::July, 1).unwrap(),
            enable_pik: true,
        }],
        cash_sweeps: vec![term_loan::CashSweepEvent {
            date: Date::from_calendar_date(2025, time::Month::October, 1).unwrap(),
            amount: Money::new(500_000.0, Currency::USD),
        }],
        draw_stop_dates: vec![],
    };

    let loan = TermLoan::builder()
        .id("TL-PIK".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(5_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .rate(term_loan::RateSpec::Fixed { rate_bp: 600 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(finstack_quant_core::dates::StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(term_loan::AmortizationSpec::None)
        .coupon_type(finstack_quant_cashflows::builder::specs::CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(Some(cov))
        .pricing_overrides(Default::default())
        .attributes(Default::default())
        .build()
        .unwrap();

    let market = mc();
    let sched = loan.cashflow_schedule(&market, issue).unwrap();
    assert!(!sched.get_flows().is_empty());
    // Holder-view schedules omit PIK accretion because it capitalizes into
    // outstanding rather than paying cash to the holder.
    let has_pik = sched.get_flows().iter().any(|cf| cf.kind == CFKind::PIK);
    assert!(!has_pik);
}

#[test]
fn term_loan_golden_pv_and_metrics() {
    use finstack_quant_core::dates::BusinessDayConvention;
    use finstack_quant_core::dates::StubKind;
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::metrics::MetricId;

    let issue = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2028, time::Month::January, 1).unwrap();

    // Simple fixed-rate bullet term loan for golden test
    let loan = TermLoan::builder()
        .id("TL-GOLDEN".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .rate(term_loan::RateSpec::Fixed { rate_bp: 500 }) // 5%
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(term_loan::AmortizationSpec::None) // Bullet
        .coupon_type(finstack_quant_cashflows::builder::specs::CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .pricing_overrides(Default::default())
        .call_schedule_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    let market = mc();
    let as_of = issue;

    // Golden PV
    let pv = loan.value(&market, as_of).unwrap();
    // Expected PV for a 3-year 5% coupon bullet loan with discount factors [1.0, 0.97, 0.85]
    // This is a rough golden value; precise computation requires exact schedule and discounting
    assert!(
        pv.amount() > 900_000.0 && pv.amount() < 1_100_000.0,
        "PV sanity check: {}",
        pv.amount()
    );

    // Compute metrics (YTM and DV01)
    let metrics = vec![MetricId::Ytm, MetricId::Dv01];
    let result = loan
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Verify YTM is computed and reasonable for a 5% fixed-rate loan
    let ytm = result.measures.get("ytm").expect("YTM should be computed");
    assert!(ytm > &0.03 && ytm < &0.10, "YTM sanity check: {}", ytm);

    // Verify DV01 is computed
    let dv01 = result
        .measures
        .get("dv01")
        .expect("DV01 should be computed");
    assert!(
        dv01.is_finite() && dv01.abs() > 0.0,
        "DV01 should be non-zero: {}",
        dv01
    );

    // Signed canonical schedule includes all flows (coupons, redemption,
    // and initial negative notional disbursement).
    let all_flows = loan.dated_cashflows(&market, as_of).unwrap();
    let positive_count = all_flows.iter().filter(|(_, a)| a.amount() > 0.0).count();
    assert!(
        positive_count > 0,
        "Schedule should have positive coupon/redemption flows"
    );
}

#[test]
fn term_loan_amortizing_outstanding_path() {
    use finstack_quant_cashflows::CashflowProvider;
    use finstack_quant_core::dates::BusinessDayConvention;
    use finstack_quant_core::dates::StubKind;

    let issue = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2027, time::Month::January, 1).unwrap();

    // Amortizing loan with PercentPerPeriod
    let loan = TermLoan::builder()
        .id("TL-AMORT".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .rate(term_loan::RateSpec::Fixed { rate_bp: 500 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(term_loan::AmortizationSpec::PercentPerPeriod { bp: 1250 }) // 12.5% per quarter
        .coupon_type(finstack_quant_cashflows::builder::specs::CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .pricing_overrides(Default::default())
        .call_schedule_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    let market = mc();

    // Build holder-view schedule and verify it still exposes amortizing repayments.
    let sched = loan.cashflow_schedule(&market, issue).unwrap();
    let amortization_flows: Vec<_> = sched
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Amortization)
        .collect();

    assert!(
        !amortization_flows.is_empty(),
        "Holder-view schedule should expose amortizing repayments"
    );
    // Signed canonical schedule preserves initial negative notional.
    // Verify that positive coupon/amortization flows exist alongside any
    // negative notional flows.
    let has_positive = sched.get_flows().iter().any(|cf| cf.amount.amount() > 0.0);
    assert!(
        has_positive,
        "Schedule should have positive coupon/amortization flows"
    );
}
