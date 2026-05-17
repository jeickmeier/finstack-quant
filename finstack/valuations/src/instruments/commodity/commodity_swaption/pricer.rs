//! Commodity swaption pricer engine.
//!
//! Provides deterministic PV for `CommoditySwaption` using Black-76
//! on the forward swap rate with annuity-based discounting.

use crate::instruments::commodity::commodity_swaption::CommoditySwaption;
use crate::instruments::common_impl::traits::Instrument;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, PricingResult,
};
use crate::results::ValuationResult;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;

/// Commodity swaption pricer using Black-76 on the forward swap rate.
pub struct CommoditySwaptionBlackPricer {
    model: ModelKey,
}

impl CommoditySwaptionBlackPricer {
    /// Create a new commodity swaption pricer with Black-76 model key.
    pub fn new() -> Self {
        Self {
            model: ModelKey::Black76,
        }
    }

    /// Create a pricer with a specific model key.
    pub fn with_model(model: ModelKey) -> Self {
        Self { model }
    }
}

impl Default for CommoditySwaptionBlackPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for CommoditySwaptionBlackPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::CommoditySwaption, self.model)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> PricingResult<ValuationResult> {
        let swaption = instrument
            .as_any()
            .downcast_ref::<CommoditySwaption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::CommoditySwaption, instrument.key())
            })?;

        let pv = swaption.value(market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        Ok(ValuationResult::stamped(swaption.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::parameters::CommodityUnderlyingParams;
    use crate::instruments::common_impl::traits::Attributes;
    use crate::instruments::OptionType;
    use crate::pricer::Pricer;
    use finstack_core::currency::Currency;
    use finstack_core::dates::{DayCount, Tenor, TenorUnit};
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::{DiscountCurve, PriceCurve};
    use finstack_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    fn flat_vol_surface(id: &str, vol: f64) -> VolSurface {
        let expiries = [0.25, 0.5, 1.0, 2.0];
        let strikes = [2.0, 3.0, 3.5, 4.0, 5.0];
        let mut builder = VolSurface::builder(id)
            .expiries(&expiries)
            .strikes(&strikes);
        for _ in &expiries {
            builder = builder.row(&vec![vol; strikes.len()]);
        }
        builder.build().expect("vol surface should build in tests")
    }

    fn build_market(as_of: Date, flat_fwd: f64, vol: f64, rate: f64) -> MarketContext {
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, (-rate * 5.0).exp())])
            .build()
            .expect("discount curve");

        let price_curve = PriceCurve::builder("NG-FORWARD")
            .base_date(as_of)
            .spot_price(flat_fwd)
            .knots([(0.0, flat_fwd), (2.0, flat_fwd)])
            .build()
            .expect("price curve");

        MarketContext::new()
            .insert(disc)
            .insert(price_curve)
            .insert_surface(flat_vol_surface("NG-VOL", vol))
    }

    fn base_swaption(option_type: OptionType, fixed_price: f64) -> CommoditySwaption {
        CommoditySwaption::builder()
            .id(InstrumentId::new("TEST-SWAPTION"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .option_type(option_type)
            .expiry(date(2025, 6, 15))
            .swap_start(date(2025, 7, 1))
            .swap_end(date(2026, 6, 30))
            .swap_frequency(Tenor::new(1, TenorUnit::Months))
            .fixed_price(fixed_price)
            .notional(10000.0)
            .forward_curve_id(CurveId::new("NG-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("NG-VOL"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("should build")
    }

    #[test]
    fn test_pricer_key() {
        let pricer = CommoditySwaptionBlackPricer::new();
        let key = pricer.key();
        assert_eq!(key.instrument, InstrumentType::CommoditySwaption);
        assert_eq!(key.model, ModelKey::Black76);
    }

    #[test]
    fn test_atm_swaption_price_positive() {
        let as_of = date(2025, 1, 2);
        let fwd = 3.50;
        let market = build_market(as_of, fwd, 0.30, 0.05);

        // ATM: fixed_price = forward
        let swaption = base_swaption(OptionType::Call, fwd);
        let pricer = CommoditySwaptionBlackPricer::new();
        let result = pricer
            .price_dyn(&swaption, &market, as_of)
            .expect("pricing should succeed");

        assert!(
            result.value.amount() > 0.0,
            "ATM swaption should have positive value, got {}",
            result.value.amount()
        );
    }

    #[test]
    fn test_deep_itm_call_approaches_intrinsic() {
        let as_of = date(2025, 1, 2);
        let fwd = 5.00;
        let market = build_market(as_of, fwd, 0.30, 0.05);

        // Deep ITM call: strike << forward
        let swaption = base_swaption(OptionType::Call, 2.00);

        let pv = swaption
            .value(&market, as_of)
            .expect("pricing should succeed");

        // Compute intrinsic ~ annuity * (F - K) * notional
        let annuity = swaption.annuity(&market, as_of).expect("annuity");
        let intrinsic = (fwd - 2.00) * annuity * swaption.notional;

        assert!(
            pv.amount() >= intrinsic * 0.95,
            "Deep ITM call PV ({}) should be near intrinsic ({})",
            pv.amount(),
            intrinsic
        );
    }

    #[test]
    fn test_put_call_parity() {
        // Put-call parity: C - P = annuity * (F - K) * notional
        let as_of = date(2025, 1, 2);
        let fwd = 3.50;
        let strike = 3.30;
        let market = build_market(as_of, fwd, 0.30, 0.05);

        let call = base_swaption(OptionType::Call, strike);
        let put = base_swaption(OptionType::Put, strike);

        let call_pv = call
            .value(&market, as_of)
            .expect("call pricing should succeed")
            .amount();
        let put_pv = put
            .value(&market, as_of)
            .expect("put pricing should succeed")
            .amount();

        let annuity = call.annuity(&market, as_of).expect("annuity");
        let forward = call.forward_swap_rate(&market, as_of).expect("forward");
        let parity_rhs = annuity * (forward - strike) * call.notional;

        let diff = (call_pv - put_pv) - parity_rhs;
        assert!(
            diff.abs() < 1.0,
            "Put-call parity violated: C-P={}, annuity*(F-K)*N={}, diff={}",
            call_pv - put_pv,
            parity_rhs,
            diff
        );
    }

    #[test]
    fn test_zero_vol_gives_intrinsic() {
        let as_of = date(2025, 1, 2);
        let fwd = 4.00;
        let strike = 3.50;

        // Use pricing override for zero vol to bypass vol surface
        let mut swaption = base_swaption(OptionType::Call, strike);
        swaption.pricing_overrides.market_quotes.implied_volatility = Some(0.0);

        let market = build_market(as_of, fwd, 0.30, 0.05);

        let pv = swaption
            .value(&market, as_of)
            .expect("pricing should succeed");

        let annuity = swaption.annuity(&market, as_of).expect("annuity");
        let forward = swaption.forward_swap_rate(&market, as_of).expect("forward");
        let expected_intrinsic = (forward - strike).max(0.0) * annuity * swaption.notional;

        assert!(
            (pv.amount() - expected_intrinsic).abs() < 0.01,
            "Zero vol call should equal intrinsic: got {}, expected {}",
            pv.amount(),
            expected_intrinsic
        );
    }

    #[test]
    fn test_zero_vol_otm_gives_zero() {
        let as_of = date(2025, 1, 2);
        let fwd = 3.00;
        let strike = 4.00;

        let mut swaption = base_swaption(OptionType::Call, strike);
        swaption.pricing_overrides.market_quotes.implied_volatility = Some(0.0);

        let market = build_market(as_of, fwd, 0.30, 0.05);

        let pv = swaption
            .value(&market, as_of)
            .expect("pricing should succeed");

        assert!(
            pv.amount().abs() < 0.01,
            "OTM call with zero vol should be ~0, got {}",
            pv.amount()
        );
    }
}
