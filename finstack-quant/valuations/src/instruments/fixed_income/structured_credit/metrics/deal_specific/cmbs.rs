//! CMBS-specific metrics (LTV, DSCR).

use crate::instruments::fixed_income::structured_credit::pricing::simulation_engine::period_helpers::term_rate_for_period;
use crate::instruments::fixed_income::structured_credit::utils::amortization::{
    level_payment_per_unit, remaining_schedule_periods,
};
use crate::instruments::fixed_income::structured_credit::{DealType, PoolAsset, StructuredCredit};
use crate::metrics::MetricContext;
use finstack_quant_core::dates::{Date, DateExt};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// Annual debt service of one loan on its live terms as of `as_of`: the
/// coupon resolved through the market's forward curves for a floating row
/// (index plus spread, floored), interest only while the loan is inside its
/// interest-only window or is not amortizing, otherwise the level payment
/// over the remaining schedule (`contractual_payment` when supplied), at
/// the deal's payment frequency.
fn annual_debt_service(
    asset: &PoolAsset,
    closing_date: Date,
    as_of: Date,
    months_per_period: u32,
    market: &MarketContext,
) -> finstack_quant_core::Result<f64> {
    // A floating coupon is the index projected from the curve at `as_of`
    // (floored) plus the spread; a fixed coupon is the stored rate.
    let coupon = match asset.index_id.as_deref() {
        Some(index) => {
            let curve = market.get_forward(index)?;
            // Project the next reset: the accrual starting one reset lag after
            // `as_of` fixes on the curve rather than on a historical fixing.
            let calendar =
                crate::cashflow::builder::calendar::resolve_calendar_strict("weekends_only")?;
            let next_reset = as_of.add_business_days(curve.reset_lag(), calendar)?;
            let projected = term_rate_for_period(curve.as_ref(), market, next_reset)?;
            let floored = asset
                .index_floor
                .map_or(projected, |floor| projected.max(floor));
            (floored + asset.spread_bp.unwrap_or(0.0) / 10_000.0).max(0.0)
        }
        None => asset.rate,
    };
    let periods_per_year = 12.0 / f64::from(months_per_period.max(1));
    let origination = asset.acquisition_date.unwrap_or(closing_date);
    let age_months = if origination < as_of {
        origination.months_until(as_of)
    } else {
        0
    };
    let interest_only =
        !asset.asset_type.is_amortizing() || asset.io_months.is_some_and(|io| age_months < io);
    if let Some(payment) = asset.contractual_payment.filter(|_| !interest_only) {
        return Ok(payment.amount() * periods_per_year);
    }
    if interest_only || coupon <= 0.0 {
        return Ok(asset.balance.amount() * coupon.max(0.0));
    }
    let months_to_maturity = if as_of < asset.maturity {
        as_of.months_until(asset.maturity)
    } else {
        0
    };
    let periods = remaining_schedule_periods(
        age_months,
        asset.amortization_term_months,
        months_to_maturity,
        months_per_period,
    );
    let period_rate = coupon * f64::from(months_per_period.max(1)) / 12.0;
    let payment = asset.balance.amount() * level_payment_per_unit(period_rate, f64::from(periods));
    Ok(payment * periods_per_year)
}

/// CMBS DSCR calculator
pub struct CmbsDscrCalculator;

impl CmbsDscrCalculator {
    /// Create a new DSCR calculator.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CmbsDscrCalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::metrics::MetricCalculator for CmbsDscrCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let cmbs = context.instrument_as::<StructuredCredit>()?;

        if cmbs.deal_type != DealType::Cmbs {
            return Err(finstack_quant_core::InputError::Invalid.into());
        }

        // Loan-level NOI drives the pool DSCR when the assets carry it: debt
        // service on each loan's live terms (floating coupons through the
        // market's curves, level payments over the remaining schedule,
        // interest only inside an interest-only window), annualized.
        let months_per_period = cmbs.frequency.months().unwrap_or(12).max(1);
        let mut pool_noi = 0.0_f64;
        let mut pool_debt_service = 0.0_f64;
        let mut currency = None;
        for asset in &cmbs.pool.assets {
            let Some(noi) = asset.noi else {
                continue;
            };
            if asset.is_defaulted {
                continue;
            }
            match currency {
                None => currency = Some(noi.currency()),
                Some(existing) if existing != noi.currency() => {
                    return Err(finstack_quant_core::Error::CurrencyMismatch {
                        expected: existing,
                        actual: noi.currency(),
                    });
                }
                Some(_) => {}
            }
            pool_noi += noi.amount();
            pool_debt_service += annual_debt_service(
                asset,
                cmbs.closing_date,
                context.as_of,
                months_per_period,
                &context.curves,
            )?;
        }
        if currency.is_some() {
            if !pool_debt_service.is_finite() || pool_debt_service <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(
                    "CMBS DSCR requires positive loan debt service".to_string(),
                ));
            }
            return Ok(pool_noi / pool_debt_service);
        }

        let noi = required_money(cmbs.credit_factors.annual_noi, "annual_noi")?;
        let debt_service = required_money(
            cmbs.credit_factors.annual_debt_service,
            "annual_debt_service",
        )?;

        if noi.currency() != debt_service.currency() {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: noi.currency(),
                actual: debt_service.currency(),
            });
        }
        if !debt_service.amount().is_finite() || debt_service.amount() <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "CMBS DSCR requires positive annual_debt_service".to_string(),
            ));
        }
        if !noi.amount().is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "CMBS DSCR requires finite annual_noi".to_string(),
            ));
        }

        Ok(noi.amount() / debt_service.amount())
    }
}

fn required_money(value: Option<Money>, field: &str) -> finstack_quant_core::Result<Money> {
    value.ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!("CMBS DSCR requires credit_factors.{field}"))
    })
}
