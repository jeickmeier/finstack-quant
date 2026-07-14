//! Canonical optioned-coupon projection for every cap/floor pathway.

use crate::cashflow::builder::periods::SchedulePeriod;
use crate::instruments::common_impl::pricing::overnight::{
    project_overnight_coupon, resolve_overnight_fixing_calendar, OvernightCouponProjectionInput,
    OvernightObservationExposure, OvernightProjectionCurve,
};
use crate::instruments::rates::cap_floor::{CapFloor, OvernightSpreadCompounding};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;

/// Resolved economics for one optioned cap/floor coupon.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OptionedCouponProjection {
    /// Projected equivalent annual coupon rate.
    pub forward: f64,
    /// Contractual date on which the optioned coupon is fully determined.
    pub fixing_date: Date,
    /// Contractual payment date after any overnight payment delay.
    pub payment_date: Date,
    /// Accrual fraction multiplying the option payoff.
    pub accrual_year_fraction: f64,
    /// Sensitivity to a parallel bump of projected overnight factors.
    pub parallel_forward_sensitivity: f64,
    /// Second sensitivity to the same parallel projected-forward bump.
    pub parallel_forward_second_sensitivity: f64,
    /// Whether this is a compounded overnight rather than term-index coupon.
    pub is_compounded_overnight: bool,
    /// Date-specific stochastic exposures for compounded overnight observations.
    pub observation_exposures: Vec<OvernightObservationExposure>,
}

/// Canonical market inputs shared by standard pricing, HW pricing, Greeks, and implied vol.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OptionedCapletInputs {
    /// Resolved contractual coupon and dates.
    pub coupon: OptionedCouponProjection,
    /// Discount factor from valuation to contractual payment.
    pub discount_factor: f64,
    /// ACT/365F option time to the contractual final fixing.
    pub time_to_fixing: f64,
}

fn fixing_series_id(cap_floor: &CapFloor) -> String {
    finstack_quant_core::market_data::fixings::fixing_series_id(cap_floor.forward_curve_id.as_str())
}

/// Resolve the exact coupon projection consumed by pricing, HW, and metrics.
pub(crate) fn resolve_optioned_coupon(
    cap_floor: &CapFloor,
    period: &SchedulePeriod,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<OptionedCouponProjection> {
    let forward_curve = market.get_forward(cap_floor.forward_curve_id.as_ref())?;
    let spread = cap_floor.spread_f64()?;

    if let Some(overnight) = &cap_floor.overnight_coupon {
        let calendar_id = overnight
            .fixing_calendar_id
            .as_deref()
            .or(cap_floor.calendar_id.as_deref());
        let calendar = resolve_overnight_fixing_calendar(
            calendar_id,
            cap_floor.notional.currency(),
            &format!("CapFloor '{}'", cap_floor.id),
        )?;
        let series_id = fixing_series_id(cap_floor);
        let fixings = market.get_series(&series_id).ok();
        let compounded_spread = match overnight.spread_compounding {
            OvernightSpreadCompounding::Exclude => 0.0,
            OvernightSpreadCompounding::Include => spread,
        };
        let projection = project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Forward(forward_curve.as_ref()),
            fixings,
            fixing_id: cap_floor.forward_curve_id.as_str(),
            as_of,
            accrual_start: period.accrual_start,
            accrual_end: period.accrual_end,
            day_count: cap_floor.day_count,
            coupon_frequency: Some(cap_floor.frequency),
            compounding: &overnight.compounding,
            fixing_calendar: calendar,
            compounded_spread,
        })?;
        let coupon_rate = match overnight.spread_compounding {
            OvernightSpreadCompounding::Exclude => projection.rate + spread,
            OvernightSpreadCompounding::Include => projection.rate,
        };
        return Ok(OptionedCouponProjection {
            forward: coupon_rate,
            fixing_date: projection.fixing_date,
            payment_date: period.payment_date,
            accrual_year_fraction: projection.accrual_year_fraction,
            parallel_forward_sensitivity: projection.parallel_forward_sensitivity,
            parallel_forward_second_sensitivity: projection.parallel_forward_second_sensitivity,
            is_compounded_overnight: true,
            observation_exposures: projection.observation_exposures,
        });
    }

    let fixing_date = period.reset_date.unwrap_or(period.accrual_start);
    // Valuation is at start of day: a fixing dated exactly `as_of` is not yet
    // published, while earlier fixings must be supplied as observations.
    let index_forward = if fixing_date < as_of {
        let series_id = fixing_series_id(cap_floor);
        let series = market.get_series(&series_id).map_err(|_| {
            finstack_quant_core::Error::Validation(format!(
                "Seasoned cap/floor requires historical fixing series '{}' for fixing date {}. \
                 Fixed-but-unpaid coupons must be valued off observed fixings, not the live \
                 forward curve.",
                series_id, fixing_date
            ))
        })?;
        series.value_on_exact(fixing_date)?
    } else {
        crate::instruments::common_impl::pricing::time::rate_between_on_dates(
            forward_curve.as_ref(),
            period.accrual_start,
            period.accrual_end,
        )?
    };

    Ok(OptionedCouponProjection {
        forward: index_forward + spread,
        fixing_date,
        payment_date: period.payment_date,
        accrual_year_fraction: period.accrual_year_fraction,
        parallel_forward_sensitivity: 1.0,
        parallel_forward_second_sensitivity: 0.0,
        is_compounded_overnight: false,
        observation_exposures: Vec::new(),
    })
}

/// Resolve the complete path-independent coupon, fixing, payment, and discount inputs.
pub(crate) fn resolve_optioned_caplet_inputs(
    cap_floor: &CapFloor,
    period: &SchedulePeriod,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<OptionedCapletInputs> {
    let coupon = resolve_optioned_coupon(cap_floor, period, market, as_of)?;
    let discount_factor = if coupon.payment_date > as_of {
        let discount = market.get_discount(cap_floor.discount_curve_id.as_ref())?;
        crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
            discount.as_ref(),
            as_of,
            coupon.payment_date,
        )?
    } else {
        0.0
    };
    let time_to_fixing = if coupon.fixing_date > as_of {
        DayCount::Act365F.year_fraction(as_of, coupon.fixing_date, DayCountContext::default())?
    } else {
        0.0
    };
    Ok(OptionedCapletInputs {
        coupon,
        discount_factor,
        time_to_fixing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::rates::cap_floor::{
        CapFloorVolType, OvernightCouponConvention, OvernightSpreadCompounding,
    };
    use crate::instruments::rates::irs::FloatingLegCompounding;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, DayCountContext};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_core::money::Money;
    use rust_decimal::Decimal;
    use time::macros::date;

    fn compounded_sofr_caplet() -> CapFloor {
        let mut caplet = CapFloor::new_caplet(
            "SOFR-CAPLET",
            Money::new(1_000_000.0, Currency::USD),
            0.04,
            date!(2025 - 01 - 02),
            date!(2025 - 04 - 02),
            DayCount::Act360,
            "USD-OIS",
            "USD-SOFR-OIS",
            "USD-CAP-VOL",
        )
        .expect("caplet");
        caplet.vol_type = CapFloorVolType::Normal;
        caplet.overnight_coupon = Some(OvernightCouponConvention {
            compounding: FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 },
            payment_delay_days: 2,
            fixing_calendar_id: Some("usny".into()),
            payment_calendar_id: Some("usny".into()),
            spread_compounding: OvernightSpreadCompounding::Exclude,
        });
        caplet
    }

    fn market(as_of: Date) -> MarketContext {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (0.5, 0.98), (1.0, 0.95)])
            .build()
            .expect("discount");
        let forward = ForwardCurve::builder("USD-SOFR-OIS", 1.0 / 360.0)
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.025), (0.2, 0.035), (0.5, 0.06), (1.0, 0.065)])
            .build()
            .expect("forward");
        let surface = VolSurface::builder("USD-CAP-VOL")
            .expiries(&[0.1, 0.5, 1.0])
            .strikes(&[0.01, 0.04, 0.08])
            .row(&[0.005, 0.005, 0.005])
            .row(&[0.005, 0.005, 0.005])
            .row(&[0.005, 0.005, 0.005])
            .build()
            .expect("surface");
        MarketContext::new()
            .insert(discount)
            .insert(forward)
            .insert_surface(surface)
    }

    #[test]
    fn sofr_cutoff_caplet_uses_compounded_coupon_and_delayed_payment() {
        let as_of = date!(2024 - 12 - 02);
        let caplet = compounded_sofr_caplet();
        let market = market(as_of);
        caplet
            .validate_for_pricing()
            .expect("valid overnight caplet");
        let period = caplet
            .pricing_periods()
            .expect("periods")
            .into_iter()
            .next()
            .expect("one period");
        let projection =
            resolve_optioned_coupon(&caplet, &period, &market, as_of).expect("projection");
        let simple = crate::instruments::common_impl::pricing::time::rate_between_on_dates(
            market
                .get_forward(caplet.forward_curve_id.as_ref())
                .expect("forward")
                .as_ref(),
            period.accrual_start,
            period.accrual_end,
        )
        .expect("simple forward");

        assert_eq!(projection.fixing_date, date!(2025 - 03 - 31));
        assert_eq!(projection.payment_date, date!(2025 - 04 - 04));
        assert!(
            (projection.forward - simple).abs() > 1.0e-6,
            "contractual compounded coupon {} must differ from old simple forward {}",
            projection.forward,
            simple
        );
    }

    #[test]
    fn pricing_and_metric_projection_share_coupon_and_payment_date() {
        let as_of = date!(2024 - 12 - 02);
        let caplet = compounded_sofr_caplet();
        let market = market(as_of);
        let period = caplet.pricing_periods().expect("periods").remove(0);
        let projection =
            resolve_optioned_coupon(&caplet, &period, &market, as_of).expect("projection");
        let pv = caplet.value(&market, as_of).expect("price").amount();
        assert!(pv.is_finite() && pv >= 0.0);

        let discount = market.get_discount("USD-OIS").expect("discount");
        let delayed_df = discount
            .df_between_dates(as_of, projection.payment_date)
            .expect("delayed df");
        let undelayed_df = discount
            .df_between_dates(as_of, period.accrual_end)
            .expect("undelayed df");
        assert!(
            (delayed_df - undelayed_df).abs() > 1.0e-8,
            "payment delay must change the discount input"
        );
    }

    #[test]
    fn nonzero_spread_policy_changes_compounded_coupon() {
        let as_of = date!(2024 - 12 - 02);
        let market = market(as_of);
        let mut exclude = compounded_sofr_caplet();
        exclude.spread = Decimal::try_from(0.01).expect("spread");
        let mut include = exclude.clone();
        include
            .overnight_coupon
            .as_mut()
            .expect("overnight terms")
            .spread_compounding = OvernightSpreadCompounding::Include;
        let period = exclude.pricing_periods().expect("periods").remove(0);

        let excluded = resolve_optioned_coupon(&exclude, &period, &market, as_of)
            .expect("excluded spread projection");
        let included = resolve_optioned_coupon(&include, &period, &market, as_of)
            .expect("included spread projection");

        assert!(
            (included.forward - excluded.forward).abs() > 1.0e-8,
            "daily-compounded and simply-added spreads must differ: {} vs {}",
            included.forward,
            excluded.forward
        );
    }

    #[test]
    fn zero_spread_is_policy_compatible() {
        let as_of = date!(2024 - 12 - 02);
        let market = market(as_of);
        let exclude = compounded_sofr_caplet();
        let mut include = exclude.clone();
        include
            .overnight_coupon
            .as_mut()
            .expect("overnight terms")
            .spread_compounding = OvernightSpreadCompounding::Include;
        let period = exclude.pricing_periods().expect("periods").remove(0);

        let excluded = resolve_optioned_coupon(&exclude, &period, &market, as_of)
            .expect("excluded spread projection");
        let included = resolve_optioned_coupon(&include, &period, &market, as_of)
            .expect("included spread projection");

        assert_eq!(included.forward, excluded.forward);
        assert_eq!(
            included.parallel_forward_sensitivity,
            excluded.parallel_forward_sensitivity
        );
    }

    #[test]
    fn term_coupon_adds_contractual_spread_after_projection() {
        let as_of = date!(2024 - 12 - 02);
        let market = market(as_of);
        let mut caplet = compounded_sofr_caplet();
        caplet.overnight_coupon = None;
        caplet.spread = Decimal::try_from(0.01).expect("spread");
        let period = caplet.pricing_periods().expect("periods").remove(0);
        let projected =
            resolve_optioned_coupon(&caplet, &period, &market, as_of).expect("term projection");
        let simple = crate::instruments::common_impl::pricing::time::rate_between_on_dates(
            market
                .get_forward(caplet.forward_curve_id.as_ref())
                .expect("forward")
                .as_ref(),
            period.accrual_start,
            period.accrual_end,
        )
        .expect("simple forward");

        assert!((projected.forward - (simple + 0.01)).abs() < 1.0e-12);
    }

    #[test]
    fn shared_path_inputs_resolve_coupon_fixing_and_payment_once() {
        let as_of = date!(2024 - 12 - 02);
        let caplet = compounded_sofr_caplet();
        let market = market(as_of);
        let period = caplet.pricing_periods().expect("periods").remove(0);

        let resolved = resolve_optioned_caplet_inputs(&caplet, &period, &market, as_of)
            .expect("shared caplet inputs");

        assert_eq!(resolved.coupon.fixing_date, date!(2025 - 03 - 31));
        assert_eq!(resolved.coupon.payment_date, date!(2025 - 04 - 04));
        assert_eq!(
            resolved.coupon.accrual_year_fraction,
            period.accrual_year_fraction
        );
        assert_eq!(
            resolved.discount_factor,
            market
                .get_discount("USD-OIS")
                .expect("discount")
                .df_between_dates(as_of, resolved.coupon.payment_date)
                .expect("payment discount")
        );
        assert_eq!(
            resolved.time_to_fixing,
            DayCount::Act365F
                .year_fraction(
                    as_of,
                    resolved.coupon.fixing_date,
                    DayCountContext::default()
                )
                .expect("fixing time")
        );
    }

    #[test]
    fn overnight_accrual_end_adjusts_before_payment_delay() {
        let as_of = date!(2024 - 12 - 02);
        let market = market(as_of);
        let mut caplet = compounded_sofr_caplet();
        caplet.maturity = date!(2025 - 07 - 04);
        caplet
            .overnight_coupon
            .as_mut()
            .expect("overnight terms")
            .compounding = FloatingLegCompounding::CompoundedInArrears {
            lookback_days: 0,
            observation_shift: None,
        };
        let period = caplet.pricing_periods().expect("periods").remove(0);

        let resolved = resolve_optioned_caplet_inputs(&caplet, &period, &market, as_of)
            .expect("resolved inputs");

        assert_eq!(period.accrual_end, date!(2025 - 07 - 07));
        let adjusted_accrual = DayCount::Act360
            .year_fraction(
                period.accrual_start,
                period.accrual_end,
                DayCountContext::default(),
            )
            .expect("adjusted accrual fraction");
        let unadjusted_accrual = DayCount::Act360
            .year_fraction(
                caplet.start_date,
                caplet.maturity,
                DayCountContext::default(),
            )
            .expect("unadjusted accrual fraction");
        assert_eq!(period.accrual_year_fraction, adjusted_accrual);
        assert_ne!(adjusted_accrual, unadjusted_accrual);
        assert_eq!(
            resolved
                .coupon
                .observation_exposures
                .last()
                .expect("last observation")
                .observation_end,
            date!(2025 - 07 - 07)
        );
        assert_eq!(resolved.coupon.payment_date, date!(2025 - 07 - 09));
    }

    #[test]
    fn same_day_term_fixing_is_not_yet_published_but_has_zero_option_time() {
        let as_of = date!(2025 - 01 - 02);
        let mut caplet = compounded_sofr_caplet();
        caplet.overnight_coupon = None;
        caplet.forward_curve_id = "TEST-TERM-3M".into();
        let period = caplet.pricing_periods().expect("periods").remove(0);
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .day_count(DayCount::Act365F)
                    .knots([(0.0, 1.0), (1.0, 0.95)])
                    .build()
                    .expect("discount"),
            )
            .insert(
                ForwardCurve::builder("TEST-TERM-3M", 0.25)
                    .base_date(as_of)
                    .day_count(DayCount::Act360)
                    .knots([(0.0, 0.12), (1.0, 0.12)])
                    .build()
                    .expect("forward"),
            );

        let resolved = resolve_optioned_caplet_inputs(&caplet, &period, &market, as_of)
            .expect("same-day fixed inputs");

        assert_eq!(resolved.coupon.forward, 0.12);
        assert_eq!(resolved.time_to_fixing, 0.0);
    }
}
