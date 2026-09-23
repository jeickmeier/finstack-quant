//! Time-basis helpers for bond pricing (model maturity vs discount-curve dates).
//!
//! Bond Monte Carlo and structural models use the cashflow spec day count for
//! simulation time, while discount factors must be taken from the discount
//! curve's own base date and day count via date-based helpers.

use crate::instruments::fixed_income::bond::pricing::quote_conversions::icma_reference_period;
use crate::instruments::fixed_income::bond::types::Bond;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::Result;

/// Remaining contractual coupon grid of a bond on the bond model clock.
///
/// Model time is the bond coupon day-count year fraction from `as_of`. For
/// ACT/ACT (ICMA) the day-count context carries the coupon frequency and the
/// quasi-coupon period surrounding `as_of`, as the yield engines do.
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
            if as_of >= rule.ex_date(cf.date)? {
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
    let horizon = coupon_flows
        .last()
        .map_or(bond.maturity, |&(d, _)| d.max(bond.maturity));
    let maturity_years = day_count.year_fraction(as_of, horizon, dc_ctx)?;
    let coupons = coupon_flows
        .into_iter()
        .map(|(date, accrual)| Ok((day_count.year_fraction(as_of, date, dc_ctx)?, accrual, date)))
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
