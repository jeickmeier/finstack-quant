//! Pricer registrations for equity instruments.
//!
//! Covers: Equity, EquityOption, EquityTotalReturnSwap, VarianceSwap,
//! EquityIndexFuture, VolatilityIndexFuture, VolatilityIndexOption,
//! RealEstateAsset, LeveredRealEstateEquity, PrivateMarketsFund.

use super::{InstrumentType, ModelKey, PricerRegistry};

/// Register pricers for equity instruments.
pub fn register_equity_pricers(registry: &mut PricerRegistry) {
    // Equity
    registry.register(
        InstrumentType::Equity,
        ModelKey::Discounting,
        crate::instruments::common_impl::GenericInstrumentPricer::<
            crate::instruments::equity::Equity,
        >::discounting(InstrumentType::Equity),
    );

    // Equity Option
    registry.register(
        InstrumentType::EquityOption,
        ModelKey::Black76,
        crate::instruments::equity::equity_option::pricer::SimpleEquityOptionBlackPricer::default(),
    );
    registry.register(
        InstrumentType::EquityOption,
        ModelKey::Discounting,
        crate::instruments::equity::equity_option::pricer::SimpleEquityOptionBlackPricer::with_model(
            ModelKey::Discounting,
        ),
    );

    registry.register(
        InstrumentType::EquityOption,
        ModelKey::HestonFourier,
        crate::instruments::equity::equity_option::pricer::EquityOptionHestonFourierPricer,
    );

    // Equity TRS
    registry.register(
        InstrumentType::EquityTotalReturnSwap,
        ModelKey::Discounting,
        crate::instruments::common_impl::GenericInstrumentPricer::<
            crate::instruments::equity::equity_trs::EquityTotalReturnSwap,
        >::discounting(InstrumentType::EquityTotalReturnSwap),
    );

    // Variance Swap
    registry.register(
        InstrumentType::VarianceSwap,
        ModelKey::Discounting,
        crate::instruments::common_impl::GenericInstrumentPricer::<
            crate::instruments::equity::variance_swap::VarianceSwap,
        >::discounting(InstrumentType::VarianceSwap),
    );

    // Equity Index Future - uses GenericInstrumentPricer
    registry.register(
        InstrumentType::EquityIndexFuture,
        ModelKey::Discounting,
        crate::instruments::common_impl::GenericInstrumentPricer::<
            crate::instruments::EquityIndexFuture,
        >::discounting(InstrumentType::EquityIndexFuture),
    );

    // Volatility Index Future
    registry.register(
        InstrumentType::VolatilityIndexFuture,
        ModelKey::Discounting,
        crate::instruments::common_impl::GenericInstrumentPricer::<
            crate::instruments::equity::vol_index_future::VolatilityIndexFuture,
        >::discounting(InstrumentType::VolatilityIndexFuture),
    );

    // Volatility Index Option
    registry.register(
        InstrumentType::VolatilityIndexOption,
        ModelKey::Discounting,
        crate::instruments::common_impl::GenericInstrumentPricer::<
            crate::instruments::equity::vol_index_option::VolatilityIndexOption,
        >::discounting(InstrumentType::VolatilityIndexOption),
    );

    // Real Estate Asset - uses GenericInstrumentPricer (curve dependencies)
    registry.register(
        InstrumentType::RealEstateAsset,
        ModelKey::Discounting,
        crate::instruments::common_impl::GenericInstrumentPricer::<
            crate::instruments::RealEstateAsset,
        >::discounting(InstrumentType::RealEstateAsset),
    );

    // Levered Real Estate Equity - uses GenericInstrumentPricer
    registry.register(
        InstrumentType::LeveredRealEstateEquity,
        ModelKey::Discounting,
        crate::instruments::common_impl::GenericInstrumentPricer::<
            crate::instruments::LeveredRealEstateEquity,
        >::discounting(InstrumentType::LeveredRealEstateEquity),
    );

    // Private Markets Fund
    registry.register(
        InstrumentType::PrivateMarketsFund,
        ModelKey::Discounting,
        crate::instruments::equity::pe_fund::pricer::PrivateMarketsFundDiscountingPricer,
    );

    // Equity Option - PDE Crank-Nicolson 1D (Black-Scholes)
    registry.register(
        InstrumentType::EquityOption,
        ModelKey::PdeCrankNicolson1D,
        crate::instruments::equity::equity_option::pde_pricer::EquityOptionPdePricer::default(),
    );

    // Equity Option - PDE ADI 2D (Heston)
    registry.register(
        InstrumentType::EquityOption,
        ModelKey::PdeAdi2D,
        crate::instruments::equity::equity_option::pde2d_pricer::EquityOptionHestonPdePricer::default(),
    );

    // Equity Option - Monte Carlo Heston

    registry.register(
        InstrumentType::EquityOption,
        ModelKey::MonteCarloHeston,
        crate::instruments::equity::equity_option::heston_mc_pricer::EquityOptionHestonMcPricer::default(),
    );

    // Equity Option - Rough Heston Fourier

    registry.register(
        InstrumentType::EquityOption,
        ModelKey::RoughHestonFourier,
        crate::instruments::equity::equity_option::rough_heston_fourier_pricer::EquityOptionRoughHestonFourierPricer,
    );

    // Equity Option - Monte Carlo Rough Heston

    registry.register(
        InstrumentType::EquityOption,
        ModelKey::MonteCarloRoughHeston,
        crate::instruments::equity::equity_option::rough_heston_mc_pricer::EquityOptionRoughHestonMcPricer::default(),
    );

    // Equity Option - Monte Carlo Rough Bergomi

    registry.register(
        InstrumentType::EquityOption,
        ModelKey::MonteCarloRoughBergomi,
        crate::instruments::equity::equity_option::rough_bergomi_mc_pricer::EquityOptionRoughBergomiMcPricer::default(),
    );
}
