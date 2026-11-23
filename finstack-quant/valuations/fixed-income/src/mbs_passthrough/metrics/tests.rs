use super::*;
use crate::mbs_passthrough::AgencyMbsPassthrough;
use crate::mbs_passthrough::pricer::price_with_spread;
use crate::mbs_passthrough::{AgencyProgram, PoolType};
use finstack_quant_cashflows::builder::PrepaymentModelSpec;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::math::solver::BrentSolver;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
use std::sync::Arc;
use time::Month;

/// The registered `Oas` metric reports Monte Carlo OAS rather than a
/// deterministic bare-curve spread.
#[test]
fn oas_metric_reports_monte_carlo_oas_not_static_zspread() {
    let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([
            (0.0, 1.0),
            (1.0, 0.96),
            (5.0, 0.80),
            (10.0, 0.60),
            (30.0, 0.30),
        ])
        .interp(InterpStyle::Linear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("valid curve");
    let market = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve");

    // Discounted quote so both spreads are clearly non-zero.
    let quote = 92.0_f64;
    let mut mbs = AgencyMbsPassthrough::builder()
        .id(InstrumentId::new("TEST-MBS-OASMETRIC"))
        .pool_id("TEST-POOL".into())
        .agency(AgencyProgram::Fnma)
        .pool_type(PoolType::Generic)
        .original_face(Money::from((1_000_000_i64, Currency::USD)))
        .current_face(Money::from((1_000_000_i64, Currency::USD)))
        .wac(0.045)
        .coupon(0.04)
        .servicing_fee_bp(25.0)
        .guarantee_fee_bp(25.0)
        .wam_months(360)
        .issue_date(Date::from_calendar_date(2024, Month::January, 1).expect("valid"))
        .maturity(Date::from_calendar_date(2054, Month::January, 1).expect("valid"))
        .prepayment_spec(PrepaymentModelSpec::psa(1.0))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .day_count(DayCount::Thirty360)
        .build()
        .expect("valid mbs");
    mbs.instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price_pct = Some(quote);

    // Reference values computed directly.
    let target = quote / 100.0 * mbs.current_face.amount();
    let static_zspread = BrentSolver::new(1e-8, 100)
        .expect("valid solver")
        .solve(
            |spread| {
                Ok(price_with_spread(&mbs, &market, as_of, spread).expect("static price") - target)
            },
            0.0,
            -0.10,
            0.20,
            0.05,
            10,
        )
        .expect("static z-spread");
    let config = McOasConfig {
        num_paths: 512,
        hw1f_mean_reversion: 0.05,
        hw1f_sigma: 0.01,
        seed: mbs
            .instrument_pricing_overrides
            .model_config
            .mc_seed(&mbs.id),
    };
    let mc_oas = calculate_mc_oas(&mbs, quote, &market, as_of, &config).expect("mc oas");

    // The registered metric.
    let ctx = MetricContext::new(
        Arc::new(mbs),
        Arc::new(market),
        as_of,
        Money::from((0_i64, Currency::USD)),
        MetricContext::default_config(),
    );
    let mut ctx = ctx;
    let metric_oas = OasCalculator
        .calculate(&mut ctx)
        .expect("Oas metric should compute");

    // The metric must equal the MC-OAS (deterministic seed) ...
    assert!(
        (metric_oas - mc_oas).abs() < 1e-9,
        "Oas metric {metric_oas} should equal MC-OAS {mc_oas}"
    );
    // ... and the MC-OAS must differ from the static Z-spread.
    assert!(
        (mc_oas - static_zspread).abs() > 1e-4,
        "MC-OAS {mc_oas} should differ from static Z-spread {static_zspread}"
    );
}

/// The Oas metric reads κ and σ from the pool's model overrides; a
/// partial pair is rejected rather than silently defaulted.
#[test]
fn oas_metric_reads_hull_white_parameters_from_model_config() {
    let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
    let market = MarketContext::new()
        .try_insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (30.0, 0.30)])
                .interp(InterpStyle::LogLinear)
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("valid curve"),
        )
        .expect("insert market curve");
    let mut mbs = AgencyMbsPassthrough::example().expect("mbs");
    mbs.instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price_pct = Some(95.0);
    let metric = |mbs: &AgencyMbsPassthrough| {
        let mut ctx = MetricContext::new(
            Arc::new(mbs.clone()),
            Arc::new(market.clone()),
            as_of,
            Money::from((0_i64, Currency::USD)),
            MetricContext::default_config(),
        );
        OasCalculator.calculate(&mut ctx)
    };
    let default_oas = metric(&mbs).expect("default oas");

    mbs.instrument_pricing_overrides
        .model_config
        .hw1f_mean_reversion = Some(0.10);
    mbs.instrument_pricing_overrides.model_config.hw1f_sigma = Some(0.015);
    let configured = metric(&mbs).expect("configured oas");
    let direct = calculate_mc_oas(
        &mbs,
        95.0,
        &market,
        as_of,
        &McOasConfig {
            num_paths: 512,
            hw1f_mean_reversion: 0.10,
            hw1f_sigma: 0.015,
            seed: mbs
                .instrument_pricing_overrides
                .model_config
                .mc_seed(&mbs.id),
        },
    )
    .expect("direct");
    assert!(
        (configured - direct).abs() < 1e-12,
        "{configured} vs {direct}"
    );
    assert!((configured - default_oas).abs() > 1e-6);

    mbs.instrument_pricing_overrides.model_config.hw1f_sigma = None;
    assert!(metric(&mbs).is_err(), "partial κ/σ pair must be rejected");
}

/// `mc_paths` and the seed scenario label reach the MC-OAS engine.
#[test]
fn mc_oas_config_reads_paths_and_seed_from_model_config() {
    let mut mbs = AgencyMbsPassthrough::example().expect("mbs");
    let base = McOasConfig::resolve(&mbs).expect("base");
    assert_eq!(base.num_paths, 512);
    assert_eq!(
        base.seed,
        finstack_quant_models::monte_carlo::seed::derive_seed(&mbs.id, "base")
    );
    mbs.instrument_pricing_overrides.model_config.mc_paths = Some(64);
    mbs.instrument_pricing_overrides
        .model_config
        .mc_seed_scenario = Some("stress".into());
    let configured = McOasConfig::resolve(&mbs).expect("configured");
    assert_eq!(configured.num_paths, 64);
    assert_eq!(
        configured.seed,
        finstack_quant_models::monte_carlo::seed::derive_seed(&mbs.id, "stress")
    );
}
