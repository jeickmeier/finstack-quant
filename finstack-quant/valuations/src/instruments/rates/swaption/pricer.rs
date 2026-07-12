//! Vanilla swaption pricer implementation.

use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::swaption::{Swaption, VolatilityModel};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::market_data::context::MarketContext;

/// Swaption pricer supporting Black-76 and instrument-selected fallback models.
pub struct SimpleSwaptionBlackPricer {
    model: ModelKey,
}

impl SimpleSwaptionBlackPricer {
    /// Create a new swaption pricer with the default Black-76 model.
    pub fn new() -> Self {
        Self {
            model: ModelKey::Black76,
        }
    }

    /// Create a swaption pricer with the specified model key.
    pub fn with_model(model: ModelKey) -> Self {
        Self { model }
    }
}

impl Default for SimpleSwaptionBlackPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for SimpleSwaptionBlackPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Swaption, self.model)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let swaption = instrument
            .as_any()
            .downcast_ref::<Swaption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::Swaption, instrument.key())
            })?;

        if as_of > swaption.expiry {
            return Ok(ValuationResult::stamped(
                swaption.id(),
                as_of,
                finstack_quant_core::money::Money::new(0.0, swaption.notional.currency()),
            ));
        }
        if as_of == swaption.expiry {
            let forward = swaption.forward_swap_rate(market, as_of).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
            let disc = market
                .get_discount(swaption.underlying_discount_curve_id().as_ref())
                .map_err(|e| {
                    PricingError::missing_market_data_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;
            let annuity = swaption
                .annuity(disc.as_ref(), as_of, forward)
                .map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;
            let strike = swaption.strike_f64().map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
            let intrinsic = match swaption.option_type {
                crate::instruments::OptionType::Call => (forward - strike).max(0.0),
                crate::instruments::OptionType::Put => (strike - forward).max(0.0),
            };
            return Ok(ValuationResult::stamped(
                swaption.id(),
                as_of,
                finstack_quant_core::money::Money::new(
                    intrinsic * annuity * swaption.notional.amount(),
                    swaption.notional.currency(),
                ),
            ));
        }

        let pv = match self.model {
            ModelKey::Black76 => {
                if swaption.sabr_params.is_some() {
                    swaption.price_sabr(market, as_of).map_err(|e| {
                        PricingError::model_failure_with_context(
                            e.to_string(),
                            PricingErrorContext::default(),
                        )
                    })?
                } else {
                    // Use Act/365F for the option time-to-expiry so the vol-surface
                    // pillar lookup is on the SAME time axis that `price_black` /
                    // `price_normal` use internally (Swaption::time_to_expiry
                    // hardcodes Act/365F). Using `swaption.day_count` here indexed
                    // the surface at a different tenor for non-Act/365F swaptions
                    // (e.g. Act/360 -> ~1.4% relative-vol error and a mis-pillared
                    // calibration).
                    let time_to_expiry = year_fraction(
                        finstack_quant_core::dates::DayCount::Act365F,
                        as_of,
                        swaption.expiry,
                    )
                    .map_err(|e| {
                        PricingError::model_failure_with_context(
                            e.to_string(),
                            PricingErrorContext::default(),
                        )
                    })?;
                    let forward = swaption.forward_swap_rate(market, as_of).map_err(|e| {
                        PricingError::model_failure_with_context(
                            e.to_string(),
                            PricingErrorContext::default(),
                        )
                    })?;
                    let vol = swaption
                        .resolve_volatility(market, forward, time_to_expiry)
                        .map_err(|e| {
                            PricingError::missing_market_data_with_context(
                                e.to_string(),
                                PricingErrorContext::default(),
                            )
                        })?;

                    match swaption.vol_model {
                        VolatilityModel::Black => swaption.price_black(market, vol, as_of),
                        VolatilityModel::Normal => swaption.price_normal(market, vol, as_of),
                    }
                    .map_err(|e| {
                        PricingError::model_failure_with_context(
                            e.to_string(),
                            PricingErrorContext::default(),
                        )
                    })?
                }
            }
            _ => swaption.value(market, as_of).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?,
        };

        Ok(ValuationResult::stamped(swaption.id(), as_of, pv))
    }
}
