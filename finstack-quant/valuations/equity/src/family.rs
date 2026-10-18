//! The equity family: cash equity, listed equity derivatives, volatility
//! products, private markets and equity-linked structures.

use finstack_quant_valuations_core::metrics::{MetricRegistry, MetricRegistryError};
use finstack_quant_valuations_core::pricer::{
    register_generic, Family, InstrumentType, PricerRegistry, PricingError,
};

/// Registration unit for the equity instruments.
pub static EQUITY: Family = Family::new("equity", register_pricers, register_metrics);

fn register_pricers(registry: &mut PricerRegistry) -> Result<(), PricingError> {
    register_generic!(registry, InstrumentType::Equity, crate::Equity);
    register_generic!(registry, InstrumentType::EquityFuture, crate::EquityFuture);
    register_generic!(
        registry,
        InstrumentType::EquityTotalReturnFuture,
        crate::EquityTotalReturnFuture
    );
    register_generic!(
        registry,
        InstrumentType::EquityFutureOption,
        crate::EquityFutureOption
    );
    register_generic!(
        registry,
        InstrumentType::VolatilityIndexFutureOption,
        crate::VolatilityIndexFutureOption
    );
    register_generic!(
        registry,
        InstrumentType::EquityTotalReturnSwap,
        crate::equity_trs::EquityTotalReturnSwap
    );
    register_generic!(
        registry,
        InstrumentType::VarianceSwap,
        crate::variance_swap::VarianceSwap
    );
    register_generic!(
        registry,
        InstrumentType::VolatilityIndexFuture,
        crate::vol_index_future::VolatilityIndexFuture
    );

    // Equity options: closed-form, Fourier, PDE and Monte Carlo engines.
    registry.register(crate::equity_option::pricing::SimpleEquityOptionBlackPricer)?;
    registry.register(crate::equity_option::pricing::EquityOptionHestonFourierPricer)?;
    registry.register(crate::equity_option::pde_pricer::EquityOptionPdePricer::default())?;
    registry
        .register(crate::equity_option::pde2d_pricer::EquityOptionHestonPdePricer::default())?;
    registry
        .register(crate::equity_option::heston_mc_pricer::EquityOptionHestonMcPricer::default())?;
    registry.register(
        crate::equity_option::rough_heston_fourier_pricer::EquityOptionRoughHestonFourierPricer,
    )?;
    registry.register(
        crate::equity_option::rough_heston_mc_pricer::EquityOptionRoughHestonMcPricer::default(),
    )?;
    registry.register(
        crate::equity_option::rough_bergomi_mc_pricer::EquityOptionRoughBergomiMcPricer::default(),
    )?;

    // Equity-linked structures.
    registry.register(crate::autocallable::pricer::AutocallableMcPricer::default())?;
    registry.register(crate::cliquet_option::pricer::CliquetOptionMcPricer::default())?;

    // Corporate and private-markets valuation.
    registry.register(crate::dcf_equity::pricer::DcfPricer)?;
    register_generic!(
        registry,
        InstrumentType::PrivateMarketsFund,
        crate::pe_fund::PrivateMarketsFund
    );
    Ok(())
}

fn register_metrics(registry: &mut MetricRegistry) -> Result<(), MetricRegistryError> {
    crate::spot::metrics::register_equity_metrics(registry)?;
    crate::equity_option::metrics::register_equity_option_metrics(registry)?;
    crate::equity_future::metrics::register_equity_future_metrics(registry)?;
    crate::equity_total_return_future::metrics::register_equity_total_return_future_metrics(
        registry,
    )?;
    crate::equity_future_option::metrics::register_equity_future_option_metrics(registry)?;
    crate::vol_index_future_option::metrics::register_vol_index_future_option_metrics(registry)?;
    crate::equity_trs::metrics::register_equity_trs_metrics(registry)?;
    crate::variance_swap::metrics::register_variance_swap_metrics(registry)?;
    crate::pe_fund::metrics::register_private_markets_fund_metrics(registry)?;
    crate::dcf_equity::metrics::register_dcf_metrics(registry)?;
    crate::vol_index_future::metrics::register_vol_index_future_metrics(registry)?;
    crate::autocallable::metrics::register_autocallable_metrics(registry)?;
    crate::cliquet_option::metrics::register_cliquet_option_metrics(registry)?;
    Ok(())
}
