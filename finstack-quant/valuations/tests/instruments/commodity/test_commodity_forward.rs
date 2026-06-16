//! Integration tests for CommodityForward pricing.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, PriceCurve};
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::commodity::commodity_forward::{
    CommodityForward, Position,
};
use finstack_quant_valuations::instruments::Attributes;
use finstack_quant_valuations::instruments::CommodityUnderlyingParams;
use finstack_quant_valuations::instruments::Instrument;
use time::Month;

/// Helper to create a test market with discount and price curves.
fn create_test_market() -> MarketContext {
    // Create a flat 5% discount curve for USD-OIS
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (0.5, 0.975), (1.0, 0.95), (5.0, 0.78)])
        .build()
        .expect("discount curve should build");

    // Create a price curve for WTI forward prices
    let price_curve = PriceCurve::builder("WTI-FORWARD")
        .base_date(base_date)
        .spot_price(75.0)
        .knots([
            (0.0, 75.0),
            (0.25, 76.0),
            (0.5, 77.0),
            (1.0, 78.0),
            (2.0, 80.0),
        ])
        .build()
        .expect("price curve should build");

    MarketContext::new()
        .insert(discount_curve)
        .insert(price_curve)
}

#[test]
fn test_commodity_forward_pricing_with_quoted_price() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let settlement = Date::from_calendar_date(2025, Month::June, 15).unwrap();
    let market = create_test_market();

    // Create an off-market forward with contract_price below market
    let forward = CommodityForward::builder()
        .id(InstrumentId::new("WTI-TEST"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .quantity(1000.0)
        .multiplier(1.0)
        .maturity(settlement)
        .position(Position::Long)
        .contract_price_opt(Some(72.0)) // Entry price below market for positive MTM
        .quoted_price_opt(Some(75.0)) // Quoted market price at $75/BBL
        .forward_curve_id(CurveId::new("WTI-FORWARD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv = forward.value(&market, as_of).expect("should price");

    // Verify basic properties
    assert_eq!(npv.currency(), Currency::USD);
    // NPV = sign × (F - K) × Q × M × DF
    // = +1 × (75 - 72) × 1000 × 1 × DF
    // = 3000 × DF
    // Should be positive because we're long and F > K
    assert!(
        npv.amount() > 0.0,
        "NPV should be positive for a long forward with F > K, got {}",
        npv.amount()
    );

    // NPV should be roughly 3000 × DF (where DF ≈ 0.97 for ~6 months at 5%)
    assert!(
        npv.amount() < 3500.0 && npv.amount() > 2500.0,
        "NPV should be around 2800-3000, got {}",
        npv.amount()
    );
}

#[test]
fn test_commodity_forward_pricing_expired() {
    // Test that an expired forward has zero value
    let as_of = Date::from_calendar_date(2025, Month::June, 1).unwrap();
    let settlement = Date::from_calendar_date(2025, Month::May, 15).unwrap(); // Already settled
    let market = create_test_market();

    let forward = CommodityForward::builder()
        .id(InstrumentId::new("EXPIRED"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .quantity(1000.0)
        .multiplier(1.0)
        .maturity(settlement)
        .position(Position::Long)
        .quoted_price_opt(Some(75.0))
        .forward_curve_id(CurveId::new("WTI-FORWARD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv = forward.value(&market, as_of).expect("should price");

    // Expired forward should have zero value
    assert_eq!(npv.amount(), 0.0);
}

#[test]
fn test_commodity_forward_forward_price_from_curve() {
    // Test forward price calculation from PriceCurve (no quoted price)
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let settlement = Date::from_calendar_date(2025, Month::March, 15).unwrap();
    let market = create_test_market();

    let forward = CommodityForward::builder()
        .id(InstrumentId::new("CURVE-TEST"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .quantity(100.0)
        .multiplier(1.0)
        .maturity(settlement)
        .position(Position::Long)
        // No quoted_price - should use PriceCurve
        .forward_curve_id(CurveId::new("WTI-FORWARD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let price = forward
        .forward_price(&market, as_of)
        .expect("should get price from PriceCurve");

    // Should interpolate from PriceCurve: ~2.5 months out from base
    // Spot = 75, 3M = 76, so ~2.5M should be around 75.8
    assert!(
        price > 75.0 && price < 76.0,
        "Forward price should be around 75.8, got {}",
        price
    );
}

#[test]
fn test_commodity_forward_at_market_npv_zero() {
    // At-market forward (no contract_price) should have NPV ≈ 0
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let settlement = Date::from_calendar_date(2025, Month::June, 15).unwrap();
    let market = create_test_market();

    let forward = CommodityForward::builder()
        .id(InstrumentId::new("AT-MARKET"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .quantity(1000.0)
        .multiplier(1.0)
        .maturity(settlement)
        .position(Position::Long)
        // No contract_price → at-market
        .forward_curve_id(CurveId::new("WTI-FORWARD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv = forward.value(&market, as_of).expect("should price");

    // At-market: K = F, so NPV = 0
    assert!(
        npv.amount().abs() < 1e-10,
        "At-market NPV should be ~0, got {}",
        npv.amount()
    );
}

#[test]
fn test_commodity_forward_long_short_symmetry() {
    // Long and short positions should have opposite NPVs
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let settlement = Date::from_calendar_date(2025, Month::June, 15).unwrap();
    let market = create_test_market();

    let long = CommodityForward::builder()
        .id(InstrumentId::new("LONG"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .quantity(1000.0)
        .multiplier(1.0)
        .maturity(settlement)
        .position(Position::Long)
        .contract_price_opt(Some(72.0))
        .forward_curve_id(CurveId::new("WTI-FORWARD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .build()
        .expect("should build");

    let short = CommodityForward::builder()
        .id(InstrumentId::new("SHORT"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .quantity(1000.0)
        .multiplier(1.0)
        .maturity(settlement)
        .position(Position::Short)
        .contract_price_opt(Some(72.0))
        .forward_curve_id(CurveId::new("WTI-FORWARD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .build()
        .expect("should build");

    let long_npv = long.value(&market, as_of).expect("long should price");
    let short_npv = short.value(&market, as_of).expect("short should price");

    // Long + Short should net to zero
    let net = long_npv.amount() + short_npv.amount();
    assert!(
        net.abs() < 1e-10,
        "Long + Short NPV should sum to 0, got {}",
        net
    );
}

// =============================================================================
// Round-trip and Parity Tests
// =============================================================================

/// Round-trip test: Construct forward at quoted price → NPV = 0
///
/// This is a key validation requirement: a forward entered at the current
/// market price should have NPV = 0 at inception.
#[test]
fn test_commodity_forward_round_trip_at_market() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let settlement = Date::from_calendar_date(2025, Month::June, 15).unwrap();
    let market = create_test_market();

    // Step 1: Get market forward price
    let probe = CommodityForward::builder()
        .id(InstrumentId::new("PROBE"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .quantity(1.0)
        .multiplier(1.0)
        .maturity(settlement)
        .position(Position::Long)
        .forward_curve_id(CurveId::new("WTI-FORWARD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .build()
        .expect("should build");

    let market_fwd_price = probe
        .forward_price(&market, as_of)
        .expect("should get price");

    // Step 2: Enter forward at this price (round-trip)
    let forward_at_market = CommodityForward::builder()
        .id(InstrumentId::new("AT-MARKET-RT"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .quantity(1000.0)
        .multiplier(1.0)
        .maturity(settlement)
        .position(Position::Long)
        .contract_price_opt(Some(market_fwd_price)) // Enter at market price
        .forward_curve_id(CurveId::new("WTI-FORWARD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .build()
        .expect("should build");

    let npv = forward_at_market
        .value(&market, as_of)
        .expect("should price");

    // Round-trip test: NPV should be zero (< 0.01bp = 0.000001 relative)
    let tolerance_abs = 1e-8; // Absolute tolerance for near-zero
    assert!(
        npv.amount().abs() < tolerance_abs,
        "Round-trip test failed: forward at market price should have NPV = 0, got {} (market_price={})",
        npv.amount(),
        market_fwd_price
    );
}

/// Analytical verification: NPV = (F - K) × Q × DF
///
/// Verifies the pricing formula matches the expected analytical result.
#[test]
fn test_commodity_forward_analytical_parity() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let settlement = Date::from_calendar_date(2025, Month::June, 15).unwrap();
    let market = create_test_market();

    let forward_price = 77.5; // Quoted price
    let contract_price = 72.0; // Entry price
    let quantity = 1000.0;
    let multiplier = 1.0;

    let forward = CommodityForward::builder()
        .id(InstrumentId::new("ANALYTICAL-TEST"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .quantity(quantity)
        .multiplier(multiplier)
        .maturity(settlement)
        .position(Position::Long)
        .contract_price_opt(Some(contract_price))
        .quoted_price_opt(Some(forward_price))
        .forward_curve_id(CurveId::new("WTI-FORWARD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .build()
        .expect("should build");

    let npv = forward.value(&market, as_of).expect("should price");

    // Get the discount factor
    let disc = market.get_discount("USD-OIS").expect("discount curve");
    let df = disc
        .df_between_dates(as_of, settlement)
        .expect("df between dates");

    // Analytical: NPV = (F - K) × Q × M × DF
    let expected_npv = (forward_price - contract_price) * quantity * multiplier * df;

    // Tolerance: 1bp of notional (0.01% of notional value)
    let notional = forward_price * quantity * multiplier;
    let tolerance = notional * 0.0001; // 1bp

    let error = (npv.amount() - expected_npv).abs();
    assert!(
        error < tolerance,
        "Analytical parity failed: expected {:.6}, got {:.6}, error={:.6} (tolerance={:.6})",
        expected_npv,
        npv.amount(),
        error,
        tolerance
    );
}

/// Test delta matches the analytical formula
///
/// Delta = sign(position) × Q × M × DF
#[test]
fn test_commodity_forward_delta_analytical() {
    use finstack_quant_valuations::metrics::MetricId;
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let settlement = Date::from_calendar_date(2025, Month::June, 15).unwrap();
    let market = create_test_market();

    let quantity = 1000.0;
    let multiplier = 1.0;

    let forward = CommodityForward::builder()
        .id(InstrumentId::new("DELTA-TEST"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .quantity(quantity)
        .multiplier(multiplier)
        .maturity(settlement)
        .position(Position::Long)
        .contract_price_opt(Some(72.0))
        .forward_curve_id(CurveId::new("WTI-FORWARD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .build()
        .expect("should build");

    // Get delta from metric calculation
    use finstack_quant_valuations::instruments::Instrument;
    let result = forward
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("should price with delta");

    let delta = result
        .measures
        .get(&MetricId::Delta)
        .expect("should have delta");

    // Get analytical delta
    let disc = market.get_discount("USD-OIS").expect("discount curve");
    let df = disc
        .df_between_dates(as_of, settlement)
        .expect("df between dates");
    let expected_delta = quantity * multiplier * df; // Long position = positive sign

    // Tolerance: 0.1% of delta
    let tolerance = expected_delta.abs() * 0.001;
    let error = (delta - expected_delta).abs();

    assert!(
        error < tolerance,
        "Delta analytical parity failed: expected {:.6}, got {:.6}, error={:.6}",
        expected_delta,
        delta,
        error
    );
}

#[test]
fn test_commodity_forward_serialization() {
    let forward = CommodityForward::example();

    let json = serde_json::to_string_pretty(&forward).expect("serialize");
    let parsed: CommodityForward = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(forward.id.as_str(), parsed.id.as_str());
    assert_eq!(forward.underlying.ticker, parsed.underlying.ticker);
    assert_eq!(forward.quantity, parsed.quantity);
    assert_eq!(forward.underlying.currency, parsed.underlying.currency);
    assert_eq!(forward.position, parsed.position);
}
