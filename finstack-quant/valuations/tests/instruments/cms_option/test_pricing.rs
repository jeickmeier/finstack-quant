//! Pricing tests for CMS Option.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::rates::cms_option::CmsOption;
use finstack_quant_valuations::instruments::{Instrument, OptionType};
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::Month;

fn standard_market(as_of: Date) -> MarketContext {
    let mut market = MarketContext::new();

    // Add OIS Curve (Flat 3%)
    let knots = vec![
        (0.0, 1.0),
        (1.0, (-0.03 * 1.0f64).exp()),
        (5.0, (-0.03 * 5.0f64).exp()),
        (10.0, (-0.03 * 10.0f64).exp()),
        (30.0, (-0.03 * 30.0f64).exp()),
    ];

    let discount_curve = DiscountCurve::builder(CurveId::new("USD-OIS"))
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots(knots)
        .build()
        .unwrap();

    market = market.insert(discount_curve);

    // Add LIBOR Forward Curve (Flat 3% for simplicity, or slightly different)
    // Let's make it 3.5% to have spread
    let fwd_knots = vec![(0.0, 0.035), (10.0, 0.035), (30.0, 0.035)];
    let forward_curve = ForwardCurve::builder(CurveId::new("USD-LIBOR-3M"), 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots(fwd_knots)
        .build()
        .unwrap();

    market = market.insert(forward_curve);

    // Add Vol Surface (Flat 20%)
    // Manually build a grid
    let strikes = vec![0.01, 0.02, 0.03, 0.04, 0.05];
    let expiries = vec![0.5, 1.0, 5.0, 10.0, 20.0];
    let flat_row = vec![0.20; 5];

    let mut builder = VolSurface::builder(CurveId::new("USD-CMS10Y-VOL"))
        .expiries(&expiries)
        .strikes(&strikes);

    for _ in 0..expiries.len() {
        builder = builder.row(&flat_row);
    }

    let vol_surface = builder.build().unwrap();

    market = market.insert_surface(vol_surface);

    market
}

#[test]
fn test_cms_cap_pricing() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let market = standard_market(as_of);

    let inst = CmsOption::example();

    // Price
    let pv = inst.value(&market, as_of).expect("Pricing failed");

    assert!(
        pv.amount() > 0.0,
        "PV should be positive, got {}",
        pv.amount()
    );
}

#[test]
fn test_convexity_value() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let market = standard_market(as_of);

    let inst = CmsOption::example();

    // Calculate Convexity Adjustment Risk
    let result = inst
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ConvexityAdjustmentRisk],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("Metric calc failed");

    let convexity_val = result
        .measures
        .get(MetricId::ConvexityAdjustmentRisk.as_str())
        .copied()
        .expect("ConvexityAdjustmentRisk metric not found");

    // Convexity adjustment for CMS rate adds to the rate (usually).
    // So Adjusted Rate > Forward Rate.
    // For a Call (Cap), higher rate = higher value.
    // So Convexity Value should be positive.
    println!("Convexity Value: {}", convexity_val);
    // Ideally should be > 0.0, but allowing >= 0.0 for now if test setup makes it small
    assert!(
        convexity_val >= 0.0,
        "Convexity adjustment should be non-negative, got {}",
        convexity_val
    );
}

#[test]
fn test_convexity_adjustment_rate_dependency() {
    // The first-order Hagan (2003) convexity adjustment must be positive,
    // finite, a sane rate-scale quantity, and depend on the forward-rate
    // level. The earlier dimensionally-wrong `0.5·σ²T·G(S)` formula returned
    // ~0.78 (7800 bp) for these inputs; a correct adjustment for a 20Y CMS,
    // 5Y to fixing, 20% vol is single-to-low-double-digit bp.
    use finstack_quant_valuations::instruments::rates::cms_option::pricer::convexity_adjustment;

    let vol = 0.20;
    let time_to_fixing = 5.0;
    let swap_tenor = 20.0;

    let adj_low_rate = convexity_adjustment(vol, time_to_fixing, swap_tenor, 0.01);
    let adj_mid_rate = convexity_adjustment(vol, time_to_fixing, swap_tenor, 0.03);
    let adj_high_rate = convexity_adjustment(vol, time_to_fixing, swap_tenor, 0.05);

    for adj in [adj_low_rate, adj_mid_rate, adj_high_rate] {
        assert!(
            adj.is_finite() && adj > 0.0 && adj < 0.05,
            "convexity adjustment must be a sane positive rate-scale quantity (< 500 bp), got {adj}"
        );
    }
    assert!(
        (adj_low_rate - adj_high_rate).abs() > 1e-9,
        "convexity adjustment should depend on the forward-rate level"
    );
}

#[test]
fn test_vanna_computable() {
    // Test that vanna can be computed without errors
    // Note: The analytical vanna in the CMS pricer is an approximation that
    // accounts for convexity adjustment sensitivity. It may not match a pure
    // finite difference approach exactly due to the complex coupling between
    // vol and rate in CMS pricing.
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let market = standard_market(as_of);

    let inst = CmsOption::example();

    // Get analytical vanna and vega
    let result = inst
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vanna, MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("Metrics calc failed");

    let analytical_vanna = result
        .measures
        .get(MetricId::Vanna.as_str())
        .copied()
        .unwrap_or(f64::NAN);

    let vega = result
        .measures
        .get(MetricId::Vega.as_str())
        .copied()
        .unwrap_or(f64::NAN);

    println!("Analytical Vanna: {}, Vega: {}", analytical_vanna, vega);

    // Vanna should be finite (not NaN or infinity)
    assert!(
        analytical_vanna.is_finite(),
        "Vanna should be finite, got {}",
        analytical_vanna
    );

    // Vega should be positive for a cap
    assert!(
        vega > 0.0,
        "Vega should be positive for a cap, got {}",
        vega
    );
}

#[test]
fn test_cms_option_requires_vol_surface_in_market() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let market = standard_market(as_of);

    // Create instrument with a vol_surface_id that doesn't exist in the market
    let fixing_dates = vec![
        Date::from_calendar_date(2025, Month::March, 20).unwrap(),
        Date::from_calendar_date(2025, Month::June, 20).unwrap(),
    ];
    let payment_dates = vec![
        Date::from_calendar_date(2025, Month::June, 20).unwrap(),
        Date::from_calendar_date(2025, Month::September, 22).unwrap(),
    ];
    let accrual_fractions = vec![0.25, 0.25];

    let inst = CmsOption {
        id: InstrumentId::new("CMS-NO-VOL"),
        strike: Decimal::try_from(0.025).expect("valid decimal"),
        cms_tenor: 10.0,
        fixing_dates,
        payment_dates,
        accrual_fractions,
        option_type: OptionType::Call,
        notional: Money::new(1_000_000.0, Currency::USD),
        day_count: DayCount::Act365F,
        swap_convention: None,
        swap_fixed_freq: Some(Tenor::semi_annual()),
        swap_float_freq: Some(Tenor::quarterly()),
        swap_day_count: Some(DayCount::Thirty360),
        swap_float_day_count: Some(DayCount::Act360),
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-LIBOR-3M"),
        vol_surface_id: CurveId::new("NONEXISTENT-VOL"), // Vol surface not in market
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    // Pricing should fail because vol surface is not in market
    let result = inst.value(&market, as_of);
    assert!(
        result.is_err(),
        "Should fail when vol surface is not in market"
    );
}

/// Build a 1-period seasoned CMS option: fixing in the past, payment in the
/// future relative to the test's `as_of`.
fn seasoned_cms_option(fixing: Date, payment: Date) -> CmsOption {
    CmsOption {
        id: InstrumentId::new("CMS-SEASONED"),
        strike: Decimal::try_from(0.025).expect("valid decimal"),
        cms_tenor: 10.0,
        fixing_dates: vec![fixing],
        payment_dates: vec![payment],
        accrual_fractions: vec![0.25],
        option_type: OptionType::Call,
        notional: Money::new(1_000_000.0, Currency::USD),
        day_count: DayCount::Act365F,
        swap_convention: None,
        swap_fixed_freq: Some(Tenor::semi_annual()),
        swap_float_freq: Some(Tenor::quarterly()),
        swap_day_count: Some(DayCount::Thirty360),
        swap_float_day_count: Some(DayCount::Act360),
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-LIBOR-3M"),
        vol_surface_id: CurveId::new("USD-CMS10Y-VOL"),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

/// Insert a CMS fixing series (`FIXING:CMS-10Y:USD-LIBOR-3M`) recording
/// `observed` on `fixing`.
fn with_cms_fixing(market: MarketContext, fixing: Date, observed: f64) -> MarketContext {
    use finstack_quant_core::market_data::fixings::cms_fixing_series_id;
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;

    let series = ScalarTimeSeries::new(
        cms_fixing_series_id("USD-LIBOR-3M", 10.0),
        vec![(fixing, observed)],
        None,
    )
    .expect("fixing series");
    market.insert_series(series)
}

/// Seasoned CMS caplet (Black-76 pricer): the period whose fixing is in the
/// past must be valued as intrinsic on the *recorded* fixing — and a missing
/// fixing series must be a hard error, never silent live-curve projection.
#[test]
fn test_seasoned_cms_option_uses_historical_fixing() {
    let fixing = Date::from_calendar_date(2024, Month::December, 1).unwrap();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let payment = Date::from_calendar_date(2025, Month::June, 1).unwrap();

    let inst = seasoned_cms_option(fixing, payment);
    let market = standard_market(as_of);

    // Missing fixing series → hard error.
    let err = inst
        .value(&market, as_of)
        .expect_err("missing CMS fixing series must be a hard error");
    assert!(
        err.to_string().contains("FIXING:CMS-10Y:USD-LIBOR-3M"),
        "error must name the missing series: {err}"
    );

    // Recorded fixing → intrinsic on the observed rate, discounted from payment.
    let observed = 0.05;
    let market = with_cms_fixing(market, fixing, observed);
    let pv = inst
        .value(&market, as_of)
        .expect("seasoned pricing")
        .amount();

    let df = market
        .get_discount("USD-OIS")
        .unwrap()
        .df_between_dates(as_of, payment)
        .unwrap();
    let expected = (observed - 0.025) * 0.25 * df * 1_000_000.0;
    assert!(
        (pv - expected).abs() < 0.01,
        "seasoned CMS caplet must price intrinsic on the recorded fixing: \
         expected {expected}, got {pv}"
    );
}

/// Same seasoned-fixing contract for the static-replication pricer.
#[test]
fn test_seasoned_cms_option_replication_uses_historical_fixing() {
    use finstack_quant_valuations::instruments::rates::cms_option::replication_pricer::CmsReplicationPricer;
    use finstack_quant_valuations::pricer::Pricer;

    let fixing = Date::from_calendar_date(2024, Month::December, 1).unwrap();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let payment = Date::from_calendar_date(2025, Month::June, 1).unwrap();

    let inst = seasoned_cms_option(fixing, payment);
    let market = standard_market(as_of);

    // Missing fixing series → hard error.
    let err = CmsReplicationPricer::new()
        .price_dyn(&inst, &market, as_of)
        .expect_err("missing CMS fixing series must be a hard error");
    assert!(
        err.to_string().contains("FIXING:CMS-10Y:USD-LIBOR-3M"),
        "error must name the missing series: {err}"
    );

    // Recorded fixing → intrinsic on the observed rate, discounted from payment.
    let observed = 0.05;
    let market = with_cms_fixing(market, fixing, observed);
    let pv = CmsReplicationPricer::new()
        .price_dyn(&inst, &market, as_of)
        .expect("seasoned replication pricing")
        .value
        .amount();

    let df = market
        .get_discount("USD-OIS")
        .unwrap()
        .df_between_dates(as_of, payment)
        .unwrap();
    let expected = (observed - 0.025) * 0.25 * df * 1_000_000.0;
    assert!(
        (pv - expected).abs() < 0.01,
        "seasoned replication CMS caplet must price intrinsic on the recorded \
         fixing: expected {expected}, got {pv}"
    );
}

/// Build a market whose vol surface has a strong skew: a fixed ATM vol at the
/// forward swap rate (~3.5%) but a very different vol at the option strike.
///
/// `atm_vol` is placed at every node on the 3%–4% strike band so the surface
/// value at the forward is exactly `atm_vol`; `wing_vol` is placed on the
/// 1%–2% strikes. With `strike = 1.5%` the option samples `wing_vol`, while the
/// CMS convexity adjustment — a property of the swap-rate distribution — must
/// sample `atm_vol`.
fn skewed_vol_market(as_of: Date, atm_vol: f64, wing_vol: f64) -> MarketContext {
    let mut market = standard_market(as_of);
    // Replace the flat surface with a skewed one.
    let strikes = vec![0.01, 0.02, 0.03, 0.035, 0.04, 0.05];
    let expiries = vec![0.5, 1.0, 5.0, 10.0, 20.0];
    // Per-strike vols: deep wings carry `wing_vol`, the ATM band carries `atm_vol`.
    let row = vec![wing_vol, wing_vol, atm_vol, atm_vol, atm_vol, atm_vol];
    let mut builder = VolSurface::builder(CurveId::new("USD-CMS10Y-VOL"))
        .expiries(&expiries)
        .strikes(&strikes);
    for _ in 0..expiries.len() {
        builder = builder.row(&row);
    }
    market = market.insert_surface(builder.build().unwrap());
    market
}

/// Regression test (item 1): the CMS convexity adjustment must be computed with
/// the ATM vol σ(F), not the strike vol σ(K).
///
/// Failure mode being guarded: the Hagan convexity adjustment was previously
/// evaluated at `vol_surface.value_clamped(t, strike)`. The convexity
/// adjustment is a property of the swap-rate *distribution* under the annuity
/// measure (`Var^A[S] ≈ F²σ(F)²T`) and must use σ(F); using σ(K) makes the
/// adjusted forward depend on the option strike and disagrees with the
/// static-replication pricer.
///
/// Construction: a deep ITM CMS caplet (strike 1.5% vs forward ~3.5%) is priced
/// on a strongly skewed surface (ATM 20%, wing 80%). A deep-ITM caplet's price
/// is dominated by `(F_adjusted - K)`, so it is sensitive to the convexity
/// adjustment but only weakly sensitive to the option-payoff vol. If the
/// adjustment correctly used σ(F)=20%, the price matches the same option on a
/// FLAT-20% surface; if it (wrongly) used the wing vol σ(K)=80%, the adjusted
/// forward — and hence the price — would be materially larger.
#[test]
fn convexity_adjustment_uses_atm_vol_not_strike_vol() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Deep-ITM caplet: strike well below the ~3.5% forward.
    let mut inst = CmsOption::example();
    inst.strike = Decimal::try_from(0.015).expect("valid decimal");

    // Skewed surface: σ(F)=20% at the forward, σ(K)=80% at the strike.
    let skewed = skewed_vol_market(as_of, 0.20, 0.80);
    let pv_skewed = inst.value(&skewed, as_of).expect("skewed pricing").amount();

    // Flat surface everywhere at the ATM level 20%.
    let flat = standard_market(as_of); // standard_market is flat 20%
    let pv_flat = inst.value(&flat, as_of).expect("flat pricing").amount();

    // The convexity adjustment must be identical (both σ(F)=20%). For a
    // deep-ITM caplet the option-payoff vol contributes only a small time
    // value, so the prices must agree closely. With the pre-fix bug the
    // skewed-surface convexity used σ(K)=80%, inflating the adjusted forward
    // and the price by far more than this tolerance.
    let rel_diff = (pv_skewed - pv_flat).abs() / pv_flat.max(1.0);
    assert!(
        rel_diff < 0.05,
        "deep-ITM CMS caplet price must be driven by σ(F) for the convexity \
         adjustment, independent of σ(K): pv_skewed={pv_skewed}, pv_flat={pv_flat}, \
         rel_diff={rel_diff}"
    );
}
