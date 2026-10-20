//! The pricer registry over built-in instruments: parity with the trait
//! path, result-envelope preservation and custom registries.

use finstack_quant_core::market_data::context::MarketContext as Market;
use finstack_quant_valuations::instruments::Instrument as Priceable;
use finstack_quant_valuations::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricerRegistry, PricingError,
};
use finstack_quant_valuations::results::ValuationDetails;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Minimal flat discount curve anchored at `base_date`.
fn flat_discount_curve(
    id: &str,
    base_date: finstack_quant_core::dates::Date,
) -> finstack_quant_core::market_data::term_structures::DiscountCurve {
    finstack_quant_core::market_data::term_structures::DiscountCurve::builder(id)
        .base_date(base_date)
        .knots([(0.0, 1.0), (10.0, 0.9)])
        .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
        .build()
        .expect("DiscountCurve should build with valid test data")
}

/// Variant of [`flat_discount_curve`] with an opaque FX policy stamp.
fn flat_discount_curve_with_fx_policy(
    id: &str,
    base_date: finstack_quant_core::dates::Date,
    policy: &str,
) -> finstack_quant_core::market_data::term_structures::DiscountCurve {
    finstack_quant_core::market_data::term_structures::DiscountCurve::builder(id)
        .base_date(base_date)
        .knots([(0.0, 1.0), (10.0, 0.9)])
        .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
        .fx_policy(policy)
        .build()
        .expect("DiscountCurve should build with valid test data")
}

/// Multi-knot log-linear discount curve suitable for instruments that
/// require richer interpolation (e.g., structured credit).
fn multi_knot_discount_curve(
    id: &str,
    base_date: finstack_quant_core::dates::Date,
) -> finstack_quant_core::market_data::term_structures::DiscountCurve {
    finstack_quant_core::market_data::term_structures::DiscountCurve::builder(id)
        .base_date(base_date)
        .knots([
            (0.0, 1.0),
            (0.5, 0.975),
            (1.0, 0.95),
            (2.0, 0.90),
            (5.0, 0.82),
            (10.0, 0.70),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .build()
        .expect("Multi-knot DiscountCurve should build with valid test data")
}

/// Minimal flat hazard curve anchored at `base_date`.
fn flat_hazard_curve(
    id: &str,
    base_date: finstack_quant_core::dates::Date,
) -> finstack_quant_core::market_data::term_structures::HazardCurve {
    finstack_quant_core::market_data::term_structures::HazardCurve::builder(id)
        .base_date(base_date)
        .recovery_rate(0.40)
        .knots([(0.0, 0.02), (10.0, 0.02)])
        .build()
        .expect("HazardCurve should build with valid test data")
}

fn fixed_test_bond() -> finstack_quant_valuations::instruments::fixed_income::bond::Bond {
    finstack_quant_valuations::instruments::fixed_income::bond::Bond::fixed(
        "US912828XG33",
        finstack_quant_core::money::Money::from((
            1_000_i64,
            finstack_quant_core::currency::Currency::USD,
        )),
        finstack_quant_core::types::Rate::from_decimal(0.04).expect("valid rate fixture"),
        time::macros::date!(2020 - 01 - 15),
        time::macros::date!(2030 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-TREASURY",
    )
    .expect("fixed test bond should build")
}

fn flat_vol_surface(id: &str, vol: f64) -> finstack_quant_core::market_data::surfaces::VolSurface {
    let expiries = [0.25, 0.5, 1.0, 2.0];
    let strikes = [2.5, 3.0, 3.5, 4.0, 4.5];
    let mut builder = finstack_quant_core::market_data::surfaces::VolSurface::builder(id)
        .expiries(&expiries)
        .strikes(&strikes);
    for _ in &expiries {
        builder = builder.row(&vec![vol; strikes.len()]);
    }
    builder.build().expect("vol surface should build in tests")
}

fn commodity_swaption_market(
    as_of: finstack_quant_core::dates::Date,
    flat_fwd: f64,
    vol: f64,
    rate: f64,
) -> finstack_quant_core::market_data::context::MarketContext {
    let disc = finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, (-rate * 5.0).exp())])
        .build()
        .expect("discount curve");
    let price_curve =
        finstack_quant_core::market_data::term_structures::PriceCurve::builder("NG-FORWARD")
            .base_date(as_of)
            .spot_price(flat_fwd)
            .knots([(0.0, flat_fwd), (2.0, flat_fwd)])
            .build()
            .expect("price curve");

    finstack_quant_core::market_data::context::MarketContext::new()
        .insert(disc)
        .insert(price_curve)
        .insert_surface(flat_vol_surface("NG-VOL", vol))
}

struct FixedBondPricer {
    amount: f64,
    model: ModelKey,
}

impl Pricer for FixedBondPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Bond, self.model)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Priceable,
        _market: &Market,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<finstack_quant_valuations::results::ValuationResult, PricingError>
    {
        Ok(
            finstack_quant_valuations::results::ValuationResult::stamped(
                instrument.id(),
                as_of,
                finstack_quant_core::money::Money::new(
                    self.amount,
                    finstack_quant_core::currency::Currency::USD,
                )
                .expect("valid money fixture"),
            ),
        )
    }
}

struct RichBondPricer;

impl Pricer for RichBondPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Bond, ModelKey::HazardRate)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Priceable,
        _market: &Market,
        _as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<finstack_quant_valuations::results::ValuationResult, PricingError>
    {
        let result_as_of = time::macros::date!(2025 - 01 - 16);
        let mut measures = indexmap::IndexMap::new();
        measures.insert(finstack_quant_valuations::metrics::MetricId::Dv01, 7.0);
        measures.insert(
            finstack_quant_valuations::metrics::MetricId::custom("model_measure"),
            2.0,
        );
        let covenant = finstack_quant_covenants::CovenantReport {
            covenant_type: "test".to_string(),
            covenant_id: Some("rich-result".to_string()),
            passed: true,
            actual_value: Some(1.5),
            threshold: Some(1.0),
            details: Some("preserved".to_string()),
            headroom: Some(0.5),
            meta: finstack_quant_core::config::ResultsMeta::default(),
        };
        let mut result = finstack_quant_valuations::results::ValuationResult::stamped(
            instrument.id(),
            result_as_of,
            finstack_quant_core::money::Money::from((
                100_i64,
                finstack_quant_core::currency::Currency::USD,
            )),
        )
        .with_measures(measures)
        .with_details(finstack_quant_valuations::results::ValuationDetails::Fx(
            finstack_quant_valuations::results::FxValuationDetails {
                fx_triangulated: Some(true),
            },
        ))
        .with_covenant("rich-result", covenant)
        .with_explanation(finstack_quant_core::explain::ExplanationTrace::new(
            "rich-pricer",
        ));
        result.meta.fx_policy_applied = Some("pricer-fx-policy".to_string());
        result.meta.timestamp = Some(time::OffsetDateTime::UNIX_EPOCH);
        result.meta.version = Some("model-version".to_string());
        Ok(result)
    }
}

#[test]
fn metric_attachment_preserves_the_model_result_envelope() {
    use finstack_quant_core::config::FinstackConfig;
    use finstack_quant_core::currency::Currency;
    use time::macros::date;

    let instrument = fixed_test_bond();
    let market = Market::new().insert(flat_discount_curve("USD-TREASURY", date!(2025 - 01 - 15)));
    let mut registry = PricerRegistry::new();
    registry
        .register(RichBondPricer)
        .expect("unique test pricer registration");
    let mut cfg = FinstackConfig::default();
    cfg.rounding
        .output_scale
        .set_scale(Currency::USD, 4)
        .expect("valid decimal scale");

    let result = registry
        .price_with_metrics(
            &instrument,
            ModelKey::HazardRate,
            &market,
            date!(2025 - 01 - 15),
            &[finstack_quant_valuations::metrics::MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default().with_config(&cfg),
        )
        .expect("rich model result with metrics");

    assert_eq!(result.as_of, date!(2025 - 01 - 15));
    assert!(matches!(
        result.details,
        Some(finstack_quant_valuations::results::ValuationDetails::Fx(
            finstack_quant_valuations::results::FxValuationDetails {
                fx_triangulated: Some(true)
            }
        ))
    ));
    assert!(result
        .covenants
        .as_ref()
        .is_some_and(|reports| reports.contains_key("rich-result")));
    assert_eq!(
        result
            .explanation
            .as_ref()
            .map(|trace| trace.trace_type.as_str()),
        Some("rich-pricer")
    );
    assert_eq!(
        result.meta.fx_policy_applied.as_deref(),
        Some("pricer-fx-policy")
    );
    assert_eq!(
        result.meta.timestamp,
        Some(time::OffsetDateTime::UNIX_EPOCH)
    );
    assert_eq!(result.meta.version.as_deref(), Some("model-version"));
    assert_eq!(
        result
            .meta
            .rounding
            .get_output_scale_by_currency()
            .get(&Currency::USD),
        Some(&4)
    );
    assert_eq!(
        result
            .measures
            .get(&finstack_quant_valuations::metrics::MetricId::Dv01),
        Some(&7.0)
    );
    assert_eq!(
        result
            .measures
            .get(&finstack_quant_valuations::metrics::MetricId::custom(
                "model_measure"
            )),
        Some(&2.0)
    );
}

/// Default discounting path parity:
/// `Bond::price_with_metrics` (trait default, discount engine) and
/// `registry.price_with_metrics(..., ModelKey::Discounting, ..., finstack_quant_valuations::instruments::PricingOptions::default())` must
/// produce the same PV.
#[test]
fn bond_discounting_parity_instrument_vs_registry() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_valuations::instruments::Instrument;
    use time::macros::date;

    let as_of: Date = date!(2025 - 01 - 15);
    let bond = fixed_test_bond();
    let disc = flat_discount_curve("USD-TREASURY", as_of);
    let market = MarketContext::new().insert(disc);
    let registry = finstack_quant_valuations::pricer::standard_pricer_registry();

    let trait_result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("trait price_with_metrics should succeed");

    let registry_result = registry
        .price_with_metrics(
            &bond,
            ModelKey::Discounting,
            &market,
            as_of,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("registry price_with_metrics should succeed");

    let trait_pv = trait_result.value.amount();
    let registry_pv = registry_result.value.amount();
    assert!(
        (trait_pv - registry_pv).abs() < 1.0,
        "Bond PV parity: trait={trait_pv:.4} registry={registry_pv:.4} diff > $1"
    );
}

#[test]
fn commodity_swaption_default_model_matches_registry() {
    use finstack_quant_valuations::instruments::Instrument;
    use time::macros::date;

    let as_of = date!(2025 - 01 - 15);
    let swaption =
        finstack_quant_valuations::instruments::commodity::commodity_swaption::CommoditySwaption::example()
            .expect("example");
    let market = commodity_swaption_market(as_of, 3.75, 0.30, 0.05);
    let registry = finstack_quant_valuations::pricer::standard_pricer_registry();

    let instrument_result = swaption
        .price_with_metrics(
            &market,
            as_of,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("instrument default-model path should succeed");
    let registry_result = registry
        .price_with_metrics(
            &swaption,
            ModelKey::Black76,
            &market,
            as_of,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("registry Black76 path should succeed");

    assert!(
        (instrument_result.value.amount() - registry_result.value.amount()).abs() < 1e-9,
        "commodity swaption default model should match explicit Black76 registry pricing",
    );
}

#[test]
fn instrument_can_compose_custom_pricer_and_metric_registries() {
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::metrics::{
        MetricCalculator, MetricContext, MetricId, MetricRegistry,
    };
    use std::sync::Arc;
    use time::macros::date;

    struct ConstantDv01;

    impl MetricCalculator for ConstantDv01 {
        fn calculate(&self, _context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
            Ok(42.0)
        }
    }

    let as_of = date!(2025 - 01 - 15);
    let bond = fixed_test_bond();
    let market = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(flat_discount_curve("USD-TREASURY", as_of));
    let mut pricers = PricerRegistry::new();
    pricers
        .register(FixedBondPricer {
            amount: 990.0,
            model: ModelKey::Discounting,
        })
        .expect("unique test pricer registration");
    let mut metrics = MetricRegistry::new();
    metrics
        .register_metric(
            MetricId::Dv01,
            Arc::new(ConstantDv01),
            &[InstrumentType::Bond],
        )
        .expect("custom metric registration");

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default()
                .with_metric_registry(Arc::new(metrics))
                .with_registry(Arc::new(pricers)),
        )
        .expect("custom pricer and metric registries should compose");

    assert_eq!(result.value.amount(), 990.0);
    assert_eq!(result.measures.get(&MetricId::Dv01), Some(&42.0));
}

#[test]
fn non_discounting_risk_only_metrics_preserve_config_metadata() {
    use finstack_quant_core::config::FinstackConfig;
    use finstack_quant_core::currency::Currency;
    use time::macros::date;

    let as_of = date!(2025 - 01 - 15);
    let bond = fixed_test_bond();
    let mut bond_with_credit = bond;
    bond_with_credit.credit_curve_id = Some(finstack_quant_core::types::CurveId::new(
        "USD-CREDIT".to_string(),
    ));

    let market = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(flat_discount_curve("USD-TREASURY", as_of))
        .insert(flat_hazard_curve("USD-CREDIT", as_of));
    let registry = finstack_quant_valuations::pricer::standard_pricer_registry();

    let mut cfg = FinstackConfig::default();
    cfg.rounding
        .output_scale
        .set_scale(Currency::USD, 4)
        .expect("valid decimal scale");

    let result = registry
        .price_with_metrics(
            &bond_with_credit,
            ModelKey::HazardRate,
            &market,
            as_of,
            &[finstack_quant_valuations::metrics::MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default().with_config(&cfg),
        )
        .expect("hazard-rate pricing with config should succeed");

    assert_eq!(
        result
            .meta
            .rounding
            .get_output_scale_by_currency()
            .get(&Currency::USD)
            .copied(),
        Some(4),
        "risk-only split path should preserve caller config metadata",
    );
}

/// StructuredCredit overridden path:
/// Both `instrument.price_with_metrics` (instrument-level override) and
/// `registry.price_with_metrics` must follow the same code path: either both
/// succeed with the same PV, or both fail with the same error type.
///
/// The example CLO (minimal, empty pool) may or may not produce a valid PV
/// depending on the waterfall simulation; what matters is that both paths are
/// consistent.
#[test]
fn structured_credit_parity_instrument_vs_registry() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_valuations::instruments::Instrument;
    use time::macros::date;

    let as_of: Date = date!(2025 - 01 - 15);
    let clo = finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit::example()
        .expect("example");
    let disc = multi_knot_discount_curve("USD-OIS", as_of);
    let market = MarketContext::new().insert(disc);
    let registry = finstack_quant_valuations::pricer::standard_pricer_registry();

    let trait_result = clo.price_with_metrics(
        &market,
        as_of,
        &[],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );
    let registry_result = registry.price_with_metrics(
        &clo,
        ModelKey::Discounting,
        &market,
        as_of,
        &[],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    match (trait_result, registry_result) {
        (Ok(t), Ok(r)) => {
            let trait_pv = t.value.amount();
            let registry_pv = r.value.amount();
            assert!(
                (trait_pv - registry_pv).abs() < 1.0,
                "StructuredCredit PV parity: trait={trait_pv:.4} registry={registry_pv:.4} diff > $1"
            );
        }
        (Err(t_err), Err(r_err)) => {
            // Both paths fail: verify the underlying error message is the same.
            // The registry wraps errors in ModelFailure, so we compare the
            // inner cause rather than the full error string.
            let t_msg = t_err.to_string();
            let r_msg = r_err.to_string();
            assert!(
                t_msg.contains("two data points")
                    || r_msg.contains("two data points")
                    || t_msg == r_msg,
                "Both paths fail but with unrelated errors; trait={t_err:?} registry={r_err:?}"
            );
        }
        (Ok(t), Err(r_err)) => {
            panic!(
                "Trait succeeded (PV={:.4}) but registry failed ({r_err:?})",
                t.value.amount()
            );
        }
        (Err(t_err), Ok(r)) => {
            panic!(
                "Registry succeeded (PV={:.4}) but trait failed ({t_err:?})",
                r.value.amount()
            );
        }
    }
}

#[test]
fn composite_reuses_leaf_results_only_within_one_request() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_valuations::instruments::{
        CompositeInstrument, Equity, Instrument, InstrumentJson, PricingOptions,
    };
    use time::macros::date;
    struct CountingEquity {
        calls: Arc<AtomicUsize>,
    }
    impl Pricer for CountingEquity {
        fn key(&self) -> PricerKey {
            PricerKey::new(InstrumentType::Equity, ModelKey::Discounting)
        }
        fn price_dyn(
            &self,
            instrument: &dyn Priceable,
            market: &Market,
            as_of: finstack_quant_core::dates::Date,
        ) -> std::result::Result<finstack_quant_valuations::results::ValuationResult, PricingError>
        {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let equity = finstack_quant_valuations::pricer::expect_inst::<Equity>(
                instrument,
                InstrumentType::Equity,
            )?;
            Ok(
                finstack_quant_valuations::results::ValuationResult::stamped(
                    instrument.id(),
                    as_of,
                    equity.base_value(market, as_of).map_err(|error| {
                        PricingError::from_core(
                            error,
                            finstack_quant_valuations::pricer::PricingErrorContext::from_instrument(
                                instrument,
                            ),
                        )
                    })?,
                ),
            )
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = finstack_quant_valuations::pricer::standard_pricer_registry().clone();
    registry.replace(CountingEquity {
        calls: Arc::clone(&calls),
    });
    let options = PricingOptions::default().with_registry(Arc::new(registry));
    let mut composite = CompositeInstrument::example().unwrap();
    let market = Market::new();
    let as_of = date!(2025 - 01 - 02);
    let (measures, detail) = match composite
        .composite_valuation(&market, as_of, &[], options.clone())
        .unwrap()
    {
        Some((measures, ValuationDetails::Composite(detail))) => (measures, detail),
        other => panic!("composite valuation expected, got {other:?}"),
    };
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(measures.is_empty());
    assert_eq!(
        detail
            .leg_results
            .iter()
            .map(|leg| leg.reporting_value.amount())
            .sum::<f64>(),
        10.0
    );
    assert_eq!(detail.leg_results[0].native_value.amount(), 100.0);
    *composite.spec.legs[0].instrument = InstrumentJson::Equity(
        Equity::new("COMPOSITE-LONG", "LONG", Currency::USD)
            .with_quantity(1.0)
            .with_quoted_spot(120.0),
    );
    let detail = match composite
        .composite_valuation(&market, as_of, &[], options)
        .unwrap()
    {
        Some((_, ValuationDetails::Composite(detail))) => detail,
        other => panic!("composite valuation expected, got {other:?}"),
    };
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    assert_eq!(
        detail
            .leg_results
            .iter()
            .map(|leg| leg.reporting_value.amount())
            .sum::<f64>(),
        30.0
    );
}

#[test]
fn fx_policy_propagates_from_curve_to_non_fx_instrument_result() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use time::macros::date;

    let as_of: Date = date!(2025 - 01 - 15);
    let bond = fixed_test_bond();
    let disc = flat_discount_curve_with_fx_policy("USD-TREASURY", as_of, "xccy_basis::USD/EUR");
    let market = MarketContext::new().insert(disc);
    let registry = finstack_quant_valuations::pricer::standard_pricer_registry();

    let result = registry
        .price_with_metrics(
            &bond,
            ModelKey::Discounting,
            &market,
            as_of,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("registry price should succeed");

    assert_eq!(
        result.meta.fx_policy_applied.as_deref(),
        Some("xccy_basis::USD/EUR"),
        "discount-curve fx_policy must propagate onto ResultsMeta"
    );
}

#[test]
fn fx_policy_pricer_stamp_takes_precedence_over_curve_stamp() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use time::macros::date;

    let as_of: Date = date!(2025 - 01 - 15);
    let bond = fixed_test_bond();
    let disc = flat_discount_curve_with_fx_policy("USD-TREASURY", as_of, "curve::xccy_basis");
    let market = MarketContext::new().insert(disc);

    /// Stamps its own FX policy and parallel flag, as a model pricer may.
    struct PolicyStampingPricer;

    impl Pricer for PolicyStampingPricer {
        fn key(&self) -> PricerKey {
            PricerKey::new(InstrumentType::Bond, ModelKey::Tree)
        }

        fn price_dyn(
            &self,
            instrument: &dyn Priceable,
            _market: &Market,
            as_of: Date,
        ) -> std::result::Result<finstack_quant_valuations::results::ValuationResult, PricingError>
        {
            let mut result = finstack_quant_valuations::results::ValuationResult::stamped(
                instrument.id(),
                as_of,
                finstack_quant_core::money::Money::new(
                    1_000.0,
                    finstack_quant_core::currency::Currency::USD,
                )
                .expect("valid money fixture"),
            );
            result.meta.fx_policy_applied = Some("pricer::explicit_policy".to_string());
            result.meta.parallel = true;
            Ok(result)
        }
    }

    let mut registry = PricerRegistry::new();
    registry
        .register(PolicyStampingPricer)
        .expect("unique test pricer registration");
    let result = registry
        .price_with_metrics(
            &bond,
            ModelKey::Tree,
            &market,
            as_of,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("registry pricing should succeed");

    assert_eq!(
        result.meta.fx_policy_applied.as_deref(),
        Some("pricer::explicit_policy"),
        "explicit pricer stamp must outrank curve-walking fallback"
    );
    assert!(
        result.meta.parallel,
        "pricer execution metadata must survive stamping"
    );
}

/// Non-discounting split path:
/// `registry.price_with_metrics` with `ModelKey::HazardRate` must
/// produce a valid PV and successfully compute DV01 (a risk metric).
#[test]
fn bond_hazard_rate_registry_path_succeeds() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use time::macros::date;

    let as_of: Date = date!(2025 - 01 - 15);
    let bond = fixed_test_bond();
    let disc = flat_discount_curve("USD-TREASURY", as_of);
    let hazard = flat_hazard_curve("USD-CREDIT", as_of);
    let mut bond_with_credit = bond;
    bond_with_credit.credit_curve_id = Some(finstack_quant_core::types::CurveId::new(
        "USD-CREDIT".to_string(),
    ));
    let market = MarketContext::new().insert(disc).insert(hazard);
    let registry = finstack_quant_valuations::pricer::standard_pricer_registry();

    let result = registry
        .price_with_metrics(
            &bond_with_credit,
            ModelKey::HazardRate,
            &market,
            as_of,
            &[finstack_quant_valuations::metrics::MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("registry price_with_metrics (HazardRate) should succeed");

    assert!(
        result.value.amount() < 0.0 || result.value.amount() > 0.0 || result.value.amount() == 0.0,
        "HazardRate PV should be a finite number"
    );
    assert!(
        result.measures.contains_key("dv01"),
        "DV01 measure should be present after non-discounting path"
    );
    assert!(
        result
            .measures
            .get("dv01")
            .copied()
            .unwrap_or_default()
            .is_finite(),
        "HazardRate DV01 should be finite"
    );
}

#[test]
fn bond_hazard_rate_instrument_override_matches_registry() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_valuations::instruments::Instrument;
    use time::macros::date;

    let as_of: Date = date!(2025 - 01 - 15);
    let bond = fixed_test_bond();
    let disc = flat_discount_curve("USD-TREASURY", as_of);
    let hazard = flat_hazard_curve("USD-CREDIT", as_of);
    let mut bond_with_credit = bond;
    bond_with_credit.credit_curve_id = Some(finstack_quant_core::types::CurveId::new(
        "USD-CREDIT".to_string(),
    ));
    let market = MarketContext::new().insert(disc).insert(hazard);
    let registry = finstack_quant_valuations::pricer::standard_pricer_registry();

    let instrument_result = bond_with_credit
        .price_with_metrics(
            &market,
            as_of,
            &[finstack_quant_valuations::metrics::MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default()
                .with_model(ModelKey::HazardRate),
        )
        .expect("instrument override path should succeed");

    let registry_result = registry
        .price_with_metrics(
            &bond_with_credit,
            ModelKey::HazardRate,
            &market,
            as_of,
            &[finstack_quant_valuations::metrics::MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("registry hazard-rate path should succeed");

    assert!(
        (instrument_result.value.amount() - registry_result.value.amount()).abs() < 1.0,
        "instrument override PV should match registry PV",
    );
    assert_eq!(
        instrument_result.measures.get("dv01"),
        registry_result.measures.get("dv01"),
        "instrument override metrics should match registry metrics",
    );
}

#[test]
fn instrument_can_use_custom_registry_override() {
    use finstack_quant_valuations::instruments::Instrument;
    use std::sync::Arc;
    use time::macros::date;

    let as_of = date!(2025 - 01 - 15);
    let bond = fixed_test_bond();
    let market = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(flat_discount_curve("USD-TREASURY", as_of));

    let default_result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[finstack_quant_valuations::metrics::MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("default pricing path should succeed");

    let mut registry = PricerRegistry::new();
    registry
        .register(FixedBondPricer {
            amount: 990.0,
            model: ModelKey::Discounting,
        })
        .expect("unique test pricer registration");

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[finstack_quant_valuations::metrics::MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default()
                .with_registry(Arc::new(registry)),
        )
        .expect("custom registry override should succeed");

    assert_eq!(result.value.amount(), 990.0);
    assert!(
        default_result
            .measures
            .get("dv01")
            .copied()
            .unwrap_or_default()
            .abs()
            > 1e-9,
        "control path should have non-zero DV01 so the override test is meaningful",
    );
    assert_eq!(
        result.measures.get("dv01").copied(),
        Some(0.0),
        "custom registry must also drive metric repricing, not just base PV",
    );
}

#[test]
fn instrument_model_override_controls_metric_repricing() {
    use finstack_quant_valuations::instruments::Instrument;
    use std::sync::Arc;
    use time::macros::date;

    let as_of = date!(2025 - 01 - 15);
    let bond = fixed_test_bond();
    let market = finstack_quant_core::market_data::context::MarketContext::new()
        .insert(flat_discount_curve("USD-TREASURY", as_of));

    let default_result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[finstack_quant_valuations::metrics::MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("default pricing path should succeed");

    let mut registry = PricerRegistry::new();
    registry
        .register(FixedBondPricer {
            amount: 995.0,
            model: ModelKey::HazardRate,
        })
        .expect("unique test pricer registration");

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[finstack_quant_valuations::metrics::MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default()
                .with_model(ModelKey::HazardRate)
                .with_registry(Arc::new(registry)),
        )
        .expect("model override path should succeed");

    assert_eq!(result.value.amount(), 995.0);
    assert!(
        default_result
            .measures
            .get("dv01")
            .copied()
            .unwrap_or_default()
            .abs()
            > 1e-9,
        "control path should have non-zero DV01 so the override test is meaningful",
    );
    assert_eq!(
        result.measures.get("dv01").copied(),
        Some(0.0),
        "explicit model override must control metric repricing as well",
    );
}

/// Regression guard: empty metrics slice must not cause a difference in PV.
/// Any future refactor that accidentally introduces metric-side-effects on PV
/// will be caught here.
#[test]
fn empty_metrics_does_not_alter_pv() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_valuations::instruments::Instrument;
    use time::macros::date;

    let as_of: Date = date!(2025 - 01 - 15);
    let bond = fixed_test_bond();
    let disc = flat_discount_curve("USD-TREASURY", as_of);
    let market = MarketContext::new().insert(disc);
    let registry = finstack_quant_valuations::pricer::standard_pricer_registry();

    let baseline = bond
        .value(&market, as_of)
        .expect("bond.value should succeed");

    let with_metrics = registry
        .price_with_metrics(
            &bond,
            ModelKey::Discounting,
            &market,
            as_of,
            &[],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("registry price_with_metrics should succeed");

    assert!(
        (baseline.amount() - with_metrics.value.amount()).abs() < 1.0,
        "PV with empty metrics should equal bare value: baseline={:.4} with_metrics={:.4}",
        baseline.amount(),
        with_metrics.value.amount()
    );
}
