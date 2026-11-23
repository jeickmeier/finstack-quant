#[allow(dead_code, unused_imports)]
mod test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/attribution_test_utils.rs"
    ));
}

use super::engine::*;
use super::factors::*;
use crate::AttributionMethod;
use crate::AttributionRequest;
use crate::ExecutionPolicy;
use crate::model_params::ModelParamsSnapshot;
use crate::pnl_support::*;
use finstack_quant_calibration::recalibration::CachedRecalibrationProvider;
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::money::fx::FxConversionPolicy;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::STANDARD_BUCKETS_YEARS;
use std::sync::Arc;
use test_utils::TestInstrument;
use time::macros::date;

#[test]
fn test_taylor_config_default() {
    let config = TaylorAttributionConfig::default();
    assert!(!config.include_gamma);
    assert_eq!(config.rate_bump_bp, 1.0);
    assert_eq!(config.credit_spread_bump_bp, 1.0);
    assert_eq!(config.vol_bump, 0.01);
}

#[test]
fn test_taylor_config_validation() {
    let mut config = TaylorAttributionConfig::default();
    assert!(config.validate().is_ok());

    config.vol_bump = 0.20;
    assert!(config.validate().is_ok());

    config.vol_bump = 0.21;
    assert!(config.validate().is_err());

    config.vol_bump = 0.0;
    assert!(config.validate().is_err());

    config.vol_bump = -0.01;
    assert!(config.validate().is_err());
}

#[test]
fn taylor_config_rejects_every_non_finite_bump() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for config in [
            TaylorAttributionConfig {
                rate_bump_bp: invalid,
                ..Default::default()
            },
            TaylorAttributionConfig {
                credit_spread_bump_bp: invalid,
                ..Default::default()
            },
            TaylorAttributionConfig {
                vol_bump: invalid,
                ..Default::default()
            },
        ] {
            assert!(config.validate().is_err());
        }
    }
}

#[test]
fn taylor_config_rejects_retired_credit_bump_key() {
    // schema-rejection-test
    let err = serde_json::from_str::<TaylorAttributionConfig>(r#"{"credit_bump_bp": 1.0}"#)
        .expect_err("retired credit bump key must be rejected");
    assert!(err.to_string().contains("unknown field"), "{err}");
    let parsed: TaylorAttributionConfig =
        serde_json::from_str(r#"{"credit_spread_bump_bp": 2.0}"#).expect("canonical key");
    assert_eq!(parsed.credit_spread_bump_bp, 2.0);
}

#[test]
fn test_taylor_config_serde_roundtrip() {
    let config = TaylorAttributionConfig {
        include_gamma: true,
        rate_bump_bp: 0.5,
        credit_spread_bump_bp: 2.0,
        vol_bump: 0.005,
    };

    let json = serde_json::to_string(&config).expect("serialize should succeed");
    let parsed: TaylorAttributionConfig =
        serde_json::from_str(&json).expect("deserialize should succeed");

    assert_eq!(parsed, config);
    assert!(
        serde_json::from_str::<TaylorAttributionConfig>(
            r#"{"include_gamma":false,"unexpected":true}"#
        )
        .is_err()
    );

    let schema = serde_json::to_value(schemars::schema_for!(TaylorAttributionConfig))
        .expect("Taylor attribution config schema");
    assert_eq!(schema["additionalProperties"], false);
}

#[test]
fn test_taylor_attribution_empty_market() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);

    let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
        "TEST-001",
        Money::from((1000_i64, Currency::USD)),
    ));

    let market_t0 = MarketContext::new();
    let market_t1 = MarketContext::new();
    let config = TaylorAttributionConfig::default();

    let finstack_config = finstack_quant_core::config::FinstackConfig::default();
    let request = AttributionRequest {
        execution_policy: ExecutionPolicy::Parallel,
        ..AttributionRequest::new(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &finstack_config,
        )
    };
    let result = attribute_pnl_taylor(&request, &config).expect("simple attribution");
    assert!(result.total_pnl.amount().abs() < 1e-10);
}

#[test]
fn test_taylor_compat_produces_pnl_attribution() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);

    let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
        "TEST-001",
        Money::from((1000_i64, Currency::USD)),
    ));

    let market_t0 = MarketContext::new();
    let market_t1 = MarketContext::new();
    let config = TaylorAttributionConfig::default();

    let attribution = crate::attribute_pnl(
        &AttributionMethod::Taylor(config),
        &crate::AttributionRequest {
            execution_policy: ExecutionPolicy::Parallel,
            ..crate::AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
    )
    .expect("taylor compat attribution should succeed");

    assert_eq!(attribution.meta.instrument_id, "TEST-001");
    assert!(matches!(
        attribution.meta.method,
        AttributionMethod::Taylor(_)
    ));
}

#[test]
fn taylor_attribution_includes_forward_curve_factors() {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;

    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);

    let fwd_t0 =
        ForwardCurve::flat("TEST-FWD", as_of_t0, 0.03, DayCount::Act360).expect("forward curve");
    let fwd_t1 =
        ForwardCurve::flat("TEST-FWD", as_of_t0, 0.04, DayCount::Act360).expect("forward curve");

    let market_t0 = MarketContext::new()
        .try_insert(fwd_t0)
        .expect("insert market curve");
    let market_t1 = MarketContext::new()
        .try_insert(fwd_t1)
        .expect("insert market curve");

    let instrument: Arc<dyn Instrument> = Arc::new(
        TestInstrument::new("FWDI", Money::from((0_i64, Currency::USD)))
            .with_forward_curves(&["TEST-FWD"]),
    );

    let config = TaylorAttributionConfig::default();
    let result = compute_taylor_result(
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Parallel,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
        &config,
    )
    .expect("taylor attribution should succeed");

    assert!(
        result
            .factors
            .iter()
            .any(|f| f.factor_name.starts_with("Forward:")),
        "expected forward curve factor, got {:?}",
        result.factors
    );
}

/// Cross-currency test instrument whose USD price reads the EUR/USD FX rate
/// from the market's FX matrix. Used to verify Taylor buckets FX-exposure
/// P&L into `fx_pnl` rather than `residual`.
#[derive(Clone)]
struct FxLinkedInstrument {
    id: String,
    /// EUR notional revalued in USD via the market FX rate.
    eur_notional: f64,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    FxLinkedInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl Instrument for FxLinkedInstrument {
    fn includes_valuation_date_cashflows(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> finstack_quant_valuations::pricer::InstrumentType {
        finstack_quant_valuations::pricer::InstrumentType::Bond
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn attributes(&self) -> &finstack_quant_core::types::Attributes {
        use std::sync::OnceLock;
        static ATTRS: OnceLock<finstack_quant_core::types::Attributes> = OnceLock::new();
        ATTRS.get_or_init(finstack_quant_core::types::Attributes::default)
    }

    fn attributes_mut(&mut self) -> &mut finstack_quant_core::types::Attributes {
        unreachable!("FxLinkedInstrument::attributes_mut should not be called")
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
    {
        Ok(finstack_quant_valuations::instruments::MarketDependencies::new())
    }

    fn base_value(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        // Price in USD as the EUR notional converted at the market FX rate.
        let usd = market
            .convert_money(
                Money::new(self.eur_notional, Currency::EUR).expect("valid money fixture"),
                Currency::USD,
                as_of,
                FxConversionPolicy::CashflowDate,
            )?
            .amount;
        Ok(usd)
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        _metrics: &[finstack_quant_valuations::metrics::MetricId],
        _options: finstack_quant_valuations::instruments::PricingOptions,
    ) -> std::result::Result<
        finstack_quant_valuations::results::ValuationResult,
        finstack_quant_valuations::pricer::PricingError,
    > {
        Ok(
            finstack_quant_valuations::results::ValuationResult::stamped(
                self.id(),
                as_of,
                self.value(market, as_of)?,
            ),
        )
    }
}

#[test]
fn taylor_buckets_fx_exposure_into_fx_pnl() {
    use finstack_quant_core::Error;
    use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};

    // FX provider with a deterministic EUR/USD rate.
    struct FixedFx(f64);
    impl FxProvider for FixedFx {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> Result<f64> {
            if from == to {
                Ok(1.0)
            } else if from == Currency::EUR && to == Currency::USD {
                Ok(self.0)
            } else if from == Currency::USD && to == Currency::EUR {
                Ok(1.0 / self.0)
            } else {
                Err(Error::Validation("FX rate not found".to_string()))
            }
        }
    }

    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);

    // USD-priced instrument whose value is a 1,000,000 EUR notional revalued
    // at the market EUR/USD rate. Only the FX rate moves between T0 and T1.
    let instrument: Arc<dyn Instrument> = Arc::new(FxLinkedInstrument {
        id: "FX-LINKED-001".to_string(),
        eur_notional: 1_000_000.0,
    });

    // T0: EUR/USD = 1.10, T1: EUR/USD = 1.20 (EUR appreciates).
    let market_t0 = MarketContext::new().insert_fx(FxMatrix::new(Arc::new(FixedFx(1.10)), None));
    let market_t1 = MarketContext::new().insert_fx(FxMatrix::new(Arc::new(FixedFx(1.20)), None));

    let config = TaylorAttributionConfig::default();
    let attribution = crate::attribute_pnl(
        &AttributionMethod::Taylor(config.clone()),
        &crate::AttributionRequest {
            execution_policy: ExecutionPolicy::Parallel,
            ..crate::AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
    )
    .expect("taylor standard attribution should succeed");

    // USD P&L: 1_000_000 EUR * (1.20 - 1.10) = 100_000 USD, driven entirely
    // by the FX-rate move.
    assert_eq!(attribution.total_pnl.currency(), Currency::USD);
    assert!(
        (attribution.total_pnl.amount() - 100_000.0).abs() < 1e-6,
        "total_pnl = {}",
        attribution.total_pnl
    );

    // REGRESSION: the FX-driven P&L must land in `fx_pnl`, NOT `residual`.
    assert!(
        (attribution.fx_pnl.amount() - 100_000.0).abs() < 1e-6,
        "fx_pnl should capture the FX-exposure P&L, got {}",
        attribution.fx_pnl
    );
    assert!(
        attribution.residual.amount().abs() < 1e-6,
        "residual should be ~0 once FX P&L is bucketed, got {}",
        attribution.residual
    );

    // The exported Fx row carries the repriced value behind the bucket:
    // fx_pnl = pv_t1 − (T1 value with the T0 FX matrix restored).
    let fx_row = attribution
        .sensitivity_steps
        .iter()
        .find(|step| step.factor == "Fx")
        .expect("exported Fx row");
    let repriced = fx_row.repriced_pv.expect("repriced value").amount();
    assert!(
        (repriced - 1_100_000.0).abs() < 1e-6,
        "repriced = {repriced}"
    );
    let pv_t1 = attribution.pv_t1.expect("pv_t1").amount();
    assert!((pv_t1 - repriced - attribution.fx_pnl.amount()).abs() < 1e-6);
    assert_eq!(fx_row.explained_pnl, attribution.fx_pnl);
    assert!(fx_row.sensitivity.is_none() && fx_row.buckets.is_empty());

    // The internal Taylor factor decomposition should also expose an "Fx" factor.
    let taylor = compute_taylor_result(
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Parallel,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
        &config,
    )
    .expect("taylor attribution should succeed");
    assert!(
        taylor.factors.iter().any(|f| f.factor_name == "Fx"),
        "expected an Fx factor, got {:?}",
        taylor.factors
    );
}

/// Malformed config bumps (≤ 0 or > sane max) must be rejected at
/// validation rather than producing a `result_invalid` flagged result.
#[test]
fn taylor_rejects_non_positive_bump_at_validation() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;

    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);

    let instrument: Arc<dyn Instrument> = Arc::new(
        TestInstrument::new("NF-001", Money::from((1000_i64, Currency::USD)))
            .with_discount_curves(&["USD-OIS"]),
    );

    let curve = |base, df1| {
        DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots(vec![(0.0, 1.0), (1.0, df1)])
            .interp(InterpStyle::Linear)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("discount curve")
    };
    let market_t0 = MarketContext::new()
        .try_insert(curve(as_of_t0, 0.98))
        .expect("insert market curve");
    let market_t1 = MarketContext::new()
        .try_insert(curve(as_of_t1, 0.97))
        .expect("insert market curve");

    for bad in [
        TaylorAttributionConfig {
            rate_bump_bp: 0.0,
            ..TaylorAttributionConfig::default()
        },
        TaylorAttributionConfig {
            rate_bump_bp: -1.0,
            ..TaylorAttributionConfig::default()
        },
        TaylorAttributionConfig {
            credit_spread_bump_bp: 0.0,
            ..TaylorAttributionConfig::default()
        },
        TaylorAttributionConfig {
            credit_spread_bump_bp: 200.0,
            ..TaylorAttributionConfig::default()
        },
    ] {
        let err = crate::attribute_pnl(
            &AttributionMethod::Taylor(bad.clone()),
            &crate::AttributionRequest {
                execution_policy: ExecutionPolicy::Parallel,
                ..crate::AttributionRequest::new(
                    &instrument,
                    &market_t0,
                    &market_t1,
                    as_of_t0,
                    as_of_t1,
                    &finstack_quant_core::config::FinstackConfig::default(),
                )
            },
        )
        .expect_err("malformed bump config must error at validation");
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("bump"),
            "validation error must mention 'bump', got: {msg}"
        );
    }
}

// ── Audit-fix regression tests (2026-08) ───────────────────────────────
//
// Cover: B7 cross-bucket gamma, credit key-rate gamma, multi-surface vega,
// vol reference-point measurement, factor-failure visibility, bump noise
// floor, total-return unexplained basis, and FX presence on either side.

use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::surfaces::{
    VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_valuations::instruments::VolatilityDependency;

pub(crate) fn extract_mock_model_params(
    instrument: &dyn Instrument,
) -> Option<ModelParamsSnapshot> {
    let mock = instrument.as_any().downcast_ref::<MockInstrument>()?;
    Some(match mock.model_cpr {
        Some(cpr) => {
            use finstack_quant_cashflows::builder::{
                DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec,
            };
            ModelParamsSnapshot::StructuredCredit {
                prepayment_spec: PrepaymentModelSpec::constant_cpr(cpr),
                default_spec: DefaultModelSpec::constant_cdr(0.0),
                recovery_spec: RecoveryModelSpec::with_lag(0.0, 0),
            }
        }
        None => ModelParamsSnapshot::None,
    })
}

pub(crate) fn apply_mock_model_params(
    instrument: &Arc<dyn Instrument>,
    params: &ModelParamsSnapshot,
) -> Option<Result<Arc<dyn Instrument>>> {
    let mock = instrument.as_any().downcast_ref::<MockInstrument>()?;
    Some(match params {
        ModelParamsSnapshot::None => Ok(Arc::clone(instrument)),
        ModelParamsSnapshot::StructuredCredit {
            prepayment_spec, ..
        } => {
            let mut clone = mock.clone();
            clone.model_cpr = Some(match prepayment_spec {
                finstack_quant_cashflows::builder::PrepaymentModelSpec::Constant { cpr } => *cpr,
                _ => {
                    return Some(Err(finstack_quant_core::Error::Validation(
                        "MockInstrument requires a constant CPR model".into(),
                    )));
                }
            });
            Ok(Arc::new(clone))
        }
        ModelParamsSnapshot::Convertible { .. } => Err(finstack_quant_core::Error::Validation(
            "MockInstrument does not accept convertible model parameters".to_string(),
        )),
    })
}

/// Payoff shapes for [`MockInstrument`].
#[derive(Clone)]
enum MockPayoff {
    /// `notional × df(tenor)` read from a discount curve.
    DiscountZero {
        curve: &'static str,
        tenor: f64,
        notional: f64,
    },
    /// Long 1Y zero and duration-matched short 5Y zero.
    HedgedZeros { short_notional: f64 },
    /// `scale × exp(−rate(tenor)·tenor)` read from a forward curve.
    ForwardConvex {
        curve: &'static str,
        tenor: f64,
        scale: f64,
    },
    /// `scale × Σ finstack_quant_models::volatility::get_surface_vol_clamped(&surface, expiry, strike)` over `reads`.
    VolSum {
        reads: Vec<(&'static str, f64, f64)>,
        scale: f64,
    },
    /// EUR notional converted at the market FX rate when an FX matrix is
    /// present; face value in USD otherwise.
    FxOptional { eur_notional: f64 },
    /// `scale × price.amount()` from a market scalar price.
    Spot { id: &'static str, scale: f64 },
    /// `scale × cpi(tenor)` from an inflation curve.
    InflationCpi {
        curve: &'static str,
        tenor: f64,
        scale: f64,
    },
    /// `scale × correlation(detachment)` from a base-correlation curve.
    Correlation {
        curve: &'static str,
        detachment: f64,
        scale: f64,
    },
    /// `scale × model_cpr` from the instrument's model-parameter snapshot.
    ModelCpr { scale: f64 },
    /// Market-independent constant USD value.
    Constant(f64),
}

/// Flexible market-reading mock used by the audit-fix regression tests.
#[derive(Clone)]
struct MockInstrument {
    id: String,
    payoff: MockPayoff,
    discount_curves: Vec<CurveId>,
    forward_curves: Vec<CurveId>,
    credit_curves: Vec<CurveId>,
    vol_deps: Vec<VolatilityDependency>,
    expiry: Option<Date>,
    /// `(measure key, value)` pairs surfaced through `price_with_metrics`.
    keyrate_measures: Vec<(String, f64)>,
    /// Optional fixed coupon for the theta window.
    coupon: Option<(Date, Money)>,
    /// Optional schedule-construction failure independent of pricing.
    cashflow_error: Option<&'static str>,
    /// Optional CPR used by [`MockPayoff::ModelCpr`] and the model-param hooks.
    model_cpr: Option<f64>,
}

impl MockInstrument {
    fn new(id: &str, payoff: MockPayoff) -> Self {
        Self {
            id: id.to_string(),
            payoff,
            discount_curves: Vec::new(),
            forward_curves: Vec::new(),
            credit_curves: Vec::new(),
            vol_deps: Vec::new(),
            expiry: None,
            keyrate_measures: Vec::new(),
            coupon: None,
            cashflow_error: None,
            model_cpr: None,
        }
    }
}

impl finstack_quant_cashflows::CashflowScheduleSource for MockInstrument {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(None)
    }

    fn raw_cashflow_schedule(
        &self,
        _market: &MarketContext,
        _as_of: Date,
    ) -> Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
        use finstack_quant_core::cashflow::{CFKind, CashFlow, CashFlowParams};
        if let Some(error) = self.cashflow_error {
            return Err(finstack_quant_core::Error::Validation(error.to_string()));
        }
        let flows: Vec<CashFlow> = self
            .coupon
            .iter()
            .map(|(date, amount)| {
                CashFlowParams::new(*date, None, *amount, CFKind::Fixed, 0.0, None)
                    .build()
                    .expect("valid cashflow")
            })
            .collect();
        Ok(finstack_quant_cashflows::schedule_from_classified_flows(
            flows,
            DayCount::Act365F,
            finstack_quant_cashflows::ScheduleBuildOpts::default(),
        ))
    }
}

impl Instrument for MockInstrument {
    fn includes_valuation_date_cashflows(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> finstack_quant_valuations::pricer::InstrumentType {
        finstack_quant_valuations::pricer::InstrumentType::Bond
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn attributes(&self) -> &finstack_quant_core::types::Attributes {
        use std::sync::OnceLock;
        static ATTRS: OnceLock<finstack_quant_core::types::Attributes> = OnceLock::new();
        ATTRS.get_or_init(finstack_quant_core::types::Attributes::default)
    }

    fn attributes_mut(&mut self) -> &mut finstack_quant_core::types::Attributes {
        unreachable!("MockInstrument::attributes_mut should not be called")
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn expiry(&self) -> Option<Date> {
        self.expiry
    }

    fn market_dependencies(
        &self,
    ) -> Result<finstack_quant_valuations::instruments::MarketDependencies> {
        let mut deps = finstack_quant_valuations::instruments::MarketDependencies::new();
        for id in &self.discount_curves {
            deps.add_discount_curve(id.clone());
        }
        for id in &self.forward_curves {
            deps.add_forward_curve(id.clone());
        }
        for id in &self.credit_curves {
            deps.add_credit_curve(id.clone());
        }
        for dep in &self.vol_deps {
            deps.add_volatility_dependency(dep.clone());
        }
        Ok(deps)
    }

    fn base_value(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        match &self.payoff {
            MockPayoff::DiscountZero {
                curve,
                tenor,
                notional,
            } => {
                let df = market.get_discount(curve)?.df(*tenor)?;
                Ok(Money::new(notional * df, Currency::USD).expect("valid money fixture"))
            }
            MockPayoff::HedgedZeros { short_notional } => {
                let curve = market.get_discount("HEDGE")?;
                Money::new(
                    1_000_000.0 * curve.df(1.0)? - short_notional * curve.df(5.0)?,
                    Currency::USD,
                )
            }
            MockPayoff::ForwardConvex {
                curve,
                tenor,
                scale,
            } => {
                let factor = market.get_forward(curve)?.df(*tenor)?;
                Ok(Money::new(scale * factor, Currency::USD).expect("valid money fixture"))
            }
            MockPayoff::VolSum { reads, scale } => {
                let mut total = 0.0;
                for (surface_id, expiry, strike) in reads {
                    let surface = market.get_surface(surface_id)?;
                    total += finstack_quant_models::volatility::get_surface_vol_clamped(
                        &surface, *expiry, *strike,
                    );
                }
                Ok(Money::new(scale * total, Currency::USD).expect("valid money fixture"))
            }
            MockPayoff::FxOptional { eur_notional } => {
                if market.fx().is_some() {
                    market
                        .convert_money(
                            Money::new(*eur_notional, Currency::EUR).expect("valid money fixture"),
                            Currency::USD,
                            as_of,
                            FxConversionPolicy::CashflowDate,
                        )
                        .map(|converted| converted.amount)
                } else {
                    Ok(Money::new(*eur_notional, Currency::USD).expect("valid money fixture"))
                }
            }
            MockPayoff::Spot { id, scale } => {
                let s = market.get_price(*id)?;
                let price = s
                    .get_price()
                    .map(|money| money.amount())
                    .or_else(|| s.get_unitless())
                    .ok_or_else(|| {
                        finstack_quant_core::Error::Validation(format!(
                            "unsupported scalar for '{id}'"
                        ))
                    })?;
                Ok(Money::new(scale * price, Currency::USD).expect("valid money fixture"))
            }
            MockPayoff::InflationCpi {
                curve,
                tenor,
                scale,
            } => {
                let cpi = market.get_inflation_curve(*curve)?.cpi(*tenor)?;
                Ok(Money::new(scale * cpi, Currency::USD).expect("valid money fixture"))
            }
            MockPayoff::Correlation {
                curve,
                detachment,
                scale,
            } => {
                let corr = market
                    .get_base_correlation(*curve)?
                    .get_correlation(*detachment)?;
                Ok(Money::new(scale * corr, Currency::USD).expect("valid money fixture"))
            }
            MockPayoff::ModelCpr { scale } => {
                let cpr = self.model_cpr.unwrap_or(0.0);
                Ok(Money::new(scale * cpr, Currency::USD).expect("valid money fixture"))
            }
            MockPayoff::Constant(value) => {
                Ok(Money::new(*value, Currency::USD).expect("valid money fixture"))
            }
        }
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        _metrics: &[finstack_quant_valuations::metrics::MetricId],
        _options: finstack_quant_valuations::instruments::PricingOptions,
    ) -> std::result::Result<
        finstack_quant_valuations::results::ValuationResult,
        finstack_quant_valuations::pricer::PricingError,
    > {
        let mut result = finstack_quant_valuations::results::ValuationResult::stamped(
            self.id(),
            as_of,
            self.value(market, as_of)?,
        );
        for (key, value) in &self.keyrate_measures {
            result.measures.insert(
                finstack_quant_valuations::metrics::MetricId::custom(key.clone()),
                *value,
            );
        }
        Ok(result)
    }
}

/// Discount curve with a flat continuously-compounded zero rate `z`,
/// knotted densely across the key-rate bucket grid (plus 1.5y).
fn flat_zero_curve(id: &'static str, base: Date, z: f64) -> DiscountCurve {
    let tenors = [
        0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0, 7.0, 10.0, 15.0, 20.0, 30.0,
    ];
    let mut knots = vec![(0.0, 1.0)];
    knots.extend(tenors.iter().map(|&t| (t, (-z * t).exp())));
    DiscountCurve::builder(id)
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots(knots)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve")
}

/// Forward curve with a flat continuously-compounded projection zero rate
/// `r`, knotted densely across the bucket grid (plus 1.5y).
fn flat_forward_curve(id: &'static str, base: Date, r: f64) -> ForwardCurve {
    let tenors = [
        0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0, 7.0, 10.0, 15.0, 20.0, 30.0,
    ];
    let mut knots = vec![(0.0, 1.0)];
    knots.extend(tenors.iter().map(|&t| (t, (-r * t).exp())));
    ForwardCurve::builder(id)
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots(knots)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("forward curve")
}

use finstack_quant_core::market_data::term_structures::ForwardCurve;

/// Two-expiry (0.5y / 5y) surface with per-expiry flat vols.
fn two_expiry_surface(id: &'static str, front: f64, back: f64) -> VolSurface {
    VolSurface::builder(id)
        .expiries(&[0.5, 5.0])
        .secondary_values(&[90.0, 110.0])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .row(&[front, front])
        .row(&[back, back])
        .build()
        .expect("vol surface")
}

/// B7: key-rate gamma must capture cross-bucket convexity. A zero maturing
/// between two buckets (t = 1.5y, triangular weights 0.5/0.5) under a
/// +100bp parallel move has analytic convexity P&L ½·t²·PV₀·Δz²; the old
/// diagonal-only per-bucket sum recovered only ~half of it.
#[test]
fn keyrate_gamma_captures_cross_bucket_convexity() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let notional = 100_000_000.0;
    let tenor = 1.5;
    let (z0, z1) = (0.03, 0.04);

    let market_t0 = MarketContext::new()
        .try_insert(flat_zero_curve("USD-OIS", as_of_t0, z0))
        .expect("insert market curve");
    let market_t1 = MarketContext::new()
        .try_insert(flat_zero_curve("USD-OIS", as_of_t1, z1))
        .expect("insert market curve");

    let mut mock = MockInstrument::new(
        "ZC-1.5Y",
        MockPayoff::DiscountZero {
            curve: "USD-OIS",
            tenor,
            notional,
        },
    );
    mock.discount_curves = vec![CurveId::new("USD-OIS")];
    let instrument: Arc<dyn Instrument> = Arc::new(mock);

    let config = TaylorAttributionConfig {
        include_gamma: true,
        ..TaylorAttributionConfig::default()
    };
    let result = compute_taylor_result(
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
        &config,
    )
    .expect("taylor attribution should succeed");

    let factor = result
        .factors
        .iter()
        .find(|f| f.factor_name == "Rates:USD-OIS")
        .expect("rates factor must be present");
    let pv0 = notional * (-z0 * tenor).exp();
    let dz = z1 - z0;
    let expected_gamma = 0.5 * tenor * tenor * pv0 * dz * dz;
    let gamma = factor.gamma_pnl.expect("gamma requested via include_gamma");
    assert!(
        ((gamma - expected_gamma) / expected_gamma).abs() < 0.01,
        "gamma_pnl {gamma:.2} must be within 1% of analytic {expected_gamma:.2} \
         (diagonal-only key-rate gamma understates it ~2x)"
    );
}

#[test]
fn directional_gamma_is_stable_for_dv01_neutral_twist() {
    let start = date!(2025 - 01 - 15);
    let rate = 0.03_f64;
    let short_notional = 1_000_000.0 * (-rate).exp() / (5.0 * (-5.0 * rate).exp());
    let mut mock = MockInstrument::new("NEUTRAL", MockPayoff::HedgedZeros { short_notional });
    mock.discount_curves = vec![CurveId::new("HEDGE")];
    let instrument: Arc<dyn Instrument> = Arc::new(mock);
    let opening = MarketContext::new()
        .try_insert(flat_zero_curve("HEDGE", start, rate))
        .expect("insert market curve");
    let mut moves = vec![0.0; STANDARD_BUCKETS_YEARS.len()];
    for (tenor, movement) in [(1.0, 10.0), (5.0, -10.0)] {
        let i = STANDARD_BUCKETS_YEARS
            .iter()
            .position(|t| *t == tenor)
            .unwrap();
        moves[i] = movement;
    }
    let closing = bump_curve_direction(
        &opening,
        &CurveId::new("HEDGE"),
        &STANDARD_BUCKETS_YEARS,
        &moves,
        1.0,
    )
    .unwrap();
    let factor = compute_curve_factor(CurveFactorInputs {
        kind: CurveKind::Discount,
        instrument: &instrument,
        market_t0: &opening,
        market_t1: &closing,
        as_of_t0: start,
        pv_t0: instrument.value(&opening, start).unwrap(),
        curve_id: &CurveId::new("HEDGE"),
        config: &TaylorAttributionConfig {
            include_gamma: true,
            ..Default::default()
        },
        repricings: &mut 0,
    })
    .unwrap();
    let expected = 0.5
        * (1_000_000.0 * (-rate).exp() - 25.0 * short_notional * (-5.0 * rate).exp())
        * 0.001_f64.powi(2);
    assert!(
        (factor.gamma_pnl.unwrap() - expected).abs() < 1e-3,
        "{factor:?}, expected {expected}"
    );
}

#[test]
fn total_return_methods_propagate_cashflow_failure() {
    let start = date!(2025 - 01 - 15);
    let end = date!(2025 - 02 - 15);
    let mut mock = MockInstrument::new("BROKEN-CASH", MockPayoff::Constant(1000.0));
    mock.cashflow_error = Some("cash schedule unavailable");
    let instrument: Arc<dyn Instrument> = Arc::new(mock);
    let market = MarketContext::new();
    let config = finstack_quant_core::config::FinstackConfig::default();
    for method in [
        AttributionMethod::Parallel,
        AttributionMethod::Waterfall(crate::default_waterfall_order()),
    ] {
        let error = crate::attribute_pnl(
            &method,
            &AttributionRequest::new(&instrument, &market, &market, start, end, &config),
        )
        .unwrap_err();
        assert!(error.to_string().contains("cash schedule unavailable"));
    }
    let opening = finstack_quant_valuations::results::ValuationResult::stamped(
        "BROKEN-CASH",
        start,
        Money::from((1000_i64, Currency::USD)),
    );
    let closing = finstack_quant_valuations::results::ValuationResult::stamped(
        "BROKEN-CASH",
        end,
        opening.value,
    );
    let error = crate::attribute_pnl_metrics_based(
        &instrument,
        &market,
        &market,
        &opening,
        &closing,
        start,
        end,
    )
    .unwrap_err();
    assert!(error.to_string().contains("cash schedule unavailable"));
}

/// B7 (forward-curve copy): same cross-bucket convexity check for the
/// forward-curve factor path.
#[test]
fn forward_keyrate_gamma_captures_cross_bucket_convexity() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let scale = 100_000_000.0;
    let tenor = 1.5;
    let (r0, r1) = (0.03, 0.04);

    let market_t0 = MarketContext::new()
        .try_insert(flat_forward_curve("TEST-FWD", as_of_t0, r0))
        .expect("insert market curve");
    let market_t1 = MarketContext::new()
        .try_insert(flat_forward_curve("TEST-FWD", as_of_t1, r1))
        .expect("insert market curve");

    let mut mock = MockInstrument::new(
        "FWD-CONVEX",
        MockPayoff::ForwardConvex {
            curve: "TEST-FWD",
            tenor,
            scale,
        },
    );
    mock.forward_curves = vec![CurveId::new("TEST-FWD")];
    let instrument: Arc<dyn Instrument> = Arc::new(mock);

    let config = TaylorAttributionConfig {
        include_gamma: true,
        ..TaylorAttributionConfig::default()
    };
    let result = compute_taylor_result(
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
        &config,
    )
    .expect("taylor attribution should succeed");

    let factor = result
        .factors
        .iter()
        .find(|f| f.factor_name == "Forward:TEST-FWD")
        .expect("forward factor must be present");
    let pv0 = scale * (-r0 * tenor).exp();
    let dr = r1 - r0;
    let expected_gamma = 0.5 * tenor * tenor * pv0 * dr * dr;
    let gamma = factor.gamma_pnl.expect("gamma requested via include_gamma");
    assert!(
        ((gamma - expected_gamma) / expected_gamma).abs() < 0.01,
        "forward gamma_pnl {gamma:.2} must be within 1% of analytic {expected_gamma:.2}"
    );
}

#[test]
fn hazard_credit_bumps_replay_quotes_and_require_recipe() {
    use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
    let start = date!(2025 - 01 - 15);
    let discount = flat_zero_curve("USD-OIS", start, 0.03);
    let convention = CdsConventionKey {
        currency: Currency::USD,
        doc_clause: CdsDocClause::IsdaNa,
    };
    let hazard = test_utils::calibrated_hazard_curve(
        &discount,
        start,
        "HAZ",
        "ISSUER",
        0.4,
        convention.clone(),
        &[(1, 100.0), (3, 120.0), (5, 150.0)],
    )
    .unwrap();
    let expected = test_utils::calibrated_hazard_curve(
        &discount,
        start,
        "HAZ",
        "ISSUER",
        0.4,
        convention,
        &[(1, 110.0), (3, 130.0), (5, 160.0)],
    )
    .unwrap();
    let market = MarketContext::new()
        .try_insert(discount)
        .expect("insert market curve")
        .try_insert(hazard)
        .expect("insert market curve");
    let bumped = bump_credit_market(
        &market,
        &CurveId::new("HAZ"),
        10.0,
        &CachedRecalibrationProvider::default(),
    )
    .unwrap();
    let delivered = bumped.get_hazard("HAZ").unwrap();
    for tenor in [0.5, 1.0, 2.0, 3.0, 4.0, 5.0] {
        assert!((delivered.sp(tenor).unwrap() - expected.sp(tenor).unwrap()).abs() < 1e-9);
    }
    let bare = finstack_quant_core::market_data::term_structures::HazardCurve::builder("BARE")
        .base_date(start)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots([(1.0, 0.02), (5.0, 0.02)])
        .build()
        .unwrap();
    let missing = market.try_insert(bare).expect("insert market curve");
    let error = bump_credit_market(
        &missing,
        &CurveId::new("BARE"),
        10.0,
        &CachedRecalibrationProvider::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("calibration recipe"));
}

/// Credit convexity must not vanish on the BucketedCs01 (key-rate) path:
/// with `include_gamma = true` a discount-style credit zero must report a
/// second-order term matching the analytic ½·t²·PV₀·Δz².
#[test]
fn credit_keyrate_path_computes_parallel_gamma() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let notional = 50_000_000.0;
    let tenor = 5.0;
    let (z0, z1) = (0.02, 0.03);

    let market_t0 = MarketContext::new()
        .try_insert(flat_zero_curve("CR-DISC", as_of_t0, z0))
        .expect("insert market curve");
    let market_t1 = MarketContext::new()
        .try_insert(flat_zero_curve("CR-DISC", as_of_t1, z1))
        .expect("insert market curve");

    let pv0 = notional * (-z0 * tenor).exp();
    // Analytic zero-rate CS01 ($/bp) for the 5y bucket.
    let cs01 = -notional * tenor * (-z0 * tenor).exp() / 10_000.0;

    let mut mock = MockInstrument::new(
        "RISKY-ZC",
        MockPayoff::DiscountZero {
            curve: "CR-DISC",
            tenor,
            notional,
        },
    );
    mock.credit_curves = vec![CurveId::new("CR-DISC")];
    mock.keyrate_measures = vec![("bucketed_cs01::CR-DISC::5y".to_string(), cs01)];
    let instrument: Arc<dyn Instrument> = Arc::new(mock);

    let config = TaylorAttributionConfig {
        include_gamma: true,
        ..TaylorAttributionConfig::default()
    };
    let result = compute_taylor_result(
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
        &config,
    )
    .expect("taylor attribution should succeed");

    // Endpoints, one metric evaluation, two directional gamma prices and theta.
    assert_eq!(result.num_repricings, 6);

    let factor = result
        .factors
        .iter()
        .find(|f| f.factor_name == "Credit:CR-DISC")
        .expect("credit factor must be present");
    // Key-rate first-order path must still drive explained P&L.
    assert!(
        (factor.explained_pnl - cs01 * 100.0).abs() < 1.0,
        "key-rate first-order explained P&L must be CS01 x 100bp, got {}",
        factor.explained_pnl
    );
    let dz = z1 - z0;
    let expected_gamma = 0.5 * tenor * tenor * pv0 * dz * dz;
    let gamma = factor
        .gamma_pnl
        .expect("credit gamma must be computed on the key-rate path too");
    assert!(
        ((gamma - expected_gamma) / expected_gamma).abs() < 0.01,
        "credit gamma_pnl {gamma:.2} must be within 1% of analytic {expected_gamma:.2}"
    );
}

/// Vega must cover every volatility dependency, not just the first: with
/// two moved surfaces both must appear as factors with their own P&L.
#[test]
fn vega_covers_all_volatility_dependencies() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);

    let market_t0 = MarketContext::new()
        .insert_surface(two_expiry_surface("VOL-A", 0.20, 0.20))
        .insert_surface(two_expiry_surface("VOL-B", 0.20, 0.20));
    // A: +2 vol points, B: +4 vol points.
    let market_t1 = MarketContext::new()
        .insert_surface(two_expiry_surface("VOL-A", 0.22, 0.22))
        .insert_surface(two_expiry_surface("VOL-B", 0.24, 0.24));

    let mut mock = MockInstrument::new(
        "TWO-VOL",
        MockPayoff::VolSum {
            reads: vec![("VOL-A", 1.0, 100.0), ("VOL-B", 1.0, 100.0)],
            scale: 1_000_000.0,
        },
    );
    mock.vol_deps = vec![
        VolatilityDependency::new("VOL-A", None, Some(100.0)),
        VolatilityDependency::new("VOL-B", None, Some(100.0)),
    ];
    mock.expiry = Some(date!(2026 - 01 - 15));
    let instrument: Arc<dyn Instrument> = Arc::new(mock);

    let config = TaylorAttributionConfig::default();
    let result = compute_taylor_result(
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
        &config,
    )
    .expect("taylor attribution should succeed");

    let factor_a = result
        .factors
        .iter()
        .find(|f| f.factor_name == "Vol:VOL-A")
        .expect("first vol surface factor must be present");
    let factor_b = result
        .factors
        .iter()
        .find(|f| f.factor_name == "Vol:VOL-B")
        .expect("second vol surface factor must not be silently dropped");

    // Linear payoff: vega = $10k/pt per surface; moves +2 / +4 points.
    assert!(
        (factor_a.explained_pnl - 20_000.0).abs() < 1.0,
        "VOL-A explained {}",
        factor_a.explained_pnl
    );
    assert!(
        (factor_b.explained_pnl - 40_000.0).abs() < 1.0,
        "VOL-B explained {}",
        factor_b.explained_pnl
    );
}

/// The vol move must be measured at the instrument's own reference
/// expiry/strike when available: a front-up/back-down term-structure
/// inversion averages to ~0 across the surface, but a front-expiry
/// instrument experienced ≈ +4 vol points.
#[test]
fn vol_move_uses_instrument_reference_point() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);

    // Front +4 pts, back −4 pts: surface-average move ≈ 0.
    let market_t0 = MarketContext::new().insert_surface(two_expiry_surface("VOL-TS", 0.20, 0.28));
    let market_t1 = MarketContext::new().insert_surface(two_expiry_surface("VOL-TS", 0.24, 0.24));

    let mut mock = MockInstrument::new(
        "FRONT-VOL",
        MockPayoff::VolSum {
            reads: vec![("VOL-TS", 0.5, 100.0)],
            scale: 1_000_000.0,
        },
    );
    mock.vol_deps = vec![VolatilityDependency::new("VOL-TS", None, Some(100.0))];
    // 183 days ≈ 0.5y to expiry — the front of the surface.
    mock.expiry = Some(date!(2025 - 07 - 17));
    let instrument: Arc<dyn Instrument> = Arc::new(mock);

    let config = TaylorAttributionConfig::default();
    let result = compute_taylor_result(
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
        &config,
    )
    .expect("taylor attribution should succeed");

    let factor = result
        .factors
        .iter()
        .find(|f| f.factor_name == "Vol:VOL-TS")
        .expect("vol factor must be present");
    assert!(
        (factor.explained_pnl - 40_000.0).abs() < 1_000.0,
        "front-expiry volatility P&L must reflect the +4-point move, got {}",
        factor.explained_pnl
    );
}

#[test]
fn volatility_expiry_times_follow_each_explicit_clock() {
    use finstack_quant_core::dates::{DayCountContextState, VolatilityClock};
    let opening_date = date!(2025 - 01 - 01);
    let closing_date = date!(2025 - 01 - 02);
    let expiry = date!(2025 - 07 - 01);
    let surface = two_expiry_surface("VOL", 0.20, 0.21);
    let clocked = |anchor| {
        surface.clone().with_expiry_clock(
            VolatilityClock::try_new(anchor, DayCount::Act360, DayCountContextState::default())
                .unwrap(),
        )
    };
    let (opening, closing) = volatility_expiry_times(
        &clocked(opening_date),
        &clocked(closing_date),
        expiry,
        opening_date,
    )
    .unwrap();
    assert!((opening - 181.0 / 360.0).abs() < 1e-14);
    assert!((closing - 180.0 / 360.0).abs() < 1e-14);
    let (opening, closing) =
        volatility_expiry_times(&surface, &surface, expiry, opening_date).unwrap();
    assert_eq!(opening.to_bits(), closing.to_bits());
    assert!((opening - 181.0 / 365.0).abs() < 1e-14);
}

/// A missing reference prevents attribution and is recorded on the result.
#[test]
fn vol_without_reference_point_reports_missing_factor() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);

    let market_t0 = MarketContext::new().insert_surface(two_expiry_surface("VOL-X", 0.20, 0.20));
    let market_t1 = MarketContext::new().insert_surface(two_expiry_surface("VOL-X", 0.22, 0.22));

    let mut mock = MockInstrument::new(
        "NO-REF-VOL",
        MockPayoff::VolSum {
            reads: vec![("VOL-X", 1.0, 100.0)],
            scale: 1_000_000.0,
        },
    );
    mock.vol_deps = vec![VolatilityDependency::new("VOL-X", None, None)];
    let instrument: Arc<dyn Instrument> = Arc::new(mock);

    let attribution = crate::attribute_pnl(
        &AttributionMethod::Taylor(TaylorAttributionConfig::default()),
        &crate::AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..crate::AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
    )
    .expect("taylor attribution should succeed");

    assert!(
        attribution
            .meta
            .notes
            .iter()
            .any(|n| n.contains("VOL-X") && n.contains("requires a reference expiry and strike")),
        "metadata must identify the missing volatility reference, got {:?}",
        attribution.meta.notes
    );
}

/// Factor failures must be visible: a curve present in market dependencies
/// but missing from one market must be recorded in metadata notes and flag
/// the result invalid — while the run still completes.
#[test]
fn failed_factor_is_recorded_in_notes_and_flags_invalid() {
    use finstack_quant_core::math::interp::InterpStyle;

    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);

    let instrument: Arc<dyn Instrument> = Arc::new(
        TestInstrument::new("FAIL-001", Money::from((1000_i64, Currency::USD)))
            .with_discount_curves(&["USD-OIS"]),
    );

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of_t0)
        .knots(vec![(0.0, 1.0), (30.0, 0.5)])
        .interp(InterpStyle::Linear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let market_t0 = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve");
    // T1 lacks the curve entirely → the rates factor must fail.
    let market_t1 = MarketContext::new();

    let attribution = crate::attribute_pnl(
        &AttributionMethod::Taylor(TaylorAttributionConfig::default()),
        &crate::AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..crate::AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
    )
    .expect("attribution must complete despite the failed factor");

    assert!(
        attribution
            .meta
            .notes
            .iter()
            .any(|n| n.contains("USD-OIS") && n.contains("failed")),
        "failed factor must be recorded in metadata notes, got {:?}",
        attribution.meta.notes
    );
    assert!(
        attribution.result_invalid,
        "a failed dependency-backed factor must flag the result invalid"
    );
}

/// Bumps below the second-difference noise floor must be rejected at
/// validation (Press et al., Numerical Recipes §5.7).
#[test]
fn taylor_rejects_sub_noise_floor_bumps() {
    for bad in [
        TaylorAttributionConfig {
            rate_bump_bp: 1e-6,
            ..TaylorAttributionConfig::default()
        },
        TaylorAttributionConfig {
            rate_bump_bp: 0.009,
            ..TaylorAttributionConfig::default()
        },
        TaylorAttributionConfig {
            credit_spread_bump_bp: 1e-6,
            ..TaylorAttributionConfig::default()
        },
        TaylorAttributionConfig {
            vol_bump: 1e-5,
            ..TaylorAttributionConfig::default()
        },
    ] {
        assert!(
            bad.validate().is_err(),
            "sub-noise-floor bump must fail validation: {bad:?}"
        );
    }

    // Boundary values remain valid.
    let ok = TaylorAttributionConfig {
        rate_bump_bp: 0.01,
        credit_spread_bump_bp: 0.01,
        vol_bump: 1e-4,
        ..TaylorAttributionConfig::default()
    };
    assert!(ok.validate().is_ok());
}

/// Public total P&L and residual include realized period coupon income.
#[test]
fn unexplained_uses_total_return_actual_pnl() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 02 - 15);

    let mut mock = MockInstrument::new("COUPON-001", MockPayoff::Constant(1_000_000.0));
    mock.coupon = Some((
        date!(2025 - 02 - 01),
        Money::from((5_000_i64, Currency::USD)),
    ));
    let instrument: Arc<dyn Instrument> = Arc::new(mock);

    let market = MarketContext::new();
    let finstack_config = finstack_quant_core::config::FinstackConfig::default();
    let request = AttributionRequest::new(
        &instrument,
        &market,
        &market,
        as_of_t0,
        as_of_t1,
        &finstack_config,
    );
    let result = attribute_pnl_taylor(&request, &TaylorAttributionConfig::default())
        .expect("coupon attribution");
    assert!(!result.result_invalid);
    assert!((result.total_pnl.amount() - 5_000.0).abs() < 1e-6);
    assert!(result.residual.amount().abs() < 1e-6);
}

#[test]
fn theta_cashflow_failures_are_recorded_without_a_successful_theta() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 02 - 15);
    let market = MarketContext::new();
    let config = TaylorAttributionConfig::default();
    let finstack_config = finstack_quant_core::config::FinstackConfig::default();

    for (cashflow_error, coupon_currency, expected_error) in [
        (
            Some("coupon schedule unavailable"),
            Currency::USD,
            "coupon schedule unavailable",
        ),
        (None, Currency::EUR, "fx_matrix"),
    ] {
        let mut mock = MockInstrument::new("FAILED-COUPON", MockPayoff::Constant(1_000_000.0));
        mock.coupon = Some((
            date!(2025 - 02 - 01),
            Money::from((5_000_i64, coupon_currency)),
        ));
        mock.cashflow_error = cashflow_error;
        let instrument: Arc<dyn Instrument> = Arc::new(mock);

        let result = compute_taylor_result(
            &AttributionRequest {
                execution_policy: ExecutionPolicy::Serial,
                ..AttributionRequest::new(
                    &instrument,
                    &market,
                    &market,
                    as_of_t0,
                    as_of_t1,
                    &finstack_quant_core::config::FinstackConfig::default(),
                )
            },
            &config,
        )
        .expect("failed period cashflow collection should retain diagnostic attribution");

        // Failed cash collection follows an attempted theta valuation.
        assert_eq!(result.num_repricings, 3);
        assert!(result.result_invalid);
        assert_eq!(result.theta_coupon_income, None);
        assert!(
            result
                .factors
                .iter()
                .all(|factor| factor.factor_name != "Theta")
        );
        assert!(result.notes.iter().any(|note| {
            note.contains(expected_error)
                && note.contains("total P&L excludes period cash receipts")
        }));

        let attribution = crate::attribute_pnl(
            &AttributionMethod::Taylor(config.clone()),
            &crate::AttributionRequest {
                execution_policy: ExecutionPolicy::Serial,
                ..crate::AttributionRequest::new(
                    &instrument,
                    &market,
                    &market,
                    as_of_t0,
                    as_of_t1,
                    &finstack_config,
                )
            },
        )
        .expect("public attribution should preserve the failed-factor diagnostics");

        assert!(attribution.result_invalid);
        assert!(attribution.carry_detail.is_none());
        assert!(
            attribution
                .meta
                .notes
                .iter()
                .any(|note| note.contains(expected_error))
        );
    }
}

/// The FX-exposure factor must run when either side carries an FX matrix.
#[test]
fn fx_factor_runs_when_only_t1_has_fx() {
    use finstack_quant_core::Error;
    use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};

    struct FixedFx(f64);
    impl FxProvider for FixedFx {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> Result<f64> {
            if from == to {
                Ok(1.0)
            } else if from == Currency::EUR && to == Currency::USD {
                Ok(self.0)
            } else if from == Currency::USD && to == Currency::EUR {
                Ok(1.0 / self.0)
            } else {
                Err(Error::Validation("FX rate not found".to_string()))
            }
        }
    }

    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);

    let instrument: Arc<dyn Instrument> = Arc::new(MockInstrument::new(
        "FX-T1-ONLY",
        MockPayoff::FxOptional {
            eur_notional: 1_000_000.0,
        },
    ));

    let market_t0 = MarketContext::new();
    let market_t1 = MarketContext::new().insert_fx(FxMatrix::new(Arc::new(FixedFx(1.20)), None));

    let result = compute_taylor_result(
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
        &TaylorAttributionConfig::default(),
    )
    .expect("taylor attribution should succeed");

    let fx = result
        .factors
        .iter()
        .find(|f| f.factor_name == "Fx")
        .expect("Fx factor must run when only T1 carries an FX matrix");
    assert!(
        (fx.explained_pnl - 200_000.0).abs() < 1e-6,
        "Fx factor must capture the T1 FX-driven P&L, got {}",
        fx.explained_pnl
    );
}

#[test]
fn taylor_buckets_spot_move_into_market_scalars_pnl() {
    use finstack_quant_core::market_data::scalars::MarketScalar;

    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let instrument: Arc<dyn Instrument> = Arc::new(MockInstrument::new(
        "SPOT-001",
        MockPayoff::Spot {
            id: "AAPL",
            scale: 100.0,
        },
    ));
    let market_t0 = MarketContext::new().insert_price(
        "AAPL",
        MarketScalar::price(Money::from((180_i64, Currency::USD))),
    );
    let market_t1 = MarketContext::new().insert_price(
        "AAPL",
        MarketScalar::price(Money::from((185_i64, Currency::USD))),
    );

    let attribution = crate::attribute_pnl(
        &AttributionMethod::Taylor(TaylorAttributionConfig::default()),
        &crate::AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..crate::AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
    )
    .expect("taylor attribution should succeed");

    // 100 shares × $5 spot move = $500, previously dumped into residual.
    assert!(
        (attribution.market_scalars_pnl.amount() - 500.0).abs() < 1e-6,
        "market_scalars_pnl = {}",
        attribution.market_scalars_pnl
    );
    assert!(
        attribution.residual.amount().abs() < 1e-6,
        "residual should shrink once the spot move is attributed, got {}",
        attribution.residual
    );
}

#[test]
fn taylor_buckets_inflation_curve_move_into_inflation_pnl() {
    use finstack_quant_core::market_data::term_structures::InflationCurve;

    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let infl = |cpi_5y| {
        InflationCurve::builder("US-CPI")
            .base_date(as_of_t0)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
            .knots([(0.0, 300.0), (5.0, cpi_5y)])
            .build()
            .expect("inflation curve")
    };
    let instrument: Arc<dyn Instrument> = Arc::new(MockInstrument::new(
        "INFL-001",
        MockPayoff::InflationCpi {
            curve: "US-CPI",
            tenor: 5.0,
            scale: 1_000.0,
        },
    ));
    let market_t0 = MarketContext::new()
        .try_insert(infl(325.0))
        .expect("insert market curve");
    let market_t1 = MarketContext::new()
        .try_insert(infl(350.0))
        .expect("insert market curve");

    let attribution = crate::attribute_pnl(
        &AttributionMethod::Taylor(TaylorAttributionConfig::default()),
        &crate::AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..crate::AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
    )
    .expect("taylor attribution should succeed");

    assert!(
        (attribution.inflation_curves_pnl.amount() - 25_000.0).abs() < 1e-6,
        "inflation_curves_pnl = {}",
        attribution.inflation_curves_pnl
    );
    assert!(
        attribution.residual.amount().abs() < 1e-6,
        "residual should shrink once the inflation move is attributed, got {}",
        attribution.residual
    );
}

#[test]
fn taylor_buckets_correlation_move_into_correlations_pnl() {
    use finstack_quant_core::market_data::term_structures::BaseCorrelationCurve;

    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let corr = |mid| {
        BaseCorrelationCurve::builder("CDX")
            .knots(vec![(3.0, 0.20), (7.0, mid), (10.0, 0.60)])
            .build()
            .expect("base correlation curve")
    };
    let instrument: Arc<dyn Instrument> = Arc::new(MockInstrument::new(
        "CORR-001",
        MockPayoff::Correlation {
            curve: "CDX",
            detachment: 7.0,
            scale: 1_000_000.0,
        },
    ));
    let market_t0 = MarketContext::new()
        .try_insert(corr(0.40))
        .expect("insert market curve");
    let market_t1 = MarketContext::new()
        .try_insert(corr(0.50))
        .expect("insert market curve");

    let attribution = crate::attribute_pnl(
        &AttributionMethod::Taylor(TaylorAttributionConfig::default()),
        &crate::AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..crate::AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
    )
    .expect("taylor attribution should succeed");

    assert!(
        (attribution.correlations_pnl.amount() - 100_000.0).abs() < 1e-6,
        "correlations_pnl = {}",
        attribution.correlations_pnl
    );
    assert!(
        attribution.residual.amount().abs() < 1e-6,
        "residual should shrink once the correlation move is attributed, got {}",
        attribution.residual
    );
}

#[test]
fn taylor_buckets_model_param_move_into_model_params_pnl() {
    use finstack_quant_cashflows::builder::{
        DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec,
    };

    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let mut instrument =
        MockInstrument::new("MODEL-001", MockPayoff::ModelCpr { scale: 1_000_000.0 });
    instrument.model_cpr = Some(0.08);
    let instrument: Arc<dyn Instrument> = Arc::new(instrument);
    let params_t0 = ModelParamsSnapshot::StructuredCredit {
        prepayment_spec: PrepaymentModelSpec::constant_cpr(0.06),
        default_spec: DefaultModelSpec::constant_cdr(0.0),
        recovery_spec: RecoveryModelSpec::with_lag(0.0, 0),
    };

    let attribution = crate::attribute_pnl(
        &AttributionMethod::Taylor(TaylorAttributionConfig::default()),
        &crate::AttributionRequest {
            model_params_t0: Some(&params_t0),
            ..crate::AttributionRequest::new(
                &instrument,
                &MarketContext::new(),
                &MarketContext::new(),
                as_of_t0,
                as_of_t1,
                &finstack_quant_core::config::FinstackConfig::default(),
            )
        },
    )
    .expect("taylor attribution should succeed");

    // scale × (0.08 − 0.06) = 20_000, previously residual.
    assert!(
        (attribution.model_params_pnl.amount() - 20_000.0).abs() < 1e-6,
        "model_params_pnl = {}",
        attribution.model_params_pnl
    );
    assert!(
        attribution.residual.amount().abs() < 1e-6,
        "residual should shrink once the model-param move is attributed, got {}",
        attribution.residual
    );
}
