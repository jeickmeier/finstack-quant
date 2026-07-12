//! Internal utilities for revolving credit facilities.
//!
//! Consolidates schedule and calendar logic to avoid duplication across
//! cashflow generation and pricing implementations.

use super::types::{BaseRateSpec, RevolvingCredit};
use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DateExt, HolidayCalendar, ScheduleBuilder,
};
use finstack_quant_core::Result;

/// Resolve the calendar for a facility from its attributes.
///
/// Looks for `calendar_id` or `calendar` metadata and returns the resolved
/// calendar if found in the global registry.
///
/// # Arguments
///
/// * `attrs` - Facility attributes containing calendar metadata
///
/// # Returns
///
/// Reference to a `HolidayCalendar` if calendar metadata is present and resolvable, `None` otherwise.
pub(super) fn resolve_facility_calendar(
    attrs: &Attributes,
) -> Option<&'static dyn HolidayCalendar> {
    let cal_code = attrs
        .get_meta("calendar_id")
        .or_else(|| attrs.get_meta("calendar"))?;
    finstack_quant_core::dates::CalendarRegistry::global().resolve_str(cal_code)
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
    let mut builder = ScheduleBuilder::new(facility.commitment_date, facility.maturity)?
        .frequency(facility.frequency)
        .stub_rule(facility.stub);

    if let Some(cal) = resolve_facility_calendar(&facility.attributes) {
        builder = builder.adjust_with(BusinessDayConvention::ModifiedFollowing, cal);
    }

    let payment_schedule = builder.build()?;
    let mut payment_dates: Vec<Date> = payment_schedule.into_iter().collect();

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
            let mut reset_builder =
                ScheduleBuilder::new(facility.commitment_date, facility.maturity)?
                    .frequency(spec.reset_freq)
                    .stub_rule(facility.stub);

            let cal =
                crate::cashflow::builder::calendar::resolve_calendar_strict(&spec.calendar_id)?;
            reset_builder = reset_builder.adjust_with(spec.bdc, cal);

            let reset_schedule = reset_builder.build()?;
            Ok(Some(reset_schedule.into_iter().collect()))
        }
        BaseRateSpec::Fixed { .. } => Ok(None),
    }
}

/// Convert a reset-effective date to its contractual fixing observation date.
pub(super) fn floating_fixing_date(
    spec: &crate::cashflow::builder::FloatingRateSpec,
    reset_effective_date: Date,
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
        .unwrap_or(spec.calendar_id.as_str());
    let calendar = crate::cashflow::builder::calendar::resolve_calendar_strict(calendar_id)?;
    reset_effective_date.add_business_days(-spec.reset_lag_days, calendar)
}

/// Apply the floating coupon payment lag without shifting accrual boundaries.
pub(super) fn floating_payment_date(
    spec: &crate::cashflow::builder::FloatingRateSpec,
    accrual_end: Date,
) -> Result<Date> {
    if spec.payment_lag_days < 0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "RevolvingCredit payment_lag_days must be non-negative, got {}",
            spec.payment_lag_days
        )));
    }
    let calendar =
        crate::cashflow::builder::calendar::resolve_calendar_strict(spec.calendar_id.as_str())?;
    accrual_end.add_business_days(spec.payment_lag_days, calendar)
}

/// Project floating rate for revolving credit facility using resolved curve.
///
/// Optimized version that takes a resolved curve to avoid repeated lookups.
///
/// Projection uses the reset-effective date. The separate contractual fixing
/// observation date is derived with [`floating_fixing_date`].
pub(super) fn project_floating_rate_with_curve(
    reset_date: finstack_quant_core::dates::Date,
    spec: &crate::cashflow::builder::FloatingRateSpec,
    fwd: &finstack_quant_core::market_data::term_structures::ForwardCurve,
    _attrs: &Attributes,
) -> Result<f64> {
    let reset_end = compute_reset_period_end(reset_date, spec)?;
    let params = crate::cashflow::builder::FloatingRateParams::try_from(spec)?;

    // Delegate to centralized projection
    crate::cashflow::builder::project_floating_rate(reset_date, reset_end, fwd, &params)
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

/// Compute the end date of a reset period given start date and frequency.
///
/// Applies frequency offset (months or days) and calendar business day adjustment
/// if configured in facility attributes.
///
/// # Arguments
///
/// * `reset_date` - Start date of the reset period
/// * `reset_freq` - Tenor determining the period length
/// * `attrs` - Facility attributes (for calendar resolution)
///
/// # Returns
///
/// End date of the reset period, adjusted for business days if calendar is configured.
///
/// # Errors
///
/// Returns an error if calendar adjustment fails.
pub(super) fn compute_reset_period_end(
    reset_date: Date,
    spec: &crate::cashflow::builder::FloatingRateSpec,
) -> Result<Date> {
    let reset_freq = spec.index_tenor.as_ref().unwrap_or(&spec.reset_freq);
    // Compute unadjusted end date based on frequency
    use finstack_quant_core::dates::TenorUnit;
    let mut reset_end = match reset_freq.unit() {
        TenorUnit::Months => reset_date.add_months(reset_freq.count() as i32),
        TenorUnit::Years => reset_date.add_months(reset_freq.count() as i32 * 12),
        TenorUnit::Weeks => reset_date + time::Duration::weeks(reset_freq.count() as i64),
        TenorUnit::Days => reset_date + time::Duration::days(reset_freq.count() as i64),
    };

    if spec.end_of_month && reset_date == reset_date.end_of_month() {
        reset_end = reset_end.end_of_month();
    }
    let calendar = crate::cashflow::builder::calendar::resolve_calendar_strict(&spec.calendar_id)?;
    reset_end = finstack_quant_core::dates::adjust(reset_end, spec.bdc, calendar)?;

    Ok(reset_end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Attributes;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::money::Money;
    use rust_decimal::Decimal;
    use time::Month;

    fn reset_spec(tenor: Tenor, calendar_id: &str) -> crate::cashflow::builder::FloatingRateSpec {
        crate::cashflow::builder::FloatingRateSpec {
            index_id: "TEST-INDEX".into(),
            spread_bp: Decimal::ZERO,
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            index_cap_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_freq: tenor,
            index_tenor: None,
            reset_lag_days: 0,
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: calendar_id.to_string(),
            fixing_calendar_id: None,
            end_of_month: false,
            payment_lag_days: 0,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        }
    }

    fn create_test_facility(
        start: Date,
        end: Date,
        payment_freq: Tenor,
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
            commitment_amount: Money::new(10_000_000.0, Currency::USD),
            drawn_amount: Money::new(5_000_000.0, Currency::USD),
            commitment_date: start,
            maturity: end,
            base_rate_spec,
            day_count: DayCount::Act360,
            frequency: payment_freq,
            fees: super::super::types::RevolvingCreditFees::default(),
            draw_repay_spec: super::super::types::DrawRepaySpec::Deterministic(vec![]),
            discount_curve_id: "USD-OIS".into(),
            credit_curve_id: None,
            recovery_rate: 0.0,
            stub: StubKind::ShortFront,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: attrs,
        }
    }

    #[test]
    fn test_resolve_facility_calendar_none() {
        let attrs = Attributes::new();
        assert!(resolve_facility_calendar(&attrs).is_none());
    }

    #[test]
    fn test_resolve_facility_calendar_with_id() {
        // Test with a calendar that exists (WMR = Weekends, Memorial days, Retail calendar)
        let attrs = Attributes::new().with_meta("calendar_id", "WMR");
        let cal = resolve_facility_calendar(&attrs);
        // Calendar resolution depends on global registry state, so we just test the mechanism
        // If a calendar doesn't exist, it returns None (which is fine for this test)
        let _ = cal;
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
                reset_freq: Tenor::quarterly(),
                index_tenor: None,
                reset_lag_days: 2,
                dc: DayCount::Act360,
                bdc: finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".to_string(),
                fixing_calendar_id: None,
                end_of_month: false,
                payment_lag_days: 0,
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
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            dc: DayCount::Act360,
            bdc: finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".to_string(),
            fixing_calendar_id: None,
            end_of_month: false,
            payment_lag_days: 2,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        };
        let forward = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(reset)
            .knots(vec![(0.0, 0.03), (1.0, 0.03)])
            .build()
            .expect("forward curve");

        let rate = project_floating_rate_with_curve(reset, &spec, &forward, &Attributes::new())
            .expect("projected coupon");
        assert!((rate - 0.02).abs() < 1e-12, "all-in cap must bind: {rate}");
        assert_eq!(
            floating_payment_date(
                &spec,
                Date::from_calendar_date(2025, Month::January, 3).expect("date")
            )
            .expect("lagged date"),
            Date::from_calendar_date(2025, Month::January, 7).expect("date")
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

        // With NYC calendar
        let facility_with_cal = create_test_facility(
            start,
            end,
            Tenor::quarterly(),
            BaseRateSpec::Fixed { rate: 0.05 },
            Some("NYC"),
        );
        let dates_with_cal = build_payment_dates(&facility_with_cal, false)
            .expect("Payment dates building should succeed in test");

        // Both should have same length (quarterly over 1 year)
        assert_eq!(dates_no_cal.len(), dates_with_cal.len());
    }

    #[test]
    fn test_compute_reset_period_end_monthly() {
        let reset_date =
            Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
        let spec = reset_spec(
            finstack_quant_core::dates::Tenor::new(
                3,
                finstack_quant_core::dates::TenorUnit::Months,
            ),
            "weekends_only",
        );
        let reset_end = compute_reset_period_end(reset_date, &spec)
            .expect("Reset period end calculation should succeed");

        // 3 months from Jan 15 should be Apr 15
        let expected = Date::from_calendar_date(2025, Month::April, 15).expect("Valid test date");
        assert_eq!(reset_end, expected);
    }

    #[test]
    fn test_compute_reset_period_end_daily() {
        let reset_date =
            Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
        let spec = reset_spec(
            finstack_quant_core::dates::Tenor::new(90, finstack_quant_core::dates::TenorUnit::Days),
            "weekends_only",
        );
        let reset_end = compute_reset_period_end(reset_date, &spec)
            .expect("Reset period end calculation should succeed");

        // 90 days from Jan 15
        let expected = reset_date + time::Duration::days(90);
        assert_eq!(reset_end, expected);
    }

    #[test]
    fn test_compute_reset_period_end_with_calendar() {
        let reset_date =
            Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
        let tenor = finstack_quant_core::dates::Tenor::new(
            1,
            finstack_quant_core::dates::TenorUnit::Months,
        );
        let weekends_spec = reset_spec(tenor, "weekends_only");
        let wmr_spec = reset_spec(tenor, "USNY");

        let end_no_cal = compute_reset_period_end(reset_date, &weekends_spec)
            .expect("Reset period end calculation should succeed");

        let end_with_cal = compute_reset_period_end(reset_date, &wmr_spec)
            .expect("Reset period end calculation should succeed");

        // Both should succeed (calendar adjustment may or may not change date)
        assert!(end_no_cal.year() == 2025);
        assert!(end_with_cal.year() == 2025);
    }

    #[test]
    fn test_apply_draw_repay_event_draw() {
        use super::super::types::DrawRepayEvent;

        let balance = Money::new(5_000_000.0, Currency::USD);
        let commitment = Money::new(10_000_000.0, Currency::USD);
        let draw_date = Date::from_calendar_date(2025, Month::March, 1).expect("Valid test date");

        let event = DrawRepayEvent {
            date: draw_date,
            amount: Money::new(2_000_000.0, Currency::USD),
            is_draw: true,
        };

        let new_balance = apply_draw_repay_event(balance, &event, commitment)
            .expect("Draw/repay event application should succeed");
        assert_eq!(new_balance.amount(), 7_000_000.0);
    }

    #[test]
    fn test_apply_draw_repay_event_repay() {
        use super::super::types::DrawRepayEvent;

        let balance = Money::new(5_000_000.0, Currency::USD);
        let commitment = Money::new(10_000_000.0, Currency::USD);
        let repay_date = Date::from_calendar_date(2025, Month::March, 1).expect("Valid test date");

        let event = DrawRepayEvent {
            date: repay_date,
            amount: Money::new(1_000_000.0, Currency::USD),
            is_draw: false,
        };

        let new_balance = apply_draw_repay_event(balance, &event, commitment)
            .expect("Draw/repay event application should succeed");
        assert_eq!(new_balance.amount(), 4_000_000.0);
    }

    #[test]
    fn test_apply_draw_repay_event_exceeds_commitment() {
        use super::super::types::DrawRepayEvent;

        let balance = Money::new(8_000_000.0, Currency::USD);
        let commitment = Money::new(10_000_000.0, Currency::USD);
        let draw_date = Date::from_calendar_date(2025, Month::March, 1).expect("Valid test date");

        let event = DrawRepayEvent {
            date: draw_date,
            amount: Money::new(3_000_000.0, Currency::USD), // Would exceed commitment
            is_draw: true,
        };

        let result = apply_draw_repay_event(balance, &event, commitment);
        assert!(result.is_err());
        assert!(result
            .expect_err("should fail")
            .to_string()
            .contains("exceed commitment"));
    }
}
