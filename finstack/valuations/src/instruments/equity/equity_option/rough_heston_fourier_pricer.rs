//! Rough Heston semi-analytical pricer via Fourier inversion.
//!
//! Uses the fractional Riccati solver from `finstack_core::math::volatility::rough_heston`
//! to price European equity options under the rough Heston model (El Euch & Rosenbaum 2019).
//! Model parameters are sourced from market scalars with sensible defaults.

use super::pricer::{collect_inputs_extended, option_currency};
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
/// Rough Heston parameters are read from market scalars:
///
/// | Scalar Key | Default | Description |
/// |---|---|---|
/// | `ROUGH_HESTON_V0` | 0.04 | Initial variance |
/// | `ROUGH_HESTON_KAPPA` | 2.0 | Mean reversion speed |
/// | `ROUGH_HESTON_THETA` | 0.04 | Long-run variance |
/// | `ROUGH_HESTON_SIGMA_V` | 0.3 | Vol-of-vol |
/// | `ROUGH_HESTON_RHO` | -0.7 | Spot-vol correlation |
/// | `ROUGH_HESTON_HURST` | 0.1 | Hurst exponent |
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

        // Source rough-Heston scalars from a single shared lookup.
        let s = crate::instruments::equity::equity_option::rough_heston_market::RoughHestonScalars::from_market(market);

        let err_ctx = crate::pricer::PricingErrorContext::from_instrument(equity_option)
            .model(crate::pricer::ModelKey::RoughHestonFourier);

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
