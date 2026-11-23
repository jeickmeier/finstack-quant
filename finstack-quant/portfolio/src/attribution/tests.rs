use super::engine::*;
use crate::Portfolio;
use crate::error::{Error, Result};
use crate::position::{Position, PositionUnit};
use crate::types::Entity;
use finstack_quant_attribution::{
    AttributionMethod, AttributionRequest, ExecutionPolicy, PnlAttribution, PnlBucket,
    TaylorAttributionConfig, default_attribution_metrics,
};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_valuations::instruments::composite::{
    CompositeLegSpec, CompositeSpec, RebalanceRule, WeightingMethod,
};
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::InstrumentType;
use finstack_quant_valuations::results::ValuationResult;
use indexmap::IndexMap;
use std::any::Any;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use time::macros::date;

#[test]
fn composite_attribution_reconciles_to_frozen_primitive_pnl() -> Result<()> {
    let spec = CompositeSpec::new(
        "A-B",
        Currency::USD,
        Money::from((100_i64, Currency::USD)),
        vec![
            CompositeLegSpec::new(
                "A",
                finstack_quant_valuations::instruments::json_loader::InstrumentJson::Equity(
                    finstack_quant_valuations::instruments::equity::spot::Equity::new(
                        "A",
                        "A",
                        Currency::USD,
                    )
                    .with_spot_id("A"),
                ),
                1.0,
            ),
            CompositeLegSpec::new(
                "B",
                finstack_quant_valuations::instruments::json_loader::InstrumentJson::Equity(
                    finstack_quant_valuations::instruments::equity::spot::Equity::new(
                        "B",
                        "B",
                        Currency::USD,
                    )
                    .with_spot_id("B"),
                ),
                -1.0,
            ),
        ],
        WeightingMethod::FixedQuantity,
        RebalanceRule::Manual,
    );
    let composite = spec.initialize_fixed(date!(2025 - 01 - 01))?.instrument;
    let position = Position::new(
        "P-COMPOSITE",
        "ENTITY",
        "A-B",
        Arc::new(composite),
        2.0,
        PositionUnit::Units,
    )?;
    let portfolio = Portfolio::builder("PORTFOLIO")
        .base_currency(Currency::USD)
        .as_of(date!(2025 - 01 - 01))
        .entity(Entity::new("ENTITY"))
        .position(position)
        .build()?;
    let market_t0 = MarketContext::new()
        .insert_price("A", MarketScalar::unitless(100.0).expect("valid scalar"))
        .insert_price("B", MarketScalar::unitless(90.0).expect("valid scalar"));
    let market_t1 = MarketContext::new()
        .insert_price("A", MarketScalar::unitless(110.0).expect("valid scalar"))
        .insert_price("B", MarketScalar::unitless(95.0).expect("valid scalar"));

    let attribution = attribute_portfolio_pnl(
        &portfolio,
        &market_t0,
        &market_t1,
        date!(2025 - 01 - 01),
        date!(2025 - 01 - 02),
        &FinstackConfig::default(),
        AttributionMethod::Parallel,
    )?;
    assert_eq!(attribution.total_pnl.amount(), 10.0);
    assert!(attribution.reconciliation_check(1.0e-9).is_reconciled);
    let position = attribution
        .by_position
        .get("P-COMPOSITE")
        .ok_or_else(|| Error::invalid_input("missing composite attribution"))?;
    assert_eq!(position.total_pnl.amount(), 10.0);
    // Endpoint values are the scaled sum of the primitive paths:
    // 2 × (100 − 90) at T0 and 2 × (110 − 95) at T1.
    let pv_t0 = position
        .pv_t0
        .ok_or_else(|| Error::invalid_input("missing composite pv_t0"))?;
    let pv_t1 = position
        .pv_t1
        .ok_or_else(|| Error::invalid_input("missing composite pv_t1"))?;
    assert!((pv_t0.amount() - 20.0).abs() < 1.0e-9);
    assert!((pv_t1.amount() - 30.0).abs() < 1.0e-9);
    assert!(position.waterfall_steps.is_empty() && position.sensitivity_steps.is_empty());
    assert_eq!(position.total_pnl.amount(), 10.0);
    assert!(
        position
            .meta
            .notes
            .iter()
            .any(|note| note.contains("frozen primitive"))
    );
    Ok(())
}

/// A foreign-currency primitive is translated into the composite's
/// reporting currency with factors at T1 FX plus the opening-value FX move,
/// and the composite records the FX policy it applied.
#[test]
fn composite_attribution_translates_foreign_primitives() -> Result<()> {
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};

    let spec = CompositeSpec::new(
        "EUR-BASKET",
        Currency::USD,
        Money::from((100_i64, Currency::USD)),
        vec![
            CompositeLegSpec::new(
                "E",
                finstack_quant_valuations::instruments::json_loader::InstrumentJson::Equity(
                    finstack_quant_valuations::instruments::equity::spot::Equity::new(
                        "E",
                        "E",
                        Currency::EUR,
                    )
                    .with_spot_id("E"),
                ),
                1.0,
            ),
            // A flat-price USD leg: no P&L, no translation.
            CompositeLegSpec::new(
                "U",
                finstack_quant_valuations::instruments::json_loader::InstrumentJson::Equity(
                    finstack_quant_valuations::instruments::equity::spot::Equity::new(
                        "U",
                        "U",
                        Currency::USD,
                    )
                    .with_spot_id("U"),
                ),
                1.0,
            ),
        ],
        WeightingMethod::FixedQuantity,
        RebalanceRule::Manual,
    );
    let composite = spec.initialize_fixed(date!(2025 - 01 - 01))?.instrument;
    let portfolio = Portfolio::builder("PORTFOLIO")
        .base_currency(Currency::USD)
        .as_of(date!(2025 - 01 - 01))
        .entity(Entity::new("ENTITY"))
        .position(Position::new(
            "P-EUR",
            "ENTITY",
            "EUR-BASKET",
            Arc::new(composite),
            2.0,
            PositionUnit::Units,
        )?)
        .build()?;
    let market = |spot: f64, eur_usd: f64| -> Result<MarketContext> {
        let provider = Arc::new(SimpleFxProvider::new());
        provider.set_quotes(&[(Currency::EUR, Currency::USD, eur_usd)])?;
        Ok(MarketContext::new()
            .insert_price("E", MarketScalar::unitless(spot).expect("valid scalar"))
            .insert_price("U", MarketScalar::unitless(50.0).expect("valid scalar"))
            .insert_fx(FxMatrix::new(provider, None)))
    };

    let attribution = attribute_portfolio_pnl(
        &portfolio,
        &market(100.0, 1.1)?,
        &market(110.0, 1.2)?,
        date!(2025 - 01 - 01),
        date!(2025 - 01 - 02),
        &FinstackConfig::default(),
        AttributionMethod::Parallel,
    )?;
    let position = attribution
        .by_position
        .get("P-EUR")
        .ok_or_else(|| Error::invalid_input("missing composite attribution"))?;
    // Per unit: 10 EUR × 1.2 + 100 EUR × (1.2 − 1.1) = 22 USD; two units held.
    assert_eq!(position.total_pnl.currency(), Currency::USD);
    assert!((position.total_pnl.amount() - 44.0).abs() < 1.0e-9);
    assert!((position.fx_translation_pnl.amount() - 20.0).abs() < 1.0e-9);
    let fx_policy = position
        .meta
        .fx_policy
        .as_ref()
        .ok_or_else(|| Error::invalid_input("composite FX policy not stamped"))?;
    assert_eq!(fx_policy.target_currency, Some(Currency::USD));
    Ok(())
}

/// Composite legs are attributed exactly like the same instruments on
/// their own. Over a multi-day window the metrics-based carry needs theta
/// priced over that window; the composite's former private dispatch
/// priced the default horizon and failed for coupon-paying legs.
#[test]
fn composite_metrics_based_carry_matches_the_standalone_legs() -> Result<()> {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
    use finstack_quant_valuations::instruments::json_loader::InstrumentJson;

    let as_of_t0 = date!(2025 - 01 - 01);
    let as_of_t1 = date!(2025 - 01 - 31);
    let bond = |id: &str, coupon: f64| -> Result<Bond> {
        Ok(Bond::fixed(
            id,
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(coupon).expect("valid rate"),
            date!(2024 - 01 - 01),
            date!(2030 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )?)
    };
    let legs = [bond("LEG-A", 0.05)?, bond("LEG-B", 0.03)?];
    let market = |base, rate: f64| -> Result<MarketContext> {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots([(0.0, 1.0), (10.0, (-rate * 10.0_f64).exp())])
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()?;
        Ok(MarketContext::new()
            .try_insert(curve)
            .expect("insert market curve"))
    };
    let market_t0 = market(as_of_t0, 0.04)?;
    let market_t1 = market(as_of_t1, 0.041)?;
    let config = FinstackConfig::default();

    let mut standalone_carry = 0.0;
    for leg in &legs {
        let instrument: Arc<dyn Instrument> = Arc::new(leg.clone());
        standalone_carry += finstack_quant_attribution::attribute_pnl(
            &AttributionMethod::MetricsBased,
            &AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &config,
            ),
        )?
        .carry
        .amount();
    }

    let spec = CompositeSpec::new(
        "TWO-BONDS",
        Currency::USD,
        Money::from((2_000_000_i64, Currency::USD)),
        legs.iter()
            .map(|leg| {
                CompositeLegSpec::new(leg.id.as_str(), InstrumentJson::Bond(leg.clone()), 1.0)
            })
            .collect(),
        WeightingMethod::FixedQuantity,
        RebalanceRule::Manual,
    );
    let composite = spec.initialize_fixed(as_of_t0)?.instrument;
    let composite_attribution = attribute_composite_primitives(
        &composite,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        &AttributionMethod::MetricsBased,
    )?;

    assert!(
        (composite_attribution.carry.amount() - standalone_carry).abs() < 1.0e-6,
        "composite carry {} vs standalone legs {standalone_carry}",
        composite_attribution.carry,
    );
    Ok(())
}

#[derive(Clone)]
struct ConfigRequiredInstrument {
    attributes: Attributes,
    pv_only_calls: Arc<AtomicUsize>,
    metric_calls: Arc<AtomicUsize>,
    base_value_calls: Arc<AtomicUsize>,
}

#[derive(Clone)]
struct EndpointFailingInstrument {
    id: String,
    attributes: Attributes,
    fail_as_of: Date,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    ConfigRequiredInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);
finstack_quant_valuations::impl_empty_cashflow_provider!(
    EndpointFailingInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl Instrument for ConfigRequiredInstrument {
    fn includes_valuation_date_cashflows(&self) -> bool {
        true
    }
    /// Test mock: reads no market data.
    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
    {
        Ok(finstack_quant_valuations::instruments::MarketDependencies::new())
    }

    fn id(&self) -> &str {
        "CONFIG_REQUIRED"
    }

    fn key(&self) -> InstrumentType {
        InstrumentType::Basket
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
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
        self.base_value_calls.fetch_add(1, Ordering::SeqCst);
        Ok(Money::from((100_i64, Currency::USD)))
    }

    fn price_with_metrics(
        &self,
        _market: &MarketContext,
        as_of: Date,
        _metrics: &[MetricId],
        options: PricingOptions,
    ) -> std::result::Result<ValuationResult, finstack_quant_valuations::pricer::PricingError> {
        if _metrics.is_empty() {
            self.pv_only_calls.fetch_add(1, Ordering::SeqCst);
        } else {
            self.metric_calls.fetch_add(1, Ordering::SeqCst);
        }
        let config = options.config.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "attribution pricing did not receive FinstackConfig".to_string(),
            )
        })?;
        if config
            .rounding
            .output_scale
            .get_overrides()
            .get(&Currency::USD)
            != Some(&4)
        {
            return Err(finstack_quant_core::Error::Validation(
                "attribution pricing received the wrong FinstackConfig".to_string(),
            )
            .into());
        }
        Ok(ValuationResult::stamped_with_meta(
            self.id(),
            as_of,
            Money::from((100_i64, Currency::USD)),
            finstack_quant_core::config::results_meta(config.as_ref()),
        ))
    }
}

impl Instrument for EndpointFailingInstrument {
    fn includes_valuation_date_cashflows(&self) -> bool {
        true
    }
    /// Test mock: reads no market data.
    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
    {
        Ok(finstack_quant_valuations::instruments::MarketDependencies::new())
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> InstrumentType {
        InstrumentType::Basket
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
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
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        if as_of == self.fail_as_of {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{} failed at the closing endpoint",
                self.id
            )));
        }
        Ok(Money::from((100_i64, Currency::USD)))
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        _metrics: &[MetricId],
        _options: PricingOptions,
    ) -> std::result::Result<ValuationResult, finstack_quant_valuations::pricer::PricingError> {
        Ok(ValuationResult::stamped(
            self.id(),
            as_of,
            self.base_value(market, as_of)?,
        ))
    }
}

fn sample_position_attr(
    position_id: &str,
    total: f64,
    carry: f64,
    residual: f64,
) -> PnlAttribution {
    let mut attr = PnlAttribution::new(
        Money::new(total, Currency::USD).expect("valid money fixture"),
        position_id,
        date!(2026 - 01 - 02),
        date!(2026 - 01 - 03),
        AttributionMethod::Parallel,
    );
    attr.carry = Money::new(carry, Currency::USD).expect("valid money fixture");
    attr.rates_curves_pnl =
        Money::new(total - carry - residual, Currency::USD).expect("valid money fixture");
    attr.residual = Money::new(residual, Currency::USD).expect("valid money fixture");
    attr
}

#[test]
fn test_default_metrics_nonempty() {
    assert!(!default_attribution_metrics().is_empty());
}

#[test]
fn metrics_based_attribution_forwards_caller_config() {
    let pv_only_calls = Arc::new(AtomicUsize::new(0));
    let metric_calls = Arc::new(AtomicUsize::new(0));
    let base_value_calls = Arc::new(AtomicUsize::new(0));
    let instrument = Arc::new(ConfigRequiredInstrument {
        attributes: Attributes::new(),
        pv_only_calls: Arc::clone(&pv_only_calls),
        metric_calls: Arc::clone(&metric_calls),
        base_value_calls: Arc::clone(&base_value_calls),
    });
    let position = crate::position::Position::new(
        "P_CONFIG",
        "E_CONFIG",
        "CONFIG_REQUIRED",
        instrument,
        1.0,
        crate::position::PositionUnit::Units,
    )
    .expect("position");
    let mut config = FinstackConfig::default();
    config
        .rounding
        .output_scale
        .set_scale(Currency::USD, 4)
        .expect("valid decimal scale");

    let portfolio = Portfolio::builder("CONFIG_PORTFOLIO")
        .base_currency(Currency::USD)
        .as_of(date!(2026 - 01 - 02))
        .entity(crate::types::Entity::new("E_CONFIG"))
        .position(position)
        .build()
        .expect("portfolio");

    attribute_portfolio_pnl(
        &portfolio,
        &MarketContext::new(),
        &MarketContext::new(),
        date!(2026 - 01 - 02),
        date!(2026 - 01 - 03),
        &config,
        AttributionMethod::MetricsBased,
    )
    .expect("metrics-based attribution must forward config");

    assert_eq!(
        metric_calls.load(Ordering::SeqCst),
        2,
        "prepared attribution must value each endpoint exactly once"
    );
    assert_eq!(pv_only_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        base_value_calls.load(Ordering::SeqCst),
        0,
        "prepared metrics attribution must not reprice T0 through value()"
    );
}

fn assert_same_financial_decomposition(actual: &PnlAttribution, expected: &PnlAttribution) {
    assert_eq!(actual.total_pnl, expected.total_pnl);
    assert_eq!(actual.carry, expected.carry);
    assert_eq!(actual.rates_curves_pnl, expected.rates_curves_pnl);
    assert_eq!(actual.credit_curves_pnl, expected.credit_curves_pnl);
    assert_eq!(actual.inflation_curves_pnl, expected.inflation_curves_pnl);
    assert_eq!(actual.correlations_pnl, expected.correlations_pnl);
    assert_eq!(actual.fx_pnl, expected.fx_pnl);
    assert_eq!(actual.cross_factor_pnl, expected.cross_factor_pnl);
    assert_eq!(actual.vol_pnl, expected.vol_pnl);
    assert_eq!(actual.model_params_pnl, expected.model_params_pnl);
    assert_eq!(actual.market_scalars_pnl, expected.market_scalars_pnl);
    assert_eq!(actual.residual, expected.residual);
    assert_eq!(actual.result_invalid, expected.result_invalid);
}

#[test]
fn repricing_methods_prepare_endpoints_once_and_preserve_decomposition() {
    let as_of_t0 = date!(2026 - 01 - 02);
    let as_of_t1 = date!(2026 - 01 - 03);
    let methods = [
        AttributionMethod::Parallel,
        AttributionMethod::Waterfall(finstack_quant_attribution::default_waterfall_order()),
        AttributionMethod::Taylor(TaylorAttributionConfig::default()),
    ];

    for method in methods {
        let pv_only_calls = Arc::new(AtomicUsize::new(0));
        let metric_calls = Arc::new(AtomicUsize::new(0));
        let base_value_calls = Arc::new(AtomicUsize::new(0));
        let instrument: Arc<dyn Instrument> = Arc::new(ConfigRequiredInstrument {
            attributes: Attributes::new(),
            pv_only_calls: Arc::clone(&pv_only_calls),
            metric_calls,
            base_value_calls,
        });
        let position = crate::position::Position::new(
            "P_CONFIG",
            "E_CONFIG",
            "CONFIG_REQUIRED",
            Arc::clone(&instrument),
            1.0,
            crate::position::PositionUnit::Units,
        )
        .expect("position");
        let portfolio = Portfolio::builder("CONFIG_PORTFOLIO")
            .base_currency(Currency::USD)
            .as_of(as_of_t0)
            .entity(crate::types::Entity::new("E_CONFIG"))
            .position(position)
            .build()
            .expect("portfolio");
        let mut config = FinstackConfig::default();
        config
            .rounding
            .output_scale
            .set_scale(Currency::USD, 4)
            .expect("valid decimal scale");
        let market_t0 = MarketContext::new();
        let market_t1 = MarketContext::new();

        let actual = attribute_portfolio_pnl(
            &portfolio,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &config,
            method.clone(),
        )
        .expect("prepared portfolio attribution");
        assert_eq!(
            pv_only_calls.load(Ordering::SeqCst),
            2,
            "{method} must prepare each ordinary endpoint exactly once"
        );

        let expected = match &method {
            AttributionMethod::Parallel => finstack_quant_attribution::attribute_pnl(
                &AttributionMethod::Parallel,
                &finstack_quant_attribution::AttributionRequest {
                    execution_policy: ExecutionPolicy::Serial,
                    ..finstack_quant_attribution::AttributionRequest::new(
                        &instrument,
                        &market_t0,
                        &market_t1,
                        as_of_t0,
                        as_of_t1,
                        &config,
                    )
                },
            ),
            AttributionMethod::Waterfall(order) => finstack_quant_attribution::attribute_pnl(
                &AttributionMethod::Waterfall(order.clone()),
                &finstack_quant_attribution::AttributionRequest {
                    strict_validation: false,
                    model_params_t0: None,
                    ..finstack_quant_attribution::AttributionRequest::new(
                        &instrument,
                        &market_t0,
                        &market_t1,
                        as_of_t0,
                        as_of_t1,
                        &config,
                    )
                },
            ),
            AttributionMethod::Taylor(taylor_config) => finstack_quant_attribution::attribute_pnl(
                &AttributionMethod::Taylor(taylor_config.clone()),
                &finstack_quant_attribution::AttributionRequest {
                    execution_policy: ExecutionPolicy::Serial,
                    ..finstack_quant_attribution::AttributionRequest::new(
                        &instrument,
                        &market_t0,
                        &market_t1,
                        as_of_t0,
                        as_of_t1,
                        &finstack_quant_core::config::FinstackConfig::default(),
                    )
                },
            ),
            AttributionMethod::MetricsBased => {
                unreachable!("test covers repricing methods only")
            }
        }
        .expect("standalone instrument attribution");
        let actual_position = actual
            .by_position
            .get("P_CONFIG")
            .expect("position attribution");
        assert_same_financial_decomposition(actual_position, &expected);
    }
}

#[test]
fn method_owned_attribution_reports_position_errors_in_input_order() {
    let as_of_t0 = date!(2026 - 01 - 02);
    let as_of_t1 = date!(2026 - 01 - 03);
    let mut portfolio_builder = Portfolio::builder("ORDERED_ERRORS")
        .base_currency(Currency::USD)
        .as_of(as_of_t0)
        .entity(crate::types::Entity::new("E"));

    for (position_id, instrument_id) in [("P_FIRST", "I_FIRST"), ("P_SECOND", "I_SECOND")] {
        let instrument = Arc::new(EndpointFailingInstrument {
            id: instrument_id.to_string(),
            attributes: Attributes::new(),
            fail_as_of: as_of_t1,
        });
        let position = crate::position::Position::new(
            position_id,
            "E",
            instrument_id,
            instrument,
            1.0,
            crate::position::PositionUnit::Units,
        )
        .expect("position");
        portfolio_builder = portfolio_builder.position(position);
    }
    let portfolio = portfolio_builder.build().expect("portfolio");

    let error = attribute_portfolio_pnl(
        &portfolio,
        &MarketContext::new(),
        &MarketContext::new(),
        as_of_t0,
        as_of_t1,
        &FinstackConfig::default(),
        AttributionMethod::Parallel,
    )
    .expect_err("both positions fail at T1");
    let message = error.to_string();
    assert!(
        message.contains("P_FIRST"),
        "logical first position must win: {message}"
    );
    assert!(
        !message.contains("P_SECOND"),
        "later position error must not win: {message}"
    );
}

#[test]
fn portfolio_attribution_rejects_missing_cross_factor_pnl() {
    let base_currency = Currency::EUR;
    let zero = Money::from((0_i64, base_currency));
    let attr = PortfolioAttribution {
        total_pnl: Money::from((100_i64, base_currency)),
        carry: zero,
        rates_curves_pnl: zero,
        credit_curves_pnl: zero,
        inflation_curves_pnl: zero,
        correlations_pnl: zero,
        fx_pnl: zero,
        fx_translation_pnl: zero,
        cross_factor_pnl: zero,
        vol_pnl: zero,
        model_params_pnl: zero,
        market_scalars_pnl: zero,
        residual: zero,
        by_position: IndexMap::new(),
        result_invalid: false,
    };
    let mut value = serde_json::to_value(&attr).expect("serialize attribution");
    value
        .as_object_mut()
        .expect("attribution serializes to object")
        .remove("cross_factor_pnl");

    let error = serde_json::from_value::<PortfolioAttribution>(value)
        .expect_err("cross_factor_pnl is required by the canonical contract");
    assert!(error.to_string().contains("cross_factor_pnl"));
}

#[test]
fn test_explain_formats_percentages_and_zero_total_safely() {
    let zero = Money::from((0_i64, Currency::USD));
    let explained = PortfolioAttribution {
        total_pnl: Money::from((200_i64, Currency::USD)),
        carry: Money::from((20_i64, Currency::USD)),
        rates_curves_pnl: Money::from((100_i64, Currency::USD)),
        credit_curves_pnl: Money::from((10_i64, Currency::USD)),
        inflation_curves_pnl: Money::from((5_i64, Currency::USD)),
        correlations_pnl: Money::from((15_i64, Currency::USD)),
        fx_pnl: Money::from((25_i64, Currency::USD)),
        fx_translation_pnl: Money::from((10_i64, Currency::USD)),
        cross_factor_pnl: zero,
        vol_pnl: Money::from((5_i64, Currency::USD)),
        model_params_pnl: Money::from((5_i64, Currency::USD)),
        market_scalars_pnl: Money::from((3_i64, Currency::USD)),
        residual: Money::from((2_i64, Currency::USD)),
        by_position: IndexMap::new(),
        result_invalid: false,
    };
    let rendered = explained.explain();
    assert!(rendered.contains("Portfolio P&L: USD 200.00"));
    assert!(rendered.contains("Carry: USD 20.00 (10.0%)"));
    assert!(rendered.contains("FX Translation: USD 10.00 (5.0%)"));
    assert!(rendered.contains("Residual: USD 2.00 (1.0%)"));

    let zero_total = PortfolioAttribution {
        total_pnl: zero,
        carry: Money::from((5_i64, Currency::USD)),
        rates_curves_pnl: zero,
        credit_curves_pnl: zero,
        inflation_curves_pnl: zero,
        correlations_pnl: zero,
        fx_pnl: zero,
        fx_translation_pnl: zero,
        cross_factor_pnl: zero,
        vol_pnl: zero,
        model_params_pnl: zero,
        market_scalars_pnl: zero,
        residual: Money::from((-5_i64, Currency::USD)),
        by_position: IndexMap::new(),
        result_invalid: false,
    };
    let zero_rendered = zero_total.explain();
    assert!(zero_rendered.contains("Carry: USD 5.00 (0.0%)"));
    assert!(zero_rendered.contains("Residual: USD -5.00 (0.0%)"));
}

/// A total below half a cent is zero for the explain text exactly as it is
/// for the bucket shares, so the table and the text never disagree.
#[test]
fn explain_and_bucket_shares_share_the_zero_total_rule() {
    let usd = |amount: f64| Money::new(amount, Currency::USD).expect("finite amount");
    let zero = usd(0.0);
    let attribution = PortfolioAttribution {
        total_pnl: usd(0.003),
        carry: usd(0.002),
        rates_curves_pnl: usd(0.001),
        credit_curves_pnl: zero,
        inflation_curves_pnl: zero,
        correlations_pnl: zero,
        fx_pnl: zero,
        fx_translation_pnl: zero,
        cross_factor_pnl: zero,
        vol_pnl: zero,
        model_params_pnl: zero,
        market_scalars_pnl: zero,
        residual: zero,
        by_position: IndexMap::new(),
        result_invalid: false,
    };
    assert!(
        attribution.total_pnl.amount() > 0.0,
        "0.003 is kept, not rounded away"
    );
    assert!(
        attribution
            .pct_of_total_by_bucket()
            .iter()
            .all(|bucket| bucket.pct_of_total.is_none())
    );
    let explained = attribution.explain();
    assert!(explained.contains("Carry: USD"), "{explained}");
    assert!(
        explained
            .lines()
            .skip(1)
            .all(|line| line.ends_with("(0.0%)")),
        "{explained}"
    );
}

/// A new `Money` field on `PortfolioAttribution` must be listed as a
/// bucket or named here as the headline value, so it cannot vanish from
/// reports; the listed buckets are exactly what reconciliation adds up.
#[test]
fn pct_of_total_by_bucket_lists_every_money_field_but_the_total() {
    let usd = |amount: i64| Money::from((amount, Currency::USD));
    let mut attribution = PortfolioAttribution {
        total_pnl: usd(200),
        carry: usd(20),
        rates_curves_pnl: usd(-50),
        credit_curves_pnl: usd(10),
        inflation_curves_pnl: usd(5),
        correlations_pnl: usd(15),
        fx_pnl: usd(25),
        fx_translation_pnl: usd(160),
        cross_factor_pnl: usd(0),
        vol_pnl: usd(5),
        model_params_pnl: usd(5),
        market_scalars_pnl: usd(3),
        residual: usd(2),
        by_position: IndexMap::new(),
        result_invalid: false,
    };

    let wire = serde_json::to_value(&attribution).expect("serializes");
    let mut money_fields: Vec<&str> = wire
        .as_object()
        .expect("object")
        .iter()
        .filter(|(_, value)| serde_json::from_value::<Money>((*value).clone()).is_ok())
        .map(|(key, _)| key.as_str())
        .filter(|key| *key != "total_pnl")
        .collect();
    money_fields.sort_unstable();
    let buckets = attribution.pct_of_total_by_bucket();
    let mut listed: Vec<&str> = buckets.iter().map(|bucket| bucket.field.as_str()).collect();
    listed.sort_unstable();
    assert_eq!(listed, money_fields);

    let rates = &buckets[1];
    assert_eq!(rates.field, PnlBucket::RatesCurvesPnl);
    assert_eq!(rates.amount, usd(-50));
    assert_eq!(rates.pct_of_total, Some(-25.0));
    let translation = buckets
        .iter()
        .find(|bucket| bucket.field == PnlBucket::FxTranslationPnl)
        .expect("fx translation is a bucket");
    assert_eq!(translation.pct_of_total, Some(80.0));
    // The buckets are the amounts reconciliation sums: they net to the total.
    let listed_sum: f64 = buckets.iter().map(|bucket| bucket.amount.amount()).sum();
    assert!((listed_sum - 200.0).abs() < 1e-9);
    assert!(attribution.reconciliation_check(1e-9).is_reconciled);

    attribution.total_pnl = usd(0);
    assert!(
        attribution
            .pct_of_total_by_bucket()
            .iter()
            .all(|bucket| bucket.pct_of_total.is_none())
    );
}

#[test]
fn test_reconciliation_check_passes_for_consistent_attribution() {
    let base_currency = Currency::USD;
    let portfolio_attr = PortfolioAttribution {
        total_pnl: Money::from((200_i64, base_currency)),
        carry: Money::from((20_i64, base_currency)),
        rates_curves_pnl: Money::from((100_i64, base_currency)),
        credit_curves_pnl: Money::from((10_i64, base_currency)),
        inflation_curves_pnl: Money::from((5_i64, base_currency)),
        correlations_pnl: Money::from((15_i64, base_currency)),
        fx_pnl: Money::from((25_i64, base_currency)),
        fx_translation_pnl: Money::from((10_i64, base_currency)),
        cross_factor_pnl: Money::from((0_i64, base_currency)),
        vol_pnl: Money::from((5_i64, base_currency)),
        model_params_pnl: Money::from((5_i64, base_currency)),
        market_scalars_pnl: Money::from((3_i64, base_currency)),
        residual: Money::from((2_i64, base_currency)),
        by_position: IndexMap::new(),
        result_invalid: false,
    };

    let report = portfolio_attr.reconciliation_check(0.01);
    assert!(
        report.is_reconciled,
        "expected reconciliation to pass, residual = {}",
        report.total_residual
    );
    assert!(
        report.total_residual.abs() < 1e-10,
        "residual should be ~0, got {}",
        report.total_residual
    );
}

#[test]
fn reconciliation_check_includes_cross_factor_pnl() {
    let base_currency = Currency::USD;
    let zero = Money::from((0_i64, base_currency));
    let portfolio_attr = PortfolioAttribution {
        total_pnl: Money::from((107_i64, base_currency)),
        carry: Money::from((20_i64, base_currency)),
        rates_curves_pnl: Money::from((80_i64, base_currency)),
        credit_curves_pnl: zero,
        inflation_curves_pnl: zero,
        correlations_pnl: zero,
        fx_pnl: zero,
        fx_translation_pnl: zero,
        vol_pnl: zero,
        cross_factor_pnl: Money::from((7_i64, base_currency)),
        model_params_pnl: zero,
        market_scalars_pnl: zero,
        residual: zero,
        by_position: IndexMap::new(),
        result_invalid: false,
    };

    let report = portfolio_attr.reconciliation_check(0.01);
    assert!(report.is_reconciled);
    assert!(report.total_residual.abs() < 1e-10);
}

#[test]
fn test_reconciliation_check_fails_when_totals_mismatch() {
    let base_currency = Currency::USD;
    let zero = Money::from((0_i64, base_currency));
    // total_pnl deliberately mismatches the sum of factor buckets
    let portfolio_attr = PortfolioAttribution {
        total_pnl: Money::from((1000_i64, base_currency)),
        carry: Money::from((100_i64, base_currency)),
        rates_curves_pnl: Money::from((500_i64, base_currency)),
        credit_curves_pnl: zero,
        inflation_curves_pnl: zero,
        correlations_pnl: zero,
        fx_pnl: zero,
        fx_translation_pnl: zero,
        cross_factor_pnl: zero,
        vol_pnl: zero,
        model_params_pnl: zero,
        market_scalars_pnl: zero,
        residual: zero,
        by_position: IndexMap::new(),
        result_invalid: false,
    };

    let report = portfolio_attr.reconciliation_check(0.01);
    assert!(
        !report.is_reconciled,
        "expected reconciliation to fail for mismatched totals"
    );
    assert!(
        (report.total_residual - 400.0).abs() < 1e-10,
        "residual should be 400.0, got {}",
        report.total_residual
    );
}

#[test]
fn embedded_fx_translation_is_preserved_once() {
    let mut position = sample_position_attr("COMPOSITE", 10.0, 0.0, 0.0);
    position.fx_translation_pnl = Money::from((10_i64, Currency::USD));
    let mut aggregate = sample_position_attr("PORTFOLIO", 0.0, 0.0, 0.0);
    aggregate.accumulate(position).expect("same-currency add");
    let result = PortfolioAttribution::from_aggregate(aggregate, IndexMap::new());
    assert_eq!(result.total_pnl.amount(), 10.0);
    assert_eq!(result.fx_translation_pnl.amount(), 10.0);
}

/// Audit (Portfolio): a single invalid constituent must flag the whole
/// aggregate invalid so downstream reporting can refuse to trust it.
#[test]
fn portfolio_aggregation_propagates_position_result_invalid() {
    let good = sample_position_attr("GOOD", 50.0, 5.0, 2.0);
    let mut bad = sample_position_attr("BAD", 100.0, 10.0, 5.0);
    bad.result_invalid = true;

    let mut aggregate = sample_position_attr("PORTFOLIO", 0.0, 0.0, 0.0);
    aggregate.accumulate(good).expect("same-currency add");
    aggregate.accumulate(bad).expect("same-currency add");

    let portfolio = PortfolioAttribution::from_aggregate(aggregate, IndexMap::new());
    assert!(
        portfolio.result_invalid,
        "one invalid position must flag the whole portfolio invalid"
    );
}

/// Audit (Portfolio): `reconciliation_check` must never report a result as
/// reconciled when `result_invalid` is set, even if the corrupted buckets
/// happen to net to within tolerance.
#[test]
fn reconciliation_check_fails_when_result_invalid_even_if_numbers_net() {
    let base = Currency::USD;
    let zero = Money::from((0_i64, base));
    let attr = PortfolioAttribution {
        total_pnl: Money::from((100_i64, base)),
        carry: Money::from((100_i64, base)),
        rates_curves_pnl: zero,
        credit_curves_pnl: zero,
        inflation_curves_pnl: zero,
        correlations_pnl: zero,
        fx_pnl: zero,
        fx_translation_pnl: zero,
        cross_factor_pnl: zero,
        vol_pnl: zero,
        model_params_pnl: zero,
        market_scalars_pnl: zero,
        residual: zero,
        by_position: IndexMap::new(),
        result_invalid: true,
    };
    // The buckets net exactly to total_pnl, so a numeric-only check would
    // pass — the result_invalid gate must still force is_reconciled false.
    let report = attr.reconciliation_check(0.01);
    assert!(
        !report.is_reconciled,
        "must not report reconciled when result_invalid is set"
    );
}
