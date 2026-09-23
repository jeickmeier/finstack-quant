//! Dollar roll risk and carry metrics.
//!
//! Standard rate-sensitivity metrics (DV01, bucketed DV01, theta) plus
//! carry-specific analytics: implied financing rate and roll specialness.

use crate::metrics::{MetricCalculator, MetricContext, MetricRegistry};

/// Implied financing rate metric calculator.
///
/// Computes the annualized implied repo rate from the dollar roll drop,
/// expected coupon income, and principal paydown between settlement dates.
/// Uses the MBS cashflow engine for carry inputs.
///
/// Prepayment follows the generic TBA pool's prepayment model.
pub(crate) struct ImpliedFinancingRateCalculator;

impl MetricCalculator for ImpliedFinancingRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let roll: &crate::instruments::DollarRoll = context.instrument_as()?;
        let result = super::carry::implied_financing_rate(roll)?;
        Ok(result.implied_rate)
    }
}

/// Roll specialness metric calculator.
///
/// Returns specialness in basis points (repo rate - implied financing rate).
/// Positive means rolling is cheaper than repo financing.
///
/// Uses the generic TBA pool's prepayment model and resolves repo financing
/// over the actual roll interval.
pub(crate) struct RollSpecialnessCalculator;

impl MetricCalculator for RollSpecialnessCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let roll: &crate::instruments::DollarRoll = context.instrument_as()?;
        let front = roll.front_settle_date()?;
        let back = roll.back_settle_date()?;
        let accrual = finstack_quant_core::dates::DayCount::Act360.year_fraction(
            front,
            back,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        let repo_rate = if let Some(id) = &roll.repo_curve_id {
            let curve = context.curves.get_forward(id)?;
            let t1 = curve.day_count().year_fraction(
                curve.base_date(),
                front,
                finstack_quant_core::dates::DayCountContext::default(),
            )?;
            let t2 = curve.day_count().year_fraction(
                curve.base_date(),
                back,
                finstack_quant_core::dates::DayCountContext::default(),
            )?;
            curve.rate_between(t1, t2)? * (t2 - t1) / accrual
        } else {
            let curve = context.curves.get_discount(&roll.discount_curve_id)?;
            (1.0 / curve.df_between_dates(front, back)? - 1.0) / accrual
        };
        super::carry::roll_specialness(roll, repo_rate)
    }
}

/// Register dollar roll metrics with the registry.
pub(crate) fn register_dollar_roll_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::DollarRoll,
        metrics: [
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::DollarRoll,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::DollarRoll,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            (ImpliedFinancingRate, ImpliedFinancingRateCalculator),
            (RollSpecialness, RollSpecialnessCalculator)
        ]
    }
    Ok(())
}

#[cfg(test)]
mod production_mortgage_audit {
    use super::*;
    use finstack_quant_core::{
        currency::Currency,
        market_data::{
            context::MarketContext,
            term_structures::{DiscountCurve, ForwardCurve},
        },
        money::Money,
    };
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn specialness_resolves_the_supplied_repo_curve() {
        let as_of = date!(2026 - 03 - 01);
        let mut roll = crate::instruments::DollarRoll::example().expect("roll");
        roll.front_settlement_date = Some(date!(2026 - 03 - 11));
        roll.back_settlement_date = Some(date!(2026 - 04 - 11));
        roll.repo_curve_id = Some("REPO".into());
        let carry =
            crate::instruments::fixed_income::dollar_roll::carry::implied_financing_rate(&roll)
                .expect("carry");
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (5.0, 1.0)])
                    .build()
                    .expect("curve"),
            )
            .insert(
                ForwardCurve::builder("REPO", 31.0 / 360.0)
                    .base_date(as_of)
                    .day_count(finstack_quant_core::dates::DayCount::Act360)
                    .knots([(0.0, 0.0375), (5.0, 0.0375)])
                    .build()
                    .expect("forward"),
            );
        let mut context = MetricContext::new(
            Arc::new(roll),
            Arc::new(market),
            as_of,
            Money::from((0_i64, Currency::USD)),
            MetricContext::default_config(),
        );
        let actual = RollSpecialnessCalculator
            .calculate(&mut context)
            .expect("specialness");
        let expected = (0.0375 - carry.implied_rate) * 10000.0;
        assert!(
            (actual - expected).abs() < 1e-7,
            "{actual} versus {expected}"
        );
    }
}

/// Prepayment-aware DV01 for the mortgage forwards and CMOs.
///
/// Hand calculation: on a flat 4% continuously compounded curve a ±1 bp
/// parallel bump moves the continuously compounded rate to the pool's WAM by
/// exactly ±1 bp, so the refinancing multiplier on the PSA speed is
/// `exp(∓β × 0.0001) = 2^(∓0.01)` (β = ln 2 / 1%). DV01 is the central
/// difference `(P_up − P_down) / 2` of the instrument repriced in the bumped
/// market with its pool at `PSA × 2^(∓0.01)`. Repricing with the unchanged
/// speed (the old behaviour for TBA, roll and CMO) misses the prepayment
/// response and gives a different number.
#[cfg(test)]
mod rate_risk_rebuild_tests {
    use crate::cashflow::builder::specs::{PrepaymentCurve, PrepaymentModelSpec};
    use crate::instruments::fixed_income::cmo::pricer::{price_cmo, resolve_collateral};
    use crate::instruments::fixed_income::dollar_roll::pricer::price_dollar_roll;
    use crate::instruments::fixed_income::tba::pricer::{create_assumed_pool, price_tba};
    use crate::instruments::{AgencyCmo, AgencyTba, DollarRoll, Instrument, PricingOptions};
    use crate::metrics::MetricId;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use time::macros::date;

    const AS_OF: Date = date!(2026 - 01 - 15);

    fn market() -> MarketContext {
        MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(AS_OF)
                .knots([(0.0, 1.0), (40.0, (-0.04_f64 * 40.0).exp())])
                .interp(InterpStyle::LogLinear)
                .build()
                .expect("curve"),
        )
    }

    fn bumped(bp: f64) -> MarketContext {
        market()
            .bump([MarketBump::Curve {
                id: "USD-OIS".into(),
                spec: BumpSpec::parallel_bp(bp),
            }])
            .expect("bump")
    }

    fn psa(model: &PrepaymentModelSpec) -> f64 {
        match model.curve {
            Some(PrepaymentCurve::Psa { speed_multiplier }) => speed_multiplier,
            _ => panic!("generic pools use a PSA model"),
        }
    }

    fn dv01(instrument: &dyn Instrument) -> f64 {
        instrument
            .price_with_metrics(
                &market(),
                AS_OF,
                &[MetricId::Dv01],
                PricingOptions::default(),
            )
            .expect("metrics")
            .measures["dv01"]
    }

    fn assert_prepayment_aware(actual: f64, expected: f64, static_dv01: f64) {
        assert!(
            (actual - expected).abs() < 1e-6 * expected.abs().max(1.0),
            "dv01 {actual} versus hand {expected}"
        );
        assert!(
            (actual - static_dv01).abs() > 1e-3 * static_dv01.abs(),
            "dv01 {actual} must differ from the static-prepayment {static_dv01}"
        );
    }

    #[test]
    fn tba_dv01_shifts_the_generic_pool_speed() {
        let mut tba = AgencyTba::example().expect("tba");
        tba.metric_pricing_overrides.bump_config.rate_bump_bp = Some(1.0);
        let base_psa = psa(&create_assumed_pool(&tba).expect("pool").prepayment_model);
        let at = |bp: f64| {
            let mut shifted = tba.clone();
            shifted.prepayment_model =
                Some(PrepaymentModelSpec::psa(base_psa * 2f64.powf(-0.01 * bp)));
            price_tba(&shifted, &bumped(bp), AS_OF)
                .expect("price")
                .amount()
        };
        let expected = (at(1.0) - at(-1.0)) / 2.0;
        let static_dv01 = (price_tba(&tba, &bumped(1.0), AS_OF).expect("up").amount()
            - price_tba(&tba, &bumped(-1.0), AS_OF)
                .expect("down")
                .amount())
            / 2.0;
        assert_prepayment_aware(dv01(&tba), expected, static_dv01);
    }

    #[test]
    fn dollar_roll_dv01_shifts_both_legs_pool_speed() {
        let mut roll = DollarRoll::example().expect("roll");
        roll.metric_pricing_overrides.bump_config.rate_bump_bp = Some(1.0);
        let base_psa = psa(&create_assumed_pool(&roll.front_leg().expect("leg"))
            .expect("pool")
            .prepayment_model);
        let at = |bp: f64| {
            let mut shifted = roll.clone();
            shifted.prepayment_model =
                Some(PrepaymentModelSpec::psa(base_psa * 2f64.powf(-0.01 * bp)));
            price_dollar_roll(&shifted, &bumped(bp), AS_OF)
                .expect("price")
                .amount()
        };
        let expected = (at(1.0) - at(-1.0)) / 2.0;
        let static_dv01 = (price_dollar_roll(&roll, &bumped(1.0), AS_OF)
            .expect("up")
            .amount()
            - price_dollar_roll(&roll, &bumped(-1.0), AS_OF)
                .expect("down")
                .amount())
            / 2.0;
        assert_prepayment_aware(dv01(&roll), expected, static_dv01);
    }

    #[test]
    fn cmo_dv01_shifts_the_collateral_speed() {
        let mut cmo = AgencyCmo::example().expect("cmo");
        cmo.metric_pricing_overrides.bump_config.rate_bump_bp = Some(1.0);
        let collateral = resolve_collateral(&cmo, AS_OF).expect("collateral");
        let base_psa = psa(&collateral.prepayment_model);
        let at = |bp: f64| {
            let mut pool = collateral.clone();
            pool.prepayment_model = PrepaymentModelSpec::psa(base_psa * 2f64.powf(-0.01 * bp));
            let mut shifted = cmo.clone();
            shifted.collateral = Some(Box::new(pool));
            price_cmo(&shifted, &bumped(bp), AS_OF)
                .expect("price")
                .amount()
        };
        let expected = (at(1.0) - at(-1.0)) / 2.0;
        let static_dv01 = (price_cmo(&cmo, &bumped(1.0), AS_OF).expect("up").amount()
            - price_cmo(&cmo, &bumped(-1.0), AS_OF)
                .expect("down")
                .amount())
            / 2.0;
        assert_prepayment_aware(dv01(&cmo), expected, static_dv01);
    }
}
