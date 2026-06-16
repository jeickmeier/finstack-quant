//! Pricing tests for commodity options.

use crate::finstack_quant_test_utils::{
    date, flat_discount_with_tenor, flat_price_curve, flat_vol_surface,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
#[allow(unused_imports)]
use finstack_quant_core::math::norm_cdf;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::commodity::commodity_option::CommodityOption;
use finstack_quant_valuations::instruments::Attributes;
use finstack_quant_valuations::instruments::CommodityUnderlyingParams;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{
    ExerciseStyle, OptionType, PricingOverrides, SettlementType,
};

fn bs_price(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    option_type: OptionType,
) -> f64 {
    if t <= 0.0 || sigma <= 0.0 {
        return match option_type {
            OptionType::Call => (spot - strike).max(0.0),
            OptionType::Put => (strike - spot).max(0.0),
        };
    }
    let sqrt_t = t.sqrt();
    let d1 = ((spot / strike).ln() + (r - q + 0.5 * sigma * sigma) * t) / (sigma * sqrt_t);
    let d2 = d1 - sigma * sqrt_t;
    let disc_q = (-q * t).exp();
    let disc_r = (-r * t).exp();

    match option_type {
        OptionType::Call => spot * disc_q * norm_cdf(d1) - strike * disc_r * norm_cdf(d2),
        OptionType::Put => strike * disc_r * norm_cdf(-d2) - spot * disc_q * norm_cdf(-d1),
    }
}

#[test]
fn test_black76_futures_based_pricing() {
    let as_of = date(2025, 1, 1);
    let expiry = date(2026, 1, 1);

    let discount_curve = flat_discount_with_tenor("USD-OIS", as_of, 0.03, 1.0);
    // Use PriceCurve with flat forward price of 100
    let price_curve = flat_price_curve("CL-FWD", as_of, 100.0, 1.0);
    let vol_surface = flat_vol_surface("CL-VOL", &[1.0], &[80.0, 100.0, 120.0], 0.20);

    let market = MarketContext::new()
        .insert(discount_curve)
        .insert(price_curve)
        .insert_surface(vol_surface);

    let option = CommodityOption::builder()
        .id(InstrumentId::new("CL-CALL-TEST"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .strike(100.0)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .quantity(1.0)
        .multiplier(1.0)
        .settlement(SettlementType::Cash)
        .forward_curve_id(CurveId::new("CL-FWD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("CL-VOL"))
        .day_count(DayCount::Act365F)
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let pv = option.value(&market, as_of).expect("should price");
    let t = DayCount::Act365F
        .year_fraction(as_of, expiry, DayCountContext::default())
        .expect("year fraction");
    let forward = option.forward_price(&market, as_of).expect("forward");
    let df = (-0.03 * t).exp();
    let expected = bs_price(forward, 100.0, 0.0, 0.0, 0.20, t, OptionType::Call) * df;

    // Allow small tolerance for day count/interpolation differences
    assert!(
        (pv.amount() - expected).abs() < 0.01,
        "PV mismatch: {} vs expected {}",
        pv.amount(),
        expected
    );
}

#[test]
fn test_futures_based_american_matches_european() {
    let as_of = date(2025, 1, 1);
    let expiry = date(2026, 1, 1);

    let discount_curve = flat_discount_with_tenor("USD-OIS", as_of, 0.02, 1.0);
    // Use PriceCurve with flat forward price of 100
    let price_curve = flat_price_curve("CL-FWD", as_of, 100.0, 1.0);
    let vol_surface = flat_vol_surface("CL-VOL", &[1.0], &[90.0, 100.0, 110.0], 0.25);

    let market = MarketContext::new()
        .insert(discount_curve)
        .insert(price_curve)
        .insert_surface(vol_surface);

    let build = |style| {
        CommodityOption::builder()
            .id(InstrumentId::new("CL-CALL-BASE"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .strike(100.0)
            .option_type(OptionType::Call)
            .exercise_style(style)
            .expiry(expiry)
            .quantity(1.0)
            .multiplier(1.0)
            .settlement(SettlementType::Cash)
            .forward_curve_id(CurveId::new("CL-FWD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("CL-VOL"))
            .day_count(DayCount::Act365F)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("should build")
    };

    let european = build(ExerciseStyle::European);
    let american = build(ExerciseStyle::American);

    let pv_eur = european.value(&market, as_of).expect("price european");
    let pv_amer = american.value(&market, as_of).expect("price american");

    // American call on futures should be close to European (no early exercise value)
    // Allow wider tolerance for tree vs closed-form numerical differences
    assert!(
        (pv_amer.amount() - pv_eur.amount()).abs() < 0.1,
        "American={} vs European={}",
        pv_amer.amount(),
        pv_eur.amount()
    );
}

#[test]
fn test_futures_based_bermudan_prices_between_european_and_american() {
    let as_of = date(2025, 1, 1);
    let expiry = date(2026, 1, 1);

    let discount_curve = flat_discount_with_tenor("USD-OIS", as_of, 0.02, 1.0);
    let price_curve = flat_price_curve("CL-FWD", as_of, 100.0, 1.0);
    let vol_surface = flat_vol_surface("CL-VOL", &[1.0], &[90.0, 100.0, 110.0], 0.25);

    let market = MarketContext::new()
        .insert(discount_curve)
        .insert(price_curve)
        .insert_surface(vol_surface);

    let build = |style| {
        CommodityOption::builder()
            .id(InstrumentId::new("CL-CALL-BERM"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .strike(100.0)
            .option_type(OptionType::Call)
            .exercise_style(style)
            .exercise_schedule_opt(Some(vec![
                date(2025, 4, 1),
                date(2025, 7, 1),
                date(2025, 10, 1),
                expiry,
            ]))
            .expiry(expiry)
            .quantity(1.0)
            .multiplier(1.0)
            .settlement(SettlementType::Cash)
            .forward_curve_id(CurveId::new("CL-FWD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("CL-VOL"))
            .day_count(DayCount::Act365F)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("should build")
    };

    let european = build(ExerciseStyle::European);
    let bermudan = build(ExerciseStyle::Bermudan);
    let american = build(ExerciseStyle::American);

    let pv_eur = european.value(&market, as_of).expect("price european");
    let pv_berm = bermudan.value(&market, as_of).expect("price bermudan");
    let pv_amer = american.value(&market, as_of).expect("price american");

    assert!(
        pv_berm.amount() + 1e-6 >= pv_eur.amount(),
        "Bermudan should not price below European: bermudan={}, european={}",
        pv_berm.amount(),
        pv_eur.amount()
    );
    assert!(
        pv_berm.amount() <= pv_amer.amount() + 1e-6,
        "Bermudan should not price above American: bermudan={}, american={}",
        pv_berm.amount(),
        pv_amer.amount()
    );
}

#[test]
fn test_spot_based_american_put_above_european() {
    let as_of = date(2025, 1, 1);
    let expiry = date(2026, 1, 1);

    let discount_curve = flat_discount_with_tenor("USD-OIS", as_of, 0.05, 1.0);
    // Price curve shows forward at 90 * exp(0.02 * 1) ≈ 91.8 (contango)
    let price_curve = flat_price_curve("CL-FWD", as_of, 91.8, 1.0);
    let vol_surface = flat_vol_surface("CL-VOL", &[1.0], &[80.0, 100.0, 120.0], 0.30);

    let market = MarketContext::new()
        .insert(discount_curve)
        .insert(price_curve)
        .insert_surface(vol_surface)
        .insert_price("CL-SPOT", MarketScalar::Unitless(90.0));

    let build = |style| {
        CommodityOption::builder()
            .id(InstrumentId::new("CL-PUT-BASE"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .strike(100.0)
            .option_type(OptionType::Put)
            .exercise_style(style)
            .expiry(expiry)
            .quantity(1.0)
            .multiplier(1.0)
            .settlement(SettlementType::Cash)
            .forward_curve_id(CurveId::new("CL-FWD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("CL-VOL"))
            .spot_id_opt(Some("CL-SPOT".to_string()))
            .day_count(DayCount::Act365F)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("should build")
    };

    let european = build(ExerciseStyle::European);
    let american = build(ExerciseStyle::American);

    let pv_eur = european.value(&market, as_of).expect("price european");
    let pv_amer = american.value(&market, as_of).expect("price american");

    assert!(pv_amer.amount() + 1e-6 >= pv_eur.amount());
}

#[test]
fn test_post_expiry_returns_zero() {
    let expiry = date(2025, 6, 15);
    let as_of_after_expiry = date(2025, 6, 16); // 1 day after expiry

    let discount_curve = flat_discount_with_tenor("USD-OIS", as_of_after_expiry, 0.03, 1.0);
    let price_curve = flat_price_curve("CL-FWD", as_of_after_expiry, 100.0, 1.0);
    let vol_surface = flat_vol_surface("CL-VOL", &[1.0], &[80.0, 100.0, 120.0], 0.20);

    let market = MarketContext::new()
        .insert(discount_curve)
        .insert(price_curve)
        .insert_surface(vol_surface);

    // ITM call option (forward 100 > strike 90)
    let itm_call = CommodityOption::builder()
        .id(InstrumentId::new("CL-CALL-EXPIRED"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .strike(90.0) // ITM: forward 100 > strike 90
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .quantity(1.0)
        .multiplier(1.0)
        .settlement(SettlementType::Cash)
        .forward_curve_id(CurveId::new("CL-FWD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("CL-VOL"))
        .day_count(DayCount::Act365F)
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    // After expiry, NPV should be 0 (option is fully settled)
    let pv = itm_call
        .value(&market, as_of_after_expiry)
        .expect("should price");
    assert_eq!(
        pv.amount(),
        0.0,
        "Post-expiry option NPV should be 0, got {}",
        pv.amount()
    );

    // ITM put option
    let itm_put = CommodityOption::builder()
        .id(InstrumentId::new("CL-PUT-EXPIRED"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .strike(110.0) // ITM: strike 110 > forward 100
        .option_type(OptionType::Put)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .quantity(1.0)
        .multiplier(1.0)
        .settlement(SettlementType::Cash)
        .forward_curve_id(CurveId::new("CL-FWD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("CL-VOL"))
        .day_count(DayCount::Act365F)
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let pv_put = itm_put
        .value(&market, as_of_after_expiry)
        .expect("should price");
    assert_eq!(
        pv_put.amount(),
        0.0,
        "Post-expiry put NPV should be 0, got {}",
        pv_put.amount()
    );
}

#[test]
fn test_at_expiry_returns_intrinsic() {
    let expiry = date(2025, 6, 15);
    let as_of_at_expiry = expiry; // Exactly at expiry

    let discount_curve = flat_discount_with_tenor("USD-OIS", as_of_at_expiry, 0.03, 1.0);
    let price_curve = flat_price_curve("CL-FWD", as_of_at_expiry, 100.0, 1.0);
    let vol_surface = flat_vol_surface("CL-VOL", &[1.0], &[80.0, 100.0, 120.0], 0.20);

    let market = MarketContext::new()
        .insert(discount_curve)
        .insert(price_curve)
        .insert_surface(vol_surface);

    // ITM call: forward 100 > strike 90, intrinsic = 10
    let itm_call = CommodityOption::builder()
        .id(InstrumentId::new("CL-CALL-AT-EXPIRY"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .strike(90.0)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .quantity(1.0)
        .multiplier(1.0)
        .settlement(SettlementType::Cash)
        .forward_curve_id(CurveId::new("CL-FWD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("CL-VOL"))
        .day_count(DayCount::Act365F)
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let pv = itm_call
        .value(&market, as_of_at_expiry)
        .expect("should price");
    // Forward = 100, Strike = 90, intrinsic = max(100 - 90, 0) = 10
    assert!(
        (pv.amount() - 10.0).abs() < 0.01,
        "At-expiry ITM call should have intrinsic value ~10, got {}",
        pv.amount()
    );

    // OTM call: forward 100 < strike 110, intrinsic = 0
    let otm_call = CommodityOption::builder()
        .id(InstrumentId::new("CL-CALL-OTM-AT-EXPIRY"))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .strike(110.0)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .quantity(1.0)
        .multiplier(1.0)
        .settlement(SettlementType::Cash)
        .forward_curve_id(CurveId::new("CL-FWD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("CL-VOL"))
        .day_count(DayCount::Act365F)
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let pv_otm = otm_call
        .value(&market, as_of_at_expiry)
        .expect("should price");
    assert!(
        pv_otm.amount().abs() < 0.01,
        "At-expiry OTM call should have intrinsic value ~0, got {}",
        pv_otm.amount()
    );
}

/// European put-call parity: C - P = DF × (F - K)
///
/// This is a fundamental no-arbitrage relationship for Black-76 options.
/// Verifies that the pricing implementation produces consistent call and put values.
#[test]
fn test_put_call_parity_european() {
    let as_of = date(2025, 1, 1);
    let expiry = date(2025, 7, 1);

    let discount_curve = flat_discount_with_tenor("USD-OIS", as_of, 0.05, 1.0);
    let price_curve = flat_price_curve("CL-FWD", as_of, 80.0, 1.0);
    let vol_surface = flat_vol_surface(
        "CL-VOL",
        &[0.25, 0.5, 1.0],
        &[60.0, 70.0, 80.0, 90.0, 100.0],
        0.30,
    );

    let market = MarketContext::new()
        .insert(discount_curve)
        .insert(price_curve)
        .insert_surface(vol_surface);

    // Test put-call parity at multiple strikes (ITM, ATM, OTM)
    for &strike in &[65.0, 75.0, 80.0, 85.0, 95.0] {
        let build = |opt_type| {
            CommodityOption::builder()
                .id(InstrumentId::new("PCP-TEST"))
                .underlying(CommodityUnderlyingParams::new(
                    "Energy",
                    "CL",
                    "BBL",
                    Currency::USD,
                ))
                .strike(strike)
                .option_type(opt_type)
                .exercise_style(ExerciseStyle::European)
                .expiry(expiry)
                .quantity(1.0)
                .multiplier(1.0)
                .settlement(SettlementType::Cash)
                .forward_curve_id(CurveId::new("CL-FWD"))
                .discount_curve_id(CurveId::new("USD-OIS"))
                .vol_surface_id(CurveId::new("CL-VOL"))
                .day_count(DayCount::Act365F)
                .pricing_overrides(PricingOverrides::default())
                .attributes(Attributes::new())
                .build()
                .expect("should build")
        };

        let call = build(OptionType::Call);
        let put = build(OptionType::Put);

        let call_pv = call
            .value(&market, as_of)
            .expect("call should price")
            .amount();
        let put_pv = put
            .value(&market, as_of)
            .expect("put should price")
            .amount();

        // Get forward price and discount factor
        let forward = call.forward_price(&market, as_of).expect("forward");
        let _t = DayCount::Act365F
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let disc = market.get_discount("USD-OIS").expect("discount curve");
        let df = disc.df_between_dates(as_of, expiry).expect("df");

        // Put-call parity: C - P = DF × (F - K)
        let lhs = call_pv - put_pv;
        let rhs = df * (forward - strike);

        let error = (lhs - rhs).abs();
        // Tolerance: 0.01 absolute, allowing for vol surface interpolation/clamping
        // at boundary strikes
        assert!(
            error < 0.01,
            "Put-call parity violated at strike={}: C-P={:.6}, DF*(F-K)={:.6}, error={:.6}",
            strike,
            lhs,
            rhs,
            error
        );
    }
}

/// Test that vol surface skew is correctly picked up by the pricing model.
///
/// With a skewed vol surface (lower vol at high strikes), ITM calls with
/// high strikes should be priced with lower vol than ATM calls.
#[test]
fn test_non_flat_vol_surface_skew() {
    use finstack_quant_core::market_data::surfaces::VolSurface;

    let as_of = date(2025, 1, 1);
    let expiry = date(2025, 7, 1);

    // Create a skewed vol surface (commodity-style: higher vol at lower strikes)
    let strikes = [60.0, 70.0, 80.0, 90.0, 100.0];
    let expiries = [0.25, 0.5, 1.0];
    // Skew: 35% at K=60, 30% at K=70, 25% at K=80 (ATM), 22% at K=90, 20% at K=100
    let skew_row = [0.35, 0.30, 0.25, 0.22, 0.20];

    let skewed_surface = VolSurface::builder("CL-VOL")
        .expiries(&expiries)
        .strikes(&strikes)
        .row(&skew_row)
        .row(&skew_row)
        .row(&skew_row)
        .build()
        .expect("skewed vol surface");

    let flat_surface = flat_vol_surface("CL-VOL", &expiries, &strikes, 0.25);

    let skewed_market = MarketContext::new()
        .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.03, 1.0))
        .insert(flat_price_curve("CL-FWD", as_of, 80.0, 1.0))
        .insert_surface(skewed_surface);

    let flat_market = MarketContext::new()
        .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.03, 1.0))
        .insert(flat_price_curve("CL-FWD", as_of, 80.0, 1.0))
        .insert_surface(flat_surface);

    // Build ATM call (strike = 80, same vol in both surfaces)
    let build_option = |strike, opt_type| {
        CommodityOption::builder()
            .id(InstrumentId::new("SKEW-TEST"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .strike(strike)
            .option_type(opt_type)
            .exercise_style(ExerciseStyle::European)
            .expiry(expiry)
            .quantity(1.0)
            .multiplier(1.0)
            .settlement(SettlementType::Cash)
            .forward_curve_id(CurveId::new("CL-FWD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("CL-VOL"))
            .day_count(DayCount::Act365F)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("should build")
    };

    // ATM call at K=80: both surfaces have 25% vol, so prices should match
    let atm_call = build_option(80.0, OptionType::Call);
    let atm_flat = atm_call
        .value(&flat_market, as_of)
        .expect("flat price")
        .amount();
    let atm_skew = atm_call
        .value(&skewed_market, as_of)
        .expect("skew price")
        .amount();
    assert!(
        (atm_flat - atm_skew).abs() < 0.01,
        "ATM prices should match: flat={}, skew={}",
        atm_flat,
        atm_skew
    );

    // Low-strike put (K=60): skewed surface has 35% vs flat 25%, should be more expensive
    let low_put = build_option(60.0, OptionType::Put);
    let low_flat = low_put
        .value(&flat_market, as_of)
        .expect("flat price")
        .amount();
    let low_skew = low_put
        .value(&skewed_market, as_of)
        .expect("skew price")
        .amount();
    assert!(
        low_skew > low_flat,
        "Low-strike put should be more expensive with skew (35% vs 25%): skew={}, flat={}",
        low_skew,
        low_flat
    );

    // High-strike call (K=100): skewed surface has 20% vs flat 25%, should be cheaper
    let high_call = build_option(100.0, OptionType::Call);
    let high_flat = high_call
        .value(&flat_market, as_of)
        .expect("flat price")
        .amount();
    let high_skew = high_call
        .value(&skewed_market, as_of)
        .expect("skew price")
        .amount();
    assert!(
        high_skew < high_flat,
        "High-strike call should be cheaper with skew (20% vs 25%): skew={}, flat={}",
        high_skew,
        high_flat
    );

    // Verify put-call parity still holds with skew
    let call_100 = build_option(100.0, OptionType::Call);
    let put_100 = build_option(100.0, OptionType::Put);
    let call_pv = call_100
        .value(&skewed_market, as_of)
        .expect("call")
        .amount();
    let put_pv = put_100.value(&skewed_market, as_of).expect("put").amount();

    let forward = call_100.forward_price(&skewed_market, as_of).expect("fwd");
    let disc = skewed_market.get_discount("USD-OIS").expect("disc");
    let df = disc.df_between_dates(as_of, expiry).expect("df");

    let lhs = call_pv - put_pv;
    let rhs = df * (forward - 100.0);
    let pcp_error = (lhs - rhs).abs();
    // Tolerance: 0.01 absolute, allowing for vol surface interpolation at boundary strikes
    assert!(
        pcp_error < 0.01,
        "Put-call parity should hold with skew: C-P={:.6}, DF*(F-K)={:.6}, error={:.6}",
        lhs,
        rhs,
        pcp_error
    );
}
