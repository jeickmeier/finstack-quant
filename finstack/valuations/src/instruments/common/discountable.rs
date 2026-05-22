//! Compatibility layer for discounting instrument cashflow schedules.

pub use finstack_core::cashflow::Discountable;

use finstack_core::dates::Date;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::Money;

/// Discount dated `Money` flows to `as_of` using the curve's own day-count and
/// date-based discount factor calculation (**holder-view** semantics).
///
/// # Cashflow-on-as_of Policy: HOLDER-VIEW (Excludes `d <= as_of`)
///
/// This helper treats valuation as occurring **just after settlement**:
/// - Cashflows where `d <= as_of` are considered already settled and are **excluded**
/// - Only future cashflows (`d > as_of`) contribute to NPV
///
/// ## When to Use
///
/// Use this for instruments where the holder has already received or paid any
/// cashflow due on `as_of`:
/// - **Term loans**: Interest accrued up to `as_of` is already paid
/// - **Bonds (dirty price)**: Coupon on `as_of` has been received
/// - **Seasoned swaps**: Past cashflows are settled
///
/// ## Alternative
///
/// For T+0 instruments or calibration where cashflows on `as_of` should be included,
/// use [`crate::instruments::common_impl::helpers::schedule_pv_using_curve_dc_raw`] which
/// includes `d == as_of` cashflows (pricing-view semantics).
///
/// # Arguments
///
/// * `disc` - Discount curve for date-based DF lookup
/// * `as_of` - Valuation date (cashflows on or before this are excluded)
/// * `flows` - Vector of (date, amount) pairs
///
/// # Returns
///
/// Sum of discounted future cashflows (holder-view NPV).
pub fn npv_by_date(
    disc: &DiscountCurve,
    as_of: Date,
    flows: &[(Date, Money)],
) -> finstack_core::Result<Money> {
    if flows.is_empty() {
        return Err(finstack_core::InputError::TooFewPoints.into());
    }

    let ccy = flows[0].1.currency();
    let mut total = Money::new(0.0, ccy);

    for (d, amt) in flows {
        // HOLDER-VIEW: exclude cashflows on or before as_of (already settled)
        if *d <= as_of {
            continue;
        }
        let df = disc.df_between_dates(as_of, *d)?;

        total = total.checked_add(*amt * df)?;
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::{CashFlowSchedule, CouponType, FixedCouponSpec, ScheduleParams};
    use finstack_core::currency::Currency;
    use finstack_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
    use finstack_core::market_data::traits::{Discounting, TermStructure};
    use finstack_core::money::Money;
    use finstack_core::types::CurveId;

    use time::Month;

    struct FlatCurve {
        id: CurveId,
    }

    impl TermStructure for FlatCurve {
        fn id(&self) -> &CurveId {
            &self.id
        }
    }

    impl Discounting for FlatCurve {
        fn base_date(&self) -> Date {
            Date::from_calendar_date(2025, Month::January, 1).expect("valid date")
        }
        fn df(&self, _t: f64) -> f64 {
            1.0
        }
    }

    fn simple_schedule() -> CashFlowSchedule {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");
        let params = ScheduleParams {
            freq: Tenor::quarterly(),
            dc: DayCount::Act365F,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        };
        let fixed = FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid rate"),
            freq: params.freq,
            dc: params.dc,
            bdc: params.bdc,
            calendar_id: params.calendar_id.clone(),
            stub: params.stub,
            end_of_month: params.end_of_month,
            payment_lag_days: params.payment_lag_days,
        };
        CashFlowSchedule::builder()
            .principal(Money::new(1_000.0, Currency::USD), issue, maturity)
            .fixed_cf(fixed)
            .build_with_curves(None)
            .expect("should build schedule")
    }

    #[test]
    fn schedule_discountable_paths_through() {
        let curve = FlatCurve {
            id: CurveId::new("USD-OIS"),
        };
        let base = curve.base_date();
        let schedule = simple_schedule();
        // Use explicit day count
        let pv = schedule
            .npv(&curve, base, Some(DayCount::Act365F))
            .expect("should calculate NPV");
        assert!(pv.amount().is_finite());
    }

    // ==================== PV SEMANTICS TESTS ====================

    fn create_test_curve() -> finstack_core::market_data::term_structures::DiscountCurve {
        use finstack_core::market_data::term_structures::DiscountCurve;
        let base = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        DiscountCurve::builder("TEST")
            .base_date(base)
            .knots([(0.0, 1.0), (0.5, 0.98), (1.0, 0.95)])
            .build()
            .expect("should build")
    }

    #[test]
    fn holder_view_excludes_cashflow_on_as_of() {
        let disc = create_test_curve();
        let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let future = Date::from_calendar_date(2024, Month::July, 1).expect("valid date");

        // Flow exactly on as_of (should be EXCLUDED in holder-view)
        let flows = vec![
            (as_of, Money::new(100.0, Currency::USD)),  // on as_of
            (future, Money::new(100.0, Currency::USD)), // future
        ];

        let pv = npv_by_date(&disc, as_of, &flows).expect("should succeed");

        // Holder-view: only future flow should contribute
        // DF for 6 months ≈ 0.98
        assert!(
            pv.amount() > 90.0 && pv.amount() < 100.0,
            "Holder-view PV should only include future flow: {}",
            pv.amount()
        );

        // Specifically: should NOT be ~200 (both flows) or ~100 (just as_of flow)
        assert!(
            pv.amount() < 105.0,
            "Should exclude as_of cashflow: {}",
            pv.amount()
        );
    }

    #[test]
    fn holder_view_excludes_past_cashflows() {
        let disc = create_test_curve();
        let as_of = Date::from_calendar_date(2024, Month::July, 1).expect("valid date");
        let past = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let future = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        let flows = vec![
            (past, Money::new(100.0, Currency::USD)),  // past (< as_of)
            (future, Money::new(50.0, Currency::USD)), // future
        ];

        let pv_holder = npv_by_date(&disc, as_of, &flows).expect("holder-view");

        // Only the future flow should contribute (past is excluded).
        assert!(
            pv_holder.amount() > 0.0 && pv_holder.amount() < 50.0,
            "Holder-view PV should only include the future flow: {}",
            pv_holder.amount()
        );
    }
}
