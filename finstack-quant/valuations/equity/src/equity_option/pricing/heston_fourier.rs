//! Equity option Heston semi-analytical pricer (Fourier inversion).

use super::{ModelInputs, stochastic_vol_inputs};
use crate::equity_option::types::EquityOption;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_models::closed_form::heston_price;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::pricer::PricingError;
use finstack_quant_valuations_core::pricer::expect_inst;

/// Equity option Heston semi-analytical pricer (Fourier inversion).
pub(crate) struct EquityOptionHestonFourierPricer;

impl finstack_quant_valuations_core::pricer::Pricer for EquityOptionHestonFourierPricer {
    fn key(&self) -> finstack_quant_valuations_core::pricer::PricerKey {
        finstack_quant_valuations_core::pricer::PricerKey::new(
            finstack_quant_valuations_core::pricer::InstrumentType::EquityOption,
            finstack_quant_valuations_core::pricer::ModelKey::HestonFourier,
        )
    }

    #[tracing::instrument(
        name = "equity_option.heston_fourier.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn finstack_quant_valuations_core::instruments::Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<finstack_quant_valuations_core::results::ValuationResult, PricingError>
    {
        let equity_option = expect_inst::<EquityOption>(
            instrument,
            finstack_quant_valuations_core::pricer::InstrumentType::EquityOption,
        )?;
        let (spot, r, q, t) = match stochastic_vol_inputs(
            equity_option,
            market,
            as_of,
            finstack_quant_valuations_core::pricer::ModelKey::HestonFourier,
            "Heston Fourier",
        )? {
            ModelInputs::Settled(pv) => {
                return Ok(
                    finstack_quant_valuations_core::results::ValuationResult::stamped(
                        equity_option.id(),
                        as_of,
                        pv,
                    ),
                );
            }
            ModelInputs::Live(inputs) => (inputs.spot, inputs.r, inputs.q, inputs.t_vol),
        };

        // Source production Heston parameters from explicit market scalars.
        // Validation is still enforced inside `HestonParams::new`.
        let err_ctx = finstack_quant_valuations_core::pricer::PricingErrorContext::from_instrument(
            equity_option,
        )
        .model(finstack_quant_valuations_core::pricer::ModelKey::HestonFourier);
        let params = finstack_quant_valuations_core::instruments::pricing::heston_market::heston_params_from_market_strict(
            market,
        )
        .map_err(|e| finstack_quant_valuations_core::pricer::PricingError::from_core(e, err_ctx.clone()))?;

        let contract = finstack_quant_models::types::OptionContractParams {
            spot,
            strike: equity_option.strike,
            rate: r,
            dividend_yield: q,
            time_to_expiry: t,
            option_type: equity_option.option_type,
        };
        let price = heston_price(&contract, &params, None).map_err(|error| {
            finstack_quant_valuations_core::pricer::PricingError::from_core(error, err_ctx)
        })?;

        let pv = Money::from_model_value(price * equity_option.quantity, equity_option.currency)
            .map_err(|error| {
                finstack_quant_valuations_core::pricer::PricingError::from_core(
                    error,
                    finstack_quant_valuations_core::pricer::PricingErrorContext::from_instrument(
                        equity_option,
                    ),
                )
            })?;
        Ok(
            finstack_quant_valuations_core::results::ValuationResult::stamped(
                equity_option.id(),
                as_of,
                pv,
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{date, market, option};
    use super::*;
    use finstack_quant_models::types::{ExerciseStyle, OptionType};
    use finstack_quant_valuations_core::pricer::Pricer;

    #[test]
    fn heston_fourier_rejects_future_discrete_dividend() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2026, 1, 1);
        let mut opt = option(expiry, OptionType::Call, ExerciseStyle::European);
        opt.discrete_dividends = vec![(date(2025, 7, 1), 2.0)];

        let err = EquityOptionHestonFourierPricer
            .price_dyn(&opt, &market(as_of, 100.0, 0.20, 0.03, 0.0), as_of)
            .expect_err("Heston Fourier must reject discrete dividends");
        let msg = err.to_string();
        assert!(
            msg.contains("discrete dividends"),
            "unexpected error message: {msg}"
        );
    }

    #[test]
    fn heston_fourier_prices_without_vol_surface() {
        use finstack_quant_core::dates::DayCount;
        use finstack_quant_core::market_data::scalars::MarketScalar;
        use finstack_quant_core::market_data::term_structures::DiscountCurve;

        let as_of = date(2025, 1, 1);
        let expiry = date(2026, 1, 1);
        let opt = option(expiry, OptionType::Call, ExerciseStyle::European);
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (2.0, (-0.03 * 2.0_f64).exp())])
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("discount curve");
        let market_no_vol = MarketContext::new()
            .try_insert(disc)
            .expect("insert curve")
            .insert_price("SPX-SPOT", MarketScalar::unitless(100.0).expect("spot"))
            .insert_price("SPX-DIV", MarketScalar::unitless(0.01).expect("div"))
            .insert_price("HESTON_KAPPA", MarketScalar::unitless(1.5).expect("kappa"))
            .insert_price("HESTON_THETA", MarketScalar::unitless(0.04).expect("theta"))
            .insert_price(
                "HESTON_SIGMA_V",
                MarketScalar::unitless(0.3).expect("sigma_v"),
            )
            .insert_price("HESTON_RHO", MarketScalar::unitless(-0.5).expect("rho"))
            .insert_price("HESTON_V0", MarketScalar::unitless(0.04).expect("v0"));

        let res = EquityOptionHestonFourierPricer
            .price_dyn(&opt, &market_no_vol, as_of)
            .expect("Heston Fourier prices without VolSurface");
        assert!(res.value.amount() > 0.0 && res.value.amount().is_finite());
    }
}
