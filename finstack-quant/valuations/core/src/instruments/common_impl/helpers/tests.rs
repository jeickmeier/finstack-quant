use super::mc::MAX_MC_PATHS;
use super::*;
use crate::instruments::{Instrument, PricingOptions};
use crate::metrics::MetricId;
use crate::pricer::InstrumentType;
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_cashflows::{ScheduleBuildOpts, schedule_from_dated_flows};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    InflationExpectedRelease, InflationIndex, InflationLag, InflationObservation,
    SeriesInterpolation,
};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, InflationCurve};
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{Attributes, CurveId};
use std::any::Any;
use std::sync::Arc;
use time::Duration;
use time::macros::date;

#[test]
fn zero_rate_validates_inputs_before_expiry_return() {
    for df in [f64::NAN, 0.0] {
        assert!(zero_rate_from_df(df, 0.0, "test").is_err());
    }
    for time in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(zero_rate_from_df(1.0, time, "test").is_err());
    }
    assert_eq!(zero_rate_from_df(1.0, -1.0, "test").unwrap(), 0.0);
}

#[test]
fn resolve_mc_paths_uses_default_when_override_missing() {
    let n = resolve_mc_paths(None, 50_000).expect("default returned");
    assert_eq!(n, 50_000);
}

#[test]
fn resolve_mc_paths_rejects_zero_override() {
    let err = resolve_mc_paths(Some(0), 50_000).expect_err("zero override must error");
    assert!(err.to_string().contains("positive"), "{err}");
}

#[test]
fn resolve_mc_paths_honours_positive_override() {
    let n = resolve_mc_paths(Some(123_456), 50_000).expect("override honoured");
    assert_eq!(n, 123_456);
}

#[test]
fn resolve_mc_paths_rejects_override_above_cap() {
    let too_many = MAX_MC_PATHS + 1;
    let err = resolve_mc_paths(Some(too_many), 50_000).expect_err("override above cap must error");
    let msg = err.to_string();
    assert!(msg.contains("MAX_MC_PATHS"));
    assert!(msg.contains(&too_many.to_string()));
}

#[test]
fn resolve_mc_paths_accepts_override_at_cap() {
    let at_cap = MAX_MC_PATHS;
    let n = resolve_mc_paths(Some(at_cap), 50_000).expect("exact cap is allowed");
    assert_eq!(n, at_cap);
}

#[test]
fn resolve_mc_paths_rejects_default_above_cap() {
    let too_many = MAX_MC_PATHS + 1;
    let err = resolve_mc_paths(None, too_many).expect_err("default above cap must error");
    assert!(err.to_string().contains("MAX_MC_PATHS"));
}

#[derive(Clone)]
struct StubInstrument {
    id: String,
    attrs: Attributes,
    instrument_pricing_overrides: crate::instruments::pricing_overrides::InstrumentPricingOverrides,
    metric_pricing_overrides: crate::instruments::pricing_overrides::MetricPricingOverrides,
    scenario_pricing_overrides: crate::instruments::pricing_overrides::ScenarioPricingOverrides,
}

crate::impl_empty_cashflow_provider!(
    StubInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl StubInstrument {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            attrs: Attributes::default(),
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
        }
    }
}

struct SingleFlowProvider;

impl finstack_quant_cashflows::CashflowScheduleSource for SingleFlowProvider {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(Some(Money::from((100_i64, Currency::USD))))
    }

    fn raw_cashflow_schedule(
        &self,
        _curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        schedule_from_dated_flows(
            vec![
                (as_of, Money::from((10_000_i64, Currency::USD))),
                (
                    as_of + Duration::days(30),
                    Money::from((100_i64, Currency::USD)),
                ),
            ],
            finstack_quant_core::cashflow::CFKind::Fixed,
            DayCount::Act365F,
            ScheduleBuildOpts {
                notional_hint: self.notional().expect("valid notional"),
                ..Default::default()
            },
        )
    }
}

impl Instrument for StubInstrument {
    fn includes_valuation_date_cashflows(&self) -> bool {
        true
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::MarketDependencies> {
        Ok(crate::instruments::MarketDependencies::new())
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> InstrumentType {
        InstrumentType::Bond
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn base_value(
        &self,
        _market: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        Ok(Money::new(123.45, Currency::USD).expect("valid money fixture"))
    }

    fn attributes(&self) -> &Attributes {
        &self.attrs
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        &mut self.attrs
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    crate::impl_focused_pricing_overrides!();

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
        options: crate::instruments::PricingOptions,
    ) -> std::result::Result<crate::results::ValuationResult, crate::pricer::PricingError> {
        let base = self.value(market, as_of)?;
        Ok(build_with_metrics_dyn(
            Arc::from(self.clone_box()),
            Arc::new(market.clone()),
            as_of,
            base,
            metrics,
            MetricBuildOptions {
                pricing: options,
                ..MetricBuildOptions::default()
            },
        )?)
    }
}

#[test]
fn stamped_result_uses_provided_config() -> finstack_quant_core::Result<()> {
    let instrument = Arc::new(StubInstrument::new("STUB"));
    let market = Arc::new(MarketContext::new());
    let as_of = date!(2024 - 01 - 01);
    let base_value = Money::from((10_i64, Currency::USD));

    let mut cfg = FinstackConfig::default();
    cfg.rounding
        .output_scale
        .set_scale(Currency::USD, 4)
        .expect("valid decimal scale");
    let cfg = Arc::new(cfg);

    let result = build_with_metrics_dyn(
        instrument,
        market,
        as_of,
        base_value,
        &[],
        MetricBuildOptions {
            pricing: PricingOptions {
                config: Some(cfg),
                ..PricingOptions::default()
            },
            ..MetricBuildOptions::default()
        },
    )?;

    let usd_scale = result.meta.rounding.output_scale(Currency::USD);
    assert_eq!(usd_scale, 4, "meta should reflect provided config");
    Ok(())
}

#[test]
fn metric_result_preserves_scenario_adjusted_base_value() -> finstack_quant_core::Result<()> {
    let mut instrument = StubInstrument::new("STUB-SHOCK");
    instrument.scenario_pricing_overrides = instrument
        .scenario_pricing_overrides
        .with_scenario_price_shock_decimal(-0.10);

    let market = MarketContext::new();
    let result = instrument.price_with_metrics(
        &market,
        date!(2024 - 01 - 01),
        &[],
        crate::instruments::PricingOptions::default(),
    )?;

    assert!((result.value.amount() - 111.105).abs() < 1e-9);
    Ok(())
}

#[test]
fn instrument_value_default_applies_scenario_price_shock_once() -> finstack_quant_core::Result<()> {
    let mut instrument = StubInstrument::new("STUB-VALUE");
    instrument.scenario_pricing_overrides = instrument
        .scenario_pricing_overrides
        .with_scenario_price_shock_decimal(-0.10);

    let market = MarketContext::new();
    let as_of = date!(2024 - 01 - 01);

    let direct = instrument.value(&market, as_of)?;
    assert!((direct.amount() - 111.105).abs() < 1e-9);

    let via_metrics = instrument
        .price_with_metrics(
            &market,
            as_of,
            &[],
            crate::instruments::PricingOptions::default(),
        )?
        .value;
    assert!((direct.amount() - via_metrics.amount()).abs() < 1e-9);
    Ok(())
}

#[test]
fn instrument_base_value_is_unshocked() -> finstack_quant_core::Result<()> {
    let mut instrument = StubInstrument::new("STUB-BASE");
    instrument.scenario_pricing_overrides = instrument
        .scenario_pricing_overrides
        .with_scenario_price_shock_decimal(-0.10);

    let market = MarketContext::new();
    let base = instrument.base_value(&market, date!(2024 - 01 - 01))?;
    assert!((base.amount() - 123.45).abs() < 1e-9);
    Ok(())
}

#[test]
fn configured_dividend_yield_must_exist() {
    let market = MarketContext::new();
    let err = resolve_optional_dividend_yield(
        &market,
        Some(&finstack_quant_core::types::PriceId::new("DIV")),
    )
    .err()
    .map(|err| err.to_string());
    assert!(
        err.as_deref()
            .is_some_and(|msg| msg.contains("Failed to fetch dividend yield"))
    );
}

#[test]
fn configured_dividend_yield_must_be_unitless() {
    let market = MarketContext::new().insert_price(
        "DIV",
        finstack_quant_core::market_data::scalars::MarketScalar::price(Money::from((
            1_i64,
            Currency::USD,
        ))),
    );
    let err = resolve_optional_dividend_yield(
        &market,
        Some(&finstack_quant_core::types::PriceId::new("DIV")),
    )
    .err()
    .map(|err| err.to_string());
    assert!(
        err.as_deref()
            .is_some_and(|msg| msg.contains("should be a unitless scalar"))
    );
}

#[test]
fn empty_schedule_without_known_currency_remains_an_error() {
    let empty = StubInstrument::new("EMPTY-CURRENCY");
    let market = MarketContext::new();
    let curve_id = CurveId::new("DISC");
    let as_of = date!(2025 - 01 - 16);
    assert!(matches!(
        schedule_pv(&empty, &market, as_of, &curve_id, false),
        Err(finstack_quant_core::Error::Input(
            finstack_quant_core::InputError::TooFewPoints
        ))
    ));
}

#[test]
fn schedule_pv_excludes_as_of_cash() -> finstack_quant_core::Result<()> {
    let as_of = date!(2024 - 01 - 01);
    let market = MarketContext::new()
        .try_insert(
            DiscountCurve::builder("DISC")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 0.95)])
                .interp(InterpStyle::Linear)
                .day_count(DayCount::Act365F)
                .extrapolation(ExtrapolationPolicy::Extend)
                .build()?,
        )
        .expect("insert market curve");

    let pv = schedule_pv(
        &SingleFlowProvider,
        &market,
        as_of,
        &CurveId::new("DISC"),
        false,
    )?;

    assert!(pv.amount() > 0.0);
    assert!(pv.amount() < 100.0);
    assert_eq!(pv.currency(), Currency::USD);
    Ok(())
}

fn inflation_market(
    cutoff: Date,
    observations: Vec<InflationObservation>,
    releases: Vec<InflationExpectedRelease>,
) -> MarketContext {
    let index = InflationIndex::new("CPI", observations, Currency::USD, cutoff)
        .unwrap()
        .with_expected_releases(releases)
        .unwrap();
    MarketContext::new()
        .insert_inflation_index(index)
        .try_insert(
            InflationCurve::builder("CPI")
                .base_date(date!(2024 - 01 - 01))
                .day_count(DayCount::Act365F)
                .interp(InterpStyle::Linear)
                .extrapolation(ExtrapolationPolicy::Flat)
                .knots([(0.0, 306.0), (5.0, 306.0)])
                .build()
                .unwrap(),
        )
        .expect("insert market curve")
}

#[test]
fn observations_win_and_outstanding_releases_admit_projection() {
    let cutoff = date!(2024 - 02 - 20);
    let january = date!(2024 - 01 - 01);
    let february = date!(2024 - 02 - 01);
    let market = inflation_market(
        cutoff,
        vec![InflationObservation {
            reference_month: january,
            value: 300.0,
            available_on: date!(2024 - 02 - 12),
        }],
        vec![
            InflationExpectedRelease {
                reference_month: january,
                expected_release: date!(2024 - 02 - 24),
                known_on: january,
            },
            InflationExpectedRelease {
                reference_month: february,
                expected_release: date!(2024 - 03 - 12),
                known_on: january,
            },
        ],
    );
    let result = resolve_reference_inflation(
        &market,
        "CPI",
        date!(2024 - 04 - 15),
        cutoff,
        InflationLag::months(3),
        SeriesInterpolation::Linear,
    )
    .unwrap();
    assert!((result.value - 302.8).abs() < 1e-12);
    assert!(!result.is_known);
    assert_eq!(result.publication_date, Some(date!(2024 - 03 - 12)));
    assert!(
        resolve_reference_inflation(
            &market,
            "CPI",
            date!(2024 - 04 - 15),
            date!(2024 - 03 - 12),
            InflationLag::months(3),
            SeriesInterpolation::Linear,
        )
        .is_err()
    );
}

#[test]
fn missing_or_overdue_release_facts_never_project() {
    let cutoff = date!(2024 - 03 - 12);
    for releases in [
        vec![],
        vec![InflationExpectedRelease {
            reference_month: date!(2024 - 02 - 01),
            expected_release: cutoff,
            known_on: date!(2024 - 01 - 01),
        }],
    ] {
        let market = inflation_market(cutoff, vec![], releases);
        assert!(
            resolve_reference_inflation(
                &market,
                "CPI",
                date!(2024 - 02 - 01),
                cutoff,
                InflationLag::None,
                SeriesInterpolation::Step,
            )
            .is_err()
        );
    }
}

#[test]
fn monthly_geometry_and_unused_anchors_do_not_depend_on_metadata() {
    let cutoff = date!(2024 - 03 - 01);
    let january = date!(2024 - 01 - 01);
    let observations = vec![InflationObservation {
        reference_month: january,
        value: 300.0,
        available_on: date!(2024 - 02 - 12),
    }];
    let market = inflation_market(cutoff, observations, vec![]);
    for (lag, contract_date) in [
        (InflationLag::None, january),
        (InflationLag::months(0), january),
        (InflationLag::days(0), january),
        (InflationLag::days(10), date!(2024 - 01 - 11)),
        (InflationLag::months(3), date!(2024 - 04 - 01)),
    ] {
        let result = resolve_reference_inflation(
            &market,
            "CPI",
            contract_date,
            cutoff,
            lag,
            SeriesInterpolation::Linear,
        )
        .unwrap();
        assert_eq!(result.value, 300.0);
        assert!(result.is_known);
    }
    let step = resolve_reference_inflation(
        &market,
        "CPI",
        date!(2024 - 01 - 15),
        cutoff,
        InflationLag::None,
        SeriesInterpolation::Step,
    )
    .unwrap();
    assert_eq!(step.value, 300.0);
    assert!(
        resolve_reference_inflation(
            &market,
            "CPI",
            date!(2024 - 01 - 15),
            cutoff,
            InflationLag::None,
            SeriesInterpolation::Linear,
        )
        .is_err()
    );
    assert!(
        resolve_reference_inflation(
            &market,
            "CPI",
            date!(2023 - 12 - 15),
            cutoff,
            InflationLag::None,
            SeriesInterpolation::Step,
        )
        .is_err()
    );
}

#[test]
fn month_lag_uses_contract_month_weight_and_preserves_extreme_levels() {
    let cutoff = date!(2024 - 03 - 01);
    for (left, right, expected) in [
        (285.0, 314.0, 313.0),
        (f64::from_bits(1), f64::from_bits(1), f64::from_bits(1)),
        (f64::MAX, f64::MAX, f64::MAX),
    ] {
        let market = inflation_market(
            cutoff,
            vec![
                InflationObservation {
                    reference_month: date!(2023 - 11 - 01),
                    value: left,
                    available_on: date!(2023 - 12 - 12),
                },
                InflationObservation {
                    reference_month: date!(2023 - 12 - 01),
                    value: right,
                    available_on: date!(2024 - 01 - 12),
                },
            ],
            vec![],
        );
        let result = resolve_reference_inflation(
            &market,
            "CPI",
            date!(2024 - 02 - 29),
            cutoff,
            InflationLag::months(3),
            SeriesInterpolation::Linear,
        )
        .unwrap();
        assert_eq!(result.value, expected);
        assert!(result.is_known);
    }
}

#[test]
fn date_offset_rejects_inflation_lag_underflow() {
    for lag in [InflationLag::months(1), InflationLag::days(1)] {
        assert!(apply_inflation_lag(Date::MIN, lag).is_err());
    }
    assert_eq!(
        apply_inflation_lag(Date::MIN, InflationLag::None).expect("no date shift"),
        Date::MIN
    );
}
