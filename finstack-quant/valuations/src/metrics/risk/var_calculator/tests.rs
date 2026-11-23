use super::*;
use crate::instruments::Instrument;
use crate::metrics::risk::{MarketHistory, apply_market_scenario};
use crate::metrics::risk::{MarketScenario, RiskFactorShift, RiskFactorType};
use crate::pricer::PricingDispatch;
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxQuery};
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::metrics::risk::{VarConfig, VarMethod};
use std::sync::Arc;
use time::macros::date;

fn sample_as_of() -> finstack_quant_core::dates::Date {
    date!(2024 - 01 - 01)
}

fn usd_ois_market(as_of: finstack_quant_core::dates::Date) -> Result<MarketContext> {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots(vec![(0.0, 1.0), (5.0, 0.85), (10.0, 0.70)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()?;
    Ok(MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve"))
}

fn history_from_scenarios(
    as_of: finstack_quant_core::dates::Date,
    window_days: u32,
    scenarios: Vec<MarketScenario>,
) -> MarketHistory {
    MarketHistory::new(as_of, window_days, scenarios)
}

#[test]
fn var_result_round_trips_through_strict_json() -> Result<()> {
    let result = VarResult::from_distribution(vec![-3.0, 1.0, -1.0, 2.0], 0.95)?;
    let json = serde_json::to_string(&result)
        .map_err(|e| finstack_quant_core::Error::Internal(e.to_string()))?;
    let back: VarResult = serde_json::from_str(&json)
        .map_err(|e| finstack_quant_core::Error::Internal(e.to_string()))?;
    assert_eq!(back, result);
    assert!(serde_json::from_str::<VarResult>(&json.replace("\"var\"", "\"vaR\"")).is_err());
    Ok(())
}

#[test]
fn pricing_dispatch_from_model_maps_default_and_explicit_keys() -> Result<()> {
    let registry = || crate::pricer::Family::unregistered().shared_pricers();
    assert!(
        PricingDispatch::from_model("default", registry())?
            .model()
            .is_none()
    );
    assert_eq!(
        PricingDispatch::from_model("discounting", registry())?.model(),
        Some(crate::pricer::ModelKey::Discounting)
    );
    assert!(PricingDispatch::from_model("no_such_model", registry()).is_err());
    Ok(())
}

#[test]
fn several_invalid_scenarios_report_the_first_scenario_in_history_order() {
    let as_of = sample_as_of();
    let market = usd_ois_market(as_of).expect("market");
    let scenarios = (0..64)
        .map(|i| {
            MarketScenario::new(
                as_of,
                vec![RiskFactorShift {
                    factor: RiskFactorType::CreditSpread {
                        curve_id: CurveId::from(format!("CRV-{i:03}")),
                        tenor_years: -1.0,
                    },
                    shift: 0.001,
                }],
            )
        })
        .collect();
    let history = history_from_scenarios(as_of, 64, scenarios);
    for _ in 0..20 {
        let error = aggregate_scenario_pnls_par(&history, &market, as_of, |_| Ok(0.0), None)
            .expect_err("every scenario is invalid")
            .to_string();
        assert!(
            error.contains("'CRV-000'"),
            "expected the first scenario, got {error}"
        );
    }
}

#[derive(Clone, Debug)]
struct CurrencyScalarInstrument {
    id: String,
    price_id: String,
    currency: Currency,
    discount_curves: Vec<CurveId>,
    attributes: Attributes,
}

crate::impl_empty_cashflow_provider!(
    CurrencyScalarInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl CurrencyScalarInstrument {
    fn new(id: &str, price_id: &str, currency: Currency) -> Self {
        Self {
            id: id.to_string(),
            price_id: price_id.to_string(),
            currency,
            discount_curves: Vec::new(),
            attributes: Attributes::new(),
        }
    }

    fn with_discount_curve(mut self, curve_id: &str) -> Self {
        self.discount_curves.push(CurveId::from(curve_id));
        self
    }
}

impl Instrument for CurrencyScalarInstrument {
    fn includes_valuation_date_cashflows(&self) -> bool {
        true
    }
    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::MarketDependencies> {
        let mut deps = crate::instruments::MarketDependencies::new();
        for curve_id in &self.discount_curves {
            deps.add_discount_curve(curve_id.clone());
        }
        Ok(deps)
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> crate::pricer::InstrumentType {
        crate::pricer::InstrumentType::Equity
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

    fn base_value(&self, market: &MarketContext, _as_of: Date) -> Result<Money> {
        let scalar = market.get_price(&self.price_id)?;
        if let Some(m) = scalar.get_price() {
            Ok(m)
        } else {
            let v = scalar.get_unitless().unwrap_or_default();
            Ok(Money::new(v, self.currency).expect("valid money fixture"))
        }
    }
}

#[test]
fn var_result_reports_losses_as_negative() -> Result<()> {
    let pnls: Vec<f64> = (0..100).map(|s| f64::from(s) - 50.0).collect();
    let result = VarResult::from_distribution(pnls, 0.99)?;

    assert!(
        result.var < 0.0,
        "VaR must be negative (losses-negative convention), got {}",
        result.var
    );
    assert!(
        result.expected_shortfall <= result.var,
        "ES ({}) must lie at or beyond VaR ({}) in the loss tail",
        result.expected_shortfall,
        result.var
    );
    assert!((result.var - (-49.01)).abs() < 1e-9);
    Ok(())
}

#[test]
fn taylor_keeps_fx_shocks_to_a_reporting_cross_leg() -> Result<()> {
    use finstack_quant_core::money::fx::FxMatrix;

    let as_of = sample_as_of();
    let matrix = FxMatrix::from_quotes(
        &[
            (Currency::EUR, Currency::USD, 1.1),
            (Currency::USD, Currency::JPY, 140.0),
        ],
        Some(Currency::USD),
    )?;
    let cross = FxQuery::new(
        Currency::EUR,
        Currency::JPY,
        as_of,
        FxConversionPolicy::CashflowDate,
    );
    let leg = FxQuery::new(
        Currency::EUR,
        Currency::USD,
        as_of,
        FxConversionPolicy::CashflowDate,
    );
    let base = matrix.rate(cross)?;
    assert!(base.triangulated);
    let bumped = matrix.with_bumped_rate(leg, 0.01)?.rate(cross)?;
    assert!((bumped.rate - base.rate * 1.01).abs() < 1e-12);

    let market = MarketContext::new().insert_fx(matrix);
    let instrument = CurrencyScalarInstrument::new("EUR", "EUR-PX", Currency::EUR);
    let scenario = MarketScenario::new(
        as_of,
        vec![RiskFactorShift {
            factor: RiskFactorType::FxSpot {
                base: Currency::EUR,
                quote: Currency::USD,
            },
            shift: 0.01,
        }],
    );
    let relevant = relevant_taylor_scenario(
        &instrument,
        Currency::EUR,
        Currency::JPY,
        &scenario,
        &market,
        as_of,
    )?;
    assert_eq!(relevant.shifts.len(), 1);
    Ok(())
}

#[test]
fn relevant_taylor_scenario_keeps_only_instrument_dependencies() -> Result<()> {
    let as_of = sample_as_of();
    let instrument = CurrencyScalarInstrument::new("USD", "USD-PX", Currency::USD)
        .with_discount_curve("USD-OIS");
    let scenario = MarketScenario::new(
        as_of,
        vec![
            RiskFactorShift {
                factor: RiskFactorType::DiscountRate {
                    curve_id: "USD-OIS".into(),
                    tenor_years: 1.4,
                    grid_years: vec![1.4],
                },
                shift: 0.0005,
            },
            RiskFactorShift {
                factor: RiskFactorType::DiscountRate {
                    curve_id: "EUR-OIS".into(),
                    tenor_years: 2.3,
                    grid_years: vec![2.3],
                },
                shift: -0.0004,
            },
        ],
    );
    let relevant = relevant_taylor_scenario(
        &instrument,
        Currency::USD,
        Currency::USD,
        &scenario,
        &MarketContext::new(),
        as_of,
    )?;
    assert_eq!(relevant.shifts.len(), 1);
    assert!(matches!(
        &relevant.shifts[0].factor,
        RiskFactorType::DiscountRate { curve_id, .. } if curve_id == "USD-OIS"
    ));
    Ok(())
}

#[test]
fn var_result_clamps_all_gain_distribution_to_zero() -> Result<()> {
    let pnls: Vec<f64> = (1..=100).map(f64::from).collect();
    let result = VarResult::from_distribution(pnls, 0.95)?;
    assert_eq!(result.var, 0.0);
    assert_eq!(result.expected_shortfall, 0.0);
    Ok(())
}

#[test]
fn test_var_config_creation() {
    let config = VarConfig::var_95();
    assert_eq!(config.confidence_level, 0.95);
    assert_eq!(config.method, VarMethod::FullRevaluation);

    let config = VarConfig::var_99().with_method(VarMethod::TaylorApproximation);
    assert_eq!(config.confidence_level, 0.99);
    assert_eq!(config.method, VarMethod::TaylorApproximation);
}

#[test]
fn mixed_currency_var_requires_explicit_reporting_currency() {
    let as_of = sample_as_of();
    let usd = CurrencyScalarInstrument::new("USD-INST", "USD-INST", Currency::USD);
    let eur = CurrencyScalarInstrument::new("EUR-INST", "EUR-INST", Currency::EUR);

    let base_market = MarketContext::new()
        .insert_price(
            "USD-INST",
            MarketScalar::price(Money::from((100_i64, Currency::USD))),
        )
        .insert_price(
            "EUR-INST",
            MarketScalar::price(Money::from((100_i64, Currency::EUR))),
        );

    let scenario = MarketScenario::new(
        as_of,
        vec![
            RiskFactorShift {
                factor: RiskFactorType::EquitySpot {
                    ticker: "USD-INST".to_string(),
                },
                shift: 0.10,
            },
            RiskFactorShift {
                factor: RiskFactorType::EquitySpot {
                    ticker: "EUR-INST".to_string(),
                },
                shift: 0.10,
            },
        ],
    );
    let history = MarketHistory::new(as_of, 1, vec![scenario]);

    let err = calculate_var(
        &[&usd as &dyn Instrument, &eur as &dyn Instrument],
        &base_market,
        &history,
        as_of,
        &VarConfig::var_95(),
        PricingDispatch::InstrumentDefault,
        None,
    )
    .expect_err(
        "mixed-currency VaR must not aggregate native-currency P&L without a reporting currency",
    );
    assert!(
        err.to_string().contains("reporting currency"),
        "expected reporting currency validation error, got: {err}"
    );
}

#[test]
fn mixed_currency_var_converts_pnl_into_reporting_currency() -> Result<()> {
    let as_of = sample_as_of();
    let usd = CurrencyScalarInstrument::new("USD-INST", "USD-INST", Currency::USD);
    let eur = CurrencyScalarInstrument::new("EUR-INST", "EUR-INST", Currency::EUR);

    let provider = finstack_quant_core::money::fx::SimpleFxProvider::new();
    provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 2.0)])
        .expect("valid rate");
    let fx = finstack_quant_core::money::fx::FxMatrix::new(Arc::new(provider), None);

    let base_market = MarketContext::new()
        .insert_price(
            "USD-INST",
            MarketScalar::price(Money::from((100_i64, Currency::USD))),
        )
        .insert_price(
            "EUR-INST",
            MarketScalar::price(Money::from((100_i64, Currency::EUR))),
        )
        .insert_fx(fx);

    let scenario = MarketScenario::new(
        as_of,
        vec![
            RiskFactorShift {
                factor: RiskFactorType::EquitySpot {
                    ticker: "USD-INST".to_string(),
                },
                shift: 0.10,
            },
            RiskFactorShift {
                factor: RiskFactorType::EquitySpot {
                    ticker: "EUR-INST".to_string(),
                },
                shift: 0.10,
            },
        ],
    );
    let history = MarketHistory::new(as_of, 1, vec![scenario]);

    let result = calculate_var(
        &[&usd as &dyn Instrument, &eur as &dyn Instrument],
        &base_market,
        &history,
        as_of,
        &VarConfig::var_95().with_reporting_currency(Currency::USD),
        PricingDispatch::InstrumentDefault,
        None,
    )?;
    assert!((result.pnl_distribution[0] - 30.0).abs() < 1e-12);
    Ok(())
}

#[test]
fn test_taylor_var_converts_pnl_at_scenario_fx() -> Result<()> {
    let as_of = sample_as_of();

    let provider = finstack_quant_core::money::fx::SimpleFxProvider::new();
    provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 2.0)])
        .expect("valid base rate");
    let fx = finstack_quant_core::money::fx::FxMatrix::new(Arc::new(provider), None);
    let base_market = MarketContext::new().insert_fx(fx);

    let scenario = MarketScenario::new(
        as_of,
        vec![RiskFactorShift {
            factor: RiskFactorType::FxSpot {
                base: Currency::EUR,
                quote: Currency::USD,
            },
            shift: 0.50,
        }],
    );

    let local_pnl = Money::from((10_i64, Currency::EUR));

    let base_converted = convert_amount_to_reporting(
        local_pnl.amount(),
        local_pnl.currency(),
        Currency::USD,
        &base_market,
        as_of,
        "Historical VaR",
    )?;
    assert!(
        (base_converted - 20.0).abs() < 1e-9,
        "base-market conversion should use the base rate (2.0), got {base_converted}"
    );

    let scenario_market = apply_market_scenario(&scenario, &base_market, None)?;
    let scenario_converted = convert_amount_to_reporting(
        local_pnl.amount(),
        local_pnl.currency(),
        Currency::USD,
        &scenario_market,
        as_of,
        "Historical VaR",
    )?;
    assert!(
        (scenario_converted - 30.0).abs() < 1e-9,
        "Taylor P&L must be converted at scenario FX (expected 30 USD), got {scenario_converted}"
    );
    assert!(
        (scenario_converted - base_converted).abs() > 1.0,
        "scenario FX conversion must differ from base FX conversion"
    );
    Ok(())
}

#[test]
fn test_var_result_from_distribution() {
    let pnls = vec![100.0, 50.0, 0.0, -25.0, -50.0, -100.0, -150.0, -200.0];

    let result = VarResult::from_distribution(pnls, 0.95).expect("pnl distribution is finite");

    assert_eq!(result.num_scenarios, 8);
    assert!(
        (result.var - (-182.5)).abs() < 1e-9,
        "interpolated 95% VaR should be 182.5, got {}",
        result.var
    );
    assert!(
        (result.expected_shortfall - (-191.25)).abs() < 1e-9,
        "interpolated 95% ES should be -191.25, got {}",
        result.expected_shortfall
    );
    assert!(result.expected_shortfall <= result.var);
}

#[derive(Clone, Debug)]
struct CountingQuadraticSpotInstrument {
    id: String,
    spot_id: String,
    pricings: Arc<std::sync::atomic::AtomicUsize>,
    attributes: Attributes,
}

crate::impl_empty_cashflow_provider!(
    CountingQuadraticSpotInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl Instrument for CountingQuadraticSpotInstrument {
    fn includes_valuation_date_cashflows(&self) -> bool {
        true
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::MarketDependencies> {
        let mut deps = crate::instruments::MarketDependencies::new();
        deps.add_market_scalar_id(self.spot_id.as_str());
        Ok(deps)
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> crate::pricer::InstrumentType {
        crate::pricer::InstrumentType::Equity
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

    fn base_value(&self, market: &MarketContext, _as_of: Date) -> Result<Money> {
        self.pricings
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let spot = market
            .get_price(&self.spot_id)?
            .get_unitless()
            .unwrap_or_default();
        let value = 100.0 * spot + 0.5 * (spot - 100.0).powi(2);
        Money::new(value, Currency::USD)
    }
}

#[test]
fn taylor_var_precomputes_sensitivities_in_constant_pricings_across_scenarios() -> Result<()> {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let as_of = sample_as_of();
    let pricings = Arc::new(AtomicUsize::new(0));
    let instrument = CountingQuadraticSpotInstrument {
        id: "COUNTING-EQ".to_string(),
        spot_id: "SPOT-COUNT".to_string(),
        pricings: Arc::clone(&pricings),
        attributes: Attributes::new(),
    };
    let market = MarketContext::new().insert_price(
        "SPOT-COUNT",
        MarketScalar::unitless(100.0).expect("valid scalar"),
    );

    let scenarios: Vec<MarketScenario> = (0..50)
        .map(|i| {
            let shift = (i as f64 - 25.0) * 0.004;
            MarketScenario::new(
                as_of,
                vec![RiskFactorShift {
                    factor: RiskFactorType::EquitySpot {
                        ticker: "SPOT-COUNT".to_string(),
                    },
                    shift,
                }],
            )
        })
        .collect();
    let history = history_from_scenarios(as_of, 50, scenarios);

    let taylor = calculate_var(
        &[&instrument as &dyn Instrument],
        &market,
        &history,
        as_of,
        &VarConfig::var_95().with_method(VarMethod::TaylorApproximation),
        PricingDispatch::InstrumentDefault,
        None,
    )?;

    let taylor_pricings = pricings.load(Ordering::Relaxed);
    assert!(
        taylor_pricings <= 5,
        "Taylor VaR should precompute sensitivities in O(1) pricings per factor, got {taylor_pricings}"
    );
    assert_eq!(taylor.num_scenarios, 50);

    // For a quadratic payoff V(r) = 10_000 + 10_000*r + 5_000*r^2,
    // second-order Taylor approximation matches full revaluation exactly.
    let full = calculate_var(
        &[&instrument as &dyn Instrument],
        &market,
        &history,
        as_of,
        &VarConfig::var_95(),
        PricingDispatch::InstrumentDefault,
        None,
    )?;
    for (t_pnl, f_pnl) in taylor.pnl_distribution.iter().zip(&full.pnl_distribution) {
        assert!(
            (t_pnl - f_pnl).abs() < 1e-6,
            "Taylor P&L ({t_pnl}) should match exact quadratic P&L ({f_pnl})"
        );
    }

    Ok(())
}

#[test]
fn test_var_result_rejects_invalid_confidence() {
    for confidence_level in [0.0, 1.0, f64::NAN, f64::INFINITY] {
        let err = VarResult::from_distribution(vec![-10.0, 5.0], confidence_level)
            .expect_err("invalid confidence should be rejected");
        assert!(
            err.to_string().contains("strictly between 0 and 1"),
            "unexpected error for confidence={confidence_level}: {err}"
        );
    }
}

#[test]
fn test_var_result_all_gain_distribution_has_zero_loss() {
    let result = VarResult::from_distribution(vec![10.0, 20.0, 30.0], 0.95)
        .expect("valid all-gain distribution");

    assert_eq!(result.var, 0.0);
    assert_eq!(result.expected_shortfall, 0.0);
}

#[test]
fn test_var_result_rejects_nan() {
    let pnls = vec![10.0, f64::NAN, -5.0];
    let err = VarResult::from_distribution(pnls, 0.95).expect_err("should reject NaNs");
    match err {
        finstack_quant_core::Error::Validation(msg) => {
            assert!(
                msg.contains("non-finite"),
                "error message should mention non-finite values"
            );
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn empty_portfolio_has_explicit_zero_var_state() -> Result<()> {
    let as_of = sample_as_of();
    let base_market = usd_ois_market(as_of)?;
    let history = history_from_scenarios(as_of, 0, vec![]);
    let result = calculate_var(
        &[],
        &base_market,
        &history,
        as_of,
        &VarConfig::var_95(),
        PricingDispatch::InstrumentDefault,
        None,
    )?;
    assert_eq!(result.var, 0.0);
    assert_eq!(result.expected_shortfall, 0.0);
    assert_eq!(result.num_scenarios, 0);
    Ok(())
}

#[test]
fn factor_sensitivity_has_zero_curvature_for_fixed_discount_linear_forward() -> Result<()> {
    let factor = RiskFactorType::ForwardRate {
        curve_id: "F".into(),
        tenor_years: 1.4,
        grid_years: vec![1.4],
    };
    let coefficient = 1_000_000.0 * 0.25 * 0.97;
    let (delta, gamma) =
        compute_factor_sensitivity(&factor, f64::INFINITY, coefficient * 0.04, |h| {
            Ok(coefficient * (0.04 + h))
        })?;
    let shift = 0.001;
    let value = delta * shift + 0.5 * gamma * shift * shift;
    assert!(gamma.abs() < 1e-6);
    assert!((value - coefficient * shift).abs() < 1e-8);
    Ok(())
}
