//! Agency TBA pricing.
//!
//! TBA pricing uses assumed pool characteristics to project cashflows
//! and discount them to present value, then compares to the trade price.

use super::AgencyTba;
use crate::cashflow::builder::specs::PrepaymentModelSpec;
use crate::instruments::fixed_income::mbs_passthrough::{
    pricer::price_mbs, AgencyMbsPassthrough, AgencyProgram, PoolType,
};
use crate::instruments::fixed_income::tba::allocation::assumed_pool_assumptions;
use finstack_quant_core::dates::{Date, DateExt, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_core::Result;

/// Create assumed pool for TBA valuation.
///
/// Uses standard assumptions for generic pool characteristics based on
/// the TBA's agency, coupon, and term.
pub(crate) fn create_assumed_pool(tba: &AgencyTba, _as_of: Date) -> Result<AgencyMbsPassthrough> {
    let settlement_date = tba.get_settlement_date()?;
    let term_months = tba.term.months();
    let defaults = assumed_pool_assumptions()?;

    // Use provided pool factor or default to 1.0 (newly issued)
    let factor = tba.pool_factor.unwrap_or(defaults.default_pool_factor);
    let maturity = settlement_date.add_months(term_months as i32);

    // Standard servicing and g-fee assumptions
    let servicing_fee = defaults.servicing_fee_rate;
    let guarantee_fee = match tba.agency {
        AgencyProgram::Gnma | AgencyProgram::GnmaI | AgencyProgram::GnmaII => {
            defaults.gnma_guarantee_fee_rate
        }
        _ => defaults.agency_guarantee_fee_rate,
    };

    // WAC = pass-through + fees
    let wac = tba.coupon + servicing_fee + guarantee_fee;

    AgencyMbsPassthrough::builder()
        .id(InstrumentId::new(format!("{}-ASSUMED", tba.id.as_str())))
        .pool_id(format!("{}-POOL", tba.id.as_str()).into())
        .agency(tba.agency)
        .pool_type(PoolType::Generic)
        .original_face(tba.notional)
        .current_face(Money::new(
            tba.notional.amount() * factor,
            tba.notional.currency(),
        ))
        .current_factor(factor)
        .wac(wac)
        .pass_through_rate(tba.coupon)
        .servicing_fee_rate(servicing_fee)
        .guarantee_fee_rate(guarantee_fee)
        .wam(term_months)
        .issue_date(settlement_date)
        .maturity(maturity)
        .prepayment_model(PrepaymentModelSpec::psa(defaults.psa_multiplier))
        .discount_curve_id(tba.discount_curve_id.clone())
        .day_count(DayCount::Thirty360)
        .build()
}

/// Resolve the assumed pool used as the canonical projected-collateral source.
pub(crate) fn resolve_assumed_pool(tba: &AgencyTba, as_of: Date) -> Result<AgencyMbsPassthrough> {
    if let Some(ref pool) = tba.assumed_pool {
        Ok(pool.as_ref().clone())
    } else {
        create_assumed_pool(tba, as_of)
    }
}

/// Price a TBA forward.
///
/// Calculates the value as the difference between the forward price
/// of the assumed pool and the trade price, discounted to valuation date.
///
/// # Arguments
///
/// * `tba` - TBA forward instrument
/// * `market` - Market context with discount curves
/// * `as_of` - Valuation date
pub(crate) fn price_tba(tba: &AgencyTba, market: &MarketContext, as_of: Date) -> Result<Money> {
    let settlement_date = tba.get_settlement_date()?;
    let assumed_pool = resolve_assumed_pool(tba, as_of)?;

    // Price the assumed pool
    let pool_pv = price_mbs(&assumed_pool, market, as_of)?;

    // For settled TBAs (settlement_date <= as_of), compute the realized P&L
    // as the net position value: delivered pool value minus trade cost.
    // The discount factor is 1.0 since settlement has already occurred.
    // For unsettled TBAs, discount the trade value back to the valuation date.
    let discount_curve = market.get_discount(&tba.discount_curve_id)?;
    let df_to_settle = if settlement_date <= as_of {
        1.0
    } else {
        discount_curve.df_between_dates(as_of, settlement_date)?
    };

    // Trade value at settlement = notional × trade_price / 100
    let trade_value_at_settle = tba.notional.amount() * tba.trade_price / 100.0;

    // PV of trade value
    let trade_pv = trade_value_at_settle * df_to_settle;

    // TBA value = Pool PV - Trade PV
    // Positive if pool is worth more than we're paying
    let value = pool_pv.amount() - trade_pv;

    Ok(Money::new(value, tba.notional.currency()))
}

/// Estimate settlement fail cost.
///
/// When a TBA trade fails to settle on the agreed date, the failing
/// party incurs a financing cost on the unsettled position.
///
/// # Formula
/// ```text
/// Fail Cost = Position Value × Fail Rate × Fail Days / 360
/// ```
///
/// # Arguments
/// * `position_value` - Current value of the unsettled position
/// * `fail_rate` - Financing rate for fails (typically Fed Funds - 300bp, floored at 0)
/// * `fail_days` - Number of days the settlement has failed
#[allow(dead_code)] // Utility available for downstream callers
pub(crate) fn estimate_fail_cost(position_value: f64, fail_rate: f64, fail_days: u32) -> f64 {
    position_value * fail_rate.max(0.0) * (fail_days as f64) / 360.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use time::Month;

    fn create_test_market(as_of: Date) -> MarketContext {
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (0.25, 0.99),
                (1.0, 0.96),
                (5.0, 0.80),
                (30.0, 0.30),
            ])
            .interp(InterpStyle::Linear)
            .build()
            .expect("valid curve");

        MarketContext::new().insert(disc)
    }

    #[test]
    fn test_create_assumed_pool() {
        let tba = AgencyTba::example().expect("AgencyTba example is valid");
        let as_of = Date::from_calendar_date(2027, Month::January, 15).expect("valid");

        let pool = create_assumed_pool(&tba, as_of).expect("should create pool");

        assert_eq!(pool.agency, tba.agency);
        assert!((pool.pass_through_rate - tba.coupon).abs() < 1e-10);
        assert!((pool.current_factor - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_resolved_pool_schedule_matches_provider_schedule() {
        let tba = AgencyTba::example().expect("AgencyTba example is valid");
        let as_of = Date::from_calendar_date(2027, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);
        let pool = resolve_assumed_pool(&tba, as_of).expect("assumed pool should resolve");
        let provider_schedule =
            crate::cashflow::traits::CashflowProvider::cashflow_schedule(&tba, &market, as_of)
                .expect("tba provider schedule");
        let pool_schedule =
            crate::cashflow::traits::CashflowProvider::cashflow_schedule(&pool, &market, as_of)
                .expect("pool provider schedule");

        assert_eq!(provider_schedule.flows.len(), pool_schedule.flows.len());
        assert_eq!(
            provider_schedule.flows.first().map(|cf| cf.kind),
            pool_schedule.flows.first().map(|cf| cf.kind)
        );
    }

    #[test]
    fn test_price_tba() {
        let tba = AgencyTba::example().expect("AgencyTba example is valid");
        let as_of = Date::from_calendar_date(2027, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        let pv = price_tba(&tba, &market, as_of).expect("should price");

        // PV should be reasonable
        assert!(pv.amount().abs() < tba.notional.amount());
    }

    #[test]
    fn test_tba_expired() {
        let mut tba = AgencyTba::example().expect("AgencyTba example is valid");
        tba.settlement_year = 2026;
        tba.settlement_month = 1;

        let as_of = Date::from_calendar_date(2027, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        let pv = price_tba(&tba, &market, as_of).expect("should price");

        // Settled TBA returns realized P&L (pool value - trade cost), not zero
        assert!(pv.amount().abs() < tba.notional.amount());
    }
}
