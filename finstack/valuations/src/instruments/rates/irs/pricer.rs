//! Interest Rate Swap pricing implementation.
//!
//! Provides NPV calculation for vanilla IRS and OIS swaps using multi-curve
//! pricing framework (separate discount and forward curves).
//!
//! # Numerical Stability
//!
//! All PV summations use Kahan compensated summation to minimize floating-point
//! rounding errors, which is critical for long-dated swaps (30Y+) with many
//! periods. This ensures deterministic, accurate results across platforms.
//!
//! # Shared Infrastructure
//!
//! This module delegates to the shared swap leg pricing infrastructure in
//! [`crate::instruments::common_impl::pricing::swap_legs`] for the core pricing logic.
//! The shared module provides Bloomberg-validated discount factor calculations
//! and Kahan summation.
//!
//! # References
//!
//! - Hull, J. C. (2018). *Options, Futures, and Other Derivatives*. Chapter 7.
//! - Kahan, W. (1965). "Further Remarks on Reducing Truncation Errors."

// Using generic pricer implementation to eliminate boilerplate

// Re-export shared swap leg pricing utilities for internal use
use crate::instruments::common_impl::numeric::decimal_to_f64;
pub(crate) use crate::instruments::common_impl::pricing::swap_legs::robust_relative_df;
use crate::instruments::common_impl::pricing::swap_legs::LegPeriod;

use crate::instruments::rates::irs::{InterestRateSwap, PayReceive};
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::ScalarTimeSeries;
use finstack_core::math::NeumaierAccumulator;
use finstack_core::money::Money;
use finstack_core::Result;

use crate::instruments::rates::irs::FloatingLegCompounding;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::market_data::term_structures::ForwardCurve;

impl InterestRateSwap {
    /// Returns true if this swap is configured as *single-curve* compounded RFR:
    /// compounded-in-arrears and the floating index id matches the discount curve id.
    ///
    /// Note: this does **not** imply the OIS identity fast path is valid; lookback
    /// and observation shift can still require full daily compounding logic.
    pub(crate) fn is_single_curve_ois(&self) -> bool {
        matches!(
            self.float.compounding,
            FloatingLegCompounding::CompoundedInArrears { .. }
                | FloatingLegCompounding::CompoundedWithObservationShift { .. }
                | FloatingLegCompounding::CompoundedWithRateCutoff { .. }
        ) && self.float.forward_curve_id == self.fixed.discount_curve_id
    }

    /// Compute PV of an overnight-indexed (compounded-in-arrears) floating leg.
    ///
    /// This method implements market-standard compounded RFR accrual with support
    /// for lookback and observation shift. It can be used for both single-curve
    /// OIS (where `proj == disc`) and multi-curve compounded swaps.
    ///
    /// # Arguments
    ///
    /// * `disc` - Discount curve for discounting coupon payments
    /// * `proj` - Projection curve (forward curve or discount curve for OIS)
    /// * `as_of` - Valuation date
    /// * `fixings` - Optional historical fixings for seasoned swaps
    ///
    /// # Errors
    ///
    /// Returns a validation error if:
    /// - Historical fixings are required but missing for observation dates before `as_of`
    /// - Calendar or date calculations fail
    /// - Numerical stability thresholds are breached
    pub(crate) fn pv_compounded_float_leg(
        &self,
        disc: &DiscountCurve,
        proj: Option<&ForwardCurve>,
        as_of: Date,
        fixings: Option<&ScalarTimeSeries>,
    ) -> Result<f64> {
        let schedule =
            crate::instruments::rates::irs::cashflow::projected_compounded_float_leg_schedule(
                self, disc, proj, as_of, fixings,
            )?;
        let mut acc = NeumaierAccumulator::new();
        for flow in schedule.flows {
            let df = robust_relative_df(disc, as_of, flow.date)?;
            acc.add(flow.amount.amount() * df);
        }
        Ok(acc.total())
    }

    /// Compute PV of fixed leg (helper for value calculation).
    ///
    /// Discounts all future fixed coupon payments using the discount curve,
    /// applying the fixed leg's day count convention and payment schedule.
    ///
    /// # Arguments
    ///
    /// * `disc` - Discount curve for discounting cashflows
    /// * `as_of` - Valuation date (only future cashflows are included)
    ///
    /// # Returns
    ///
    /// Present value of the fixed leg in the swap's notional currency.
    ///
    /// # Numerical Stability
    ///
    /// Uses Kahan compensated summation for accurate PV calculation on
    /// long-dated swaps with many periods (30Y+ = 60+ semi-annual payments).
    /// Delegates to the shared swap leg pricing infrastructure.
    ///
    /// # Errors
    ///
    /// Returns a validation error if the valuation date discount factor is below
    /// the numerical stability threshold (DF_EPSILON = 1e-10).
    pub(crate) fn pv_fixed_leg(
        &self,
        disc: &finstack_core::market_data::term_structures::DiscountCurve,
        as_of: Date,
    ) -> finstack_core::Result<f64> {
        let sched = crate::instruments::rates::irs::cashflow::fixed_leg_schedule(self)?;

        // Convert cashflow schedule to LegPeriod iterator for shared pricing.
        //
        // `fixed_leg_schedule` delegates to the cashflow builder, whose emitted
        // fixed coupon `date` is already the adjusted payment date. The shared
        // fixed-leg pricer expects that date in `accrual_end`, so pass
        // `payment_lag_days = 0` below to avoid applying the lag twice.
        let periods = sched
            .flows
            .iter()
            .filter(|cf| {
                cf.kind == crate::cashflow::primitives::CFKind::Fixed
                    || cf.kind == crate::cashflow::primitives::CFKind::Stub
            })
            .map(|cf| LegPeriod {
                accrual_start: cf.date, // Not used by fixed-leg pricer
                accrual_end: cf.date,   // Accrual-end date; used for payment delay
                reset_date: None,
                year_fraction: cf.accrual_factor,
            });

        // Build fixed leg params
        let params = crate::instruments::common_impl::pricing::swap_legs::FixedLegParams {
            rate: decimal_to_f64(self.fixed.rate, "fixed leg rate")?,
            day_count: self.fixed.day_count,
            payment_lag_days: 0,
            calendar_id: self.fixed.calendar_id.clone(),
        };

        // Use shared pricing function
        crate::instruments::common_impl::pricing::swap_legs::pv_fixed_leg(
            periods,
            self.notional.amount(),
            &params,
            disc,
            as_of,
        )
    }

    /// Compute PV of floating leg (helper for value calculation).
    ///
    /// Projects floating rate coupons using the forward curve, applies any
    /// quoted spread, and discounts to present value. This method is used for
    /// term-rate swaps (LIBOR-style, SOFR 3M) where the floating leg requires
    /// forward rate projection.
    ///
    /// # Arguments
    ///
    /// * `disc` - Discount curve for discounting cashflows
    /// * `fwd` - Forward curve for projecting floating rates
    /// * `as_of` - Valuation date (only future cashflows are included)
    /// * `fixings` - Optional historical fixings for seasoned swaps. Required when
    ///   `reset_date < as_of` for any period; if missing, returns an error.
    ///
    /// # Returns
    ///
    /// Present value of the floating leg in the swap's notional currency.
    ///
    /// # Market Standards (ISDA 2006)
    ///
    /// - Reset dates are computed as `accrual_start - reset_lag_days`, adjusted
    ///   using the fixing calendar if specified (otherwise the payment calendar).
    /// - Forward rates are projected using the forward curve's day count convention
    ///   and base date, ensuring consistency with curve construction.
    /// - Accrual fractions use the floating leg's day count convention (e.g., ACT/360).
    /// - For seasoned swaps with past resets, historical fixings are used instead of
    ///   forward projection.
    ///
    /// # Numerical Stability
    ///
    /// Uses Kahan compensated summation for accurate PV calculation on
    /// long-dated swaps with many periods (30Y+ = 120+ quarterly payments).
    /// Delegates to the shared swap leg pricing infrastructure.
    ///
    /// # Errors
    ///
    /// Returns a validation error if:
    /// - The valuation date discount factor is below the numerical stability threshold
    /// - Historical fixings are required but not provided or missing for a reset date
    pub(crate) fn pv_float_leg(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<f64> {
        // Delegate rate projection to the cashflow builder so the pricer and
        // the signed schedule surfaced by `cashflow_schedule` / `dated_cashflows`
        // use bit-for-bit identical coupon amounts. The builder follows the
        // ISDA 2006 convention of projecting the index rate over
        // `[reset_date, reset_date + index_tenor]` (an unadjusted tenor),
        // whereas an inlined projection over the BDC-adjusted accrual window
        // introduces small but compounding mismatches across the leg.
        use finstack_core::cashflow::CFKind;

        let disc = context.get_discount(self.fixed.discount_curve_id.as_ref())?;
        let float_sched =
            crate::instruments::rates::irs::cashflow::float_leg_schedule_with_curves_as_of(
                self,
                Some(context),
                Some(as_of),
            )?;

        let mut acc = NeumaierAccumulator::new();
        for flow in float_sched.flows {
            if flow.kind != CFKind::FloatReset {
                continue;
            }
            let payment_date = flow.date;
            if payment_date <= as_of {
                continue;
            }
            let df = robust_relative_df(disc.as_ref(), as_of, payment_date)?;
            acc.add(flow.amount.amount() * df);
        }
        Ok(acc.total())
    }
}

/// Compute the net present value (NPV) of an interest rate swap.
///
/// Calculates the swap's mark-to-market value by computing the present value
/// of both fixed and floating legs, then taking their difference according to
/// the swap's pay/receive direction.
///
/// For OIS swaps (overnight-indexed with compounding), uses the discount-only
/// method. For term-rate swaps, projects floating rates from the forward curve.
///
/// # Arguments
///
/// * `irs` - The interest rate swap to value
/// * `context` - Market context containing discount and forward curves
/// * `as_of` - Valuation date
///
/// # Returns
///
/// Net present value of the swap in the notional currency. Positive values
/// indicate the swap is in-the-money for the holder (based on pay/receive side).
///
/// # Errors
///
/// Returns an error if:
/// - Required curves (discount or forward) are not found in the market context
/// - Discount factor calculations fail due to numerical instability
/// - Date calculations fail
///
/// # Examples
///
/// ```text
/// use finstack_valuations::instruments::rates::irs::{InterestRateSwap, pricer};
/// use finstack_core::market_data::context::MarketContext;
/// use finstack_core::dates::Date;
/// # use time::Month;
///
/// # fn example() -> finstack_core::Result<()> {
/// let irs = InterestRateSwap::example_standard()?;
/// // Build market context with required curves
/// let mut context = MarketContext::new();
/// // ... add USD-OIS and USD-SOFR-3M curves ...
///
/// let as_of = Date::from_calendar_date(2024, Month::January, 1)
///     .map_err(|e| finstack_core::Error::Validation(format!("{}", e)))?;
///
/// let npv = pricer::compute_pv(&irs, &context, as_of)?;
/// println!("Swap NPV: {}", npv);
/// # Ok(())
/// # }
/// ```
pub(crate) fn compute_pv(
    irs: &InterestRateSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    let npv_val = compute_pv_raw(irs, context, as_of)?;
    Ok(Money::new(npv_val, irs.notional.currency()))
}

/// Compute the raw Net Present Value (f64) without rounding.
pub(crate) fn compute_pv_raw(
    irs: &InterestRateSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    let disc = context.get_discount(irs.fixed.discount_curve_id.as_ref())?;
    let fixings = finstack_core::market_data::fixings::get_fixing_series(
        context,
        irs.float.forward_curve_id.as_str(),
    )
    .ok();
    let pv_fixed = irs.pv_fixed_leg(disc.as_ref(), as_of)?;
    let pv_float = match irs.float.compounding {
        FloatingLegCompounding::Simple => irs.pv_float_leg(context, as_of)?,
        FloatingLegCompounding::CompoundedInArrears { .. }
        | FloatingLegCompounding::CompoundedWithObservationShift { .. }
        | FloatingLegCompounding::CompoundedWithRateCutoff { .. } => {
            let proj = if irs.is_single_curve_ois() {
                context
                    .get_forward(irs.float.forward_curve_id.as_ref())
                    .ok()
            } else {
                Some(context.get_forward(irs.float.forward_curve_id.as_ref())?)
            };
            irs.pv_compounded_float_leg(disc.as_ref(), proj.as_deref(), as_of, fixings)?
        }
    };

    Ok(match irs.side {
        PayReceive::Pay => pv_float - pv_fixed,
        PayReceive::Receive => pv_fixed - pv_float,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::pricing::swap_legs::add_payment_delay;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::rates::irs::cashflow::full_signed_schedule_with_curves;
    use finstack_core::cashflow::CFKind;
    use finstack_core::currency::Currency;
    use finstack_core::dates::DayCountContext;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::scalars::ScalarTimeSeries;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::market_data::term_structures::ForwardCurve;
    use finstack_core::money::Money;
    use finstack_core::types::{CurveId, InstrumentId};
    use time::Month;

    #[test]
    fn is_single_curve_ois_classification() {
        // Start from the example vanilla IRS (term-rate style)
        let mut irs =
            InterestRateSwap::example_standard().expect("Example should construct successfully");
        assert!(
            !irs.is_single_curve_ois(),
            "Vanilla term-rate IRS with Simple compounding must not be OIS"
        );

        // Turn it into an OIS-style swap: use overnight compounding and align
        // the floating index with the fixed-leg discount curve.
        irs.float.compounding = FloatingLegCompounding::sofr();
        irs.float.forward_curve_id = irs.fixed.discount_curve_id.clone();

        assert!(
            irs.is_single_curve_ois(),
            "Swap with overnight compounding and matching index/discount curves \
             should be classified as OIS"
        );
    }

    fn date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("valid month"), d)
            .expect("valid date")
    }

    fn discount_irs_schedule(
        swap: &InterestRateSwap,
        ctx: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        let disc = ctx.get_discount(swap.fixed.discount_curve_id.as_ref())?;
        let schedule = full_signed_schedule_with_curves(swap, Some(ctx))?;
        schedule.flows.iter().try_fold(0.0, |acc, flow| {
            let payment_date = match flow.kind {
                CFKind::Fixed | CFKind::Stub => flow.date,
                CFKind::FloatReset => flow.date,
                _ => flow.date,
            };
            let df = disc.df_between_dates(as_of, payment_date)?;
            Ok(acc + flow.amount.amount() * df)
        })
    }

    #[test]
    fn compounded_ois_seasoned_uses_fixings_and_projection() {
        use finstack_core::dates::{BusinessDayConvention, DateExt, DayCount, Tenor};

        // Use August 2024 dates: no NYSE holidays between Aug 1 and Aug 30, so
        // weekday stepping (used by the hand-computed expected value) and NYSE
        // business-day stepping (used by the pricer after M-10 calendar defaulting)
        // produce identical daily schedules and the two valuations agree.
        let as_of = date(2024, 8, 14);
        let start = date(2024, 8, 1);
        let end = date(2024, 8, 30);

        let disc_id = CurveId::new("USD-OIS");
        let fwd_id = CurveId::new("USD-OIS-FWD");
        let disc_rate: f64 = 0.02;
        let df_1y = (-disc_rate).exp();
        let disc = DiscountCurve::builder(disc_id.clone())
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (1.0, df_1y)])
            .build()
            .expect("discount curve");

        let fwd_rate = 0.03;
        let fwd = ForwardCurve::builder(fwd_id.clone(), 1.0 / 12.0)
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots(vec![(0.0, fwd_rate), (1.0, fwd_rate)])
            .build()
            .expect("forward curve");

        let fixing_rate = 0.05;
        let mut obs = Vec::new();
        let mut d = start;
        while d < as_of {
            obs.push((d, fixing_rate));
            d = d.add_weekdays(1);
        }
        let fixings = ScalarTimeSeries::new(format!("FIXING:{}", fwd_id.as_str()), obs, None)
            .expect("fixings series");

        let ctx = MarketContext::new()
            .insert(disc.clone())
            .insert(fwd)
            .insert_series(fixings);

        let swap = InterestRateSwap::builder()
            .id(InstrumentId::new("OIS-SEASONED"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .side(crate::instruments::rates::irs::PayReceive::Pay)
            .fixed(
                crate::instruments::common_impl::parameters::legs::FixedLegSpec {
                    discount_curve_id: disc_id.clone(),
                    rate: rust_decimal::Decimal::ZERO,
                    frequency: Tenor::monthly(),
                    day_count: DayCount::Act360,
                    bdc: BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: finstack_core::dates::StubKind::ShortFront,
                    start,
                    end,
                    par_method: None,
                    compounding_simple: true,
                    payment_lag_days: 0,
                    end_of_month: false,
                },
            )
            .float(
                crate::instruments::common_impl::parameters::legs::FloatLegSpec {
                    discount_curve_id: disc_id.clone(),
                    forward_curve_id: fwd_id.clone(),
                    spread_bp: rust_decimal::Decimal::ZERO,
                    frequency: Tenor::monthly(),
                    day_count: DayCount::Act360,
                    bdc: BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: finstack_core::dates::StubKind::ShortFront,
                    reset_lag_days: 0,
                    fixing_calendar_id: None,
                    start,
                    end,
                    compounding: FloatingLegCompounding::fedfunds(),
                    payment_lag_days: 0,
                    end_of_month: false,
                },
            )
            .build()
            .expect("swap");

        let pv = swap.value(&ctx, as_of).expect("seasoned OIS PV");

        let mut acc = 1.0;
        let mut day = start;
        while day < end {
            let next = day.add_weekdays(1);
            let step_end = if next > end { end } else { next };
            let dcf = DayCount::Act360
                .year_fraction(day, step_end, DayCountContext::default())
                .expect("dcf");
            let r = if day < as_of { fixing_rate } else { fwd_rate };
            acc *= 1.0 + r * dcf;
            day = step_end;
        }

        let payment_date = add_payment_delay(end, 0, None).expect("payment delay");
        let df = robust_relative_df(&disc, as_of, payment_date).expect("df");
        let expected = 1_000_000.0 * (acc - 1.0) * df;

        let diff = (pv.amount() - expected).abs();
        // Allow small tolerance for day count/business day handling differences
        assert!(
            diff < 0.01,
            "Seasoned OIS PV should match fixing/projection compounding, diff={}",
            diff
        );
    }

    #[test]
    fn compounded_ois_with_lookback_uses_discount_only_projection_when_forward_missing() {
        // Single-curve OIS setups often only load the discount curve. We still want
        // lookback/shift to take effect (i.e., not be silently ignored).
        let as_of = date(2024, 1, 1);
        let start = date(2024, 2, 1);
        let end = date(2024, 5, 1);

        let disc_id = CurveId::new("USD-OIS");
        let disc = DiscountCurve::builder(disc_id.clone())
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (0.25, 0.99), (1.0, 0.95)])
            .build()
            .expect("discount curve");

        let ctx = MarketContext::new().insert(disc);

        let swap_no_lookback = InterestRateSwap::builder()
            .id(InstrumentId::new("OIS-NO-LOOKBACK"))
            .notional(Money::new(10_000_000.0, Currency::USD))
            .side(crate::instruments::rates::irs::PayReceive::Pay)
            .fixed(
                crate::instruments::common_impl::parameters::legs::FixedLegSpec {
                    discount_curve_id: disc_id.clone(),
                    rate: rust_decimal::Decimal::try_from(0.03).expect("valid"),
                    frequency: finstack_core::dates::Tenor::quarterly(),
                    day_count: finstack_core::dates::DayCount::Act360,
                    bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: finstack_core::dates::StubKind::ShortFront,
                    start,
                    end,
                    par_method: None,
                    compounding_simple: true,
                    payment_lag_days: 0,
                    end_of_month: false,
                },
            )
            .float(
                crate::instruments::common_impl::parameters::legs::FloatLegSpec {
                    discount_curve_id: disc_id.clone(),
                    forward_curve_id: disc_id.clone(), // single-curve: same id as discount
                    spread_bp: rust_decimal::Decimal::ZERO,
                    frequency: finstack_core::dates::Tenor::quarterly(),
                    day_count: finstack_core::dates::DayCount::Act360,
                    bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: finstack_core::dates::StubKind::ShortFront,
                    reset_lag_days: 0,
                    start,
                    end,
                    compounding: FloatingLegCompounding::fedfunds(), // lookback=0
                    fixing_calendar_id: None,
                    payment_lag_days: 0,
                    end_of_month: false,
                },
            )
            .build()
            .expect("swap");

        let mut swap_lookback = swap_no_lookback.clone();
        swap_lookback.id = InstrumentId::new("OIS-LOOKBACK-2D");
        // Cleared-OIS presets (sofr/sonia/...) are plain in-arrears with no
        // lookback; use an explicit FRN-style 2-day lookback to exercise the
        // lookback path without a forward curve.
        swap_lookback.float.compounding = FloatingLegCompounding::CompoundedInArrears {
            lookback_days: 2,
            observation_shift: None,
        };

        // Both should price without a forward curve present.
        let pv0 = swap_no_lookback.value(&ctx, as_of).expect("pv no lookback");
        let pv2 = swap_lookback.value(&ctx, as_of).expect("pv lookback");

        // The lookback should have a non-zero effect under a non-flat curve.
        assert!(
            (pv0.amount() - pv2.amount()).abs() > 1e-8,
            "Expected PV to differ with lookback; pv0={}, pv2={}",
            pv0.amount(),
            pv2.amount()
        );
    }

    /// Tests that compounded OIS pricing fails when fixing_calendar_id is specified but not
    /// found in the CalendarRegistry.
    ///
    /// This validates the fix for the silent fallback to weekday stepping issue identified
    /// in the quant code review. For RFR compounding (SOFR, ESTR), incorrect calendar
    /// handling produces material accrual errors.
    #[test]
    fn compounded_ois_fails_when_calendar_missing() {
        let as_of = date(2024, 1, 1);
        let start = date(2024, 2, 1);
        let end = date(2024, 5, 1);

        let disc_id = CurveId::new("USD-OIS");
        let disc = DiscountCurve::builder(disc_id.clone())
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (0.25, 0.99), (1.0, 0.95)])
            .build()
            .expect("discount curve");

        let ctx = MarketContext::new().insert(disc);

        // Create swap with an explicitly specified but non-existent fixing calendar
        let swap = InterestRateSwap::builder()
            .id(InstrumentId::new("OIS-MISSING-CAL"))
            .notional(Money::new(10_000_000.0, Currency::USD))
            .side(crate::instruments::rates::irs::PayReceive::Pay)
            .fixed(
                crate::instruments::common_impl::parameters::legs::FixedLegSpec {
                    discount_curve_id: disc_id.clone(),
                    rate: rust_decimal::Decimal::try_from(0.03).expect("valid"),
                    frequency: finstack_core::dates::Tenor::quarterly(),
                    day_count: finstack_core::dates::DayCount::Act360,
                    bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: finstack_core::dates::StubKind::ShortFront,
                    start,
                    end,
                    par_method: None,
                    compounding_simple: true,
                    payment_lag_days: 0,
                    end_of_month: false,
                },
            )
            .float(
                crate::instruments::common_impl::parameters::legs::FloatLegSpec {
                    discount_curve_id: disc_id.clone(),
                    forward_curve_id: disc_id.clone(),
                    spread_bp: rust_decimal::Decimal::ZERO,
                    frequency: finstack_core::dates::Tenor::quarterly(),
                    day_count: finstack_core::dates::DayCount::Act360,
                    bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: finstack_core::dates::StubKind::ShortFront,
                    reset_lag_days: 0,
                    start,
                    end,
                    compounding: FloatingLegCompounding::sofr(),
                    // This calendar ID does not exist in the registry
                    fixing_calendar_id: Some("NONEXISTENT-CALENDAR-XYZ".to_string()),
                    payment_lag_days: 0,
                    end_of_month: false,
                },
            )
            .build()
            .expect("swap");

        // Pricing should fail with a validation error about the missing calendar
        let result = swap.value(&ctx, as_of);
        assert!(
            result.is_err(),
            "Expected pricing to fail when fixing_calendar_id is specified but not found"
        );

        let err = result.expect_err("Expected pricing to fail");
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("NONEXISTENT-CALENDAR-XYZ") || err_msg.contains("calendar"),
            "Error message should mention the missing calendar ID, got: {}",
            err_msg
        );
    }

    /// Tests that compounded OIS pricing succeeds with convention defaults when no
    /// fixing calendar is specified.
    ///
    /// When fixing_calendar_id is None, we fall back to the rate index conventions
    /// (calendar + payment lag) instead of weekday-only stepping.
    #[test]
    fn compounded_ois_succeeds_with_weekday_stepping_when_no_calendar() {
        let as_of = date(2024, 1, 1);
        let start = date(2024, 2, 1);
        let end = date(2024, 5, 1);

        let disc_id = CurveId::new("USD-OIS");
        let disc = DiscountCurve::builder(disc_id.clone())
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (0.25, 0.99), (1.0, 0.95)])
            .build()
            .expect("discount curve");

        let ctx = MarketContext::new().insert(disc);

        // Create swap with NO fixing_calendar_id (defaults should be applied)
        let swap = InterestRateSwap::builder()
            .id(InstrumentId::new("OIS-NO-CALENDAR"))
            .notional(Money::new(10_000_000.0, Currency::USD))
            .side(crate::instruments::rates::irs::PayReceive::Pay)
            .fixed(
                crate::instruments::common_impl::parameters::legs::FixedLegSpec {
                    discount_curve_id: disc_id.clone(),
                    rate: rust_decimal::Decimal::try_from(0.03).expect("valid"),
                    frequency: finstack_core::dates::Tenor::quarterly(),
                    day_count: finstack_core::dates::DayCount::Act360,
                    bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: finstack_core::dates::StubKind::ShortFront,
                    start,
                    end,
                    par_method: None,
                    compounding_simple: true,
                    payment_lag_days: 0,
                    end_of_month: false,
                },
            )
            .float(
                crate::instruments::common_impl::parameters::legs::FloatLegSpec {
                    discount_curve_id: disc_id.clone(),
                    forward_curve_id: disc_id.clone(),
                    spread_bp: rust_decimal::Decimal::ZERO,
                    frequency: finstack_core::dates::Tenor::quarterly(),
                    day_count: finstack_core::dates::DayCount::Act360,
                    bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: finstack_core::dates::StubKind::ShortFront,
                    reset_lag_days: 0,
                    start,
                    end,
                    compounding: FloatingLegCompounding::sofr(),
                    // No fixing_calendar_id - intentional weekday stepping
                    fixing_calendar_id: None,
                    payment_lag_days: 0,
                    end_of_month: false,
                },
            )
            .build()
            .expect("swap");

        let resolved = swap.resolved_float_leg().expect("resolved float leg");
        // calendar_id: None stays None (no longer overridden by convention).
        // Users wanting convention calendars should set them explicitly.
        assert_eq!(resolved.calendar_id.as_deref(), None);
        assert_eq!(resolved.fixing_calendar_id.as_deref(), None);
        // payment_lag_days: 0 is explicitly set and stays 0 (not overridden by convention).
        // Use negative value (e.g., -1) to request convention default.
        assert_eq!(resolved.payment_lag_days, 0);

        // Pricing should succeed when defaults are applied
        let result = swap.value(&ctx, as_of);
        assert!(
            result.is_ok(),
            "Expected pricing to succeed with convention defaults, got: {:?}",
            result.err()
        );

        // Verify we get a reasonable PV (non-zero, finite)
        let pv = result.expect("Pricing should succeed");
        assert!(
            pv.amount().is_finite(),
            "PV should be finite, got: {}",
            pv.amount()
        );
    }

    #[test]
    fn compounded_ois_value_raw_matches_leg_pricer_without_forward_curve() {
        use finstack_core::dates::{BusinessDayConvention, DayCount, Tenor};

        let as_of = date(2024, 1, 1);
        let start = date(2024, 2, 1);
        let end = date(2024, 5, 1);

        let disc_id = CurveId::new("USD-OIS");
        let disc = DiscountCurve::builder(disc_id.clone())
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (0.25, 0.99), (1.0, 0.95)])
            .build()
            .expect("discount curve");
        let ctx = MarketContext::new().insert(disc.clone());

        let swap = InterestRateSwap::builder()
            .id(InstrumentId::new("OIS-VALUE-RAW-NO-FWD"))
            .notional(Money::new(10_000_000.0, Currency::USD))
            .side(crate::instruments::rates::irs::PayReceive::Pay)
            .fixed(
                crate::instruments::common_impl::parameters::legs::FixedLegSpec {
                    discount_curve_id: disc_id.clone(),
                    rate: rust_decimal::Decimal::try_from(0.03).expect("valid"),
                    frequency: Tenor::quarterly(),
                    day_count: DayCount::Act360,
                    bdc: BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: finstack_core::dates::StubKind::ShortFront,
                    start,
                    end,
                    par_method: None,
                    compounding_simple: true,
                    payment_lag_days: 0,
                    end_of_month: false,
                },
            )
            .float(
                crate::instruments::common_impl::parameters::legs::FloatLegSpec {
                    discount_curve_id: disc_id.clone(),
                    forward_curve_id: disc_id.clone(),
                    spread_bp: rust_decimal::Decimal::ZERO,
                    frequency: Tenor::quarterly(),
                    day_count: DayCount::Act360,
                    bdc: BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: finstack_core::dates::StubKind::ShortFront,
                    reset_lag_days: 0,
                    fixing_calendar_id: None,
                    start,
                    end,
                    compounding: FloatingLegCompounding::sofr(),
                    payment_lag_days: 0,
                    end_of_month: false,
                },
            )
            .build()
            .expect("swap");

        let pv_fixed = swap.pv_fixed_leg(&disc, as_of).expect("fixed leg");
        let pv_float = swap
            .pv_compounded_float_leg(&disc, None, as_of, None)
            .expect("compounded float leg");
        let expected = pv_float - pv_fixed;

        let actual = compute_pv_raw(&swap, &ctx, as_of).expect("value_raw should succeed");
        let diff = (actual - expected).abs();
        assert!(
            diff < 1e-8,
            "value_raw should use compounded OIS leg pricing, diff={diff}"
        );
    }

    #[test]
    fn compounded_ois_schedule_matches_leg_pricer_without_forward_curve() {
        use finstack_core::dates::{BusinessDayConvention, DayCount, Tenor};

        let as_of = date(2024, 1, 1);
        let start = date(2024, 2, 1);
        let end = date(2024, 5, 1);

        let disc_id = CurveId::new("USD-OIS");
        let disc = DiscountCurve::builder(disc_id.clone())
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (0.25, 0.99), (1.0, 0.95)])
            .build()
            .expect("discount curve");
        let ctx = MarketContext::new().insert(disc);

        let swap = InterestRateSwap::builder()
            .id(InstrumentId::new("OIS-SCHEDULE-NO-FWD"))
            .notional(Money::new(10_000_000.0, Currency::USD))
            .side(crate::instruments::rates::irs::PayReceive::Pay)
            .fixed(
                crate::instruments::common_impl::parameters::legs::FixedLegSpec {
                    discount_curve_id: disc_id.clone(),
                    rate: rust_decimal::Decimal::try_from(0.03).expect("valid"),
                    frequency: Tenor::quarterly(),
                    day_count: DayCount::Act360,
                    bdc: BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: finstack_core::dates::StubKind::ShortFront,
                    start,
                    end,
                    par_method: None,
                    compounding_simple: true,
                    payment_lag_days: 0,
                    end_of_month: false,
                },
            )
            .float(
                crate::instruments::common_impl::parameters::legs::FloatLegSpec {
                    discount_curve_id: disc_id.clone(),
                    forward_curve_id: disc_id.clone(),
                    spread_bp: rust_decimal::Decimal::ZERO,
                    frequency: Tenor::quarterly(),
                    day_count: DayCount::Act360,
                    bdc: BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: finstack_core::dates::StubKind::ShortFront,
                    reset_lag_days: 0,
                    fixing_calendar_id: None,
                    start,
                    end,
                    compounding: FloatingLegCompounding::sofr(),
                    payment_lag_days: 0,
                    end_of_month: false,
                },
            )
            .build()
            .expect("swap");

        let expected = compute_pv_raw(&swap, &ctx, as_of).expect("leg-pricer pv");
        let actual = discount_irs_schedule(&swap, &ctx, as_of).expect("schedule pv");
        let diff = (actual - expected).abs();
        assert!(
            diff < 1e-8,
            "canonical IRS schedule should support single-curve compounded OIS, diff={diff}"
        );
    }

    #[test]
    fn compounded_ois_schedule_matches_seasoned_fixings_and_projection() {
        use finstack_core::dates::{BusinessDayConvention, DateExt, DayCount, Tenor};

        let as_of = date(2024, 8, 14);
        let start = date(2024, 8, 1);
        let end = date(2024, 8, 30);

        let disc_id = CurveId::new("USD-OIS");
        let fwd_id = CurveId::new("USD-OIS-FWD");
        let disc_rate: f64 = 0.02;
        let df_1y = (-disc_rate).exp();
        let disc = DiscountCurve::builder(disc_id.clone())
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (1.0, df_1y)])
            .build()
            .expect("discount curve");

        let fwd_rate = 0.03;
        let fwd = ForwardCurve::builder(fwd_id.clone(), 1.0 / 12.0)
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots(vec![(0.0, fwd_rate), (1.0, fwd_rate)])
            .build()
            .expect("forward curve");

        let fixing_rate = 0.05;
        let mut obs = Vec::new();
        let mut d = start;
        while d < as_of {
            obs.push((d, fixing_rate));
            d = d.add_weekdays(1);
        }
        let fixings = ScalarTimeSeries::new(format!("FIXING:{}", fwd_id.as_str()), obs, None)
            .expect("fixings series");

        let ctx = MarketContext::new()
            .insert(disc)
            .insert(fwd)
            .insert_series(fixings);

        let swap = InterestRateSwap::builder()
            .id(InstrumentId::new("OIS-SCHEDULE-SEASONED"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .side(crate::instruments::rates::irs::PayReceive::Pay)
            .fixed(
                crate::instruments::common_impl::parameters::legs::FixedLegSpec {
                    discount_curve_id: disc_id.clone(),
                    rate: rust_decimal::Decimal::ZERO,
                    frequency: Tenor::monthly(),
                    day_count: DayCount::Act360,
                    bdc: BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: finstack_core::dates::StubKind::ShortFront,
                    start,
                    end,
                    par_method: None,
                    compounding_simple: true,
                    payment_lag_days: 0,
                    end_of_month: false,
                },
            )
            .float(
                crate::instruments::common_impl::parameters::legs::FloatLegSpec {
                    discount_curve_id: disc_id.clone(),
                    forward_curve_id: fwd_id.clone(),
                    spread_bp: rust_decimal::Decimal::ZERO,
                    frequency: Tenor::monthly(),
                    day_count: DayCount::Act360,
                    bdc: BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: finstack_core::dates::StubKind::ShortFront,
                    reset_lag_days: 0,
                    fixing_calendar_id: None,
                    start,
                    end,
                    compounding: FloatingLegCompounding::fedfunds(),
                    payment_lag_days: 0,
                    end_of_month: false,
                },
            )
            .build()
            .expect("swap");

        let expected = compute_pv_raw(&swap, &ctx, as_of).expect("leg-pricer pv");
        let actual = discount_irs_schedule(&swap, &ctx, as_of).expect("schedule pv");
        let diff = (actual - expected).abs();
        assert!(
            diff < 1e-8,
            "canonical IRS schedule should preserve seasoned compounded OIS economics, diff={diff}"
        );
    }

    /// Test OIS floating leg PV matches analytical identity: PV_float = N × (DF(start) - DF(end))
    ///
    /// For single-curve OIS with no lookback, no observation shift, and no spread,
    /// the compounded floating leg PV should exactly equal the discount factor identity.
    /// This is a fundamental property of OIS pricing that must hold.
    ///
    /// # References
    ///
    /// - Hull, J.C. "Options, Futures, and Other Derivatives", Chapter 7
    /// - The identity follows from: ∏(1 + r_i × dcf_i) = DF(start)/DF(end)
    ///   when r_i is derived from the discount curve.
    #[test]
    fn ois_floating_leg_matches_analytical_identity() {
        use finstack_core::dates::{BusinessDayConvention, DayCount, Tenor};

        let as_of = date(2024, 1, 2); // Tuesday
                                      // Use dates that don't require business day adjustment (both weekdays)
        let start = date(2024, 3, 4); // Monday
        let end = date(2024, 6, 3); // Monday

        // Create a non-flat curve to make the test meaningful
        let disc_id = CurveId::new("USD-OIS");
        let disc = DiscountCurve::builder(disc_id.clone())
            .base_date(as_of)
            .knots(vec![
                (0.0, 1.0),
                (0.25, 0.9925), // ~3% rate
                (0.5, 0.9850),  // ~3% rate
                (1.0, 0.9650),  // ~3.5% rate
            ])
            .build()
            .expect("discount curve");

        let _ctx = MarketContext::new().insert(disc.clone());

        // Create an OIS swap with NO lookback, NO observation shift, NO spread.
        // These are the conditions under which the identity is exact.
        let swap = InterestRateSwap::builder()
            .id(InstrumentId::new("OIS-IDENTITY-TEST"))
            .notional(Money::new(10_000_000.0, Currency::USD))
            .side(crate::instruments::rates::irs::PayReceive::Pay)
            .fixed(
                crate::instruments::common_impl::parameters::legs::FixedLegSpec {
                    discount_curve_id: disc_id.clone(),
                    rate: rust_decimal::Decimal::ZERO, // Zero fixed rate for this test
                    frequency: Tenor::quarterly(),
                    day_count: DayCount::Act360,
                    bdc: BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: finstack_core::dates::StubKind::ShortFront,
                    start,
                    end,
                    par_method: None,
                    compounding_simple: true,
                    payment_lag_days: 0, // No payment delay for exact identity
                    end_of_month: false,
                },
            )
            .float(
                crate::instruments::common_impl::parameters::legs::FloatLegSpec {
                    discount_curve_id: disc_id.clone(),
                    forward_curve_id: disc_id.clone(), // Single-curve: forward = discount
                    spread_bp: rust_decimal::Decimal::ZERO, // No spread
                    frequency: Tenor::quarterly(),
                    day_count: DayCount::Act360,
                    bdc: BusinessDayConvention::ModifiedFollowing,
                    calendar_id: None,
                    stub: finstack_core::dates::StubKind::ShortFront,
                    reset_lag_days: 0,
                    start,
                    end,
                    compounding: FloatingLegCompounding::CompoundedInArrears {
                        lookback_days: 0,        // No lookback
                        observation_shift: None, // No observation shift
                    },
                    fixing_calendar_id: None,
                    payment_lag_days: 0, // No payment delay
                    end_of_month: false,
                },
            )
            .build()
            .expect("swap");

        // Calculate floating leg PV using the pricer
        let pv_float = swap
            .pv_compounded_float_leg(&disc, None, as_of, None)
            .expect("float leg PV");

        // Calculate analytical identity: N × (DF(start) - DF(end))
        // Note: This is the PV of receiving the floating leg payments
        let df_start = disc.df_between_dates(as_of, start).expect("df_start");
        let df_end = disc.df_between_dates(as_of, end).expect("df_end");
        let expected_pv = swap.notional.amount() * (df_start - df_end);

        // The identity should hold to high precision (< 1 currency unit on 10MM)
        let error = (pv_float - expected_pv).abs();
        assert!(
            error < 1.0,
            "OIS floating leg identity violated!\n\
             Computed PV:  {:.6}\n\
             Expected PV:  {:.6}\n\
             Error:        {:.6}\n\
             DF(start):    {:.6}\n\
             DF(end):      {:.6}",
            pv_float,
            expected_pv,
            error,
            df_start,
            df_end
        );

        // Additional check: error should be < 0.01% of notional
        let relative_error = error / swap.notional.amount();
        assert!(
            relative_error < 1e-6,
            "OIS identity relative error too large: {:.2e}",
            relative_error
        );
    }
}
