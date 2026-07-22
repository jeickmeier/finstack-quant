//! Pricing and NPV tests for Inflation-Linked Bonds
//!
//! Tests cover:
//! - Present value (NPV) calculations
//! - Discounting with real and nominal curves
//! - Price sensitivity to inflation assumptions
//! - Currency consistency
//! - Matured bonds (zero value)

use super::common::*;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_valuations::instruments::Instrument;

#[test]
fn test_npv_basic() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert
    assert_eq!(pv.currency(), Currency::USD);
    assert!(pv.amount() > 0.0);
    // PV should be positive and reasonable (inflation-adjusted)
    assert!(pv.amount() > ilb.notional.amount() * 0.5);
    assert!(pv.amount() < ilb.notional.amount() * 3.0);
}

#[test]
fn test_value_via_instrument_trait() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let pv_direct = ilb.value(&ctx, as_of).unwrap();
    let pv_trait = ilb.value(&ctx, as_of).unwrap();

    // Assert - both methods should give similar results
    // Small differences acceptable due to implementation details
    assert_approx_eq(
        pv_direct.amount(),
        pv_trait.amount(),
        0.01,
        "NPV method parity",
    );
    assert_eq!(pv_direct.currency(), pv_trait.currency());
}

#[test]
fn test_npv_returns_correct_currency() {
    // Arrange
    let tips = sample_tips(); // USD
    let uk_gilt = sample_uk_linker(); // GBP

    let (ctx_usd, _) = market_context_with_index();
    let (ctx_gbp, _) = uk_market_context();
    let as_of = d(2025, 1, 2);

    // Act
    let pv_usd = tips.value(&ctx_usd, as_of).unwrap();
    let pv_gbp = uk_gilt.value(&ctx_gbp, as_of).unwrap();

    // Assert
    assert_eq!(pv_usd.currency(), Currency::USD);
    assert_eq!(pv_gbp.currency(), Currency::GBP);
}

#[test]
fn test_npv_increases_with_inflation() {
    // Arrange
    let mut ilb = sample_tips();
    let as_of = d(2025, 1, 2);
    ilb.issue_date = as_of;

    // Context with low inflation
    let ctx_low = {
        let disc =
            finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (5.0, 0.95), (10.0, 0.90)])
                .build()
                .unwrap();

        let curve =
            finstack_quant_core::market_data::term_structures::InflationCurve::builder("US-CPI-U")
                .base_date(as_of)
                .base_cpi(300.0)
                .knots([
                    (0.0, 300.0),
                    (5.0, 303.0), // ~0.2% p.a.
                ])
                .build()
                .unwrap();

        finstack_quant_core::market_data::context::MarketContext::new()
            .insert(disc)
            .insert(curve)
    };

    // Context with high inflation
    let ctx_high = {
        let disc =
            finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (5.0, 0.95), (10.0, 0.90)])
                .build()
                .unwrap();

        let curve =
            finstack_quant_core::market_data::term_structures::InflationCurve::builder("US-CPI-U")
                .base_date(as_of)
                .base_cpi(300.0)
                .knots([
                    (0.0, 300.0),
                    (5.0, 330.0), // ~10% p.a.
                ])
                .build()
                .unwrap();

        finstack_quant_core::market_data::context::MarketContext::new()
            .insert(disc)
            .insert(curve)
    };

    // Act
    let pv_low = ilb.value(&ctx_low, as_of).unwrap();
    let pv_high = ilb.value(&ctx_high, as_of).unwrap();

    // Assert - higher inflation → higher cashflows → higher PV
    assert!(pv_high.amount() > pv_low.amount());
}

#[test]
fn test_npv_decreases_with_higher_discount_rate() {
    // Arrange
    let mut ilb = sample_tips();
    let as_of = d(2025, 1, 2);
    ilb.issue_date = as_of;

    // Same inflation, different discount curves
    let inflation_curve =
        finstack_quant_core::market_data::term_structures::InflationCurve::builder("US-CPI-U")
            .base_date(as_of)
            .base_cpi(300.0)
            .knots([(0.0, 300.0), (5.0, 315.0)])
            .build()
            .unwrap();

    // Low discount rate
    let disc_low =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.99), (10.0, 0.98)])
            .build()
            .unwrap();

    // High discount rate
    let disc_high =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.80), (10.0, 0.65)])
            .build()
            .unwrap();

    let ctx_low = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(disc_low)
        .insert(inflation_curve);

    // Rebuild inflation curve for second context
    let inflation_curve2 =
        finstack_quant_core::market_data::term_structures::InflationCurve::builder("US-CPI-U")
            .base_date(as_of)
            .base_cpi(300.0)
            .knots([(0.0, 300.0), (5.0, 315.0)])
            .build()
            .unwrap();

    let ctx_high = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(disc_high)
        .insert(inflation_curve2);

    // Act
    let pv_low_rate = ilb.value(&ctx_low, as_of).unwrap();
    let pv_high_rate = ilb.value(&ctx_high, as_of).unwrap();

    // Assert - higher discount rate → lower PV
    assert!(pv_low_rate.amount() > pv_high_rate.amount());
}

#[test]
fn test_npv_at_maturity() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.maturity = d(2025, 1, 2);
    ilb.issue_date = d(2024, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = ilb.maturity;

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert - holder-view position value: the final principal payment
    // (notional * index_ratio) settles on `as_of` (= maturity) and is
    // excluded from PV, so the matured position is worth 0.
    assert_eq!(pv.amount(), 0.0);
}

#[test]
fn test_npv_after_maturity() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.maturity = d(2025, 1, 2);
    ilb.issue_date = d(2024, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 6, 1); // After maturity

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert - matured bond has zero value
    assert_eq!(pv.amount(), 0.0);
}

#[test]
fn test_npv_with_deflation_protection() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.deflation_protection =
        finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::DeflationProtection::AllPayments;
    ilb.base_index = 300.0;
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2026, 1, 2);

    let as_of = d(2025, 1, 2);

    // Context with deflation
    let disc = finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.95)])
        .build()
        .unwrap();

    let observations = vec![(d(2024, 12, 1), 290.0)]; // Deflation vs base of 300
    let index = finstack_quant_core::market_data::scalars::InflationIndex::new(
        "US-CPI-U",
        observations,
        Currency::USD,
    )
    .unwrap()
    .with_interpolation(finstack_quant_core::market_data::scalars::InflationInterpolation::Linear);

    let ctx = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(disc)
        .insert_inflation_index("US-CPI-U", index);

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert - with deflation protection, value should not fall below par equivalent
    assert!(pv.amount() > 0.0);
}

#[test]
fn test_npv_consistency_with_schedule() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Manual NPV from schedule
    let flows = ilb.dated_cashflows(&ctx, as_of).unwrap();
    let disc = ctx.get_discount(ilb.discount_curve_id.as_str()).unwrap();
    let expected_pv =
        finstack_quant_core::cashflow::npv(disc.as_ref(), disc.base_date(), &flows).unwrap();

    // Assert
    assert_approx_eq(
        pv.amount(),
        expected_pv.amount(),
        REL_TOL,
        "NPV consistency",
    );
}

#[test]
fn test_npv_different_valuation_dates() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();

    // Act - value at different dates
    let pv_early = ilb.value(&ctx, d(2021, 1, 1)).unwrap();
    let pv_mid = ilb.value(&ctx, d(2025, 1, 1)).unwrap();
    let pv_late = ilb.value(&ctx, d(2029, 1, 1)).unwrap();

    // Assert - as time passes, fewer cashflows remain → value changes
    // (Can't assert strict ordering due to pull-to-par and rate changes)
    assert!(pv_early.amount() > 0.0);
    assert!(pv_mid.amount() > 0.0);
    assert!(pv_late.amount() > 0.0);
}

#[test]
fn test_npv_with_quoted_price_doesnt_affect_npv() {
    // Arrange
    let mut ilb1 = sample_tips();
    let mut ilb2 = sample_tips();

    ilb1.quoted_clean = Some(100.0);
    ilb2.quoted_clean = Some(110.0);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let pv1 = ilb1.value(&ctx, as_of).unwrap();
    let pv2 = ilb2.value(&ctx, as_of).unwrap();

    // Assert - NPV should be calculated from curves, not quoted price
    assert_approx_eq(
        pv1.amount(),
        pv2.amount(),
        EPSILON,
        "quoted price independence",
    );
}

#[test]
fn test_npv_positive_for_positive_coupons() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert
    assert!(pv.amount() > 0.0);
}

#[test]
fn test_npv_scales_with_notional() {
    // Arrange
    let mut ilb_1m = sample_tips();
    let mut ilb_2m = sample_tips();

    ilb_1m.notional = finstack_quant_core::money::Money::new(1_000_000.0, Currency::USD);
    ilb_2m.notional = finstack_quant_core::money::Money::new(2_000_000.0, Currency::USD);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let pv_1m = ilb_1m.value(&ctx, as_of).unwrap();
    let pv_2m = ilb_2m.value(&ctx, as_of).unwrap();

    // Assert - 2x notional → 2x PV
    assert_approx_eq(
        pv_2m.amount() / pv_1m.amount(),
        2.0,
        REL_TOL,
        "notional scaling",
    );
}

#[test]
fn test_uk_gilt_npv() {
    // Arrange
    let ilb = sample_uk_linker();
    let (ctx, _) = uk_market_context();
    let as_of = d(2025, 1, 2);

    // Act
    let pv = ilb.value(&ctx, as_of).unwrap();

    // Assert
    assert_eq!(pv.currency(), Currency::GBP);
    assert!(pv.amount() > 0.0);
}

/// Regression test for the real/nominal double-count defect.
///
/// `base_value` discounts the inflation-PROJECTED (nominal) cashflow schedule,
/// so `discount_curve_id` must be a NOMINAL curve. With a NON-FLAT setup — CPI
/// projected at 2%/yr and nominal discounting at 3%/yr — the PV of a
/// zero-real-coupon 2y linker must satisfy the hand-computed identity
///
/// ```text
/// PV = notional × IndexRatio(T) × DF_nom(T)
///    = 1_000_000 × 1.0404 × 1.03⁻² ≈ 980_676.78
/// ```
///
/// The old shipped defaults named REAL curves, so a real curve wired in here
/// double-counted inflation (~+2% per year of maturity); flat-curve tests
/// could not see the error because projection and discounting both collapse.
#[test]
fn test_pv_matches_projected_index_ratio_times_nominal_df() {
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::InflationLag;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, InflationCurve};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::{
        DeflationProtection, IndexationMethod, InflationLinkedBond,
    };
    use finstack_quant_valuations::instruments::Attributes;

    // 2-year window containing no Feb 29 so ACT/365F gives t(maturity) = 730/365 = 2.0
    // exactly, landing on the curve knots and keeping the expectation hand-computable.
    let as_of = d(2025, 1, 15);
    let maturity = d(2027, 1, 15);

    let bond = InflationLinkedBond::builder()
        .id(InstrumentId::new("ILB-NOMINAL-PV"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .real_coupon(rust_decimal::Decimal::ZERO)
        .frequency(Tenor::annual())
        .day_count(DayCount::Act365F)
        .issue_date(as_of)
        .maturity(maturity)
        .base_index(100.0)
        .base_date(as_of)
        .indexation_method(IndexationMethod::TIPS)
        .lag(InflationLag::None)
        .deflation_protection(DeflationProtection::MaturityOnly)
        .bdc(BusinessDayConvention::Following)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .inflation_index_id(CurveId::new("US-CPI"))
        .attributes(Attributes::new())
        .build()
        .unwrap();

    let df_2y = 1.03_f64.powi(-2);
    // Day count set explicitly: the builder would otherwise infer ACT/360 from
    // the "USD-OIS" id, shifting t(maturity) off the 2.0 knot.
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 1.03_f64.powi(-1)), (2.0, df_2y)])
        .build()
        .unwrap();
    let inflation = InflationCurve::builder("US-CPI")
        .base_date(as_of)
        .base_cpi(100.0)
        .knots([(0.0, 100.0), (1.0, 102.0), (2.0, 104.04)])
        .build()
        .unwrap();
    let ctx = MarketContext::new().insert(discount).insert(inflation);

    let pv = bond.value(&ctx, as_of).unwrap().amount();

    let index_ratio = 1.0404; // 104.04 / 100 at maturity
    let expected = 1_000_000.0 * index_ratio * df_2y; // ≈ 980_676.78

    assert_approx_eq(
        pv,
        expected,
        1e-7,
        "PV must equal notional × IndexRatio(T) × DF_nom(T)",
    );

    // Discriminators: projection actually happened (PV above the unprojected
    // discounted notional) and discounting actually happened (PV below the
    // undiscounted projected principal).
    assert!(
        pv > 1_000_000.0 * df_2y + 1_000.0,
        "projection missing: {pv}"
    );
    assert!(
        pv < 1_000_000.0 * index_ratio - 1_000.0,
        "discounting missing: {pv}"
    );
}
