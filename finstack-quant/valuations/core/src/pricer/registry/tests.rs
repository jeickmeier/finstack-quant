use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

// ─── Parity tests: instrument trait path vs registry path ────────────────

#[derive(Clone)]
struct InvalidTestInstrument {
    id: finstack_quant_core::types::InstrumentId,
    attributes: finstack_quant_core::types::Attributes,
    base_calls: Arc<AtomicUsize>,
    raw_calls: Arc<AtomicUsize>,
}

crate::impl_empty_cashflow_provider!(
    InvalidTestInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl Instrument for InvalidTestInstrument {
    fn includes_valuation_date_cashflows(&self) -> bool {
        true
    }
    /// Test mock: reads no market data.
    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::MarketDependencies> {
        Ok(crate::instruments::MarketDependencies::new())
    }

    crate::impl_instrument_base!(InstrumentType::Bond, *crate::pricer::Family::unregistered());

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        Err(finstack_quant_core::Error::Validation(
            "synthetic invalid instrument".to_string(),
        ))
    }

    fn base_value(
        &self,
        _market: &MarketContext,
        _as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        self.base_calls.fetch_add(1, Ordering::SeqCst);
        Ok(finstack_quant_core::money::Money::from((
            100_i64,
            finstack_quant_core::currency::Currency::USD,
        )))
    }

    fn base_value_raw(
        &self,
        _market: &MarketContext,
        _as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        self.raw_calls.fetch_add(1, Ordering::SeqCst);
        Ok(100.123_456_789)
    }
}

#[derive(Clone)]
struct RawLifecycleInstrument {
    id: finstack_quant_core::types::InstrumentId,
    attributes: finstack_quant_core::types::Attributes,
    raw_value: f64,
    effective_as_of: finstack_quant_core::dates::Date,
    raw_calls: Arc<AtomicUsize>,
    scenario: crate::instruments::pricing_overrides::ScenarioPricingOverrides,
}

crate::impl_empty_cashflow_provider!(
    RawLifecycleInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl Instrument for RawLifecycleInstrument {
    fn includes_valuation_date_cashflows(&self) -> bool {
        true
    }
    /// Test mock: reads no market data.
    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::MarketDependencies> {
        Ok(crate::instruments::MarketDependencies::new())
    }

    crate::impl_instrument_base!(InstrumentType::Bond, *crate::pricer::Family::unregistered());

    fn resolve_pricing_as_of(
        &self,
        _market: &MarketContext,
        _requested: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::dates::Date {
        self.effective_as_of
    }

    fn base_value(
        &self,
        _market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        assert_eq!(as_of, self.effective_as_of);
        Ok(finstack_quant_core::money::Money::new(
            self.raw_value,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"))
    }

    fn base_value_raw(
        &self,
        _market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        assert_eq!(as_of, self.effective_as_of);
        self.raw_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.raw_value)
    }

    fn base_value_raw_with_currency(
        &self,
        _market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<(f64, finstack_quant_core::currency::Currency)> {
        assert_eq!(as_of, self.effective_as_of);
        self.raw_calls.fetch_add(1, Ordering::SeqCst);
        Ok((self.raw_value, finstack_quant_core::currency::Currency::USD))
    }

    fn get_scenario_pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::ScenarioPricingOverrides> {
        Some(&self.scenario)
    }
}

struct CountingBondPricer {
    calls: Arc<AtomicUsize>,
}

impl Pricer for CountingBondPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Bond, ModelKey::Discounting)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        _market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<crate::results::ValuationResult, PricingError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(crate::results::ValuationResult::stamped(
            instrument.id(),
            as_of,
            finstack_quant_core::money::Money::from((
                100_i64,
                finstack_quant_core::currency::Currency::USD,
            )),
        ))
    }
}

#[test]
fn canonical_pricing_routes_validate_before_dispatch_or_model_resolution() {
    use crate::instruments::Instrument;
    use time::macros::date;

    let base_calls = Arc::new(AtomicUsize::new(0));
    let raw_calls = Arc::new(AtomicUsize::new(0));
    let pricer_calls = Arc::new(AtomicUsize::new(0));
    let instrument = InvalidTestInstrument {
        id: finstack_quant_core::types::InstrumentId::new("INVALID-TEST"),
        attributes: finstack_quant_core::types::Attributes::default(),
        base_calls: Arc::clone(&base_calls),
        raw_calls: Arc::clone(&raw_calls),
    };
    let market = MarketContext::new();
    let as_of = date!(2025 - 01 - 15);
    let mut registry = PricerRegistry::new();
    registry
        .register(CountingBondPricer {
            calls: Arc::clone(&pricer_calls),
        })
        .expect("unique test pricer registration");

    let registry_err = registry
        .price_with_metrics(
            &instrument,
            ModelKey::Discounting,
            &market,
            as_of,
            &[],
            crate::instruments::PricingOptions::default(),
        )
        .expect_err("invalid registry request must fail validation");
    assert!(matches!(registry_err, PricingError::InvalidInput { .. }));

    let metric_err = registry
        .price_with_metrics(
            &instrument,
            ModelKey::Discounting,
            &market,
            as_of,
            &[crate::metrics::MetricId::Dv01],
            crate::instruments::PricingOptions::default(),
        )
        .expect_err("invalid metric request must fail validation");
    assert!(matches!(metric_err, PricingError::InvalidInput { .. }));

    let raw_err = registry
        .price_raw(&instrument, ModelKey::Discounting, &market, as_of)
        .expect_err("invalid raw request must fail validation");
    assert!(matches!(raw_err, PricingError::InvalidInput { .. }));

    let unknown_model_err = registry
        .price_with_metrics(
            &instrument,
            ModelKey::HazardRate,
            &market,
            as_of,
            &[],
            crate::instruments::PricingOptions::default(),
        )
        .expect_err("validation must precede unknown-model resolution");
    assert!(matches!(
        unknown_model_err,
        PricingError::InvalidInput { .. }
    ));

    let registry = Arc::new(registry);
    let trait_err = instrument
        .price_with_metrics(
            &market,
            as_of,
            &[],
            crate::instruments::PricingOptions::default().with_registry(registry),
        )
        .expect_err("trait pricing request must fail validation");
    assert!(matches!(trait_err, PricingError::InvalidInput { .. }));

    let value_err = instrument
        .value(&market, as_of)
        .expect_err("direct value request must fail validation");
    assert!(matches!(
        value_err,
        finstack_quant_core::Error::Validation(_)
    ));

    let raw_value_err = instrument
        .value_raw(&market, as_of)
        .expect_err("direct raw value request must fail validation");
    assert!(matches!(
        raw_value_err,
        finstack_quant_core::Error::Validation(_)
    ));

    let raw_currency_err = instrument
        .value_raw_with_currency(&market, as_of)
        .expect_err("currency-aware raw value request must fail validation");
    assert!(matches!(
        raw_currency_err,
        finstack_quant_core::Error::Validation(_)
    ));

    assert_eq!(pricer_calls.load(Ordering::SeqCst), 0);
    assert_eq!(base_calls.load(Ordering::SeqCst), 0);
    assert_eq!(raw_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn raw_lifecycle_preserves_precision_resolves_date_and_applies_scenario_once() {
    use crate::instruments::Instrument;
    use time::macros::date;

    let requested_as_of = date!(2025 - 01 - 15);
    let effective_as_of = date!(2025 - 01 - 17);
    let raw_value = 123.456_789_123;
    let raw_calls = Arc::new(AtomicUsize::new(0));
    let instrument = RawLifecycleInstrument {
        id: finstack_quant_core::types::InstrumentId::new("RAW-LIFECYCLE"),
        attributes: finstack_quant_core::types::Attributes::default(),
        raw_value,
        effective_as_of,
        raw_calls: Arc::clone(&raw_calls),
        scenario: crate::instruments::pricing_overrides::ScenarioPricingOverrides::default()
            .with_scenario_price_shock_decimal(-0.10),
    };
    let market = MarketContext::new();
    let mut registry = PricerRegistry::new();
    registry
        .register(crate::instruments::pricing::GenericInstrumentPricer::<
            RawLifecycleInstrument,
        >::discounting(InstrumentType::Bond))
        .expect("unique test pricer registration");

    let base_raw = instrument
        .base_value_raw(&market, effective_as_of)
        .expect("unchecked raw kernel");
    let direct_raw = instrument
        .value_raw(&market, requested_as_of)
        .expect("direct raw lifecycle");
    let (direct_raw_with_currency, direct_currency) = instrument
        .value_raw_with_currency(&market, requested_as_of)
        .expect("currency-aware direct raw lifecycle");
    let registry_raw = registry
        .price_raw(&instrument, ModelKey::Discounting, &market, requested_as_of)
        .expect("registry raw lifecycle");
    let result = registry
        .price_with_metrics(
            &instrument,
            ModelKey::Discounting,
            &market,
            requested_as_of,
            &[],
            crate::instruments::PricingOptions::default(),
        )
        .expect("registry money lifecycle");
    let expected_money = result.value;
    let mut metric_context = crate::metrics::MetricContext::new(
        Arc::new(instrument),
        Arc::new(market.clone()),
        effective_as_of,
        expected_money,
        crate::metrics::MetricContext::default_config(),
    );
    metric_context.set_pricer_dispatch(PricingDispatch::registered(
        ModelKey::Discounting,
        Arc::new(registry),
    ));
    let metric_reprice = metric_context
        .reprice_money(&market, requested_as_of)
        .expect("metric scenario lifecycle");

    assert_eq!(base_raw, raw_value);
    let expected = raw_value * 0.9;
    assert_eq!(direct_raw, expected);
    assert_eq!(direct_raw_with_currency, expected);
    assert_eq!(
        direct_currency,
        finstack_quant_core::currency::Currency::USD
    );
    assert_eq!(registry_raw, expected);
    assert_eq!(result.as_of, effective_as_of);
    assert_eq!(metric_reprice, expected_money);
    assert_eq!(raw_calls.load(Ordering::SeqCst), 4);
}
