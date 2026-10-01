//! FX barrier expiry metadata and the shared theta horizon.

use crate::instruments::test_support::date::date;
use crate::instruments::test_support::discount_forward_curves::flat_discount_with_tenor;
use crate::instruments::test_support::volatility::flat_vol_surface;
use finstack_quant_core::dates::Tenor;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_valuations::instruments::{FxBarrierOption, Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;

#[test]
fn fx_barrier_generic_theta_horizon_caps_at_expiry() -> finstack_quant_core::Result<()> {
    let as_of = date(2024, 12, 30);
    let expiry = date(2025, 1, 1);
    let market = MarketContext::new()
        .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.03, 5.0))
        .insert(flat_discount_with_tenor("EUR-OIS", as_of, 0.01, 5.0))
        .insert_surface(flat_vol_surface(
            "EURUSD-VOL",
            &[0.25, 0.5, 1.0, 2.0, 5.0],
            &[0.9, 1.0, 1.1, 1.2, 1.3],
            0.15,
        ))
        .insert_price("EURUSD-SPOT", MarketScalar::Unitless(1.10));
    let mut option = FxBarrierOption::example()?;
    option.expiry = expiry;
    option.barrier = 1.60;
    option.monitoring_start_date = Some(as_of);
    option.observed_barrier_breached = Some(false);
    option.metric_pricing_overrides.theta_period = Some(Tenor::weekly());

    assert_eq!(Instrument::expiry(&option), Some(expiry));
    assert_eq!(option.last_payment_date(&market, as_of)?, Some(expiry));
    let base_pv = option.value(&market, as_of)?.amount();
    let result = option.price_with_metrics(
        &market,
        as_of,
        &[
            MetricId::Theta,
            MetricId::ThetaCarry,
            MetricId::ThetaPeriodDays,
        ],
        PricingOptions::default(),
    )?;
    assert_eq!(result.metric(MetricId::ThetaPeriodDays), Some(2.0));
    assert_eq!(result.metric(MetricId::ThetaCarry), Some(0.0));
    // With spot at strike and an unbreached remote barrier, terminal intrinsic
    // value is zero, independently fixing theta to the lost time value.
    assert!((result.metric(MetricId::Theta).expect("theta") + base_pv).abs() < 1e-8);

    let expired = option.price_with_metrics(
        &market,
        expiry + time::Duration::days(1),
        &[MetricId::Theta, MetricId::ThetaPeriodDays],
        PricingOptions::default(),
    )?;
    assert_eq!(expired.metric(MetricId::Theta), Some(0.0));
    assert_eq!(expired.metric(MetricId::ThetaPeriodDays), Some(0.0));
    Ok(())
}
