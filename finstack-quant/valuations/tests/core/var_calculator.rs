//! Historical VaR over built-in instruments.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::risk::var_calculator::relevant_taylor_scenario;
use finstack_quant_valuations::metrics::risk::var_calculator::{
    calculate_var, VarConfig, VarMethod,
};
use finstack_quant_valuations::metrics::risk::{
    MarketHistory, MarketScenario, RiskFactorShift, RiskFactorType,
};
use finstack_quant_valuations::pricer::PricingDispatch;
use std::sync::Arc;
use time::macros::date;

fn sample_as_of() -> Date {
    date!(2024 - 01 - 01)
}

fn usd_ois_curve(as_of: Date) -> Result<DiscountCurve> {
    DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots(vec![(0.0, 1.0), (5.0, 0.85), (10.0, 0.70)])
        .build()
}

fn usd_ois_market(as_of: Date) -> Result<MarketContext> {
    Ok(MarketContext::new().insert(usd_ois_curve(as_of)?))
}

/// Fixed-rate USD bond used by the VaR tests.
fn standard_bond(id: &str, as_of: Date, maturity: Date) -> Bond {
    Bond::fixed(
        id,
        Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("standard_bond should build successfully")
}

/// Single-factor USD-OIS rate scenario.
fn usd_ois_rate_scenario(date: Date, shift: f64) -> MarketScenario {
    MarketScenario::new(
        date,
        vec![RiskFactorShift {
            factor: RiskFactorType::DiscountRate {
                curve_id: CurveId::from("USD-OIS"),
                tenor_years: 5.0,
            },
            shift,
        }],
    )
}

/// Market history from `(date, shift)` pairs on the USD-OIS 5y node.
fn history_from_rate_shifts(as_of: Date, shifts: &[(Date, f64)]) -> MarketHistory {
    let scenarios = shifts
        .iter()
        .map(|(date, shift)| usd_ois_rate_scenario(*date, *shift))
        .collect();
    MarketHistory::new(as_of, shifts.len() as u32, scenarios)
}

fn history_from_scenarios(
    as_of: Date,
    window_days: u32,
    scenarios: Vec<MarketScenario>,
) -> MarketHistory {
    MarketHistory::new(as_of, window_days, scenarios)
}

#[test]
fn taylor_equity_pnl_uses_market_spot_ids_and_aggregates_shared_spots() -> Result<()> {
    use finstack_quant_valuations::instruments::equity::spot::Equity;

    let as_of = sample_as_of();
    let mut a = Equity::new("position-a", "DISPLAY-A", Currency::USD)
        .with_quantity(10.0)
        .with_spot_id("SPOT-A")
        .with_div_yield_id("DIV-A");
    a.discount_curve_id = "USD-OIS".into();
    let mut b = Equity::new("position-b", "DISPLAY-B", Currency::USD)
        .with_quantity(-10.0)
        .with_spot_id("SPOT-B");
    b.discount_curve_id = "USD-OIS".into();
    let mut second_a = a.clone();
    second_a.id = "another-position-a".into();
    second_a.quantity = Some(3.0);
    let market = usd_ois_market(as_of)?
        .insert_price("SPOT-A", MarketScalar::Unitless(100.0))
        .insert_price("SPOT-B", MarketScalar::Unitless(80.0))
        .insert_price("UNRELATED", MarketScalar::Unitless(500.0))
        .insert_price("DIV-A", MarketScalar::Unitless(0.02));
    let history = MarketHistory::new(
        as_of,
        1,
        [("SPOT-A", -0.1), ("SPOT-B", 0.2), ("UNRELATED", -0.3)]
            .into_iter()
            .map(|(ticker, shift)| {
                MarketScenario::new(
                    as_of,
                    vec![RiskFactorShift {
                        factor: RiskFactorType::EquitySpot {
                            ticker: ticker.to_owned(),
                        },
                        shift,
                    }],
                )
            })
            .collect(),
    );

    for (positions, expected) in [
        (vec![&a as &dyn Instrument, &b], vec![-160.0, -100.0, 0.0]),
        (vec![&b as &dyn Instrument], vec![-160.0, 0.0, 0.0]),
        (
            vec![&a as &dyn Instrument, &second_a],
            vec![-130.0, 0.0, 0.0],
        ),
    ] {
        for method in [VarMethod::FullRevaluation, VarMethod::TaylorApproximation] {
            let actual = calculate_var(
                &positions,
                &market,
                &history,
                as_of,
                &VarConfig::var_95().with_method(method),
                PricingDispatch::InstrumentDefault,
                None,
            )?;
            for (actual, expected) in actual.pnl_distribution.iter().zip(&expected) {
                assert!(
                    (actual - expected).abs() < 1e-9,
                    "{method:?}: {actual} != {expected}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn taylor_quoted_equity_has_no_market_spot_sensitivity() -> Result<()> {
    let as_of = sample_as_of();
    let mut equity = finstack_quant_valuations::instruments::equity::spot::Equity::new(
        "position",
        "DISPLAY",
        Currency::USD,
    )
    .with_quantity(10.0)
    .with_spot_id("SPOT")
    .with_quoted_spot(120.0);
    equity.discount_curve_id = "USD-OIS".into();
    let market = usd_ois_market(as_of)?.insert_price("SPOT", MarketScalar::Unitless(100.0));
    let history = MarketHistory::new(
        as_of,
        1,
        vec![MarketScenario::new(
            as_of,
            vec![RiskFactorShift {
                factor: RiskFactorType::EquitySpot {
                    ticker: "SPOT".to_owned(),
                },
                shift: -0.5,
            }],
        )],
    );
    let result = calculate_var(
        &[&equity as &dyn Instrument],
        &market,
        &history,
        as_of,
        &VarConfig::var_95().with_method(VarMethod::TaylorApproximation),
        PricingDispatch::InstrumentDefault,
        None,
    )?;
    assert_eq!(result.pnl_distribution, vec![0.0]);
    Ok(())
}

#[test]
fn taylor_multifactor_instrument_accepts_empty_direction() -> Result<()> {
    let as_of = sample_as_of();
    let basket = finstack_quant_valuations::instruments::exotics::basket::Basket::example()?;
    let market = usd_ois_market(as_of)?
        .insert_price("AAPL-SPOT", MarketScalar::Unitless(100.0))
        .insert_price("UST10Y-PRICE", MarketScalar::Unitless(100.0));
    let history = MarketHistory::new(as_of, 1, vec![MarketScenario::new(as_of, vec![])]);
    let result = calculate_var(
        &[&basket as &dyn Instrument],
        &market,
        &history,
        as_of,
        &VarConfig::var_95().with_method(VarMethod::TaylorApproximation),
        PricingDispatch::InstrumentDefault,
        None,
    )
    .expect("empty direction has zero risk for a multi-asset basket");
    assert_eq!(result.pnl_distribution, vec![0.0]);
    Ok(())
}

#[test]
fn historical_vol_point_revaluation_matches_manual_surface() -> Result<()> {
    let as_of = sample_as_of();
    let option =
        finstack_quant_valuations::instruments::equity::equity_option::EquityOption::european(
            "call",
            "DISPLAY",
            105.0,
            date!(2025 - 01 - 01),
            1.0,
            Currency::USD,
            finstack_quant_models::types::OptionType::Call,
        )?;
    let base_surface = VolSurface::builder("EQUITY-VOL")
        .expiries(&[0.5, 1.5])
        .strikes(&[100.0, 110.0])
        .row(&[0.20, 0.23])
        .row(&[0.24, 0.25])
        .build()?;
    let manual_surface = VolSurface::builder("EQUITY-VOL")
        .expiries(&[0.5, 1.5])
        .strikes(&[100.0, 110.0])
        .row(&[0.21, 0.23])
        .row(&[0.24, 0.23])
        .build()?;
    let market = usd_ois_market(as_of)?
        .insert_price("EQUITY-SPOT", MarketScalar::Unitless(100.0))
        .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(0.0))
        .insert_surface(base_surface);
    let manual_market = market.clone().insert_surface(manual_surface);
    let expected_pnl =
        option.value(&manual_market, as_of)?.amount() - option.value(&market, as_of)?.amount();
    assert!(
        expected_pnl.abs() > 1e-6,
        "fixture must produce a nonzero point-shock P&L"
    );
    let history = MarketHistory::new(
        as_of,
        1,
        vec![MarketScenario::new(
            as_of,
            vec![
                RiskFactorShift {
                    factor: RiskFactorType::ImpliedVol {
                        vol_surface_id: "EQUITY-VOL".into(),
                        expiry_years: 0.5,
                        strike: 100.0,
                    },
                    shift: 0.01,
                },
                RiskFactorShift {
                    factor: RiskFactorType::ImpliedVol {
                        vol_surface_id: "EQUITY-VOL".into(),
                        expiry_years: 1.5,
                        strike: 110.0,
                    },
                    shift: -0.02,
                },
            ],
        )],
    );
    let actual = calculate_var(
        &[&option as &dyn Instrument],
        &market,
        &history,
        as_of,
        &VarConfig::var_95(),
        PricingDispatch::InstrumentDefault,
        None,
    )?;
    assert!((actual.pnl_distribution[0] - expected_pnl).abs() < 1e-12);
    assert_eq!(
        market.get_surface("EQUITY-VOL")?.vols(),
        &[0.20, 0.23, 0.24, 0.25]
    );
    Ok(())
}

#[test]
fn test_taylor_method_matches_full_revaluation_small_shifts() -> Result<()> {
    let as_of = sample_as_of();
    let bond = standard_bond("TEST-BOND", as_of, date!(2029 - 01 - 01));
    let base_market = usd_ois_market(as_of)?;
    let history = history_from_rate_shifts(
        as_of,
        &[
            (date!(2023 - 12 - 31), 0.0005),
            (date!(2023 - 12 - 30), -0.0003),
            (date!(2023 - 12 - 29), 0.0002),
        ],
    );
    let full_config = VarConfig::var_95();
    let taylor_config = VarConfig::var_95().with_method(VarMethod::TaylorApproximation);

    let full = calculate_var(
        &[&bond as &dyn Instrument],
        &base_market,
        &history,
        as_of,
        &full_config,
        PricingDispatch::InstrumentDefault,
        None,
    )?;
    let taylor = calculate_var(
        &[&bond as &dyn Instrument],
        &base_market,
        &history,
        as_of,
        &taylor_config,
        PricingDispatch::InstrumentDefault,
        None,
    )?;

    assert!(
        taylor.var < 0.0,
        "Taylor VaR should be negative (losses-negative convention)"
    );
    let diff = (taylor.var - full.var).abs();
    let rel = diff / full.var.abs().max(1.0);
    assert!(
        rel < 0.15,
        "Taylor VaR should be close to full revaluation (diff: {:.4}%)",
        rel * 100.0
    );

    Ok(())
}

/// Verifies the Taylor VaR shift convention: shifts are in decimal form (e.g., 0.0001 = 1bp)
/// and DV01 is "per basis point", so the P&L formula is:
///
///   P&L = DV01_per_bp × shift_decimal × 10,000
///       = DV01_per_bp × shift_bp
///
/// This test uses a known linear instrument (bond) with a single scenario
/// to verify the scaling convention produces matching results.
#[test]
fn test_taylor_shift_convention_matches_full_revaluation_linear_instrument() -> Result<()> {
    let as_of = sample_as_of();
    // 5-year bond: approximately linear in rate changes for small shifts
    let bond = standard_bond("LINEAR-BOND", as_of, date!(2029 - 01 - 01));
    let base_market = usd_ois_market(as_of)?;

    // Single scenario with a small parallel rate shift (5bp = 0.0005 decimal)
    // This tests the convention: shift.shift is in decimal form
    let shift_decimal = 0.0005; // 5 basis points
    let shift_bp = shift_decimal * 10_000.0; // = 5.0 bp

    let history = history_from_rate_shifts(as_of, &[(date!(2023 - 12 - 31), shift_decimal)]);

    // Full revaluation: direct repricing under shifted curve
    let full_config = VarConfig::var_95();
    let full_result = calculate_var(
        &[&bond as &dyn Instrument],
        &base_market,
        &history,
        as_of,
        &full_config,
        PricingDispatch::InstrumentDefault,
        None,
    )?;

    // Taylor approximation: P&L ≈ DV01 × shift_bp
    let taylor_config = VarConfig::var_95().with_method(VarMethod::TaylorApproximation);
    let taylor_result = calculate_var(
        &[&bond as &dyn Instrument],
        &base_market,
        &history,
        as_of,
        &taylor_config,
        PricingDispatch::InstrumentDefault,
        None,
    )?;

    // With a single scenario, VaR = P&L magnitude
    // For a linear instrument, Taylor should closely match full revaluation
    let full_pnl = full_result.pnl_distribution[0];
    let taylor_pnl = taylor_result.pnl_distribution[0];

    // Verify P&L has correct sign (rates up → bond value down → negative P&L)
    assert!(
        full_pnl < 0.0,
        "Full revaluation P&L should be negative for rate increase"
    );
    assert!(
        taylor_pnl < 0.0,
        "Taylor P&L should be negative for rate increase"
    );

    // Verify Taylor approximation is within 5% of full revaluation for this linear case
    // (Small difference expected due to convexity and discrete bumping)
    let rel_diff = (taylor_pnl - full_pnl).abs() / full_pnl.abs();
    assert!(
        rel_diff < 0.05,
        "Taylor P&L ({:.2}) should be within 5% of full revaluation P&L ({:.2}), got {:.2}%",
        taylor_pnl,
        full_pnl,
        rel_diff * 100.0
    );

    // Document the convention explicitly in the test
    // The Taylor formula in taylor_pnl_for_scenario is:
    //   pnl += dv01 * shift.shift * 10_000.0
    //
    // This means:
    //   - shift.shift is in DECIMAL form (0.0001 = 1bp)
    //   - DV01 is in currency units PER BASIS POINT
    //   - The 10,000 factor converts shift_decimal to shift_bp
    //
    // Example: DV01 = -500 (loses $500 per 1bp rise)
    //          shift = 0.0005 (5bp rise)
    //          P&L = -500 * 0.0005 * 10,000 = -500 * 5 = -2500
    println!(
        "Shift convention test passed: \
         shift_decimal={}, shift_bp={}, full_pnl={:.2}, taylor_pnl={:.2}, rel_diff={:.2}%",
        shift_decimal,
        shift_bp,
        full_pnl,
        taylor_pnl,
        rel_diff * 100.0
    );

    Ok(())
}

#[test]
fn test_var_calculation_simple_bond() -> Result<()> {
    let as_of = sample_as_of();

    let bond = standard_bond("TEST-BOND", as_of, date!(2029 - 01 - 01));
    let base_market = usd_ois_market(as_of)?;

    let history = history_from_rate_shifts(
        as_of,
        &[
            (date!(2023 - 12 - 31), 0.0050),
            (date!(2023 - 12 - 30), -0.0030),
            (date!(2023 - 12 - 29), 0.0010),
        ],
    );
    let config = VarConfig::var_95();

    // Calculate VaR
    let result = calculate_var(
        &[&bond as &dyn Instrument],
        &base_market,
        &history,
        as_of,
        &config,
        PricingDispatch::InstrumentDefault,
        None,
    )?;

    // Verify results
    assert_eq!(result.num_scenarios, 3);
    assert!(
        result.var < 0.0,
        "VaR should be negative (losses-negative convention)"
    );
    assert!(
        result.expected_shortfall <= result.var,
        "ES should be >= VaR"
    );

    // P&L distribution should have 3 values
    assert_eq!(result.pnl_distribution.len(), 3);

    // Distribution should be sorted (worst first)
    for i in 1..result.pnl_distribution.len() {
        assert!(
            result.pnl_distribution[i] >= result.pnl_distribution[i - 1],
            "P&L distribution should be sorted"
        );
    }

    Ok(())
}

#[test]
fn test_var_empty_history_is_rejected_for_non_empty_portfolio() -> Result<()> {
    let as_of = sample_as_of();
    let bond = standard_bond("TEST-BOND", as_of, date!(2029 - 01 - 01));
    let base_market = usd_ois_market(as_of)?;
    let history = history_from_scenarios(as_of, 0, vec![]);
    let config = VarConfig::var_95();

    let error = calculate_var(
        &[&bond as &dyn Instrument],
        &base_market,
        &history,
        as_of,
        &config,
        PricingDispatch::InstrumentDefault,
        None,
    )
    .expect_err("empty history must not report zero risk");
    assert!(error.to_string().contains("at least one market scenario"));
    Ok(())
}

#[test]
fn test_portfolio_var_with_diversification() -> Result<()> {
    let as_of = sample_as_of();

    let bond1 = standard_bond("BOND-5Y", as_of, date!(2029 - 01 - 01));
    let bond2 = standard_bond("BOND-2Y", as_of, date!(2026 - 01 - 01));

    let market = Arc::new(usd_ois_market(as_of)?);

    let history = history_from_rate_shifts(
        as_of,
        &[
            (date!(2023 - 12 - 31), 0.0100),
            (date!(2023 - 12 - 30), -0.0075),
            (date!(2023 - 12 - 29), 0.0025),
            (date!(2023 - 12 - 28), -0.0050),
        ],
    );
    let config = VarConfig::var_95();

    // Calculate individual VaRs
    let var1 = calculate_var(
        &[&bond1 as &dyn Instrument],
        market.as_ref(),
        &history,
        as_of,
        &config,
        PricingDispatch::InstrumentDefault,
        None,
    )?;
    let var2 = calculate_var(
        &[&bond2 as &dyn Instrument],
        market.as_ref(),
        &history,
        as_of,
        &config,
        PricingDispatch::InstrumentDefault,
        None,
    )?;
    let sum_individual_vars = var1.var.abs() + var2.var.abs();

    // Calculate portfolio VaR
    let instruments: Vec<&dyn Instrument> = vec![&bond1, &bond2];
    let portfolio_var = calculate_var(
        &instruments,
        market.as_ref(),
        &history,
        as_of,
        &config,
        PricingDispatch::InstrumentDefault,
        None,
    )?;

    // Verify portfolio VaR <= sum of individual VaRs
    // With only a few scenarios and both bonds having similar rate sensitivity,
    // we might not see diversification benefit in this simple test
    assert!(
        portfolio_var.var.abs() <= sum_individual_vars + 0.01, // Allow small numerical tolerance
        "Portfolio VaR ({}) should be <= sum of individual VaRs ({})",
        portfolio_var.var.abs(),
        sum_individual_vars
    );

    // Calculate diversification benefit (may be zero or small with limited scenarios)
    let diversification_benefit = sum_individual_vars - portfolio_var.var.abs();
    assert!(
        diversification_benefit >= -0.01, // Allow small numerical t olerance
        "Diversification benefit should be non-negative, got {}",
        diversification_benefit
    );

    println!("Individual VaR 1: ${:.2}", var1.var);
    println!("Individual VaR 2: ${:.2}", var2.var);
    println!("Sum of individual VaRs: ${:.2}", sum_individual_vars);
    println!("Portfolio VaR: ${:.2}", portfolio_var.var.abs());
    println!("Diversification benefit: ${:.2}", diversification_benefit);

    Ok(())
}

#[test]
fn directional_taylor_keeps_nonstandard_tenor_and_unrelated_curves() -> Result<()> {
    let as_of = sample_as_of();
    let mut usd = standard_bond("USD", as_of, date!(2028 - 01 - 01));
    usd.discount_curve_id = "USD-OIS".into();
    let mut eur = standard_bond("EUR-CURVE", as_of, date!(2028 - 01 - 01));
    eur.discount_curve_id = "EUR-OIS".into();
    let market = usd_ois_market(as_of)?.insert(
        DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (10.0, 0.8)])
            .build()?,
    );
    let scenario = MarketScenario::new(
        as_of,
        vec![
            RiskFactorShift {
                factor: RiskFactorType::DiscountRate {
                    curve_id: "USD-OIS".into(),
                    tenor_years: 1.4,
                },
                shift: 0.0005,
            },
            RiskFactorShift {
                factor: RiskFactorType::DiscountRate {
                    curve_id: "EUR-OIS".into(),
                    tenor_years: 2.3,
                },
                shift: -0.0004,
            },
        ],
    );
    let history = MarketHistory::new(as_of, 1, vec![scenario]);
    let taylor = calculate_var(
        &[&usd, &eur],
        &market,
        &history,
        as_of,
        &VarConfig::var_95().with_method(VarMethod::TaylorApproximation),
        PricingDispatch::InstrumentDefault,
        None,
    )?;
    let full = calculate_var(
        &[&usd, &eur],
        &market,
        &history,
        as_of,
        &VarConfig::var_95(),
        PricingDispatch::InstrumentDefault,
        None,
    )?;
    assert!((taylor.pnl_distribution[0] - full.pnl_distribution[0]).abs() < 1e-4);
    let usd_only =
        relevant_taylor_scenario(&usd, Currency::USD, Currency::USD, &history.scenarios[0])?;
    assert_eq!(usd_only.shifts.len(), 1);
    Ok(())
}
