use super::*;
use crate::cds::CreditDefaultSwap;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

#[test]
fn expected_loss_conditions_on_survival_even_below_the_old_floor() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::{context::MarketContext, term_structures::HazardCurve};
    use finstack_quant_core::money::Money;
    use std::sync::Arc;
    use time::macros::date;

    let base = date!(1900 - 01 - 01);
    let as_of = date!(2025 - 01 - 01);
    let end = date!(2026 - 01 - 01);
    let mut cds = CreditDefaultSwap::example().expect("example");
    cds.premium_leg.end = end;
    cds.notional = Money::new(10_000_000.0, Currency::USD).expect("notional");
    cds.protection_leg.recovery_rate = 0.4;
    for tail_hazard in [0.0, 0.02, 10.0] {
        let hazard = HazardCurve::builder(cds.protection_leg.credit_curve_id.clone())
            .base_date(base)
            .knots([(100.0, 7.0), (101.0, tail_hazard)])
            .recovery_rate(0.4)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .build()
            .expect("curve");
        assert!(hazard.sp_on_date(as_of).expect("survival") < 1e-12);
        let market = Arc::new(
            MarketContext::new()
                .try_insert(hazard)
                .expect("insert market curve"),
        );
        let mut context = MetricContext::new(
            Arc::new(cds.clone()),
            Arc::clone(&market),
            as_of,
            Money::new(0.0, Currency::USD).expect("mark"),
            MetricContext::default_config(),
        );
        let expected = 6_000_000.0 * -(-tail_hazard).exp_m1();
        let actual = ExpectedLossCalculator.calculate(&mut context).expect("EL");
        assert!((actual - expected).abs() < 1e-8);
        let mut matured = MetricContext::new(
            Arc::new(cds.clone()),
            market,
            end,
            Money::new(0.0, Currency::USD).expect("mark"),
            MetricContext::default_config(),
        );
        assert_eq!(
            ExpectedLossCalculator
                .calculate(&mut matured)
                .expect("matured"),
            0.0
        );
    }
}

#[test]
fn cds_quote_override_returns_provider_contract() {
    let mut cds = CreditDefaultSwap::example().expect("example");
    cds.instrument_pricing_overrides.market_quotes.cds_quote_bp = Some(321.0);
    let request = deal_quote_override(&cds).expect("deal quote override");
    assert_eq!(request.contract_end, cds.premium_leg.end);
    assert_eq!(request.spread_bp, 321.0);
}
