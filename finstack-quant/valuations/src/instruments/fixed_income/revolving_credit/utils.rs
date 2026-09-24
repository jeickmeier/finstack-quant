//! Internal utilities for revolving credit facilities.
//!
//! Consolidates schedule and calendar logic to avoid duplication across
//! cashflow generation and pricing implementations.

use super::types::{BaseRateSpec, RevolvingCredit};
use crate::instruments::common_impl::pricing::overnight::{
    adjust_overnight_accrual_boundaries, project_overnight_coupon, OvernightCouponProjectionInput,
    OvernightProjectionCurve,
};
use crate::instruments::common_impl::pricing::overnight_conventions;
use crate::instruments::rates::irs::FloatingLegCompounding;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DateExt, DayCount, Tenor};
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::Result;

/// Build the canonical accrual/payment periods for a revolving credit facility.
///
/// Accrual boundaries remain unadjusted while payment dates follow the
/// facility's `business_day_convention`, `calendar_id` and
/// `payment_lag_days`. Keeping the complete period objects prevents
/// payment-date adjustment from changing contractual accrual.
pub(super) fn build_payment_periods(
    facility: &RevolvingCredit,
) -> Result<Vec<crate::cashflow::builder::periods::SchedulePeriod>> {
    use crate::cashflow::builder::periods::{build_periods, BuildPeriodsParams};

    let periods = build_periods(BuildPeriodsParams {
        start: facility.commitment_date,
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
        roll_rule: crate::cashflow::builder::specs::RollRule::None,
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
pub(super) fn build_payment_dates(facility: &RevolvingCredit) -> Result<Vec<Date>> {
    let periods = build_payment_periods(facility)?;
    let payment_dates: Vec<Date> = std::iter::once(facility.commitment_date)
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
pub(super) fn build_accrual_boundary_dates(facility: &RevolvingCredit) -> Result<Vec<Date>> {
    let periods = build_payment_periods(facility)?;
    Ok(std::iter::once(facility.commitment_date)
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
pub(super) fn build_observation_dates(facility: &RevolvingCredit) -> Result<Vec<Date>> {
    let mut dates = build_accrual_boundary_dates(facility)?;
    if let Some(resets) = build_reset_dates(facility)? {
        dates.extend(
            resets
                .into_iter()
                .filter(|&reset| reset > facility.commitment_date && reset < facility.maturity),
        );
    }
    // Commitment, margin and fee steps slice accrual in both engines, so the
    // path is observed on them too.
    dates.extend(
        facility
            .step_dates()
            .into_iter()
            .filter(|&step| step > facility.commitment_date && step < facility.maturity),
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
pub(super) fn floating_params_at(
    facility: &RevolvingCredit,
    spec: &crate::cashflow::builder::FloatingRateSpec,
    date: Date,
) -> Result<crate::cashflow::builder::FloatingRateParams> {
    let mut params = crate::cashflow::builder::FloatingRateParams::try_from(spec)?;
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
pub(super) fn fixed_rate_at(facility: &RevolvingCredit, rate: f64, date: Date) -> f64 {
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
pub(super) fn index_basis_at(
    date: Date,
    anchor: Date,
    fwd: &ForwardCurve,
    disc: &dyn Discounting,
) -> Result<f64> {
    use finstack_quant_models::rates::clock::{model_time, ModelDiscountCurve};

    let index_forward = crate::cashflow::builder::rate_helpers::project_index_rate(date, fwd)?;
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
pub(super) fn build_reset_dates(facility: &RevolvingCredit) -> Result<Option<Vec<Date>>> {
    match &facility.base_rate_spec {
        BaseRateSpec::Floating(spec) => {
            use crate::cashflow::builder::periods::{build_periods, BuildPeriodsParams};

            let periods = build_periods(BuildPeriodsParams {
                start: facility.commitment_date,
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
                roll_rule: crate::cashflow::builder::specs::RollRule::None,
            })?;
            Ok(Some(
                periods
                    .into_iter()
                    .map(|period| period.accrual_start)
                    .collect(),
            ))
        }
        BaseRateSpec::Fixed { .. } => Ok(None),
    }
}

/// Calendar identifier the payment and reset schedules are built on: the
/// facility's `calendar_id`, or weekends only when unset.
pub(super) fn schedule_calendar_id(facility: &RevolvingCredit) -> &str {
    facility
        .calendar_id
        .as_deref()
        .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID)
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
pub(super) fn floating_fixing_date(
    spec: &crate::cashflow::builder::FloatingRateSpec,
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
        .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID);
    let calendar = crate::cashflow::builder::calendar::resolve_calendar_strict(calendar_id)?;
    reset_effective_date.add_business_days(-spec.reset_lag_days, calendar)
}

/// Inputs for one revolver floating coupon, term or overnight.
pub(super) struct RevolverFloatingProjection<'a> {
    /// Accrual start of the (sub)period being projected.
    pub accrual_start: Date,
    /// Accrual end of the (sub)period being projected.
    pub accrual_end: Date,
    /// Valuation date separating realized fixings from projected observations.
    pub as_of: Date,
    /// Floating-rate specification on the facility.
    pub spec: &'a crate::cashflow::builder::FloatingRateSpec,
    /// Forward curve for the index.
    pub fwd: &'a ForwardCurve,
    /// Facility accrual day count (used when `overnight_basis` is unset).
    pub day_count: DayCount,
    /// Payment frequency used by context-sensitive overnight day counts.
    pub coupon_frequency: Tenor,
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

/// Resolve overnight compounding for a revolver floating spec.
///
/// Explicit `overnight_compounding` wins. Otherwise a registered overnight RFR
/// index (for example `USD-SOFR-OIS`) selects compounded-in-arrears. Term
/// indices such as `USD-SOFR-3M` stay on the simple term path.
pub(super) fn resolved_overnight_compounding(
    spec: &crate::cashflow::builder::FloatingRateSpec,
) -> Result<Option<FloatingLegCompounding>> {
    overnight_conventions::resolved_overnight_compounding(
        spec.index_id.as_str(),
        spec.overnight_compounding.as_ref(),
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
pub(super) fn project_revolver_floating_rate(
    input: RevolverFloatingProjection<'_>,
    projected_fixings: Option<&mut Vec<crate::cashflow::fixings::ProjectedFixing>>,
) -> Result<f64> {
    let mut params = crate::cashflow::builder::FloatingRateParams::try_from(input.spec)?;
    params.spread_bp += input.margin_delta_bp;
    let Some(compounding) = resolved_overnight_compounding(input.spec)? else {
        return crate::cashflow::builder::project_floating_rate(
            input.accrual_start,
            input.fwd,
            &params,
        );
    };

    let calendar_id = input
        .spec
        .fixing_calendar_id
        .as_deref()
        .or(input.calendar_id);
    let calendar =
        crate::instruments::common_impl::pricing::overnight::resolve_overnight_fixing_calendar(
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
        return Ok(crate::cashflow::builder::rate_helpers::calculate_floating_rate(0.0, &params));
    }

    // Index bounds applied per daily fixing (the default for floored SOFR
    // loans) act on each observation before compounding, as in the cashflows
    // builder's overnight replay; `Period` bounds act on the compounded rate.
    let daily_constraints = input.spec.overnight_index_constraints
        == crate::cashflow::builder::OvernightIndexConstraintApplication::Daily
        && (params.index_floor_bp.is_some() || params.index_cap_bp.is_some());
    let projection = project_overnight_coupon(OvernightCouponProjectionInput {
        curve: OvernightProjectionCurve::Forward(input.fwd),
        fixings: input.fixings,
        fixing_id: input.spec.index_id.as_str(),
        as_of: input.as_of,
        accrual_start,
        accrual_end,
        day_count,
        coupon_frequency: Some(input.coupon_frequency),
        compounding: &compounding,
        fixing_calendar: calendar,
        compounded_spread: 0.0,
        need_observation_exposures: projected_fixings.is_some() || daily_constraints,
    })?;
    if let Some(out) = projected_fixings {
        out.extend(projection.observation_exposures.iter().map(|observation| {
            crate::cashflow::fixings::ProjectedFixing {
                series_id: format!("FIXING:{}", input.spec.index_id),
                date: observation.observation_start,
                value: Some(observation.projected_rate),
            }
        }));
    }
    let index_rate = if daily_constraints {
        let bound = |rate: f64| {
            let floored = params
                .index_floor_bp
                .map_or(rate, |floor| rate.max(floor * 1e-4));
            params
                .index_cap_bp
                .map_or(floored, |cap| floored.min(cap * 1e-4))
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
    Ok(crate::cashflow::builder::rate_helpers::calculate_floating_rate(index_rate, &params))
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
/// * `commitment_amount` - Total facility commitment (for draw validation)
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
pub(super) fn apply_draw_repay_event(
    current_balance: finstack_quant_core::money::Money,
    event: &super::types::DrawRepayEvent,
    commitment_amount: finstack_quant_core::money::Money,
) -> Result<finstack_quant_core::money::Money> {
    if event.is_draw {
        let new_balance = current_balance.checked_add(event.amount)?;
        // Validate draw does not exceed commitment
        if new_balance.amount() > commitment_amount.amount() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Draw on {} would exceed commitment: {} > {}",
                event.date, new_balance, commitment_amount
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

/// Linearly interpolate a rate from sorted knot points with flat extrapolation.
///
/// Uses `partition_point` for O(log n) interval lookup. Degenerate inputs are
/// handled defensively: empty inputs yield `0.0`, mismatched lengths use the
/// common prefix, and a zero-width interval (duplicate knots) returns the
/// left-hand rate instead of dividing by zero.
///
/// # Arguments
///
/// * `t` - Query time in years (same clock as `times`).
/// * `times` - Sorted (non-decreasing) knot times in years.
/// * `rates` - Rates at each knot (decimal, e.g. `0.05` for 5%).
pub(super) fn interpolate_rate(t: f64, times: &[f64], rates: &[f64]) -> f64 {
    if times.is_empty() || rates.is_empty() {
        return 0.0;
    }
    if times.len() == 1 || rates.len() == 1 || t <= times[0] {
        return rates[0];
    }
    let n = times.len().min(rates.len());
    if t >= times[n - 1] {
        return rates[n - 1];
    }
    let idx = times[..n].partition_point(|&time| time <= t);
    let i = idx.saturating_sub(1);
    let width = times[i + 1] - times[i];
    if width <= 0.0 {
        return rates[i];
    }
    let alpha = (t - times[i]) / width;
    rates[i] + alpha * (rates[i + 1] - rates[i])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Attributes;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::money::Money;
    use time::Month;

    fn create_test_facility(
        start: Date,
        end: Date,
        payment_frequency: Tenor,
        base_rate_spec: BaseRateSpec,
        calendar_id: Option<&str>,
    ) -> RevolvingCredit {
        use finstack_quant_core::dates::StubKind;

        RevolvingCredit {
            id: "TEST-RC".into(),
            commitment_amount: Money::from((10_000_000_i64, Currency::USD)),
            drawn_amount: Money::from((5_000_000_i64, Currency::USD)),
            commitment_date: start,
            maturity: end,
            base_rate_spec,
            day_count: DayCount::Act360,
            frequency: payment_frequency,
            commitment_schedule: Vec::new(),
            margin_steps: Vec::new(),
            lc: None,
            scheduled_fees: Vec::new(),
            oid_eir: None,
            fees: super::super::types::RevolvingCreditFees::default(),
            draw_repay_spec: super::super::types::DrawRepaySpec::Deterministic(vec![]),
            discount_curve_id: "USD-OIS".into(),
            credit_curve_id: None,
            recovery_rate: 0.0,
            leq: 0.0,
            stub: StubKind::ShortFront,
            business_day_convention:
                finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing,
            calendar_id: calendar_id.map(str::to_string),
            payment_lag_days: 0,
            settlement_days: 0,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        }
    }

    #[test]
    fn test_build_payment_dates_ends_on_or_before_maturity() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");
        let facility = create_test_facility(
            start,
            end,
            Tenor::quarterly(),
            BaseRateSpec::Fixed { rate: 0.05 },
            None,
        );

        let dates =
            build_payment_dates(&facility).expect("Payment dates building should succeed in test");
        assert!(dates.len() >= 2);
        // Verify no sentinel: last date should be at or before maturity
        assert!(*dates.last().expect("Dates should not be empty") <= end);
    }

    #[test]
    fn payment_adjustment_does_not_move_accrual_boundaries() {
        let start = Date::from_calendar_date(2026, Month::January, 3).expect("date");
        let maturity = Date::from_calendar_date(2027, Month::January, 3).expect("date");
        let facility = create_test_facility(
            start,
            maturity,
            Tenor::annual(),
            BaseRateSpec::Fixed { rate: 0.05 },
            None,
        );

        let periods = build_payment_periods(&facility).expect("periods");
        let final_period = periods.last().expect("period");
        assert_eq!(final_period.accrual_end, maturity);
        assert_eq!(
            final_period.payment_date,
            Date::from_calendar_date(2027, Month::January, 4).expect("date")
        );

        let observation_dates = build_accrual_boundary_dates(&facility).expect("boundaries");
        assert_eq!(observation_dates.last(), Some(&maturity));
    }

    #[test]
    fn test_build_reset_dates_fixed_returns_none() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");
        let facility = create_test_facility(
            start,
            end,
            Tenor::quarterly(),
            BaseRateSpec::Fixed { rate: 0.05 },
            None,
        );

        let reset_dates =
            build_reset_dates(&facility).expect("Reset dates building should succeed in test");
        assert!(reset_dates.is_none());
    }

    #[test]
    fn test_build_reset_dates_floating_returns_some() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");
        let facility = create_test_facility(
            start,
            end,
            Tenor::quarterly(),
            BaseRateSpec::Floating(crate::cashflow::builder::FloatingRateSpec {
                index_id: "USD-SOFR-3M".into(),
                spread_bp: rust_decimal::Decimal::try_from(200.0).expect("valid"),
                gearing: rust_decimal::Decimal::ONE,
                gearing_includes_spread: true,
                index_floor_bp: None,
                all_in_cap_bp: None,
                all_in_floor_bp: None,
                index_cap_bp: None,
                overnight_index_constraints: Default::default(),
                reset_frequency: Tenor::quarterly(),
                index_tenor: None,
                reset_lag_days: 2,
                fixing_calendar_id: None,
                overnight_compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
            }),
            None,
        );

        let reset_dates =
            build_reset_dates(&facility).expect("Reset dates building should succeed in test");
        assert!(reset_dates.is_some());
        let dates = reset_dates.expect("Reset dates should exist for floating rate");
        assert!(dates.len() >= 2);
    }

    #[test]
    fn floating_projection_applies_full_coupon_economics() {
        use finstack_quant_core::market_data::term_structures::ForwardCurve;
        use rust_decimal::Decimal;

        let reset = Date::from_calendar_date(2025, Month::January, 2).expect("date");
        let spec = crate::cashflow::builder::FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            spread_bp: Decimal::from(100),
            gearing: Decimal::new(5, 1),
            gearing_includes_spread: false,
            index_floor_bp: None,
            index_cap_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: Some(Decimal::from(200)),
            overnight_index_constraints: Default::default(),
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        };
        let forward = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(reset)
            .knots(vec![(0.0, 0.03), (1.0, 0.03)])
            .build()
            .expect("forward curve");

        let rate = crate::cashflow::builder::project_floating_rate(
            reset,
            &forward,
            &crate::cashflow::builder::FloatingRateParams::try_from(&spec).expect("rate params"),
        )
        .expect("projected coupon");
        assert!((rate - 0.02).abs() < 1e-12, "all-in cap must bind: {rate}");
    }

    #[test]
    fn overnight_index_uses_shared_compounding_engine() {
        use crate::instruments::common_impl::pricing::overnight::{
            project_overnight_coupon, OvernightCouponProjectionInput, OvernightProjectionCurve,
        };
        use finstack_quant_core::dates::calendar_by_id;
        use finstack_quant_core::market_data::term_structures::ForwardCurve;
        use rust_decimal::Decimal;

        let start = Date::from_calendar_date(2025, Month::January, 2).expect("date");
        let end = Date::from_calendar_date(2025, Month::April, 2).expect("date");
        let spec = crate::cashflow::builder::FloatingRateSpec {
            index_id: "USD-SOFR-OIS".into(),
            spread_bp: Decimal::ZERO,
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            index_cap_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: Some("usny".into()),
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        };
        let forward = ForwardCurve::builder("USD-SOFR-OIS", 0.25)
            .base_date(start)
            .knots(vec![(0.0, 0.04), (1.0, 0.04)])
            .build()
            .expect("forward curve");
        let revolver_rate = project_revolver_floating_rate(
            RevolverFloatingProjection {
                accrual_start: start,
                accrual_end: end,
                as_of: start,
                spec: &spec,
                fwd: &forward,
                day_count: DayCount::Act360,
                coupon_frequency: Tenor::quarterly(),
                currency: Currency::USD,
                calendar_id: None,
                margin_delta_bp: 0.0,
                fixings: None,
            },
            None,
        )
        .expect("overnight revolver coupon");

        let compounding = resolved_overnight_compounding(&spec)
            .expect("resolve")
            .expect("overnight");
        let calendar = calendar_by_id("usny").expect("usny");
        let expected = project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Forward(&forward),
            fixings: None,
            fixing_id: "USD-SOFR-OIS",
            as_of: start,
            accrual_start: start,
            accrual_end: end,
            day_count: DayCount::Act360,
            coupon_frequency: Some(Tenor::quarterly()),
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
            need_observation_exposures: false,
        })
        .expect("shared overnight coupon");
        assert!(
            (revolver_rate - expected.rate).abs() < 1e-12,
            "revolver overnight rate {revolver_rate} != shared engine {}",
            expected.rate
        );
    }

    /// A 0% SOFR index floor applied daily floors every negative fixing
    /// before compounding; applied per period it floors only the compounded
    /// rate. Fixings are -1% before 2025-02-14 and +2% from it, all realized.
    /// The expected rates are compounded by hand over the usny business days.
    #[test]
    fn overnight_index_floor_applies_daily_or_per_period() {
        use crate::cashflow::builder::{
            OvernightCompoundingMethod, OvernightIndexConstraintApplication,
        };
        use finstack_quant_core::dates::{calendar_by_id, DateExt};
        use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
        use finstack_quant_core::market_data::term_structures::ForwardCurve;
        use rust_decimal::Decimal;

        let start = Date::from_calendar_date(2025, Month::January, 2).expect("date");
        let end = Date::from_calendar_date(2025, Month::April, 2).expect("date");
        let switch = Date::from_calendar_date(2025, Month::February, 14).expect("date");
        let fixing = |date: Date| if date < switch { -0.01 } else { 0.02 };
        let calendar = calendar_by_id("usny").expect("usny");
        let mut observations = Vec::new();
        let mut date = start;
        while date < end {
            observations.push((date, fixing(date)));
            date = date
                .add_business_days(1, calendar)
                .expect("next business day");
        }
        let series = ScalarTimeSeries::new("FIXING:USD-SOFR-OIS", observations.clone(), None)
            .expect("fixings");

        // Hand calculation: ∏(1 + r_i · d_i / 360) over each business day,
        // d_i the calendar days to the next business day.
        let compound = |floor: bool| -> f64 {
            let mut product = 1.0;
            for (i, (date, rate)) in observations.iter().enumerate() {
                let next = observations.get(i + 1).map_or(end, |(next, _)| *next);
                let days = f64::from(i32::try_from((next - *date).whole_days()).expect("days"));
                let rate = if floor { rate.max(0.0) } else { *rate };
                product *= 1.0 + rate * days / 360.0;
            }
            let period_days = f64::from(i32::try_from((end - start).whole_days()).expect("days"));
            (product - 1.0) * 360.0 / period_days
        };
        let daily_expected = compound(true);
        let period_expected = compound(false).max(0.0);
        assert!(
            daily_expected > period_expected + 1e-4,
            "fixings straddle zero"
        );

        let forward = ForwardCurve::builder("USD-SOFR-OIS", 1.0 / 360.0)
            .base_date(start)
            .knots(vec![(0.0, 0.03), (1.0, 0.03)])
            .build()
            .expect("forward curve");
        let rate = |application: OvernightIndexConstraintApplication| -> f64 {
            let spec = crate::cashflow::builder::FloatingRateSpec {
                index_id: "USD-SOFR-OIS".into(),
                spread_bp: Decimal::ZERO,
                gearing: Decimal::ONE,
                gearing_includes_spread: true,
                index_floor_bp: Some(Decimal::ZERO),
                index_cap_bp: None,
                all_in_floor_bp: None,
                all_in_cap_bp: None,
                overnight_index_constraints: application,
                reset_frequency: Tenor::quarterly(),
                index_tenor: None,
                reset_lag_days: 0,
                fixing_calendar_id: Some("usny".into()),
                overnight_compounding: Some(OvernightCompoundingMethod::CompoundedInArrears),
                overnight_basis: Some(DayCount::Act360),
                fallback: Default::default(),
            };
            project_revolver_floating_rate(
                RevolverFloatingProjection {
                    accrual_start: start,
                    accrual_end: end,
                    as_of: end,
                    spec: &spec,
                    fwd: &forward,
                    day_count: DayCount::Act360,
                    coupon_frequency: Tenor::quarterly(),
                    currency: Currency::USD,
                    calendar_id: None,
                    margin_delta_bp: 0.0,
                    fixings: Some(&series),
                },
                None,
            )
            .expect("overnight coupon")
        };
        let daily = rate(OvernightIndexConstraintApplication::Daily);
        let period = rate(OvernightIndexConstraintApplication::Period);
        assert!(
            (daily - daily_expected).abs() < 1e-12,
            "daily floor {daily} vs hand {daily_expected}"
        );
        assert!(
            (period - period_expected).abs() < 1e-12,
            "period floor {period} vs hand {period_expected}"
        );
    }

    #[test]
    fn test_calendar_adjustment_applied() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");

        // Without calendar
        let facility_no_cal = create_test_facility(
            start,
            end,
            Tenor::quarterly(),
            BaseRateSpec::Fixed { rate: 0.05 },
            None,
        );
        let dates_no_cal = build_payment_dates(&facility_no_cal)
            .expect("Payment dates building should succeed in test");

        // With NYSE calendar
        let facility_with_cal = create_test_facility(
            start,
            end,
            Tenor::quarterly(),
            BaseRateSpec::Fixed { rate: 0.05 },
            Some("nyse"),
        );
        let dates_with_cal = build_payment_dates(&facility_with_cal)
            .expect("Payment dates building should succeed in test");

        // Both should have same length (quarterly over 1 year)
        assert_eq!(dates_no_cal.len(), dates_with_cal.len());
    }

    #[test]
    fn test_apply_draw_repay_event_draw() {
        use super::super::types::DrawRepayEvent;

        let balance = Money::from((5_000_000_i64, Currency::USD));
        let commitment = Money::from((10_000_000_i64, Currency::USD));
        let draw_date = Date::from_calendar_date(2025, Month::March, 1).expect("Valid test date");

        let event = DrawRepayEvent {
            date: draw_date,
            amount: Money::from((2_000_000_i64, Currency::USD)),
            is_draw: true,
        };

        let new_balance = apply_draw_repay_event(balance, &event, commitment)
            .expect("Draw/repay event application should succeed");
        assert_eq!(new_balance.amount(), 7_000_000.0);
    }

    #[test]
    fn test_apply_draw_repay_event_repay() {
        use super::super::types::DrawRepayEvent;

        let balance = Money::from((5_000_000_i64, Currency::USD));
        let commitment = Money::from((10_000_000_i64, Currency::USD));
        let repay_date = Date::from_calendar_date(2025, Month::March, 1).expect("Valid test date");

        let event = DrawRepayEvent {
            date: repay_date,
            amount: Money::from((1_000_000_i64, Currency::USD)),
            is_draw: false,
        };

        let new_balance = apply_draw_repay_event(balance, &event, commitment)
            .expect("Draw/repay event application should succeed");
        assert_eq!(new_balance.amount(), 4_000_000.0);
    }

    #[test]
    fn test_apply_draw_repay_event_exceeds_commitment() {
        use super::super::types::DrawRepayEvent;

        let balance = Money::from((8_000_000_i64, Currency::USD));
        let commitment = Money::from((10_000_000_i64, Currency::USD));
        let draw_date = Date::from_calendar_date(2025, Month::March, 1).expect("Valid test date");

        let event = DrawRepayEvent {
            date: draw_date,
            amount: Money::from((3_000_000_i64, Currency::USD)), // Would exceed commitment
            is_draw: true,
        };

        let result = apply_draw_repay_event(balance, &event, commitment);
        assert!(result.is_err());
        assert!(result
            .expect_err("should fail")
            .to_string()
            .contains("exceed commitment"));
    }

    #[test]
    fn interpolate_rate_handles_duplicate_knots_without_nan() {
        let times = [0.0, 1.0, 1.0, 2.0];
        let rates = [0.01, 0.02, 0.03, 0.04];
        let v = interpolate_rate(1.0, &times, &rates);
        assert!(v.is_finite());
        assert!((v - 0.03).abs() < 1e-15);
        assert!((interpolate_rate(0.5, &times, &rates) - 0.015).abs() < 1e-15);
        assert!((interpolate_rate(-1.0, &times, &rates) - 0.01).abs() < 1e-15);
        assert!((interpolate_rate(5.0, &times, &rates) - 0.04).abs() < 1e-15);
        assert_eq!(interpolate_rate(1.0, &[], &[]), 0.0);
    }
}
