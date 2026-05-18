//! Time-basis helpers for bond pricing (model maturity vs discount-curve dates).
//!
//! Bond Monte Carlo and structural models use the cashflow spec day count for
//! simulation time, while discount factors must be taken from the discount
//! curve's own base date and day count via date-based helpers.

use crate::instruments::fixed_income::bond::types::Bond;
use finstack_core::dates::{Date, DayCountContext};
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::Result;

/// Model maturity in years from `as_of` to bond maturity using the bond cashflow day count.
pub(crate) fn bond_model_maturity_years(bond: &Bond, as_of: Date) -> Result<f64> {
    bond.cashflow_spec
        .day_count()
        .year_fraction(as_of, bond.maturity, DayCountContext::default())
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
/// `model_time` runs on the bond clock; each DF is `DF(as_of → date_at_model_time)`.
pub(crate) fn bond_cashflow_dfs_on_model_grid(
    disc: &DiscountCurve,
    as_of: Date,
    maturity: Date,
    mat_years: f64,
    steps_per_year: usize,
) -> Result<Vec<(f64, f64)>> {
    if mat_years <= 0.0 || steps_per_year == 0 {
        return Ok(Vec::new());
    }
    let n = (mat_years * steps_per_year as f64).round().max(1.0) as usize;
    let mut dfs = Vec::with_capacity(n);
    for i in 1..=n {
        let t = i as f64 / steps_per_year as f64;
        let pay_date = date_at_model_time(as_of, maturity, mat_years, t);
        let df = disc.df_between_dates(as_of, pay_date)?;
        dfs.push((t, df));
    }
    Ok(dfs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;
    use finstack_core::dates::DayCount;
    use finstack_core::money::Money;
    use time::macros::date;

    #[test]
    fn bond_model_maturity_respects_day_count() {
        let as_of = date!(2025 - 01 - 01);
        let maturity = date!(2030 - 01 - 01);
        let bond_365 = crate::instruments::fixed_income::bond::Bond::fixed(
            "B365",
            Money::new(100.0, Currency::USD),
            0.05,
            as_of,
            maturity,
            "USD-OIS",
        )
        .expect("bond");
        let y_365 = bond_model_maturity_years(&bond_365, as_of).expect("yf");

        let bond_360 = crate::instruments::fixed_income::bond::Bond::builder()
            .id("B360".into())
            .notional(Money::new(100.0, Currency::USD))
            .issue_date(as_of)
            .maturity(maturity)
            .cashflow_spec(
                crate::instruments::fixed_income::bond::CashflowSpec::fixed(
                    0.05,
                    finstack_core::dates::Tenor::semi_annual(),
                    DayCount::Thirty360,
                )
                .expect("spec"),
            )
            .discount_curve_id("USD-OIS".into())
            .attributes(crate::instruments::Attributes::new())
            .build()
            .expect("bond");
        let y_360 = bond_model_maturity_years(&bond_360, as_of).expect("yf");

        assert!(y_365 > 0.0);
        assert!(y_360 > 0.0);
        assert!(
            (y_365 - y_360).abs() > 1e-6,
            "Act365F vs Thirty360 should differ: {y_365} vs {y_360}"
        );
    }
}
