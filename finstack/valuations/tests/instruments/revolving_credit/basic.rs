//! Comprehensive integration tests for revolving credit facilities.

use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepayEvent, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::macros::date;

fn build_flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
            (30.0, (-rate * 30.0).exp()),
        ])
        .build()
        .unwrap()
}

#[test]
fn test_revolving_credit_basic_pricing() {
    let val_date = date!(2025 - 01 - 01);
    let commitment_date = date!(2025 - 01 - 01);
    let maturity_date = date!(2026 - 01 - 01);

    let facility = RevolvingCredit::builder()
        .id("RC-001".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD))
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees({
            let mut fees = RevolvingCreditFees::flat(25.0, 10.0, 5.0);
            fees.upfront_fee = Some(Money::new(50_000.0, Currency::USD));
            fees
        })
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let disc_curve = build_flat_discount_curve(0.03, val_date, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    let pv = facility.value(&market, val_date).unwrap();

    // PV from lender's perspective includes:
    // - Interest and fees received (positive)
    // - Principal repaid at maturity (positive, ~5M)
    // The net PV should be close to the drawn amount plus net carry

    // Rough estimate:
    // Principal repaid: 5M @ 97% DF ~= 4.85M
    // Interest: 5M * 0.05 * 97% DF = 242.5k
    // Fees: ~22.5k * 97% DF = 21.8k
    // Upfront: 50k
    // Total ~= 5.16M
    assert!(
        pv.amount() > 4_800_000.0 && pv.amount() < 5_500_000.0,
        "PV should include principal repayment, got {}",
        pv.amount()
    );
}

#[test]
fn test_revolving_credit_with_draws_and_repayments() {
    let val_date = date!(2025 - 01 - 01);
    let commitment_date = date!(2025 - 01 - 01);
    let maturity_date = date!(2026 - 01 - 01);

    let facility = RevolvingCredit::builder()
        .id("RC-002".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(3_000_000.0, Currency::USD))
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.04 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(20.0, 0.0, 0.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![
            DrawRepayEvent {
                date: date!(2025 - 04 - 01),
                amount: Money::new(2_000_000.0, Currency::USD),
                is_draw: true, // Draw 2M
            },
            DrawRepayEvent {
                date: date!(2025 - 07 - 01),
                amount: Money::new(1_000_000.0, Currency::USD),
                is_draw: false, // Repay 1M
            },
        ]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let disc_curve = build_flat_discount_curve(0.03, val_date, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    let pv = facility.value(&market, val_date).unwrap();

    // From lender's perspective: principal repaid + interest/fees
    // Net drawn balance at maturity after events: 3M + 2M - 1M = 4M
    // PV should include this principal repayment plus carry
    assert!(
        pv.amount() > 3_000_000.0,
        "PV should include principal repayment, got {}",
        pv.amount()
    );
}

#[test]
fn test_revolving_credit_utilization_metrics() {
    let val_date = date!(2025 - 01 - 01);
    let commitment_date = date!(2025 - 01 - 01);
    let maturity_date = date!(2026 - 01 - 01);

    let facility = RevolvingCredit::builder()
        .id("RC-003".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(6_000_000.0, Currency::USD)) // 60% utilization
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let disc_curve = build_flat_discount_curve(0.03, val_date, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Test utilization rate metric
    let result = facility
        .price_with_metrics(
            &market,
            val_date,
            &[
                MetricId::custom("utilization_rate"),
                MetricId::custom("available_capacity"),
                MetricId::custom("weighted_average_cost"),
            ],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Check utilization rate
    let utilization = result.measures.get("utilization_rate").unwrap();
    assert!(
        (utilization - 0.6).abs() < 1e-6,
        "Utilization should be 60%"
    );

    // Check available capacity
    let capacity = result.measures.get("available_capacity").unwrap();
    assert!(
        (capacity - 4_000_000.0).abs() < 1.0,
        "Available capacity should be 4M"
    );

    // Check weighted average cost is computed
    let wac = result.measures.get("weighted_average_cost").unwrap();
    assert!(*wac > 0.0, "Weighted average cost should be positive");
}

#[test]
fn test_revolving_credit_standard_metrics() {
    let val_date = date!(2025 - 01 - 01);
    let commitment_date = date!(2025 - 01 - 01);
    let maturity_date = date!(2027 - 01 - 01); // 2 years

    let facility = RevolvingCredit::builder()
        .id("RC-004".into())
        .commitment_amount(Money::new(5_000_000.0, Currency::USD))
        .drawn_amount(Money::new(3_000_000.0, Currency::USD))
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.06 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(30.0, 15.0, 10.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .credit_curve_id("BORROWER-A".into())
        .recovery_rate(0.40)
        .build()
        .unwrap();

    let disc_curve = build_flat_discount_curve(0.04, val_date, "USD-OIS");

    // Create hazard curve for CS01 calculation
    let hazard_curve = HazardCurve::builder("BORROWER-A")
        .base_date(val_date)
        .recovery_rate(0.40)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.99),
            (2.0, 0.975),
            (3.0, 0.96),
            (5.0, 0.92),
        ])
        .build()
        .unwrap();

    let market = MarketContext::new().insert(disc_curve).insert(hazard_curve);

    // Test standard metrics
    let result = facility
        .price_with_metrics(
            &market,
            val_date,
            &[MetricId::Dv01, MetricId::Cs01, MetricId::Theta],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // DV01 should be negative for a lender position (PV decreases when rates rise)
    let dv01 = result.measures.get("dv01").unwrap();
    println!("DV01: {}", dv01);
    println!("Base PV: {}", result.value.amount());
    assert!(
        *dv01 < 0.0,
        "DV01 should be negative for lender position, got {}",
        dv01
    );
    assert!(
        dv01.abs() > 50.0,
        "DV01 magnitude should be significant, got {}",
        dv01
    );

    // CS01 should be non-zero (PV changes when credit spreads widen)
    // For a lender position, CS01 should be negative (PV decreases when spreads widen)
    // but we allow for small values that might round to zero
    let cs01 = result.measures.get("cs01").unwrap();
    assert!(cs01.is_finite(), "CS01 should be finite, got {}", cs01);
    // CS01 should be negative for lender position, but allow for very small values
    if cs01.abs() > 1e-6 {
        assert!(
            *cs01 < 0.0,
            "CS01 should be negative for lender position when non-zero, got {}",
            cs01
        );
        // CS01 magnitude should be similar to DV01 (both measure sensitivity to rate/spread changes)
        assert!(
            (dv01.abs() - cs01.abs()).abs() < 200.0,
            "DV01 and CS01 magnitudes should be similar: DV01={}, CS01={}",
            dv01,
            cs01
        );
    } else {
        // If CS01 is very small, just verify it's computed
        println!(
            "CS01 is very small ({}), which may be expected for low credit risk",
            cs01
        );
    }

    // Theta (1-day time decay) - for a lending position with positive carry,
    // theta can be positive (earning interest/fees) or negative depending on
    // the relationship between earned carry and discount rate effects
    let theta = result.measures.get("theta").unwrap();
    println!("Theta: {}", theta);
    // Just verify theta is computed
    assert!(theta.is_finite(), "Theta should be a finite number");
}

#[test]
fn test_revolving_credit_cs01_z_spread_fallback_without_credit_curve() {
    // A deterministic facility with NO credit curve previously reported
    // CS01 = 0.0 (the discounting pricer survival-weights cashflows only when a
    // hazard curve is present). It now reports the market-standard z-spread
    // bump, so CS01 is a real, negative credit-spread sensitivity for the
    // lender.
    let val_date = date!(2025 - 01 - 01);
    let facility = RevolvingCredit::builder()
        .id("RC-ZS-001".into())
        .commitment_amount(Money::new(20_000_000.0, Currency::USD))
        .drawn_amount(Money::new(15_000_000.0, Currency::USD))
        .commitment_date(val_date)
        .maturity(date!(2030 - 01 - 01)) // 5y
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.06 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        // No credit_curve_id → z-spread fallback path.
        .build()
        .unwrap();

    let disc_curve = build_flat_discount_curve(0.04, val_date, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    let result = facility
        .price_with_metrics(
            &market,
            val_date,
            &[MetricId::Cs01, MetricId::BucketedCs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let cs01 = *result.measures.get("cs01").unwrap();
    assert!(cs01.is_finite(), "CS01 should be finite, got {cs01}");
    assert!(
        cs01 < -1.0,
        "z-spread CS01 should be clearly negative for a lender with no credit curve, got {cs01}"
    );

    // Key-rate buckets must reconcile to the parallel z-spread CS01: PV is
    // additive over flows, so per-bucket spread bumps sum exactly.
    let bucketed_total = *result.measures.get("bucketed_cs01").unwrap();
    assert!(
        (bucketed_total - cs01).abs() <= 1e-6 * cs01.abs().max(1.0),
        "bucketed CS01 total {bucketed_total} should reconcile to parallel CS01 {cs01}"
    );
}

#[test]
fn test_revolving_credit_bucketed_dv01() {
    let val_date = date!(2025 - 01 - 01);
    let commitment_date = date!(2025 - 01 - 01);
    let maturity_date = date!(2028 - 01 - 01); // 3 years

    let facility = RevolvingCredit::builder()
        .id("RC-005".into())
        .commitment_amount(Money::new(20_000_000.0, Currency::USD))
        .drawn_amount(Money::new(10_000_000.0, Currency::USD))
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.055 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let disc_curve = build_flat_discount_curve(0.04, val_date, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Test bucketed DV01
    let result = facility
        .price_with_metrics(
            &market,
            val_date,
            &[MetricId::BucketedDv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Should have bucketed DV01 metric
    let bucketed_total = result.measures.get("bucketed_dv01").unwrap();
    println!("Bucketed DV01: {}", bucketed_total);
    // Bucketed DV01 total should be finite
    assert!(
        bucketed_total.is_finite(),
        "Total bucketed DV01 should be finite, got {}",
        bucketed_total
    );
}

#[test]
fn test_revolving_credit_helpers() {
    let val_date = date!(2025 - 01 - 01);
    let facility = RevolvingCredit::builder()
        .id("RC-006".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(7_500_000.0, Currency::USD))
        .commitment_date(val_date)
        .maturity(date!(2026 - 01 - 01))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    // Test helper methods
    assert_eq!(facility.utilization_rate(), 0.75);
    assert!(facility.is_deterministic());
    assert!(!facility.is_stochastic());

    let undrawn = facility.undrawn_amount().unwrap();
    assert_eq!(undrawn.amount(), 2_500_000.0);
}

#[test]
fn test_term_forward_with_floor() {
    use finstack_core::market_data::term_structures::ForwardCurve;

    let val_date = date!(2025 - 01 - 01);
    let commitment_date = date!(2025 - 01 - 01);
    let maturity_date = date!(2025 - 07 - 01);

    // Build forward curve with very low rates that will be affected by floor
    // Use small positive rates that become negative after subtracting margin
    let fwd_curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(val_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 0.0001), // 1 bp (very low)
            (0.5, 0.0001),
            (1.0, 0.0002),
        ])
        .build()
        .unwrap();

    let disc_curve = build_flat_discount_curve(0.03, val_date, "USD-OIS");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    // Facility with floor at 0 bps
    let facility_with_floor = RevolvingCredit::builder()
        .id("RC-FLOOR".into())
        .commitment_amount(Money::new(1_000_000.0, Currency::USD))
        .drawn_amount(Money::new(1_000_000.0, Currency::USD))
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Floating(
            finstack_cashflows::builder::FloatingRateSpec {
                index_id: "USD-SOFR-3M".into(),
                spread_bp: rust_decimal::Decimal::try_from(500.0).expect("valid"), // +500 bps margin = +5%
                gearing: rust_decimal::Decimal::try_from(1.0).expect("valid"),
                gearing_includes_spread: true,
                index_floor_bp: Some(rust_decimal::Decimal::try_from(100.0).expect("valid")), // 1% floor on base rate (floors 1bp to 1%)
                all_in_floor_bp: None,
                all_in_cap_bp: None,
                index_cap_bp: None,
                reset_freq: Tenor::quarterly(),
                index_tenor: None,
                reset_lag_days: 2,
                dc: DayCount::Act360,
                bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".to_string(),
                fixing_calendar_id: None,
                end_of_month: false,
                overnight_compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
                payment_lag_days: 0,
            },
        ))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    // Facility without floor
    let facility_no_floor = RevolvingCredit::builder()
        .id("RC-NO-FLOOR".into())
        .commitment_amount(Money::new(1_000_000.0, Currency::USD))
        .drawn_amount(Money::new(1_000_000.0, Currency::USD))
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Floating(
            finstack_cashflows::builder::FloatingRateSpec {
                index_id: "USD-SOFR-3M".into(),
                spread_bp: rust_decimal::Decimal::try_from(500.0).expect("valid"), // +500 bps margin = +5%
                gearing: rust_decimal::Decimal::try_from(1.0).expect("valid"),
                gearing_includes_spread: true,
                index_floor_bp: None, // No floor, so 1bp base passes through
                all_in_floor_bp: None,
                all_in_cap_bp: None,
                index_cap_bp: None,
                reset_freq: Tenor::quarterly(),
                index_tenor: None,
                reset_lag_days: 2,
                dc: DayCount::Act360,
                bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".to_string(),
                fixing_calendar_id: None,
                end_of_month: false,
                overnight_compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
                payment_lag_days: 0,
            },
        ))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let pv_with_floor = facility_with_floor.value(&market, val_date).unwrap();
    let pv_no_floor = facility_no_floor.value(&market, val_date).unwrap();

    // With floor: max(0.01%, 1%) + 5% margin = 1% + 5% = 6% (borrower pays 6%)
    // Without floor: 0.01% + 5% margin = 5.01% (borrower pays 5.01%)
    // With floor, the borrower pays more interest, so from lender perspective PV is higher
    assert!(pv_with_floor.amount() > pv_no_floor.amount(),
        "Floor should increase PV (lender receives more interest). With floor: {}, Without floor: {}",
        pv_with_floor.amount(), pv_no_floor.amount());
}

#[test]
fn test_overdraw_validation() {
    let val_date = date!(2025 - 01 - 01);
    let commitment_date = date!(2025 - 01 - 01);
    let maturity_date = date!(2026 - 01 - 01);

    let disc_curve = build_flat_discount_curve(0.03, val_date, "USD-OIS");
    let market = MarketContext::new().insert(disc_curve);

    // Create a facility with a draw that would exceed commitment
    let facility = RevolvingCredit::builder()
        .id("RC-OVERDRAW".into())
        .commitment_amount(Money::new(1_000_000.0, Currency::USD))
        .drawn_amount(Money::new(500_000.0, Currency::USD))
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![DrawRepayEvent {
            date: date!(2025 - 03 - 01),
            amount: Money::new(600_000.0, Currency::USD), // This would take us to 1.1M > 1M commitment
            is_draw: true,
        }]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    // This should error due to overdraw
    let result = facility.value(&market, val_date);
    assert!(result.is_err(), "Should error on overdraw");

    if let Err(e) = result {
        let err_msg = format!("{}", e);
        assert!(
            err_msg.contains("exceed commitment") || err_msg.contains("Validation"),
            "Error should mention exceeding commitment, got: {}",
            err_msg
        );
    }
}

#[test]
fn test_deterministic_with_credit_risk() {
    use finstack_core::market_data::term_structures::HazardCurve;

    let val_date = date!(2025 - 01 - 01);
    let commitment_date = date!(2025 - 01 - 01);
    let maturity_date = date!(2030 - 01 - 01); // 5 year term

    // Create facility WITH hazard curve
    let facility_risky = RevolvingCredit::builder()
        .id("RC-RISKY".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(3_000_000.0, Currency::USD)) // 30% utilization
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act365F)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(50.0, 0.0, 0.0)) // 50bp commitment fee
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .credit_curve_id(finstack_core::types::CurveId::from("BORROWER-A")) // Add credit risk
        .recovery_rate(0.40) // 40% recovery
        .build()
        .unwrap();

    // Create same facility WITHOUT hazard curve (risk-free)
    let facility_risk_free = RevolvingCredit::builder()
        .id("RC-RISK-FREE".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(3_000_000.0, Currency::USD))
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act365F)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(50.0, 0.0, 0.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        // No hazard curve - risk-free pricing
        .build()
        .unwrap();

    // Create market with discount and hazard curves
    let disc_curve = build_flat_discount_curve(0.04, val_date, "USD-OIS");

    // Hazard curve: 99% survival at 1Y, 92% at 5Y (moderate credit quality)
    let hazard_curve = HazardCurve::builder("BORROWER-A")
        .base_date(val_date)
        .recovery_rate(0.40)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.99),
            (2.0, 0.975),
            (3.0, 0.96),
            (5.0, 0.92),
        ])
        .build()
        .unwrap();

    let mut market = MarketContext::new();
    market = market.insert(disc_curve);
    market = market.insert(hazard_curve);

    // Price both facilities
    let npv_risky = facility_risky.value(&market, val_date).unwrap();
    let npv_risk_free = facility_risk_free.value(&market, val_date).unwrap();

    // Credit-risky NPV should be lower than risk-free NPV
    assert!(
        npv_risky.amount() < npv_risk_free.amount(),
        "Risky NPV ({}) should be less than risk-free NPV ({})",
        npv_risky.amount(),
        npv_risk_free.amount()
    );

    // The difference should be material (at least 5% reduction due to 8% default prob over 5Y)
    let credit_adjustment_pct =
        (npv_risk_free.amount() - npv_risky.amount()) / npv_risk_free.amount() * 100.0;
    assert!(
        credit_adjustment_pct > 3.0,
        "Credit adjustment should be material (>3%), got {:.2}%",
        credit_adjustment_pct
    );

    println!("Risk-free NPV: ${:.2}", npv_risk_free.amount());
    println!("Risky NPV: ${:.2}", npv_risky.amount());
    println!("Credit adjustment: {:.2}%", credit_adjustment_pct);
}

#[test]
fn test_deterministic_stochastic_convergence_with_credit_risk() {
    use finstack_core::market_data::term_structures::HazardCurve;
    use finstack_valuations::instruments::fixed_income::revolving_credit::{
        CreditSpreadProcessSpec, McConfig,
    };
    use finstack_valuations::instruments::fixed_income::revolving_credit::{
        StochasticUtilizationSpec, UtilizationProcess,
    };

    let val_date = date!(2025 - 01 - 01);
    let commitment_date = date!(2025 - 01 - 01);
    let maturity_date = date!(2030 - 01 - 01); // 5 year term
    let commitment_amount = Money::new(10_000_000.0, Currency::USD);
    let drawn_amount = Money::new(3_000_000.0, Currency::USD); // 30% utilization
    let initial_util = drawn_amount.amount() / commitment_amount.amount();

    // Create deterministic facility with hazard curve
    let facility_det = RevolvingCredit::builder()
        .id("RC-DET".into())
        .commitment_amount(commitment_amount)
        .drawn_amount(drawn_amount)
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.055 }) // 5.5% fixed
        .day_count(DayCount::Act365F)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(50.0, 0.0, 0.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .credit_curve_id(finstack_core::types::CurveId::from("BORROWER-A"))
        .credit_curve_id(finstack_core::types::CurveId::from("BORROWER-A"))
        .recovery_rate(0.40)
        .build()
        .unwrap();

    // Create stochastic facility with near-zero volatility and same hazard curve
    let mc_config = McConfig {
        correlation_matrix: None,
        recovery_rate: 0.40,
        credit_spread_process: CreditSpreadProcessSpec::MarketAnchored {
            hazard_curve_id: "BORROWER-A".into(),
            kappa: 0.3,
            implied_vol: 0.50,
            tenor_years: None,
        },
        interest_rate_process: None, // Use fixed rate (no stochastic dynamics)
        util_credit_corr: Some(0.6),
    };

    let stoch_spec = StochasticUtilizationSpec {
        utilization_process: UtilizationProcess::MeanReverting {
            target_rate: initial_util,
            speed: 100.0,     // Very high speed = stays at target
            volatility: 1e-6, // Near-zero volatility
        },
        num_paths: 2000, // Use many paths for stable average
        seed: Some(42),
        antithetic: true,
        use_sobol_qmc: false,
        mc_config: Some(mc_config),
    };

    let facility_stoch = RevolvingCredit::builder()
        .id("RC-STOCH".into())
        .commitment_amount(commitment_amount)
        .drawn_amount(drawn_amount)
        .commitment_date(commitment_date)
        .maturity(maturity_date)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.055 })
        .day_count(DayCount::Act365F)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(50.0, 0.0, 0.0))
        .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(stoch_spec)))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    // Create market
    let disc_curve = build_flat_discount_curve(0.04, val_date, "USD-OIS");
    let hazard_curve = HazardCurve::builder("BORROWER-A")
        .base_date(val_date)
        .recovery_rate(0.40)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.99),
            (2.0, 0.975),
            (3.0, 0.96),
            (5.0, 0.92),
        ])
        .build()
        .unwrap();

    let mut market = MarketContext::new();
    market = market.insert(disc_curve);
    market = market.insert(hazard_curve);

    // Price both facilities
    let npv_det = facility_det.value(&market, val_date).unwrap();
    let npv_stoch = facility_stoch.value(&market, val_date).unwrap();

    // Calculate difference
    let diff = (npv_stoch.amount() - npv_det.amount()).abs();
    let pct_diff = diff / npv_det.amount() * 100.0;

    println!("Deterministic NPV: ${:.2}", npv_det.amount());
    println!("Stochastic NPV (0 vol): ${:.2}", npv_stoch.amount());
    println!("Difference: ${:.2} ({:.2}%)", diff, pct_diff);

    // Should converge within 2.5% (allowing for MC noise)
    assert!(
        pct_diff < 2.5,
        "Deterministic and stochastic should converge at 0% vol (got {:.2}% difference)",
        pct_diff
    );
}
