//! Equity option metrics module.
//!
//! Splits equity option metrics into focused calculators per greek and
//! registers them with the `MetricRegistry`. Calculators reuse the pricing
//! engine helpers to ensure consistency between PV and greeks.

mod dividend_risk;
pub(crate) mod selected_model;
mod spot_greek_decay;

use crate::equity_option::EquityOption;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::bump_scalar_price;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::{MetricContext, MetricRegistry};

/// Relative spot bump shared by the charm, speed and color stencils.
///
/// Starts from the resolved spot bump (`valuations.sensitivities.v1` layered
/// with `metric_pricing_overrides.bump_config`). When
/// `bump_config.adaptive_bumps` is set and no explicit
/// `bump_config.spot_bump_decimal` is given, the bump widens with moneyness,
/// capped at 5× the resolved bump, so deep ITM/OTM stencils stay out of the
/// noise floor.
fn spot_bump_decimal(
    context: &MetricContext,
    option: &EquityOption,
    spot: f64,
) -> finstack_quant_core::Result<f64> {
    let resolved = sens_config::resolve(context)?.spot_bump_decimal;
    let adaptive = context.get_metric_pricing_overrides().is_some_and(|po| {
        po.bump_config.adaptive_bumps && po.bump_config.spot_bump_decimal.is_none()
    });
    if adaptive {
        let moneyness = (spot - option.strike).abs() / option.strike;
        Ok(resolved * (1.0 + 2.0 * moneyness).min(5.0))
    } else {
        Ok(resolved)
    }
}

/// Register equity option metrics with the registry.
pub(crate) fn register_equity_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::sensitivities::cross_factor::{
        make_spot_bumper, make_vol_bumper, CrossFactorCalculator,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::EquityOption,
        metrics: [
            (Delta, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::equity_option::EquityOption>::delta()),
            (Gamma, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::equity_option::EquityOption>::gamma()),
            (Vega, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::equity_option::EquityOption>::vega()),
            (BucketedVega, finstack_quant_valuations_core::metrics::sensitivities::vega::KeyRateVega::<
                crate::equity_option::EquityOption,
            >::default()),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
            (Theta, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::equity_option::EquityOption>::theta()),
            (Rho, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::equity_option::EquityOption>::rho()),
            (ImpliedVol, ImpliedVolCalculator),
            (Vanna, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::equity_option::EquityOption>::vanna()),
            (Volga, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::equity_option::EquityOption>::volga()),
            (Charm, spot_greek_decay::CharmCalculator),
            (Color, spot_greek_decay::ColorCalculator),
            (Speed, SpeedCalculator),
            // Custom metric: Dividend risk (dividend yield sensitivity per 1bp)
            (Dividend01, dividend_risk::DividendRiskCalculator),
            (CrossGammaSpotVol, CrossFactorCalculator::new(make_spot_bumper, make_vol_bumper)),
        ]
    }
    Ok(())
}

/// Implied volatility calculator for equity options.
///
/// Solves for σ such that model price(σ) equals the observed option premium in
/// `instrument_pricing_overrides.market_quotes.quoted_premium`.
pub(crate) struct ImpliedVolCalculator;

impl MetricCalculator for ImpliedVolCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &EquityOption = context.instrument_as()?;
        let target_price = option
            .instrument_pricing_overrides
            .market_quotes
            .required_quoted_premium()?;
        option.implied_vol(&context.curves, context.as_of, target_price)
    }
}

/// Speed calculator for equity options.
///
/// Computes speed (∂³V/∂S³), which measures how gamma changes with spot.
///
/// Speed ≈ (Gamma(S+h) - Gamma(S-h)) / (2h)
///
/// Where Gamma(S) is computed at current spot, and Gamma(S±h) at bumped spots.
pub(crate) struct SpeedCalculator;

impl MetricCalculator for SpeedCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &EquityOption = context.instrument_as()?;
        let as_of = context.as_of;
        let base_pv = context.reprice_instrument_raw(option, &context.curves, as_of)?;

        let t = option.day_count.year_fraction(
            as_of,
            option.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        let current_spot =
            finstack_quant_valuations_core::instruments::common_impl::helpers::scalar_price_amount(
                context.curves.get_price(&option.spot_id)?,
                option.currency,
            )?;

        let bump_pct = spot_bump_decimal(context, option, current_spot)?;
        let spot_bump = current_spot * bump_pct;

        let curves_up_up = bump_scalar_price(&context.curves, &option.spot_id, 2.0 * bump_pct)?;
        let pv_up_up = context.reprice_instrument_raw(option, &curves_up_up, as_of)?;
        let curves_up = bump_scalar_price(&context.curves, &option.spot_id, bump_pct)?;
        let pv_up = context.reprice_instrument_raw(option, &curves_up, as_of)?;
        let gamma_up = (pv_up_up - 2.0 * pv_up + base_pv) / (spot_bump * spot_bump);

        let curves_down = bump_scalar_price(&context.curves, &option.spot_id, -bump_pct)?;
        let pv_down = context.reprice_instrument_raw(option, &curves_down, as_of)?;
        let curves_down_down =
            bump_scalar_price(&context.curves, &option.spot_id, -2.0 * bump_pct)?;
        let pv_down_down = context.reprice_instrument_raw(option, &curves_down_down, as_of)?;
        let gamma_down = (base_pv - 2.0 * pv_down + pv_down_down) / (spot_bump * spot_bump);

        // Speed = (Gamma(S+h) - Gamma(S-h)) / (2h)
        let speed = (gamma_up - gamma_down) / (2.0 * spot_bump);

        Ok(speed)
    }
}
