//! Dividend risk calculator for equity options.
//!
//! Dividend01 is the change in PV for a 1bp (0.0001) move in dividend yield;
//! see [`crate::instruments::equity::dividend01`] for the finite-difference
//! contract. For options, dividend yield affects the forward price
//! `F = S * exp((r - q) * T)`: a higher yield reduces the forward, making
//! calls less valuable and puts more valuable. For cash schedules, a basis-point
//! shock moves the prepaid-forward-equivalent yield and proportionally scales
//! future cash dividends while preserving dates.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::dividend01::{dividend01_central_diff, DIVIDEND_BUMP_BP};
use crate::instruments::equity::equity_option::EquityOption;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Dividend risk calculator for equity options.
pub(crate) struct DividendRiskCalculator;

impl MetricCalculator for DividendRiskCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &EquityOption = context.instrument_as()?;

        let t = option.day_count.year_fraction(
            context.as_of,
            option.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        let future = option
            .discrete_dividends
            .iter()
            .filter(|(date, _)| *date > context.as_of && *date <= option.expiry)
            .collect::<Vec<_>>();
        if future.is_empty() {
            return dividend01_central_diff(option, option.div_yield_id.as_ref(), context);
        }
        let discount = context
            .curves
            .get_discount(option.discount_curve_id.as_str())?;
        let mut dividend_pv = 0.0;
        for (date, amount) in future {
            dividend_pv += amount * discount.df_between_dates(context.as_of, *date)?;
        }
        let inputs =
            super::super::pricing::collect_inputs_extended(option, &context.curves, context.as_of)?;
        let raw_spot = inputs.spot + dividend_pv;
        if dividend_pv <= 0.0 || inputs.spot <= 0.0 || !dividend_pv.is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "discrete Dividend01 requires positive dividend PV and prepaid spot".into(),
            ));
        }
        // Express the cash schedule as its prepaid-forward-equivalent yield,
        // q = -ln((S-PV(D))/S)/T. Move q by one bp, scaling future cash amounts
        // proportionally and preserving their dates. Past dividends stay fixed.
        let q = -(inputs.spot / raw_spot).ln() / t;
        let q_down = (q - DIVIDEND_BUMP_BP).max(0.0);
        let q_up = q + DIVIDEND_BUMP_BP;
        let price = |yield_quote: f64| -> Result<f64> {
            let scale = raw_spot * -(-yield_quote * t).exp_m1() / dividend_pv;
            let mut bumped = option.clone();
            bumped.div_yield_id = None;
            for (date, amount) in &mut bumped.discrete_dividends {
                if *date > context.as_of && *date <= option.expiry {
                    *amount *= scale;
                }
            }
            bumped.discrete_dividends.retain(|(date, amount)| {
                *date <= context.as_of || *date > option.expiry || *amount > 0.0
            });
            Ok(bumped.value(&context.curves, context.as_of)?.amount())
        };
        crate::metrics::scaled_central_diff_by_width(
            price(q_up)?,
            price(q_down)?,
            q_up - q_down,
            DIVIDEND_BUMP_BP,
        )
    }
}
