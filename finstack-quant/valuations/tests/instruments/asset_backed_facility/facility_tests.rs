//! A warehouse line against five first-lien loans: the borrowing base
//! excludes ineligible and over-concentrated collateral, a deficiency repays
//! the lender before the residual sees a cent, the unused fee accrues on the
//! undrawn commitment, the term-out amortizes the facility sequentially,
//! and the instrument prices and reports its metrics through the registry.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::{
    AdvanceRate, AmortizationEvent, AssetBackedFacility, BorrowingBaseRules, ConcentrationLimit,
    ConcentrationScope, EligibilityRule, TermOutSpec,
};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, DealType, PoolAsset,
};
use finstack_quant_valuations::instruments::{Instrument, InstrumentEnvelope, InstrumentJson};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::{standard_pricer_registry, ModelKey};
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

fn market() -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(close())
        .knots((0..=12).map(|i| (f64::from(i), (-0.05 * f64::from(i)).exp())))
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

/// Five bullet first-lien loans at 8%: 30/20/20/15/15M, each its own obligor.
fn collateral() -> AssetPool {
    let mut pool = AssetPool::new("WH-POOL", DealType::Clo, Currency::USD);
    for (i, size) in [30.0, 20.0, 20.0, 15.0, 15.0].into_iter().enumerate() {
        let mut loan = PoolAsset::floating_rate_loan(
            format!("L{i}"),
            usd(size * 1_000_000.0),
            "USD-SOFR-3M",
            0.0,
            d(2032, 1, 1),
            DayCount::Act360,
        );
        loan.index_id = None;
        loan.spread_bp = None;
        loan.rate = 0.08;
        loan.obligor_id = Some(format!("OBL-{i}"));
        loan.industry = Some(if i < 3 { "software" } else { "services" }.to_string());
        pool.assets.push(loan);
    }
    pool
}

fn rules() -> BorrowingBaseRules {
    BorrowingBaseRules {
        advance_rates: vec![AdvanceRate {
            asset_class: "first_lien_loan".to_string(),
            rate: 0.8,
            eligibility: EligibilityRule::default(),
        }],
        concentration_limits: vec![ConcentrationLimit {
            scope: ConcentrationScope::Obligor,
            max_pct: 20.0,
        }],
    }
}

/// Clean facility: 6% fixed, 50 bp unused fee, quarterly, two-year revolving
/// period and a 24-month term-out; no prepayments or defaults.
fn facility(drawn: f64, commitment: f64) -> AssetBackedFacility {
    let mut facility = AssetBackedFacility::builder()
        .id(InstrumentId::new("WH-1".to_string()))
        .collateral(collateral())
        .borrowing_base_rules(rules())
        .commitment(usd(commitment))
        .drawn(usd(drawn))
        .margin_bp(600.0)
        .unused_fee_bp(50.0)
        .closing_date(close())
        .revolving_end(d(2026, 1, 1))
        .maturity(d(2032, 1, 1))
        .frequency(Tenor::quarterly())
        .payment_calendar_id("nyse".to_string())
        .term_out(TermOutSpec { months: 24 })
        .discount_curve_id(CurveId::new("USD-OIS".to_string()))
        .build()
        .expect("facility");
    facility.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    facility.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    facility.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.5, 6);
    facility
}

#[test]
fn borrowing_base_excludes_concentration_excess_and_ineligible_collateral() {
    let facility = facility(60_000_000.0, 100_000_000.0);
    let report = facility.borrowing_base().expect("report");
    // 100M eligible; OBL-0 holds 30M against a 20M cap -> 10M excess; 80% of 90M.
    assert!((report.eligible_collateral.amount() - 100_000_000.0).abs() < 1e-6);
    assert!((report.concentration_excess.amount() - 10_000_000.0).abs() < 1e-6);
    assert!((report.borrowing_base.amount() - 72_000_000.0).abs() < 1e-6);

    // A defaulted loan drops out of the eligible balance.
    let mut defaulted = facility.clone();
    defaulted.collateral.assets[4].is_defaulted = true;
    let report = defaulted.borrowing_base().expect("report");
    assert!((report.eligible_collateral.amount() - 85_000_000.0).abs() < 1e-6);
    // Cap is now 17M: OBL-0 (30M) and the two 20M obligors are trimmed to 17M
    // each -> 19M excess; base = 0.8 × 66M.
    assert!(
        (report.concentration_excess.amount() - 19_000_000.0).abs() < 1e-6,
        "excess {} base {}",
        report.concentration_excess.amount(),
        report.borrowing_base.amount()
    );
    assert!((report.borrowing_base.amount() - 52_800_000.0).abs() < 1e-6);

    // Collateral without a matching class is ineligible unless a "*" rate exists,
    // and a maturity limit knocks out long-dated collateral.
    let mut narrow = facility.clone();
    narrow.borrowing_base_rules.advance_rates[0].asset_class = "high_yield_bond".to_string();
    assert_eq!(
        narrow
            .borrowing_base()
            .expect("report")
            .borrowing_base
            .amount(),
        0.0
    );
    narrow.borrowing_base_rules.advance_rates.push(AdvanceRate {
        asset_class: "*".to_string(),
        rate: 0.5,
        eligibility: EligibilityRule {
            exclude_defaulted: true,
            max_maturity: Some(d(2030, 1, 1)),
        },
    });
    assert_eq!(
        narrow
            .borrowing_base()
            .expect("report")
            .borrowing_base
            .amount(),
        0.0
    );
    narrow.borrowing_base_rules.advance_rates[1]
        .eligibility
        .max_maturity = None;
    assert!(
        (narrow
            .borrowing_base()
            .expect("report")
            .borrowing_base
            .amount()
            - 45_000_000.0)
            .abs()
            < 1e-6
    );

    // Live balances drive the base as the pool amortizes.
    let live: Vec<f64> = vec![15e6, 10e6, 10e6, 7.5e6, 7.5e6];
    let report = facility
        .borrowing_base_rules
        .evaluate(&facility.collateral, Some(&live))
        .expect("report");
    assert!((report.borrowing_base.amount() - 36_000_000.0).abs() < 1e-6);
}

/// Drawn above the borrowing base: every dollar of interest left after the
/// facility coupon repays the facility, and the residual receives nothing
/// until the deficiency is cured.
#[test]
fn borrowing_base_deficiency_forces_repayment_before_the_residual() {
    let deficient = facility(90_000_000.0, 100_000_000.0);
    assert!(
        deficient
            .borrowing_base()
            .expect("report")
            .borrowing_base
            .amount()
            < 90_000_000.0
    );
    let projection = deficient.project(&market(), close()).expect("projection");
    let first_date = projection.facility.cashflows[0].0;
    let first_principal = projection
        .facility
        .principal_flows
        .iter()
        .find(|(date, _)| *date == first_date)
        .map_or(0.0, |(_, amount)| amount.amount());
    let first_interest = projection
        .facility
        .interest_flows
        .iter()
        .find(|(date, _)| *date == first_date)
        .map_or(0.0, |(_, amount)| amount.amount());
    let collections = projection.diagnostics.periods[0]
        .interest_collections
        .amount();
    assert!(first_principal > 0.0, "deficiency repays the facility");
    assert!(
        (first_principal - (collections - first_interest)).abs() < 1e-6,
        "all interest after the coupon repays: {first_principal} vs {}",
        collections - first_interest
    );
    let residual_first = projection
        .residual
        .cashflows
        .iter()
        .find(|(date, _)| *date == first_date)
        .map_or(0.0, |(_, amount)| amount.amount());
    let test = &projection.diagnostics.periods[0].coverage_tests[0];
    assert!(
        residual_first.abs() < 1e-6,
        "the residual waits for the cure (test {test:?}, note principal {first_principal}, residual {residual_first})"
    );
    assert_eq!(test.test_id, "BB_FACILITY");
    assert!(!test.passing);
    assert!((test.ratio - 72.0 / 90.0).abs() < 1e-9);

    // A facility inside its borrowing base pays the residual from day one and
    // repays nothing while revolving.
    let covered = facility(60_000_000.0, 100_000_000.0);
    let projection = covered.project(&market(), close()).expect("projection");
    assert!(projection.diagnostics.periods[0].coverage_tests[0].passing);
    let first_date = projection.facility.cashflows[0].0;
    assert!(projection
        .facility
        .principal_flows
        .iter()
        .all(|(date, amount)| *date > d(2026, 1, 1) || amount.amount().abs() < 1e-6));
    assert!(projection
        .residual
        .cashflows
        .iter()
        .any(|(date, amount)| *date == first_date && amount.amount() > 0.0));
}

#[test]
fn unused_fee_accrues_on_the_undrawn_commitment() {
    let facility = facility(60_000_000.0, 100_000_000.0);
    let projection = facility.project(&market(), close()).expect("projection");
    let period = &projection.facility.accrual_periods[0];
    let accrual = DayCount::Act360
        .year_fraction(period.start, period.end, DayCountContext::default())
        .expect("accrual");
    let expected = 40_000_000.0 * 0.005 * accrual;
    let (date, fee) = projection.unused_fees[0];
    assert_eq!(date, period.payment_date);
    assert!(
        (fee.amount() - expected).abs() < 1e-6,
        "{} vs {expected}",
        fee.amount()
    );
    assert_eq!(
        projection.unused_fees.len(),
        projection.facility.accrual_periods.len()
    );

    let lender: Vec<_> = projection.lender_cashflows();
    let interest_plus_fee = projection.facility.interest_flows[0].1.amount() + fee.amount();
    assert!((lender[0].1.amount() - interest_plus_fee).abs() < 1e-6);

    let no_fee = {
        let mut f = facility;
        f.unused_fee_bp = 0.0;
        f
    };
    assert!(no_fee
        .project(&market(), close())
        .expect("projection")
        .unused_fees
        .is_empty());
}

/// After revolving ends (here accelerated by a dated amortization event),
/// collateral principal repays the facility first; the residual only sees
/// principal once the facility is retired inside the term-out window.
#[test]
fn term_out_amortizes_the_facility_sequentially() {
    let mut facility = facility(60_000_000.0, 100_000_000.0);
    facility.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.60);
    facility.amortization_events = vec![AmortizationEvent::Date {
        date: d(2025, 1, 1),
    }];
    assert_eq!(facility.effective_revolving_end(), d(2025, 1, 1));
    assert_eq!(facility.repayment_date(), d(2027, 1, 1));
    let deal = facility.synthesized_deal().expect("deal");
    assert_eq!(deal.maturity, d(2032, 1, 1));
    assert_eq!(
        deal.call_assumption.as_ref().map(|call| call.date),
        Some(d(2027, 1, 1))
    );
    assert_eq!(
        deal.pool.reinvestment_period.as_ref().map(|p| p.end_date),
        Some(d(2025, 1, 1))
    );

    let projection = facility.project(&market(), close()).expect("projection");
    // Revolving: no facility principal before the event date.
    let early: Vec<_> = projection
        .facility
        .principal_flows
        .iter()
        .filter(|(date, amount)| *date <= d(2025, 1, 1) && amount.amount().abs() >= 1e-6)
        .map(|(date, amount)| (*date, amount.amount()))
        .collect();
    let early_tests: Vec<_> = projection
        .diagnostics
        .periods
        .iter()
        .filter(|p| p.payment_date <= d(2025, 1, 1))
        .map(|p| {
            (
                p.payment_date,
                p.pool_balance.amount(),
                p.funding_account.amount(),
                p.coverage_tests.clone(),
            )
        })
        .collect();
    assert!(
        early.is_empty(),
        "early principal {early:?}; periods {early_tests:?}"
    );
    let total_principal: f64 = projection
        .facility
        .principal_flows
        .iter()
        .map(|(_, amount)| amount.amount())
        .sum();
    assert!(
        (total_principal - 60_000_000.0).abs() < 1e-6,
        "{total_principal}"
    );
    assert_eq!(projection.facility.final_balance.amount(), 0.0);
    let repaid_on = projection
        .facility
        .principal_flows
        .iter()
        .filter(|(_, amount)| amount.amount() > 0.0)
        .map(|(date, _)| *date)
        .max()
        .expect("repayment dates");
    assert!(repaid_on <= d(2027, 1, 1));
    // Sequential: the residual receives principal only once the facility is gone.
    let residual_principal_start = projection
        .residual
        .principal_flows
        .iter()
        .filter(|(_, amount)| amount.amount() > 0.0)
        .map(|(date, _)| *date)
        .min();
    assert!(residual_principal_start.is_some_and(|date| date >= repaid_on));
}

#[test]
fn facility_prices_and_reports_metrics_through_the_registry() {
    let facility = facility(60_000_000.0, 100_000_000.0);
    let market = market();
    let value = facility.base_value(&market, close()).expect("value");
    assert!(
        value.amount() > 55_000_000.0 && value.amount() < 70_000_000.0,
        "{}",
        value.amount()
    );

    let registry = standard_pricer_registry();
    let result = registry
        .price_with_metrics(
            &facility,
            ModelKey::Discounting,
            &market,
            close(),
            &[
                MetricId::AbfBorrowingBase,
                MetricId::AbfBorrowingBaseCushion,
                MetricId::AbfAdvanceRateUtilization,
                MetricId::AbfFacilityIrr,
                MetricId::AbfResidualIrr,
                MetricId::Dv01,
            ],
            Default::default(),
        )
        .expect("priced");
    assert_eq!(result.value, value);
    let metric = |id: MetricId| result.metric(id).expect("metric");
    assert!((metric(MetricId::AbfBorrowingBase) - 72_000_000.0).abs() < 1e-6);
    assert!((metric(MetricId::AbfBorrowingBaseCushion) - 12.0 / 72.0 * 100.0).abs() < 1e-9);
    assert!((metric(MetricId::AbfAdvanceRateUtilization) - 60.0 / 72.0).abs() < 1e-12);
    let irr = metric(MetricId::AbfFacilityIrr);
    assert!(
        irr > 0.055 && irr < 0.075,
        "a 6% par facility earns ~6%: {irr}"
    );
    assert!(metric(MetricId::AbfResidualIrr).is_finite());
    assert!(metric(MetricId::Dv01) < 0.0);

    // JSON round trip through the canonical envelope.
    let json = serde_json::to_string(&InstrumentEnvelope::new(
        InstrumentJson::AssetBackedFacility(Box::new(facility.clone())),
    ))
    .expect("serialize");
    let envelope: InstrumentEnvelope = serde_json::from_str(&json).expect("deserialize");
    match envelope.instrument {
        InstrumentJson::AssetBackedFacility(parsed) => {
            assert_eq!(parsed.drawn, facility.drawn);
            assert_eq!(parsed.borrowing_base_rules, facility.borrowing_base_rules);
        }
        other => panic!("unexpected instrument {}", other.type_tag()),
    }
    let example = AssetBackedFacility::example().expect("example facility builds");
    example.validate_invariants().expect("example validates");
    assert_eq!(example.closing_date.add_months(24), example.revolving_end);
}
