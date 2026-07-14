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

use crate::instruments::rates::cap_floor::pricing::black::price_caplet_floorlet;
use crate::instruments::rates::cap_floor::pricing::normal;
use crate::instruments::rates::cap_floor::pricing::payoff::CapletFloorletInputs;
use crate::instruments::rates::cap_floor::pricing::pricer::price_lognormal_quote_with_fallback;
use crate::instruments::rates::cap_floor::pricing::projection::resolve_optioned_caplet_inputs;
use crate::instruments::rates::cap_floor::{CapFloor, CapFloorVolType, RateOptionType};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::Result;

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
        let vol_shift = option.resolved_vol_shift();
        let resolved_vol_type = option.vol_type;
        match resolved_vol_type {
            CapFloorVolType::Lognormal if forward_rate <= 0.0 || strike <= 0.0 => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Lognormal implied vol requires positive forward and strike; got \
                     forward={forward_rate:.6}, strike={strike:.6}"
                )));
            }
            CapFloorVolType::ShiftedLognormal
                if forward_rate + vol_shift <= 0.0 || strike + vol_shift <= 0.0 =>
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Shifted-lognormal implied vol requires positive shifted forward and strike; \
                     got forward+shift={:.6}, strike+shift={:.6}",
                    forward_rate + vol_shift,
                    strike + vol_shift
                )));
            }
            _ => {}
        }

        let discount_factor = resolved_inputs.discount_factor;

        let accrual_fraction = projection.accrual_year_fraction;
        let is_cap = matches!(
            option.rate_option_type,
            RateOptionType::Cap | RateOptionType::Caplet
        );

        // Set up inputs for Black model
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

        // Objective function: convention-specific model price - market price = 0.
        let objective = |vol: f64| {
            let mut inputs = base_inputs;
            inputs.volatility = vol.max(0.0);
            let price = match resolved_vol_type {
                CapFloorVolType::Normal => normal::price_caplet_floorlet(inputs),
                CapFloorVolType::Lognormal => price_caplet_floorlet(inputs),
                CapFloorVolType::ShiftedLognormal => price_caplet_floorlet(CapletFloorletInputs {
                    forward: inputs.forward + vol_shift,
                    strike: inputs.strike + vol_shift,
                    ..inputs
                }),
                CapFloorVolType::Auto => price_lognormal_quote_with_fallback(inputs),
            };
            match price {
                Ok(price) => price.amount() - market_price,
                Err(_) => f64::NAN,
            }
        };

        // Solve for implied volatility using Brent solver
        let mut solver = BrentSolver::new().tolerance(1e-6);
        solver.max_iterations = 50;

        let initial_guess = match resolved_vol_type {
            CapFloorVolType::Normal => 0.01,
            _ => 0.20,
        };
        let implied_vol = solver.solve(objective, initial_guess)?;

        // Sanity check result
        if implied_vol > 0.0 && implied_vol < 5.0 {
            Ok(implied_vol)
        } else {
            Err(finstack_quant_core::Error::Validation(
                "Unreasonable implied volatility".to_string(),
            ))
        }
    }
}
