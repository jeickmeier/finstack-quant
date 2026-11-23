//! Revolving-credit payment/reset schedule construction and floating-rate projection.

use super::super::types::RevolvingCredit;
use finstack_quant_cashflows::builder::FloatingLegCompounding;
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DateExt, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_valuations_core::instruments::pricing::overnight::{
    OvernightCouponProjectionInput, OvernightProjectionCurve, adjust_overnight_accrual_boundaries,
    project_overnight_coupon,
};
use finstack_quant_valuations_core::instruments::pricing::overnight_conventions;

/// Historical fixing series of a floating-rate facility's index, when the
/// market carries one; `Ok(None)` for fixed-rate facilities or a missing series.
pub(crate) fn resolve_fixings<'a>(
    facility: &RevolvingCredit,
    market: &'a MarketContext,
) -> Result<Option<&'a ScalarTimeSeries>> {
    match &facility.rate {
        RateSpec::Floating(spec) => {
            let index_id = finstack_quant_core::market_data::fixings::validate_fixing_index_id(
                spec.index_id.as_ref(),
            )?;
            let series_id = finstack_quant_core::market_data::fixings::fixing_series_id(index_id);
            match market.get_series(&series_id) {
                Ok(series) => Ok(Some(series)),
                Err(err) if err.kind() == finstack_quant_core::ErrorKind::NotFound => Ok(None),
                Err(err) => Err(err),
            }
        }
        RateSpec::Fixed { .. } => Ok(None),
    }
}

/// Build the canonical accrual/payment periods for a revolving credit facility.
///
/// Accrual boundaries remain unadjusted while payment dates follow the
/// facility's `business_day_convention`, `calendar_id` and
/// `payment_lag_days`. Keeping the complete period objects prevents
/// payment-date adjustment from changing contractual accrual.
pub(crate) fn build_payment_periods(
    facility: &RevolvingCredit,
) -> Result<Vec<finstack_quant_cashflows::builder::SchedulePeriod>> {
    use finstack_quant_cashflows::builder::{BuildPeriodsParams, build_periods};

    let periods = build_periods(BuildPeriodsParams {
        start: facility.issue_date,
        end: facility.maturity,
        frequency: facility.frequency,
        stub: facility.stub,
        business_day_convention: facility.business_day_convention,
        calendar_id: schedule_calendar_id(facility),
        end_of_month: false,
        day_count: facility.day_count,
        payment_lag_days: facility.payment_lag_days as i32,
        reset_lag_days: None,
        adjust_accrual_dates: false,
        roll_rule: finstack_quant_cashflows::builder::RollRule::None,
    })?;

    if periods.is_empty() {
        return Err(finstack_quant_core::InputError::TooFewPoints.into());
    }
    Ok(periods)
}

/// Build payment schedule dates for a revolving credit facility.
///
/// Generates a payment schedule from commitment to maturity with the facility's
/// payment frequency, applying calendar adjustments if configured.
///
/// # Arguments
///
/// * `facility` - The revolving credit facility
///
/// # Returns
///
/// The commitment date followed by each period's payment date.
///
/// # Errors
///
/// Returns an error if the schedule builder fails or produces fewer than 2 dates.
pub(crate) fn build_payment_dates(facility: &RevolvingCredit) -> Result<Vec<Date>> {
    let periods = build_payment_periods(facility)?;
    let payment_dates: Vec<Date> = std::iter::once(facility.issue_date)
        .chain(periods.into_iter().map(|period| period.payment_date))
        .collect();

    if payment_dates.len() < 2 {
        return Err(finstack_quant_core::InputError::TooFewPoints.into());
    }

    Ok(payment_dates)
}

/// Build contractual accrual-boundary dates used for MC state observations.
///
/// The first date is the commitment date and each subsequent date is an
/// unadjusted accrual end. Payment adjustment is applied only when emitting the
/// corresponding cashflow.
pub(crate) fn build_accrual_boundary_dates(facility: &RevolvingCredit) -> Result<Vec<Date>> {
    let periods = build_payment_periods(facility)?;
    Ok(std::iter::once(facility.issue_date)
        .chain(periods.into_iter().map(|period| period.accrual_end))
        .collect())
}

/// Build the Monte Carlo observation dates: contractual accrual boundaries
/// plus every floating-rate reset date strictly inside the facility life,
/// sorted and deduplicated.
///
/// The path generator records factor state on these dates, so a reset
/// frequency shorter than the payment frequency sees a fresh short-rate
/// observation at each reset rather than at the accrual-period start only.
pub(crate) fn build_observation_dates(facility: &RevolvingCredit) -> Result<Vec<Date>> {
    let mut dates = build_accrual_boundary_dates(facility)?;
    if let Some(resets) = build_reset_dates(facility)? {
        dates.extend(
            resets
                .into_iter()
                .filter(|&reset| reset > facility.issue_date && reset < facility.maturity),
        );
    }
    // Commitment, margin and fee steps slice accrual in both engines, so the
    // path is observed on them too.
    dates.extend(
        facility
            .step_dates()
            .into_iter()
            .filter(|&step| step > facility.issue_date && step < facility.maturity),
    );
    dates.sort_unstable();
    dates.dedup();
    Ok(dates)
}

/// Floating-rate parameters in force on `date`: the facility spec with the
/// cumulative margin step delta applied to the spread.
///
/// # Arguments
///
/// * `facility` - Facility carrying `margin_steps`.
/// * `spec` - The facility's floating-rate specification.
/// * `date` - Accrual date the margin is evaluated on.
pub(crate) fn floating_params_at(
    facility: &RevolvingCredit,
    spec: &finstack_quant_cashflows::builder::FloatingRateSpec,
    date: Date,
) -> Result<finstack_quant_cashflows::builder::FloatingRateParams> {
    let mut params = finstack_quant_cashflows::builder::FloatingRateParams::try_from(spec)?;
    params.spread_bp += facility.margin_delta_bp_at(date);
    Ok(params)
}

/// Fixed all-in rate in force on `date`: the contractual rate plus the
/// cumulative margin step delta.
///
/// # Arguments
///
/// * `facility` - Facility carrying `margin_steps`.
/// * `rate` - Contractual fixed rate, decimal.
/// * `date` - Accrual date the margin is evaluated on.
pub(crate) fn fixed_rate_at(facility: &RevolvingCredit, rate: f64, date: Date) -> f64 {
    rate + facility.margin_delta_bp_at(date) * 1e-4
}

/// Deterministic index-over-OIS basis at `date`: the term index forward minus
/// the discount curve's instantaneous forward on the ACT/365F model clock.
///
/// The stochastic-rates path simulates the OIS numeraire short rate on the
/// model clock anchored at `anchor` (the same clock `prepare_hw1f_params` and
/// `initial_short_rate_from_curve` use), so a term-index fixing is rebuilt as
/// `F_index(t) + (r_t − f_OIS(t))`. Adding the basis keeps the σ → 0 limit
/// equal to the deterministic index forward and preserves the OIS/index basis
/// in expectation; using the bare short rate silently priced every index
/// coupon off the discount curve.
///
/// # Arguments
///
/// * `date` - Reset-effective date of the fixing being projected.
/// * `anchor` - Simulation anchor (t = 0 of the short-rate process): the later
///   of the valuation and commitment dates.
/// * `fwd` - Term index forward curve (e.g. `USD-SOFR-3M`).
/// * `disc` - Facility discount curve the Hull-White process was fitted to.
/// * `accrual_start` - Inclusive start of the full contractual payment coupon.
/// * `accrual_end` - Exclusive end of that coupon, before payment adjustment.
/// * `accrual_year_fraction` - Full coupon fraction on the facility's contractual
///   day count, retaining any context-sensitive denominator.
pub(crate) fn index_basis_at(
    date: Date,
    anchor: Date,
    fwd: &ForwardCurve,
    disc: &dyn Discounting,
    accrual_start: Date,
    accrual_end: Date,
    accrual_year_fraction: f64,
) -> Result<f64> {
    use finstack_quant_models::rates::clock::{ModelDiscountCurve, model_time};

    let index_forward = finstack_quant_cashflows::builder::project_index_rate(
        date,
        fwd,
        accrual_start,
        accrual_end,
        accrual_year_fraction,
    )?;
    let model_curve = ModelDiscountCurve::new(disc, anchor)?;
    let ois_forward = model_curve.instantaneous_forward(model_time(anchor, date).max(0.0))?;
    Ok(index_forward - ois_forward)
}

/// Build reset schedule dates for floating rate facilities.
///
/// For floating rate facilities, generates a reset schedule from commitment to
/// maturity with the reset frequency, applying calendar adjustments if configured.
/// For fixed rate facilities, returns `None`.
///
/// # Arguments
///
/// * `facility` - The revolving credit facility
///
/// # Returns
///
/// - `Some(Vec<Date>)` for floating rate facilities with reset dates
/// - `None` for fixed rate facilities
///
/// # Errors
///
/// Returns an error if the schedule builder fails for floating rate facilities.
pub(crate) fn build_reset_dates(facility: &RevolvingCredit) -> Result<Option<Vec<Date>>> {
    match &facility.rate {
        RateSpec::Floating(spec) => {
            use finstack_quant_cashflows::builder::{BuildPeriodsParams, build_periods};

            let periods = build_periods(BuildPeriodsParams {
                start: facility.issue_date,
                end: facility.maturity,
                frequency: spec.reset_frequency,
                stub: facility.stub,
                business_day_convention: facility.business_day_convention,
                calendar_id: schedule_calendar_id(facility),
                end_of_month: false,
                day_count: facility.day_count,
                payment_lag_days: 0,
                reset_lag_days: None,
                adjust_accrual_dates: false,
                roll_rule: finstack_quant_cashflows::builder::RollRule::None,
            })?;
            Ok(Some(
                periods
                    .into_iter()
                    .map(|period| period.accrual_start)
                    .collect(),
            ))
        }
        RateSpec::Fixed { .. } => Ok(None),
    }
}

/// Calendar identifier the payment and reset schedules are built on: the
/// facility's `calendar_id`, or weekends only when unset.
pub(crate) fn schedule_calendar_id(facility: &RevolvingCredit) -> &str {
    facility
        .calendar_id
        .as_deref()
        .unwrap_or(finstack_quant_core::dates::WEEKENDS_ONLY_ID)
}

/// Convert a reset-effective date to its contractual fixing observation date.
///
/// # Arguments
///
/// * `spec` - Floating-rate specification; `reset_lag_days` and an optional
///   `fixing_calendar_id` drive the roll.
/// * `reset_effective_date` - Unadjusted date the fixing takes effect.
/// * `facility_calendar_id` - The facility's `calendar_id`, used when the
///   spec carries no fixing calendar; `None` rolls on weekends only.
pub(crate) fn floating_fixing_date(
    spec: &finstack_quant_cashflows::builder::FloatingRateSpec,
    reset_effective_date: Date,
    facility_calendar_id: Option<&str>,
) -> Result<Date> {
    if spec.reset_lag_days < 0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "RevolvingCredit reset_lag_days must be non-negative, got {}",
            spec.reset_lag_days
        )));
    }
    let calendar_id = spec
        .fixing_calendar_id
        .as_deref()
        .or(facility_calendar_id)
        .unwrap_or(finstack_quant_core::dates::WEEKENDS_ONLY_ID);
    let calendar = finstack_quant_core::dates::calendar_by_id_strict(calendar_id)?;
    reset_effective_date.add_business_days(-spec.reset_lag_days, calendar)
}

/// Inputs for one revolver floating coupon, term or overnight.
pub(crate) struct RevolverFloatingProjection<'a> {
    /// Accrual start of the (sub)period being projected.
    pub accrual_start: Date,
    /// Accrual end of the (sub)period being projected.
    pub accrual_end: Date,
    /// Valuation date separating realized fixings from projected observations.
    pub as_of: Date,
    /// Floating-rate specification on the facility.
    pub spec: &'a finstack_quant_cashflows::builder::FloatingRateSpec,
    /// Forward curve for the index.
    pub fwd: &'a ForwardCurve,
    /// Facility accrual day count (used when `overnight_basis` is unset).
    pub day_count: DayCount,
    /// Payment frequency used by context-sensitive overnight day counts.
    pub coupon_frequency: Tenor,
    /// Actual full facility coupon containing the projected accrual slice.
    pub coupon_period: (Date, Date),
    /// Facility currency, used to pick a default overnight calendar.
    pub currency: Currency,
    /// The facility's `calendar_id`, when set.
    pub calendar_id: Option<&'a str>,
    /// Cumulative margin step delta in force for the window, in basis points,
    /// added to the spec's spread.
    pub margin_delta_bp: f64,
    /// Optional historical overnight/term fixings.
    pub fixings: Option<&'a ScalarTimeSeries>,
}

/// Resolve overnight compounding from a floating index id and explicit override.
///
/// An explicit compounding setting wins: overnight variants return `Some`,
/// `Simple` returns `None`, and `SimpleAverage` returns a validation error.
/// When absent, looks up `index_id` via
/// [`overnight_conventions::compounding_from_index_id`].
pub(crate) fn resolved_overnight_compounding_for_index(
    index_id: &str,
    explicit: Option<&FloatingLegCompounding>,
) -> Result<Option<FloatingLegCompounding>> {
    if let Some(method) = explicit {
        return match method {
            FloatingLegCompounding::SimpleAverage => Err(finstack_quant_core::Error::Validation(
                "SimpleAverage overnight coupons are not supported by the shared \
                 compounded-in-arrears projector"
                    .to_string(),
            )),
            FloatingLegCompounding::Simple => Ok(None),
            other => Ok(Some(*other)),
        };
    }
    match overnight_conventions::compounding_from_index_id(index_id)? {
        Some(FloatingLegCompounding::Simple) | None => Ok(None),
        Some(compounding) => Ok(Some(compounding)),
    }
}

/// Resolve overnight compounding for a revolver floating spec.
///
/// Explicit `compounding` wins. Otherwise a registered overnight RFR
/// index (for example `USD-SOFR-OIS`) selects compounded-in-arrears. Term
/// indices such as `USD-SOFR-3M` stay on the simple term path.
pub(crate) fn resolved_overnight_compounding(
    spec: &finstack_quant_cashflows::builder::FloatingRateSpec,
) -> Result<Option<FloatingLegCompounding>> {
    resolved_overnight_compounding_for_index(
        spec.forward_curve_id.as_str(),
        spec.compounding.as_ref(),
    )
}

/// Project a revolver floating coupon, choosing term vs overnight from the spec.
///
/// # Arguments
///
/// * `input` - Accrual window, market curves, and the facility floating spec.
/// * `projected_fixings` - Optional schedule sink for raw overnight observations;
///   deterministic schedules retain these for subsequent time-roll scenarios.
///
/// # Errors
///
/// Returns a validation or market-data error when overnight calendars, fixings,
/// or curve lookups fail.
pub(crate) fn project_revolver_floating_rate(
    input: RevolverFloatingProjection<'_>,
    projected_fixings: Option<&mut Vec<finstack_quant_cashflows::fixings::ProjectedFixing>>,
) -> Result<f64> {
    let mut params = finstack_quant_cashflows::builder::FloatingRateParams::try_from(input.spec)?;
    params.spread_bp += input.margin_delta_bp;
    let Some(compounding) = resolved_overnight_compounding(input.spec)? else {
        let (coupon_start, coupon_end) = input.coupon_period;
        let accrual_context = if input.day_count == DayCount::Act365L {
            finstack_quant_core::dates::DayCountContext {
                frequency: Some(input.coupon_frequency),
                coupon_period: Some(input.coupon_period),
                ..Default::default()
            }
        } else {
            Default::default()
        };
        let coupon_accrual =
            input
                .day_count
                .year_fraction(coupon_start, coupon_end, accrual_context)?;
        return finstack_quant_cashflows::builder::project_floating_rate(
            input.accrual_start,
            input.fwd,
            &params,
            coupon_start,
            coupon_end,
            coupon_accrual,
        );
    };

    let calendar_id = input
        .spec
        .fixing_calendar_id
        .as_deref()
        .or(input.calendar_id);
    let calendar =
        finstack_quant_valuations_core::instruments::pricing::overnight::resolve_overnight_fixing_calendar(
            calendar_id,
            input.currency,
            "RevolvingCredit",
        )?;
    let day_count = input.spec.overnight_basis.unwrap_or(input.day_count);
    let (accrual_start, accrual_end) = adjust_overnight_accrual_boundaries(
        input.accrual_start,
        input.accrual_end,
        BusinessDayConvention::ModifiedFollowing,
        calendar,
    )?;
    if accrual_end <= accrual_start {
        return Ok(finstack_quant_cashflows::builder::calculate_floating_rate(
            0.0, &params,
        ));
    }

    // Index bounds applied per daily fixing (the default for floored SOFR
    // loans) act on each observation before compounding, as in the cashflows
    // builder's overnight replay; `Period` bounds act on the compounded rate.
    let daily_constraints = input.spec.overnight_index_constraints
        == finstack_quant_cashflows::builder::OvernightIndexConstraintApplication::Daily
        && (params.index_floor_bp.is_some() || params.index_cap_bp.is_some());
    let projection = project_overnight_coupon(OvernightCouponProjectionInput {
        curve: OvernightProjectionCurve::Forward(input.fwd),
        fixings: input.fixings,
        fixing_id: input.spec.index_id.as_str(),
        as_of: input.as_of,
        accrual_start,
        accrual_end,
        end_is_termination_date: None,
        day_count,
        coupon_frequency: Some(input.coupon_frequency),
        coupon_period: input.coupon_period,
        compounding: &compounding,
        fixing_calendar: calendar,
        compounded_spread: 0.0,
        need_observation_exposures: projected_fixings.is_some() || daily_constraints,
    })?;
    if let Some(out) = projected_fixings {
        out.extend(projection.projected_fixings(input.spec.index_id.as_str()));
    }
    let index_rate = if daily_constraints {
        let bound = |rate: f64| {
            finstack_quant_cashflows::builder::apply_bp_bounds(
                rate,
                params.index_floor_bp,
                params.index_cap_bp,
            )
        };
        let factor = projection
            .observation_exposures
            .iter()
            .fold(1.0, |product, observation| {
                product
                    * (1.0
                        + bound(observation.projected_rate)
                            * observation.factor_accrual_year_fraction)
            });
        params.index_floor_bp = None;
        params.index_cap_bp = None;
        (factor - 1.0) / projection.accrual_year_fraction
    } else {
        projection.rate
    };
    Ok(finstack_quant_cashflows::builder::calculate_floating_rate(
        index_rate, &params,
    ))
}

/// Apply a draw/repay event to current balance with commitment limit validation.
///
/// Updates the balance by adding (draw) or subtracting (repayment) the event amount.
/// For draws, validates that the new balance does not exceed the commitment amount.
///
/// # Arguments
///
/// * `current_balance` - Current drawn balance
/// * `event` - Draw or repayment event to apply
/// * `commitment` - Total facility commitment (for draw validation)
///
/// # Returns
///
/// Updated balance after applying the event.
///
/// # Errors
///
/// Returns a validation error if:
/// - A draw would exceed the commitment amount
/// - Checked arithmetic fails (e.g., repayment exceeds balance)
pub(crate) fn apply_draw_repay_event(
    current_balance: finstack_quant_core::money::Money,
    event: &super::super::types::DrawRepayEvent,
    commitment: finstack_quant_core::money::Money,
) -> Result<finstack_quant_core::money::Money> {
    if event.is_draw {
        let new_balance = current_balance.checked_add(event.amount)?;
        // Validate draw does not exceed commitment
        if new_balance.amount() > commitment.amount() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Draw on {} would exceed commitment: {} > {}",
                event.date, new_balance, commitment
            )));
        }
        Ok(new_balance)
    } else {
        let event_amount = event.amount.amount();
        if event_amount > current_balance.amount() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Repayment on {} of {} exceeds current balance of {}",
                event.date, event.amount, current_balance
            )));
        }
        current_balance.checked_sub(event.amount)
    }
}
