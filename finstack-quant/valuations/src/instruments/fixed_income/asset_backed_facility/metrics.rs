//! Facility metrics: borrowing base, cushion, advance-rate utilization and
//! the lender / residual IRRs, plus the standard rate and spread
//! sensitivities through the generic calculators.

use std::sync::Arc;

use crate::instruments::fixed_income::asset_backed_facility::AssetBackedFacility;
use crate::instruments::fixed_income::structured_credit::calculate_equity_metrics;
use crate::metrics::{
    MetricCalculator, MetricContext, MetricId, MetricRegistry, ZSpreadCs01, ZSpreadCs01Inputs,
};
use crate::pricer::InstrumentType;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;

impl ZSpreadCs01 for AssetBackedFacility {
    /// The lender's projected receipts (interest, principal, unused fees)
    /// on or after `as_of`, z-bumped at the payment frequency.
    fn z_spread_cs01_inputs(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<ZSpreadCs01Inputs> {
        let years = self.frequency.to_years();
        let compounds_per_year = if years > 0.0 && years.is_finite() {
            (1.0 / years).round().max(1.0)
        } else {
            1.0
        };
        let flows = self
            .project(curves, as_of)?
            .lender_cashflows()?
            .into_iter()
            .filter(|(date, _)| *date >= as_of)
            .collect();
        Ok(ZSpreadCs01Inputs {
            settlement: as_of,
            discount_curve_id: self.discount_curve_id.clone(),
            compounds_per_year,
            flows,
        })
    }
}

/// Borrowing base on the closing collateral, in currency units.
#[derive(Debug, Default, Clone, Copy)]
pub struct AbfBorrowingBaseCalculator;

impl MetricCalculator for AbfBorrowingBaseCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &AssetBackedFacility = context.instrument_as()?;
        Ok(facility.borrowing_base()?.borrowing_base.amount())
    }
}

/// Borrowing-base cushion: `(borrowing base − drawn) / borrowing base` in
/// percent (negative when the facility is in deficiency).
#[derive(Debug, Default, Clone, Copy)]
pub struct AbfBorrowingBaseCushionCalculator;

impl MetricCalculator for AbfBorrowingBaseCushionCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &AssetBackedFacility = context.instrument_as()?;
        let base = facility.borrowing_base()?.borrowing_base.amount();
        if base <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "borrowing-base cushion needs a positive borrowing base".to_string(),
            ));
        }
        Ok((base - facility.drawn.amount()) / base * 100.0)
    }
}

/// Advance-rate utilization: `drawn / borrowing base` as a decimal (above 1
/// in deficiency).
#[derive(Debug, Default, Clone, Copy)]
pub struct AbfAdvanceRateUtilizationCalculator;

impl MetricCalculator for AbfAdvanceRateUtilizationCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &AssetBackedFacility = context.instrument_as()?;
        let base = facility.borrowing_base()?.borrowing_base.amount();
        if base <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "advance-rate utilization needs a positive borrowing base".to_string(),
            ));
        }
        Ok(facility.drawn.amount() / base)
    }
}

/// Lender IRR as an annual decimal (see `AssetBackedFacility::facility_irr`).
#[derive(Debug, Default, Clone, Copy)]
pub struct AbfFacilityIrrCalculator;

impl MetricCalculator for AbfFacilityIrrCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &AssetBackedFacility = context.instrument_as()?;
        let market = Arc::clone(&context.curves);
        facility.facility_irr(&market, context.as_of)
    }
}

/// Residual IRR as an annual decimal: the equity XIRR of the synthetic deal
/// at par.
#[derive(Debug, Default, Clone, Copy)]
pub struct AbfResidualIrrCalculator;

impl MetricCalculator for AbfResidualIrrCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &AssetBackedFacility = context.instrument_as()?;
        let market = Arc::clone(&context.curves);
        let deal = facility.synthesized_deal()?;
        calculate_equity_metrics(&deal, &market, context.as_of, None)?
            .irr
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "residual IRR does not solve for this projection".to_string(),
                )
            })
    }
}

/// Register the facility metrics.
///
/// # Arguments
///
/// * `registry` - Metric registry receiving the facility calculators.
///
/// # Errors
///
/// Returns the registry's error when a metric id is already registered for
/// the instrument type.
pub(crate) fn register_asset_backed_facility_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::AssetBackedFacility,
        metrics: [
            (Dv01, crate::metrics::UnifiedDv01Calculator::<AssetBackedFacility>::new(
                crate::metrics::Dv01CalculatorConfig::parallel_combined()
            )),
            (Cs01, crate::metrics::ZSpreadParallelCs01::<AssetBackedFacility>::hazard_when_credit_curve()),
            (BucketedCs01, crate::metrics::ZSpreadBucketedCs01::<AssetBackedFacility>::hazard_when_credit_curve()),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<AssetBackedFacility>::new(
                crate::metrics::Dv01CalculatorConfig::triangular_key_rate()
            )),
        ]
    }

    registry.register_metric(
        MetricId::AbfBorrowingBase,
        Arc::new(AbfBorrowingBaseCalculator),
        &[InstrumentType::AssetBackedFacility],
    )?;
    registry.register_metric(
        MetricId::AbfBorrowingBaseCushion,
        Arc::new(AbfBorrowingBaseCushionCalculator),
        &[InstrumentType::AssetBackedFacility],
    )?;
    registry.register_metric(
        MetricId::AbfAdvanceRateUtilization,
        Arc::new(AbfAdvanceRateUtilizationCalculator),
        &[InstrumentType::AssetBackedFacility],
    )?;
    registry.register_metric(
        MetricId::AbfFacilityIrr,
        Arc::new(AbfFacilityIrrCalculator),
        &[InstrumentType::AssetBackedFacility],
    )?;
    registry.register_metric(
        MetricId::AbfResidualIrr,
        Arc::new(AbfResidualIrrCalculator),
        &[InstrumentType::AssetBackedFacility],
    )?;
    Ok(())
}
