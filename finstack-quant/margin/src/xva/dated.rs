//! Curve admission and normalization for dated valuation-adjustment grids.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};

pub(super) fn discount_factors(
    origin: Date,
    currency: Currency,
    dates: &[Date],
    curve: &DiscountCurve,
) -> finstack_quant_core::Result<Vec<f64>> {
    if curve.base_date() > origin
        || curve
            .rate_calibration()
            .is_some_and(|recipe| recipe.currency != currency)
    {
        return Err(finstack_quant_core::Error::Validation(
            "XVA discount curve has an incompatible currency or begins after the valuation cut"
                .into(),
        ));
    }
    let anchor = curve.df_on_date(origin)?;
    if !anchor.is_finite() || anchor <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "XVA discount factor at the valuation cut must be finite and positive".into(),
        ));
    }
    dates
        .iter()
        .map(|&date| {
            let factor = curve.df_on_date(date)? / anchor;
            if !factor.is_finite() || factor <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "XVA discount factor on {date} is outside the positive finite domain"
                )));
            }
            Ok(factor)
        })
        .collect()
}

pub(super) fn survival_factors(
    origin: Date,
    currency: Currency,
    dates: &[Date],
    curve: Option<&HazardCurve>,
) -> finstack_quant_core::Result<Vec<f64>> {
    let Some(curve) = curve else {
        return Ok(vec![1.0; dates.len()]);
    };
    if curve.base_date() > origin || curve.currency().is_some_and(|ccy| ccy != currency) {
        return Err(finstack_quant_core::Error::Validation(
            "XVA survival curve has an incompatible currency or begins after the valuation cut"
                .into(),
        ));
    }
    let anchor = curve.sp_on_date(origin)?;
    if !anchor.is_finite() || anchor <= 0.0 || anchor > 1.0 {
        return Err(finstack_quant_core::Error::Validation(
            "XVA survival at the valuation cut must be in (0, 1]".into(),
        ));
    }
    let mut previous = 1.0;
    dates
        .iter()
        .map(|&date| {
            let factor = curve.sp_on_date(date)? / anchor;
            if !(0.0..=previous).contains(&factor) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "XVA survival on {date} must be finite, in [0, 1] and nonincreasing"
                )));
            }
            previous = factor;
            Ok(factor)
        })
        .collect()
}
