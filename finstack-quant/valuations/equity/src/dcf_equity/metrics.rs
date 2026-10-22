//! DCF-specific metrics (enterprise value, equity value, terminal value PV, DV01,
//! price-per-share, and diluted shares).
//!
//! These metrics are registered under the `DCF` instrument type and integrate
//! with the unified DV01 framework used across valuations.

use crate::dcf_equity::{pricer, DiscountedCashFlow};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{
    MetricCalculator, MetricContext, MetricRegistry, RfComponentPriced,
};

impl RfComponentPriced for DiscountedCashFlow {
    fn pv_with_rf_bump(
        &self,
        _market: &MarketContext,
        _as_of: Date,
        bump_at: &dyn Fn(f64) -> f64,
    ) -> Result<f64> {
        pricer::pv_with_rf_bump(self, bump_at)
    }
}

/// Calculator for Enterprise Value metric.
///
/// Computes EV from PV components directly (before equity bridge / discounts).
/// Always discounts at WACC , matching `compute_pv`.
struct EnterpriseValueCalculator;

impl MetricCalculator for EnterpriseValueCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let dcf: &DiscountedCashFlow = context.instrument_as()?;
        let (pv_explicit, pv_terminal) = pricer::pv_components_with_rf_bump(dcf, &|_| 0.0)?;
        Ok(pv_explicit + pv_terminal)
    }
}

/// Calculator for Equity Value metric.
///
/// The priced value: equity bridge and valuation discounts applied.
struct EquityValueCalculator;

impl MetricCalculator for EquityValueCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        Ok(context.base_value.amount())
    }
}

/// Calculator for Terminal Value PV metric.
///
/// Always discounts at WACC , matching `compute_pv`.
struct TerminalValuePVCalculator;

impl MetricCalculator for TerminalValuePVCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let dcf: &DiscountedCashFlow = context.instrument_as()?;
        Ok(pricer::pv_components_with_rf_bump(dcf, &|_| 0.0)?.1)
    }
}

/// Calculator for Equity Price Per Share metric.
///
/// Returns equity value / diluted shares using the treasury stock method.
/// Errors if `shares_outstanding` is not set (or diluted shares is
/// non-positive) rather than emitting a silent `NaN` that would poison
/// downstream aggregations.
struct EquityPricePerShareCalculator;

impl MetricCalculator for EquityPricePerShareCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let dcf: &DiscountedCashFlow = context.instrument_as()?;
        dcf.equity_value_per_share(context.base_value.amount())
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "DCF '{}' EquityPricePerShare requires shares_outstanding to be set \
                 and diluted shares to be positive",
                    dcf.id.as_str()
                ))
            })
    }
}

/// Calculator for diluted share count metric.
///
/// Returns diluted shares via treasury stock method.
/// Errors if `shares_outstanding` is not set rather than emitting a silent
/// `NaN` that would poison downstream aggregations.
struct EquitySharesCalculator;

impl MetricCalculator for EquitySharesCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let dcf: &DiscountedCashFlow = context.instrument_as()?;
        dcf.diluted_shares(context.base_value.amount())
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "DCF '{}' EquityShares requires shares_outstanding to be set",
                    dcf.id.as_str()
                ))
            })
    }
}

/// Registers all DCF metrics to a registry.
///
/// Includes WACC sensitivity (`custom("dcf::wacc01")`) plus enterprise value,
/// equity value, terminal value, price-per-share, and diluted-share metrics.
/// - Enterprise value (`MetricId::EnterpriseValue`)
/// - Equity value (`MetricId::EquityValue`)
/// - Terminal value PV (`MetricId::TerminalValuePV`)
/// - Equity price per share (`MetricId::EquityPricePerShare`)
/// - Diluted shares (`MetricId::EquityShares`)
pub(crate) fn register_dcf_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Dcf,
        metrics: [
            // WACC sensitivity is registered separately under `dcf::wacc01`;
            // DCF has no direct market-curve DV01.
            (EnterpriseValue, EnterpriseValueCalculator),
            (EquityValue, EquityValueCalculator),
            (TerminalValuePV, TerminalValuePVCalculator),
            (EquityPricePerShare, EquityPricePerShareCalculator),
            (EquityShares, EquitySharesCalculator),
        ],
        custom: [
            ("dcf::wacc01", finstack_quant_valuations_core::metrics::RfComponentDv01Calculator::<crate::dcf_equity::DiscountedCashFlow>::new()),
        ]
    }
    Ok(())
}
