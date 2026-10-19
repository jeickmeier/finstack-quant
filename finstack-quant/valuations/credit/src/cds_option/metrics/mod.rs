//! CDS Option metrics module.
//!
//! Provides metric calculators specific to `CdsOption`, split into focused
//! files. The calculators compose with the shared metrics framework and are
//! registered via `register_cds_option_metrics`.
//!
//! Exposed metrics:
//! - Delta, Gamma, Vega, Theta
//! - CS01 (quoted CDS spread sensitivity)
//! - DV01 (swap-curve quote sensitivity; canonical IR sensitivity)
//! - ParSpread (Bloomberg CDSO ATM forward spread in bp)
//! - Implied Volatility (placeholder)

pub(crate) mod delta;
mod dv01;
pub(crate) mod gamma;
mod recovery01;

use crate::cds_option::bloomberg_quadrature;
use crate::cds_option::pricer;
use crate::cds_option::pricer::resolve_sigma;
use crate::cds_option::pricer::synthetic_underlying_cds;
use crate::cds_option::CdsOption;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::common_impl::traits::Instrument;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;
use finstack_quant_valuations_core::recalibration::HazardRecalibrationAction;
use finstack_quant_valuations_core::recalibration::HazardRecalibrationRequest;
use finstack_quant_valuations_core::recalibration::QuoteBump;

/// Per-deal CS01 conventions for `CdsOption`.
///
/// Drives the generic credit CS01 calculator
/// ([`crate::cs01::CreditParallelCs01`]). The CDS
/// option does not carry its own doc clause / valuation convention; CS01 is a
/// quote-spread risk measured against the *synthetic underlying CDS*, so the
/// conventions are read off `synthetic_underlying_cds`.
///
/// `cs01_precheck` short-circuits CS01 to `0.0` once the option has expired
/// (no spread risk remains). Before expiry, standard CS01 requires a lossless
/// hazard calibration recipe and measures a par-spread bump with hazard
/// re-bootstrap. Directly specified hazard curves are rejected rather than
/// silently interpreted as quote-spread risk.
///
impl crate::cs01::CdsCs01Conventions for crate::cds_option::CdsOption {
    fn cs01_bootstrap_convention(
        &self,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<(
        finstack_quant_valuations_core::market::conventions::ids::CdsDocClause,
        crate::cds::CdsValuationConvention,
    )> {
        // CS01 is a quote-spread risk measured against the synthetic
        // underlying CDS; its doc clause and valuation convention drive the
        // hazard re-bootstrap.
        let synthetic = crate::cds_option::pricer::synthetic_underlying_cds(self, as_of)?;
        Ok((
            crate::cds::metrics::market_doc_clause(&synthetic),
            synthetic.valuation_convention,
        ))
    }

    fn cs01_precheck(
        &self,
        context: &finstack_quant_valuations_core::metrics::MetricContext,
        _hazard_id: &finstack_quant_core::types::CurveId,
    ) -> finstack_quant_core::Result<Option<f64>> {
        let as_of = context.as_of;
        if as_of >= self.expiry {
            tracing::debug!(
                instrument_id = %self.id,
                as_of = %as_of,
                expiry = %self.expiry,
                "CDS Option CS01: Instrument already expired, returning 0.0"
            );
            return Ok(Some(0.0));
        }

        // The shared standard-CS01 engine enforces the lossless replay-recipe
        // requirement for live options.
        Ok(None)
    }
}

/// Register all CDS Option metrics with the registry
pub(crate) fn register_cds_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CdsOption,
        metrics: [
            (Delta, delta::DeltaCalculator),
            (Gamma, gamma::GammaCalculator),
            (Vega, VegaCalculator),
            (Cs01, crate::cs01::CreditParallelCs01::<
                crate::cds_option::CdsOption,
            >::default()),
            (BucketedCs01, crate::cs01::CreditBucketedCs01::<
                crate::cds_option::CdsOption,
            >::default()),
            (SpreadDv01, UnderlyingSpreadDv01Calculator),
            (Dv01, dv01::CdsOptionDv01Calculator),
            (Theta, ThetaCalculator),
            (ParSpread, ParSpreadCalculator),
            (ImpliedVol, ImpliedVolCalculator),
            (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            // Recovery01 (custom metric - recovery rate sensitivity)
            (Recovery01, recovery01::Recovery01Calculator),
        ]
    }
    Ok(())
}

/// Implied volatility metric for credit options on CDS spreads.
///
/// Computes the Black-on-spreads implied volatility that matches the observed
/// option premium in `instrument_pricing_overrides.market_quotes.quoted_premium`
/// using the CDS option pricer and core math solvers (HybridSolver).
pub(crate) struct ImpliedVolCalculator;

impl finstack_quant_valuations_core::metrics::MetricCalculator for ImpliedVolCalculator {
    fn calculate(
        &self,
        context: &mut finstack_quant_valuations_core::metrics::MetricContext,
    ) -> finstack_quant_core::Result<f64> {
        let option: &CdsOption = context.instrument_as()?;
        let target_price = option
            .instrument_pricing_overrides
            .market_quotes
            .required_quoted_premium()?;
        option.implied_vol(&context.curves, context.as_of, target_price, None)
    }
}

/// Black forward CDS spread in basis points.
pub(crate) struct ParSpreadCalculator;

impl MetricCalculator for ParSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CdsOption = context.instrument_as()?;
        pricer::forward_spread_bp(option, &context.curves, context.as_of)
    }
}

/// Spread DV01 calculator for the option's synthetic underlying CDS.
///
/// Reports the Bloomberg CDSW-style Spread DV01 of the synthetic underlying CDS
/// linked from a CDS option, not the option's own CS01. This follows the CDSW
/// screen convention: apply a +1bp parallel bump to the quoted CDS par-spread
/// curve, re-bootstrap the hazard curve, and revalue the underlying CDS.
pub(crate) struct UnderlyingSpreadDv01Calculator;

impl MetricCalculator for UnderlyingSpreadDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CdsOption = context.instrument_as()?;
        let cds = synthetic_underlying_cds(option, context.as_of)?;
        let hazard = context.curves.get_hazard(&option.credit_curve_id)?;
        finstack_quant_valuations_core::metrics::sensitivities::cs01::require_hazard_replay(
            hazard.as_ref(),
            "CDS option underlying spread DV01",
        )?;
        let bumped_hazard = context.rebuild_hazard_curve(
            HazardRecalibrationRequest {
                hazard: std::sync::Arc::clone(&hazard),
                source_market: std::sync::Arc::clone(&context.curves),
                target_market: std::sync::Arc::clone(&context.curves),
                discount_curve_id: option.discount_curve_id.clone(),
                doc_clause: None,
                cds_valuation_convention: None,
                deal_quote_override: None,
                action: HazardRecalibrationAction::SpreadBump(QuoteBump::ParallelBp(1.0)),
            },
            "cs01",
        )?;
        let bumped_market = context
            .curves
            .as_ref()
            .clone()
            .insert(bumped_hazard.as_ref().clone());
        let base_pv = cds.value(&context.curves, context.as_of)?.amount();
        let bumped_pv = cds.value(&bumped_market, context.as_of)?.amount();
        Ok(bumped_pv - base_pv)
    }
}

/// CDS-option theta calculator using the CDSO pricer's settlement-aware time convention.
pub(crate) struct ThetaCalculator;

impl MetricCalculator for ThetaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CdsOption = context.instrument_as()?;
        option.theta(&context.curves, context.as_of)
    }
}

const VEGA_VOL_BUMP: f64 = 0.01;

/// Vega calculator for credit options on CDS spreads.
///
/// Vega(1%) is the change in option premium for a `+0.01` (one absolute
/// percentage point) shift in the lognormal spread volatility, computed
/// as a one-sided forward difference through the canonical Bloomberg
/// CDSO numerical-quadrature pricer (DOCS 2055833 §2.5).
pub(crate) struct VegaCalculator;

impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CdsOption = context.instrument_as()?;
        vega(option, &context.curves, context.as_of)
    }
}

/// Bloomberg CDSO Vega(1%) — one-sided forward difference of the
/// canonical Bloomberg quadrature NPV on a `+0.01` lognormal-vol bump.
pub(crate) fn vega(
    option: &CdsOption,
    curves: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
) -> Result<f64> {
    option.validate_supported_configuration()?;
    let base_sigma = resolve_sigma(option, curves, as_of)?;
    let bumped_sigma = base_sigma + VEGA_VOL_BUMP;
    if bumped_sigma > super::types::MAX_IMPLIED_VOL {
        return Err(finstack_quant_core::Error::Validation(format!(
            "vega bump pushes implied vol above ceiling: base={base_sigma} bumped={bumped_sigma}"
        )));
    }
    let cds = synthetic_underlying_cds(option, as_of)?;
    let pv_base = bloomberg_quadrature::npv(option, &cds, curves, base_sigma, as_of)?.amount();
    let pv_bumped = bloomberg_quadrature::npv(option, &cds, curves, bumped_sigma, as_of)?.amount();
    Ok(pv_bumped - pv_base)
}
