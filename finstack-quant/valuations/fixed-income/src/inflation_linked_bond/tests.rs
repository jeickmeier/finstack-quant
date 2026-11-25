use super::*;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::Result;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    InflationExpectedRelease, InflationIndex, InflationLag, InflationObservation,
};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, InflationCurve};
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{Attributes, CurveId, InstrumentId};
use finstack_quant_valuations_test_support::inflation::simple_index;
use rust_decimal::Decimal;
use time::Month;

#[test]
fn stored_cpi_is_not_known_before_publication() {
    use time::macros::date;
    let mut bond = InflationLinkedBond::example().expect("example");
    bond.base_cpi = 100.0;
    bond.lag = InflationLag::months(3);
    let index = InflationIndex::new("US-CPI", vec![], Currency::USD, date!(2025 - 04 - 14))
        .expect("index")
        .with_expected_releases(vec![InflationExpectedRelease {
            reference_month: date!(2025 - 03 - 01),
            expected_release: date!(2025 - 04 - 15),
            known_on: date!(2025 - 04 - 14),
        }])
        .expect("publication");
    let curve = InflationCurve::builder("US-CPI")
        .base_date(date!(2025 - 03 - 01))
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Flat)
        .knots([(0.0, 120.0), (2.0, 120.0)])
        .build()
        .expect("curve");
    let market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve")
        .insert_inflation_index(index);
    assert_eq!(
        bond.index_ratio_from_market(date!(2025 - 06 - 01), &market, date!(2025 - 04 - 14))
            .expect("projected"),
        1.2
    );
    for as_of in [date!(2025 - 04 - 15), date!(2025 - 04 - 16)] {
        assert_eq!(
            bond.index_ratio_from_market(date!(2025 - 06 - 01), &market, as_of)
                .expect("unchanged explicit snapshot"),
            1.2
        );
    }
    let published = InflationIndex::new(
        "US-CPI",
        vec![InflationObservation {
            reference_month: date!(2025 - 03 - 01),
            value: 150.0,
            available_on: date!(2025 - 04 - 15),
        }],
        Currency::USD,
        date!(2025 - 04 - 15),
    )
    .unwrap();
    assert_eq!(
        bond.index_ratio_from_market(
            date!(2025 - 06 - 01),
            &market.insert_inflation_index(published),
            date!(2025 - 04 - 15),
        )
        .unwrap(),
        1.5
    );
}

#[test]
fn protected_unknown_cpi_includes_option_time_value() {
    use finstack_quant_core::market_data::surfaces::{
        VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
    };
    use time::macros::date;
    let mut bond = InflationLinkedBond::example().expect("example");
    bond.base_cpi = 100.0;
    bond.lag = InflationLag::None;
    let as_of = date!(2025 - 01 - 01);
    let payment = date!(2026 - 01 - 01);
    let market = MarketContext::new()
        .try_insert(
            InflationCurve::builder("US-CPI")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .interp(InterpStyle::LogLinear)
                .extrapolation(ExtrapolationPolicy::Flat)
                .knots([(0.0, 100.0), (2.0, 100.0)])
                .build()
                .expect("curve"),
        )
        .expect("insert market curve")
        .insert_inflation_index(simple_index("US-CPI", as_of, 100.0, Currency::USD));
    assert!(
        (bond
            .payment_index_ratio(payment, &market, as_of, false)
            .expect("unprotected")
            - 1.0)
            .abs()
            < 1e-12
    );
    assert!(
        bond.payment_index_ratio(payment, &market, as_of, true)
            .is_err()
    );
    bond.floor_vol_surface_id = Some("CPI-RATIO-VOL".into());
    for sigma in [0.0, 0.2] {
        let surface = VolSurface::builder("CPI-RATIO-VOL")
            .expiries(&[0.1, 2.0])
            .secondary_values(&[0.5, 1.5])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[sigma, sigma])
            .row(&[sigma, sigma])
            .build()
            .expect("ratio vol");
        let market = market.clone().insert_surface(surface);
        let ratio = bond
            .payment_index_ratio(payment, &market, as_of, true)
            .expect("floor");
        // ATM lognormal ratio: E[max(X,1)] = 2 Phi(sigma/2).
        let expected = 2.0
            * finstack_quant_core::math::special_functions::norm_cdf(sigma / 2.0)
                .expect("valid normal variate");
        assert!((ratio - expected).abs() < 1e-12, "{ratio} vs {expected}");
    }
}

#[test]
fn rejects_retired_top_level_quoted_clean_key() {
    let mut value = serde_json::to_value(InflationLinkedBond::example().expect("example"))
        .expect("serialize linker");
    // schema-rejection-test
    value["quoted_clean"] = serde_json::json!(100.0);
    let error = serde_json::from_value::<InflationLinkedBond>(value)
        .expect_err("retired quoted_clean must be rejected");
    assert!(error.to_string().contains("unknown field"), "{error}");
}

// ── C8 regression: breakeven inflation compounding convention ──────────────

/// Regression test for C8: the Fisher identity is only exact when both the
/// nominal yield and the real yield are expressed in the **same** compounding
/// convention.  Prior to this fix, `breakeven_inflation` applied the Fisher
/// formula directly to a Street-compounded (semi-annual) real yield and an
/// annually-compounded nominal yield, mixing conventions.
///
/// Setup: a 10-year semi-annual ILB with a 4% real coupon priced at par.
/// At par the real yield equals the coupon rate: 4 % (Street / semi-annual).
/// The annual equivalent is (1 + 0.04/2)^2 − 1 = 4.04 %.
///
/// With an 8% annually-compounded nominal yield:
///   correct  breakeven = (1.08) / (1.0404) − 1 ≈ 3.8062 %
///   (wrong)  breakeven = (1.08) / (1.04)   − 1 ≈ 3.8462 %     [~4 bp error]
///
/// The test therefore:
///   • verifies the result is within 0.5 bp of the analytically-correct value
///   • verifies the result is NOT within 0.5 bp of the convention-mismatched value
#[test]
fn breakeven_inflation_uses_consistent_annual_compounding_c8() {
    let as_of = d(2024, Month::January, 15);
    let maturity = d(2034, Month::January, 15); // 10-year bond

    // Semi-annual ILB with a 4% real coupon, priced at par (100).
    // At par the real YTM equals the coupon: 4 % Street (semi-annual).
    let mut bond = InflationLinkedBond {
        id: InstrumentId::new("ILB-C8"),
        notional: Money::from((1_000_000_i64, Currency::USD)),
        real_coupon: Decimal::try_from(0.04).expect("valid coupon"),
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Thirty360,
        issue_date: as_of,
        maturity,
        base_cpi: 100.0,
        base_date: as_of,
        indexation_method: IndexationMethod::Tips,
        lag: InflationLag::None,
        deflation_protection: DeflationProtection::MaturityOnly,
        floor_vol_surface_id: None,
        business_day_convention: BusinessDayConvention::Following,
        stub: StubKind::None,
        calendar_id: None,
        discount_curve_id: CurveId::new("USD-NOM"),
        inflation_index_id: CurveId::new("US-CPI"),
        // par → real yield == coupon (4 % semi-annual)
        instrument_pricing_overrides:
            finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides::default()
                .with_quoted_clean_price_pct(100.0),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
    };

    // The nominal yield is expressed in annual compounding (effective annual).
    let nominal_annual = 0.08_f64;

    // Analytically correct breakeven using annual Fisher identity:
    //   real_annual  = (1 + 0.04/2)^2 − 1 = 1.02^2 − 1
    //   breakeven    = (1 + nominal_annual) / (1 + real_annual) − 1
    let real_semi_annual = 0.04_f64;
    let real_annual = (1.0 + real_semi_annual / 2.0).powi(2) - 1.0; // 4.04 %
    let expected_breakeven = (1.0 + nominal_annual) / (1.0 + real_annual) - 1.0;

    // The convention-mismatched (wrong) result for comparison.
    let wrong_breakeven = (1.0 + nominal_annual) / (1.0 + real_semi_annual) - 1.0;

    // The difference between the correct and wrong answer must be at least 3 bp —
    // confirming this is a genuine discriminator, not a trivial check.
    let discrimination = (wrong_breakeven - expected_breakeven).abs();
    assert!(
        discrimination > 3e-4,
        "test is not discriminating: wrong={wrong_breakeven:.6}, correct={expected_breakeven:.6}, diff={discrimination:.6}"
    );

    let result = bond
        .breakeven_inflation(nominal_annual, as_of)
        .expect("breakeven_inflation should succeed");

    // Must be within 0.5 bp of the analytically-correct answer.
    let tol = 5e-5; // 0.5 bp
    assert!(
        (result - expected_breakeven).abs() < tol,
        "breakeven mismatch: got {result:.6} ({:.4}%), expected {expected_breakeven:.6} ({:.4}%), diff = {:.2} bp",
        result * 100.0,
        expected_breakeven * 100.0,
        (result - expected_breakeven).abs() * 10_000.0,
    );

    // Must NOT be within 0.5 bp of the convention-mismatched value.
    assert!(
        (result - wrong_breakeven).abs() > tol,
        "result {result:.6} is suspiciously close to the convention-mismatched value {wrong_breakeven:.6}"
    );

    // Mutation: verify annual frequency bond uses a different correction factor.
    // Annual Street == Annual compounding, so no convention gap → both old and
    // new code agree, confirming the fix is frequency-aware.
    bond.frequency = Tenor::annual();
    bond.real_coupon = Decimal::try_from(0.04).expect("valid");
    let result_annual_frequency = bond
        .breakeven_inflation(nominal_annual, as_of)
        .expect("annual-frequency breakeven");
    // For annual frequency Street ≡ Annual, so the correction is zero;
    // result should be within 0.5 bp of the "wrong" annual calculation.
    assert!(
        (result_annual_frequency - wrong_breakeven).abs() < tol,
        "annual-frequency bond should not be corrected: got {result_annual_frequency:.6}, wrong={wrong_breakeven:.6}"
    );
}

/// Regression: the canonical TIPS/linker constructors default to ACT/ACT
/// (ISMA), which requires a coupon frequency in the `DayCountContext`.
/// Previously every real year-fraction call passed `DayCountContext::default()`
/// (frequency = `None`) and errored with `MissingFrequencyForActActIsma`, so
/// a default `example()` bond could not be scheduled or accrued at all —
/// tests only passed by overriding `day_count` to `Thirty360`.
#[test]
fn act_act_isma_real_schedule_and_accrual_need_no_day_count_override() {
    let bond = InflationLinkedBond::example().expect("example");
    assert_eq!(bond.day_count, DayCount::ActActIsma);

    let as_of = d(2026, Month::April, 10); // mid-life, between coupons

    let schedule = bond
        .real_cashflow_schedule()
        .expect("ACT/ACT (ISMA) real schedule must not require a day-count override");
    let timed = bond
        .real_flow_times(&schedule, as_of)
        .expect("real yield times");
    assert!(!timed.is_empty(), "schedule should contain future flows");

    let accrued = real_accrued(&bond, as_of)
        .expect("ACT/ACT (ISMA) accrued interest must not require a day-count override");
    assert!(accrued.is_finite() && accrued >= 0.0, "accrued = {accrued}");
}

#[test]
fn real_duration_uses_dirty_model_price_denominator() {
    use crate::bond::pricing::quote_conversions::{YieldCompounding, price_from_ytm_timed};

    let issue = d(2024, Month::January, 15);
    let as_of = d(2024, Month::April, 15);
    let maturity = d(2034, Month::January, 15);
    let bond = InflationLinkedBond {
        id: InstrumentId::new("ILB-DURATION-DIRTY"),
        notional: Money::from((1_000_000_i64, Currency::USD)),
        real_coupon: Decimal::try_from(0.04).expect("valid coupon"),
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Thirty360,
        issue_date: issue,
        maturity,
        base_cpi: 100.0,
        base_date: issue,
        indexation_method: IndexationMethod::Tips,
        lag: InflationLag::None,
        deflation_protection: DeflationProtection::MaturityOnly,
        floor_vol_surface_id: None,
        business_day_convention: BusinessDayConvention::Following,
        stub: StubKind::None,
        calendar_id: None,
        discount_curve_id: CurveId::new("USD-OIS"),
        inflation_index_id: CurveId::new("US-CPI"),
        instrument_pricing_overrides:
            finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides::default()
                .with_quoted_clean_price_pct(100.0),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
    };
    let duration = bond.real_duration(as_of, 1.0).expect("real duration");
    let y0 = bond.real_yield(100.0, as_of).expect("real yield");
    let schedule = bond.real_cashflow_schedule().expect("real schedule");
    let flows = bond
        .real_flow_times(&schedule, as_of)
        .expect("real yield times");
    let price_from_yield = |y: f64| -> Result<f64> {
        let price = price_from_ytm_timed(&flows, bond.frequency, y, YieldCompounding::Street)?;
        Ok(price / bond.notional.amount() * 100.0)
    };
    let bp = 1e-4;
    let p_up = price_from_yield(y0 + bp).expect("price up");
    let p_dn = price_from_yield(y0 - bp).expect("price down");
    let p_dirty = price_from_yield(y0).expect("dirty model price");
    let dp_dy = (p_up - p_dn) / (2.0 * bp);
    let expected_dirty_duration = -(dp_dy / p_dirty);
    let clean_denominator_duration = -(dp_dy / 100.0);

    assert!(
        (p_dirty - 100.0).abs() > 0.05,
        "test must be mid-period with non-trivial accrued dirty/clean gap, dirty={p_dirty}"
    );
    assert!(
        (duration - expected_dirty_duration).abs() < 1e-10,
        "real duration must use dirty model price denominator: got {duration}, expected {expected_dirty_duration}"
    );
    assert!(
        (duration - clean_denominator_duration).abs() > 1e-4,
        "test must distinguish dirty denominator from clean denominator"
    );
}

#[test]
fn act365l_real_yield_and_duration_retain_coupon_periods() {
    use crate::bond::pricing::quote_conversions::{YieldCompounding, price_from_ytm_timed};

    let notional = 1_000_000.0;
    let yield_rate: f64 = 0.031;
    let fixtures = [
        (
            Tenor::annual(),
            d(2023, Month::March, 1),
            d(2023, Month::June, 1),
            d(2025, Month::March, 1),
            92.0 / 366.0,
            vec![
                (d(2024, Month::March, 1), 274.0 / 366.0, 20_000.0),
                (
                    d(2025, Month::March, 3),
                    274.0 / 366.0 + 1.0,
                    notional + 20_000.0,
                ),
            ],
        ),
        (
            Tenor::semi_annual(),
            d(2023, Month::October, 15),
            d(2023, Month::December, 15),
            d(2025, Month::April, 15),
            61.0 / 366.0,
            vec![
                (d(2024, Month::April, 15), 122.0 / 366.0, 10_000.0),
                (d(2024, Month::October, 15), 305.0 / 366.0, 10_000.0),
                (
                    d(2025, Month::April, 15),
                    305.0 / 366.0 + 182.0 / 365.0,
                    notional + notional * 0.02 * 182.0 / 365.0,
                ),
            ],
        ),
    ];
    for (frequency, issue, as_of, maturity, accrued_fraction, expected) in fixtures {
        let mut bond = sample_bond(DeflationProtection::None);
        bond.day_count = DayCount::Act365L;
        bond.frequency = frequency;
        bond.issue_date = issue;
        bond.maturity = maturity;
        let m = if frequency == Tenor::annual() {
            1.0
        } else {
            2.0
        };
        let base = 1.0 + yield_rate / m;
        let dirty_price: f64 = expected
            .iter()
            .map(|&(_, t, amount)| amount * base.powf(-m * t))
            .sum();
        let accrued = notional * 0.02 * accrued_fraction;
        let clean_price = (dirty_price - accrued) / notional * 100.0;
        bond.instrument_pricing_overrides =
            finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides::default()
                .with_quoted_clean_price_pct(clean_price);

        let schedule = bond
            .real_cashflow_schedule()
            .expect("real ACT/365L schedule");
        let timed = bond
            .real_flow_times(&schedule, as_of)
            .expect("real yield times");
        // Principal and final coupon share the adjusted payment date but
        // remain on the contractual maturity clock; combine for comparison.
        for &(payment, t, amount) in &expected {
            let matching: Vec<_> = timed
                .iter()
                .filter(|&&(date, _, _)| date == payment)
                .collect();
            assert!(!matching.is_empty());
            assert!(
                matching
                    .iter()
                    .all(|&&(_, actual_t, _)| (actual_t - t).abs() < 1e-12)
            );
            let actual_amount: f64 = matching.iter().map(|&&(_, _, cash)| cash.amount()).sum();
            assert!((actual_amount - amount).abs() < 1e-7);
        }
        let solved = bond.real_yield(clean_price, as_of).expect("real yield");
        assert!((solved - yield_rate).abs() < 1e-11);
        let repriced = price_from_ytm_timed(&timed, frequency, solved, YieldCompounding::Street)
            .expect("real yield price");
        assert!((repriced - dirty_price).abs() < 1e-6);
        let duration = bond.real_duration(as_of, 1.0).expect("real duration");
        let expected_duration: f64 = expected
            .iter()
            .map(|&(_, t, amount)| amount * t * base.powf(-m * t - 1.0))
            .sum::<f64>()
            / dirty_price;
        assert!((duration - expected_duration).abs() < 1e-7);
    }
}

fn d(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).expect("valid date")
}

fn market_with_deflation(as_of: Date) -> MarketContext {
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (3.0, 1.0)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve should build");
    let inflation = InflationCurve::builder("US-CPI")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Flat)
        .knots([(0.0, 100.0), (1.0, 95.0), (2.0, 90.0)])
        .build()
        .expect("inflation curve should build");
    MarketContext::new()
        .try_insert(discount)
        .expect("insert market curve")
        .try_insert(inflation)
        .expect("insert market curve")
        .insert_inflation_index(simple_index("US-CPI", as_of, 100.0, Currency::USD))
        .insert_surface(
            finstack_quant_core::market_data::surfaces::VolSurface::builder("CPI-ZERO-VOL")
                .expiries(&[0.1, 40.0])
                .secondary_values(&[0.5, 1.5])
                .secondary_axis(finstack_quant_core::market_data::surfaces::VolSurfaceAxis::Strike)
                .quote_type(
                    finstack_quant_core::market_data::surfaces::VolQuoteType::BlackLognormal,
                )
                .interpolation_mode(
                    finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol,
                )
                .row(&[0.0, 0.0])
                .row(&[0.0, 0.0])
                .build()
                .expect("zero-vol floor fixture"),
        )
}

fn sample_bond(deflation_protection: DeflationProtection) -> InflationLinkedBond {
    InflationLinkedBond {
        id: InstrumentId::new("ILB"),
        notional: Money::from((1_000_000_i64, Currency::USD)),
        real_coupon: Decimal::try_from(0.02).expect("valid coupon"),
        frequency: Tenor::annual(),
        day_count: DayCount::Thirty360,
        issue_date: d(2024, Month::January, 15),
        maturity: d(2026, Month::January, 15),
        base_cpi: 100.0,
        base_date: d(2024, Month::January, 15),
        indexation_method: IndexationMethod::Tips,
        lag: InflationLag::None,
        deflation_protection,
        floor_vol_surface_id: Some("CPI-ZERO-VOL".into()),
        business_day_convention: BusinessDayConvention::Following,
        stub: StubKind::None,
        calendar_id: None,
        discount_curve_id: CurveId::new("USD-OIS"),
        inflation_index_id: CurveId::new("US-CPI"),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
    }
}

fn real_accrued(bond: &InflationLinkedBond, as_of: Date) -> Result<f64> {
    bond.real_accrued_from(&bond.real_cashflow_schedule()?, as_of)
}

#[test]
fn accrued_real_interest_matches_hand_calculation() {
    // 30/360 annual 2% on 1,000,000: coupon 20,000; 2024-01-15 → 2024-07-15
    // is 180/360 of the period, so accrued = 10,000.
    let bond = sample_bond(DeflationProtection::None);
    let accrued = real_accrued(&bond, d(2024, Month::July, 15)).expect("accrued");
    assert!(
        (accrued - 10_000.0).abs() < 1e-6,
        "30/360 accrued {accrued}"
    );

    // ACT/ACT ICMA semi-annual 2%: coupon 10,000 over 2024-01-15 → 07-15
    // (182 days); 2024-03-01 is 46 days in, so 10,000 × 46/182.
    let mut icma = sample_bond(DeflationProtection::None);
    icma.frequency = Tenor::semi_annual();
    icma.day_count = DayCount::ActActIsma;
    icma.maturity = d(2029, Month::January, 15);
    let accrued = real_accrued(&icma, d(2024, Month::March, 1)).expect("accrued");
    let expected = 10_000.0 * 46.0 / 182.0;
    assert!(
        (accrued - expected).abs() < 1e-6,
        "ICMA accrued {accrued} vs {expected}"
    );

    // Before issue and on/after maturity nothing accrues.
    assert_eq!(
        real_accrued(&icma, d(2023, Month::December, 1)).expect("accrued"),
        0.0
    );
    assert_eq!(
        real_accrued(&icma, d(2029, Month::January, 15)).expect("accrued"),
        0.0
    );
}

#[test]
fn hybrid_ref_cpi_resolves_published_and_projected_anchors_independently() {
    let mut bond = sample_bond(DeflationProtection::None);
    bond.lag = InflationLag::months(3);
    let index = InflationIndex::new(
        "US-CPI",
        vec![
            (d(2025, Month::January, 1), 100.0),
            (d(2025, Month::February, 1), 105.0),
            (d(2025, Month::March, 1), 110.0),
        ]
        .into_iter()
        .map(|(reference_month, value)| InflationObservation {
            reference_month,
            value,
            available_on: d(2025, Month::March, 1),
        })
        .collect(),
        Currency::USD,
        d(2025, Month::March, 1),
    )
    .expect("published CPI")
    .with_expected_releases(vec![InflationExpectedRelease {
        reference_month: d(2025, Month::April, 1),
        expected_release: d(2025, Month::May, 15),
        known_on: d(2025, Month::March, 1),
    }])
    .unwrap();
    let curve = InflationCurve::builder("US-CPI")
        .base_date(d(2025, Month::March, 1))
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Flat)
        .knots([(0.0, 110.0), (31.0 / 365.0, 120.0)])
        .build()
        .expect("projected CPI");
    let market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve")
        .insert_inflation_index(index);

    let ratio = bond
        .index_ratio_from_market(d(2025, Month::June, 15), &market, d(2025, Month::March, 1))
        .expect("hybrid RefCPI");
    let expected = (110.0 + (14.0 / 30.0) * (120.0 - 110.0)) / bond.base_cpi;
    assert!((ratio - expected).abs() < 1e-12);
}

/// Principal on a weekend maturity (Sunday 2034-01-15) is paid on the
/// Following business day, 2034-01-16,
/// in both the PV schedule and the real-yield schedule, together with the
/// final coupon.
#[test]
fn principal_paid_on_adjusted_maturity_in_pv_and_real_schedules() {
    let as_of = d(2024, Month::January, 15);
    let mut bond = sample_bond(DeflationProtection::None);
    bond.issue_date = d(2024, Month::January, 15);
    bond.maturity = d(2034, Month::January, 15); // Sunday
    let inflation = InflationCurve::builder("US-CPI")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Flat)
        .knots([(0.0, 100.0), (12.0, 100.0)])
        .build()
        .expect("inflation curve");
    let market = MarketContext::new()
        .try_insert(inflation)
        .expect("insert market curve")
        .insert_inflation_index(simple_index("US-CPI", as_of, 100.0, Currency::USD));

    let adjusted = d(2034, Month::January, 16);
    let schedule = bond.cashflow_schedule(&market, as_of).expect("pv schedule");
    let principal_dates: Vec<Date> = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.get_kind() == CFKind::Notional)
        .map(|cf| cf.get_date())
        .collect();
    assert_eq!(principal_dates, vec![adjusted]);
    assert_ne!(principal_dates[0], bond.maturity);

    let real = bond.real_cashflow_schedule().expect("real schedule");
    assert_eq!(
        real.get_flows().last().map(|flow| flow.get_date()),
        Some(adjusted)
    );
}

#[test]
fn maturity_only_deflation_floor_applies_only_to_principal() {
    let as_of = d(2024, Month::January, 15);
    let market = market_with_deflation(as_of);
    let bond = sample_bond(DeflationProtection::MaturityOnly);

    let schedule = bond
        .cashflow_schedule(&market, as_of)
        .expect("schedule should build");
    assert!(schedule.get_flows().len() >= 3);

    let first_coupon = schedule
        .get_flows()
        .iter()
        .find(|flow| flow.get_kind() == CFKind::InflationCoupon)
        .expect("coupon flow")
        .get_amount()
        .amount();
    let principal = schedule
        .get_flows()
        .iter()
        .find(|flow| flow.get_date() == bond.maturity && flow.get_kind() == CFKind::Notional)
        .expect("principal flow")
        .get_amount()
        .amount();

    assert!(
        first_coupon < 20_000.0,
        "coupon should still deflate under maturity-only floor"
    );
    assert_eq!(principal, 1_000_000.0);
}

#[test]
fn all_payments_deflation_floor_applies_to_coupons_and_principal() {
    let as_of = d(2024, Month::January, 15);
    let market = market_with_deflation(as_of);
    let bond = sample_bond(DeflationProtection::AllPayments);

    let schedule = bond
        .cashflow_schedule(&market, as_of)
        .expect("schedule should build");
    assert!(schedule.get_flows().len() >= 3);

    let first_coupon = schedule
        .get_flows()
        .iter()
        .find(|flow| flow.get_kind() == CFKind::InflationCoupon)
        .expect("coupon flow")
        .get_amount()
        .amount();
    let principal = schedule
        .get_flows()
        .iter()
        .find(|flow| flow.get_date() == bond.maturity && flow.get_kind() == CFKind::Notional)
        .expect("principal flow")
        .get_amount()
        .amount();

    assert_eq!(first_coupon, 20_000.0);
    assert_eq!(principal, 1_000_000.0);
}

#[test]
fn cashflow_schedule_marks_inflation_coupons_explicitly() {
    let as_of = d(2024, Month::January, 15);
    let market = market_with_deflation(as_of);
    let bond = sample_bond(DeflationProtection::AllPayments);

    let schedule = bond
        .cashflow_schedule(&market, as_of)
        .expect("full schedule should build");

    assert!(
        schedule
            .get_flows()
            .iter()
            .any(|flow| flow.get_kind() == CFKind::InflationCoupon),
        "expected inflation-linked coupons in full schedule"
    );
    assert!(
        schedule
            .get_flows()
            .iter()
            .any(|flow| flow.get_date() == bond.maturity && flow.get_kind() == CFKind::Notional),
        "expected principal notional flow at maturity"
    );
}

/// UK legacy 8-month step lag reads the whole-month print: for a date of
/// 2025-06-15 the reference month is October 2024, i.e. the curve at
/// 2024-10-01 = 100 (a knot), giving ratio 100/100 = 1.0. Reading the
/// curve on 2024-10-15 would interpolate toward the November value 110.
#[test]
fn uk_step_lag_reads_first_of_month_from_curve_and_hybrid() {
    use finstack_quant_core::market_data::scalars::InflationIndex;

    let mut bond = sample_bond(DeflationProtection::None);
    bond.indexation_method = IndexationMethod::Uk;
    bond.lag = InflationLag::months(8);
    let curve = InflationCurve::builder("US-CPI")
        .base_date(d(2024, Month::October, 1))
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Flat)
        .knots([(0.0, 100.0), (31.0 / 365.0, 110.0)])
        .build()
        .expect("curve");
    let date = d(2025, Month::June, 15);

    let mid_month = curve
        .cpi_on_date(d(2024, Month::October, 15))
        .expect("mid-month cpi");
    assert!((mid_month - 100.0).abs() > 1.0, "fixture must discriminate");

    // Hybrid: prints published through September 2024, so October comes
    // from the projection curve on its first-of-month anchor.
    let index = InflationIndex::new(
        "US-CPI",
        vec![
            (d(2024, Month::August, 1), 98.0),
            (d(2024, Month::September, 1), 99.0),
        ]
        .into_iter()
        .map(|(reference_month, value)| InflationObservation {
            reference_month,
            value,
            available_on: d(2024, Month::September, 1),
        })
        .collect(),
        Currency::USD,
        d(2024, Month::September, 1),
    )
    .expect("published CPI")
    .with_expected_releases(vec![InflationExpectedRelease {
        reference_month: d(2024, Month::October, 1),
        expected_release: d(2024, Month::November, 15),
        known_on: d(2024, Month::September, 1),
    }])
    .unwrap();
    let market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve")
        .insert_inflation_index(index);
    let hybrid = bond
        .index_ratio_from_market(date, &market, d(2024, Month::September, 1))
        .expect("hybrid ratio");
    assert!((hybrid - 1.0).abs() < 1e-12, "hybrid ratio {hybrid}");
}

/// Regression test for the UK gilt lag validation that was previously
/// `debug_assert!`-only (a release-build silent miscalc). A non-standard
/// lag (anything other than 3 or 8 months) must now fail explicitly when
/// `index_ratio` is called.
#[test]
fn uk_gilt_non_standard_lag_returns_err() {
    let as_of = d(2024, Month::January, 15);
    let mut bond = sample_bond(DeflationProtection::AllPayments);
    bond.indexation_method = IndexationMethod::Uk;
    bond.lag = InflationLag::months(5); // non-standard

    let index = InflationIndex::new(
        "UK-RPI",
        vec![
            (d(2023, Month::January, 1), 100.0),
            (d(2024, Month::January, 1), 102.0),
        ]
        .into_iter()
        .map(|(reference_month, value)| InflationObservation {
            reference_month,
            value,
            available_on: as_of,
        })
        .collect(),
        Currency::USD,
        as_of,
    )
    .expect("index builds");

    let result = bond.index_ratio(as_of, &index);
    let err = result.expect_err("non-standard UK lag must fail at runtime");
    assert!(
        err.to_string().contains("UK gilt indexation"),
        "error message should identify the issue: {err}"
    );
}

#[test]
fn uk_gilt_standard_lags_pass_validation() {
    let as_of = d(2024, Month::January, 15);
    let mut bond = sample_bond(DeflationProtection::AllPayments);
    bond.indexation_method = IndexationMethod::Uk;
    bond.lag = InflationLag::months(3); // modern UK standard

    // Use a Linear-interpolated index to satisfy the modern UK invariant.
    let index = InflationIndex::new(
        "US-CPI",
        vec![
            (d(2023, Month::October, 1), 100.0),
            (d(2023, Month::November, 1), 102.0),
        ]
        .into_iter()
        .map(|(reference_month, value)| InflationObservation {
            reference_month,
            value,
            available_on: as_of,
        })
        .collect(),
        Currency::USD,
        as_of,
    )
    .expect("index builds");

    // Modern (3-month) lag must succeed; we don't care about the exact ratio,
    // just that the validation didn't reject the bond.
    bond.index_ratio(as_of, &index)
        .expect("3-month UK gilt lag is valid");
}
#[test]
fn protected_cpi_ratio_matches_independent_stochastic_density_grid() {
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use time::macros::date;
    let as_of = date!(2025 - 01 - 01);
    let payment = date!(2026 - 01 - 01);
    let mut bond = InflationLinkedBond::example().unwrap();
    bond.base_cpi = 100.0;
    bond.lag = InflationLag::None;
    bond.floor_vol_surface_id = Some("RATIO-VOL".into());
    let budget = 1e-5; // 0.1 bp of principal for the expected ratio.
    for forward in [0.8_f64, 1.0, 1.2] {
        for sigma in [0.05_f64, 0.2, 0.5] {
            let market = MarketContext::new()
                .try_insert(
                    InflationCurve::builder("US-CPI")
                        .base_date(as_of)
                        .day_count(DayCount::Act365F)
                        .interp(InterpStyle::LogLinear)
                        .extrapolation(ExtrapolationPolicy::Flat)
                        .knots([(0.0, 100.0 * forward), (2.0, 100.0 * forward)])
                        .build()
                        .unwrap(),
                ).expect("insert market curve")
                .insert_inflation_index(simple_index("US-CPI", as_of, 100.0, Currency::USD))
                .insert_surface(
                    VolSurface::builder("RATIO-VOL")
                        .expiries(&[1.0])
                        .secondary_values(&[1.0])
                        .secondary_axis(
                            finstack_quant_core::market_data::surfaces::VolSurfaceAxis::Strike,
                        )
                        .quote_type(
                            finstack_quant_core::market_data::surfaces::VolQuoteType::BlackLognormal,
                        )
                        .interpolation_mode(
                            finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol,
                        )
                        .row(&[sigma])
                        .build()
                        .unwrap(),
                );
            let actual = bond
                .payment_index_ratio(payment, &market, as_of, true)
                .unwrap();
            let split = (-forward.ln() + 0.5 * sigma * sigma) / sigma;
            let mut references = Vec::new();
            for steps in [256, 512, 1024] {
                let mut expected = 0.0;
                for (left, right) in [
                    (-10.0, split.clamp(-10.0, 10.0)),
                    (split.clamp(-10.0, 10.0), 10.0),
                ] {
                    let h = (right - left) / steps as f64;
                    for i in 0..=steps {
                        let z = left + i as f64 * h;
                        let ratio = forward * (sigma * z - 0.5 * sigma * sigma).exp();
                        let weight = if i == 0 || i == steps {
                            1.0
                        } else if i % 2 == 0 {
                            2.0
                        } else {
                            4.0
                        };
                        expected += h / 3.0 * weight * ratio.max(1.0) * (-0.5 * z * z).exp()
                            / std::f64::consts::TAU.sqrt();
                    }
                }
                assert!(
                    (actual - expected).abs() < budget,
                    "F={forward}, sigma={sigma}, grid={steps}, ratio={actual}, density={expected}"
                );
                references.push(expected);
            }
            assert!(
                references
                    .windows(2)
                    .all(|pair| (pair[0] - pair[1]).abs() < budget)
            );
        }
    }
}

#[test]
fn serde_rejects_invalid_ilb_invariants() {
    let mut val =
        serde_json::to_value(InflationLinkedBond::example().expect("example")).expect("serialize");
    val["base_cpi"] = serde_json::json!(-10.0);
    assert!(serde_json::from_value::<InflationLinkedBond>(val).is_err());
}

#[test]
fn reference_cpi_uses_contractual_unadjusted_dates_when_bdc_shifts_payment() {
    use finstack_quant_cashflows::CashflowScheduleSource;
    // 2027-01-16 is Saturday -> Following shifts payment_date to Monday 2027-01-18,
    // while Reference CPI lookup remains anchored to contractual unadjusted dates.
    let issue = d(2025, Month::January, 16);
    let maturity = d(2027, Month::January, 16);

    let mut unadj = InflationLinkedBond::example().expect("example");
    unadj.issue_date = issue;
    unadj.maturity = maturity;
    unadj.base_date = issue;
    unadj.base_cpi = 300.0;
    unadj.inflation_index_id = CurveId::new("US-CPI");
    unadj.lag = InflationLag::None;
    unadj.deflation_protection = DeflationProtection::None;
    unadj.floor_vol_surface_id = None;
    unadj.frequency = Tenor::annual();
    unadj.business_day_convention = BusinessDayConvention::Unadjusted;
    unadj.calendar_id = None;

    let mut following = unadj.clone();
    following.business_day_convention = BusinessDayConvention::Following;

    let curve = InflationCurve::builder("US-CPI")
        .base_date(issue)
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Flat)
        .knots([(0.0, 300.0), (1.0, 306.0), (2.0, 312.0), (3.0, 324.0)])
        .build()
        .expect("curve");
    let market = MarketContext::new()
        .try_insert(curve)
        .expect("market")
        .insert_inflation_index(simple_index("US-CPI", issue, 300.0, Currency::USD));

    let sched_unadj = unadj
        .raw_cashflow_schedule(&market, issue)
        .expect("unadjusted schedule");
    let sched_fol = following
        .raw_cashflow_schedule(&market, issue)
        .expect("following schedule");

    let unadj_last = sched_unadj.get_flows().last().expect("unadj last");
    let fol_last = sched_fol.get_flows().last().expect("fol last");
    assert_eq!(unadj_last.get_date(), d(2027, Month::January, 16));
    assert_eq!(fol_last.get_date(), d(2027, Month::January, 18));
    assert!(
        (unadj_last.get_amount().amount() - fol_last.get_amount().amount()).abs() < 1e-9,
        "principal indexed amount should be invariant to BDC shift: unadj={}, fol={}",
        unadj_last.get_amount().amount(),
        fol_last.get_amount().amount()
    );
}
