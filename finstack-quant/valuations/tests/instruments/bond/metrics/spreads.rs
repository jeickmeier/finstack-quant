//! Z-spread and I-spread calculator tests.

use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::{Error, InputError};
use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_oas;
use finstack_quant_valuations::instruments::fixed_income::bond::ZSpreadCalculator;
use finstack_quant_valuations::instruments::fixed_income::bond::{
    Bond, BondSettlementConvention, CallPut, CallPutSchedule,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::InstrumentPricingOverrides;
use finstack_quant_valuations::metrics::{MetricCalculator, MetricContext, MetricId};
use std::sync::Arc;
use time::macros::date;

#[test]
fn test_z_spread_discount_bond() {
    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "ZSPR1",
        Money::new(100.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(95.0);

    let curve =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .build()
            .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new().insert(curve);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ZSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let z = *result.measures.get("z_spread").unwrap();
    assert!(z > 0.0); // Discount bond has positive spread
}

#[test]
fn test_z_spread_reports_bond_compounding_spread() {
    use finstack_quant_core::dates::{DayCount, DayCountContext};
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let notional = Money::new(1_000_000.0, Currency::USD);
    let base_zero_rate = 0.03_f64;
    let target_z = 0.01_f64;
    let years = DayCount::Act365F
        .year_fraction(as_of, maturity, DayCountContext::default())
        .unwrap();
    let mut bond = Bond::fixed(
        "ZSPR-COMPOUNDING",
        notional,
        0.0,
        as_of,
        maturity,
        "USD-OIS",
    )
    .expect("bond");
    bond.cashflow_spec =
        finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec::fixed_rate(
            0.0.into(),
            finstack_quant_core::dates::Tenor::annual(),
            DayCount::Act365F,
        )
        .expect("finite test coupon");
    bond.settlement_convention = None;
    let base_df = (1.0 + base_zero_rate).powf(-years);

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (years, base_df)])
        .build()
        .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new().insert(curve);
    let target_dirty = bond
        .cashflow_schedule(&market, as_of)
        .unwrap()
        .into_iter()
        .filter(|flow| flow.date > as_of)
        .map(|flow| {
            let t = DayCount::Act365F
                .year_fraction(as_of, flow.date, DayCountContext::default())
                .unwrap();
            let df_base = market
                .get_discount("USD-OIS")
                .unwrap()
                .df_between_dates(as_of, flow.date)
                .unwrap();
            let base_rate = df_base.powf(-1.0 / t) - 1.0;
            flow.amount.amount() * (1.0 + base_rate + target_z).powf(-t)
        })
        .sum::<f64>();
    bond.instrument_pricing_overrides = InstrumentPricingOverrides::default()
        .with_quoted_clean_price(100.0 * target_dirty / notional.amount());

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ZSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("Z-spread should solve");
    let z = *result.measures.get("z_spread").unwrap();

    assert!(
        (z - target_z).abs() < 2e-4,
        "Z-spread should be reported in the bond's annual compounding convention: target={target_z}, got={z}",
    );
}

#[test]
fn test_i_spread_uses_quote_date_for_settlement_based_curve() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    let as_of = date!(2025 - 01 - 06);
    let quote_date = date!(2025 - 01 - 08);
    let mut bond = Bond::fixed(
        "ISPR-QUOTE-DATE",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 08),
        "USD-OIS",
    )
    .unwrap();
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(99.0);

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(quote_date)
        .knots([(0.0, 1.0), (5.0, 0.85)])
        .build()
        .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new().insert(curve);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm, MetricId::ISpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("I-spread should use settlement/quote date when the curve is quote-date based");

    assert!(result.measures["i_spread"].is_finite());
}

#[test]
fn test_asw_market_price_adjustment_has_correct_economic_sign() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "ASW-DISCOUNT-SIGN",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.85)])
        .build()
        .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new().insert(curve);

    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(98.0);
    let discount_result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ASWPar, MetricId::ASWMarket],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("discount bond ASW metrics should compute");
    assert!(
        discount_result.measures["asw_market"] > discount_result.measures["asw_par"],
        "discount bond ASW market spread should exceed par ASW: par={}, market={}",
        discount_result.measures["asw_par"],
        discount_result.measures["asw_market"]
    );

    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(102.0);
    let premium_result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ASWPar, MetricId::ASWMarket],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("premium bond ASW metrics should compute");
    assert!(
        premium_result.measures["asw_market"] < premium_result.measures["asw_par"],
        "premium bond ASW market spread should be below par ASW: par={}, market={}",
        premium_result.measures["asw_par"],
        premium_result.measures["asw_market"]
    );
}

#[test]
fn test_asw_market_uses_configured_forward_curve() {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};

    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "ASW-FORWARD-CURVE",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    bond.instrument_pricing_overrides = InstrumentPricingOverrides::default()
        .with_quoted_clean_price(98.0)
        .with_asw_forward_curve_id("USD-SOFR-6M");

    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.85)])
        .build()
        .unwrap();
    let low_forward_curve = ForwardCurve::builder("USD-SOFR-6M", 0.5)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.01), (5.0, 0.01)])
        .build()
        .unwrap();
    let market_with_forward = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(discount_curve.clone())
        .insert(low_forward_curve);
    let market_without_forward =
        finstack_quant_core::market_data::context::MarketContext::new().insert(discount_curve);

    let with_forward = bond
        .price_with_metrics(
            &market_with_forward,
            as_of,
            &[MetricId::ASWMarket],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("ASW with configured forward curve should compute")
        .measures["asw_market"];

    bond.instrument_pricing_overrides
        .model_config
        .asw_forward_curve_id = None;
    let discount_proxy = bond
        .price_with_metrics(
            &market_without_forward,
            as_of,
            &[MetricId::ASWMarket],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("discount-proxy ASW should compute")
        .measures["asw_market"];

    assert!(
        (with_forward - discount_proxy).abs() > 1e-3,
        "ASW market should use the configured forward curve: with_forward={with_forward}, discount_proxy={discount_proxy}"
    );
}

#[test]
fn test_asw_market_falls_back_to_bond_forward_curve_id() {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};

    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "ASW-BOND-FORWARD-FALLBACK",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    bond.forward_curve_id = Some("USD-SOFR-6M".into());
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(98.0);

    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.85)])
        .build()
        .unwrap();
    let low_forward_curve = ForwardCurve::builder("USD-SOFR-6M", 0.5)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.01), (5.0, 0.01)])
        .build()
        .unwrap();
    let high_forward_curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.04), (5.0, 0.04)])
        .build()
        .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(discount_curve)
        .insert(low_forward_curve)
        .insert(high_forward_curve);

    let fallback = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ASWMarket],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("ASW with bond forward curve fallback should compute")
        .measures["asw_market"];

    bond.instrument_pricing_overrides
        .model_config
        .asw_forward_curve_id = Some("USD-SOFR-3M".into());
    let explicit_override = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ASWMarket],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("ASW explicit forward override should compute")
        .measures["asw_market"];

    assert!(
        (fallback - explicit_override).abs() > 1e-3,
        "ASW should use bond.forward_curve_id only when asw_forward_curve_id is absent: fallback={fallback}, explicit_override={explicit_override}"
    );
}

#[test]
fn test_oas_metric_uses_bond_tree_pricing_overrides() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};

    let as_of = date!(2025 - 01 - 01);
    let mut base_bond = Bond::fixed(
        "OAS-CONFIG",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2032 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    base_bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2028 - 01 - 01),
            end_date: date!(2028 - 01 - 01),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (7.0, 0.78)])
        .build()
        .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new().insert(curve);

    let mut low_vol_bond = base_bond.clone();
    low_vol_bond.instrument_pricing_overrides = InstrumentPricingOverrides::default()
        .with_quoted_clean_price(99.0)
        .with_implied_vol(0.001);

    let mut high_vol_bond = base_bond;
    high_vol_bond.instrument_pricing_overrides = InstrumentPricingOverrides::default()
        .with_quoted_clean_price(99.0)
        .with_implied_vol(0.05);

    let low = low_vol_bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Oas],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("low-vol OAS should price")
        .measures["oas"];
    let high = high_vol_bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Oas],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("high-vol OAS should price")
        .measures["oas"];

    assert!(
        (low - high).abs() > 1e-6,
        "OAS metric should respond to bond tree volatility overrides: low={low}, high={high}"
    );
}

#[test]
fn test_oas_metric_uses_tree_discount_curve_override() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};

    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "OAS-TREE-CURVE",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2032 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    bond.settlement_convention = None;
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2028 - 04 - 01),
            end_date: date!(2028 - 04 - 01),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond.instrument_pricing_overrides = InstrumentPricingOverrides::default()
        .with_quoted_clean_price(99.0)
        .with_implied_vol(0.01)
        .with_tree_discount_curve_id("USD-TREE");

    let pricing_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (7.0, 0.78)])
        .build()
        .unwrap();
    let tree_curve = DiscountCurve::builder("USD-TREE")
        .base_date(as_of)
        .knots([(0.0, 1.0), (7.0, 0.92)])
        .build()
        .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(pricing_curve)
        .insert(tree_curve);

    let with_override = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Oas],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("OAS with tree curve override should price")
        .measures["oas"];

    bond.instrument_pricing_overrides
        .model_config
        .tree_discount_curve_id = None;
    let without_override = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Oas],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("OAS without tree curve override should price")
        .measures["oas"];

    assert!(
        (with_override - without_override).abs() > 1e-4,
        "OAS should use the configured tree discount curve: with_override={with_override}, without_override={without_override}"
    );
}

#[test]
fn test_embedded_option_value_uses_solved_oas_and_holder_sign() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "EMBEDDED-OAS-BASIS",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2032 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    bond.settlement_convention = None;
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2028 - 01 - 01),
            end_date: date!(2028 - 01 - 01),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond.instrument_pricing_overrides = InstrumentPricingOverrides::default()
        .with_quoted_clean_price(103.0)
        .with_implied_vol(0.02);

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (7.0, 0.82)])
        .build()
        .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new().insert(curve);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Oas, MetricId::EmbeddedOptionValue],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("embedded option value should compute");
    let oas = result.measures["oas"];
    let actual = result.measures["embedded_option_value"];

    let mut straight_bond = bond.clone();
    straight_bond.call_put = Some(CallPutSchedule::default());
    let expected = price_from_oas(&bond, &market, as_of, oas).expect("callable OAS price")
        - price_from_oas(&straight_bond, &market, as_of, oas).expect("straight OAS price");

    assert!(
        (actual - expected).abs() < 1e-6,
        "embedded option value should be holder-view model price difference at solved OAS: actual={actual}, expected={expected}, oas={oas}"
    );
    assert!(
        actual < 0.0,
        "callable bond embedded option value should be negative from holder perspective, got {actual}"
    );
}

#[test]
fn test_embedded_option_value_uses_settlement_date_oas_pricing_basis() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    let as_of = date!(2025 - 01 - 02);
    let quote_date = date!(2025 - 01 - 07);
    let quoted_oas = 0.0065;
    let mut bond = Bond::fixed(
        "EMBEDDED-QUOTE-DATE-BASIS",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2032 - 01 - 02),
        "USD-OIS",
    )
    .unwrap();
    bond.settlement_convention = Some(BondSettlementConvention {
        settlement_days: 3,
        ex_coupon_days: 0,
        ex_coupon_calendar_id: None,
    });
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2028 - 01 - 02),
            end_date: date!(2028 - 01 - 02),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond.instrument_pricing_overrides = serde_json::from_value(serde_json::json!({
        "quoted_oas": quoted_oas,
        "implied_volatility": 0.20,
        "tree_steps": 80,
        "vol_model": "black",
        "mean_reversion": 0.0
    }))
    .expect("BDT pricing overrides should deserialize");

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(quote_date)
        .knots([(0.0, 1.0), (7.0, 0.82)])
        .build()
        .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new().insert(curve);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::EmbeddedOptionValue],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("embedded option value should compute");
    let actual = result.measures["embedded_option_value"];

    let mut straight_bond = bond.clone();
    straight_bond.call_put = Some(CallPutSchedule::default());
    let expected = price_from_oas(&bond, &market, quote_date, quoted_oas)
        .expect("callable quote-date OAS price")
        - price_from_oas(&straight_bond, &market, quote_date, quoted_oas)
            .expect("straight quote-date OAS price");

    assert!(
        (actual - expected).abs() < 1e-6,
        "embedded option value should use quote-date OAS pricing basis: actual={actual}, expected={expected}"
    );
}

#[test]
fn test_callable_bond_vega_is_registered_and_bumps_implied_volatility() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "CALLABLE-VEGA",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2032 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2028 - 01 - 01),
            end_date: date!(2028 - 01 - 01),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond.instrument_pricing_overrides = InstrumentPricingOverrides::default()
        .with_quoted_clean_price(103.0)
        .with_implied_vol(0.02);

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (7.0, 0.82)])
        .build()
        .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new().insert(curve);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("callable bond vega should compute");
    let vega = *result
        .measures
        .get("vega")
        .expect("bond vega should be registered");

    assert!(vega.is_finite(), "vega should be finite, got {vega}");
    assert!(
        vega < 0.0,
        "callable bond holder-view vega should be negative because higher volatility increases issuer call value, got {vega}"
    );
}

#[test]
fn test_callable_bond_oas_and_vega_use_explicit_bdt_tree_path() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "CALLABLE-BDT-OAS-VEGA",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2032 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2028 - 01 - 01),
            end_date: date!(2028 - 01 - 01),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond.instrument_pricing_overrides = serde_json::from_value(serde_json::json!({
        "quoted_clean_price": 103.0,
        "implied_volatility": 0.20,
        "tree_steps": 40,
        "vol_model": "black",
        "mean_reversion": 0.0
    }))
    .expect("BDT pricing overrides should deserialize");

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (7.0, 0.82)])
        .build()
        .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new().insert(curve);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Oas, MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("explicit BDT callable OAS and vega should compute");

    let oas = result.measures["oas"];
    let vega = result.measures["vega"];
    assert!(oas.is_finite(), "BDT OAS should be finite, got {oas}");
    assert!(vega.is_finite(), "BDT vega should be finite, got {vega}");
    assert!(
        vega < 0.0,
        "holder-view callable BDT vega should be negative, got {vega}"
    );
}

#[test]
fn test_callable_bond_vega_is_invariant_to_vol_bump_size() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    fn vega_with_bump(
        base_bond: &Bond,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        bump: f64,
    ) -> f64 {
        let mut bond = base_bond.clone();
        bond.metric_pricing_overrides = bond.metric_pricing_overrides.with_vol_bump(bump);
        bond.price_with_metrics(
            market,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("callable bond vega should compute")
        .measures["vega"]
    }

    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "CALLABLE-VEGA-BUMP-INVARIANT",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2032 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2028 - 01 - 01),
            end_date: date!(2028 - 01 - 01),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond.instrument_pricing_overrides = serde_json::from_value(serde_json::json!({
        "quoted_clean_price": 103.0,
        "implied_volatility": 0.20,
        "implied_volatility": 0.20,
        "tree_steps": 40,
        "vol_model": "black",
        "mean_reversion": 0.0
    }))
    .expect("BDT pricing overrides should deserialize");

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (7.0, 0.82)])
        .build()
        .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new().insert(curve);

    let vega_half_point = vega_with_bump(&bond, &market, as_of, 0.005);
    let vega_one_point = vega_with_bump(&bond, &market, as_of, 0.01);
    let scale = vega_one_point.abs().max(1e-12);

    assert!(
        (vega_half_point - vega_one_point).abs() / scale < 0.05,
        "vega should be normalized per one vol point, not scale with finite-difference bump: half_point={vega_half_point}, one_point={vega_one_point}"
    );
}

#[test]
fn test_callable_bdt_oas_recovers_settlement_date_clean_price() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    let as_of = date!(2025 - 01 - 02);
    let quote_date = date!(2025 - 01 - 07);
    let target_oas = 0.0065;
    let notional = Money::new(1_000_000.0, Currency::USD);
    let mut bond = Bond::fixed(
        "CALLABLE-BDT-QUOTE-DATE-OAS",
        notional,
        0.05,
        as_of,
        date!(2032 - 01 - 02),
        "USD-OIS",
    )
    .unwrap();
    bond.settlement_convention = Some(BondSettlementConvention {
        settlement_days: 3,
        ex_coupon_days: 0,
        ex_coupon_calendar_id: None,
    });
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2028 - 01 - 02),
            end_date: date!(2028 - 01 - 02),
            price_pct_of_par: 150.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond.instrument_pricing_overrides = serde_json::from_value(serde_json::json!({
        "implied_volatility": 0.20,
        "tree_steps": 80,
        "vol_model": "black",
        "mean_reversion": 0.0
    }))
    .expect("BDT pricing overrides should deserialize");

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(quote_date)
        .knots([(0.0, 1.0), (7.0, 0.82)])
        .build()
        .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new().insert(curve);
    let dirty_at_quote =
        price_from_oas(&bond, &market, quote_date, target_oas).expect("quote-date OAS price");
    let schedule = bond
        .cashflow_schedule(&market, quote_date)
        .expect("cashflow schedule");
    let accrued_at_quote = finstack_quant_cashflows::accrued_interest_amount(
        &schedule,
        quote_date,
        &bond.accrual_config(),
    )
    .expect("quote-date accrued");
    let quoted_clean_price = (dirty_at_quote - accrued_at_quote) / notional.amount() * 100.0;
    bond.instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price = Some(quoted_clean_price);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Oas],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("OAS metric should solve from quote-date clean price");
    let actual_oas = result.measures["oas"];

    assert!(
        (actual_oas - target_oas).abs() < 1e-6,
        "OAS should recover quote-date target: actual={actual_oas}, target={target_oas}, clean={quoted_clean_price}"
    );
}

#[test]
fn test_callable_bond_value_uses_same_bdt_tree_dispatch_as_oas_pricer() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "CALLABLE-BDT-VALUE",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2032 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2028 - 01 - 01),
            end_date: date!(2028 - 01 - 01),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond.instrument_pricing_overrides = serde_json::from_value(serde_json::json!({
        "implied_volatility": 0.20,
        "tree_steps": 40,
        "vol_model": "black",
        "mean_reversion": 0.0
    }))
    .expect("BDT pricing overrides should deserialize");

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (7.0, 0.82)])
        .build()
        .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new().insert(curve);

    let direct_value = bond
        .value(&market, as_of)
        .expect("direct value should price");
    let canonical_tree_value =
        price_from_oas(&bond, &market, as_of, 0.0).expect("canonical tree value should price");

    assert!(
        (direct_value.amount() - canonical_tree_value).abs() < 1e-6,
        "direct callable value should use the same tree dispatch as price_from_oas: direct={}, canonical={}",
        direct_value.amount(),
        canonical_tree_value
    );
}

/// Z-spread should surface a missing discount curve error instead of silently returning 0.0
/// when pricing fails inside the root-finding objective (e.g., missing discount curve).
#[test]
fn test_z_spread_missing_discount_curve_returns_error() {
    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "ZSPR-MISSING-DC",
        Money::new(100.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(95.0);

    // Market context with NO discount curves – any attempt to build a Z-spread PV should fail
    let market = finstack_quant_core::market_data::context::MarketContext::new();

    // Minimal metric context: base value is arbitrary since Z-spread uses quoted clean price
    let base_value = Money::new(100.0, Currency::USD);
    let mut mctx = MetricContext::new(
        Arc::new(bond),
        Arc::new(market),
        as_of,
        base_value,
        MetricContext::default_config(),
    );

    // Pre-populate accrued to bypass the metric dependency and force the failure into
    // the Z-spread pricing helper (missing discount curve), not missing accrued.
    mctx.computed.insert(MetricId::Accrued, 0.0);

    let calc = ZSpreadCalculator::default();
    let result = calc.calculate(&mut mctx);

    // Expect a propagated input error (missing discount curve), never an apparent "perfect fit" z=0.0.
    match result {
        Err(Error::Input(InputError::MissingCurve { requested, .. })) => {
            assert!(
                requested.contains("USD-OIS"),
                "expected missing discount curve id to mention USD-OIS, got {}",
                requested
            );
        }
        Err(e) => panic!(
            "expected InputError::MissingCurve for missing discount curve, got {}",
            e
        ),
        Ok(z) => panic!(
            "expected Z-spread calculation to fail for missing discount curve, but got z={}",
            z
        ),
    }
}

/// Z-spread round-trip when a coupon falls exactly on the settlement/quote date.
///
/// `ZSpreadCalculator` filters cashflows with `d > quote_date` (strictly after),
/// so it DROPS a coupon dated exactly on `quote_date`. `price_from_z_spread` must
/// apply the same filter — `d <= as_of` skip — so the forward solve and inversion
/// are anchored at the same set of cashflows and round-trip without error.
///
/// Setup: bond issued and priced on 2025-01-01 with no settlement lag, whose
/// first annual coupon falls exactly on that date. Any remaining coupons beyond
/// `as_of` are unaffected; only the on-date coupon is the discriminator.
#[test]
fn test_z_spread_roundtrip_coupon_exactly_on_settlement_date() {
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_z_spread;
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;

    // The settlement/quote date equals `as_of` because no settlement lag is set.
    let as_of = date!(2025 - 01 - 01);
    // Issue one year before so there is a coupon falling exactly on `as_of`.
    let issue_date = date!(2024 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let notional = Money::new(1_000_000.0, Currency::USD);
    let coupon_rate = 0.05;

    let mut bond = Bond::fixed(
        "ZSPR-ON-DATE-COUPON",
        notional,
        coupon_rate,
        issue_date,
        maturity,
        "USD-OIS",
    )
    .expect("bond should build");
    // Explicit annual day-count to make the on-date coupon unambiguous.
    bond.cashflow_spec =
        CashflowSpec::fixed_rate(coupon_rate.into(), Tenor::annual(), DayCount::Act365F)
            .expect("finite test coupon");
    // No settlement lag: quote_date == as_of.
    bond.settlement_convention = None;

    // Flat discount curve based at `as_of`.
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (6.0, 0.82)])
        .build()
        .expect("curve");
    let market = MarketContext::new().insert(disc);

    // Quote the bond at a clean price that is off-par so the Z-spread is non-zero.
    let clean_pct = 98.5_f64;
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(clean_pct);

    // Forward path: solve Z-spread from the quoted clean price.
    let res = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ZSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("Z-spread metric should solve");
    let z = *res.measures.get("z_spread").expect("z_spread measure");

    // Inversion path: recover dirty price from z.
    // `price_from_z_spread` is called with `as_of` as the time origin — exactly
    // the same anchor the ZSpreadCalculator used for its objective function.
    let dirty_inverted =
        price_from_z_spread(&bond, &market, as_of, z).expect("price_from_z_spread should invert");

    // The ZSpreadCalculator target was: dirty = clean% * notional / 100 + accrued.
    // Accrued at `as_of` is zero for an annual bond on its coupon date, so:
    // target_dirty = clean_pct * notional / 100.
    let accrued_at_as_of = 0.0_f64; // first coupon is exactly on as_of, accrual resets.
    let target_dirty = clean_pct * notional.amount() / 100.0 + accrued_at_as_of;

    // Round-trip tolerance: 1e-8 currency units (well below $0.01 per $1M).
    assert!(
        (dirty_inverted - target_dirty).abs() < 1e-8,
        "Z-spread price round-trip mismatch for coupon on settlement date: \
         target_dirty={target_dirty:.10}, inverted={dirty_inverted:.10}, \
         error={:.6e}",
        (dirty_inverted - target_dirty).abs()
    );
}

/// YTM price round-trip when settlement lag places the quote date after the
/// valuation date.
///
/// `price_from_ytm` (the quote-override inversion) must use the same cashflow
/// set and time origin as the YTM metric solver so that `solve_YTM(flows,
/// quote_date, dirty) = y` implies `price_from_ytm(flows, quote_date, y) =
/// dirty`.
///
/// Both paths build flows via `pricing_dated_cashflows(as_of=trade_date)` and
/// anchor discounting at `quote_date`, so any flow in `(trade_date, quote_date]`
/// must be filtered by both in the same way.  `price_from_ytm_compounded_params`
/// already skips `date <= as_of (= quote_date)`, which is the correct behaviour.
/// This test documents and guards that symmetry.
#[test]
fn test_ytm_roundtrip_settlement_lag_two_days() {
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::{
        compute_quotes, BondQuoteInput,
    };
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;

    // Trade date / valuation date.
    let as_of = date!(2024 - 12 - 30);
    // T+2 settlement: quote_date = 2025-01-01.
    let quote_date = date!(2025 - 01 - 01);
    // Bond with a coupon exactly on quote_date: tests flows in (as_of, quote_date].
    let issue_date = date!(2024 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let notional = Money::new(1_000_000.0, Currency::USD);
    let coupon_rate = 0.05;

    let mut bond = Bond::fixed(
        "YTM-SETTLE-LAG",
        notional,
        coupon_rate,
        issue_date,
        maturity,
        "USD-OIS",
    )
    .expect("bond should build");
    bond.cashflow_spec =
        CashflowSpec::fixed_rate(coupon_rate.into(), Tenor::annual(), DayCount::Act365F)
            .expect("finite test coupon");
    // 2 settlement days so that quote_date = as_of + 2 weekdays = 2025-01-01.
    bond.settlement_convention = Some(BondSettlementConvention {
        settlement_days: 2,
        ex_coupon_days: 0,
        ex_coupon_calendar_id: None,
    });

    // Discount curve based at quote_date (standard for settlement-date pricing).
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(quote_date)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (6.0, 0.82)])
        .build()
        .expect("curve");
    let market = MarketContext::new().insert(disc);

    // Choose an explicit YTM and derive the implied clean price.
    let target_ytm = 0.055_f64;
    let quotes = compute_quotes(&bond, &market, as_of, BondQuoteInput::Ytm(target_ytm))
        .expect("compute_quotes from YTM should succeed");
    let clean_pct = quotes.clean_price_pct;

    // Feed the clean price back and re-solve YTM.
    let mut bond_with_price = bond;
    bond_with_price.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(clean_pct);
    let res = bond_with_price
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("YTM metric should solve");
    let recovered_ytm = *res.measures.get("ytm").expect("ytm measure");

    // Round-trip tolerance: 0.01 bp = 1e-6 in decimal.
    assert!(
        (recovered_ytm - target_ytm).abs() < 1e-6,
        "YTM price round-trip mismatch with settlement lag: \
         target={target_ytm:.10}, recovered={recovered_ytm:.10}, \
         error={:.6e}",
        (recovered_ytm - target_ytm).abs()
    );
}

/// Z-spread solver should converge for IG, HY, and distressed fixed-rate bonds
/// with realistic spreads up to ~3000 bp and maintain tight price residuals.
#[test]
fn test_z_spread_solver_convergence_across_spread_regimes() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_valuations::instruments::InstrumentPricingOverrides;

    let as_of = date!(2025 - 01 - 01);
    let maturity_ig = date!(2028 - 01 - 01); // shorter IG
    let maturity_hy = date!(2032 - 01 - 01); // medium HY
    let maturity_distressed = date!(2035 - 01 - 01); // longer distressed
    let notional = Money::new(1_000_000.0, Currency::USD);

    // Simple discount curve; Z-spread will be applied as an exponential shift.
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.7)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let market = MarketContext::new().insert(disc);

    let bond_ig = Bond::fixed(
        "ZSPR-CONV-IG",
        notional,
        0.03,
        as_of,
        maturity_ig,
        "USD-OIS",
    )
    .unwrap();
    let bond_hy = Bond::fixed(
        "ZSPR-CONV-HY",
        notional,
        0.06,
        as_of,
        maturity_hy,
        "USD-OIS",
    )
    .unwrap();
    let bond_distressed = Bond::fixed(
        "ZSPR-CONV-DIST",
        notional,
        0.10,
        as_of,
        maturity_distressed,
        "USD-OIS",
    )
    .unwrap();

    // (target z-spread, bond) scenarios from IG through distressed.
    let scenarios: Vec<(f64, Bond)> = vec![
        (0.01, bond_ig),         // 100 bp IG
        (0.07, bond_hy),         // 700 bp HY
        (0.30, bond_distressed), // 3000 bp distressed
    ];

    for (target_z, base_bond) in scenarios {
        // `price_from_z_spread` takes the valuation `as_of` and derives the
        // settlement (`quote_date`) origin internally. For these bonds the
        // 2-business-day settlement of a Wednesday `as_of` lands on Friday,
        // which coincides with `as_of + 2` calendar days used for the accrued
        // reconstruction below.
        let settlement_days = base_bond.settlement_days().unwrap_or(0) as i64;
        let quote_date = as_of + time::Duration::days(settlement_days);

        // Price the bond at the target Z-spread to obtain a dirty price.
        let dirty_target =
            finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_z_spread(
                &base_bond, &market, as_of, target_z,
            )
            .expect("pricing with target Z-spread should succeed");

        // Convert to a clean price (% of par) at the quote/settlement date
        // (dirty = clean + accrued at quote_date).
        // Accrued must be computed at the quote/settlement date, not `as_of`.
        let schedule = base_bond
            .cashflow_schedule(&market, quote_date)
            .expect("build full schedule");
        let accrued = finstack_quant_cashflows::accrued_interest_amount(
            &schedule,
            quote_date,
            &base_bond.accrual_config(),
        )
        .expect("accrued at quote date");
        let clean_ccy = dirty_target - accrued;
        let clean_px = clean_ccy / notional.amount() * 100.0;

        let mut bond = base_bond.clone();
        bond.instrument_pricing_overrides =
            InstrumentPricingOverrides::default().with_quoted_clean_price(clean_px);

        // Run Z-spread metric via the normal pipeline.
        let result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::ZSpread],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .expect("Z-spread metric should converge for realistic spreads");
        let z = *result
            .measures
            .get("z_spread")
            .expect("z_spread measure should be present");

        assert!(
            (z - target_z).abs() < 5e-8,
            "Z-spread solver should recover target z (target={}, got={})",
            target_z,
            z
        );

        // Re-price with solved z and verify price residual is tiny.
        let dirty_repriced =
            finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_z_spread(
                &bond, &market, as_of, z,
            )
            .expect("repricing with solved Z-spread should succeed");
        let price_error = (dirty_repriced - dirty_target).abs() / notional.amount();

        assert!(
            price_error < 1e-6,
            "Price residual should be < 1e-6 * notional, got {}",
            price_error
        );
    }
}

/// Item 2 regression: the documented `Z-spread metric -> price_from_z_spread`
/// round-trip must hold for a bond **with a settlement lag**.
///
/// The `ZSpreadCalculator` solves the spread on the settlement (`quote_date`)
/// time axis. `price_from_z_spread` takes the valuation `as_of` and must derive
/// the same settlement origin internally, so that
/// `price_from_z_spread(bond, market, as_of, solve(...)) == dirty_target`
/// even when `quote_date != as_of`.
#[test]
fn test_z_spread_roundtrip_with_settlement_lag() {
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_z_spread;
    use finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec;

    let as_of = date!(2025 - 01 - 06);
    let issue_date = date!(2023 - 01 - 06);
    let maturity = date!(2030 - 01 - 06);
    let notional = Money::new(1_000_000.0, Currency::USD);

    let mut bond = Bond::fixed(
        "ZSPR-SETTLE-LAG",
        notional,
        0.05,
        issue_date,
        maturity,
        "USD-OIS",
    )
    .expect("bond should build");
    bond.cashflow_spec = CashflowSpec::fixed_rate(0.05.into(), Tenor::annual(), DayCount::Act365F)
        .expect("finite test coupon");
    // 3-business-day settlement lag => quote_date strictly after as_of.
    bond.settlement_convention = Some(BondSettlementConvention {
        settlement_days: 3,
        ..Default::default()
    });

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (7.0, 0.80)])
        .build()
        .expect("curve");
    let market = MarketContext::new().insert(disc);

    // Quote the bond off-par so the Z-spread is non-zero.
    let clean_pct = 97.25_f64;
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(clean_pct);

    // Forward path: solve Z-spread and read the settlement-anchored dirty price
    // (the metric `DirtyPrice` is exactly the ZSpreadCalculator's solve target:
    // clean quote + accrued at settlement).
    let res = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ZSpread, MetricId::DirtyPrice],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("Z-spread metric should solve");
    let z = *res.measures.get("z_spread").expect("z_spread measure");
    let target_dirty = *res
        .measures
        .get("dirty_price")
        .expect("dirty_price measure");

    // Inversion path: recover the dirty price from z, passing the *valuation*
    // `as_of`. This is the function's documented contract (its parameter is
    // named `as_of`). It must round-trip despite the settlement lag.
    let dirty_inverted =
        price_from_z_spread(&bond, &market, as_of, z).expect("price_from_z_spread should invert");

    // Residual tolerance scaled to notional: the Z-spread solver tolerance
    // (1e-10 on the spread axis) maps to a price residual of roughly
    // duration * notional * 1e-10 ~ 1e-3, so 1e-6 * notional is comfortable.
    let price_error = (dirty_inverted - target_dirty).abs() / notional.amount();
    assert!(
        price_error < 1e-6,
        "Z-spread price round-trip must hold under a settlement lag: \
         target_dirty={target_dirty:.6}, inverted={dirty_inverted:.6}, \
         relative_error={price_error:.6e}"
    );
}

/// Issue A regression: `price_from_z_spread` must return `Err` (not `Ok(INFINITY)` or `Ok(NaN)`)
/// when the z-spread is so extremely negative that the compounding denominator
/// `1 + (base_rate + z) / m` goes non-positive.
///
/// Before the fix, `z_spread_discount_factor` silently returned `f64::INFINITY` on a
/// non-positive denominator. `price_from_z_spread` accumulated the infinity into the
/// `NeumaierAccumulator` and returned `Ok(f64::INFINITY)` — a wrong-but-non-finite
/// result with no error signal to the caller.
#[test]
fn test_z_spread_negative_denom_returns_err_not_infinity() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_z_spread;

    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let notional = Money::new(1_000_000.0, Currency::USD);

    let mut bond = Bond::fixed("ZSPR-NEG-DENOM", notional, 0.05, as_of, maturity, "USD-OIS")
        .expect("bond should build");
    bond.settlement_convention = None;

    // Flat curve at 3% -> base_rate ≈ 0.03 for each cashflow.
    // With m = 1, denom = 1 + (0.03 + z). Setting z = -2.0 (-200%) makes
    // denom = 1 + (0.03 - 2.0) = -0.97 <= 0.  price_from_z_spread should
    // propagate this as Err, not return Ok(INFINITY).
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.861)])
        .build()
        .expect("curve");
    let market = MarketContext::new().insert(disc);

    // z = -5.0 drives the compounding denominator well below zero for m=2
    // (semi-annual, the default for Bond::fixed):
    //   denom = 1 + (base_rate + z) / m ≈ 1 + (0.03 - 5.0) / 2 = -1.485
    let extreme_negative_z = -5.0_f64;
    let result = price_from_z_spread(&bond, &market, as_of, extreme_negative_z);

    assert!(
        result.is_err(),
        "price_from_z_spread with a non-positive compounding denominator must return Err, \
         not Ok(INFINITY) or Ok(NaN). Got: {:?}",
        result
    );
}

/// Issue A regression (solver path): when every cashflow's z-spread discount factor
/// is non-finite because the z-spread bracket extends into the non-positive denominator
/// regime, the `ZSpreadCalculator` solver must surface a clear error — not silently
/// return a spurious finite spread caused by an opaque bracket failure.
///
/// We verify this by quoting a bond at a price that can only be matched by a z-spread
/// so extremely negative that the compounding denominator goes non-positive for every
/// candidate z in the Brent bracket.  The solver must return `Err`.
#[test]
fn test_z_spread_solver_non_positive_base_df_returns_err() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    let as_of = date!(2025 - 01 - 01);
    let notional = Money::new(1_000_000.0, Currency::USD);

    let mut bond = Bond::fixed(
        "ZSPR-BAD-DF",
        notional,
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .expect("bond should build");
    bond.settlement_convention = None;

    // Healthy curve — the z-spread solver will scan both sides of the bracket.
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.861)])
        .build()
        .expect("curve");
    let market = MarketContext::new().insert(disc);

    // Quote an astronomically high price (10× par): no physically valid spread
    // can match this — the solver must fail rather than return a meaningless z.
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(10_000.0_f64);

    let result = bond.price_with_metrics(
        &market,
        as_of,
        &[MetricId::ZSpread],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    assert!(
        result.is_err(),
        "Z-spread solver against an unreachable price target must return Err, \
         not a meaningless finite spread. Got: {:?}",
        result
    );
}
