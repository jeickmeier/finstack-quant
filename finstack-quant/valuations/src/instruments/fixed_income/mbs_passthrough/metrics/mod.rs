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
pub(crate) mod oas;

pub(crate) use duration::{effective_convexity, effective_duration};
pub(crate) use mc_oas::{calculate_mc_oas, McOasConfig};
#[cfg(test)]
pub(crate) use oas::calculate_static_zspread;

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
/// Reports the **Monte Carlo OAS** ([`calculate_mc_oas`]): a true
/// option-adjusted spread computed over stochastic Hull-White rate paths with
/// rate-dependent prepayment. The bare-curve static Z-spread
/// ([`calculate_static_zspread`]) does not account for the prepayment option and is
/// retained only as a separate public-API helper — it is *not* what the `Oas`
/// metric returns.
pub(crate) struct OasCalculator;

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
        let result = calculate_mc_oas(
            mbs,
            market_price,
            context.curves.as_ref(),
            context.as_of,
            &McOasConfig::default(),
        )?;
        Ok(result.oas)
    }
}

/// Register agency MBS passthrough metrics with the registry.
pub(crate) fn register_mbs_passthrough_metrics(registry: &mut MetricRegistry) {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::specs::PrepaymentModelSpec;
    use crate::instruments::fixed_income::mbs_passthrough::{AgencyProgram, PoolType};
    use crate::metrics::{MetricCalculator, MetricContext};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use std::sync::Arc;
    use time::Month;

    /// Item 4 regression: the registered `Oas` metric must report the Monte
    /// Carlo OAS, not the bare-curve static Z-spread.
    ///
    /// Before the fix, `OasCalculator` called `calculate_oas` (static
    /// Z-spread mislabeled as OAS). This test computes the static Z-spread and
    /// the MC-OAS independently for a discounted pool and asserts the metric
    /// matches the MC-OAS and is materially different from the static spread.
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
            .original_face(Money::new(1_000_000.0, Currency::USD))
            .current_face(Money::new(1_000_000.0, Currency::USD))
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
        let static_zspread = calculate_static_zspread(&mbs, quote, &market, as_of)
            .expect("static z-spread")
            .spread;
        let mc_oas = calculate_mc_oas(&mbs, quote, &market, as_of, &McOasConfig::default())
            .expect("mc oas")
            .oas;

        // The registered metric.
        let ctx = MetricContext::new(
            Arc::new(mbs),
            Arc::new(market),
            as_of,
            Money::new(0.0, Currency::USD),
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
        // ... and the MC-OAS must differ from the static Z-spread, proving the
        // metric is no longer the mislabeled static spread.
        assert!(
            (mc_oas - static_zspread).abs() > 1e-4,
            "MC-OAS {mc_oas} should differ from static Z-spread {static_zspread}"
        );
    }
}
