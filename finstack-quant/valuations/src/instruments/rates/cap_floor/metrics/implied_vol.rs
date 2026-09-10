//! Implied volatility calculator for interest rate options.
//!
//! Uses root-finding with the instrument's volatility convention: Bachelier for
//! normal vol, Black for lognormal vol, and shifted Black for shifted-lognormal vol.
//!
//! # Limitations
//!
//! This calculator is designed for **single-period caplets/floorlets only**.
//! For multi-period caps/floors, use cap stripping to bootstrap per-caplet
//! implied volatilities. Calling this metric on a `Cap` or `Floor` instrument
//! will return an error directing the caller to the appropriate workflow.

use crate::instruments::common_impl::vol_resolution::ResolvedVolatility;
use crate::instruments::rates::cap_floor::pricing::payoff::CapletFloorletInputs;
use crate::instruments::rates::cap_floor::pricing::pricer::{
    price_caplet_quote, resolve_caplet_volatility,
};
use crate::instruments::rates::cap_floor::pricing::projection::resolve_optioned_caplet_inputs;
use crate::instruments::rates::cap_floor::{CapFloor, RateOptionType};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::math::solver::BrentSolver;
use finstack_quant_core::Result;
use finstack_quant_models::volatility::VolatilityConvention;

/// Implied volatility calculator using the cap/floor's quoted-volatility model.
///
/// # Supported Instruments
///
/// Only `Caplet` and `Floorlet` (single-period) instruments are supported.
/// Calling this metric on a multi-period `Cap` or `Floor` will return an error.
/// For multi-period instruments, use cap stripping to extract per-caplet vols.
pub(crate) struct ImpliedVolCalculator;

impl MetricCalculator for ImpliedVolCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CapFloor = context.instrument_as()?;
        let strike = option.strike_f64()?;

        // Implied vol is only well-defined for single-period caplets/floorlets.
        // For multi-period caps/floors, a flat implied vol would require cap stripping
        // (bootstrapping per-caplet vols), which is not supported here.
        if matches!(
            option.rate_option_type,
            RateOptionType::Cap | RateOptionType::Floor
        ) {
            return Err(finstack_quant_core::Error::Validation(
                "ImpliedVol is only supported for single-period Caplet/Floorlet instruments. \
                 For multi-period Cap/Floor instruments, use cap stripping to extract per-caplet \
                 implied volatilities."
                    .to_string(),
            ));
        }

        // Need market price to solve for implied volatility.
        // The quoted_clean_price is passed via the MetricContext pricing overrides,
        // not stored on the instrument itself.
        let market_price = context
            .get_instrument_overrides()
            .and_then(|po| po.market_quotes.quoted_clean_price)
            .ok_or_else(|| {
                finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                    id: "Market price required for implied vol (set via pricing overrides)"
                        .to_string(),
                })
            })?;

        // Use the same canonical schedule the pricer uses so fixing date,
        // payment date, forward period, and accrual all match pricing exactly.
        // (Single-period caplet/floorlet, validated above, so exactly one period.)
        let period = option
            .pricing_periods()?
            .into_iter()
            .next()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "Implied vol requires a non-empty caplet/floorlet schedule".to_string(),
                )
            })?;
        if period.payment_date <= context.as_of {
            return Ok(0.0);
        }
        let resolved_inputs = resolve_optioned_caplet_inputs(
            option,
            &period,
            context.curves.as_ref(),
            context.as_of,
        )?;
        let projection = &resolved_inputs.coupon;

        let time_to_fixing = resolved_inputs.time_to_fixing;

        if time_to_fixing <= 0.0 {
            return Ok(0.0); // Expired/seasoned option has no implied vol
        }

        // Use curve-consistent helpers for forward rate and discount factor
        // (same as in the main pricing implementation)
        let forward_rate = projection.forward;
        let mut quote_option = option.clone();
        quote_option
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.0);
        let quote = resolve_caplet_volatility(
            &quote_option,
            context.curves.as_ref(),
            time_to_fixing,
            strike,
        )?;
        quote.model_rates(forward_rate, strike)?;
        if !market_price.is_finite() || market_price < 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "implied volatility requires a finite non-negative option price".to_owned(),
            ));
        }

        let discount_factor = resolved_inputs.discount_factor;

        let accrual_fraction = projection.accrual_year_fraction;
        let is_cap = matches!(
            option.rate_option_type,
            RateOptionType::Cap | RateOptionType::Caplet
        );

        let base_inputs = CapletFloorletInputs {
            is_cap,
            notional: option.notional.amount(),
            strike,
            forward: forward_rate,
            discount_factor,
            volatility: 0.0, // Will be varied in solver
            time_to_fixing,
            accrual_year_fraction: accrual_fraction,
            currency: option.notional.currency(),
        };

        let objective = |sigma: f64| {
            price_caplet_quote(base_inputs, ResolvedVolatility { sigma, ..quote })
                .map_or(f64::NAN, |price| {
                    (price.amount() - market_price) / option.notional.amount()
                })
        };
        let mut solver = BrentSolver::new().tolerance(1e-12);
        solver.max_iterations = 100;
        let upper = match quote.convention {
            VolatilityConvention::Normal => 1.0,
            _ => 5.0,
        };
        let implied_vol = solver.solve_in_bracket(objective, 0.0, upper)?;

        // Sanity check result
        if implied_vol >= 0.0 && implied_vol <= upper {
            Ok(implied_vol)
        } else {
            Err(finstack_quant_core::Error::Validation(
                "Unreasonable implied volatility".to_string(),
            ))
        }
    }
}
