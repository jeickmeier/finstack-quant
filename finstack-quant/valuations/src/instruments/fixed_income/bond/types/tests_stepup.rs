//! Bond tests: floating CFKind, serde, pricing_cashflows, and step-up coupon bonds.

use super::*;
use crate::instruments::common_impl::traits::{Attributes, Instrument};
use crate::instruments::InstrumentPricingOverrides;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;
use time::Month;

#[test]
fn test_bond_dated_cashflows_include_floating_cfkind() {
    use crate::cashflow::traits::CashflowProvider;

    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2026, Month::July, 1).expect("Valid test date");

    let frn = Bond::floating(
        "FRN-CFKIND-TEST",
        Money::new(1_000_000.0, Currency::USD),
        "USD-LIBOR-3M",
        200,
        issue,
        maturity,
        Tenor::quarterly(),
        DayCount::Act365F,
        "USD-OIS",
    )
    .unwrap();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (2.0, 0.90)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("CashFlowSchedule builder should succeed with valid test data");

    let fwd = ForwardCurve::builder("USD-LIBOR-3M", 0.25)
        .base_date(issue - time::Duration::days(10))
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.04), (2.0, 0.045)])
        .build()
        .expect("CashFlowSchedule builder should succeed with valid test data");

    let market = MarketContext::new().insert(disc).insert(fwd);

    // Build simplified schedule
    let flows = frn
        .dated_cashflows(&market, issue)
        .expect("Schedule building should succeed in test");

    // Should have multiple flows (quarterly coupons + redemption)
    // Approximately 6 quarters over 18 months
    assert!(
        flows.len() >= 5,
        "FRN should have multiple quarterly flows, got {}",
        flows.len()
    );

    // Signed schedule includes both positive (coupons, redemption) and negative (initial notional) flows
    let has_positive = flows.iter().any(|(_, m)| m.amount() > 0.0);
    let has_negative = flows.iter().any(|(_, m)| m.amount() < 0.0);
    assert!(
        has_positive,
        "Signed FRN schedule has positive flows (coupons, redemption)"
    );
    assert!(
        has_negative,
        "Signed FRN schedule has negative flows (initial notional)"
    );

    // Verify flows are sorted by date
    for i in 1..flows.len() {
        assert!(
            flows[i].0 >= flows[i - 1].0,
            "Flows should be sorted by date"
        );
    }
}

#[test]
fn test_bond_serde_allows_missing_issue_date_with_custom_cashflows() {
    use crate::cashflow::builder::{CashFlowSchedule, CouponType, FixedCouponSpec};
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};

    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid");
    let maturity = Date::from_calendar_date(2027, Month::January, 15).expect("valid");

    let schedule = CashFlowSchedule::builder()
        .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: Decimal::try_from(0.05).expect("valid"),
            schedule: finstack_quant_cashflows::builder::ScheduleParams {
                freq: Tenor::semi_annual(),

                dc: DayCount::Act365F,

                bdc: BusinessDayConvention::Following,

                calendar_id: "weekends_only".to_string(),

                stub: StubKind::None,

                end_of_month: false,

                payment_lag_days: 0,

                adjust_accrual_dates: false,
            },
        })
        .build(None)
        .expect("schedule build");

    let bond = Bond::from_cashflows("SERDE-NO-ISSUE", schedule, "USD-OIS", Some(99.0))
        .expect("bond from cashflows");

    let mut value = serde_json::to_value(&bond).expect("serialize");
    let obj = value
        .as_object_mut()
        .expect("Bond should serialize to an object");
    obj.remove("issue_date");

    let restored: Bond = serde_json::from_value(value).expect("deserialize");
    assert_eq!(restored.issue_date, issue);
    assert!(restored.issue_date < restored.maturity);
}

#[test]
fn test_bond_serde_rejects_missing_issue_date_without_custom_cashflows() {
    let mut value =
        serde_json::to_value(Bond::example().expect("Bond example is valid")).expect("serialize");
    let obj = value
        .as_object_mut()
        .expect("Bond should serialize to an object");
    obj.remove("issue_date");
    let err = serde_json::from_value::<Bond>(value).expect_err("expected error");
    assert!(
        err.to_string().contains("Bond requires `issue_date`"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn test_bond_serde_rejects_missing_issue_date_even_with_clean_price() {
    let mut value =
        serde_json::to_value(Bond::example().expect("Bond example is valid")).expect("serialize");
    let obj = value
        .as_object_mut()
        .expect("Bond should serialize to an object");
    obj.remove("issue_date");
    obj.insert(
        "pricing_overrides".to_string(),
        serde_json::to_value(InstrumentPricingOverrides::default().with_quoted_clean_price(99.0))
            .expect("serialize pricing overrides"),
    );
    let err = serde_json::from_value::<Bond>(value).expect_err("expected error");
    assert!(
        err.to_string().contains("Bond requires `issue_date`"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn test_bond_custom_cashflows_serde_roundtrip() {
    use crate::cashflow::builder::{CashFlowSchedule, CouponType, FixedCouponSpec};
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};

    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid");
    let maturity = Date::from_calendar_date(2027, Month::January, 15).expect("valid");

    let schedule = CashFlowSchedule::builder()
        .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: Decimal::try_from(0.06).expect("valid"),
            schedule: finstack_quant_cashflows::builder::ScheduleParams {
                freq: Tenor::semi_annual(),

                dc: DayCount::Act365F,

                bdc: BusinessDayConvention::Following,

                calendar_id: "weekends_only".to_string(),

                stub: StubKind::None,

                end_of_month: false,

                payment_lag_days: 0,

                adjust_accrual_dates: false,
            },
        })
        .build(None)
        .expect("schedule build");

    let flow_count = schedule.get_flows().len();
    let bond = Bond::from_cashflows("SERDE-RT", schedule, "USD-OIS", Some(98.0))
        .expect("bond from cashflows");

    let json = serde_json::to_string(&bond).expect("serialize");
    let restored: Bond = serde_json::from_str(&json).expect("deserialize");

    assert!(restored.custom_cashflows.is_some());
    assert_eq!(
        restored
            .custom_cashflows
            .as_ref()
            .expect("should have custom cashflows")
            .get_flows()
            .len(),
        flow_count
    );
}

#[test]
fn test_bond_serde_roundtrip_preserves_funding_curve_id() {
    let mut value =
        serde_json::to_value(Bond::example().expect("Bond example is valid")).expect("serialize");
    let obj = value
        .as_object_mut()
        .expect("Bond should serialize to an object");
    obj.insert(
        "funding_curve_id".to_string(),
        serde_json::Value::String("USD-REPO".to_string()),
    );

    let restored: Bond = serde_json::from_value(value).expect("deserialize");
    assert_eq!(
        restored
            .funding_curve_id
            .as_ref()
            .expect("funding curve should deserialize")
            .as_str(),
        "USD-REPO"
    );
    assert_eq!(
        restored
            .funding_curve_id()
            .expect("bond should expose funding curve through Instrument")
            .as_str(),
        "USD-REPO"
    );
}

#[test]
fn bond_price_merton_mc_api() {
    use crate::instruments::fixed_income::bond::pricing::engine::merton_mc::MertonMcConfig;
    use crate::models::credit::MertonModel;

    // Use Corporate convention (30/360) to avoid ActActIsma frequency requirement
    let bond = Bond::with_convention(
        "CORP-TEST",
        finstack_quant_core::money::Money::new(
            1_000_000.0,
            finstack_quant_core::currency::Currency::USD,
        ),
        finstack_quant_core::types::Rate::from_decimal(0.05),
        time::macros::date!(2024 - 01 - 15),
        time::macros::date!(2034 - 01 - 15),
        crate::instruments::common_impl::parameters::BondConvention::Corporate,
        "USD-OIS",
    )
    .expect("valid corporate bond");
    let merton = MertonModel::new(200.0, 0.25, 100.0, 0.04).expect("valid");
    let config = MertonMcConfig::new(merton).num_paths(1000).seed(42);
    let result = bond
        .price_merton_mc(&config, 0.04, time::macros::date!(2024 - 01 - 15))
        .expect("ok");
    assert!(
        result.clean_price_pct > 0.0 && result.clean_price_pct < 200.0,
        "Price should be reasonable: got {}",
        result.clean_price_pct
    );
}

/// M2.11: the Merton MC engine assumes a constant notional with bullet
/// redemption; amortizing bonds must be rejected, not silently priced as
/// bullets off the base coupon spec.
#[test]
fn bond_price_merton_mc_rejects_amortizing() {
    use crate::cashflow::builder::AmortizationSpec;
    use crate::instruments::fixed_income::bond::pricing::engine::merton_mc::MertonMcConfig;
    use crate::models::credit::MertonModel;
    use finstack_quant_core::types::CurveId;

    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid");
    let maturity = Date::from_calendar_date(2028, Month::January, 1).expect("valid");
    let step1 = Date::from_calendar_date(2026, Month::January, 1).expect("valid");

    let base = CashflowSpec::fixed(0.05, Tenor::annual(), DayCount::Act365F).expect("spec");
    let amort = AmortizationSpec::StepRemaining {
        schedule: vec![
            (step1, Money::new(500_000.0, Currency::USD)),
            (maturity, Money::new(0.0, Currency::USD)),
        ],
    };
    let bond = Bond::builder()
        .id("AMORT-MERTON".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(CashflowSpec::amortizing(base, amort))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .instrument_pricing_overrides(crate::instruments::InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("bond builds");

    let merton = MertonModel::new(200.0, 0.25, 100.0, 0.04).expect("valid");
    let config = MertonMcConfig::new(merton).num_paths(100).seed(42);
    let err = bond
        .price_merton_mc(&config, 0.04, issue)
        .expect_err("amortizing bonds must be rejected by Merton MC");
    assert!(
        err.to_string().contains("amortizing"),
        "error should explain the amortization limitation, got: {err}"
    );
}

#[test]
fn pricing_cashflows_discount_only() {
    use finstack_quant_core::types::CurveId;

    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
    let maturity = Date::from_calendar_date(2027, Month::January, 15).expect("Valid test date");

    let bond = Bond::builder()
        .id("PC_DISC".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                .expect("finite test coupon"),
        )
        .discount_curve_id(CurveId::new("USD-OIS"))
        .instrument_pricing_overrides(crate::instruments::InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("Bond builder should succeed");

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (3.0, 0.92)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("DiscountCurve builder should succeed");
    let market = MarketContext::new().insert(disc);

    let frame = bond
        .pricing_cashflows(&market, Some(issue))
        .expect("pricing_cashflows should succeed");

    assert!(
        !frame.discount_factors.is_empty(),
        "Should have discount factors"
    );
    assert!(
        frame.survival_probs.is_none(),
        "No hazard curve -> no survival probs"
    );
    for df in &frame.discount_factors {
        assert!(*df > 0.0 && *df <= 1.0, "DF should be in (0,1]: got {df}");
    }
    assert_eq!(frame.pvs.len(), frame.discount_factors.len());
}

#[test]
fn pricing_cashflows_with_hazard_curve() {
    use finstack_quant_core::market_data::term_structures::HazardCurve;
    use finstack_quant_core::types::CurveId;

    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
    let maturity = Date::from_calendar_date(2027, Month::January, 15).expect("Valid test date");

    let bond = Bond::builder()
        .id("PC_HAZARD".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                .expect("finite test coupon"),
        )
        .discount_curve_id(CurveId::new("USD-OIS"))
        .credit_curve_id_opt(Some(CurveId::new("USD-CREDIT")))
        .instrument_pricing_overrides(crate::instruments::InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("Bond builder should succeed");

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (3.0, 0.92)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("DiscountCurve builder should succeed");

    let hazard = HazardCurve::builder("USD-CREDIT")
        .base_date(issue)
        .recovery_rate(0.40)
        .knots([(0.0, 0.02), (3.0, 0.02)])
        .build()
        .expect("HazardCurve builder should succeed");

    let market = MarketContext::new().insert(disc).insert(hazard);

    let frame = bond
        .pricing_cashflows(&market, Some(issue))
        .expect("pricing_cashflows should succeed");

    assert!(
        !frame.discount_factors.is_empty(),
        "Should have discount factors"
    );
    let sp = frame
        .survival_probs
        .as_ref()
        .expect("Hazard curve provided -> survival probs should be present");
    assert_eq!(
        sp.len(),
        frame.discount_factors.len(),
        "survival_probs and discount_factors must have same length"
    );

    for (i, sp_opt) in sp.iter().enumerate() {
        let s = sp_opt.expect("Each row should have a survival probability");
        assert!(
            s > 0.0 && s <= 1.0,
            "Survival prob should be in (0,1]: got {s} at row {i}"
        );
    }

    // PV = amount * DF * survival_prob (for future cashflows)
    for (i, pv) in frame.pvs.iter().enumerate() {
        if frame.pay_dates[i] > issue {
            let expected = frame.amounts[i] * frame.discount_factors[i] * sp[i].unwrap_or(1.0);
            assert!(
                (*pv - expected).abs() < 1e-6,
                "PV mismatch at row {i}: got {pv}, expected {expected}"
            );
        }
    }
}

// =========================================================================
// Step-up coupon bond tests
// =========================================================================

/// Helper: build a step-up bond and return its full cashflow schedule.
fn build_step_up_bond_schedule(
    initial_rate: f64,
    step_schedule: Vec<(Date, f64)>,
    issue: Date,
    maturity: Date,
) -> (Bond, crate::cashflow::builder::CashFlowSchedule) {
    let bond = Bond::builder()
        .id("STEP-UP-TEST".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::step_up(
                initial_rate,
                step_schedule,
                Tenor::semi_annual(),
                DayCount::Thirty360,
            )
            .expect("finite test rate"),
        )
        .discount_curve_id("USD-OIS".into())
        .instrument_pricing_overrides(crate::instruments::InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("Bond builder should succeed for step-up test");

    let market = MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(issue)
            .knots([(0.0, 1.0), (5.0, 0.90)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("DiscountCurve builder should succeed in test"),
    );

    let schedule = bond
        .full_cashflow_schedule(&market)
        .expect("full_cashflow_schedule should succeed for step-up bond");

    (bond, schedule)
}

#[test]
fn step_up_no_steps_equals_fixed_rate() {
    use crate::cashflow::primitives::CFKind;

    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2027, Month::January, 1).expect("Valid test date");

    // Step-up with no steps = fixed rate at initial_rate
    let (_, step_sched) = build_step_up_bond_schedule(0.05, vec![], issue, maturity);

    // Build equivalent fixed-rate bond
    let fixed_bond = Bond::builder()
        .id("FIXED-EQUIV".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Thirty360)
                .expect("finite test coupon"),
        )
        .discount_curve_id("USD-OIS".into())
        .instrument_pricing_overrides(crate::instruments::InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("Bond builder should succeed for fixed test");

    let market = MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(issue)
            .knots([(0.0, 1.0), (5.0, 0.90)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("DiscountCurve builder should succeed in test"),
    );

    let fixed_sched = fixed_bond
        .full_cashflow_schedule(&market)
        .expect("full_cashflow_schedule should succeed for fixed bond");

    // Compare coupon flows
    let step_coupons: Vec<_> = step_sched
        .get_flows()
        .iter()
        .filter(|cf| matches!(cf.kind, CFKind::Fixed | CFKind::Stub))
        .collect();
    let fixed_coupons: Vec<_> = fixed_sched
        .get_flows()
        .iter()
        .filter(|cf| matches!(cf.kind, CFKind::Fixed | CFKind::Stub))
        .collect();

    assert_eq!(
        step_coupons.len(),
        fixed_coupons.len(),
        "Step-up with no steps should produce same number of coupons as fixed"
    );

    for (s, f) in step_coupons.iter().zip(fixed_coupons.iter()) {
        assert_eq!(s.date, f.date, "Coupon dates should match");
        assert!(
            (s.amount.amount() - f.amount.amount()).abs() < 1e-6,
            "Coupon amounts should match: step={}, fixed={}",
            s.amount.amount(),
            f.amount.amount()
        );
        assert_eq!(s.rate, f.rate, "Coupon rates should match");
    }
}

#[test]
fn step_up_one_step_changes_rate() {
    use crate::cashflow::primitives::CFKind;

    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2027, Month::January, 1).expect("Valid test date");
    let step_date = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");

    // 3% for first year, then 5% for second year
    let (_, schedule) = build_step_up_bond_schedule(0.03, vec![(step_date, 0.05)], issue, maturity);

    let coupons: Vec<_> = schedule
        .get_flows()
        .iter()
        .filter(|cf| matches!(cf.kind, CFKind::Fixed | CFKind::Stub))
        .collect();

    // Semi-annual over 2 years = 4 coupon periods
    assert_eq!(
        coupons.len(),
        4,
        "Should have 4 semi-annual coupons over 2 years"
    );

    // First 2 coupons (before step_date) should have initial_rate = 3%
    for c in &coupons[..2] {
        assert_eq!(
            c.rate,
            Some(0.03),
            "Pre-step coupon rate should be 3%, got {:?} on {}",
            c.rate,
            c.date
        );
    }

    // Last 2 coupons (on or after step_date) should have stepped rate = 5%
    for c in &coupons[2..] {
        assert_eq!(
            c.rate,
            Some(0.05),
            "Post-step coupon rate should be 5%, got {:?} on {}",
            c.rate,
            c.date
        );
    }

    // Verify amounts are consistent with rates:
    // Notional = 1,000,000, 30/360 semi-annual, so each period is 0.5 years
    // 3% coupon: 1,000,000 * 0.03 * 0.5 = 15,000
    // 5% coupon: 1,000,000 * 0.05 * 0.5 = 25,000
    for c in &coupons[..2] {
        assert!(
            (c.amount.amount() - 15_000.0).abs() < 1.0,
            "3% coupon should be ~15000, got {}",
            c.amount.amount()
        );
    }
    for c in &coupons[2..] {
        assert!(
            (c.amount.amount() - 25_000.0).abs() < 1.0,
            "5% coupon should be ~25000, got {}",
            c.amount.amount()
        );
    }
}

#[test]
fn step_up_multiple_steps() {
    use crate::cashflow::primitives::CFKind;

    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2028, Month::January, 1).expect("Valid test date");
    let step1 = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");
    let step2 = Date::from_calendar_date(2027, Month::January, 1).expect("Valid test date");

    // 2% -> 4% after 1 year -> 6% after 2 years
    let (_, schedule) =
        build_step_up_bond_schedule(0.02, vec![(step1, 0.04), (step2, 0.06)], issue, maturity);

    let coupons: Vec<_> = schedule
        .get_flows()
        .iter()
        .filter(|cf| matches!(cf.kind, CFKind::Fixed | CFKind::Stub))
        .collect();

    // Semi-annual over 3 years = 6 coupon periods
    assert_eq!(
        coupons.len(),
        6,
        "Should have 6 semi-annual coupons over 3 years"
    );

    // Verify rates by period
    let expected_rates = [0.02, 0.02, 0.04, 0.04, 0.06, 0.06];
    for (i, (c, &expected_rate)) in coupons.iter().zip(expected_rates.iter()).enumerate() {
        assert_eq!(
            c.rate,
            Some(expected_rate),
            "Period {} rate should be {}, got {:?}",
            i,
            expected_rate,
            c.rate
        );
    }
}

#[test]
fn step_up_serde_roundtrip() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2027, Month::January, 1).expect("Valid test date");
    let step_date = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");

    let spec = CashflowSpec::step_up(
        0.03,
        vec![(step_date, 0.05)],
        Tenor::semi_annual(),
        DayCount::Thirty360,
    )
    .expect("finite test rate");

    let bond = Bond::builder()
        .id("SERDE-STEP-UP".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(spec)
        .discount_curve_id("USD-OIS".into())
        .instrument_pricing_overrides(crate::instruments::InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("Bond builder should succeed");

    // Serialize to JSON
    let json = serde_json::to_string(&bond).expect("Serialization should succeed");

    // Verify it contains StepUp tag
    assert!(
        json.contains("StepUp"),
        "Serialized JSON should contain StepUp variant tag"
    );

    // Deserialize back
    let bond2: Bond = serde_json::from_str(&json).expect("Deserialization should succeed");

    // Verify roundtrip fidelity
    assert_eq!(bond.id.as_str(), bond2.id.as_str());
    assert_eq!(bond.issue_date, bond2.issue_date);
    assert_eq!(bond.maturity, bond2.maturity);
    assert_eq!(
        bond.cashflow_spec.frequency(),
        bond2.cashflow_spec.frequency()
    );
    assert_eq!(
        bond.cashflow_spec.day_count(),
        bond2.cashflow_spec.day_count()
    );

    // Verify the spec details survived roundtrip
    match (&bond.cashflow_spec, &bond2.cashflow_spec) {
        (CashflowSpec::StepUp(a), CashflowSpec::StepUp(b)) => {
            assert_eq!(a.initial_rate, b.initial_rate);
            assert_eq!(a.step_schedule.len(), b.step_schedule.len());
            for ((d1, r1), (d2, r2)) in a.step_schedule.iter().zip(b.step_schedule.iter()) {
                assert_eq!(d1, d2);
                assert_eq!(r1, r2);
            }
        }
        _ => panic!("Expected StepUp variant after roundtrip"),
    }
}

#[test]
fn step_up_frequency_and_day_count() {
    let _issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let step_date = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");

    let spec = CashflowSpec::step_up(
        0.03,
        vec![(step_date, 0.05)],
        Tenor::quarterly(),
        DayCount::Act360,
    )
    .expect("finite test rate");

    assert_eq!(spec.frequency(), Tenor::quarterly());
    assert_eq!(spec.day_count(), DayCount::Act360);
}
