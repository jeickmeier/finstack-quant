//! Agency TBA pricing.
//!
//! TBA pricing uses assumed pool characteristics to project cashflows
//! and discount them to present value, then compares to the trade price.

use super::AgencyTba;
use crate::cashflow::builder::specs::PrepaymentModelSpec;
use crate::instruments::fixed_income::mbs_passthrough::{
    pricer::{price_mbs, quote_basis_pool},
    AgencyMbsPassthrough, PoolType,
};
use crate::instruments::fixed_income::tba::allocation::assumed_pool_assumptions;
use finstack_quant_core::dates::{Date, DateExt, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_core::Result;

/// Create the generic assumed pool for TBA valuation.
///
/// Uses standard assumptions for generic pool characteristics based on
/// the TBA's agency, coupon, and term, and the TBA's `prepayment_model`
/// when set (embedded generic PSA otherwise).
pub(crate) fn create_assumed_pool(tba: &AgencyTba) -> Result<AgencyMbsPassthrough> {
    let settlement_date = tba.get_settlement_date()?;
    let term_months = tba.term.months();
    let defaults = assumed_pool_assumptions()?;

    // Use provided pool factor or default to 1.0 (newly issued)
    let factor = tba.pool_factor.unwrap_or(defaults.default_pool_factor);
    if !factor.is_finite() || !(0.0..=1.0).contains(&factor) || factor == 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "AgencyTba '{}' pool_factor must be finite and within (0, 1], got {}",
            tba.id, factor
        )));
    }
    let issue_date = Date::from_calendar_date(settlement_date.year(), settlement_date.month(), 1)
        .map_err(|err| finstack_quant_core::Error::Validation(err.to_string()))?;
    let maturity = issue_date.add_months(term_months as i32);

    // Standard servicing and g-fee assumptions
    let servicing_fee = defaults.servicing_fee_rate;
    let guarantee_fee = if tba.agency.is_gnma() {
        defaults.gnma_guarantee_fee_rate
    } else {
        defaults.agency_guarantee_fee_rate
    };

    // WAC = pass-through + fees
    let wac = tba.coupon + servicing_fee + guarantee_fee;

    AgencyMbsPassthrough::builder()
        .id(InstrumentId::new(format!("{}-ASSUMED", tba.id.as_str())))
        .pool_id(format!("{}-POOL", tba.id.as_str()).into())
        .agency(tba.agency)
        .pool_type(PoolType::Generic)
        // TBA notional is the current trade face being purchased. Recover the
        // pool's original face from factor; do not scale down purchased face.
        .original_face(Money::new(
            tba.notional.amount() / factor,
            tba.notional.currency(),
        )?)
        .current_face(tba.notional)
        .current_factor(factor)
        .wac(wac)
        .pass_through_rate(tba.coupon)
        .servicing_fee_rate(servicing_fee)
        .guarantee_fee_rate(guarantee_fee)
        .wam(term_months)
        .issue_date(issue_date)
        .maturity(maturity)
        .prepayment_model(
            tba.prepayment_model
                .clone()
                .unwrap_or_else(|| PrepaymentModelSpec::psa(defaults.psa_multiplier)),
        )
        .discount_curve_id(tba.discount_curve_id.clone())
        .day_count(DayCount::Thirty360)
        .build()
}

/// Resolve the assumed pool used as the canonical projected-collateral source.
pub(crate) fn resolve_assumed_pool(tba: &AgencyTba) -> Result<AgencyMbsPassthrough> {
    if let Some(ref pool) = tba.assumed_pool {
        crate::instruments::Instrument::validate_invariants(pool.as_ref())?;
        if tba
            .pool_factor
            .is_some_and(|factor| (factor - pool.current_factor).abs() > 1e-12)
        {
            return Err(finstack_quant_core::Error::Validation(
                "TBA pool_factor must agree with the explicitly supplied pool".into(),
            ));
        }
        let settlement = tba.get_settlement_date()?;
        if pool.issue_date > settlement {
            return Err(finstack_quant_core::Error::Validation(
                "TBA delivered pool must be issued on or before settlement".into(),
            ));
        }
        // Delivery transfers the settlement-month accrual onward. Prior-month
        // P&I belongs to the seller even when its agency payment is still due.
        let mut resolved = quote_basis_pool(pool, settlement)?;
        let scale = tba.notional.amount() / resolved.current_face.amount();
        if !scale.is_finite()
            || scale <= 0.0
            || resolved.current_face.currency() != tba.notional.currency()
        {
            return Err(finstack_quant_core::Error::Validation(
                "TBA assumed pool must have positive current face in the trade currency".into(),
            ));
        }
        resolved.original_face = Money::new(
            resolved.original_face.amount() * scale,
            tba.notional.currency(),
        )?;
        resolved.current_face = tba.notional;
        Ok(resolved)
    } else {
        create_assumed_pool(tba)
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
    if as_of >= settlement_date {
        return Ok(Money::from((0_i64, tba.notional.currency())));
    }
    let assumed_pool = resolve_assumed_pool(tba)?;

    let pool_pv = price_mbs(&assumed_pool, market, settlement_date)?;

    // Before settlement, discount the contractual trade value back to the
    // valuation date. After settlement the forward is extinguished; any
    // delivered pool must be represented as a separate MBS position.
    let discount_curve = market.get_discount(&tba.discount_curve_id)?;
    let df_to_settle = discount_curve.df_between_dates(as_of, settlement_date)?;

    let trade_value_at_settle = tba.notional.amount() * tba.trade_price / 100.0
        + crate::instruments::fixed_income::mbs_passthrough::pricer::settlement_accrued_interest(
            &assumed_pool,
            settlement_date,
        )?;

    // PV of trade value
    let trade_pv = trade_value_at_settle * df_to_settle;

    // TBA value = Pool PV - Trade PV
    // Positive if pool is worth more than we're paying
    let value = pool_pv.amount() * df_to_settle - trade_pv;

    Money::new(value, tba.notional.currency())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::mbs_passthrough::AgencyProgram;
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

        let pool = create_assumed_pool(&tba).expect("should create pool");

        assert_eq!(pool.agency, tba.agency);
        assert!((pool.pass_through_rate - tba.coupon).abs() < 1e-10);
        assert!((pool.current_factor - 1.0).abs() < 1e-10);
    }

    #[test]
    fn assumed_pool_guarantee_fee_follows_agency_family() {
        let defaults = assumed_pool_assumptions().expect("embedded TBA assumptions");
        let cases = [
            (AgencyProgram::Fnma, defaults.agency_guarantee_fee_rate),
            (AgencyProgram::Fhlmc, defaults.agency_guarantee_fee_rate),
            (AgencyProgram::GnmaI, defaults.gnma_guarantee_fee_rate),
            (AgencyProgram::GnmaII, defaults.gnma_guarantee_fee_rate),
        ];

        for (agency, expected_fee) in cases {
            let mut tba = AgencyTba::example().expect("AgencyTba example is valid");
            tba.agency = agency;

            let pool = create_assumed_pool(&tba).expect("should create pool");
            assert_eq!(pool.guarantee_fee_rate, expected_fee);
            assert_eq!(
                pool.wac,
                tba.coupon + defaults.servicing_fee_rate + expected_fee
            );
        }
    }

    #[test]
    fn tba_cashflow_provider_rejects_physical_delivery_projection() {
        let tba = AgencyTba::example().expect("AgencyTba example is valid");
        let as_of = Date::from_calendar_date(2027, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);
        let err =
            crate::cashflow::traits::CashflowProvider::cashflow_schedule(&tba, &market, as_of)
                .expect_err("physical TBA delivery is not a standalone cashflow schedule");
        assert!(err.to_string().contains("physically settled forward"));
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
    fn assumed_pool_json_roundtrip_preserves_pricing_behavior() {
        let mut tba = AgencyTba::example().expect("AgencyTba example is valid");
        tba.assumed_pool = Some(Box::new(
            AgencyMbsPassthrough::example().expect("assumed pool"),
        ));
        let roundtripped: AgencyTba =
            serde_json::from_str(&serde_json::to_string(&tba).expect("serialize TBA"))
                .expect("deserialize TBA");
        let as_of = Date::from_calendar_date(2027, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        let original = price_tba(&tba, &market, as_of).expect("original price");
        let restored = price_tba(&roundtripped, &market, as_of).expect("restored price");
        assert_eq!(restored, original);
    }

    #[test]
    fn test_tba_expired() {
        let mut tba = AgencyTba::example().expect("AgencyTba example is valid");
        tba.settlement_year = 2026;
        tba.settlement_month = 1;

        let as_of = Date::from_calendar_date(2027, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        let pv = price_tba(&tba, &market, as_of).expect("should price");

        assert_eq!(pv.amount(), 0.0);
    }
}

#[cfg(test)]
mod production_mortgage_audit {
    use super::*;
    use crate::instruments::fixed_income::mbs_passthrough::pricer::generate_cashflows;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use time::macros::date;

    #[test]
    fn tba_owns_settlement_month_and_pays_settlement_accrued() {
        let mut tba = AgencyTba::example().expect("tba");
        let as_of = date!(2026 - 03 - 01);
        let settle = date!(2026 - 03 - 11);
        tba.settlement_date = Some(settle);
        let market = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (40.0, 1.0)])
                .build()
                .expect("curve"),
        );
        let pool = create_assumed_pool(&tba).expect("pool");
        assert_eq!(
            pool.issue_date, as_of,
            "assumed pool starts on issue-month boundary"
        );
        let flows = generate_cashflows(&pool, as_of, None).expect("flows");
        assert_eq!(flows[0].period_start, as_of);
        let dirty = flows.iter().map(|cf| cf.total).sum::<f64>();
        let expected =
            dirty - tba.notional.amount() * (tba.trade_price / 100.0 + tba.coupon * 10.0 / 360.0);
        let pv = price_tba(&tba, &market, as_of).expect("pv").amount();
        assert!((pv - expected).abs() < 1e-7, "pv={pv}, expected={expected}");
    }

    #[test]
    fn delivered_pool_excludes_sellers_prior_month_receivable() {
        let mut tba = AgencyTba::example().expect("tba");
        let settle = date!(2026 - 03 - 11);
        tba.settlement_date = Some(settle);
        let mut pool = create_assumed_pool(&tba).expect("pool");
        pool.issue_date = date!(2025 - 01 - 01);
        tba.assumed_pool = Some(Box::new(pool));
        let resolved = resolve_assumed_pool(&tba).expect("pool");
        let flows = generate_cashflows(&resolved, settle, Some(1)).expect("flows");
        assert_eq!(flows[0].period_start, date!(2026 - 03 - 01));
    }

    #[test]
    fn tba_assumed_pool_is_scaled_to_purchased_current_face() {
        let mut tba = AgencyTba::example().expect("tba");
        let mut pool = create_assumed_pool(&tba).expect("pool");
        pool.original_face =
            Money::new(tba.notional.amount() * 2.0 / 0.6, tba.notional.currency()).expect("money");
        pool.current_face =
            Money::new(tba.notional.amount() * 2.0, tba.notional.currency()).expect("money");
        pool.current_factor = 0.6;
        tba.assumed_pool = Some(Box::new(pool));
        let resolved = resolve_assumed_pool(&tba).expect("resolve");
        assert_eq!(resolved.current_face, tba.notional);
        assert!((resolved.original_face.amount() * 0.6 - tba.notional.amount()).abs() < 1e-8);
    }
}
