#[allow(dead_code, unused_imports)]
mod test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/attribution_test_utils.rs"
    ));
}

use finstack_quant_valuations::instruments::RateOptionType;

use super::shifts::*;
use super::*;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::diff::{
    measure_credit_curve_shift, measure_inflation_curve_shift, measure_inflation_index_shift,
    TenorSamplingMethod,
};
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::{VolQuoteType, VolSurface};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::results::ValuationResult;
use indexmap::IndexMap;
use std::sync::{Arc, OnceLock};
use test_utils::TestInstrument;
use time::macros::date;

#[derive(Clone)]
struct SpotVolTestInstrument {
    id: String,
    value: Money,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    SpotVolTestInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl SpotVolTestInstrument {
    fn new(id: &str, value: Money) -> Self {
        Self {
            id: id.to_string(),
            value,
        }
    }
}

impl Instrument for SpotVolTestInstrument {
    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> finstack_quant_valuations::pricer::InstrumentType {
        finstack_quant_valuations::pricer::InstrumentType::EquityOption
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn attributes(&self) -> &finstack_quant_valuations::instruments::Attributes {
        static ATTRS: OnceLock<finstack_quant_valuations::instruments::Attributes> =
            OnceLock::new();
        ATTRS.get_or_init(finstack_quant_valuations::instruments::Attributes::default)
    }

    fn attributes_mut(&mut self) -> &mut finstack_quant_valuations::instruments::Attributes {
        unreachable!("SpotVolTestInstrument::attributes_mut should not be called")
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations::instruments::MarketDependencies::new();
        deps.add_market_scalar_id("TEST-SPOT");
        deps.add_volatility_dependency(
            finstack_quant_valuations::instruments::VolatilityDependency::new(
                finstack_quant_core::types::CurveId::new("TEST-VOL"),
                Some(finstack_quant_core::types::PriceId::new("TEST-SPOT")),
                None,
            ),
        );
        Ok(deps)
    }

    fn base_value(&self, _market: &MarketContext, _as_of: Date) -> Result<Money> {
        Ok(self.value)
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        _metrics: &[MetricId],
        _options: finstack_quant_valuations::instruments::PricingOptions,
    ) -> finstack_quant_valuations::Result<ValuationResult> {
        Ok(ValuationResult::stamped(
            self.id(),
            as_of,
            self.value(market, as_of)?,
        ))
    }
}

#[test]
fn test_metrics_based_carry_matches_theta() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
        "TEST-THETA",
        Money::from((1_000_i64, Currency::USD)),
    ));

    let mut measures_t0 = IndexMap::new();
    measures_t0.insert(MetricId::Theta, -5.0);

    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-THETA",
        as_of_t0,
        Money::from((1_000_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-THETA",
        as_of_t1,
        Money::from((995_i64, Currency::USD)),
        meta,
    );

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &MarketContext::new(),
        &MarketContext::new(),
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("metrics-based attribution should succeed");

    assert!((attribution.carry.amount() + 5.0).abs() < 1e-9);
    assert!((attribution.total_pnl.amount() + 5.0).abs() < 1e-9);
    assert!(attribution.residual_within_tolerance(0.01, 0.01));
}

/// A multi-day window without `ThetaPeriodDays` must fail closed: linear
/// coupon extrapolation is not allowed.
#[test]
fn metrics_based_carry_without_horizon_stamp_rejects_multi_day_window() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 20); // 5-day window
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
        "TEST-CARRY-NO-HORIZON",
        Money::from((1000_i64, Currency::USD)),
    ));

    let mut measures_t0 = IndexMap::new();
    measures_t0.insert(MetricId::CarryTotal, -5.0);

    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-CARRY-NO-HORIZON",
        as_of_t0,
        Money::from((1000_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-CARRY-NO-HORIZON",
        as_of_t1,
        Money::from((975_i64, Currency::USD)),
        meta,
    );

    let err = attribute_pnl_metrics_based(
        &instrument,
        &MarketContext::new(),
        &MarketContext::new(),
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect_err("multi-day carry without theta_period_days must fail");
    assert!(
        err.to_string().contains("theta_period_days"),
        "error must require the horizon stamp, got: {err}"
    );

    // A 1-day window scales by 1 — no distortion, no note.
    let meta2 = finstack_quant_core::config::results_meta(&FinstackConfig::default());
    let mut measures_1d = IndexMap::new();
    measures_1d.insert(MetricId::CarryTotal, -5.0);
    measures_1d.insert(MetricId::FundingCost, 0.0);
    let val_t0_1d = ValuationResult::stamped_with_meta(
        "TEST-CARRY-NO-HORIZON",
        as_of_t0,
        Money::from((1000_i64, Currency::USD)),
        meta2.clone(),
    )
    .with_measures(measures_1d);
    let val_t1_1d = ValuationResult::stamped_with_meta(
        "TEST-CARRY-NO-HORIZON",
        date!(2025 - 01 - 16),
        Money::from((995_i64, Currency::USD)),
        meta2,
    );
    let attribution_1d = attribute_pnl_metrics_based(
        &instrument,
        &MarketContext::new(),
        &MarketContext::new(),
        &val_t0_1d,
        &val_t1_1d,
        as_of_t0,
        date!(2025 - 01 - 16),
    )
    .expect("metrics-based attribution should succeed");
    assert!((attribution_1d.carry.amount() + 5.0).abs() < 1e-9);
    assert!(
        !attribution_1d
            .meta
            .notes
            .iter()
            .any(|n| n.contains("assumed 1-day producer horizon")),
        "a 1-day window has no scaling distortion and must not be noted"
    );
}

#[test]
fn metrics_based_carry_matching_horizon_stamp_uses_metrics_as_is() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 20); // 5-day window
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
        "TEST-CARRY-MATCHED-HORIZON",
        Money::from((1000_i64, Currency::USD)),
    ));

    let mut measures_t0 = IndexMap::new();
    measures_t0.insert(MetricId::CarryTotal, -12.0);
    measures_t0.insert(MetricId::CouponIncome, 8.0);
    measures_t0.insert(MetricId::PullToPar, -15.0);
    measures_t0.insert(MetricId::RollDown, -5.0);
    measures_t0.insert(MetricId::FundingCost, 0.0);
    measures_t0.insert(MetricId::ThetaPeriodDays, 5.0);

    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-CARRY-MATCHED-HORIZON",
        as_of_t0,
        Money::from((1000_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-CARRY-MATCHED-HORIZON",
        as_of_t1,
        Money::from((988_i64, Currency::USD)),
        meta,
    );

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &MarketContext::new(),
        &MarketContext::new(),
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("matched-horizon carry should succeed");

    assert!((attribution.carry.amount() + 12.0).abs() < 1e-9);
    let detail = attribution.carry_detail.expect("carry detail");
    assert_eq!(detail.coupon_income.expect("coupon").total.amount(), 8.0);
}

#[test]
fn production_carry_adds_back_funding_to_match_gross_total_pnl() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 20); // 5-day window
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
        "TEST-CARRY-MATCHED-HORIZON",
        Money::from((1000_i64, Currency::USD)),
    ));

    let mut measures_t0 = IndexMap::new();
    measures_t0.insert(MetricId::CarryTotal, -15.0);
    measures_t0.insert(MetricId::CouponIncome, 8.0);
    measures_t0.insert(MetricId::PullToPar, -15.0);
    measures_t0.insert(MetricId::RollDown, -5.0);
    measures_t0.insert(MetricId::FundingCost, 3.0);
    measures_t0.insert(MetricId::ThetaPeriodDays, 5.0);

    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-CARRY-MATCHED-HORIZON",
        as_of_t0,
        Money::from((1000_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-CARRY-MATCHED-HORIZON",
        as_of_t1,
        Money::from((988_i64, Currency::USD)),
        meta,
    );

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &MarketContext::new(),
        &MarketContext::new(),
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("matched-horizon carry should succeed");

    assert!((attribution.carry.amount() + 12.0).abs() < 1e-9);
    assert!(attribution.residual.amount().abs() < 1e-9);
    let detail = attribution.carry_detail.expect("carry detail");
    assert_eq!(detail.funding_cost.unwrap().amount(), 3.);
    assert_eq!(detail.coupon_income.expect("coupon").total.amount(), 8.0);
}

#[test]
fn metrics_based_carry_mismatched_horizon_rejects_even_with_coupon() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 20); // 5-day window, 1-day producer
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
        "TEST-CARRY-MISMATCHED-HORIZON",
        Money::from((1000_i64, Currency::USD)),
    ));

    let mut measures_t0 = IndexMap::new();
    measures_t0.insert(MetricId::CarryTotal, -4.5);
    measures_t0.insert(MetricId::CouponIncome, 13.7);
    measures_t0.insert(MetricId::PullToPar, -8.2);
    measures_t0.insert(MetricId::RollDown, -10.0);
    measures_t0.insert(MetricId::FundingCost, 0.0);
    measures_t0.insert(MetricId::ThetaPeriodDays, 1.0);

    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-CARRY-MISMATCHED-HORIZON",
        as_of_t0,
        Money::from((1000_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-CARRY-MISMATCHED-HORIZON",
        as_of_t1,
        Money::from((975_i64, Currency::USD)),
        meta,
    );

    let error = attribute_pnl_metrics_based(
        &instrument,
        &MarketContext::new(),
        &MarketContext::new(),
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect_err("coupon income does not make mismatched-horizon carry scalable");

    assert!(error.to_string().contains("matching theta_period_days"));
}

#[test]
fn metrics_based_missing_carry_metric_adds_note() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
        "TEST-MISSING-CARRY",
        Money::from((1000_i64, Currency::USD)),
    ));
    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-MISSING-CARRY",
        as_of_t0,
        Money::from((1000_i64, Currency::USD)),
        meta.clone(),
    );
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-MISSING-CARRY",
        as_of_t1,
        Money::from((1000_i64, Currency::USD)),
        meta,
    );

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &MarketContext::new(),
        &MarketContext::new(),
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("metrics-based attribution should succeed");

    assert_eq!(attribution.carry.amount(), 0.0);
    assert!(
        attribution
            .meta
            .notes
            .iter()
            .any(|note| note.contains("neither CarryTotal nor Theta")),
        "missing carry inputs should be visible in notes: {:?}",
        attribution.meta.notes
    );
}

#[test]
fn test_metrics_based_carry_decomposition_populates_detail_fields() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
        "TEST-CARRY-DECOMP",
        Money::from((100_000_i64, Currency::USD)),
    ));

    let mut measures_t0 = IndexMap::new();
    measures_t0.insert(MetricId::Theta, -5.0);
    measures_t0.insert(MetricId::CarryTotal, -4.5);
    measures_t0.insert(MetricId::CouponIncome, 13.7);
    measures_t0.insert(MetricId::PullToPar, -8.2);
    measures_t0.insert(MetricId::RollDown, -10.0);
    measures_t0.insert(MetricId::FundingCost, 0.0);

    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-CARRY-DECOMP",
        as_of_t0,
        Money::from((100_000_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-CARRY-DECOMP",
        as_of_t1,
        Money::new(99_995.5, Currency::USD).expect("valid money fixture"),
        meta,
    );

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &MarketContext::new(),
        &MarketContext::new(),
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("metrics-based attribution should succeed");

    let detail = attribution
        .carry_detail
        .expect("carry_detail should be populated");
    assert_eq!(attribution.carry.amount(), -4.5);
    assert_eq!(
        detail
            .coupon_income
            .as_ref()
            .expect("coupon income")
            .total
            .amount(),
        13.7
    );
    assert_eq!(detail.pull_to_par.expect("pull to par").amount(), -8.2);
    assert_eq!(
        detail.roll_down.as_ref().expect("roll down").total.amount(),
        -10.0
    );
    assert_eq!(detail.funding_cost.expect("funding cost").amount(), 0.0);

    // Partition check: populated sub-lines should sum to total.
    let comp = detail
        .coupon_income
        .as_ref()
        .map(|l| l.total.amount())
        .unwrap_or(0.0)
        + detail.pull_to_par.map(|m| m.amount()).unwrap_or(0.0)
        + detail
            .roll_down
            .as_ref()
            .map(|l| l.total.amount())
            .unwrap_or(0.0)
        - detail.funding_cost.map(|m| m.amount()).unwrap_or(0.0);
    assert!(
        (comp - detail.total.amount()).abs() < 1e-6,
        "carry lines should partition total: {comp} vs {}",
        detail.total.amount()
    );
}

#[test]
fn test_metrics_based_rates_bucketed_dv01() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    let instrument: Arc<dyn Instrument> = Arc::new(
        TestInstrument::new("TEST-RATES", Money::from((100_000_i64, Currency::USD)))
            .with_discount_curves(&["USD-OIS"]),
    );

    let market_t0 = MarketContext::new().insert(make_flat_curve("USD-OIS", as_of_t0, 0.02));
    let market_t1 = MarketContext::new().insert(make_flat_curve("USD-OIS", as_of_t1, 0.0201));

    let mut measures_t0 = IndexMap::new();
    measures_t0.insert(MetricId::custom("bucketed_dv01::USD-OIS"), -400.0);

    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-RATES",
        as_of_t0,
        Money::from((100_000_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-RATES",
        as_of_t1,
        Money::from((99_600_i64, Currency::USD)),
        meta,
    );

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("metrics-based attribution should succeed");

    assert!((attribution.rates_curves_pnl.amount() + 400.0).abs() < 1e-6);
    assert!(attribution.residual_within_tolerance(0.1, 1.0));
}

#[test]
fn inflation_attribution_uses_only_declared_market_dependencies() {
    use finstack_quant_core::market_data::term_structures::InflationCurve;

    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let instrument: Arc<dyn Instrument> = Arc::new(
        TestInstrument::new("TEST-INFLATION", Money::from((100000_i64, Currency::USD)))
            .with_inflation_curves(&["US-CPI"]),
    );
    let curve = |id: &str, end_cpi: f64| {
        InflationCurve::builder(id)
            .base_date(as_of_t0)
            .base_cpi(100.0)
            .knots([(0.0, 100.0), (10.0, end_cpi)])
            .build()
            .expect("inflation curve")
    };
    let market_t0 = MarketContext::new()
        .insert(curve("US-CPI", 120.0))
        .insert(curve("EU-HICP", 120.0));
    let market_t1 = MarketContext::new()
        .insert(curve("US-CPI", 120.12))
        .insert(curve("EU-HICP", 180.0));

    let mut measures = IndexMap::new();
    measures.insert(MetricId::Inflation01, 100.0);
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());
    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-INFLATION",
        as_of_t0,
        Money::from((100000_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-INFLATION",
        as_of_t1,
        Money::from((100100_i64, Currency::USD)),
        meta,
    );

    let expected_shift = measure_inflation_curve_shift("US-CPI", &market_t0, &market_t1)
        .expect("US inflation shift");
    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("inflation attribution");

    assert!((attribution.inflation_curves_pnl.amount() - 100.0 * expected_shift).abs() < 1e-9);
}

#[test]
fn inflation_attribution_supports_index_only_sources() {
    use finstack_quant_core::market_data::scalars::InflationIndex;

    let as_of_t0 = date!(2025 - 12 - 15);
    let as_of_t1 = date!(2026 - 01 - 15);
    let instrument: Arc<dyn Instrument> = Arc::new(
        TestInstrument::new(
            "TEST-INFLATION-INDEX",
            Money::from((100000_i64, Currency::USD)),
        )
        .with_inflation_curves(&["US-CPI"]),
    );
    let index = |include_new_print: bool| {
        let mut observations = vec![
            (date!(2025 - 01 - 01), 100.0),
            (date!(2025 - 12 - 01), 110.0),
        ];
        if include_new_print {
            observations.push((date!(2026 - 01 - 01), 112.0));
        }
        InflationIndex::new("US-CPI", observations, Currency::USD).expect("inflation index")
    };
    let market_t0 = MarketContext::new().insert_inflation_index("US-CPI", index(false));
    let market_t1 = MarketContext::new().insert_inflation_index("US-CPI", index(true));

    let mut measures = IndexMap::new();
    measures.insert(MetricId::Inflation01, 100.0);
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());
    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-INFLATION-INDEX",
        as_of_t0,
        Money::from((100000_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-INFLATION-INDEX",
        as_of_t1,
        Money::from((100100_i64, Currency::USD)),
        meta,
    );

    let expected_shift =
        measure_inflation_index_shift("US-CPI", &market_t0, &market_t1).expect("index shift");
    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("index-only inflation attribution");

    assert!((attribution.inflation_curves_pnl.amount() - 100.0 * expected_shift).abs() < 1e-9);
    assert!(attribution.inflation_curves_pnl.amount() > 0.0);
}

#[test]
fn test_metric_id_new_variants() {
    // Test that new MetricId variants exist and serialize correctly
    assert_eq!(MetricId::IrConvexity.as_str(), "ir_convexity");
    assert_eq!(MetricId::CsGamma.as_str(), "cs_gamma");
    assert_eq!(MetricId::InflationConvexity.as_str(), "inflation_convexity");

    // Test that they're distinct from existing metrics
    assert_ne!(MetricId::IrConvexity.as_str(), MetricId::Convexity.as_str());
    assert_ne!(MetricId::CsGamma.as_str(), MetricId::Gamma.as_str());
}

#[test]
fn test_extract_bucketed_dv01_per_curve() {
    use finstack_quant_core::types::CurveId;

    // Test with explicit per-curve keys
    let mut measures = IndexMap::new();
    measures.insert(MetricId::custom("bucketed_dv01::USD-OIS"), -100.0);
    measures.insert(MetricId::custom("bucketed_dv01::USD-SOFR"), -50.0);
    measures.insert(MetricId::custom("bucketed_dv01::EUR-OIS"), -75.0);

    let curve_ids = vec![
        CurveId::new("USD-OIS"),
        CurveId::new("USD-SOFR"),
        CurveId::new("EUR-OIS"),
    ];

    let bucketed = extract_bucketed_dv01_per_curve(&measures, &curve_ids);

    assert_eq!(bucketed.len(), 3);
    assert_eq!(bucketed.get(&CurveId::new("USD-OIS")), Some(&-100.0));
    assert_eq!(bucketed.get(&CurveId::new("USD-SOFR")), Some(&-50.0));
    assert_eq!(bucketed.get(&CurveId::new("EUR-OIS")), Some(&-75.0));
}

#[test]
fn test_extract_bucketed_dv01_single_curve() {
    use finstack_quant_core::types::CurveId;

    // Test with single curve using base key
    let mut measures = IndexMap::new();
    measures.insert(MetricId::custom("bucketed_dv01"), -250.0);

    let curve_ids = vec![CurveId::new("USD-OIS")];

    let bucketed = extract_bucketed_dv01_per_curve(&measures, &curve_ids);

    assert_eq!(bucketed.len(), 1);
    assert_eq!(bucketed.get(&CurveId::new("USD-OIS")), Some(&-250.0));
}

/// Audit Major (shifts.rs): the producer NEVER emits a `bucketed_dv01::{curve}`
/// per-curve total — it flattens per-tenor keys `bucketed_dv01::{curve}::{label}`
/// plus a scalar `bucketed_dv01` total. With only the real producer shape
/// present, the per-curve extraction must derive the total by summing the
/// per-tenor keys.
#[test]
fn test_extract_bucketed_dv01_sums_per_tenor_keys_when_direct_key_absent() {
    use finstack_quant_core::types::CurveId;

    // Real producer shape: ONLY per-tenor keys, no "bucketed_dv01::USD-OIS".
    let mut measures = IndexMap::new();
    measures.insert(MetricId::custom("bucketed_dv01::USD-OIS::5y"), -100.0);
    measures.insert(MetricId::custom("bucketed_dv01::USD-OIS::10y"), -200.0);
    measures.insert(MetricId::custom("bucketed_dv01::USD-OIS::30y"), -50.0);

    let curve_ids = vec![CurveId::new("USD-OIS")];
    let bucketed = extract_bucketed_dv01_per_curve(&measures, &curve_ids);

    assert_eq!(bucketed.len(), 1);
    assert_eq!(bucketed.get(&CurveId::new("USD-OIS")), Some(&-350.0));

    // The direct per-curve key, when present, wins over the per-tenor sum
    // (backward compatibility).
    let mut measures_direct = IndexMap::new();
    measures_direct.insert(MetricId::custom("bucketed_dv01::USD-OIS"), -999.0);
    measures_direct.insert(MetricId::custom("bucketed_dv01::USD-OIS::5y"), -100.0);
    let bucketed_direct = extract_bucketed_dv01_per_curve(&measures_direct, &curve_ids);
    assert_eq!(bucketed_direct.get(&CurveId::new("USD-OIS")), Some(&-999.0));

    // Pattern 2 (scalar `bucketed_dv01` total attributed to the single curve)
    // must fire for a curve declared as BOTH discount and projection: the
    // deduped `rates_curve_ids` has length 1, so the single-curve branch is
    // reachable (before the dedup fix the list had length 2 and it was not).
    let mut measures_scalar = IndexMap::new();
    measures_scalar.insert(MetricId::custom("bucketed_dv01"), -250.0);
    let deduped_single = vec![CurveId::new("USD-OIS")];
    let bucketed_scalar = extract_bucketed_dv01_per_curve(&measures_scalar, &deduped_single);
    assert_eq!(bucketed_scalar.get(&CurveId::new("USD-OIS")), Some(&-250.0));
}

#[test]
fn test_extract_bucketed_dv01_empty() {
    use finstack_quant_core::types::CurveId;

    // Test with no bucketed metrics
    let measures = IndexMap::new();
    let curve_ids = vec![CurveId::new("USD-OIS")];

    let bucketed = extract_bucketed_dv01_per_curve(&measures, &curve_ids);

    assert_eq!(bucketed.len(), 0);
}

#[test]
fn test_extract_bucketed_dv01_partial_coverage() {
    use finstack_quant_core::types::CurveId;

    // Test with some curves having bucketed metrics and others not
    let mut measures = IndexMap::new();
    measures.insert(MetricId::custom("bucketed_dv01::USD-OIS"), -100.0);
    // USD-SOFR is missing

    let curve_ids = vec![CurveId::new("USD-OIS"), CurveId::new("USD-SOFR")];

    let bucketed = extract_bucketed_dv01_per_curve(&measures, &curve_ids);

    assert_eq!(bucketed.len(), 1);
    assert_eq!(bucketed.get(&CurveId::new("USD-OIS")), Some(&-100.0));
    assert_eq!(bucketed.get(&CurveId::new("USD-SOFR")), None);
}

/// `Vanna` (∂²V/∂S_abs∂σ, per unit spot per vol point) must NOT be used as
/// a fallback for `CrossGammaSpotVol` in attribution because their unit
/// conventions differ by a factor of S₀ / 100.  When only `Vanna` is present (no
/// `CrossGammaSpotVol`), the Spot×Vol cross P&L must be zero (goes to
/// residual) rather than silently mis-scaled.
#[test]
fn test_vanna_alone_does_not_produce_spot_vol_cross_pnl() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    let instrument: Arc<dyn Instrument> = Arc::new(SpotVolTestInstrument::new(
        "TEST-SPOT-VOL",
        Money::from((100_i64, Currency::USD)),
    ));

    let surface_t0 = VolSurface::builder("TEST-VOL")
        .expiries(&[1.0])
        .strikes(&[100.0])
        .row(&[0.20])
        .build()
        .expect("test vol surface should build");
    let surface_t1 = VolSurface::builder("TEST-VOL")
        .expiries(&[1.0])
        .strikes(&[100.0])
        .row(&[0.21])
        .build()
        .expect("test vol surface should build");

    let market_t0 = MarketContext::new()
        .insert_price("TEST-SPOT", MarketScalar::Unitless(100.0))
        .insert_surface(surface_t0);
    let market_t1 = MarketContext::new()
        .insert_price("TEST-SPOT", MarketScalar::Unitless(110.0))
        .insert_surface(surface_t1);

    // Only Vanna is present — NO CrossGammaSpotVol.
    let mut measures_t0 = IndexMap::new();
    measures_t0.insert(MetricId::Vega, 2.0);
    measures_t0.insert(MetricId::Vanna, 3.0);

    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-SPOT-VOL",
        as_of_t0,
        Money::from((100_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-SPOT-VOL",
        as_of_t1,
        Money::from((132_i64, Currency::USD)),
        meta,
    );

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("metrics-based attribution should succeed");

    // Vol P&L: Vega × Δσ_pct_pt = 2.0 × 1.0 = 2.0
    assert!((attribution.vol_pnl.amount() - 2.0).abs() < 1e-9);
    // Spot×Vol cross P&L must be zero: Vanna is not a valid substitute for
    // CrossGammaSpotVol (wrong unit convention).
    assert!(
        attribution.cross_factor_pnl.amount().abs() < 1e-9,
        "cross_factor_pnl should be zero when only Vanna is available (not CrossGammaSpotVol); \
         got {}",
        attribution.cross_factor_pnl.amount()
    );
    // cross_factor_detail should be None (no cross terms found).
    assert!(
        attribution.cross_factor_detail.is_none(),
        "cross_factor_detail should be None when no CrossGamma metrics are present"
    );
}

#[test]
fn hw1f_cap_surface_shock_does_not_affect_explicit_parameter_pricing() {
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::market_data::bumps::{
        BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
    };
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_core::types::CurveId;
    use finstack_quant_valuations::instruments::rates::cap_floor::{CapFloor, CapFloorVolType};
    use finstack_quant_valuations::instruments::PricingOptions;
    use finstack_quant_valuations::pricer::ModelKey;

    let as_of_t0 = date!(2024 - 01 - 01);
    let as_of_t1 = date!(2024 - 01 - 02);
    let mut cap = CapFloor::new(
        "HW-SURFACE-CAP",
        RateOptionType::Cap,
        Money::from((1000000_i64, Currency::USD)),
        0.05,
        date!(2024 - 04 - 01),
        date!(2029 - 04 - 01),
        Some(Tenor::quarterly()),
        DayCount::Act365F,
        "USD-OIS",
        "USD-LIBOR-3M",
        "USD-CAP-VOL",
    )
    .expect("cap");
    cap.vol_type = CapFloorVolType::Normal;
    cap.instrument_pricing_overrides
        .model_config
        .hw1f_mean_reversion = Some(0.03);
    cap.instrument_pricing_overrides.model_config.hw1f_sigma = Some(0.01);

    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of_t0)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (10.0, (-0.05_f64 * 10.0).exp())])
        .build()
        .expect("discount");
    let forward = ForwardCurve::builder("USD-LIBOR-3M", 0.25)
        .base_date(as_of_t0)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 0.05), (10.0, 0.05)])
        .build()
        .expect("forward");
    let surface = VolSurface::builder("USD-CAP-VOL")
        .expiries(&[0.25, 1.0, 5.0, 10.0])
        .strikes(&[0.05])
        .quote_type(VolQuoteType::Normal)
        .row(&[0.010])
        .row(&[0.010])
        .row(&[0.010])
        .row(&[0.010])
        .build()
        .expect("surface");
    let market_t0 = MarketContext::new()
        .insert(discount)
        .insert(forward)
        .insert_surface(surface);
    let market_t1 = market_t0
        .bump([MarketBump::Curve {
            id: CurveId::from("USD-CAP-VOL"),
            spec: BumpSpec {
                mode: BumpMode::Multiplicative,
                units: BumpUnits::Factor,
                value: 1.10,
                bump_type: BumpType::Parallel,
            },
        }])
        .expect("vol shock");
    let options = PricingOptions::default().with_model(ModelKey::HullWhite1F);
    let metrics = [MetricId::Vega];

    let val_t0 = cap
        .price_with_metrics(&market_t0, as_of_t0, &metrics, options.clone())
        .expect("t0 price");
    let val_t1 = cap
        .price_with_metrics(&market_t1, as_of_t1, &metrics, options)
        .expect("t1 price");
    let instrument: Arc<dyn Instrument> = Arc::new(cap);

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("attribution");

    let vega = val_t0.measures.get("vega").copied().unwrap_or(0.0);
    assert!(
        vega.abs() < 1e-12,
        "explicit-parameter HW1F pricing must not sample the vol surface; got vega={vega}, measures={:?}",
        val_t0.measures
    );
    assert!(
        attribution.vol_pnl.amount().abs() < 1e-12,
        "a surface-only move must have zero HW1F vol P&L when parameters are explicit"
    );
}

/// Regression test: `CrossGammaSpotVol` (in pct-spot × vol-point units,
/// produced by `CrossFactorCalculator`) multiplied by `avg_spot_shift_pct`
/// and `avg_vol_shift_abs` must give the correct cross P&L.
///
/// Setup:
///   S₀ = 100, S₁ = 110  → avg_spot_shift_pct = 10.0 (pct-pt)
///   σ₀ = 0.20, σ₁ = 0.21 → avg_vol_shift_abs = 1.0 (vol-pt)
///   CrossGammaSpotVol = 0.005 ($ per pct-pt spot per vol-pt)
///
/// Expected cross P&L = 0.005 × 10.0 × 1.0 = 0.05
#[test]
fn test_cross_gamma_spot_vol_uses_pct_spot_move() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    let instrument: Arc<dyn Instrument> = Arc::new(SpotVolTestInstrument::new(
        "TEST-SPOT-VOL-CGAMMA",
        Money::from((100_i64, Currency::USD)),
    ));

    let surface_t0 = VolSurface::builder("TEST-VOL")
        .expiries(&[1.0])
        .strikes(&[100.0])
        .row(&[0.20])
        .build()
        .expect("test vol surface should build");
    let surface_t1 = VolSurface::builder("TEST-VOL")
        .expiries(&[1.0])
        .strikes(&[100.0])
        .row(&[0.21])
        .build()
        .expect("test vol surface should build");

    let market_t0 = MarketContext::new()
        .insert_price("TEST-SPOT", MarketScalar::Unitless(100.0))
        .insert_surface(surface_t0);
    let market_t1 = MarketContext::new()
        .insert_price("TEST-SPOT", MarketScalar::Unitless(110.0))
        .insert_surface(surface_t1);

    // CrossGammaSpotVol is explicitly present (pct-spot × vol-point units).
    // Vanna is also set to a different value to confirm it is NOT used.
    let cross_gamma_spot_vol = 0.005_f64; // $ per pct-pt spot per vol-pt
    let mut measures_t0 = IndexMap::new();
    measures_t0.insert(MetricId::Vega, 2.0);
    measures_t0.insert(MetricId::Vanna, 999.0); // must be ignored
    measures_t0.insert(MetricId::CrossGammaSpotVol, cross_gamma_spot_vol);

    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-SPOT-VOL-CGAMMA",
        as_of_t0,
        Money::from((100_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-SPOT-VOL-CGAMMA",
        as_of_t1,
        Money::new(102.07, Currency::USD).expect("valid money fixture"), // arbitrary end value
        meta,
    );

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("metrics-based attribution should succeed");

    // avg_spot_shift_pct = (110/100 - 1) × 100 = 10.0
    // avg_vol_shift_abs  = (0.21 - 0.20) × 100 = 1.0
    // expected cross P&L = 0.005 × 10.0 × 1.0 = 0.05
    let expected_cross_pnl = cross_gamma_spot_vol * 10.0 * 1.0;
    assert!(
        (attribution.cross_factor_pnl.amount() - expected_cross_pnl).abs() < 1e-9,
        "cross P&L should be {expected_cross_pnl} (pct-spot units); got {}",
        attribution.cross_factor_pnl.amount()
    );
    let detail = attribution
        .cross_factor_detail
        .expect("cross factor detail should be populated");
    let spot_vol_entry = detail
        .by_pair
        .get("Spot×Vol")
        .expect("Spot×Vol entry should be present");
    assert!(
        (spot_vol_entry.amount() - expected_cross_pnl).abs() < 1e-9,
        "Spot×Vol detail should be {expected_cross_pnl}; got {}",
        spot_vol_entry.amount()
    );
}

/// Test instrument exposing a credit-curve dependency and a vol-surface
/// dependency (the convertible shape) so the Credit×Vol cross term can be
/// exercised without a full convertible pricer.
#[derive(Clone)]
struct CreditVolTestInstrument {
    id: String,
    value: Money,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    CreditVolTestInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl CreditVolTestInstrument {
    fn new(id: &str, value: Money) -> Self {
        Self {
            id: id.to_string(),
            value,
        }
    }
}

impl Instrument for CreditVolTestInstrument {
    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> finstack_quant_valuations::pricer::InstrumentType {
        finstack_quant_valuations::pricer::InstrumentType::Convertible
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn attributes(&self) -> &finstack_quant_valuations::instruments::Attributes {
        static ATTRS: OnceLock<finstack_quant_valuations::instruments::Attributes> =
            OnceLock::new();
        ATTRS.get_or_init(finstack_quant_valuations::instruments::Attributes::default)
    }

    fn attributes_mut(&mut self) -> &mut finstack_quant_valuations::instruments::Attributes {
        unreachable!("CreditVolTestInstrument::attributes_mut should not be called")
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations::instruments::MarketDependencies::new();
        deps.add_credit_curve(finstack_quant_core::types::CurveId::new("ACME-HAZ"));
        deps.add_volatility_dependency(
            finstack_quant_valuations::instruments::VolatilityDependency::new(
                finstack_quant_core::types::CurveId::new("TEST-VOL"),
                None,
                None,
            ),
        );
        Ok(deps)
    }

    fn base_value(&self, _market: &MarketContext, _as_of: Date) -> Result<Money> {
        Ok(self.value)
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        _metrics: &[MetricId],
        _options: finstack_quant_valuations::instruments::PricingOptions,
    ) -> finstack_quant_valuations::Result<ValuationResult> {
        Ok(ValuationResult::stamped(
            self.id(),
            as_of,
            self.value(market, as_of)?,
        ))
    }
}

/// Audit fix: `CrossGammaCreditVol` (bp-credit × vol-point units) must be
/// consumed by the cross-factor block — a convertible's credit-vol
/// cross-gamma previously flowed silently into the residual.
///
/// Setup:
///   hazard 0.02 → 0.03 (par-spread move measured by
///   `measure_credit_curve_shift`, same value asserted here)
///   σ 0.20 → 0.21 → avg_vol_shift_abs = 1.0 vol-pt
///   CrossGammaCreditVol = 0.005 ($ per bp-credit per vol-pt)
#[test]
fn test_cross_gamma_credit_vol_pairs_bp_and_vol_points() {
    use finstack_quant_core::market_data::term_structures::HazardCurve;

    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    let instrument: Arc<dyn Instrument> = Arc::new(CreditVolTestInstrument::new(
        "TEST-CREDIT-VOL-CGAMMA",
        Money::from((100_i64, Currency::USD)),
    ));

    let hazard = |as_of: Date, h: f64| {
        HazardCurve::builder("ACME-HAZ")
            .base_date(as_of)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .recovery_rate(0.4)
            .knots([(0.0, h), (5.0, h)])
            .build()
            .expect("hazard curve should build")
    };
    let surface = |vol: f64| {
        VolSurface::builder("TEST-VOL")
            .expiries(&[1.0])
            .strikes(&[100.0])
            .row(&[vol])
            .build()
            .expect("test vol surface should build")
    };

    let market_t0 = MarketContext::new()
        .insert(hazard(as_of_t0, 0.02))
        .insert_surface(surface(0.20));
    let market_t1 = MarketContext::new()
        .insert(hazard(as_of_t1, 0.03))
        .insert_surface(surface(0.21));

    // The same signed par-spread move the attribution preamble measures.
    let credit_shift_bp = measure_credit_curve_shift(
        "ACME-HAZ",
        &market_t0,
        &market_t1,
        TenorSamplingMethod::Standard,
    )
    .expect("credit curve shift should be measurable");
    assert!(
        credit_shift_bp > 0.0,
        "hazard widening must produce a positive par-spread move"
    );

    let cross_gamma_credit_vol = 0.005_f64; // $ per bp-credit per vol-pt
    let mut measures_t0 = IndexMap::new();
    measures_t0.insert(MetricId::CrossGammaCreditVol, cross_gamma_credit_vol);

    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-CREDIT-VOL-CGAMMA",
        as_of_t0,
        Money::from((100_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-CREDIT-VOL-CGAMMA",
        as_of_t1,
        Money::from((101_i64, Currency::USD)),
        meta,
    );

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("metrics-based attribution should succeed");

    // avg_vol_shift_abs = (0.21 − 0.20) × 100 = 1.0 vol-pt
    let expected_cross_pnl = cross_gamma_credit_vol * credit_shift_bp * 1.0;
    let detail = attribution
        .cross_factor_detail
        .expect("cross factor detail should be populated");
    let credit_vol_entry = detail
        .by_pair
        .get("Credit×Vol")
        .expect("Credit×Vol entry should be present");
    assert!(
        (credit_vol_entry.amount() - expected_cross_pnl).abs() < 1e-9,
        "Credit×Vol detail should be {expected_cross_pnl}; got {}",
        credit_vol_entry.amount()
    );
    assert!(
        (attribution.cross_factor_pnl.amount() - expected_cross_pnl).abs() < 1e-9,
        "cross P&L total should equal the Credit×Vol term; got {}",
        attribution.cross_factor_pnl.amount()
    );
}

/// Test instrument declaring a configurable list of credit-curve dependencies.
#[derive(Clone)]
struct CreditCurvesTestInstrument {
    id: String,
    value: Money,
    credit_curves: Vec<String>,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    CreditCurvesTestInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl CreditCurvesTestInstrument {
    fn new(id: &str, value: Money, credit_curves: &[&str]) -> Self {
        Self {
            id: id.to_string(),
            value,
            credit_curves: credit_curves.iter().map(|s| (*s).to_string()).collect(),
        }
    }
}

impl Instrument for CreditCurvesTestInstrument {
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

    fn attributes(&self) -> &finstack_quant_valuations::instruments::Attributes {
        static ATTRS: OnceLock<finstack_quant_valuations::instruments::Attributes> =
            OnceLock::new();
        ATTRS.get_or_init(finstack_quant_valuations::instruments::Attributes::default)
    }

    fn attributes_mut(&mut self) -> &mut finstack_quant_valuations::instruments::Attributes {
        unreachable!("CreditCurvesTestInstrument::attributes_mut should not be called")
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations::instruments::MarketDependencies::new();
        for curve in &self.credit_curves {
            deps.add_credit_curve(finstack_quant_core::types::CurveId::new(curve.clone()));
        }
        Ok(deps)
    }

    fn base_value(&self, _market: &MarketContext, _as_of: Date) -> Result<Money> {
        Ok(self.value)
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        _metrics: &[MetricId],
        _options: finstack_quant_valuations::instruments::PricingOptions,
    ) -> finstack_quant_valuations::Result<ValuationResult> {
        Ok(ValuationResult::stamped(
            self.id(),
            as_of,
            self.value(market, as_of)?,
        ))
    }
}

/// Audit Major (credit.rs): a non-empty per-tenor `bucketed_cs01` map must NOT
/// unconditionally set `credit_has_data = true` — when every keyrate curve is
/// skipped (shift unmeasurable), the aggregate `Cs01 × avg(Δs)` fallback must
/// still run. Fixture: MISSING-HAZ carries per-tenor CS01 but is absent from
/// both markets; ACME-HAZ has no per-tenor CS01 but measurably widened, and an
/// aggregate Cs01 is present → credit P&L must use the aggregate fallback.
#[test]
fn test_credit_aggregate_fallback_when_all_keyrate_curves_unmeasurable() {
    use finstack_quant_core::market_data::term_structures::HazardCurve;

    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    let instrument: Arc<dyn Instrument> = Arc::new(CreditCurvesTestInstrument::new(
        "TEST-CREDIT-FALLBACK",
        Money::from((100_000_i64, Currency::USD)),
        &["MISSING-HAZ", "ACME-HAZ"],
    ));

    let hazard = |as_of: Date, h: f64| {
        HazardCurve::builder("ACME-HAZ")
            .base_date(as_of)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .recovery_rate(0.4)
            .knots([(0.0, h), (5.0, h)])
            .build()
            .expect("hazard curve should build")
    };
    // MISSING-HAZ is deliberately absent from both markets: its per-tenor
    // shift is unmeasurable, so the key-rate loop skips it.
    let market_t0 = MarketContext::new().insert(hazard(as_of_t0, 0.02));
    let market_t1 = MarketContext::new().insert(hazard(as_of_t1, 0.03));

    let expected_shift_bp = measure_credit_curve_shift(
        "ACME-HAZ",
        &market_t0,
        &market_t1,
        TenorSamplingMethod::Standard,
    )
    .expect("ACME-HAZ shift should be measurable");
    assert!(expected_shift_bp > 0.0);

    let aggregate_cs01 = -50.0_f64;
    let mut measures_t0 = IndexMap::new();
    measures_t0.insert(MetricId::custom("bucketed_cs01::MISSING-HAZ::5y"), -30.0);
    measures_t0.insert(MetricId::Cs01, aggregate_cs01);

    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-CREDIT-FALLBACK",
        as_of_t0,
        Money::from((100_000_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-CREDIT-FALLBACK",
        as_of_t1,
        Money::from((95_000_i64, Currency::USD)),
        meta,
    );

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("metrics-based attribution should succeed");

    let credit_pnl = attribution.credit_curves_pnl.amount();
    let expected_pnl = aggregate_cs01 * expected_shift_bp;
    assert!(
        credit_pnl != 0.0,
        "aggregate Cs01 fallback must run when every keyrate curve was skipped; got 0"
    );
    assert!(
        (credit_pnl - expected_pnl).abs() < 1e-9,
        "credit P&L must be Cs01 × avg shift = {expected_pnl}; got {credit_pnl}"
    );
    // Skipped keyrate curves must be visible in the notes.
    assert!(
        attribution
            .meta
            .notes
            .iter()
            .any(|n| n.contains("MISSING-HAZ")),
        "skipped keyrate credit curve must be noted; notes: {:?}",
        attribution.meta.notes
    );
}

/// Test instrument declaring TWO spot (market-scalar) dependencies, in
/// declaration order FLAT-SPOT then REAL-SPOT.
#[derive(Clone)]
struct MultiSpotTestInstrument {
    id: String,
    value: Money,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    MultiSpotTestInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl MultiSpotTestInstrument {
    fn new(id: &str, value: Money) -> Self {
        Self {
            id: id.to_string(),
            value,
        }
    }
}

impl Instrument for MultiSpotTestInstrument {
    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> finstack_quant_valuations::pricer::InstrumentType {
        finstack_quant_valuations::pricer::InstrumentType::EquityOption
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn attributes(&self) -> &finstack_quant_valuations::instruments::Attributes {
        static ATTRS: OnceLock<finstack_quant_valuations::instruments::Attributes> =
            OnceLock::new();
        ATTRS.get_or_init(finstack_quant_valuations::instruments::Attributes::default)
    }

    fn attributes_mut(&mut self) -> &mut finstack_quant_valuations::instruments::Attributes {
        unreachable!("MultiSpotTestInstrument::attributes_mut should not be called")
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations::instruments::MarketDependencies::new();
        deps.add_market_scalar_id("FLAT-SPOT");
        deps.add_market_scalar_id("REAL-SPOT");
        Ok(deps)
    }

    fn base_value(&self, _market: &MarketContext, _as_of: Date) -> Result<Money> {
        Ok(self.value)
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        _metrics: &[MetricId],
        _options: finstack_quant_valuations::instruments::PricingOptions,
    ) -> finstack_quant_valuations::Result<ValuationResult> {
        Ok(ValuationResult::stamped(
            self.id(),
            as_of,
            self.value(market, as_of)?,
        ))
    }
}

/// Aggregate Delta and cross-Gamma use the first declared scalar, matching the
/// canonical sensitivity producer even when another scalar moves more.
#[test]
fn test_primary_spot_driver_matches_sensitivity_producer() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    let instrument: Arc<dyn Instrument> = Arc::new(MultiSpotTestInstrument::new(
        "TEST-MULTI-SPOT",
        Money::from((100_000_i64, Currency::USD)),
    ));

    // FLAT-SPOT is declared FIRST and is measurable but unmoved (50 → 50);
    // REAL-SPOT is a different dependency (100 → 110).
    let market_t0 = MarketContext::new()
        .insert_price("FLAT-SPOT", MarketScalar::Unitless(50.0))
        .insert_price("REAL-SPOT", MarketScalar::Unitless(100.0));
    let market_t1 = MarketContext::new()
        .insert_price("FLAT-SPOT", MarketScalar::Unitless(50.0))
        .insert_price("REAL-SPOT", MarketScalar::Unitless(110.0));

    let mut measures_t0 = IndexMap::new();
    measures_t0.insert(MetricId::Delta, 1_000.0);

    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-MULTI-SPOT",
        as_of_t0,
        Money::from((100_000_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-MULTI-SPOT",
        as_of_t1,
        Money::from((110_000_i64, Currency::USD)),
        meta,
    );

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("metrics-based attribution should succeed");

    // Delta (1000) × primary spot move (0) = 0.
    let spot_pnl = attribution.market_scalars_pnl.amount();
    assert!(
        spot_pnl.abs() < 1e-9,
        "primary spot shift must match the sensitivity producer; got {spot_pnl}"
    );
}

fn make_flat_curve(id: &str, base_date: Date, rate: f64) -> DiscountCurve {
    let mut knots = Vec::new();
    knots.push((0.0, 1.0));
    for tenor in finstack_quant_core::market_data::diff::STANDARD_TENORS {
        let discount = (-rate * tenor).exp();
        knots.push((*tenor, discount));
    }

    DiscountCurve::builder(id)
        .base_date(base_date)
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .expect("flat curve construction should succeed")
}

/// Build a discount curve whose zero rate at each standard tenor is taken
/// from `rates_by_tenor` (parallel to `STANDARD_TENORS`).
fn make_curve_from_zero_rates(id: &str, base_date: Date, rates_by_tenor: &[f64]) -> DiscountCurve {
    let mut knots = vec![(0.0, 1.0)];
    for (tenor, &rate) in finstack_quant_core::market_data::diff::STANDARD_TENORS
        .iter()
        .zip(rates_by_tenor.iter())
    {
        knots.push((*tenor, (-rate * tenor).exp()));
    }
    DiscountCurve::builder(id)
        .base_date(base_date)
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .expect("per-tenor curve construction should succeed")
}

/// Audit item #3: when per-tenor (key-rate) `bucketed_dv01` is available the
/// rates attribution must pair each tenor's DV01 with that tenor's realized
/// shift. For a steepener (short tenors down, long tenors up) the signed
/// average shift is ~0; an average-shift × parallel-DV01 product would
/// report ~0 rates P&L, but the key-rate-aware sum is materially non-zero.
#[test]
fn test_metrics_based_rates_keyrate_aware_for_steepener() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    let instrument: Arc<dyn Instrument> = Arc::new(
        TestInstrument::new("TEST-KEYRATE", Money::from((100_000_i64, Currency::USD)))
            .with_discount_curves(&["USD-OIS"]),
    );

    // T0 flat at 3%. T1 steepener: short tenors −10bp, long tenors +10bp,
    // arranged so the average over the 9 standard tenors is ~0.
    let t0_rates = [0.03_f64; 9];
    let t1_rates = [
        0.029, 0.029, 0.029, 0.0295, 0.030, 0.0305, 0.031, 0.031, 0.031,
    ];
    let market_t0 =
        MarketContext::new().insert(make_curve_from_zero_rates("USD-OIS", as_of_t0, &t0_rates));
    let market_t1 =
        MarketContext::new().insert(make_curve_from_zero_rates("USD-OIS", as_of_t1, &t1_rates));

    // Per-tenor key-rate DV01: concentrated at the LONG end (10y/30y),
    // so the steepener's long-end rise dominates the attributed P&L.
    let mut measures_t0 = IndexMap::new();
    for (label, dv01) in [
        ("3m", -1.0),
        ("6m", -1.0),
        ("1y", -2.0),
        ("2y", -3.0),
        ("3y", -4.0),
        ("5y", -6.0),
        ("7y", -8.0),
        ("10y", -40.0),
        ("30y", -120.0),
    ] {
        measures_t0.insert(
            MetricId::custom(format!("bucketed_dv01::USD-OIS::{label}")),
            dv01,
        );
    }

    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-KEYRATE",
        as_of_t0,
        Money::from((100_000_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-KEYRATE",
        as_of_t1,
        Money::from((99_000_i64, Currency::USD)),
        meta,
    );

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("metrics-based attribution should succeed");

    // Long-end DV01 (−40, −120) paired with the long-end +10bp rise gives a
    // large negative rates P&L; the short-end −10bp moves partly offset it.
    // The key-rate-aware total is materially non-zero — NOT the ~0 an
    // average-shift attribution would have produced.
    let rates_pnl = attribution.rates_curves_pnl.amount();
    // Exact pin (hand-verified): Σ dv01 × Δr
    //   = 1·10 + 1·10 + 2·10 + 3·5 + 4·0 − 6·5 − 8·10 − 40·10 − 120·10 = −1655.
    assert!(
        (rates_pnl - (-1655.0)).abs() < 1e-6,
        "key-rate-aware steepener attribution must equal the hand-verified −1655, got {rates_pnl}"
    );
    // A note must record that key-rate (per-tenor) DV01 was used.
    assert!(
        attribution
            .meta
            .notes
            .iter()
            .any(|n| n.contains("key-rate")),
        "a note must record key-rate attribution; notes: {:?}",
        attribution.meta.notes
    );
}

/// Audit B2: a curve listed as BOTH a discount and a forward/projection
/// dependency (standard single-curve OIS/SOFR IRS, FRNs) must contribute to
/// rates P&L exactly ONCE. Before the fix, `rates_curve_ids` was built as
/// discount ⧺ forward with no cross-list dedup, so the same curve was walked
/// twice by the key-rate loop and rates P&L doubled (−3310 instead of −1655
/// for the steepener fixture).
#[test]
fn test_rates_curve_in_both_discount_and_forward_lists_counts_once() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    // Same curve id declared as BOTH discount and forward dependency.
    let instrument: Arc<dyn Instrument> = Arc::new(
        TestInstrument::new(
            "TEST-SINGLE-CURVE",
            Money::from((100_000_i64, Currency::USD)),
        )
        .with_discount_curves(&["USD-OIS"])
        .with_forward_curves(&["USD-OIS"]),
    );

    // Steepener fixture identical to
    // `test_metrics_based_rates_keyrate_aware_for_steepener`.
    let t0_rates = [0.03_f64; 9];
    let t1_rates = [
        0.029, 0.029, 0.029, 0.0295, 0.030, 0.0305, 0.031, 0.031, 0.031,
    ];
    let market_t0 =
        MarketContext::new().insert(make_curve_from_zero_rates("USD-OIS", as_of_t0, &t0_rates));
    let market_t1 =
        MarketContext::new().insert(make_curve_from_zero_rates("USD-OIS", as_of_t1, &t1_rates));

    let mut measures_t0 = IndexMap::new();
    for (label, dv01) in [
        ("3m", -1.0),
        ("6m", -1.0),
        ("1y", -2.0),
        ("2y", -3.0),
        ("3y", -4.0),
        ("5y", -6.0),
        ("7y", -8.0),
        ("10y", -40.0),
        ("30y", -120.0),
    ] {
        measures_t0.insert(
            MetricId::custom(format!("bucketed_dv01::USD-OIS::{label}")),
            dv01,
        );
    }

    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-SINGLE-CURVE",
        as_of_t0,
        Money::from((100_000_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-SINGLE-CURVE",
        as_of_t1,
        Money::from((98_345_i64, Currency::USD)),
        meta,
    );

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("metrics-based attribution should succeed");

    // Single-curve value (hand-verified): Σ dv01 × Δr = −1655. A curve that is
    // both discount and projection must NOT count twice (−3310).
    let rates_pnl = attribution.rates_curves_pnl.amount();
    assert!(
        (rates_pnl - (-1655.0)).abs() < 1e-6,
        "curve in both discount and forward lists must contribute once; \
         expected -1655, got {rates_pnl}"
    );
}

/// Audit Moderate (rates.rs): the convexity block's average shift must be the
/// DV01-weighted mean `Σ|DV01_i|·Δr_i / Σ|DV01_i|`, not an unweighted mean over
/// DV01 cells. For the steepener fixture the unweighted signed mean is exactly
/// 0.0 (short −10bp cancels long +10bp), which killed the convexity term even
/// though the position's risk sits at the long end (+10bp).
#[test]
fn test_rates_convexity_uses_dv01_weighted_average_shift() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    let instrument: Arc<dyn Instrument> = Arc::new(
        TestInstrument::new(
            "TEST-KEYRATE-CONVEXITY",
            Money::from((100_000_i64, Currency::USD)),
        )
        .with_discount_curves(&["USD-OIS"]),
    );

    // Same steepener fixture as `test_metrics_based_rates_keyrate_aware_for_steepener`.
    let t0_rates = [0.03_f64; 9];
    let t1_rates = [
        0.029, 0.029, 0.029, 0.0295, 0.030, 0.0305, 0.031, 0.031, 0.031,
    ];
    let market_t0 =
        MarketContext::new().insert(make_curve_from_zero_rates("USD-OIS", as_of_t0, &t0_rates));
    let market_t1 =
        MarketContext::new().insert(make_curve_from_zero_rates("USD-OIS", as_of_t1, &t1_rates));

    let dv01s = [
        ("3m", -1.0),
        ("6m", -1.0),
        ("1y", -2.0),
        ("2y", -3.0),
        ("3y", -4.0),
        ("5y", -6.0),
        ("7y", -8.0),
        ("10y", -40.0),
        ("30y", -120.0),
    ];
    let convexity = 0.5_f64; // street convexity (per-100)
    let mut measures_t0 = IndexMap::new();
    for (label, dv01) in dv01s {
        measures_t0.insert(
            MetricId::custom(format!("bucketed_dv01::USD-OIS::{label}")),
            dv01,
        );
    }
    measures_t0.insert(MetricId::Convexity, convexity);

    let val_t0 = ValuationResult::stamped_with_meta(
        "TEST-KEYRATE-CONVEXITY",
        as_of_t0,
        Money::from((100_000_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "TEST-KEYRATE-CONVEXITY",
        as_of_t1,
        Money::from((98_345_i64, Currency::USD)),
        meta,
    );

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("metrics-based attribution should succeed");

    // First-order: Σ dv01 × Δr = −1655 (hand-verified).
    // Convexity effective shift: DV01-weighted mean
    //   Σ|DV01_i|·Δr_i / Σ|DV01_i| = 1655 / 185 ≈ +8.9459bp (NOT 0.0).
    let shifts_bp = [-10.0, -10.0, -10.0, -5.0, 0.0, 5.0, 10.0, 10.0, 10.0];
    let weighted: f64 = dv01s
        .iter()
        .zip(shifts_bp.iter())
        .map(|((_, dv01), s)| dv01.abs() * s)
        .sum();
    let total_weight: f64 = dv01s.iter().map(|(_, dv01)| dv01.abs()).sum();
    let weighted_avg_bp = weighted / total_weight;
    assert!((weighted_avg_bp - 8.9459459).abs() < 1e-4);

    let shift_decimal = weighted_avg_bp / 10_000.0;
    let expected_convexity_pnl =
        0.5 * 100_000.0 * convexity * 100.0 * shift_decimal * shift_decimal;
    assert!(
        expected_convexity_pnl > 1.0,
        "fixture must exercise a material convexity term"
    );

    let expected_total = -1655.0 + expected_convexity_pnl;
    let rates_pnl = attribution.rates_curves_pnl.amount();
    assert!(
        (rates_pnl - expected_total).abs() < 1e-6,
        "convexity must use the DV01-weighted effective shift (≈+8.95bp), not the \
         unweighted mean (0.0); expected {expected_total}, got {rates_pnl}"
    );
}

/// W56: a NaN/Inf factor sensitivity must produce `result_invalid = true`
/// instead of panicking inside `Money::new`.
///
/// Injects `f64::NAN` as the aggregate `Dv01` metric (the fallback path
/// that reads `val_t0.measures["dv01"]` and computes `dv01 * avg_shift`)
/// then asserts the attribution returns without panic and sets
/// `result_invalid = true`.
#[test]
fn nan_factor_sensitivity_sets_result_invalid_instead_of_panicking() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    // A TestInstrument with one discount curve so a measurable rate shift
    // exists — that keeps us in the `dv01 * avg_shift` branch where a NaN
    // DV01 will flow into `Money::new`.
    let instrument: Arc<dyn Instrument> = Arc::new(
        TestInstrument::new("NAN-DV01", Money::from((100000_i64, Currency::USD)))
            .with_discount_curves(&["USD-OIS"]),
    );

    let market_t0 = MarketContext::new().insert(make_flat_curve("USD-OIS", as_of_t0, 0.02));
    let market_t1 = MarketContext::new().insert(make_flat_curve("USD-OIS", as_of_t1, 0.0201));

    // Inject NaN as the Dv01 sensitivity — simulates an overflowed or
    // corrupt Greek value reaching the attribution engine.
    let mut measures_t0 = IndexMap::new();
    measures_t0.insert(MetricId::Dv01, f64::NAN);

    let val_t0 = ValuationResult::stamped_with_meta(
        "NAN-DV01",
        as_of_t0,
        Money::from((100000_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "NAN-DV01",
        as_of_t1,
        Money::from((99600_i64, Currency::USD)),
        meta,
    );

    // Must NOT panic; must return Ok(_) with result_invalid = true.
    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("attribution must not return Err on NaN sensitivity");

    assert!(
        attribution.result_invalid,
        "result_invalid must be true when a NaN factor sensitivity is detected; \
         got result_invalid = false"
    );

    // Residual should be a finite sentinel (zero), not NaN/Inf.
    assert!(
        attribution.residual.amount().is_finite(),
        "residual must be finite (sentinel zero) when result_invalid; got {}",
        attribution.residual.amount()
    );
}

/// W56 (Inf variant): same contract with +Inf sensitivity.
#[test]
fn inf_factor_sensitivity_sets_result_invalid_instead_of_panicking() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

    let instrument: Arc<dyn Instrument> = Arc::new(
        TestInstrument::new("INF-DV01", Money::from((100000_i64, Currency::USD)))
            .with_discount_curves(&["USD-OIS"]),
    );
    let market_t0 = MarketContext::new().insert(make_flat_curve("USD-OIS", as_of_t0, 0.02));
    let market_t1 = MarketContext::new().insert(make_flat_curve("USD-OIS", as_of_t1, 0.0201));

    let mut measures_t0 = IndexMap::new();
    measures_t0.insert(MetricId::Dv01, f64::INFINITY);

    let val_t0 = ValuationResult::stamped_with_meta(
        "INF-DV01",
        as_of_t0,
        Money::from((100000_i64, Currency::USD)),
        meta.clone(),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped_with_meta(
        "INF-DV01",
        as_of_t1,
        Money::from((99600_i64, Currency::USD)),
        meta,
    );

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("attribution must not return Err on Inf sensitivity");

    assert!(
        attribution.result_invalid,
        "result_invalid must be true for Inf factor sensitivity"
    );
    assert!(
        attribution.residual.amount().is_finite(),
        "residual must be finite sentinel when result_invalid"
    );
}

#[test]
fn theta_with_coupon_cannot_be_scaled_to_another_horizon() {
    let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
        "THETA-HORIZON",
        Money::from((1000_i64, Currency::USD)),
    ));
    let market = MarketContext::new();
    let start = date!(2025 - 01 - 01);
    let end = date!(2025 - 01 - 11);
    let opening = ValuationResult::stamped(
        "THETA-HORIZON",
        start,
        Money::from((1000_i64, Currency::USD)),
    )
    .with_measures(IndexMap::from([
        (MetricId::Theta, 100.0),
        (MetricId::CouponIncome, 100.0),
        (MetricId::ThetaPeriodDays, 1.0),
    ]));
    let closing = ValuationResult::stamped("THETA-HORIZON", end, opening.value);
    let error = attribute_pnl_metrics_based(
        &instrument,
        &market,
        &market,
        &opening,
        &closing,
        start,
        end,
    )
    .unwrap_err();
    assert!(error.to_string().contains("matching theta_period_days"));
}

#[test]
fn invalid_carry_horizon_is_not_treated_as_an_absent_stamp() {
    let start = date!(2025 - 01 - 01);
    let end = date!(2025 - 01 - 02);
    let value = Money::from((1000_i64, Currency::USD));
    let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new("HORIZON", value));
    let market = MarketContext::new();
    let closing = ValuationResult::stamped("HORIZON", end, value);
    for horizon in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let opening =
            ValuationResult::stamped("HORIZON", start, value).with_measures(IndexMap::from([
                (MetricId::CarryTotal, 1.0),
                (MetricId::ThetaPeriodDays, horizon),
            ]));
        let error = attribute_pnl_metrics_based(
            &instrument,
            &market,
            &market,
            &opening,
            &closing,
            start,
            end,
        )
        .unwrap_err();
        assert!(error.to_string().contains("matching theta_period_days"));
    }
}

#[test]
fn aggregate_vega_rejects_twist_without_reference_point() {
    let instrument: Arc<dyn Instrument> = Arc::new(SpotVolTestInstrument::new(
        "VOL-TWIST",
        Money::from((1000_i64, Currency::USD)),
    ));
    let surface = |front, back| {
        VolSurface::builder("TEST-VOL")
            .expiries(&[0.5, 5.0])
            .strikes(&[90.0, 110.0])
            .row(&[front, front])
            .row(&[back, back])
            .build()
            .unwrap()
    };
    let opening_market = MarketContext::new().insert_surface(surface(0.2, 0.2));
    // The new closing knot moves while all opening grid points stay flat.
    let closing_surface = VolSurface::builder("TEST-VOL")
        .expiries(&[0.5, 2.0, 5.0])
        .strikes(&[90.0, 110.0])
        .row(&[0.2, 0.2])
        .row(&[0.3, 0.3])
        .row(&[0.2, 0.2])
        .build()
        .unwrap();
    let closing_market = MarketContext::new().insert_surface(closing_surface);
    let start = date!(2025 - 01 - 01);
    let opening =
        ValuationResult::stamped("VOL-TWIST", start, Money::from((1000_i64, Currency::USD)))
            .with_measures(IndexMap::from([(MetricId::Vega, 10.0)]));
    let result = attribute_pnl_metrics_based(
        &instrument,
        &opening_market,
        &closing_market,
        &opening,
        &opening,
        start,
        start,
    )
    .unwrap();
    assert!(result.result_invalid, "{:?}", result.meta.notes);
    assert_eq!(result.vol_pnl.amount(), 0.0);
}
