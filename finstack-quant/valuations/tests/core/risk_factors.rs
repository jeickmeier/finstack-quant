//! Risk-factor extraction over built-in instruments.

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_valuations::metrics::risk::{extract_risk_factors, RiskFactorType};
use finstack_quant_valuations::metrics::STANDARD_BUCKETS_YEARS;

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use time::macros::date;

#[test]
fn test_risk_factor_label() {
    let factor = RiskFactorType::DiscountRate {
        curve_id: CurveId::from("USD-OIS"),
        tenor_years: 5.0,
    };
    assert_eq!(factor.label(), "USD-OIS::disc::5y");

    let factor = RiskFactorType::CreditSpread {
        curve_id: CurveId::from("AAPL"),
        tenor_years: 10.0,
    };
    assert_eq!(factor.label(), "AAPL::credit::10y");

    let factor = RiskFactorType::ForwardRate {
        curve_id: CurveId::from("USD-LIBOR-3M"),
        tenor_years: 2.0,
    };
    assert_eq!(factor.label(), "USD-LIBOR-3M::fwd::2y");

    let factor = RiskFactorType::FxSpot {
        base: Currency::EUR,
        quote: Currency::USD,
    };
    assert_eq!(factor.label(), "EURUSD::fx_spot");
}

#[test]
fn test_extract_discount_factors_from_bond() -> Result<()> {
    use finstack_quant_valuations::instruments::Bond;

    let as_of = date!(2024 - 01 - 01);

    // Use Bond::fixed factory method
    let bond = Bond::fixed(
        "TEST-BOND",
        Money::from((100_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        date!(2029 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )?;

    // Create market with discount curve
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots(vec![(0.0, 1.0), (1.0, 0.96), (5.0, 0.85), (10.0, 0.70)])
        .build()?;

    let market = MarketContext::new().insert(curve);

    // Extract risk factors
    let factors = extract_risk_factors(&bond, &market)?;

    // Should have discount rate factors at standard tenors
    assert!(!factors.is_empty(), "Should extract risk factors");

    // Verify we have discount rate factors
    let discount_factors: Vec<_> = factors
        .iter()
        .filter_map(|f| match f {
            RiskFactorType::DiscountRate {
                curve_id,
                tenor_years,
            } => Some((curve_id.as_str(), *tenor_years)),
            _ => None,
        })
        .collect();

    assert!(
        !discount_factors.is_empty(),
        "Should have discount rate factors"
    );
    assert!(
        discount_factors.iter().all(|(cid, _)| *cid == "USD-OIS"),
        "All factors should be for USD-OIS curve"
    );

    // Verify we're using standard tenors
    for (_, tenor) in &discount_factors {
        assert!(
            STANDARD_BUCKETS_YEARS.contains(tenor),
            "Tenor {} should be in standard bucket list",
            tenor
        );
    }

    Ok(())
}

#[test]
fn test_extract_factors_empty_market() -> Result<()> {
    use finstack_quant_valuations::instruments::Bond;

    let as_of = date!(2024 - 01 - 01);
    let bond = Bond::fixed(
        "TEST-BOND",
        Money::from((100_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        date!(2029 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )?;

    // Empty market - curve exists in instrument but not in market
    let market = MarketContext::new();

    let factors = extract_risk_factors(&bond, &market)?;

    // Should return empty vector when curves don't exist
    assert!(
        factors.is_empty(),
        "Should have no factors for empty market"
    );

    Ok(())
}

#[test]
fn test_extract_equity_and_vol_factors() -> Result<()> {
    use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;

    let expiry = date!(2025 - 06 - 01);
    let option = EquityOption::builder()
        .id(finstack_quant_core::types::InstrumentId::new("EQO"))
        .underlying_ticker("AAPL".to_string())
        .strike(100.0)
        .option_type(finstack_quant_valuations::instruments::OptionType::Call)
        .exercise_style(finstack_quant_valuations::instruments::ExerciseStyle::European)
        .expiry(expiry)
        .quantity(100.0)
        .currency(Currency::USD)
        .day_count(DayCount::Act365F)
        .settlement(finstack_quant_valuations::instruments::SettlementType::Cash)
        .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
        .spot_id("EQUITY-SPOT".into())
        .vol_surface_id(finstack_quant_core::types::CurveId::new("EQUITY-VOL"))
        .div_yield_id_opt(Some(finstack_quant_core::types::PriceId::new(
            "EQUITY-DIVYIELD",
        )))
        .instrument_pricing_overrides(
            finstack_quant_valuations::instruments::InstrumentPricingOverrides::default(),
        )
        .attributes(finstack_quant_valuations::instruments::Attributes::new())
        .build()?;

    let base_date = date!(2024 - 01 - 01);
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots(vec![(0.0, 1.0), (1.0, 0.98)])
        .build()?;

    let market = MarketContext::new()
        .insert(curve)
        .insert_price(&option.spot_id, MarketScalar::Unitless(150.0))
        .insert_surface(
            finstack_quant_core::market_data::surfaces::VolSurface::builder(
                option.vol_surface_id.clone(),
            )
            .expiries(&[0.5, 1.0])
            .strikes(&[90.0, 100.0])
            .row(&[0.24, 0.25])
            .row(&[0.26, 0.27])
            .build()?,
        );

    let factors = extract_risk_factors(&option, &market)?;
    let nodes: Vec<_> = factors
        .iter()
        .filter(|factor| matches!(factor, RiskFactorType::ImpliedVol { .. }))
        .cloned()
        .collect();
    assert_eq!(nodes.len(), 4);
    for factor in nodes {
        let scenario = finstack_quant_valuations::metrics::risk::MarketScenario::new(
            base_date,
            vec![finstack_quant_valuations::metrics::risk::RiskFactorShift {
                factor,
                shift: 0.01,
            }],
        );
        scenario
            .apply(&market, None)
            .expect("every extracted volatility node must replay");
    }

    assert!(
        factors.iter().any(
            |f| matches!(f, RiskFactorType::EquitySpot { ticker } if ticker == option.spot_id.as_str())
        ),
        "should include equity spot factor"
    );
    assert!(
        factors.iter().any(|f| matches!(f, RiskFactorType::ImpliedVol { vol_surface_id, .. } if vol_surface_id == &option.vol_surface_id)),
        "should include vol surface factor"
    );

    Ok(())
}

#[test]
fn test_extract_vol_factors_from_fx_option() -> Result<()> {
    use finstack_quant_valuations::instruments::fx::fx_option::{
        FxDeltaConvention, FxDeltaConventionKind, FxOption,
    };
    use finstack_quant_valuations::instruments::{
        Attributes, InstrumentPricingOverrides, OptionType,
    };

    let as_of = date!(2024 - 01 - 01);
    let option = FxOption::builder()
        .id(finstack_quant_core::types::InstrumentId::new("FXO"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .strike(1.10)
        .option_type(OptionType::Call)
        .delta_convention(
            FxDeltaConvention::new(FxDeltaConventionKind::Forward, Currency::USD, "test")
                .expect("valid delta convention"),
        )
        .expiry(date!(2025 - 01 - 01))
        .day_count(DayCount::Act365F)
        .notional(Money::from((1_000_000_i64, Currency::EUR)))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()?;

    let usd_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots(vec![(0.0, 1.0), (1.0, 0.98)])
        .build()?;
    let eur_curve = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots(vec![(0.0, 1.0), (1.0, 0.99)])
        .build()?;
    let market = MarketContext::new()
        .insert(usd_curve)
        .insert(eur_curve)
        .insert_surface(
            finstack_quant_core::market_data::surfaces::VolSurface::builder("EURUSD-VOL")
                .expiries(&[1.0])
                .strikes(&[1.10])
                .row(&[0.12])
                .build()?,
        );

    let factors = extract_risk_factors(&option, &market)?;

    assert!(
        factors.iter().any(|f| matches!(f, RiskFactorType::ImpliedVol { vol_surface_id, .. } if vol_surface_id == &option.vol_surface_id)),
        "FX option should include its volatility surface factor"
    );
    Ok(())
}
