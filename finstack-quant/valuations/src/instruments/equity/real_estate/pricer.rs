//! Real estate asset pricer implementation.

use super::RealEstateAsset;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Error as CoreError;

pub(crate) fn compute_pv(
    asset: &RealEstateAsset,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    if let Some(appraisal) = &asset.appraisal_value {
        if appraisal.currency() != asset.currency {
            return Err(CoreError::Validation(format!(
                "Appraisal currency {} does not match instrument currency {}",
                appraisal.currency(),
                asset.currency
            )));
        }
        return Ok(*appraisal);
    }

    let value = match asset.valuation_method {
        super::RealEstateValuationMethod::Dcf => compute_npv_dcf(asset, market, as_of)?,
        super::RealEstateValuationMethod::DirectCap => compute_npv_direct_cap(asset, as_of)?,
    };

    Ok(Money::new(value, asset.currency))
}

/// Asset value with the risk-free component of the discount rate bumped per
/// tenor .
///
/// Appraisal-value and direct-cap valuations carry no discount-rate input,
/// so they are constant under the bump (zero rf DV01).
pub(crate) fn pv_with_rf_bump(
    asset: &RealEstateAsset,
    as_of: Date,
    bump_at: &dyn Fn(f64) -> f64,
) -> finstack_quant_core::Result<f64> {
    if let Some(appraisal) = &asset.appraisal_value {
        if appraisal.currency() != asset.currency {
            return Err(CoreError::Validation(format!(
                "Appraisal currency {} does not match instrument currency {}",
                appraisal.currency(),
                asset.currency
            )));
        }
        return Ok(appraisal.amount());
    }
    match asset.valuation_method {
        super::RealEstateValuationMethod::Dcf => pv_with_rf_bump_dcf(asset, as_of, bump_at),
        super::RealEstateValuationMethod::DirectCap => compute_npv_direct_cap(asset, as_of),
    }
}

/// DCF NPV at the property discount rate.
///
/// Always discounts at the asset's own `discount_rate` via `(1 + r)^{-t}`
/// (the previous behavior silently switched to
/// risk-free curve discounting whenever `discount_curve_id` was loaded in
/// the market context). The PV does not depend on the market context. Rate
/// sensitivity comes from bumping the risk-free component inside the rate
/// (see [`pv_with_rf_bump`]).
pub(crate) fn compute_npv_dcf(
    asset: &RealEstateAsset,
    _market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    pv_with_rf_bump_dcf(asset, as_of, &|_| 0.0)
}

/// DCF NPV with the risk-free component of the discount rate bumped by
/// `bump_at(t)` (absolute, decimal) at each cashflow tenor `t` in years.
///
/// `bump_at = |_| 0.0` reproduces the unbumped NPV exactly. The terminal cap
/// rate is *not* bumped; it has its own sensitivity metric
/// (`real_estate::cap_rate_sensitivity`).
pub(crate) fn pv_with_rf_bump_dcf(
    asset: &RealEstateAsset,
    as_of: Date,
    bump_at: &dyn Fn(f64) -> f64,
) -> finstack_quant_core::Result<f64> {
    let rate = asset
        .discount_rate
        .ok_or_else(|| CoreError::Validation("Missing discount_rate for DCF".into()))?;
    if rate <= -1.0 {
        return Err(CoreError::Validation(
            "discount_rate must be greater than -100%".into(),
        ));
    }

    let horizon = if let Some(sale_date) = asset.sale_date {
        if sale_date < as_of {
            return Err(CoreError::Validation(
                "sale_date must be on/after as_of".into(),
            ));
        }
        sale_date
    } else {
        last_noi(asset, as_of)?.0
    };

    let flows = future_unlevered_flows(asset, as_of)?
        .into_iter()
        .filter(|(date, _)| *date <= horizon)
        .collect::<Vec<_>>();

    let terminal_at_horizon = sale_proceeds_at(asset, as_of, horizon)?;
    if flows.is_empty() && terminal_at_horizon.is_none() {
        return Err(CoreError::Validation(
            "No cashflows on/before horizon date and no terminal proceeds configured".into(),
        ));
    }

    let pv_acq_cost = acquisition_cost_total(asset)?;

    let pv_flows: f64 = flows
        .iter()
        .map(|(date, amount)| {
            let t = year_fraction(asset, as_of, *date)?;
            Ok(amount / (1.0 + rate + bump_at(t)).powf(t))
        })
        .collect::<finstack_quant_core::Result<Vec<f64>>>()?
        .into_iter()
        .sum();

    let pv_terminal = match terminal_at_horizon {
        Some((date, amount)) => {
            let t = year_fraction(asset, as_of, date)?;
            amount / (1.0 + rate + bump_at(t)).powf(t)
        }
        None => 0.0,
    };

    Ok(pv_flows + pv_terminal - pv_acq_cost)
}

pub(crate) fn compute_npv_direct_cap(
    asset: &RealEstateAsset,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    let cap_rate = asset
        .cap_rate
        .ok_or_else(|| CoreError::Validation("Missing cap_rate for direct cap".into()))?;
    if cap_rate <= 0.0 {
        return Err(CoreError::Validation("cap_rate must be positive".into()));
    }

    let noi = if let Some(noi) = asset.stabilized_noi {
        noi
    } else {
        future_noi_flows(asset, as_of)?
            .first()
            .map(|(_, amount)| *amount)
            .ok_or_else(|| CoreError::Validation("NOI schedule is empty".into()))?
    };

    Ok(noi / cap_rate)
}

pub(crate) fn future_noi_flows(
    asset: &RealEstateAsset,
    as_of: Date,
) -> finstack_quant_core::Result<Vec<(Date, f64)>> {
    let mut flows: Vec<(Date, f64)> = asset
        .noi_schedule
        .iter()
        .copied()
        .filter(|(date, _)| *date >= as_of)
        .collect();
    if flows.is_empty() {
        return Err(CoreError::Validation(
            "NOI schedule must include at least one flow on/after as_of".into(),
        ));
    }
    flows.sort_by_key(|(date, _)| *date);
    Ok(flows)
}

pub(crate) fn future_unlevered_flows(
    asset: &RealEstateAsset,
    as_of: Date,
) -> finstack_quant_core::Result<Vec<(Date, f64)>> {
    let mut noi = future_noi_flows(asset, as_of)?;
    let mut capex: Vec<(Date, f64)> = asset
        .capex_schedule
        .as_ref()
        .map(|schedule| {
            schedule
                .iter()
                .copied()
                .filter(|(date, _)| *date >= as_of)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    capex.sort_by_key(|(date, _)| *date);

    noi.extend(capex.into_iter().map(|(date, amount)| (date, -amount)));
    noi.sort_by_key(|(date, _)| *date);

    let mut merged: Vec<(Date, f64)> = Vec::with_capacity(noi.len());
    for (date, amount) in noi {
        if let Some((last_date, last_amount)) = merged.last_mut() {
            if *last_date == date {
                *last_amount += amount;
                continue;
            }
        }
        merged.push((date, amount));
    }
    Ok(merged)
}

pub(crate) fn acquisition_cost_total(asset: &RealEstateAsset) -> finstack_quant_core::Result<f64> {
    let mut total = asset.acquisition_cost.unwrap_or(0.0);
    for money in &asset.acquisition_costs {
        if money.currency() != asset.currency {
            return Err(CoreError::Validation(
                "acquisition_costs currency must match instrument currency".into(),
            ));
        }
        total += money.amount();
    }
    Ok(total)
}

pub(crate) fn disposition_cost_total(asset: &RealEstateAsset) -> finstack_quant_core::Result<f64> {
    let mut total = 0.0;
    for money in &asset.disposition_costs {
        if money.currency() != asset.currency {
            return Err(CoreError::Validation(
                "disposition_costs currency must match instrument currency".into(),
            ));
        }
        total += money.amount();
    }
    Ok(total)
}

pub(crate) fn sale_proceeds_at(
    asset: &RealEstateAsset,
    as_of: Date,
    exit_date: Date,
) -> finstack_quant_core::Result<Option<(Date, f64)>> {
    if exit_date < as_of {
        return Err(CoreError::Validation(
            "exit_date must be on/after as_of".into(),
        ));
    }

    let gross = if let Some(sale_price) = asset.sale_price {
        if sale_price.currency() != asset.currency {
            return Err(CoreError::Validation(
                "sale_price currency must match instrument currency".into(),
            ));
        }
        sale_price.amount()
    } else if let Some(cap_rate) = asset.terminal_cap_rate {
        if cap_rate <= 0.0 {
            return Err(CoreError::Validation(
                "terminal_cap_rate must be positive".into(),
            ));
        }
        let terminal_noi_n = future_noi_flows(asset, as_of)?
            .iter()
            .copied()
            .rfind(|(date, _)| *date <= exit_date)
            .map(|(_, amount)| amount)
            .ok_or_else(|| {
                CoreError::Validation("No NOI on/before exit_date for terminal value".into())
            })?;
        let growth = asset.terminal_growth_rate.unwrap_or(0.0);
        if !(-1.0..=0.20).contains(&growth) {
            return Err(CoreError::Validation(format!(
                "terminal_growth_rate must be in [-100%, 20%], got {growth}"
            )));
        }
        let terminal_noi_n1 = terminal_noi_n * (1.0 + growth);
        terminal_noi_n1 / cap_rate
    } else {
        return Ok(None);
    };

    let mut net = gross;
    if let Some(pct) = asset.disposition_cost_pct {
        if !(0.0..1.0).contains(&pct) {
            return Err(CoreError::Validation(
                "disposition_cost_pct must be in [0, 1)".into(),
            ));
        }
        net *= 1.0 - pct;
    }
    net -= disposition_cost_total(asset)?;

    Ok(Some((exit_date, net)))
}

pub(crate) fn first_noi(asset: &RealEstateAsset, as_of: Date) -> finstack_quant_core::Result<f64> {
    future_noi_flows(asset, as_of)?
        .first()
        .map(|(_, amount)| *amount)
        .ok_or_else(|| CoreError::Validation("NOI schedule is empty".into()))
}

pub(crate) fn last_noi(
    asset: &RealEstateAsset,
    as_of: Date,
) -> finstack_quant_core::Result<(Date, f64)> {
    future_noi_flows(asset, as_of)?
        .last()
        .copied()
        .ok_or_else(|| CoreError::Validation("NOI schedule is empty".into()))
}

pub(crate) fn unlevered_flows(
    asset: &RealEstateAsset,
    as_of: Date,
) -> finstack_quant_core::Result<Vec<(Date, f64)>> {
    future_unlevered_flows(asset, as_of)
}

pub(crate) fn noi_flows(
    asset: &RealEstateAsset,
    as_of: Date,
) -> finstack_quant_core::Result<Vec<(Date, f64)>> {
    future_noi_flows(asset, as_of)
}

pub(crate) fn terminal_sale_proceeds(
    asset: &RealEstateAsset,
    as_of: Date,
) -> finstack_quant_core::Result<Option<(Date, f64)>> {
    let terminal_date = asset.sale_date.unwrap_or(last_noi(asset, as_of)?.0);
    sale_proceeds_at(asset, as_of, terminal_date)
}

fn year_fraction(
    asset: &RealEstateAsset,
    start: Date,
    end: Date,
) -> finstack_quant_core::Result<f64> {
    asset
        .day_count
        .year_fraction(start, end, DayCountContext::default())
}

#[cfg(test)]
mod tests {
    use super::super::RealEstateValuationMethod;
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Result<Date, Box<dyn std::error::Error>> {
        let m = Month::try_from(month)?;
        Ok(Date::from_calendar_date(year, m, day)?)
    }

    #[test]
    fn compute_pv_prices_dcf_cashflows_and_terminal_value() -> Result<(), Box<dyn std::error::Error>>
    {
        let valuation_date = date(2025, 1, 1)?;
        let noi1 = date(2026, 1, 1)?;
        let noi2 = date(2027, 1, 1)?;

        let asset = RealEstateAsset::builder()
            .id(InstrumentId::new("RE-DCF-UNIT"))
            .currency(Currency::USD)
            .valuation_date(valuation_date)
            .valuation_method(RealEstateValuationMethod::Dcf)
            .noi_schedule(vec![(noi1, 100.0), (noi2, 100.0)])
            .discount_rate_opt(Some(0.10))
            .terminal_cap_rate_opt(Some(0.08))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Default::default())
            .build()?;

        let market = MarketContext::new();
        let pv = compute_pv(&asset, &market, valuation_date)?;

        let t1 =
            DayCount::Act365F.year_fraction(valuation_date, noi1, DayCountContext::default())?;
        let t2 =
            DayCount::Act365F.year_fraction(valuation_date, noi2, DayCountContext::default())?;
        let expected = 100.0 / (1.0_f64 + 0.10).powf(t1)
            + 100.0 / (1.0_f64 + 0.10).powf(t2)
            + (100.0 / 0.08) / (1.0_f64 + 0.10).powf(t2);

        assert!((pv.amount() - expected).abs() < 0.01);
        assert_eq!(pv.currency(), Currency::USD);
        Ok(())
    }
}
