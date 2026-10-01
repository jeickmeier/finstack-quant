//! Time-basis helpers for bond pricing (model maturity vs discount-curve dates).
//!
//! Bond Monte Carlo and structural models use the cashflow spec day count for
//! simulation time, except ACT/365L uses the standard ACT/365F model clock.
//! Coupon ACT/365L remains a contractual accrual convention. Discount factors
//! use the discount curve's own base date and day count at actual payment dates.

use crate::instruments::fixed_income::bond::pricing::quote_conversions::icma_reference_period;
use crate::instruments::fixed_income::bond::types::Bond;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::Result;

/// Payment dates and actual coupon boundaries for ACT/365L bond timing.
///
/// Ordinary bonds use the canonical period builder without projecting coupon
/// amounts. Custom schedules retain the accrual metadata owned by each coupon.
///
/// The tuples contain `(payment_date, accrual_start, accrual_end)`.
///
/// # Arguments
///
/// * `bond` - Bond supplying schedule conventions or coupon-owned accrual metadata.
pub(crate) fn bond_coupon_dates(bond: &Bond) -> Result<Vec<(Date, Date, Date)>> {
    if let Some(schedule) = &bond.custom_cashflows {
        let mut periods = Vec::new();
        for flow in schedule.get_flows() {
            if !(flow.kind.is_interest_like() || flow.kind == CFKind::Pik) {
                continue;
            }
            let accrual = flow.accrual.as_ref().ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "ACT/365L bond timing requires contractual coupon accrual metadata".into(),
                )
            })?;
            let period = accrual.coupon_period.ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "ACT/365L custom bond timing requires explicit full coupon_period metadata"
                        .into(),
                )
            })?;
            if period.0 >= period.1 || accrual.start < period.0 || accrual.end > period.1 {
                return Err(finstack_quant_core::Error::Validation(
                    "ACT/365L custom bond accrual metadata lies outside its full coupon period"
                        .into(),
                ));
            }
            periods.push((flow.date, period.0, period.1));
        }
        periods.sort_unstable();
        periods.dedup();
        let mut boundaries: Vec<_> = periods
            .iter()
            .map(|&(_, start, end)| (start, end))
            .collect();
        boundaries.sort_unstable();
        boundaries.dedup();
        if boundaries.windows(2).any(|pair| pair[0].1 > pair[1].0) {
            return Err(finstack_quant_core::Error::Validation(
                "ACT/365L custom bond timing requires non-overlapping full coupon periods".into(),
            ));
        }
        return Ok(periods);
    }
    use finstack_quant_cashflows::builder::periods::{build_periods, BuildPeriodsParams};
    Ok(build_periods(BuildPeriodsParams::from_schedule(
        bond.cashflow_spec.schedule(),
        bond.issue_date,
        bond.maturity,
        None,
    ))?
    .into_iter()
    .map(|period| {
        (
            period.payment_date,
            period.accrual_start,
            period.accrual_end,
        )
    })
    .collect())
}

/// Actual full coupon boundaries, independent of operational payment dates.
///
/// # Arguments
///
/// * `bond` - Bond supplying canonical periods or explicit custom coupon metadata.
pub(crate) fn bond_coupon_periods(bond: &Bond) -> Result<Vec<(Date, Date)>> {
    let mut periods: Vec<_> = bond_coupon_dates(bond)?
        .into_iter()
        .map(|(_, start, end)| (start, end))
        .collect();
    periods.sort_unstable();
    periods.dedup();
    Ok(periods)
}

/// Sum core ACT/365L fractions over the actual enclosing coupon of each slice.
///
/// Missing coverage, including a payment lag outside the final accrual period,
/// is an error rather than an inferred coupon denominator.
///
/// # Arguments
///
/// * `frequency` - Contractual coupon tenor selecting the annual or nonannual denominator rule.
/// * `periods` - Actual full coupon accrual boundaries covering the requested interval.
/// * `start` - Inclusive origin of the bond clock interval.
/// * `end` - Exclusive endpoint of the interval; must not precede `start`.
pub(crate) fn act365l_year_fraction(
    frequency: Tenor,
    periods: &[(Date, Date)],
    start: Date,
    end: Date,
) -> Result<f64> {
    if start > end {
        return Err(finstack_quant_core::InputError::InvalidDateRange.into());
    }
    let mut current = start;
    let mut total = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    while current < end {
        let &(coupon_start, coupon_end) = periods
            .iter()
            .find(|&&(coupon_start, coupon_end)| coupon_start <= current && current < coupon_end)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "ACT/365L bond timing has no enclosing contractual coupon for {current} through {end}"
                ))
            })?;
        let slice_end = end.min(coupon_end);
        total.add(DayCount::Act365L.year_fraction(
            current,
            slice_end,
            DayCountContext {
                frequency: Some(frequency),
                coupon_period: Some((coupon_start, coupon_end)),
                ..DayCountContext::default()
            },
        )?);
        current = slice_end;
    }
    Ok(total.total())
}

/// Remaining contractual coupon grid of a bond on the bond model clock.
///
/// Model time is the bond coupon day-count year fraction from `as_of`. For
/// ACT/ACT (ICMA) the day-count context carries the coupon frequency and the
/// quasi-coupon period surrounding `as_of`, as the yield engines do.
/// ACT/365L uses the standard ACT/365F model clock at actual payment dates,
/// independent of contractual coupon fractions and operational payment lag.
#[derive(Debug, Clone)]
pub(crate) struct BondModelSchedule {
    /// Final redemption date: the later of maturity and the last coupon date.
    pub(crate) horizon: Date,
    /// Model time of `horizon` in years.
    pub(crate) maturity_years: f64,
    /// Remaining coupons as `(model time, accrual year fraction, payment date)`.
    /// Coupons inside an ex-coupon window at `as_of` are excluded.
    pub(crate) coupons: Vec<(f64, f64, Date)>,
    /// Accrued interest at `as_of` in currency units (negative inside an
    /// ex-coupon window).
    pub(crate) accrued: f64,
}

/// Build the remaining coupon grid and horizon of a fixed-coupon bond.
///
/// Coupon dates and accrual fractions come from the bond's own cashflow
/// schedule (cash, stub and PIK coupons), so the full next coupon is kept and
/// a model driven by this grid produces a dirty value.
pub(crate) fn bond_model_schedule(bond: &Bond, as_of: Date) -> Result<BondModelSchedule> {
    let schedule = bond.full_cashflow_schedule(&MarketContext::new())?;
    let ex_coupon = bond.accrual_config().ex_coupon;
    let mut coupon_flows: Vec<(Date, f64)> = Vec::new();
    for cf in schedule.get_flows() {
        if cf.date <= as_of || !(cf.kind.is_interest_like() || cf.kind == CFKind::Pik) {
            continue;
        }
        if let Some(rule) = &ex_coupon {
            if rule.is_ex_coupon(cf.date, as_of)? {
                continue;
            }
        }
        // Split coupons emit a cash and a PIK flow on the same date for one
        // accrual period; keep one entry per payment date.
        if coupon_flows.last().is_some_and(|&(d, _)| d == cf.date) {
            continue;
        }
        coupon_flows.push((cf.date, cf.accrual_factor));
    }

    let day_count = bond.cashflow_spec.day_count();
    let frequency = bond.cashflow_spec.frequency();
    let dc_ctx = DayCountContext {
        frequency: Some(frequency),
        coupon_period: icma_reference_period(
            day_count,
            frequency,
            coupon_flows.iter().map(|&(d, _)| d),
            as_of,
        ),
        ..DayCountContext::default()
    };
    let year_fraction = |date| {
        if day_count == DayCount::Act365L {
            Ok(finstack_quant_models::rates::clock::model_time(as_of, date))
        } else {
            day_count.year_fraction(as_of, date, dc_ctx)
        }
    };
    let horizon = coupon_flows
        .last()
        .map_or(bond.maturity, |&(d, _)| d.max(bond.maturity));
    let maturity_years = year_fraction(horizon)?;
    let coupons = coupon_flows
        .into_iter()
        .map(|(date, accrual)| Ok((year_fraction(date)?, accrual, date)))
        .collect::<Result<Vec<_>>>()?;
    let accrued = crate::cashflow::accrual::accrued_interest_amount(
        &schedule,
        as_of,
        &bond.accrual_config(),
    )?;
    Ok(BondModelSchedule {
        horizon,
        maturity_years,
        coupons,
        accrued,
    })
}

/// Constant continuously-compounded rate implied by the curve DF from `as_of` to `maturity`.
///
/// Uses [`DiscountCurve::df_between_dates`] and `mat_years` on the bond model clock:
/// `r = -ln(DF) / mat_years`.
pub(crate) fn implied_flat_discount_rate_from_curve(
    disc: &DiscountCurve,
    as_of: Date,
    maturity: Date,
    mat_years: f64,
) -> Result<f64> {
    if mat_years <= 0.0 {
        return Ok(0.0);
    }
    let df = disc.df_between_dates(as_of, maturity)?;
    if df > 0.0 {
        Ok(-df.ln() / mat_years)
    } else {
        Ok(0.0)
    }
}

/// Map model time `t` (years on the bond clock, `0..=mat_years`) to a calendar date.
fn date_at_model_time(as_of: Date, maturity: Date, mat_years: f64, t: f64) -> Date {
    if mat_years <= 0.0 || t <= 0.0 {
        return as_of;
    }
    if t >= mat_years {
        return maturity;
    }
    let span_days = (maturity - as_of).whole_days();
    if span_days <= 0 {
        return as_of;
    }
    let offset = ((t / mat_years) * span_days as f64).round() as i32;
    as_of + time::Duration::days(i64::from(offset))
}

/// Build `(model_time, df)` knots for Merton MC cashflow discounting.
///
/// `model_time` runs on the bond clock. Knots sit on the simulation grid
/// (`DF(as_of → date_at_model_time)`, used for default-time recoveries) plus
/// one exact knot per coupon and at the horizon, so contractual cashflows
/// are discounted to their actual payment dates.
pub(crate) fn bond_cashflow_dfs_on_model_grid(
    disc: &DiscountCurve,
    as_of: Date,
    schedule: &BondModelSchedule,
    steps_per_year: usize,
) -> Result<Vec<(f64, f64)>> {
    let mat_years = schedule.maturity_years;
    if mat_years <= 0.0 || steps_per_year == 0 {
        return Ok(Vec::new());
    }
    let n = (mat_years * steps_per_year as f64).round().max(1.0) as usize;
    let mut dfs = Vec::with_capacity(n + schedule.coupons.len() + 1);
    for i in 1..=n {
        let t = i as f64 / steps_per_year as f64;
        let pay_date = date_at_model_time(as_of, schedule.horizon, mat_years, t);
        dfs.push((t, disc.df_between_dates(as_of, pay_date)?));
    }
    for &(t, _, date) in &schedule.coupons {
        dfs.push((t, disc.df_between_dates(as_of, date)?));
    }
    dfs.push((mat_years, disc.df_between_dates(as_of, schedule.horizon)?));
    // Exact cashflow knots win over grid knots at the same model time.
    dfs.reverse();
    dfs.sort_by(|a, b| a.0.total_cmp(&b.0));
    dfs.dedup_by(|later, earlier| (later.0 - earlier.0).abs() < 1e-12);
    Ok(dfs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::money::Money;
    use time::macros::date;

    #[test]
    fn act365l_bond_model_and_coupon_clocks_keep_independent_bases() {
        let mut spec = crate::instruments::fixed_income::bond::CashflowSpec::fixed(
            0.05,
            Tenor::semi_annual(),
            DayCount::Act365L,
        )
        .expect("spec");
        if let crate::instruments::fixed_income::bond::CashflowSpec::Fixed(spec) = &mut spec {
            spec.schedule.business_day_convention =
                finstack_quant_core::dates::BusinessDayConvention::Unadjusted;
        }
        let bond = Bond::builder()
            .id("ACT365L-MODEL".into())
            .notional(Money::from((1000_i64, Currency::USD)))
            .issue_date(date!(2023 - 10 - 15))
            .maturity(date!(2025 - 04 - 15))
            .cashflow_spec(spec)
            .discount_curve_id("USD-OIS".into())
            .attributes(crate::instruments::Attributes::new())
            .build()
            .expect("bond");
        let as_of = date!(2023 - 12 - 15);
        let model = bond_model_schedule(&bond, as_of).expect("model schedule");
        let expected = 305.0 / 366.0 + 182.0 / 365.0;
        assert!(
            (model.maturity_years - (bond.maturity - as_of).whole_days() as f64 / 365.0).abs()
                < 1e-14
        );
        assert_eq!(model.coupons.len(), 3);
        assert!((model.coupons[0].0 - 122.0 / 365.0).abs() < 1e-14);
        assert!((model.coupons[0].1 - 183.0 / 366.0).abs() < 1e-14);
        assert!((model.accrued - 1000.0 * 0.05 * 61.0 / 366.0).abs() < 1e-12);
        let periods = bond_coupon_periods(&bond).expect("coupon periods");
        assert!(
            act365l_year_fraction(Tenor::semi_annual(), &periods, as_of, date!(2025 - 04 - 16),)
                .is_err(),
            "payment lag beyond the final coupon requires explicit coverage"
        );
        // Deferring coupons to one payment date retains distinct contractual
        // periods: the last uses 365, while the earlier ones use 366.
        let mut custom = bond
            .full_cashflow_schedule(&MarketContext::new())
            .expect("custom source");
        custom.update_flows(|flow| {
            if flow.kind.is_interest_like() {
                flow.date = bond.maturity;
            }
        });
        let custom_bond = bond.clone().with_custom_cashflows(custom.clone());
        let custom_periods = bond_coupon_periods(&custom_bond).expect("explicit coupon periods");
        assert_eq!(custom_periods, periods);
        assert!(
            (act365l_year_fraction(Tenor::semi_annual(), &custom_periods, as_of, bond.maturity,)
                .expect("custom clock")
                - expected)
                .abs()
                < 1e-14
        );
        let custom_flows = custom_bond
            .pricing_dated_cashflows(&MarketContext::new(), as_of)
            .expect("deferred coupon flows");
        assert!(
            crate::instruments::fixed_income::bond::pricing::quote_conversions::bond_flow_times(
                &custom_bond,
                &custom_flows,
                as_of,
            )
            .is_err(),
            "dated-flow aggregation cannot recover distinct coupon timings on one payment date"
        );
        let mut delayed = bond.clone();
        delayed.maturity = date!(2025 - 06 - 01);
        if let crate::instruments::fixed_income::bond::CashflowSpec::Fixed(spec) =
            &mut delayed.cashflow_spec
        {
            spec.schedule.business_day_convention =
                finstack_quant_core::dates::BusinessDayConvention::Following;
            spec.schedule.payment_lag_days = 3;
        }
        let delayed_model = bond_model_schedule(&delayed, as_of).expect("delayed model schedule");
        assert_eq!(delayed_model.horizon, date!(2025 - 06 - 05));
        assert!(
            (delayed_model.maturity_years
                - (date!(2025 - 06 - 05) - as_of).whole_days() as f64 / 365.0)
                .abs()
                < 1e-14
        );
        let curve = DiscountCurve::builder("MODEL-DATES")
            .base_date(as_of)
            .knots([(0.0, 1.0), (3.0, 0.88)])
            .build()
            .expect("model discount curve");
        let dfs = bond_cashflow_dfs_on_model_grid(&curve, as_of, &delayed_model, 12)
            .expect("actual payment discount knots");
        let final_df = dfs
            .iter()
            .find(|(time, _)| (*time - delayed_model.maturity_years).abs() < 1e-14)
            .expect("horizon discount factor")
            .1;
        assert!(
            (final_df
                - curve
                    .df_between_dates(as_of, date!(2025 - 06 - 05))
                    .expect("payment date discount"))
            .abs()
                < 1e-14
        );
        // A subinterval cannot identify its enclosing coupon without full
        // boundaries, and overlapping full coupons are ambiguous.
        custom.update_flows(|flow| {
            if let Some(accrual) = &mut flow.accrual {
                accrual.coupon_period = None;
            }
        });
        assert!(bond_coupon_periods(&bond.clone().with_custom_cashflows(custom)).is_err());
        let mut overlap = custom_bond.custom_cashflows.expect("custom schedule");
        overlap.update_flows(|flow| {
            if let Some(accrual) = &mut flow.accrual {
                if accrual.start == bond.issue_date {
                    accrual.coupon_period = Some((bond.issue_date, bond.maturity));
                }
            }
        });
        assert!(bond_coupon_periods(&bond.with_custom_cashflows(overlap)).is_err());
    }

    #[test]
    fn bond_model_maturity_respects_day_count() {
        let as_of = date!(2025 - 01 - 01);
        let maturity = date!(2030 - 01 - 01);
        let bond_365 = crate::instruments::fixed_income::bond::Bond::builder()
            .id("B365".into())
            .notional(Money::from((100_i64, Currency::USD)))
            .issue_date(as_of)
            .maturity(maturity)
            .cashflow_spec(
                crate::instruments::fixed_income::bond::CashflowSpec::fixed(
                    0.05,
                    finstack_quant_core::dates::Tenor::semi_annual(),
                    DayCount::Act365F,
                )
                .expect("spec"),
            )
            .discount_curve_id("USD-OIS".into())
            .attributes(crate::instruments::Attributes::new())
            .build()
            .expect("bond");
        let y_365 = bond_model_schedule(&bond_365, as_of)
            .expect("yf")
            .maturity_years;

        let bond_360 = crate::instruments::fixed_income::bond::Bond::builder()
            .id("B360".into())
            .notional(Money::from((100_i64, Currency::USD)))
            .issue_date(as_of)
            .maturity(maturity)
            .cashflow_spec(
                crate::instruments::fixed_income::bond::CashflowSpec::fixed(
                    0.05,
                    finstack_quant_core::dates::Tenor::semi_annual(),
                    DayCount::Thirty360,
                )
                .expect("spec"),
            )
            .discount_curve_id("USD-OIS".into())
            .attributes(crate::instruments::Attributes::new())
            .build()
            .expect("bond");
        let y_360 = bond_model_schedule(&bond_360, as_of)
            .expect("yf")
            .maturity_years;

        assert!(y_365 > 0.0);
        assert!(y_360 > 0.0);
        assert!(
            (y_365 - y_360).abs() > 1e-6,
            "Act365F vs Thirty360 should differ: {y_365} vs {y_360}"
        );
    }
}
