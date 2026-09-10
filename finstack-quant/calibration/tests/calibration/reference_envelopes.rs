//! Reference envelope integration tests.
//!
//! Each test loads one of the JSON examples under
//! `finstack-quant/valuations/examples/market_bootstrap/`, runs it through the
//! calibration engine, and asserts that the resulting `MarketContext` answers
//! a typical analyst-style accessor query. The reference envelopes are the
//! canonical user-facing examples for the "build a MarketContext from quotes"
//! workflow.

use finstack_quant_calibration::api::engine;
use finstack_quant_calibration::api::schema::CalibrationEnvelope;
use finstack_quant_core::market_data::context::MarketContext;
use std::path::PathBuf;

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/market_bootstrap")
}

pub(crate) fn load_envelope(file_name: &str) -> CalibrationEnvelope {
    let path = examples_dir().join(file_name);
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    serde_json::from_str(&json).unwrap_or_else(|err| {
        panic!(
            "deserialize {} as CalibrationEnvelope: {err}",
            path.display()
        )
    })
}

pub(crate) fn execute(envelope: &CalibrationEnvelope) -> MarketContext {
    let result = engine::execute(envelope).expect("calibration engine succeeded");
    MarketContext::try_from(result.result.final_market)
        .expect("rehydrate MarketContext from final_market state")
}

#[test]
fn sparse_surface_report_matches_published_grid_and_enforces_tolerance() {
    use finstack_quant_calibration::api::market_datum::MarketDatum;
    use finstack_quant_calibration::api::schema::StepParams;
    use finstack_quant_calibration::quotes::vol::VolQuote;
    let mut envelope = load_envelope("08_equity_vol_surface.json");
    envelope.plan.settings.vol_surface.validation_tolerance = 0.001;
    envelope.plan.settings.fail_on_bad_fit = false;
    let base_date = match &mut envelope.plan.steps[1].params {
        StepParams::VolSurface(params) => {
            params.target_strikes = vec![140.0, 180.0, 220.0];
            params.base_date
        }
        _ => unreachable!("equity surface example"),
    };
    let result = engine::execute(&envelope).expect("diagnostic calibration");
    let report = &result.result.step_reports[&envelope.plan.steps[1].id];
    let market = MarketContext::try_from(result.result.final_market).expect("market");
    let surface = market.get_surface("AAPL-EQUITY-VOL").expect("surface");
    let mut worst = 0.0_f64;
    for datum in &envelope.market_data {
        if let MarketDatum::VolQuote(VolQuote::OptionVol {
            id,
            expiry,
            strike,
            vol,
            ..
        }) = datum
        {
            let t = (*expiry - base_date).whole_days() as f64 / 365.0;
            let residual = finstack_quant_models::volatility::get_surface_vol(&surface, t, *strike)
                .expect("published lookup")
                - vol;
            assert!((report.residuals[id.as_str()] - residual).abs() < 1e-12);
            worst = worst.max(residual.abs());
        }
    }
    assert!((report.max_residual - worst).abs() < 1e-12);
    assert!(worst > 0.001);
    assert!(!report.success);
    envelope.plan.settings.fail_on_bad_fit = true;
    assert!(engine::execute(&envelope).is_err());
}

#[test]
fn parametric_calibration_rejects_disconnected_discount_curve() {
    let mut envelope =
        serde_json::to_value(load_envelope("01_usd_discount.json")).expect("envelope");
    let discount = envelope["plan"]["steps"][0].clone();
    let step = serde_json::json!({
        "id": "NS", "quote_set": discount["quote_set"], "kind": "parametric",
        "curve_id": "NS", "base_date": discount["base_date"], "model": "ns",
        "discount_curve_id": discount["curve_id"]
    });
    envelope["plan"]["steps"]
        .as_array_mut()
        .expect("steps")
        .push(step);
    let error = serde_json::from_value::<CalibrationEnvelope>(envelope)
        .expect_err("separate discount option must not be accepted");
    assert!(error.to_string().contains("discount_curve_id"));
}

#[test]
fn distressed_hazard_quotes_do_not_expand_explicit_bounds() {
    use finstack_quant_calibration::api::market_datum::MarketDatum;
    use finstack_quant_calibration::quotes::cds::CdsQuote;
    let mut envelope = load_envelope("03_single_name_hazard.json");
    for datum in &mut envelope.market_data {
        if let MarketDatum::CdsQuote(CdsQuote::CdsParSpread { spread_bp, .. }) = datum {
            *spread_bp = 2000.0;
        }
    }
    envelope.plan.settings.hazard_curve.hazard_hard_max = 0.1;
    envelope.plan.settings.fail_on_bad_fit = true;
    assert!(
        engine::execute(&envelope).is_err(),
        "20% spread cannot fit a hazard bounded at 10% with 40% recovery"
    );
    envelope.plan.settings.hazard_curve.hazard_hard_max = 1.0;
    let result = engine::execute(&envelope).expect("explicitly adequate bounds");
    let report = &result.result.step_reports[&envelope.plan.steps[1].id];
    assert_eq!(
        report.success_tolerance,
        Some(envelope.plan.settings.hazard_curve.validation_tolerance)
    );
}

#[test]
fn example_01_usd_discount_builds_queryable_curve() {
    let envelope = load_envelope("01_usd_discount.json");
    let market = execute(&envelope);

    let curve = market
        .get_discount("USD-OIS")
        .expect("USD-OIS discount curve present in calibrated market");

    // Discount factor at the curve base date is 1.0 by construction; a
    // forward date should produce a strictly positive DF less than 1 as a
    // sanity check that the bootstrap actually populated knot points.
    let df_today = curve.df(0.0);
    assert!(
        (df_today - 1.0).abs() < 1e-9,
        "df at t=0 should be 1.0, got {df_today}"
    );

    let df_one_year = curve.df(1.0);
    assert!(
        df_one_year > 0.0 && df_one_year < 1.0,
        "df at t=1y should be in (0, 1), got {df_one_year}"
    );
}

#[test]
fn example_03_single_name_hazard_composes_on_initial_market() {
    let envelope = load_envelope("03_single_name_hazard.json");
    let market = execute(&envelope);

    // Discount curve must survive from source_market unchanged.
    market
        .get_discount("USD-OIS")
        .expect("discount curve carried through from source_market");

    // Hazard curve must be produced by the calibration step.
    let hazard = market
        .get_hazard("ISSUER-A-CDS")
        .expect("hazard curve present after single-name CDS calibration");

    // Survival probability must be in (0, 1) for any positive horizon.
    let survival_one_year = hazard.sp(1.0);
    assert!(
        survival_one_year > 0.0 && survival_one_year < 1.0,
        "sp(1y) should be in (0, 1), got {survival_one_year}"
    );
}

#[test]
fn example_09_fx_matrix_supports_cross_rate_lookup() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::fx::FxQuery;
    use time::macros::date;

    let envelope = load_envelope("09_fx_matrix.json");
    let market = execute(&envelope);

    let fx = market
        .fx()
        .expect("FX matrix must be present in the snapshot-only market");

    let as_of = date!(2026 - 05 - 08);

    // --- Direct quote: EUR/USD ---
    let eur_usd = fx
        .rate(FxQuery::new(Currency::EUR, Currency::USD, as_of))
        .expect("EUR/USD direct quote must be retrievable");
    assert!(
        !eur_usd.triangulated,
        "EUR/USD is a direct quote and should not be marked as triangulated"
    );
    assert!(
        eur_usd.rate > 0.5 && eur_usd.rate < 2.0,
        "EUR/USD rate should be in a sane range (0.5, 2.0), got {}",
        eur_usd.rate
    );

    // --- Triangulated cross: EUR/JPY via USD pivot ---
    // EUR/JPY is not a direct quote; it must be computed as EUR→USD→JPY.
    let eur_jpy = fx
        .rate(FxQuery::new(Currency::EUR, Currency::JPY, as_of))
        .expect("EUR/JPY cross rate must be computable via USD triangulation");
    assert!(
        eur_jpy.triangulated,
        "EUR/JPY should be marked as triangulated (no direct quote supplied)"
    );
    assert!(
        eur_jpy.rate > 0.0,
        "EUR/JPY triangulated rate must be positive, got {}",
        eur_jpy.rate
    );

    // Sanity: EUR/JPY ≈ EUR/USD × USD/JPY = 1.085 × 152.40 ≈ 165.35
    let expected_approx = eur_usd.rate
        * fx.rate(FxQuery::new(Currency::USD, Currency::JPY, as_of))
            .expect("USD/JPY direct quote must be retrievable")
            .rate;
    assert!(
        (eur_jpy.rate - expected_approx).abs() < 1e-6,
        "EUR/JPY cross rate {:.6} should match EUR/USD × USD/JPY {:.6}",
        eur_jpy.rate,
        expected_approx
    );
}

#[test]
fn example_02_usd_3m_forward_builds_queryable_curve() {
    let envelope = load_envelope("02_usd_3m_forward_curve.json");
    let market = execute(&envelope);

    // Discount curve passes through unchanged from source_market.
    market
        .get_discount("USD-OIS")
        .expect("discount curve carried through from source_market");

    // Forward curve must be produced by the calibration step.
    let forward = market
        .get_forward("USD-SOFR-3M")
        .expect("forward curve present after forward step");

    // Forward rate at t=1y should be a sane positive rate.
    let rate_one_year = forward.rate(1.0);
    assert!(
        rate_one_year > 0.0 && rate_one_year < 0.20,
        "forward rate at t=1y should be in (0, 0.20), got {rate_one_year}"
    );
}

#[test]
fn example_04_cdx_ig_hazard_builds_queryable_curve() {
    let envelope = load_envelope("04_cdx_ig_hazard.json");
    let market = execute(&envelope);

    market
        .get_discount("USD-OIS")
        .expect("discount carried through from source_market");

    let hazard = market
        .get_hazard("CDX-NA-IG-46")
        .expect("CDX hazard curve present after calibration");

    let survival_5y = hazard.sp(5.0);
    assert!(
        survival_5y > 0.0 && survival_5y < 1.0,
        "5y survival should be in (0, 1), got {survival_5y}"
    );
}

#[test]
fn example_05_cdx_base_correlation_builds_queryable_curve() {
    let envelope = load_envelope("05_cdx_base_correlation.json");
    let market = execute(&envelope);

    market
        .get_discount("USD-OIS")
        .expect("discount carried through from source_market");
    market
        .get_hazard("CDX-NA-IG-46")
        .expect("CDX index hazard carried through from source_market");

    let bc = market
        .get_base_correlation("CDX-NA-IG-46_CORR")
        .expect("base correlation curve present after calibration");

    // detachment_pct is in percentage units: 7.0 means the 7% detachment point.
    let corr_7pct = bc.correlation(7.0);
    assert!(
        (0.0..=1.0).contains(&corr_7pct),
        "base correlation at 7% detachment should be in [0, 1], got {corr_7pct}"
    );
}

#[test]
fn example_06_cdx_index_vol_builds_queryable_surface() {
    let envelope = load_envelope("06_cdx_index_vol.json");
    let market = execute(&envelope);

    market
        .get_discount("USD-OIS")
        .expect("discount carried through from source_market");
    market
        .get_hazard("CDX-NA-IG-46")
        .expect("CDX index hazard carried through from source_market");

    let surface = market
        .get_surface("CDX-NA-IG-46-CDSO-VOL")
        .expect("CDX index vol surface present after calibration");

    // Sanity-query the SABR surface at the ATM forward (~55 bp) and 40-day
    // expiry (June 2026 from base 2026-05-08 ≈ 0.10959y on Act365F). With
    // fail_on_bad_fit relaxed the fit is approximate, but the queried vol
    // must still be a positive number in a sane lognormal range.
    let vol =
        finstack_quant_models::volatility::get_surface_vol(&surface, 40.0 / 365.0, 0.00552848)
            .expect("vol query at ATM forward should succeed");
    assert!(
        vol > 0.0 && vol < 5.0,
        "ATM vol should be in (0, 5), got {vol}"
    );
}

#[test]
fn example_07_swaption_vol_surface_builds_queryable_surface() {
    let envelope = load_envelope("07_swaption_vol_surface.json");
    let market = execute(&envelope);

    market
        .get_discount("USD-OIS")
        .expect("discount carried through from source_market");
    market
        .get_forward("USD-SOFR-3M")
        .expect("forward carried through from source_market");

    // swaption_vol calibration produces a VolCube (SABR params on expiry x tenor
    // grid). Resolve through the valuations-owned volatility source resolver.
    let surface =
        finstack_quant_valuations::market::resolve_vol_source(&market, "USD-SWAPTION-NORMAL-VOL")
            .expect("swaption vol cube present after calibration");

    // Sanity-query the produced surface at a representative (expiry, tenor, strike).
    // Use a 1y expiry × 5y swap × ATM-ish strike. With normal vols, ATM-ish for
    // a 5% rate environment is around 0.05; SABR will pin closely if non-flat,
    // approximately if flat (see 06_cdx_index_vol.json for the flat-grid issue).
    let vol = surface
        .get_normal_vol(1.0, 5.0, 0.05)
        .expect("vol query at 1y × 5y × ATM should succeed");
    // The input quotes and reconstructed cube both use decimal normal volatility.
    assert!(surface.get_vol(1.0, 5.0, 0.05).is_err());
    assert!(
        vol > 0.0 && vol < 0.05,
        "normal volatility must be below 500 bp, got {vol}"
    );
}

#[test]
fn example_08_equity_vol_surface_builds_queryable_surface() {
    use finstack_quant_core::market_data::scalars::MarketScalar;

    let envelope = load_envelope("08_equity_vol_surface.json");
    let market = execute(&envelope);

    market
        .get_discount("USD-OIS")
        .expect("discount carried through from source_market");

    // Equity spot price must be present in source_market.prices.
    let scalar = market
        .get_price("AAPL")
        .expect("AAPL spot price present in source_market.prices");
    let spot = match scalar {
        MarketScalar::Price(m) => m.amount(),
        MarketScalar::Unitless(v) => *v,
    };
    assert!(spot > 0.0, "AAPL spot should be positive, got {spot}");

    // Equity vol surface produced by the calibration step. The vol_surface
    // step stores its output as a VolSurface (not a VolCube), so use
    // get_surface — which returns Arc<VolSurface>, already implementing
    // concrete VolSource rather than a trait-object provider.
    let surface = market
        .get_surface("AAPL-EQUITY-VOL")
        .expect("AAPL equity vol surface present after calibration");

    let vol = finstack_quant_models::volatility::get_surface_vol(&surface, 0.5, 175.0)
        .expect("vol query at 6m × ATM should succeed");
    assert!(
        vol > 0.0 && vol < 5.0,
        "AAPL equity vol should be positive, got {vol}"
    );
}

#[test]
fn example_10_bond_prices_supports_lookup() {
    use finstack_quant_core::market_data::scalars::MarketScalar;

    let envelope = load_envelope("10_bond_prices.json");
    let market = execute(&envelope);

    // At least two distinct bond IDs must be retrievable.
    let scalar1 = market
        .get_price("US-TREASURY-10Y-2026-05-08")
        .expect("US 10Y treasury price present in source_market.prices");
    let p1 = match scalar1 {
        MarketScalar::Price(m) => m.amount(),
        MarketScalar::Unitless(v) => *v,
    };

    let scalar2 = market
        .get_price("IBM-7YR-2033")
        .expect("IBM corporate bond price present");
    let p2 = match scalar2 {
        MarketScalar::Price(m) => m.amount(),
        MarketScalar::Unitless(v) => *v,
    };

    assert!(p1 > 0.0, "treasury price should be positive, got {p1}");
    assert!(p2 > 0.0, "IBM bond price should be positive, got {p2}");
}

#[test]
fn example_11_equity_spots_and_dividends_support_lookup() {
    use finstack_quant_core::market_data::scalars::MarketScalar;

    let envelope = load_envelope("11_equity_spots_dividends.json");
    let market = execute(&envelope);

    let scalar_aapl = market
        .get_price("AAPL")
        .expect("AAPL spot present in source_market.prices");
    let aapl_spot = match scalar_aapl {
        MarketScalar::Price(m) => m.amount(),
        MarketScalar::Unitless(v) => *v,
    };
    assert!(aapl_spot > 0.0, "AAPL spot should be positive");

    let scalar_msft = market.get_price("MSFT").expect("MSFT spot present");
    let msft_spot = match scalar_msft {
        MarketScalar::Price(m) => m.amount(),
        MarketScalar::Unitless(v) => *v,
    };
    assert!(msft_spot > 0.0, "MSFT spot should be positive");

    // Dividend schedule for AAPL must be retrievable by schedule ID.
    let aapl_divs = market
        .get_dividend_schedule("AAPL-DIVS")
        .expect("AAPL dividend schedule present in source_market.dividends");
    // The schedule should have at least one dividend entry.
    assert!(
        !aapl_divs.get_events().is_empty(),
        "AAPL dividend schedule should be non-empty"
    );
}

#[test]
fn example_12_full_credit_desk_market_chains_steps() {
    let envelope = load_envelope("12_full_credit_desk_market.json");
    let market = execute(&envelope);

    // All three calibrated curves should be present.
    market
        .get_discount("USD-OIS")
        .expect("discount curve produced by step");
    let hazard = market
        .get_hazard("CDX-NA-IG-46")
        .expect("hazard curve produced by step");
    market
        .get_base_correlation("CDX-NA-IG-46_CORR")
        .expect("base correlation curve produced by step");

    // FX matrix from source_market must survive.
    let fx = market.fx().expect("fx matrix from source_market");
    let _ = fx;

    // Hazard at 5y reflects the SYNTHETIC sub-1bp CDS spreads used to keep
    // the chained tranche calibration well-bracketed (see envelope's
    // plan.description for the rationale). Realistic IG hazards would give
    // SP(5y) ≈ 0.92 — the narrow band below locks the synthetic value so
    // that any unit-class regression in step 2 of the chain is caught here.
    let sp_5y = hazard.sp(5.0);
    assert!(
        (0.999..1.0).contains(&sp_5y),
        "5y SP should reflect synthetic sub-1bp spreads (~0.9997), got {sp_5y}"
    );
}
