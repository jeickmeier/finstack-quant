//! Implied volatility calculator for interest rate options.
//!
//! Uses root-finding to solve for the Black volatility that reproduces the
//! observed market price of the option.
//!
//! # Limitations
//!
//! This calculator is designed for **single-period caplets/floorlets only**.
//! For multi-period caps/floors, use cap stripping to bootstrap per-caplet
//! implied volatilities. Calling this metric on a `Cap` or `Floor` instrument
//! will return an error directing the caller to the appropriate workflow.

use crate::instruments::common_impl::pricing::time::{
    rate_period_on_dates, relative_df_discount_curve,
};
use crate::instruments::rates::cap_floor::pricing::black::price_caplet_floorlet;
use crate::instruments::rates::cap_floor::pricing::payoff::CapletFloorletInputs;
use crate::instruments::rates::cap_floor::{CapFloor, RateOptionType};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::math::solver::{BrentSolver, Solver};
use finstack_core::Result;

/// Implied volatility calculator using Black model.
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
            return Err(finstack_core::Error::Validation(
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
                finstack_core::Error::Input(finstack_core::InputError::NotFound {
                    id: "Market price required for implied vol (set via pricing overrides)"
                        .to_string(),
                })
            })?;

        // Get curves from market context
        let forward_curve = context.curves.get_forward(&option.forward_curve_id)?;
        let discount_curve = context.curves.get_discount(&option.discount_curve_id)?;
        let dc_ctx = finstack_core::dates::DayCountContext::default();

        // Use the same canonical schedule the pricer uses so fixing date,
        // payment date, forward period, and accrual all match pricing exactly.
        // (Single-period caplet/floorlet, validated above, so exactly one period.)
        let period = option
            .pricing_periods()?
            .into_iter()
            .next()
            .ok_or_else(|| {
                finstack_core::Error::Validation(
                    "Implied vol requires a non-empty caplet/floorlet schedule".to_string(),
                )
            })?;
        let fixing_date = option.option_fixing_date(&period);

        // Use instrument day count for time-to-fixing (consistent with pricing and vol surface lookup)
        let time_to_fixing = if fixing_date > context.as_of {
            option
                .day_count
                .year_fraction(context.as_of, fixing_date, dc_ctx)?
        } else {
            0.0
        };

        if time_to_fixing <= 0.0 {
            return Ok(0.0); // Expired/seasoned option has no implied vol
        }

        // Use curve-consistent helpers for forward rate and discount factor
        // (same as in the main pricing implementation)
        let forward_rate = rate_period_on_dates(
            forward_curve.as_ref(),
            period.accrual_start,
            period.accrual_end,
        )?;

        // Black implied vol is only defined for a positive forward. Fail loudly
        // rather than letting the solver thrash against an undefined objective.
        if forward_rate <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "Black implied vol requires a positive forward rate; got {forward_rate:.6}. \
                 Use a normal (Bachelier) implied-vol workflow for non-positive forwards."
            )));
        }

        let discount_factor = relative_df_discount_curve(
            discount_curve.as_ref(),
            context.as_of,
            period.payment_date,
        )?;

        let accrual_fraction = period.accrual_year_fraction;
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

        // Objective function: Black price - market price = 0
        let objective = |vol: f64| {
            // Keep the objective well-defined and sign-consistent so the solver can
            // bracket a root robustly.
            //
            // At vol -> 0, the Black price approaches 0, so the residual approaches
            // `-market_price`.
            if vol <= 0.0 {
                return -market_price;
            }

            let mut inputs = base_inputs;
            inputs.volatility = vol;

            match price_caplet_floorlet(inputs) {
                Ok(price) => price.amount() - market_price,
                Err(_) => market_price, // Treat pricing failure as a large positive residual
            }
        };

        // Solve for implied volatility using Brent solver
        let mut solver = BrentSolver::new().tolerance(1e-6);
        solver.max_iterations = 50;

        // Initial guess: 20% volatility, reasonable bounds 0.1% to 300%
        let implied_vol = solver.solve(objective, 0.20)?;

        // Sanity check result
        if implied_vol > 0.0 && implied_vol < 5.0 {
            Ok(implied_vol)
        } else {
            Err(finstack_core::Error::Validation(
                "Unreasonable implied volatility".to_string(),
            ))
        }
    }
}
