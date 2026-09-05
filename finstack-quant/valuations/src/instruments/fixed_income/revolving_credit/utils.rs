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
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::rates::irs::FloatingLegCompounding;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DateExt, DayCount, Tenor};
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::Result;

/// Build the canonical accrual/payment periods for a revolving credit facility.
///
/// Accrual boundaries remain unadjusted while payment dates follow Modified
/// Following on the configured facility calendar. Keeping the complete period
/// objects prevents payment-date adjustment from changing contractual accrual.
pub(super) fn build_payment_periods(
    facility: &RevolvingCredit,
) -> Result<Vec<crate::cashflow::builder::periods::SchedulePeriod>> {
    use crate::cashflow::builder::periods::{build_periods, BuildPeriodsParams};

    let calendar_id = facility
        .attributes
        .get_meta("calendar_id")
        .or_else(|| facility.attributes.get_meta("calendar"))
        .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID);
    let periods = build_periods(BuildPeriodsParams {
        start: facility.commitment_date,
        end: facility.maturity,
        frequency: facility.frequency,
        stub: facility.stub,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id,
        end_of_month: false,
        day_count: facility.day_count,
        payment_lag_days: 0,
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
/// * `include_sentinel` - If true, appends a date one day after the last payment
///   to ensure terminal cashflows are included in period aggregation (exclusive end semantics)
///
/// # Returns
///
/// Vector of payment dates, optionally with a sentinel date appended.
///
/// # Errors
///
/// Returns an error if the schedule builder fails or produces fewer than 2 dates.
pub(super) fn build_payment_dates(
    facility: &RevolvingCredit,
    include_sentinel: bool,
) -> Result<Vec<Date>> {
    let periods = build_payment_periods(facility)?;
    let mut payment_dates: Vec<Date> = std::iter::once(facility.commitment_date)
        .chain(periods.into_iter().map(|period| period.payment_date))
        .collect();

    // Add sentinel if requested (for period PV aggregation with exclusive end semantics)
    if include_sentinel {
        if let Some(&last) = payment_dates.last() {
            payment_dates.push(last + time::Duration::days(1));
        }
    }

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

            let calendar_id = facility
                .attributes
                .get_meta("calendar_id")
                .or_else(|| facility.attributes.get_meta("calendar"))
                .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID);
            let periods = build_periods(BuildPeriodsParams {
                start: facility.commitment_date,
                end: facility.maturity,
                frequency: spec.reset_frequency,
                stub: facility.stub,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                calendar_id,
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

/// Convert a reset-effective date to its contractual fixing observation date.
pub(super) fn floating_fixing_date(
    spec: &crate::cashflow::builder::FloatingRateSpec,
    reset_effective_date: Date,
    attrs: &Attributes,
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
        .or_else(|| attrs.get_meta("calendar_id"))
        .or_else(|| attrs.get_meta("calendar"))
        .unwrap_or("weekends_only");
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
    /// Facility attributes (calendar metadata).
    pub attributes: &'a Attributes,
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

/// Project floating rate for revolving credit facility using resolved curve.
///
/// Term indices project a single forward from the reset-effective date.
/// Overnight RFR indices (or an explicit overnight method) compound daily
/// fixings over `[accrual_start, accrual_end]` via the shared overnight engine.
/// Gearing, spread, and floors/caps are applied afterwards in both cases.
pub(super) fn project_floating_rate_with_curve(
    reset_date: Date,
    spec: &crate::cashflow::builder::FloatingRateSpec,
    fwd: &ForwardCurve,
) -> Result<f64> {
    let params = crate::cashflow::builder::FloatingRateParams::try_from(spec)?;
    crate::cashflow::builder::project_floating_rate(reset_date, fwd, &params)
}

/// Project a revolver floating coupon, choosing term vs overnight from the spec.
///
/// # Arguments
///
/// * `input` - Accrual window, market curves, and the facility floating spec.
///
/// # Errors
///
/// Returns a validation or market-data error when overnight calendars, fixings,
/// or curve lookups fail.
pub(super) fn project_revolver_floating_rate(input: RevolverFloatingProjection<'_>) -> Result<f64> {
    let params = crate::cashflow::builder::FloatingRateParams::try_from(input.spec)?;
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
        .or_else(|| input.attributes.get_meta("calendar_id"))
        .or_else(|| input.attributes.get_meta("calendar"));
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
        need_observation_exposures: false,
    })?;
    Ok(crate::cashflow::builder::rate_helpers::calculate_floating_rate(projection.rate, &params))
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

        let mut attrs = Attributes::new();
        if let Some(cal_id) = calendar_id {
            attrs = attrs.with_meta("calendar_id", cal_id);
        }

        RevolvingCredit {
            id: "TEST-RC".into(),
            commitment_amount: Money::from((10_000_000_i64, Currency::USD)),
            drawn_amount: Money::from((5_000_000_i64, Currency::USD)),
            commitment_date: start,
            maturity: end,
            base_rate_spec,
            day_count: DayCount::Act360,
            frequency: payment_frequency,
            fees: super::super::types::RevolvingCreditFees::default(),
            draw_repay_spec: super::super::types::DrawRepaySpec::Deterministic(vec![]),
            discount_curve_id: "USD-OIS".into(),
            credit_curve_id: None,
            recovery_rate: 0.0,
            stub: StubKind::ShortFront,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: attrs,
        }
    }

    #[test]
    fn test_build_payment_dates_no_sentinel() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");
        let facility = create_test_facility(
            start,
            end,
            Tenor::quarterly(),
            BaseRateSpec::Fixed { rate: 0.05 },
            None,
        );

        let dates = build_payment_dates(&facility, false)
            .expect("Payment dates building should succeed in test");
        assert!(dates.len() >= 2);
        // Verify no sentinel: last date should be at or before maturity
        assert!(*dates.last().expect("Dates should not be empty") <= end);
    }

    #[test]
    fn test_build_payment_dates_with_sentinel() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        let end = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");
        let facility = create_test_facility(
            start,
            end,
            Tenor::quarterly(),
            BaseRateSpec::Fixed { rate: 0.05 },
            None,
        );

        let dates_no_sentinel = build_payment_dates(&facility, false)
            .expect("Payment dates building should succeed in test");
        let dates_with_sentinel = build_payment_dates(&facility, true)
            .expect("Payment dates building should succeed in test");

        // With sentinel should have one more date
        assert_eq!(dates_with_sentinel.len(), dates_no_sentinel.len() + 1);

        // Sentinel should be one day after the last payment date
        let last_payment = dates_no_sentinel.last().expect("Dates should not be empty");
        let sentinel = dates_with_sentinel
            .last()
            .expect("Dates should not be empty");
        assert_eq!(*sentinel, *last_payment + time::Duration::days(1));
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

        let rate =
            project_floating_rate_with_curve(reset, &spec, &forward).expect("projected coupon");
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
        let attrs = Attributes::new();
        let revolver_rate = project_revolver_floating_rate(RevolverFloatingProjection {
            accrual_start: start,
            accrual_end: end,
            as_of: start,
            spec: &spec,
            fwd: &forward,
            day_count: DayCount::Act360,
            coupon_frequency: Tenor::quarterly(),
            currency: Currency::USD,
            attributes: &attrs,
            fixings: None,
        })
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
        let dates_no_cal = build_payment_dates(&facility_no_cal, false)
            .expect("Payment dates building should succeed in test");

        // With NYSE calendar
        let facility_with_cal = create_test_facility(
            start,
            end,
            Tenor::quarterly(),
            BaseRateSpec::Fixed { rate: 0.05 },
            Some("nyse"),
        );
        let dates_with_cal = build_payment_dates(&facility_with_cal, false)
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
