//! Bond future metrics and risk calculations.
//!
//! This module provides risk metrics for bond futures, including:
//! - Contract DV01 (dollar value of a basis point) via parallel bump of discount curve
//! - Bucketed DV01 by tenor using key-rate sensitivities
//! - Theta (time decay)
//!
//! # DV01 Calculation
//!
//! Bond future DV01 is calculated by bumping the discount curve and observing the change
//! in the contract's NPV. The conversion factor scaling is handled automatically through
//! the pricing formula:
//!
//! - Model Price = CTD Clean Price / Conversion Factor
//! - NPV = (Quoted Price - Model Price) × Contract Size × Number of Contracts
//!
//! When the discount curve is bumped:
//! 1. CTD bond's clean price changes by Δ
//! 2. Model price changes by Δ / CF (conversion factor scales the sensitivity)
//! 3. NPV changes by -Δ / CF × Contract Size × Num Contracts
//! 4. DV01 = -ΔNPV / (1 basis point)
//!
//! This ensures that the contract DV01 correctly reflects the conversion factor scaling.
//!
//! # Examples
//!
//! ```text
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::types::{CurveId, InstrumentId};
//! use finstack_quant_core::dates::Date;
//! use finstack_quant_valuations::instruments::fixed_income::bond_future::{BondFuture, DeliverableBond};
//! use finstack_quant_valuations::instruments::{Instrument, Position};
//! use finstack_quant_valuations::metrics::MetricId;
//! use time::Month;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let market = MarketContext::new();
//! let as_of = Date::from_calendar_date(2025, Month::January, 15)?;
//!
//! // Create a bond future (minimal, for compilation)
//! let future = BondFuture::ust_10y(
//!     InstrumentId::new("TYH5"),
//!     Money::new(1_000_000.0, Currency::USD),
//!     Date::from_calendar_date(2025, Month::March, 20)?,
//!     Date::from_calendar_date(2025, Month::March, 21)?,
//!     Date::from_calendar_date(2025, Month::March, 31)?,
//!     125.50,
//!     Position::Long,
//!     vec![DeliverableBond {
//!         bond_id: InstrumentId::new("US912828XG33"),
//!         conversion_factor: 0.8234,
//!     }],
//!     InstrumentId::new("US912828XG33"),
//!     CurveId::new("USD-TREASURY"),
//! )?;
//!
//! // Calculate DV01 via the pricing registry
//! let result = future.price_with_metrics(
//!     &market,
//!     as_of,
//!     &[MetricId::Dv01, MetricId::BucketedDv01],
//! )?;
//!
//! let dv01 = result.measures.get(MetricId::Dv01.as_str()).copied();
//! let bucketed_dv01 = result.measures.get(MetricId::BucketedDv01.as_str()).copied();
//! # let _ = (dv01, bucketed_dv01);
//! # Ok(())
//! # }
//! ```

use crate::metrics::MetricRegistry;
use crate::pricer::InstrumentType;

mod pricing;

/// Register all bond future metrics to a registry.
///
/// Registers the following metrics:
/// - **Dv01**: Parallel DV01 (all tenor buckets bumped together)
/// - **BucketedDv01**: Key-rate DV01 by standard IR buckets (3M, 6M, 1Y, 2Y, 3Y, 5Y, 7Y, 10Y, 15Y, 20Y, 30Y)
/// - **Theta**: Time decay (universal metric)
///
/// Each metric is registered with the "BondFuture" instrument type to ensure
/// proper applicability filtering.
///
/// # Examples
///
/// ```text
/// use finstack_quant_valuations::metrics::MetricRegistry;
/// use finstack_quant_valuations::instruments::fixed_income::bond_future::metrics::register_bond_future_metrics;
///
/// let mut registry = MetricRegistry::new();
/// register_bond_future_metrics(&mut registry);
/// ```
pub(crate) fn register_bond_future_metrics(registry: &mut MetricRegistry) {
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::BondFuture,
        metrics: [
            (FuturesPrice, pricing::FuturesPriceCalculator),
            (ConversionFactor, pricing::ConversionFactorCalculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::fixed_income::bond_future::BondFuture,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::fixed_income::bond_future::BondFuture,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{MetricId, MetricRegistry};

    #[test]
    fn test_metrics_registration() {
        let mut registry = MetricRegistry::new();
        register_bond_future_metrics(&mut registry);

        // Verify metrics are registered for BondFuture
        let metrics = registry.metrics_for_instrument(InstrumentType::BondFuture);

        // Verify DV01 is registered
        assert!(
            metrics.contains(&MetricId::Dv01),
            "DV01 metric should be registered for BondFuture"
        );

        // Verify Bucketed DV01 is registered
        assert!(
            metrics.contains(&MetricId::BucketedDv01),
            "BucketedDv01 metric should be registered for BondFuture"
        );
        assert!(
            metrics.contains(&MetricId::FuturesPrice),
            "FuturesPrice metric should be registered for BondFuture"
        );
        assert!(
            metrics.contains(&MetricId::ConversionFactor),
            "ConversionFactor metric should be registered for BondFuture"
        );
    }
}
