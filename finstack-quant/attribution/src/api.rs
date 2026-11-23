//! Top-level attribution request and public entry points ([`attribute_pnl`], [`pnl_bridge`]).

use crate::{
    AttributionMethod, CreditFactorDetailOptions, ExecutionPolicy, ModelParamsSnapshot,
    PnlAttribution, credit_decomposition, metrics_based, parallel, pnl_support, spec, taylor,
    translate_to_target_currency, waterfall,
};
use finstack_quant_core::config::{FinstackConfig, RoundingContext};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_factor::credit::hierarchy::CreditFactorModel;
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use std::sync::Arc;

/// Inputs shared by the repricing-based attribution methods.
///
/// Construct with [`AttributionRequest::new`] and override the optional
/// fields with struct-update syntax:
///
/// ```rust,ignore
/// let request = AttributionRequest {
///     execution_policy: ExecutionPolicy::Parallel,
///     ..AttributionRequest::new(&instrument, &market_t0, &market_t1, t0, t1, &config)
/// };
/// ```
#[derive(Clone, Copy)]
pub struct AttributionRequest<'a> {
    /// Instrument to attribute (the T₁ instrument when model parameters moved).
    pub instrument: &'a Arc<dyn Instrument>,
    /// Opening market state.
    pub market_t0: &'a MarketContext,
    /// Closing market state.
    pub market_t1: &'a MarketContext,
    /// Opening valuation date.
    pub as_of_t0: Date,
    /// Closing valuation date.
    pub as_of_t1: Date,
    /// Finstack configuration (rounding context, sensitivity bump sizes).
    /// Taylor uses this rounding context and its own [`TaylorAttributionConfig`](crate::TaylorAttributionConfig)
    /// for numerical bump sizes.
    pub config: &'a FinstackConfig,
    /// Scheduling policy for independent factor repricings. Waterfall is
    /// always serial and stamps `Serial` regardless of this value.
    pub execution_policy: ExecutionPolicy,
    /// Waterfall only: fail when a factor cannot be isolated instead of
    /// recording a warning. Defaults to `true`.
    pub strict_validation: bool,
    /// Parallel only: also compute the full cross-factor decomposition.
    pub full_cross_attribution: bool,
    /// Opening model-parameter snapshot; `None` takes parameters from
    /// `instrument`, so model-parameter P&L is zero.
    pub model_params_t0: Option<&'a ModelParamsSnapshot>,
    /// Credit-factor model for the per-issuer hierarchy detail: the
    /// waterfall and parallel methods reprice a cascade, the metrics-based
    /// and Taylor methods back-solve it from a measured CS01, and every
    /// method splits the carry into rates and credit parts.
    pub credit_factor_model: Option<&'a CreditFactorModel>,
    /// Detail options for the credit-factor cascade output.
    pub credit_factor_detail_options: &'a CreditFactorDetailOptions,
    /// Unscaled instrument values at `(T₀, T₁)` already priced by a portfolio
    /// engine; when `Some`, the two ordinary endpoint repricings are skipped.
    pub prepared_endpoints: Option<(Money, Money)>,
    /// Reporting currency. When it differs from the instrument's pricing
    /// currency the result is translated with
    /// [`translate_to_target_currency`]; `None` reports in the pricing
    /// currency.
    pub target_currency: Option<Currency>,
    /// Absolute residual tolerance stamped into `meta.tolerance_abs`;
    /// `None` keeps the method's default.
    pub tolerance_abs: Option<f64>,
    /// Percentage residual tolerance stamped into `meta.tolerance_pct`;
    /// `None` keeps the method's default.
    pub tolerance_pct: Option<f64>,
    /// Metrics-based only: the metrics to price at T₀. `None` prices the
    /// default menu narrowed to what the instrument supports, and drops the
    /// quote-space credit metrics with a note when that menu fails on a
    /// hazard curve that has no calibration recipe; a named metric is not
    /// narrowed and errors if it is inapplicable or fails.
    pub metrics: Option<&'a [MetricId]>,
    /// Metrics-based only: pricing options for the T₀ metrics and the T₁
    /// value, with `config` attached. `None` uses the calibration crate's
    /// cached recalibration provider.
    pub pricing_options: Option<&'a PricingOptions>,
}

impl<'a> AttributionRequest<'a> {
    /// Build a request with default optional fields: serial execution,
    /// strict waterfall validation, no cross-factor detail, no model-parameter
    /// snapshot, no credit-factor model, default detail options, no
    /// prepared endpoints, the pricing currency, the method's default
    /// tolerances, the default metric menu and the default pricing options.
    ///
    /// # Arguments
    ///
    /// * `instrument` - Instrument to reprice at both dates and under every
    ///   factor restoration.
    /// * `market_t0` - Opening market state used for the T₀ value and factor
    ///   restorations.
    /// * `market_t1` - Closing market state used for the T₁ value.
    /// * `as_of_t0` - Opening valuation date.
    /// * `as_of_t1` - Closing valuation date; must not precede `as_of_t0`.
    /// * `config` - Finstack configuration whose rounding context is stamped
    ///   into the result and whose sensitivity extension sets bump sizes.
    #[must_use]
    pub fn new(
        instrument: &'a Arc<dyn Instrument>,
        market_t0: &'a MarketContext,
        market_t1: &'a MarketContext,
        as_of_t0: Date,
        as_of_t1: Date,
        config: &'a FinstackConfig,
    ) -> Self {
        Self {
            instrument,
            market_t0,
            market_t1,
            as_of_t0,
            as_of_t1,
            config,
            execution_policy: ExecutionPolicy::Serial,
            strict_validation: spec::DEFAULT_STRICT_VALIDATION,
            full_cross_attribution: false,
            model_params_t0: None,
            credit_factor_model: None,
            credit_factor_detail_options: &CreditFactorDetailOptions::DEFAULT,
            prepared_endpoints: None,
            target_currency: None,
            tolerance_abs: None,
            tolerance_pct: None,
            metrics: None,
            pricing_options: None,
        }
    }
}

/// Run one attribution method on a request.
///
/// After the method runs, the result carries the request's rounding
/// context and tolerances and a freshly computed residual; with a
/// credit-factor model it gains the credit-factor detail and the
/// carry's rates/credit split, and with a `target_currency` it is
/// translated into that currency. [`AttributionSpec::execute`](crate::AttributionSpec::execute) and the
/// language bindings run this same pipeline.
///
/// # Arguments
///
/// * `method` - Attribution methodology. `MetricsBased` prices both endpoints
///   with `request.metrics` or the default metric menu the instrument
///   supports (without quote-space credit metrics, and with a note, when an
///   instrument hazard curve has no calibration recipe); call [`attribute_pnl_metrics_based`](crate::attribute_pnl_metrics_based) directly when the
///   [`finstack_quant_valuations::results::ValuationResult`]s are already priced.
/// * `request` - Instrument, market states, dates, configuration and the
///   optional overrides described on [`AttributionRequest`].
///
/// # Errors
///
/// Returns method-specific validation, market-data, repricing, currency and
/// FX-translation errors, and an error when the residual cannot be
/// computed (factor amounts in mixed currencies).
pub fn attribute_pnl(
    method: &AttributionMethod,
    request: &AttributionRequest<'_>,
) -> finstack_quant_core::Result<PnlAttribution> {
    pnl_support::validate_attribution_period(request.as_of_t0, request.as_of_t1)?;
    let mut attribution = match method {
        AttributionMethod::Parallel => parallel::attribute_pnl_parallel(request)?,
        AttributionMethod::Waterfall(order) => {
            waterfall::attribute_pnl_waterfall(request, order.clone())?
        }
        AttributionMethod::Taylor(taylor_config) => {
            taylor::attribute_pnl_taylor(request, taylor_config)?
        }
        AttributionMethod::MetricsBased => metrics_based::attribute_request(request)?,
    };
    attribution.meta.rounding = RoundingContext::from(request.config);
    attribution.compute_residual()?;
    if let Some(tolerance) = request.tolerance_abs {
        attribution.meta.tolerance_abs = tolerance;
    }
    if let Some(tolerance) = request.tolerance_pct {
        attribution.meta.tolerance_pct = tolerance;
    }

    if let Some(model) = request.credit_factor_model {
        // Repricing methods fill the detail inside their cascade; the linear
        // methods back-solve it here. Additive: `credit_curves_pnl` is unchanged.
        if matches!(
            method,
            AttributionMethod::MetricsBased | AttributionMethod::Taylor(_)
        ) {
            let mut notes = Vec::new();
            match credit_decomposition::credit_factor_detail(
                request,
                model,
                &attribution,
                &mut notes,
            ) {
                Ok(Some(detail)) => {
                    attribution.credit_factor_detail = Some(detail);
                    attribution.meta.num_repricings += 2;
                }
                Ok(None) => {}
                Err(e) => notes.push(format!("credit_factor_detail computation failed: {e}")),
            }
            attribution.meta.notes.extend(notes);
        }
        if let Err(e) =
            credit_decomposition::carry_credit_decomposition(request, model, &mut attribution)
        {
            attribution.meta.notes.push(format!(
                "credit_carry_decomposition computation failed: {e}"
            ));
        }
    }

    if let Some(target_currency) = request
        .target_currency
        .filter(|target| *target != attribution.total_pnl.currency())
    {
        let val_t0 = match attribution.pv_t0 {
            Some(value) => value,
            None => {
                attribution.meta.num_repricings += 1;
                pnl_support::instrument_at_t0(request.instrument, request.model_params_t0)?
                    .value(request.market_t0, request.as_of_t0)?
            }
        };
        translate_to_target_currency(
            &mut attribution,
            val_t0,
            target_currency,
            request.market_t0,
            request.market_t1,
            request.as_of_t0,
            request.as_of_t1,
        )?;
    }
    Ok(attribution)
}

/// Minimal, no-frills P&L bridge: `value(T₁) − value(T₀)`.
///
/// This is the **cheapest** attribution entry point — it prices the
/// instrument once at each date in each market state and returns the
/// scalar total P&L in `target_currency`. FX conversion uses `market_t0` for
/// the T₀ value and `market_t1` for the T₁ value. Use it when you just
/// need the headline number and don't care which
/// factors contributed. For a factor-level decomposition, reach for
/// [`attribute_pnl`] or [`attribute_pnl_metrics_based`](crate::attribute_pnl_metrics_based).
///
/// This is intentionally a thin wrapper over direct repricing plus
/// date-matched FX conversion: the function is cheap, it allocates no scratch
/// buffers, and it contains no factor iteration. Benchmark the heavier
/// methodologies against this baseline to quantify the cost of factor
/// attribution.
///
/// # Arguments
///
/// * `instrument` - Instrument to reprice at both valuation dates.
/// * `market_t0` - Opening market state used to calculate the T₀ value and
///   opening FX conversion.
/// * `market_t1` - Closing market state used to calculate the T₁ value and
///   closing FX conversion.
/// * `as_of_t0` - Opening valuation date used for the T₀ repricing.
/// * `as_of_t1` - Closing valuation date used for the T₁ repricing.
/// * `target_currency` - Currency to report P&L in; FX is resolved from the
///   date-specific market contexts.
///
/// # Returns
///
/// The total P&L `v_t1 − v_t0` in `target_currency`.
///
/// # Errors
///
/// Returns an error if either repricing call fails or if the FX
/// conversion cannot be resolved from the provided market contexts.
///
/// # Examples
///
/// ```no_run
/// use finstack_quant_attribution::pnl_bridge;
/// use finstack_quant_valuations::instruments::Instrument;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use std::sync::Arc;
/// use time::macros::date;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let instrument: Arc<dyn Instrument> = unimplemented!("obtain the instrument under test");
/// let market_t0 = MarketContext::new();
/// let market_t1 = MarketContext::new();
///
/// let pnl = pnl_bridge(
///     &instrument,
///     &market_t0,
///     &market_t1,
///     date!(2025 - 01 - 15),
///     date!(2025 - 01 - 16),
///     Currency::USD,
/// )?;
/// println!("Daily P&L: {pnl}");
/// # Ok(())
/// # }
/// ```
pub fn pnl_bridge(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    target_currency: Currency,
) -> finstack_quant_core::Result<Money> {
    let v_t0 = instrument.value(market_t0, as_of_t0)?;
    let v_t1 = instrument.value(market_t1, as_of_t1)?;
    pnl_support::compute_pnl_with_fx(
        v_t0,
        v_t1,
        target_currency,
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
    )
}
