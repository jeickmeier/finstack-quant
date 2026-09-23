//! Agency MBS risk metrics.
//!
//! This module provides MBS-specific risk metrics including:
//!
//! - **OAS (Option-Adjusted Spread)**: Spread over risk-free that equates
//!   model price to market price
//! - **Effective Duration**: Duration accounting for prepayment sensitivity
//! - **Effective Convexity**: Convexity accounting for prepayment sensitivity
//! - **Bucketed DV01**: Key-rate bucketed interest-rate sensitivities via
//!   the generic `UnifiedDv01Calculator` with triangular key-rate config.

pub(crate) mod duration;
pub(crate) mod mc_oas;

/// Prepayment rate-sensitivity coefficient β shared by every rate-dependent
/// prepayment adjustment in this module.
///
/// Applied as the refinancing-incentive multiplier `exp(-β·Δr)` on prepayment
/// speed. β is calibrated so a 100 bp rate move roughly doubles (rates down)
/// or halves (rates up) the prepayment speed — the standard rule-of-thumb
/// shape of the empirical refinancing S-curve (Hayre 2001; Fabozzi 2016):
/// `β = ln(2) / 0.01 ≈ 69.3`.
///
/// Both the effective duration/convexity engine ([`duration`]) and the
/// Monte-Carlo OAS engine ([`mc_oas`], via [`McOasConfig`]'s default
/// `prepay_rate_sensitivity`) use this constant so the two engines price the
/// prepayment option consistently; the MC config field remains overridable.
pub(crate) const PREPAY_RATE_SENSITIVITY: f64 = std::f64::consts::LN_2 / 0.01;

pub(crate) use duration::{effective_convexity, effective_duration};
pub(crate) use mc_oas::{calculate_mc_oas, McOasConfig};

use crate::instruments::fixed_income::mbs_passthrough::AgencyMbsPassthrough;
use crate::metrics::{MetricCalculator, MetricContext, MetricRegistry};

/// Calculator for effective duration (mapped to DurationMod).
pub(crate) struct EffectiveDurationCalculator;

impl MetricCalculator for EffectiveDurationCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let mbs: &AgencyMbsPassthrough = context.instrument_as()?;
        effective_duration(mbs, context.curves.as_ref(), context.as_of, None)
    }
}

/// Calculator for effective convexity (mapped to Convexity).
pub(crate) struct EffectiveConvexityCalculator;

impl MetricCalculator for EffectiveConvexityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let mbs: &AgencyMbsPassthrough = context.instrument_as()?;
        effective_convexity(mbs, context.curves.as_ref(), context.as_of, None)
    }
}

/// Calculator for option-adjusted spread (OAS).
///
/// Reports the Monte Carlo OAS from [`calculate_mc_oas`], using stochastic
/// Hull-White rate paths and rate-dependent prepayment. The Hull-White mean
/// reversion κ and short-rate volatility σ come from
/// `instrument_pricing_overrides.model_config.hw1f_mean_reversion` and
/// `hw1f_sigma`, supplied together; without them the engine uses κ = 5% and
/// σ = 1%. These are user inputs: the metric does not calibrate them to
/// swaptions.
pub(crate) struct OasCalculator;

/// Resolve the MC-OAS configuration from the pool's model overrides.
fn mc_oas_config(mbs: &AgencyMbsPassthrough) -> finstack_quant_core::Result<McOasConfig> {
    let model = &mbs.instrument_pricing_overrides.model_config;
    if model.hw1f_sigma_schedule.is_some() {
        return Err(finstack_quant_core::Error::Validation(
            "MBS MC-OAS takes a scalar hw1f_sigma; hw1f_sigma_schedule is not supported".into(),
        ));
    }
    match (model.hw1f_mean_reversion, model.hw1f_sigma) {
        (None, None) => Ok(McOasConfig::default()),
        (Some(hw_kappa), Some(hw_sigma)) => Ok(McOasConfig {
            hw_kappa,
            hw_sigma,
            ..McOasConfig::default()
        }),
        _ => Err(finstack_quant_core::Error::Validation(
            "MBS MC-OAS requires hw1f_mean_reversion and hw1f_sigma together".into(),
        )),
    }
}

impl MetricCalculator for OasCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let mbs: &AgencyMbsPassthrough = context.instrument_as()?;
        let market_price = mbs
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "mbs.pricing_overrides.quoted_clean_price".to_string(),
                })
            })?;
        // True option-adjusted spread from the Monte Carlo model.
        calculate_mc_oas(
            mbs,
            market_price,
            context.curves.as_ref(),
            context.as_of,
            &mc_oas_config(mbs)?,
        )
    }
}

/// Register agency MBS passthrough metrics with the registry.
pub(crate) fn register_mbs_passthrough_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::AgencyMbsPassthrough,
        metrics: [
            (DurationMod, EffectiveDurationCalculator),
            (Convexity, EffectiveConvexityCalculator),
            (Oas, OasCalculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::AgencyMbsPassthrough,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::AgencyMbsPassthrough,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::specs::PrepaymentModelSpec;
    use crate::instruments::fixed_income::mbs_passthrough::pricer::price_with_spread;
    use crate::instruments::fixed_income::mbs_passthrough::{AgencyProgram, PoolType};
    use crate::metrics::{MetricCalculator, MetricContext};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::math::solver::{BrentSolver, Solver};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
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
            .build()
            .expect("valid curve");
        let market = MarketContext::new().insert(disc);

        // Discounted quote so both spreads are clearly non-zero.
        let quote = 92.0_f64;
        let mut mbs = AgencyMbsPassthrough::builder()
            .id(InstrumentId::new("TEST-MBS-OASMETRIC"))
            .pool_id("TEST-POOL".into())
            .agency(AgencyProgram::Fnma)
            .pool_type(PoolType::Generic)
            .original_face(Money::from((1_000_000_i64, Currency::USD)))
            .current_face(Money::from((1_000_000_i64, Currency::USD)))
            .current_factor(1.0)
            .wac(0.045)
            .pass_through_rate(0.04)
            .servicing_fee_rate(0.0025)
            .guarantee_fee_rate(0.0025)
            .wam(360)
            .issue_date(Date::from_calendar_date(2024, Month::January, 1).expect("valid"))
            .maturity(Date::from_calendar_date(2054, Month::January, 1).expect("valid"))
            .prepayment_model(PrepaymentModelSpec::psa(1.0))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Thirty360)
            .build()
            .expect("valid mbs");
        mbs.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price = Some(quote);

        // Reference values computed directly.
        let target = quote / 100.0 * mbs.current_face.amount();
        let static_zspread = BrentSolver::new()
            .tolerance(1e-8)
            .max_iterations(100)
            .bracket_bounds(-0.10, 0.20)
            .initial_bracket_size(Some(0.05))
            .solve(
                |spread| {
                    price_with_spread(&mbs, &market, as_of, spread).expect("static price") - target
                },
                0.0,
            )
            .expect("static z-spread");
        let mc_oas =
            calculate_mc_oas(&mbs, quote, &market, as_of, &McOasConfig::default()).expect("mc oas");

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
        let market = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (30.0, 0.30)])
                .interp(InterpStyle::LogLinear)
                .build()
                .expect("valid curve"),
        );
        let mut mbs = AgencyMbsPassthrough::example().expect("mbs");
        mbs.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price = Some(95.0);
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
                hw_kappa: 0.10,
                hw_sigma: 0.015,
                ..McOasConfig::default()
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
}
