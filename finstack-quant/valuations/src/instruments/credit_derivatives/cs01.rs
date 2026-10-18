//! Credit-convention-aware CS01 for the credit-derivative family.
//!
//! The generic bump-and-reprice CS01 in the metrics core shifts a hazard
//! curve in place. A CDS-family instrument instead re-bootstraps its hazard
//! curve from bumped par quotes under its own doc clause and valuation
//! convention, which [`CdsCs01Conventions`] supplies; single-name CDS add a
//! hazard-reprice cache so the key-rate series reuses one premium-leg
//! decomposition.

use std::marker::PhantomData;

use crate::instruments::common_impl::traits::Instrument;
use crate::market::conventions::ids::CdsDocClause;
use crate::market::conventions::CdsValuationConvention;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::sensitivities::cs01::{
    compute_key_rate_cs01_series_with_context_raw, compute_parallel_cs01_with_context_raw,
    cs01_reval, resolve_optional_cs01_curves, Cs01Request,
};
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use crate::recalibration::DealCdsQuoteOverride;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;

/// Per-deal CS01 conventions supplied by credit instruments (CDS, CDS Option).
///
/// The generic [`GenericParallelCs01`] / [`GenericBucketedCs01`] calculators
/// re-bootstrap the hazard curve under fixed `doc_clause: None` /
/// `cds_valuation_convention: None`. For genuine credit instruments those are
/// **per-deal** quote-convention inputs to the par-spread→hazard bootstrap, so
/// they must be read off the specific instrument at `calculate()` time rather
/// than baked into a per-type calculator.
///
/// This trait collects every credit-specific CS01 input so the
/// [`CreditParallelCs01`] / [`CreditBucketedCs01`] calculators can fully
/// reproduce the behaviour of the former bespoke `cds` / `cds_option` CS01
/// calculators with byte-identical output.
pub(crate) trait CdsCs01Conventions {
    /// Doc clause and valuation convention used to re-bootstrap the hazard
    /// curve from par spreads.
    ///
    /// `as_of` is supplied so instruments that derive the convention from a
    /// date-dependent synthetic underlying (e.g. a CDS option) can build it
    /// against the actual valuation date.
    fn cs01_bootstrap_convention(
        &self,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<(CdsDocClause, CdsValuationConvention)>;

    /// Optional pre-check run before the CS01 compute.
    ///
    /// Returning `Ok(Some(v))` short-circuits CS01 to `v` (e.g. a CDS option
    /// past expiry reports `0.0`). Returning `Err(..)` surfaces a hard
    /// validation/calibration error (e.g. a CDS option whose hazard curve
    /// carries no par-spread points). `Ok(None)` proceeds normally.
    fn cs01_precheck(
        &self,
        _context: &MetricContext,
        _hazard_id: &CurveId,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(None)
    }

    /// Optional deal-level clean-spread override passed to the provider.
    fn cs01_deal_quote_override(&self) -> Option<DealCdsQuoteOverride> {
        None
    }
}

/// Resolved CDS spread-risk inputs shared by CS01 and CS-Gamma.
pub(crate) struct PreparedCdsRiskContext {
    pub(crate) hazard_id: CurveId,
    pub(crate) discount_id: CurveId,
    pub(crate) doc_clause: CdsDocClause,
    pub(crate) valuation_convention: CdsValuationConvention,
    pub(crate) deal_quote_override: Option<DealCdsQuoteOverride>,
}

impl PreparedCdsRiskContext {
    /// Build the quote-space CS01 request for this deal's conventions.
    pub(crate) fn cs01_request(&self, bump_bp: f64) -> Cs01Request {
        Cs01Request {
            bump_bp,
            discount_curve_id: self.discount_id.clone(),
            doc_clause: Some(self.doc_clause),
            cds_valuation_convention: Some(self.valuation_convention),
            deal_quote_override: self.deal_quote_override,
        }
    }
}

/// Resolve CDS spread-risk inputs, apply any deal-quote market override, run
/// the calculation, and restore the original market on both success and error.
pub(crate) fn with_prepared_cds_risk_context<I>(
    context: &mut MetricContext,
    missing_credit_curve_value: Option<f64>,
    operation: &str,
    calculate: impl FnOnce(
        &mut MetricContext,
        &PreparedCdsRiskContext,
    ) -> finstack_quant_core::Result<f64>,
) -> finstack_quant_core::Result<f64>
where
    I: Instrument + CdsCs01Conventions + 'static,
{
    let instrument: &I = context.instrument_as()?;
    let Some((hazard_id, discount_id)) =
        resolve_optional_cs01_curves(instrument, missing_credit_curve_value.is_some(), operation)?
    else {
        return Ok(missing_credit_curve_value.unwrap_or(0.0));
    };

    if let Some(value) = context
        .instrument_as::<I>()?
        .cs01_precheck(context, &hazard_id)?
    {
        return Ok(value);
    }

    let discount_id = discount_id.ok_or_else(|| finstack_quant_core::Error::Calibration {
        message: format!(
            "{operation} requires the calibration discount curve for hazard curve '{}'",
            hazard_id.as_str()
        ),
        category: "cs01_rebootstrap".to_string(),
    })?;
    let (doc_clause, valuation_convention) = context
        .instrument_as::<I>()?
        .cs01_bootstrap_convention(context.as_of)?;
    let deal_quote_override = context.instrument_as::<I>()?.cs01_deal_quote_override();

    let prepared = PreparedCdsRiskContext {
        hazard_id,
        discount_id,
        doc_clause,
        valuation_convention,
        deal_quote_override,
    };
    calculate(context, &prepared)
}

/// Credit-convention-aware parallel CS01 calculator.
///
/// Behaves like [`GenericParallelCs01`] but reads the per-deal `doc_clause`
/// and `cds_valuation_convention` (and optional pre-check / curve override)
/// from the instrument via [`CdsCs01Conventions`]. Used by `CreditDefaultSwap`
/// and `CdsOption`.
pub(crate) struct CreditParallelCs01<I> {
    _phantom: PhantomData<I>,
}

impl<I> Default for CreditParallelCs01<I> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<I> MetricCalculator for CreditParallelCs01<I>
where
    I: Instrument + CdsCs01Conventions + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        with_prepared_cds_risk_context::<I>(context, None, "CS01", |context, prepared| {
            let bump_bp = sens_config::from_context_or_default(
                context.get_config(),
                context.get_metric_pricing_overrides(),
            )?
            .credit_spread_bump_bp;
            let reval = cs01_reval(context);
            let request = prepared.cs01_request(bump_bp);
            let cs01 = compute_parallel_cs01_with_context_raw(
                context,
                &prepared.hazard_id,
                &request,
                reval,
            )?;
            context.computed.insert(
                MetricId::composite(&MetricId::Cs01, &[prepared.hazard_id.as_str()]),
                cs01,
            );
            Ok(cs01)
        })
    }
}

/// Credit-convention-aware bucketed (key-rate) CS01 calculator.
///
/// Behaves like [`GenericBucketedCs01`] but reads the per-deal CDS bootstrap
/// convention from the instrument via [`CdsCs01Conventions`].
pub(crate) struct CreditBucketedCs01<I> {
    _phantom: PhantomData<I>,
}

impl<I> Default for CreditBucketedCs01<I> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<I> MetricCalculator for CreditBucketedCs01<I>
where
    I: Instrument + CdsCs01Conventions + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        with_prepared_cds_risk_context::<I>(context, None, "bucketed CS01", |context, prepared| {
            let defaults = sens_config::from_context_or_default(
                context.get_config(),
                context.get_metric_pricing_overrides(),
            )?;
            let bump_bp = defaults.credit_spread_bump_bp;
            let series_id =
                MetricId::composite(&MetricId::BucketedCs01, &[prepared.hazard_id.as_str()]);
            let cds_cache = match context.instrument_as::<
                    crate::instruments::credit_derivatives::cds::CreditDefaultSwap,
                >() {
                    Ok(cds) => Some(
                        crate::instruments::credit_derivatives::cds::pricing::CdsHazardRepriceCache::try_new(
                            cds,
                            context.curves.as_ref(),
                            context.as_of,
                        )?,
                    ),
                    Err(_) => None,
                };
            let request = prepared.cs01_request(bump_bp);
            if let Some(cache) = cds_cache {
                let hazard_key = prepared.hazard_id.clone();
                compute_key_rate_cs01_series_with_context_raw(
                    context,
                    &prepared.hazard_id,
                    series_id,
                    &request,
                    move |temp_ctx: &MarketContext| {
                        let surv = temp_ctx.get_hazard(hazard_key.as_str())?;
                        cache.npv(surv.as_ref())
                    },
                )
            } else {
                let reval = cs01_reval(context);
                compute_key_rate_cs01_series_with_context_raw(
                    context,
                    &prepared.hazard_id,
                    series_id,
                    &request,
                    reval,
                )
            }
        })
    }
}
