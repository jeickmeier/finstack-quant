//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::instruments::fixed_income::bond::pricing::quote_conversions::periods_per_year;
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::Bond;
use crate::market::conventions::ConventionRegistry;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, StubKind, Tenor};
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, RateCalibrationPillar, RateCalibrationQuote,
};

/// I-Spread: bond yield minus the interpolated swap par rate at the same horizon.
///
/// ```text
/// I-Spread = yield − par_swap_rate(quote_date → horizon)
/// ```
///
/// Both rates compound at the bond's coupon frequency.
///
/// For a callable or puttable bond with a quoted price the yield is the
/// yield-to-worst and the horizon is the workout date; otherwise the yield is
/// the Street YTM and the horizon is maturity. The par rate comes from
/// [`i_spread_par_rate`], which the quote engine's inverse also uses.
///
/// # Dependencies
///
/// Requires `Ytm` metric to be computed first.
#[derive(Debug, Clone, Default)]
pub(crate) struct ISpreadCalculator;

impl MetricCalculator for ISpreadCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Ytm]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let workout =
            crate::instruments::fixed_income::bond::metrics::context_workout_path(context)?;
        let bond: &Bond = context.instrument_as()?;

        let ytm = context
            .computed
            .get(&MetricId::Ytm)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:Ytm".to_string(),
                })
            })?;

        let disc = context.curves.get_discount(&bond.discount_curve_id)?;
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        let (yield_rate, horizon) = match workout {
            Some((workout_yield, workout_flows, _)) => (
                workout_yield,
                workout_flows
                    .last()
                    .map_or(bond.maturity, |(date, _)| *date),
            ),
            None => (ytm, bond.maturity),
        };
        Ok(yield_rate - i_spread_par_rate(bond, disc.as_ref(), quote_ctx.quote_date, horizon)?)
    }
}

/// Swap par rate from `quote_date` to `horizon` used as the I-spread benchmark,
/// expressed on the bond's yield compounding frequency.
///
/// Interpolates the curve's swap calibration quotes when the discount curve
/// carries them; otherwise builds a proxy fixed leg on the bond's coupon
/// frequency and day count (annual ACT/ACT for non-fixed coupons) and returns
/// `(DF(start) − DF(end)) / annuity` on the bond's discount curve.
///
/// A swap quote compounds at its own fixed-leg frequency (annual for SOFR
/// OIS), while the yield it is subtracted from compounds at the bond's coupon
/// frequency. Each quote is therefore restated before interpolation:
///
/// ```text
/// r_bond = m × ((1 + r_swap / n)^(n / m) − 1)
/// ```
///
/// with `n` the swap's fixed-leg payments per year and `m` the bond's coupon
/// payments per year. The proxy leg already pays on the bond's frequency.
///
/// # Arguments
///
/// * `bond` - Bond supplying the yield compounding frequency and the proxy
///   fixed-leg conventions.
/// * `disc` - The bond's discount curve, used as the swap curve proxy.
/// * `quote_date` - Settlement/quote date where the proxy leg starts.
/// * `horizon` - Workout or maturity date where the proxy leg ends.
///
/// # Errors
///
/// Returns an error when a swap calibration quote references a rate index
/// with no registered fixed-leg conventions, or when the proxy fixed leg has
/// fewer than two schedule dates or a near-zero annuity.
pub(crate) fn i_spread_par_rate(
    bond: &Bond,
    disc: &DiscountCurve,
    quote_date: Date,
    horizon: Date,
) -> finstack_quant_core::Result<f64> {
    let (day_count, frequency) = match &bond.cashflow_spec {
        crate::instruments::fixed_income::bond::CashflowSpec::Fixed(spec) => {
            (spec.schedule.day_count, spec.schedule.frequency)
        }
        _ => (DayCount::ActAct, Tenor::annual()),
    };
    if let Some(par_swap_rate) = interpolated_swap_quote_rate(disc, quote_date, horizon, frequency)?
    {
        return Ok(par_swap_rate);
    }
    let dates: Vec<Date> = finstack_quant_core::dates::ScheduleBuilder::new(quote_date, horizon)?
        .frequency(frequency)
        .stub_rule(StubKind::ShortFront)
        .build()?
        .into_iter()
        .collect();
    if dates.len() < 2 {
        return Err(finstack_quant_core::Error::Validation(
            "I-spread calculation requires at least two fixed-leg schedule dates".to_string(),
        ));
    }
    let (par_swap_rate, annuity) =
        crate::instruments::fixed_income::bond::pricing::quote_conversions::par_rate_and_annuity_from_discount(
            disc,
            day_count,
            Some(frequency),
            &dates,
        )?;
    if annuity.abs() < 1e-12 {
        return Err(finstack_quant_core::Error::Validation(
            "I-spread calculation is undefined for near-zero fixed-leg annuity".to_string(),
        ));
    }
    Ok(par_swap_rate)
}

/// Interpolate the curve's swap calibration quotes at `maturity`, restating
/// each quote from its fixed-leg frequency to `yield_frequency` first.
fn interpolated_swap_quote_rate(
    disc: &DiscountCurve,
    quote_date: Date,
    maturity: Date,
    yield_frequency: Tenor,
) -> finstack_quant_core::Result<Option<f64>> {
    let Some(calibration) = disc.rate_calibration() else {
        return Ok(None);
    };
    let yield_periods = periods_per_year(yield_frequency)?;
    let mut swap_quotes = Vec::new();
    for quote in &calibration.quotes {
        let RateCalibrationQuote::Swap {
            index_id,
            pillar,
            rate,
            ..
        } = quote
        else {
            continue;
        };
        let time = match pillar {
            RateCalibrationPillar::Tenor(tenor) => tenor.to_years(),
            RateCalibrationPillar::Date(date) => disc.day_count().year_fraction(
                disc.base_date(),
                *date,
                DayCountContext::default(),
            )?,
        };
        let swap_periods = periods_per_year(
            ConventionRegistry::try_global()?
                .require_rate_index(index_id)?
                .default_fixed_leg_frequency,
        )?;
        let restated =
            yield_periods * ((1.0 + rate / swap_periods).powf(swap_periods / yield_periods) - 1.0);
        swap_quotes.push((time, restated));
    }
    if swap_quotes.len() < 2 {
        return Ok(None);
    }
    swap_quotes.sort_by(|left, right| left.0.total_cmp(&right.0));

    let target = disc.day_count().year_fraction(
        quote_date,
        maturity,
        finstack_quant_core::dates::DayCountContext::default(),
    )?;
    if target <= swap_quotes[0].0 {
        return Ok(Some(swap_quotes[0].1));
    }
    for pair in swap_quotes.windows(2) {
        let (t0, r0) = pair[0];
        let (t1, r1) = pair[1];
        if target <= t1 {
            let weight = (target - t0) / (t1 - t0);
            return Ok(Some(r0 + weight * (r1 - r0)));
        }
    }
    Ok(swap_quotes.last().map(|(_, rate)| *rate))
}
