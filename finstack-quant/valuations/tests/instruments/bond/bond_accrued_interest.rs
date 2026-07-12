//! Tests for bond accrued interest calculations with different accrual methods.
//!
//! Validates Linear, Compounded (ICMA Rule 251), and Indexed accrual conventions
//! against known market calculations.
//!
//! Note: These tests exercise the core `accrued_interest_amount()` function
//! directly. For metrics-interface tests (via `MetricId::Accrued`), see
//! `metrics/accrued.rs` which validates the integration with the metrics framework.

use finstack_quant_cashflows::{accrued_interest_amount, CashflowProvider};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::{AccrualMethod, Bond};
use time::Month;

fn make_date(y: i32, m: u8, d: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
}

#[test]
fn test_accrued_interest_linear_default() {
    // Standard bond with linear accrual (default)
    let bond = Bond::fixed(
        "LINEAR_TEST",
        Money::new(100.0, Currency::USD),
        0.06, // 6% annual, semi-annual payments = 3% per period
        make_date(2025, 1, 1),
        make_date(2030, 1, 1),
        "USD-OIS",
    )
    .unwrap();

    // Default accrual method should be Linear
    assert!(matches!(bond.accrual_method, AccrualMethod::Linear));

    // Accrual halfway through first coupon period
    // Period: 2025-01-01 to 2025-07-01 (180 days in 30/360)
    // As of: 2025-04-01 (90 days)
    let as_of = make_date(2025, 4, 1);

    let schedule = bond
        .cashflow_schedule(&MarketContext::new(), as_of)
        .unwrap();
    let accrued = accrued_interest_amount(&schedule, as_of, &bond.accrual_config()).unwrap();

    // Expected: 3% coupon * (90/180) = 1.5% of notional = $1.50
    let expected = 1.50;
    assert!(
        (accrued - expected).abs() < 1e-6,
        "Linear accrual: expected {}, got {}",
        expected,
        accrued
    );
}

#[test]
fn test_accrued_interest_compounded_vs_linear() {
    // Compare compounded vs linear accrual for same bond

    // Linear accrual bond (default)
    let bond_linear = Bond::fixed(
        "LINEAR",
        Money::new(100.0, Currency::USD),
        0.06, // 6% annual coupon, semi-annual = 3% per period
        make_date(2025, 1, 1),
        make_date(2030, 1, 1),
        "USD-OIS",
    )
    .unwrap();

    // Compounded accrual bond
    let mut bond_compounded = Bond::fixed(
        "COMPOUNDED",
        Money::new(100.0, Currency::USD),
        0.06,
        make_date(2025, 1, 1),
        make_date(2030, 1, 1),
        "USD-OIS",
    )
    .unwrap();
    bond_compounded.accrual_method = AccrualMethod::Compounded;

    // Accrual at quarter-point (90 days out of 180)
    let as_of = make_date(2025, 4, 1);

    let sched_linear = bond_linear
        .cashflow_schedule(&MarketContext::new(), as_of)
        .unwrap();
    let accrued_linear =
        accrued_interest_amount(&sched_linear, as_of, &bond_linear.accrual_config()).unwrap();

    let sched_comp = bond_compounded
        .cashflow_schedule(&MarketContext::new(), as_of)
        .unwrap();
    let accrued_compounded =
        accrued_interest_amount(&sched_comp, as_of, &bond_compounded.accrual_config()).unwrap();

    // Linear: 3% × (90/180) = 1.50%
    let expected_linear = 1.50;

    // Compounded (ICMA Rule 251): 100 × [(1.03)^(90/180) - 1]
    // = 100 × [(1.03)^0.5 - 1]
    // = 100 × [1.014889 - 1]
    // = 1.4889%
    let expected_compounded = 1.4889;

    assert!(
        (accrued_linear - expected_linear).abs() < 1e-2,
        "Linear: expected {}, got {}",
        expected_linear,
        accrued_linear
    );

    assert!(
        (accrued_compounded - expected_compounded).abs() < 1e-2,
        "Compounded: expected {}, got {}",
        expected_compounded,
        accrued_compounded
    );

    // Difference should be material (~1bp on $100 notional)
    assert!(
        (accrued_linear - accrued_compounded).abs() > 0.005,
        "Linear ({}) and compounded ({}) should differ materially",
        accrued_linear,
        accrued_compounded
    );
}

#[test]
fn test_accrued_interest_compounded_zero_coupon() {
    // Zero-coupon bond should have zero accrued regardless of method
    let mut bond = Bond::fixed(
        "ZERO",
        Money::new(100.0, Currency::USD),
        0.0, // Zero coupon
        make_date(2025, 1, 1),
        make_date(2030, 1, 1),
        "USD-OIS",
    )
    .unwrap();
    bond.accrual_method = AccrualMethod::Compounded;

    let as_of = make_date(2025, 4, 1);
    let schedule = bond
        .cashflow_schedule(&MarketContext::new(), as_of)
        .unwrap();
    let accrued = accrued_interest_amount(&schedule, as_of, &bond.accrual_config()).unwrap();

    assert!(
        accrued.abs() < 1e-10,
        "Zero-coupon bond should have zero accrued"
    );
}

#[test]
fn test_accrued_interest_ex_coupon_period() {
    // Market standard (e.g. UK gilts): within the ex-coupon window the seller
    // keeps the coupon and accrued interest is NEGATIVE — the buyer is
    // compensated for the remaining stub from settlement to the coupon date.
    let mut bond = Bond::fixed(
        "EX_COUPON",
        Money::new(100.0, Currency::USD),
        0.05,
        make_date(2025, 1, 1),
        make_date(2030, 1, 1),
        "USD-OIS",
    )
    .unwrap();
    bond.settlement_convention = Some(
        finstack_quant_valuations::instruments::fixed_income::bond::BondSettlementConvention {
            ex_coupon_days: 7,
            ..Default::default()
        },
    );

    // 5 days before coupon (within ex-coupon window)
    let coupon_date = make_date(2025, 7, 1);
    let as_of = coupon_date - time::Duration::days(5);

    let schedule = bond
        .cashflow_schedule(&MarketContext::new(), as_of)
        .unwrap();
    let accrued = accrued_interest_amount(&schedule, as_of, &bond.accrual_config()).unwrap();

    assert!(
        accrued < 0.0,
        "Accrued should be negative during ex-coupon period, got {accrued}"
    );
    // Semi-annual coupon = 2.50 per 100; remaining stub is 5 days out of the
    // ~181-day period: accrued ≈ -2.50 × 5/181.
    let period_days = (coupon_date - make_date(2025, 1, 1)).whole_days() as f64;
    let expected = -2.5 * 5.0 / period_days;
    assert!(
        (accrued - expected).abs() < 0.02,
        "Expected ~{expected}, got {accrued}"
    );
}

#[test]
fn test_accrued_interest_just_before_ex_coupon_window_positive() {
    // One day before the ex-date the bond still trades cum-coupon with nearly
    // a full period of positive accrued interest.
    let mut bond = Bond::fixed(
        "CUM_COUPON",
        Money::new(100.0, Currency::USD),
        0.05,
        make_date(2025, 1, 1),
        make_date(2030, 1, 1),
        "USD-OIS",
    )
    .unwrap();
    bond.settlement_convention = Some(
        finstack_quant_valuations::instruments::fixed_income::bond::BondSettlementConvention {
            ex_coupon_days: 7,
            ..Default::default()
        },
    );

    let coupon_date = make_date(2025, 7, 1);
    let as_of = coupon_date - time::Duration::days(8); // one day before ex-date

    let schedule = bond
        .cashflow_schedule(&MarketContext::new(), as_of)
        .unwrap();
    let accrued = accrued_interest_amount(&schedule, as_of, &bond.accrual_config()).unwrap();

    assert!(
        accrued > 2.0,
        "Cum-coupon accrued should be close to the full coupon, got {accrued}"
    );
}

#[test]
fn test_accrued_interest_at_coupon_boundaries() {
    let bond = Bond::fixed(
        "BOUNDARY",
        Money::new(100.0, Currency::USD),
        0.04, // 4% annual, semi-annual = 2% per period
        make_date(2025, 1, 1),
        make_date(2030, 1, 1),
        "USD-OIS",
    )
    .unwrap();

    // Midway through first coupon period (should have accrual)
    let midway = make_date(2025, 4, 1); // 90 days into first 180-day period
    let schedule = bond
        .cashflow_schedule(&MarketContext::new(), midway)
        .unwrap();
    let accrued_midway =
        accrued_interest_amount(&schedule, midway, &bond.accrual_config()).unwrap();

    // Should be positive: 2% × (90/180) = 1% of notional = $1.00
    assert!(
        accrued_midway > 0.99 && accrued_midway < 1.01,
        "Accrued at midway: {}",
        accrued_midway
    );
}

#[test]
fn test_accrued_interest_amortizing_schedule_driven() {
    use finstack_quant_cashflows::builder::AmortizationSpec;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::dates::DayCountContext;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;

    // 3-year annual amortizing bond, 5% coupon, 1/3 principal returned each year.
    let issue = make_date(2025, 1, 1);
    let year1 = make_date(2026, 1, 1);
    let year2 = make_date(2027, 1, 1);
    let maturity = make_date(2028, 1, 1);

    let notional = Money::new(1_000_000.0, Currency::USD);

    // StepRemaining schedule encodes remaining outstanding after each date.
    // For a 3-year, 1/3-per-year amortization this means:
    // - After year1: 2/3 notional outstanding
    // - After year2: 1/3 notional outstanding
    // - At maturity: 0 outstanding
    let amort_spec = AmortizationSpec::StepRemaining {
        schedule: vec![
            (year1, Money::new(2.0 * 1_000_000.0 / 3.0, Currency::USD)),
            (year2, Money::new(1.0 * 1_000_000.0 / 3.0, Currency::USD)),
            (maturity, Money::new(0.0, Currency::USD)),
        ],
    };
    let base_spec =
        CashflowSpec::fixed(0.05, Tenor::annual(), DayCount::Act365F).expect("finite test coupon");
    let cashflow_spec = CashflowSpec::amortizing(base_spec, amort_spec);

    let bond = Bond::builder()
        .id("AMORT_AI".into())
        .notional(notional)
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(cashflow_spec)
        .discount_curve_id("USD-OIS".into())
        .pricing_overrides(Default::default())
        .build()
        .unwrap();

    // Simple downward-sloping discount curve; actual level is irrelevant for AI.
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (3.0, 0.9)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let curves = MarketContext::new().insert(disc);

    // Midway through last coupon period (year2 to maturity): outstanding notional
    // is 1/3 of original. We verify that accrued interest uses the actual
    // schedule coupon for this period, not `notional * rate` on the original
    // notional.
    let as_of = make_date(2027, 7, 1);

    // Use issue date to retrieve the full schedule (including historical coupon
    // periods) since accrued interest calculation needs the complete period
    // structure. This mirrors the production path which uses
    // `Bond::full_cashflow_schedule` internally.
    let schedule = bond.cashflow_schedule(&curves, issue).unwrap();
    let accrued = accrued_interest_amount(&schedule, as_of, &bond.accrual_config()).unwrap();

    let schedule = bond
        .cashflow_schedule(&curves, issue)
        .expect("Full schedule retrieval should succeed in test");
    use finstack_quant_cashflows::primitives::CFKind;
    let mut coupon_dates: Vec<(Date, f64)> = Vec::new();
    for cf in &schedule.flows {
        if matches!(cf.kind, CFKind::Fixed | CFKind::Stub) {
            if let Some((d, total)) = coupon_dates.last_mut() {
                if *d == cf.date {
                    *total += cf.amount.amount();
                    continue;
                }
            }
            coupon_dates.push((cf.date, cf.amount.amount()));
        }
    }
    assert!(
        coupon_dates.len() >= 2,
        "Amortizing test schedule should have at least two coupon dates"
    );
    // Locate the period containing `as_of`.
    //
    // The accrual engine uses the builder-supplied `accrual_factor` on the
    // coupon flow as the *period length* (in year fractions) when present,
    // rather than recomputing it from the flow's payment date. This matters
    // when the payment date has been shifted by a business-day convention
    // (e.g. ModifiedFollowing moving a Saturday maturity to the next Monday):
    // the intended coupon period is still "1 year", not "1 year + 2 days".
    //
    // Mirror that convention here so the expected value tracks the impl.
    type CouponInfo = (Date, f64, Option<f64>, Option<(Date, Date)>);
    let coupon_info: Vec<CouponInfo> = {
        use finstack_quant_cashflows::primitives::CFKind;
        let mut out: Vec<CouponInfo> = Vec::new();
        for cf in &schedule.flows {
            if !matches!(cf.kind, CFKind::Fixed | CFKind::Stub) {
                continue;
            }
            let af = if cf.accrual_factor > 0.0 {
                Some(cf.accrual_factor)
            } else {
                None
            };
            let accrual_period = cf.accrual.map(|accrual| (accrual.start, accrual.end));
            if let Some(last) = out.last_mut() {
                if last.0 == cf.date {
                    last.1 += cf.amount.amount();
                    last.2 = last.2.or(af);
                    last.3 = last.3.or(accrual_period);
                    continue;
                }
            }
            out.push((cf.date, cf.amount.amount(), af, accrual_period));
        }
        out
    };
    let mut expected = 0.0;
    let mut prev = issue;
    for (payment_date, coupon_total, af, accrual_period) in coupon_info {
        let (accrual_start, accrual_end) = accrual_period.unwrap_or((prev, payment_date));
        if accrual_start <= as_of && as_of < accrual_end {
            let total_period = match af {
                Some(v) => v,
                None => schedule
                    .day_count
                    .year_fraction(accrual_start, accrual_end, DayCountContext::default())
                    .unwrap(),
            };
            let dc_elapsed = schedule
                .day_count
                .year_fraction(accrual_start, as_of, DayCountContext::default())
                .unwrap()
                .max(0.0);
            let dc_total = schedule
                .day_count
                .year_fraction(accrual_start, accrual_end, DayCountContext::default())
                .unwrap();
            let elapsed = if dc_total.is_finite() && dc_total > 0.0 {
                total_period * dc_elapsed / dc_total
            } else {
                dc_elapsed
            }
            .clamp(0.0, total_period);
            expected = coupon_total * (elapsed / total_period);
            break;
        }
        prev = accrual_end;
    }
    assert!(
        expected > 0.0,
        "Expected schedule-derived accrued interest should be positive"
    );

    assert!(
        (accrued - expected).abs() < 1.0,
        "Amortizing AI should be schedule-driven; expected ~{}, got {}",
        expected,
        accrued
    );
}

#[test]
fn test_accrual_method_serialization() {
    // Test that accrual method survives JSON roundtrip
    let mut bond = Bond::fixed(
        "SERDE_TEST",
        Money::new(1000.0, Currency::EUR),
        0.025,
        make_date(2025, 1, 1),
        make_date(2035, 1, 1),
        "EUR-OIS",
    )
    .unwrap();
    bond.accrual_method = AccrualMethod::Compounded;

    let json = serde_json::to_string(&bond).expect("Serialization should succeed in test");
    let deserialized: Bond =
        serde_json::from_str(&json).expect("Deserialization should succeed in test");

    // Verify accrual method survived roundtrip
    match &deserialized.accrual_method {
        AccrualMethod::Compounded => {
            // Compounded accrual method has no additional fields
        }
        _ => panic!("Expected Compounded accrual method"),
    }
}

#[test]
fn test_bond_deserialization_defaults_accrual_method_and_call_put_period() {
    let bond = Bond::fixed(
        "CALL_PERIOD_DEFAULT_ACCRUAL",
        Money::new(1000.0, Currency::USD),
        0.05,
        make_date(2025, 1, 1),
        make_date(2030, 1, 1),
        "USD-OIS",
    )
    .unwrap();

    let mut json = serde_json::to_value(&bond).expect("Bond serialization should succeed in test");
    let spec = json
        .as_object_mut()
        .expect("serialized bond should be a JSON object");
    spec.remove("accrual_method");
    spec.insert(
        "call_put".to_string(),
        serde_json::json!({
            "calls": [{
                "start_date": "2027-01-01",
                "end_date": "2028-01-01",
                "price_pct_of_par": 101.0
            }],
            "puts": []
        }),
    );

    let deserialized: Bond =
        serde_json::from_value(json).expect("bond should accept call/put periods");
    assert!(matches!(deserialized.accrual_method, AccrualMethod::Linear));

    let roundtrip =
        serde_json::to_value(&deserialized).expect("Bond serialization should succeed in test");
    let call = &roundtrip["call_put"]["calls"][0];
    assert_eq!(call["start_date"], "2027-01-01");
    assert_eq!(call["end_date"], "2028-01-01");
    assert!(call.get("date").is_none());
    assert!(roundtrip.get("accrual_method").is_none());
}
