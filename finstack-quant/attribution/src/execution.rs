//! Attribution spec execution dispatch.

use super::parallel::attribute_pnl_parallel_with_credit_model;
use super::spec::{default_attribution_metrics, AttributionResult, AttributionSpec};
use super::waterfall::attribute_pnl_waterfall_with_credit_model;
use super::{attribute_pnl_metrics_based, attribute_pnl_taylor, AttributionMethod};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;

impl AttributionSpec {
    /// Execute the attribution specification.
    ///
    /// Returns a complete result with the P&L attribution and metadata.
    pub fn execute(&self) -> Result<AttributionResult> {
        // Reconstruct instrument from JSON
        let instrument = self.instrument.clone().into_boxed()?;
        let instrument_arc: std::sync::Arc<dyn Instrument> = std::sync::Arc::from(instrument);

        // Reconstruct market contexts
        let market_t0 = MarketContext::try_from(self.market_t0.clone())?;
        let market_t1 = MarketContext::try_from(self.market_t1.clone())?;

        // Determine instrument currency for config (avoids hardcoding USD)
        let instrument_ccy = instrument_arc
            .value(&market_t0, self.as_of_t0)
            .ok()
            .map(|m| m.currency());

        // Build config (defaults unless overridden)
        let config = self.build_finstack_config(instrument_ccy)?;

        // Determine strict validation
        let strict_validation = self
            .config
            .as_ref()
            .and_then(|c| c.strict_validation)
            .unwrap_or(false);
        let execution_policy = self
            .config
            .as_ref()
            .and_then(|c| c.execution_policy)
            .unwrap_or_default();

        // Resolve optional credit-factor model for waterfall/parallel cascade.
        // Borrow the boxed model directly — the cascade entry points take
        // `Option<&CreditFactorModel>`, so deref-borrowing avoids deep-cloning
        // the entire model (issuer betas, covariance, hierarchy, diagnostics)
        // on every spec execution.
        let resolved_credit_model = self.credit_factor_model.as_deref();

        // Execute attribution based on method
        let mut attribution = match &self.method {
            AttributionMethod::Parallel => attribute_pnl_parallel_with_credit_model(
                &instrument_arc,
                &market_t0,
                &market_t1,
                self.as_of_t0,
                self.as_of_t1,
                &config,
                self.model_params_t0.as_ref(),
                resolved_credit_model,
                &self.credit_factor_detail_options,
                self.full_cross_attribution,
                execution_policy,
            )?,

            AttributionMethod::Waterfall(order) => attribute_pnl_waterfall_with_credit_model(
                &instrument_arc,
                &market_t0,
                &market_t1,
                self.as_of_t0,
                self.as_of_t1,
                &config,
                order.clone(),
                strict_validation,
                self.model_params_t0.as_ref(),
                resolved_credit_model,
                &self.credit_factor_detail_options,
            )?,

            AttributionMethod::Taylor(ref taylor_config) => attribute_pnl_taylor(
                &instrument_arc,
                &market_t0,
                &market_t1,
                self.as_of_t0,
                self.as_of_t1,
                taylor_config,
                execution_policy,
            )?,

            AttributionMethod::MetricsBased => {
                // Determine metrics to use
                let metrics = if let Some(ref cfg) = self.config {
                    if let Some(ref metric_names) = cfg.metrics {
                        let mut parsed = Vec::new();
                        let mut unknown = Vec::new();

                        for name in metric_names {
                            match MetricId::parse_strict(name) {
                                Ok(id) => parsed.push(id),
                                Err(_) => unknown.push(name.clone()),
                            }
                        }

                        if !unknown.is_empty() {
                            return Err(finstack_quant_core::Error::Validation(format!(
                                "Unknown metric names: {}",
                                unknown.join(", ")
                            )));
                        }

                        parsed
                    } else {
                        default_attribution_metrics()
                    }
                } else {
                    default_attribution_metrics()
                };

                // Compute valuations with metrics
                let val_t0 = instrument_arc.price_with_metrics(
                    &market_t0,
                    self.as_of_t0,
                    &metrics,
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )?;
                let val_t1 = instrument_arc.price_with_metrics(
                    &market_t1,
                    self.as_of_t1,
                    &metrics,
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )?;

                attribute_pnl_metrics_based(
                    &instrument_arc,
                    &market_t0,
                    &market_t1,
                    &val_t0,
                    &val_t1,
                    self.as_of_t0,
                    self.as_of_t1,
                )?
            }
        };

        // Apply tolerance overrides if provided
        if let Some(ref cfg) = self.config {
            if let Some(tol_abs) = cfg.tolerance_abs {
                attribution.meta.tolerance_abs = tol_abs;
            }
            if let Some(tol_pct) = cfg.tolerance_pct {
                attribution.meta.tolerance_pct = tol_pct;
            }
        }

        // Optional: credit-factor hierarchy decomposition of credit_curves_pnl.
        // Wired for MetricsBased and Taylor (PR-7). Other methods leave the
        // field None; they will be wired in PR-8a/b. The existing
        // `credit_curves_pnl` field is unchanged numerically — this is purely
        // additive detail.
        if let Some(model_ref) = &self.credit_factor_model {
            let linear_path = matches!(
                self.method,
                AttributionMethod::MetricsBased | AttributionMethod::Taylor(_)
            );
            // PR-8a: Parallel and Waterfall now populate `credit_factor_detail`
            // internally via the per-step credit cascade. The linear methods
            // (PR-7) still go through the back-solve in
            // `compute_credit_factor_detail`.
            if linear_path {
                let mut detail_notes: Vec<String> = Vec::new();
                match self.compute_credit_factor_detail(
                    model_ref,
                    &instrument_arc,
                    &market_t0,
                    &market_t1,
                    &attribution,
                    &mut detail_notes,
                ) {
                    Ok(Some(detail)) => {
                        attribution.credit_factor_detail = Some(detail);
                        // The detail back-solve performs 2 CS01 repricings.
                        attribution.meta.num_repricings += 2;
                    }
                    Ok(None) => {
                        if detail_notes.is_empty() {
                            attribution.meta.notes.push(
                                "credit_factor_model supplied but no resolvable issuer/CS01 \
                                 on instrument; credit_factor_detail omitted"
                                    .into(),
                            );
                        }
                    }
                    Err(e) => {
                        attribution
                            .meta
                            .notes
                            .push(format!("credit_factor_detail computation failed: {e}"));
                    }
                }
                attribution.meta.notes.extend(detail_notes);
            }
            // For Parallel / Waterfall methods, the detail (if any) is already
            // populated inside the method itself.

            // PR-8b: split coupon_income / roll_down into rates / credit parts
            // and emit `credit_carry_decomposition` (the second lens, §7.2).
            // Best-effort: failures fall back to leaving the existing scalar
            // CarryDetail untouched and append a diagnostic note.
            //
            // All four methods populate `carry_detail` (parallel / waterfall /
            // Taylor via `apply_total_return_carry`; metrics-based from the
            // carry decomposition metrics), so the split is attempted on every
            // path — the decomposition logic is method-agnostic.
            match self.compute_carry_credit_split_and_decomposition(
                model_ref,
                &instrument_arc,
                &market_t0,
                &mut attribution,
            ) {
                Ok(()) => {}
                Err(e) => attribution.meta.notes.push(format!(
                    "credit_carry_decomposition computation failed: {e}"
                )),
            }
        }

        // Item 2: optional target-currency translation. Runs as a final
        // post-processing step so direct callers of the per-method functions
        // keep their existing native-currency behavior; only the JSON-spec
        // pipeline (used by the bindings) picks up `target_ccy`.
        if let Some(target_ccy) = self.config.as_ref().and_then(|c| c.target_ccy) {
            if let Some(instr_ccy) = instrument_ccy {
                if target_ccy != instr_ccy {
                    // Re-price T0 in native currency to obtain val_t0 for the
                    // translation formula. We can't reuse the per-method
                    // val_t0 because it's not surfaced; the extra reprice is
                    // cheap relative to a full attribution run.
                    //
                    // Note: when T0 model parameters were
                    // supplied, the methods priced their val_t0 with the
                    // T0-PARAMETER instrument — the translation must use the
                    // same instrument or `fx_translation_pnl` and the
                    // recovered val_t1 drift by the parameter-induced
                    // valuation gap.
                    let t0_instrument = match self.model_params_t0.as_ref() {
                        Some(params) => {
                            match crate::model_params::with_model_params(&instrument_arc, params) {
                                Ok(inst) => inst,
                                Err(e) => {
                                    attribution.meta.notes.push(format!(
                                        "target_ccy translation: T0 model-parameter application \
                                         failed ({e}); using T1-parameter instrument for val_t0"
                                    ));
                                    std::sync::Arc::clone(&instrument_arc)
                                }
                            }
                        }
                        None => std::sync::Arc::clone(&instrument_arc),
                    };
                    match t0_instrument.value(&market_t0, self.as_of_t0) {
                        Ok(val_t0_native) => {
                            attribution.meta.num_repricings += 1;
                            match crate::translate_to_target_ccy(
                                &mut attribution,
                                val_t0_native,
                                target_ccy,
                                &market_t0,
                                &market_t1,
                                self.as_of_t0,
                                self.as_of_t1,
                            ) {
                                Ok(()) => {}
                                Err(e) => attribution
                                    .meta
                                    .notes
                                    .push(format!("target_ccy translation failed: {e}")),
                            }
                        }
                        Err(e) => attribution.meta.notes.push(format!(
                            "target_ccy translation skipped: T0 reprice failed - {e}"
                        )),
                    }
                }
            }
        }

        // The currency-detection probe at the top of `execute` performed one
        // full valuation; account for it so `num_repricings` reflects true
        // pricing cost.
        if instrument_ccy.is_some() {
            attribution.meta.num_repricings += 1;
        }

        // Create results metadata
        let results_meta = finstack_quant_core::config::results_meta(&config);

        Ok(AttributionResult {
            attribution,
            results_meta,
        })
    }
}
