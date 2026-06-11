//! DCF-specific metrics (enterprise value, equity value, terminal value PV, DV01,
//! price-per-share, and diluted shares).
//!
//! These metrics are registered under the `DCF` instrument type and integrate
//! with the unified DV01 framework used across valuations.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::dcf_equity::{pricer, DiscountedCashFlow};
use crate::metrics::{MetricCalculator, MetricContext, MetricRegistry, RfComponentPriced};
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::Result;

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

/// Helper: downcast to [`DiscountedCashFlow`] or return a validation error.
fn downcast_dcf(context: &MetricContext) -> Result<&DiscountedCashFlow> {
    context
        .instrument
        .as_any()
        .downcast_ref::<DiscountedCashFlow>()
        .ok_or_else(|| {
            finstack_core::Error::Validation("Expected DiscountedCashFlow instrument".into())
        })
}

/// Calculator for Enterprise Value metric.
///
/// Computes EV from PV components directly (before equity bridge / discounts).
/// Always discounts at WACC (review finding M14), matching `compute_pv`.
struct EnterpriseValueCalculator;

impl MetricCalculator for EnterpriseValueCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let dcf = downcast_dcf(context)?;
        // Compute EV directly from discounted PV components (before equity bridge / discounts).
        let terminal_value = dcf.calculate_terminal_value()?;
        let pv_explicit = dcf.calculate_pv_explicit_flows();
        let pv_terminal = dcf.discount_terminal_value(terminal_value)?;
        Ok(pv_explicit + pv_terminal)
    }
}

/// Calculator for Equity Value metric.
///
/// Consistent with the `value()` method: applies equity bridge and valuation discounts.
struct EquityValueCalculator;

impl MetricCalculator for EquityValueCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let dcf = downcast_dcf(context)?;
        let equity = dcf.value(context.curves.as_ref(), context.as_of)?;
        Ok(equity.amount())
    }
}

/// Calculator for Terminal Value PV metric.
///
/// Always discounts at WACC (review finding M14), matching `compute_pv`.
struct TerminalValuePVCalculator;

impl MetricCalculator for TerminalValuePVCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let dcf = downcast_dcf(context)?;
        let terminal_value = dcf.calculate_terminal_value()?;
        dcf.discount_terminal_value(terminal_value)
    }
}

/// Calculator for Equity Price Per Share metric.
///
/// Returns equity value / diluted shares using the treasury stock method.
/// Returns `NaN` if `shares_outstanding` is not set.
struct EquityPricePerShareCalculator;

impl MetricCalculator for EquityPricePerShareCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let dcf = downcast_dcf(context)?;
        let equity = dcf.value(context.curves.as_ref(), context.as_of)?;
        Ok(dcf
            .equity_value_per_share(equity.amount())
            .unwrap_or(f64::NAN))
    }
}

/// Calculator for diluted share count metric.
///
/// Returns diluted shares via treasury stock method.
/// Returns `NaN` if `shares_outstanding` is not set.
struct EquitySharesCalculator;

impl MetricCalculator for EquitySharesCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let dcf = downcast_dcf(context)?;
        let equity = dcf.value(context.curves.as_ref(), context.as_of)?;
        Ok(dcf.diluted_shares(equity.amount()).unwrap_or(f64::NAN))
    }
}

/// Registers all DCF metrics to a registry.
///
/// Includes:
/// - Parallel DV01 (`MetricId::Dv01`)
/// - Bucketed DV01 (`MetricId::BucketedDv01`)
/// - Theta (`MetricId::Theta`)
/// - Enterprise value (`MetricId::EnterpriseValue`)
/// - Equity value (`MetricId::EquityValue`)
/// - Terminal value PV (`MetricId::TerminalValuePV`)
/// - Equity price per share (`MetricId::EquityPricePerShare`)
/// - Diluted shares (`MetricId::EquityShares`)
pub(crate) fn register_dcf_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::DCF,
        metrics: [
            // Rate risk via rf-component bump inside the WACC (review finding
            // M14): DCF always discounts at WACC, so DV01 bumps the additive
            // risk-free component of the rate rather than a market curve.
            (
                Dv01,
                crate::metrics::RfComponentDv01Calculator::<
                    crate::instruments::equity::dcf_equity::DiscountedCashFlow,
                >::new(crate::metrics::RfDv01Mode::Parallel)
            ),
            (
                BucketedDv01,
                crate::metrics::RfComponentDv01Calculator::<
                    crate::instruments::equity::dcf_equity::DiscountedCashFlow,
                >::new(crate::metrics::RfDv01Mode::Bucketed)
            ),
            (EnterpriseValue, EnterpriseValueCalculator),
            (EquityValue, EquityValueCalculator),
            (TerminalValuePV, TerminalValuePVCalculator),
            (EquityPricePerShare, EquityPricePerShareCalculator),
            (EquityShares, EquitySharesCalculator),
        ]
    }
}
