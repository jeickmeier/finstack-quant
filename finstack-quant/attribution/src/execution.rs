//! Attribution spec execution dispatch.

use super::spec::{default_attribution_metrics, AttributionResult, AttributionSpec};
use super::{attribute_pnl_metrics_based, AttributionMethod};
use finstack_quant_calibration::recalibration::CachedRecalibrationProvider;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{currency::Currency, dates::Date, money::Money, Error, Result};
use finstack_quant_valuations::instruments::model_params::ModelParamsSnapshot;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use std::sync::Arc;

#[derive(Debug)]
struct RoundingCurrencyProbe {
    currency: Option<Currency>,
    value: Option<Money>,
}

impl RoundingCurrencyProbe {
    fn successful_valuations(&self) -> usize {
        usize::from(self.value.is_some())
    }
}

/// Resolves the rounding currency and retains a successful opening valuation.
fn probe_rounding_currency(
    instrument: &dyn Instrument,
    market_t0: &MarketContext,
    as_of_t0: Date,
    rounding_scale: Option<u32>,
) -> Result<RoundingCurrencyProbe> {
    if rounding_scale.is_none() {
        return Ok(RoundingCurrencyProbe {
            currency: None,
            value: None,
        });
    }

    match instrument.value(market_t0, as_of_t0) {
        Ok(value) => Ok(RoundingCurrencyProbe {
            currency: Some(value.currency()),
            value: Some(value),
        }),
        Err(_) => Ok(RoundingCurrencyProbe {
            currency: instrument.notional()?.map(|notional| notional.currency()),
            value: None,
        }),
    }
}

/// Returns the configured target only when the completed attribution needs translation.
fn target_currency_requiring_translation(
    target_currency: Option<Currency>,
    attribution: &crate::PnlAttribution,
) -> Option<Currency> {
    target_currency.filter(|target| *target != attribution.total_pnl.currency())
}

/// Resolves the opening value used by target-currency translation.
fn translation_t0_value(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    as_of_t0: Date,
    model_params_t0: Option<&ModelParamsSnapshot>,
    rounding_probe_value: Option<Money>,
    num_repricings: &mut usize,
) -> Result<Money> {
    if model_params_t0.is_none() {
        if let Some(value) = rounding_probe_value {
            return Ok(value);
        }
    }

    let t0_instrument = match model_params_t0 {
        Some(params) => crate::model_params::with_model_params(instrument, params)?,
        None => Arc::clone(instrument),
    };
    let value = t0_instrument.value(market_t0, as_of_t0)?;
    *num_repricings += 1;
    Ok(value)
}

/// Run `f`, converting a Rust panic into [`finstack_quant_core::Error::Internal`].
///
/// Language hosts must not let an unwind cross their boundary (a WASM unwind
/// aborts the module instance; a PyO3 unwind surfaces as a `BaseException`),
/// so the containment lives here, once.
pub(crate) fn contain_panic<T>(label: &str, f: impl FnOnce() -> Result<T>) -> Result<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(panic) => {
            let message = panic
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic payload".to_string());
            Err(finstack_quant_core::Error::Internal(format!(
                "attribution panicked in {label}: {message}"
            )))
        }
    }
}

impl AttributionSpec {
    /// Execute the attribution with panic containment.
    ///
    /// Identical to [`Self::execute`] except that a Rust panic inside the
    /// pipeline is returned as [`finstack_quant_core::Error::Internal`]
    /// instead of unwinding. Language bindings call this variant.
    ///
    /// # Errors
    ///
    /// Everything [`Self::execute`] returns, plus `Error::Internal` for a
    /// contained panic.
    pub fn execute_contained(&self) -> Result<AttributionResult> {
        contain_panic("execute", || self.execute())
    }

    /// Execute the attribution specification.
    ///
    /// Returns a complete result with the P&L attribution and metadata.
    ///
    /// # Errors
    ///
    /// Propagates instrument and market reconstruction, configured rounding,
    /// pricing, FX conversion, and method-specific attribution errors. For the
    /// metrics-based method, unknown configured metric names are rejected
    /// before valuation.
    pub fn execute(&self) -> Result<AttributionResult> {
        crate::helpers::validate_attribution_period(self.as_of_t0, self.as_of_t1)?;
        let instrument = self.instrument.clone().into_boxed()?;
        let instrument_arc: Arc<dyn Instrument> = Arc::from(instrument);

        let market_t0 = MarketContext::try_from(self.market_t0.clone())?;
        let market_t1 = MarketContext::try_from(self.market_t1.clone())?;

        let rounding_scale = self
            .config
            .as_ref()
            .and_then(|config| config.rounding_scale);
        let rounding_probe = probe_rounding_currency(
            instrument_arc.as_ref(),
            &market_t0,
            self.as_of_t0,
            rounding_scale,
        )?;
        let config = self.build_finstack_config(rounding_probe.currency)?;

        let strict_validation = self
            .config
            .as_ref()
            .and_then(|c| c.strict_validation)
            .unwrap_or(super::spec::DEFAULT_STRICT_VALIDATION);
        let execution_policy = self
            .config
            .as_ref()
            .and_then(|c| c.execution_policy)
            .unwrap_or_default();

        let request = crate::AttributionRequest {
            execution_policy,
            strict_validation,
            full_cross_attribution: self.full_cross_attribution,
            model_params_t0: self.model_params_t0.as_ref(),
            credit_factor_model: self.credit_factor_model.as_deref(),
            credit_factor_detail_options: &self.credit_factor_detail_options,
            ..crate::AttributionRequest::new(
                &instrument_arc,
                &market_t0,
                &market_t1,
                self.as_of_t0,
                self.as_of_t1,
                &config,
            )
        };

        let mut attribution = match &self.method {
            AttributionMethod::Parallel
            | AttributionMethod::Waterfall(_)
            | AttributionMethod::Taylor(_) => crate::attribute_pnl(&self.method, &request)?,

            AttributionMethod::MetricsBased => {
                let instrument_t0 = match &self.model_params_t0 {
                    Some(params) => {
                        crate::model_params::with_model_params(&instrument_arc, params)?
                    }
                    None => Arc::clone(&instrument_arc),
                };
                let mut metrics_instrument = instrument_t0.clone_box();
                if let Some(overrides) = metrics_instrument.get_metric_pricing_overrides_mut() {
                    overrides.theta_period =
                        Some(format!("{}D", (self.as_of_t1 - self.as_of_t0).whole_days()));
                }
                let metrics_instrument: Arc<dyn Instrument> = Arc::from(metrics_instrument);
                let metrics = match self.config.as_ref().and_then(|cfg| cfg.metrics.as_ref()) {
                    Some(metric_names) => {
                        let mut parsed = Vec::new();
                        let mut unknown = Vec::new();
                        for name in metric_names {
                            match MetricId::parse_strict(name) {
                                Ok(id) => parsed.push(id),
                                Err(_) => unknown.push(name.clone()),
                            }
                        }
                        if !unknown.is_empty() {
                            return Err(Error::Validation(format!(
                                "Unknown metric names: {}",
                                unknown.join(", ")
                            )));
                        }
                        parsed
                    }
                    // No caller-named list: fall back to the engine's own
                    // cross-instrument menu. That menu is a superset, not a
                    // request, so narrow it to what this instrument type
                    // actually supports. A metric the caller named above stays
                    // unnarrowed and still errors if it is inapplicable,
                    // because that is a genuine request.
                    None => {
                        let registry = finstack_quant_valuations::metrics::standard_registry();
                        registry.applicable_subset(
                            &default_attribution_metrics(),
                            metrics_instrument.key(),
                        )
                    }
                };

                // Attach FinstackConfig so sensitivity bump knobs (e.g. rate_bump_bp)
                // reach the producer instead of silently falling back to defaults.
                let pricing_options =
                    finstack_quant_valuations::instruments::PricingOptions::default()
                        .with_config(&config)
                        .with_recalibration_provider(Arc::new(CachedRecalibrationProvider::new()));
                let val_t0 = metrics_instrument.price_with_metrics(
                    &market_t0,
                    self.as_of_t0,
                    &metrics,
                    pricing_options.clone(),
                )?;
                let val_t1 = instrument_arc.price_with_metrics(
                    &market_t1,
                    self.as_of_t1,
                    &[],
                    pricing_options,
                )?;

                let mut result = attribute_pnl_metrics_based(
                    &metrics_instrument,
                    &market_t0,
                    &market_t1,
                    &val_t0,
                    &val_t1,
                    self.as_of_t0,
                    self.as_of_t1,
                )?;
                result.meta.num_repricings = 2;
                if self.model_params_t0.is_some() {
                    let closing_value_opening_params =
                        instrument_t0.value(&market_t1, self.as_of_t1)?;
                    result.model_params_pnl =
                        val_t1.value.checked_sub(closing_value_opening_params)?;
                    result.meta.num_repricings += 1;
                }
                result
            }
        };
        attribution.meta.rounding = finstack_quant_core::config::rounding_context_from(&config);
        attribution.compute_residual()?;
        attribution.meta.num_repricings += rounding_probe.successful_valuations();

        if let Some(ref cfg) = self.config {
            if let Some(tol_abs) = cfg.tolerance_abs {
                attribution.meta.tolerance_abs = tol_abs;
            }
            if let Some(tol_pct) = cfg.tolerance_pct {
                attribution.meta.tolerance_pct = tol_pct;
            }
        }

        // Linear methods back-solve credit-factor detail here; reprice methods
        // populate it inside the cascade. Additive: `credit_curves_pnl` is unchanged.
        if let Some(model_ref) = &self.credit_factor_model {
            let linear_path = matches!(
                self.method,
                AttributionMethod::MetricsBased | AttributionMethod::Taylor(_)
            );
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

            if let Err(e) = self.compute_carry_credit_split_and_decomposition(
                model_ref,
                &instrument_arc,
                &market_t0,
                &mut attribution,
            ) {
                attribution.meta.notes.push(format!(
                    "credit_carry_decomposition computation failed: {e}"
                ));
            }
        }

        let configured_target = self
            .config
            .as_ref()
            .and_then(|config| config.target_currency);
        if let Some(target_currency) =
            target_currency_requiring_translation(configured_target, &attribution)
        {
            let val_t0_native = translation_t0_value(
                &instrument_arc,
                &market_t0,
                self.as_of_t0,
                self.model_params_t0.as_ref(),
                rounding_probe.value,
                &mut attribution.meta.num_repricings,
            )?;
            crate::translate_to_target_currency(
                &mut attribution,
                val_t0_native,
                target_currency,
                &market_t0,
                &market_t1,
                self.as_of_t0,
                self.as_of_t1,
            )?;
        }

        let results_meta = finstack_quant_core::config::results_meta(&config);

        Ok(AttributionResult {
            attribution,
            results_meta,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AttributionConfig;
    use finstack_quant_cashflows::builder::{
        CashFlowMeta, CashFlowSchedule, CashflowRepresentation,
    };
    use finstack_quant_cashflows::{
        schedule_from_classified_flows, CashflowScheduleSource, ScheduleBuildOpts,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::Attributes;
    use finstack_quant_valuations::instruments::model_params::ModelParamsSnapshot;
    use finstack_quant_valuations::instruments::{InstrumentJson, MarketDependencies};
    use finstack_quant_valuations::pricer::InstrumentType;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use time::macros::date;

    #[derive(Clone)]
    struct CurrencyTestInstrument {
        id: String,
        attributes: Attributes,
        value_currency: Currency,
        value_fails: bool,
        valuation_calls: Arc<AtomicUsize>,
    }

    impl CurrencyTestInstrument {
        fn successful(value_currency: Currency) -> Self {
            Self {
                id: "currency-test".to_string(),
                attributes: Attributes::default(),
                value_currency,
                value_fails: false,
                valuation_calls: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn failing() -> Self {
            Self {
                value_fails: true,
                ..Self::successful(Currency::EUR)
            }
        }

        fn valuation_calls(&self) -> usize {
            self.valuation_calls.load(Ordering::SeqCst)
        }
    }

    impl CashflowScheduleSource for CurrencyTestInstrument {
        fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
            Ok(Some(Money::from((1_000_000_i64, Currency::USD))))
        }

        fn raw_cashflow_schedule(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<CashFlowSchedule> {
            Ok(schedule_from_classified_flows(
                Vec::new(),
                DayCount::Act365F,
                ScheduleBuildOpts {
                    notional_hint: self.notional().expect("valid notional"),
                    meta: CashFlowMeta {
                        representation: CashflowRepresentation::NoResidual,
                        ..Default::default()
                    },
                },
            ))
        }
    }

    impl Instrument for CurrencyTestInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::Bond
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn attributes(&self) -> &Attributes {
            &self.attributes
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attributes
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn base_value(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            self.valuation_calls.fetch_add(1, Ordering::SeqCst);
            if self.value_fails {
                Err(finstack_quant_core::Error::Validation(
                    "intentional test valuation failure".to_string(),
                ))
            } else {
                Ok(Money::from((100_i64, self.value_currency)))
            }
        }

        fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
            Ok(MarketDependencies::new())
        }
    }

    fn spec_with_config(config: AttributionConfig) -> AttributionSpec {
        let bond = finstack_quant_valuations::instruments::Bond::example()
            .expect("test bond should build");
        let market = finstack_quant_core::market_data::context::MarketContextState::from(
            &MarketContext::new(),
        );
        AttributionSpec {
            instrument: InstrumentJson::Bond(bond),
            market_t0: market.clone(),
            market_t1: market,
            as_of_t0: date!(2025 - 01 - 01),
            as_of_t1: date!(2025 - 01 - 02),
            method: AttributionMethod::Parallel,
            model_params_t0: None,
            config: Some(config),
            credit_factor_model: None,
            credit_factor_detail_options: Default::default(),
            full_cross_attribution: false,
        }
    }

    fn config(rounding_scale: Option<u32>, target_currency: Option<Currency>) -> AttributionConfig {
        AttributionConfig {
            tolerance_abs: None,
            tolerance_pct: None,
            metrics: None,
            strict_validation: None,
            rounding_scale,
            rate_bump_bp: None,
            target_currency,
            execution_policy: None,
        }
    }

    #[test]
    fn rounding_probe_uses_value_currency_for_overrides() {
        let instrument = CurrencyTestInstrument::successful(Currency::EUR);
        let market = MarketContext::new();

        let probe = probe_rounding_currency(&instrument, &market, date!(2025 - 01 - 01), Some(6))
            .expect("valid probe_rounding_currency fixture");
        let finstack_config = spec_with_config(config(Some(6), None))
            .build_finstack_config(probe.currency)
            .expect("rounding config should build");

        assert_eq!(
            probe.value.map(|value| value.currency()),
            Some(Currency::EUR)
        );
        assert_eq!(
            finstack_config
                .rounding
                .output_scale
                .overrides
                .get(&Currency::EUR),
            Some(&6)
        );
        assert!(!finstack_config
            .rounding
            .output_scale
            .overrides
            .contains_key(&Currency::USD));
        assert_eq!(probe.successful_valuations(), 1);
        assert_eq!(instrument.valuation_calls(), 1);
    }

    #[test]
    fn target_only_skips_probe_and_uses_attribution_currency() {
        let instrument = CurrencyTestInstrument::successful(Currency::EUR);
        let market = MarketContext::new();

        let probe = probe_rounding_currency(&instrument, &market, date!(2025 - 01 - 01), None)
            .expect("valid probe_rounding_currency fixture");
        let attribution = crate::PnlAttribution::new(
            Money::from((10_i64, Currency::EUR)),
            instrument.id(),
            date!(2025 - 01 - 01),
            date!(2025 - 01 - 02),
            AttributionMethod::Parallel,
        );

        assert_eq!(instrument.valuation_calls(), 0);
        assert_eq!(probe.currency, None);
        assert_eq!(probe.value, None);
        assert_eq!(
            target_currency_requiring_translation(Some(Currency::USD), &attribution),
            Some(Currency::USD)
        );
        assert_eq!(
            target_currency_requiring_translation(Some(Currency::EUR), &attribution),
            None
        );
    }

    #[test]
    fn reusable_probe_avoids_duplicate_but_model_override_reprices() {
        let instrument = CurrencyTestInstrument::successful(Currency::EUR);
        let instrument_arc: Arc<dyn Instrument> = Arc::new(instrument.clone());
        let market = MarketContext::new();
        let probe = Money::from((100_i64, Currency::EUR));
        let mut num_repricings = 1;

        let reused = translation_t0_value(
            &instrument_arc,
            &market,
            date!(2025 - 01 - 01),
            None,
            Some(probe),
            &mut num_repricings,
        )
        .expect("probe should be reusable");

        assert_eq!(reused, probe);
        assert_eq!(instrument.valuation_calls(), 0);
        assert_eq!(num_repricings, 1);

        let repriced = translation_t0_value(
            &instrument_arc,
            &market,
            date!(2025 - 01 - 01),
            Some(&ModelParamsSnapshot::None),
            Some(probe),
            &mut num_repricings,
        )
        .expect("model-parameter path should reprice");

        assert_eq!(repriced.currency(), Currency::EUR);
        assert_eq!(instrument.valuation_calls(), 1);
        assert_eq!(num_repricings, 2);
    }

    #[test]
    fn failed_rounding_probe_falls_back_to_notional_without_counting() {
        let instrument = CurrencyTestInstrument::failing();
        let market = MarketContext::new();

        let probe = probe_rounding_currency(&instrument, &market, date!(2025 - 01 - 01), Some(6))
            .expect("valid probe_rounding_currency fixture");

        assert_eq!(probe.currency, Some(Currency::USD));
        assert_eq!(probe.value, None);
        assert_eq!(probe.successful_valuations(), 0);
        assert_eq!(instrument.valuation_calls(), 1);
    }
}
