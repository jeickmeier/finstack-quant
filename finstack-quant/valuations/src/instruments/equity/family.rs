//! The equity family: cash equity, listed equity derivatives, volatility
//! products, private markets, real estate and equity-linked structures.

use crate::metrics::{MetricRegistry, MetricRegistryError};
use crate::pricer::{register_generic, Family, InstrumentType, PricerRegistry, PricingError};

/// Registration unit for the equity instruments.
pub static EQUITY: Family = Family::new("equity", register_pricers, register_metrics);

fn register_pricers(registry: &mut PricerRegistry) -> Result<(), PricingError> {
    register_generic!(
        registry,
        InstrumentType::Equity,
        crate::instruments::equity::Equity
    );
    register_generic!(
        registry,
        InstrumentType::EquityFuture,
        crate::instruments::EquityFuture
    );
    register_generic!(
        registry,
        InstrumentType::EquityTotalReturnFuture,
        crate::instruments::EquityTotalReturnFuture
    );
    register_generic!(
        registry,
        InstrumentType::EquityFutureOption,
        crate::instruments::EquityFutureOption
    );
    register_generic!(
        registry,
        InstrumentType::VolatilityIndexFutureOption,
        crate::instruments::VolatilityIndexFutureOption
    );
    register_generic!(
        registry,
        InstrumentType::EquityTotalReturnSwap,
        crate::instruments::equity::equity_trs::EquityTotalReturnSwap
    );
    register_generic!(
        registry,
        InstrumentType::VarianceSwap,
        crate::instruments::equity::variance_swap::VarianceSwap
    );
    register_generic!(
        registry,
        InstrumentType::VolatilityIndexFuture,
        crate::instruments::equity::vol_index_future::VolatilityIndexFuture
    );

    // Equity options: closed-form, Fourier, PDE and Monte Carlo engines.
    registry.register(
        crate::instruments::equity::equity_option::pricing::SimpleEquityOptionBlackPricer,
    )?;
    registry.register(
        crate::instruments::equity::equity_option::pricing::EquityOptionHestonFourierPricer,
    )?;
    registry.register(
        crate::instruments::equity::equity_option::pde_pricer::EquityOptionPdePricer::default(),
    )?;
    registry.register(
        crate::instruments::equity::equity_option::pde2d_pricer::EquityOptionHestonPdePricer::default(),
    )?;
    registry.register(
        crate::instruments::equity::equity_option::heston_mc_pricer::EquityOptionHestonMcPricer::default(),
    )?;
    registry.register(
        crate::instruments::equity::equity_option::rough_heston_fourier_pricer::EquityOptionRoughHestonFourierPricer,
    )?;
    registry.register(
        crate::instruments::equity::equity_option::rough_heston_mc_pricer::EquityOptionRoughHestonMcPricer::default(),
    )?;
    registry.register(
        crate::instruments::equity::equity_option::rough_bergomi_mc_pricer::EquityOptionRoughBergomiMcPricer::default(),
    )?;

    // Equity-linked structures.
    registry.register(
        crate::instruments::equity::autocallable::pricer::AutocallableMcPricer::default(),
    )?;
    registry.register(
        crate::instruments::equity::cliquet_option::pricer::CliquetOptionMcPricer::default(),
    )?;

    // Corporate, private-markets and real-estate valuation.
    registry.register(crate::instruments::equity::dcf_equity::pricer::DcfPricer)?;
    register_generic!(
        registry,
        InstrumentType::RealEstateAsset,
        crate::instruments::RealEstateAsset
    );
    register_generic!(
        registry,
        InstrumentType::LeveredRealEstateEquity,
        crate::instruments::LeveredRealEstateEquity
    );
    register_generic!(
        registry,
        InstrumentType::PrivateMarketsFund,
        crate::instruments::equity::pe_fund::PrivateMarketsFund
    );
    Ok(())
}

fn register_metrics(registry: &mut MetricRegistry) -> Result<(), MetricRegistryError> {
    crate::instruments::equity::spot::metrics::register_equity_metrics(registry)?;
    crate::instruments::equity::equity_option::metrics::register_equity_option_metrics(registry)?;
    crate::instruments::equity::equity_future::metrics::register_equity_future_metrics(registry)?;
    crate::instruments::equity::equity_total_return_future::metrics::register_equity_total_return_future_metrics(registry)?;
    crate::instruments::equity::equity_future_option::metrics::register_equity_future_option_metrics(registry)?;
    crate::instruments::equity::vol_index_future_option::metrics::register_vol_index_future_option_metrics(registry)?;
    crate::instruments::equity::equity_trs::metrics::register_equity_trs_metrics(registry)?;
    crate::instruments::equity::variance_swap::metrics::register_variance_swap_metrics(registry)?;
    crate::instruments::equity::pe_fund::metrics::register_private_markets_fund_metrics(registry)?;
    crate::instruments::equity::dcf_equity::metrics::register_dcf_metrics(registry)?;
    crate::instruments::equity::vol_index_future::metrics::register_vol_index_future_metrics(
        registry,
    )?;
    crate::instruments::equity::real_estate::metrics::register_real_estate_metrics(registry)?;
    crate::instruments::equity::real_estate::metrics::register_levered_real_estate_metrics(
        registry,
    )?;
    crate::instruments::equity::autocallable::metrics::register_autocallable_metrics(registry)?;
    crate::instruments::equity::cliquet_option::metrics::register_cliquet_option_metrics(registry)?;
    Ok(())
}
