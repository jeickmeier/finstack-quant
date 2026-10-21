//! Attribution spec execution dispatch.

use crate::spec::{AttributionInputs, AttributionResult, AttributionSpec};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{currency::Currency, dates::Date, money::Money, Error, Result};
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

/// Parse configured metric names, rejecting every unknown name at once.
fn parse_metric_names(names: &[String]) -> Result<Vec<MetricId>> {
    let (parsed, unknown): (Vec<_>, Vec<_>) = names
        .iter()
        .map(|name| MetricId::parse_strict(name).map_err(|_| name.as_str()))
        .partition(|parsed| parsed.is_ok());
    if !unknown.is_empty() {
        let unknown: Vec<&str> = unknown.into_iter().filter_map(|name| name.err()).collect();
        return Err(Error::Validation(format!(
            "Unknown metric names: {}",
            unknown.join(", ")
        )));
    }
    Ok(parsed.into_iter().filter_map(|id| id.ok()).collect())
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
    /// Execute the attribution specification.
    ///
    /// Returns a complete result with the P&L attribution and metadata. A Rust
    /// panic inside the pipeline is returned as an error instead of unwinding,
    /// so language bindings can call this directly.
    ///
    /// # Errors
    ///
    /// Propagates instrument and market reconstruction, configured rounding,
    /// pricing, FX conversion, and method-specific attribution errors. For the
    /// metrics-based method, unknown configured metric names are rejected
    /// before valuation. A contained panic is
    /// [`finstack_quant_core::Error::Internal`].
    pub fn execute(&self) -> Result<AttributionResult> {
        contain_panic("execute", || {
            crate::helpers::validate_attribution_period(
                self.inputs.as_of_t0,
                self.inputs.as_of_t1,
            )?;
            let market_t0 = MarketContext::try_from(self.inputs.market_t0.clone())?;
            let market_t1 = MarketContext::try_from(self.inputs.market_t1.clone())?;
            self.inputs
                .execute_instrument(&self.instrument, &market_t0, &market_t1)
        })
    }
}

impl AttributionInputs {
    pub(crate) fn execute_instrument(
        &self,
        instrument: &finstack_quant_valuations::instruments::InstrumentJson,
        market_t0: &MarketContext,
        market_t1: &MarketContext,
    ) -> Result<AttributionResult> {
        let instrument_arc: Arc<dyn Instrument> = Arc::from(instrument.clone().into_boxed()?);
        let rounding_scale = self
            .config
            .as_ref()
            .and_then(|config| config.rounding_scale);
        let rounding_probe = probe_rounding_currency(
            instrument_arc.as_ref(),
            market_t0,
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

        // A metric the caller named stays unnarrowed and still errors if it is
        // inapplicable, because that is a genuine request.
        let metrics = self
            .config
            .as_ref()
            .and_then(|config| config.metrics.as_deref())
            .map(parse_metric_names)
            .transpose()?;
        let request = crate::AttributionRequest {
            execution_policy,
            strict_validation,
            full_cross_attribution: self.full_cross_attribution,
            model_params_t0: self.model_params_t0.as_ref(),
            credit_factor_model: self.credit_factor_model.as_deref(),
            credit_factor_detail_options: &self.credit_factor_detail_options,
            target_currency: self
                .config
                .as_ref()
                .and_then(|config| config.target_currency),
            tolerance_abs: self.config.as_ref().and_then(|config| config.tolerance_abs),
            tolerance_pct: self.config.as_ref().and_then(|config| config.tolerance_pct),
            metrics: metrics.as_deref(),
            ..crate::AttributionRequest::new(
                &instrument_arc,
                market_t0,
                market_t1,
                self.as_of_t0,
                self.as_of_t1,
                &config,
            )
        };

        let mut attribution = crate::attribute_pnl(&self.method, &request)?;
        attribution.meta.num_repricings += rounding_probe.successful_valuations();

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
    use crate::AttributionMethod;
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
        let market = finstack_quant_core::market_data::context::MarketContextState::try_from(
            &MarketContext::new(),
        )
        .expect("coherent market snapshot");
        AttributionSpec {
            instrument: InstrumentJson::Bond(bond),
            inputs: crate::AttributionInputs {
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
            },
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
            .inputs
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
                .get_overrides()
                .get(&Currency::EUR),
            Some(&6)
        );
        assert!(!finstack_config
            .rounding
            .output_scale
            .get_overrides()
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
        assert_eq!(instrument.valuation_calls(), 0);
        assert_eq!(probe.currency, None);
        assert_eq!(probe.value, None);
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
