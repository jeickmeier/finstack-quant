//! Rough Heston semi-analytical pricer via Fourier inversion.
//!
//! Uses the fractional Riccati solver from `finstack_core::math::volatility::rough_heston`
//! to price European equity options under the rough Heston model (El Euch & Rosenbaum 2019).
//! Model parameters are sourced from required market scalars.

use super::pricer::{
    collect_inputs_extended, option_currency, reject_future_discrete_dividends_for_stochastic_vol,
};
use super::types::EquityOption;
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::common_impl::traits::Instrument;
use crate::pricer::PricingError;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;

/// Equity option rough Heston semi-analytical pricer (Fourier inversion).
///
/// Prices European options by solving the fractional Riccati ODE for the
/// characteristic function and performing numerical Fourier inversion via the
/// Lewis (2000) single-integral formula.
///
/// Rough Heston parameters are read from required market scalars:
///
/// | Scalar Key | Description |
/// |---|---|
/// | `ROUGH_HESTON_V0` | Initial variance |
/// | `ROUGH_HESTON_KAPPA` | Mean reversion speed |
/// | `ROUGH_HESTON_THETA` | Long-run variance |
/// | `ROUGH_HESTON_SIGMA_V` | Vol-of-vol |
/// | `ROUGH_HESTON_RHO` | Spot-vol correlation |
/// | `ROUGH_HESTON_HURST` | Hurst exponent |
pub(crate) struct EquityOptionRoughHestonFourierPricer;

impl EquityOptionRoughHestonFourierPricer {
    /// Create a new rough Heston Fourier pricer.
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for EquityOptionRoughHestonFourierPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::pricer::Pricer for EquityOptionRoughHestonFourierPricer {
    fn key(&self) -> crate::pricer::PricerKey {
        crate::pricer::PricerKey::new(
            crate::pricer::InstrumentType::EquityOption,
            crate::pricer::ModelKey::RoughHestonFourier,
        )
    }

    #[tracing::instrument(
        name = "equity_option.rough_heston_fourier.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn crate::instruments::common_impl::traits::Instrument,
        market: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> std::result::Result<crate::results::ValuationResult, PricingError> {
        let equity_option = instrument
            .as_any()
            .downcast_ref::<EquityOption>()
            .ok_or_else(|| {
                crate::pricer::PricingError::type_mismatch(
                    crate::pricer::InstrumentType::EquityOption,
                    instrument.key(),
                )
            })?;

        reject_future_discrete_dividends_for_stochastic_vol(
            equity_option,
            as_of,
            crate::pricer::ModelKey::RoughHestonFourier,
            "rough Heston Fourier",
        )?;

        let inputs = collect_inputs_extended(equity_option, market, as_of).map_err(|e| {
            crate::pricer::PricingError::model_failure_with_context(
                e.to_string(),
                crate::pricer::PricingErrorContext::from_instrument(equity_option)
                    .model(crate::pricer::ModelKey::RoughHestonFourier),
            )
        })?;
        let (spot, r, q, _sigma, t) = (inputs.spot, inputs.r, inputs.q, inputs.sigma, inputs.t_vol);

        if t <= 0.0 {
            let intrinsic = match equity_option.option_type {
                OptionType::Call => (spot - equity_option.strike).max(0.0),
                OptionType::Put => (equity_option.strike - spot).max(0.0),
            };
            return Ok(crate::results::ValuationResult::stamped(
                equity_option.id(),
                as_of,
                Money::new(
                    intrinsic * equity_option.notional.amount(),
                    option_currency(equity_option),
                ),
            ));
        }

        let err_ctx = crate::pricer::PricingErrorContext::from_instrument(equity_option)
            .model(crate::pricer::ModelKey::RoughHestonFourier);

        // Source production rough-Heston parameters from explicit market scalars.
        let s = crate::instruments::equity::equity_option::rough_heston_market::RoughHestonScalars::from_market_strict(market)
            .map_err(|e| crate::pricer::PricingError::from_core(e, err_ctx.clone()))?;

        let params = finstack_core::math::volatility::rough_heston::RoughHestonFourierParams::new(
            s.v0, s.kappa, s.theta, s.sigma_v, s.rho, s.hurst,
        )
        .map_err(|e| crate::pricer::PricingError::from_core(e, err_ctx))?;

        let is_call = matches!(equity_option.option_type, OptionType::Call);
        let price = params.price_european(spot, equity_option.strike, r, q, t, is_call);

        let pv = Money::new(
            price * equity_option.notional.amount(),
            option_currency(equity_option),
        );
        Ok(crate::results::ValuationResult::stamped(
            equity_option.id(),
            as_of,
            pv,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::parameters::ExerciseStyle;
    use crate::instruments::{Attributes, PricingOverrides, SettlementType};
    use crate::pricer::Pricer;
    use finstack_core::currency::Currency;
    use finstack_core::dates::{Date, DayCount};
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("month"), day)
            .expect("valid date")
    }

    fn market(as_of: Date) -> MarketContext {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (10.0, (-0.03_f64 * 10.0).exp())])
            .build()
            .expect("curve");
        let surface = VolSurface::builder("SPX-VOL")
            .expiries(&[1.0])
            .strikes(&[100.0])
            .row(&[0.20])
            .build()
            .expect("surface");
        MarketContext::new()
            .insert(curve)
            .insert_surface(surface)
            .insert_price("SPX-SPOT", MarketScalar::Unitless(100.0))
            .insert_price("ROUGH_HESTON_V0", MarketScalar::Unitless(0.04))
            .insert_price("ROUGH_HESTON_KAPPA", MarketScalar::Unitless(2.0))
            .insert_price("ROUGH_HESTON_THETA", MarketScalar::Unitless(0.04))
            .insert_price("ROUGH_HESTON_SIGMA_V", MarketScalar::Unitless(0.3))
            .insert_price("ROUGH_HESTON_RHO", MarketScalar::Unitless(-0.7))
            .insert_price("ROUGH_HESTON_HURST", MarketScalar::Unitless(0.1))
    }

    fn option(expiry: Date) -> EquityOption {
        EquityOption::builder()
            .id(InstrumentId::new("EQ-OPT-ROUGH-TEST"))
            .underlying_ticker("SPX".to_string())
            .strike(100.0)
            .option_type(OptionType::Call)
            .exercise_style(ExerciseStyle::European)
            .expiry(expiry)
            .notional(Money::new(100.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .settlement(SettlementType::Cash)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("equity option")
    }

    #[test]
    fn rough_heston_fourier_rejects_future_discrete_dividend() {
        let as_of = date(2025, 1, 1);
        let mut inst = option(date(2026, 1, 1));
        inst.discrete_dividends = vec![(date(2025, 7, 1), 2.0)];

        let err = EquityOptionRoughHestonFourierPricer::new()
            .price_dyn(&inst, &market(as_of), as_of)
            .expect_err("rough Heston Fourier must reject discrete dividends");
        let msg = err.to_string();
        assert!(
            msg.contains("discrete dividends"),
            "unexpected error message: {msg}"
        );
    }
}
