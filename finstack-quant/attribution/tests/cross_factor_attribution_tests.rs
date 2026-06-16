//! End-to-end integration tests for cross-factor P&L attribution.

use finstack_quant_attribution::{
    attribute_pnl_metrics_based, attribute_pnl_parallel, ExecutionPolicy, PnlAttribution,
};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DateExt, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::{
    Attributes, Instrument, InstrumentCurves, MarketDependencies,
};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::InstrumentType;
use finstack_quant_valuations::results::ValuationResult;
use indexmap::IndexMap;
use rust_decimal::Decimal;
use std::sync::{Arc, OnceLock};
use time::macros::date;
use time::Date;

fn build_discount_curve(id: &str, as_of: Date, rate: f64) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (1.0f64, (-rate).exp()),
            (5.0f64, (-rate * 5.0).exp()),
            (10.0f64, (-rate * 10.0).exp()),
        ])
        .build()
        .expect("discount curve should build")
}

fn build_hazard_curve(id: &str, as_of: Date, hazard_rate: f64) -> HazardCurve {
    HazardCurve::builder(id)
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots([(0.0f64, hazard_rate), (5.0f64, hazard_rate)])
        .build()
        .expect("hazard curve should build")
}

#[derive(Clone)]
struct RatesCreditInteractionInstrument {
    id: String,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    RatesCreditInteractionInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl RatesCreditInteractionInstrument {
    fn new(id: &str) -> Self {
        Self { id: id.to_string() }
    }
}

impl Instrument for RatesCreditInteractionInstrument {
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
        static ATTRS: OnceLock<Attributes> = OnceLock::new();
        ATTRS.get_or_init(Attributes::default)
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        unreachable!("test instrument attributes_mut should not be called")
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        let mut deps = MarketDependencies::new();
        deps.add_curves(
            InstrumentCurves::builder()
                .discount(CurveId::new("USD-OIS"))
                .credit(CurveId::new("ACME-HAZ"))
                .build()?,
        );
        Ok(deps)
    }

    fn base_value(&self, market: &MarketContext, _as_of: Date) -> Result<Money> {
        let rate = market.get_discount("USD-OIS")?.zero(1.0);
        let hazard = market.get_hazard("ACME-HAZ")?.hazard_rate(1.0);
        Ok(Money::new(1_000_000.0 * rate * hazard, Currency::USD))
    }
}

#[test]
fn single_factor_instrument_has_zero_cross_factor() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let market_t0 = MarketContext::new().insert(build_discount_curve("USD-OIS", as_of_t0, 0.03));
    let market_t1 = MarketContext::new().insert(build_discount_curve("USD-OIS", as_of_t1, 0.031));

    let deposit = Arc::new(
        Deposit::builder()
            .id(InstrumentId::new("DEP-1Y"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of_t0)
            .maturity(as_of_t0.add_months(12))
            .day_count(DayCount::Act360)
            .quote_rate_opt(Some(Decimal::ZERO))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("deposit should build"),
    ) as Arc<dyn Instrument>;

    let attr = attribute_pnl_parallel(
        &deposit,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &FinstackConfig::default(),
        ExecutionPolicy::Parallel,
    )
    .expect("parallel attribution should succeed");

    assert_eq!(attr.cross_factor_pnl.amount(), 0.0);
    assert!(attr
        .cross_factor_detail
        .as_ref()
        .map(|detail| detail.total.amount().abs() < 1e-12)
        .unwrap_or(true));
}

#[test]
fn synthetic_parallel_instrument_surfaces_rates_credit_cross_factor() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let market_t0 = MarketContext::new()
        .insert(build_discount_curve("USD-OIS", as_of_t0, 0.01))
        .insert(build_hazard_curve("ACME-HAZ", as_of_t0, 0.01));
    let market_t1 = MarketContext::new()
        .insert(build_discount_curve("USD-OIS", as_of_t1, 0.02))
        .insert(build_hazard_curve("ACME-HAZ", as_of_t1, 0.02));

    let instrument =
        Arc::new(RatesCreditInteractionInstrument::new("PARALLEL-XFACTOR")) as Arc<dyn Instrument>;

    let parallel = attribute_pnl_parallel(
        &instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &FinstackConfig::default(),
        ExecutionPolicy::Parallel,
    )
    .expect("parallel attribution should succeed");

    let parallel_detail = parallel
        .cross_factor_detail
        .as_ref()
        .expect("parallel cross-factor detail should be present");
    assert!(parallel_detail.by_pair.contains_key("Rates×Credit"));
    assert!(parallel.cross_factor_pnl.amount().abs() > 0.0);
    assert_eq!(
        parallel_detail.total.amount(),
        parallel.cross_factor_pnl.amount()
    );

    // The cross term is stored as the additive reconciliation contribution
    // (the negated mixed second difference). For V = k·r·h each isolated
    // factor is measured with the other at its (higher) T1 level, so the sum
    // of isolated effects overstates the total and the additive cross term
    // must be negative: −k·(r1−r0)·(h1−h0).
    assert!(
        parallel.cross_factor_pnl.amount() < 0.0,
        "additive cross term should be negative for an up-up co-movement on V = k·r·h, got {}",
        parallel.cross_factor_pnl.amount()
    );

    // With the pairwise cross term extracted, this purely bilinear two-factor
    // instrument must reconcile exactly: total = rates + credit + cross, so
    // the residual collapses to ~0 (it was −2× the interaction before the
    // sign fix).
    assert!(
        parallel.residual.amount().abs() < 1e-6,
        "residual should be ~0 once the cross term is extracted, got {}",
        parallel.residual.amount()
    );
}

fn assert_policy_equivalent(left: &PnlAttribution, right: &PnlAttribution) {
    assert_eq!(left.total_pnl, right.total_pnl);
    assert_eq!(left.rates_curves_pnl, right.rates_curves_pnl);
    assert_eq!(left.credit_curves_pnl, right.credit_curves_pnl);
    assert_eq!(left.cross_factor_pnl, right.cross_factor_pnl);
    assert_eq!(left.residual, right.residual);
    assert_eq!(left.meta.num_repricings, right.meta.num_repricings);
    assert_eq!(
        left.cross_factor_detail
            .as_ref()
            .map(|detail| &detail.by_pair),
        right
            .cross_factor_detail
            .as_ref()
            .map(|detail| &detail.by_pair)
    );
}

#[test]
fn serial_execution_policy_matches_parallel_cross_factor_attribution() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let market_t0 = MarketContext::new()
        .insert(build_discount_curve("USD-OIS", as_of_t0, 0.01))
        .insert(build_hazard_curve("ACME-HAZ", as_of_t0, 0.01));
    let market_t1 = MarketContext::new()
        .insert(build_discount_curve("USD-OIS", as_of_t1, 0.02))
        .insert(build_hazard_curve("ACME-HAZ", as_of_t1, 0.02));

    let instrument =
        Arc::new(RatesCreditInteractionInstrument::new("POLICY-XFACTOR")) as Arc<dyn Instrument>;

    let parallel = attribute_pnl_parallel(
        &instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &FinstackConfig::default(),
        ExecutionPolicy::Parallel,
    )
    .expect("parallel policy attribution should succeed");

    let serial = attribute_pnl_parallel(
        &instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &FinstackConfig::default(),
        ExecutionPolicy::Serial,
    )
    .expect("serial policy attribution should succeed");

    assert_policy_equivalent(&parallel, &serial);
}

#[test]
fn synthetic_metrics_based_instrument_surfaces_rates_credit_cross_factor() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let market_t0 = MarketContext::new()
        .insert(build_discount_curve("USD-OIS", as_of_t0, 0.01))
        .insert(build_hazard_curve("ACME-HAZ", as_of_t0, 0.01));
    let market_t1 = MarketContext::new()
        .insert(build_discount_curve("USD-OIS", as_of_t1, 0.0101))
        .insert(build_hazard_curve("ACME-HAZ", as_of_t1, 0.0102));

    let instrument =
        Arc::new(RatesCreditInteractionInstrument::new("METRICS-XFACTOR")) as Arc<dyn Instrument>;

    let mut measures_t0 = IndexMap::new();
    measures_t0.insert(MetricId::Theta, 0.0);
    measures_t0.insert(MetricId::Dv01, 0.0);
    measures_t0.insert(MetricId::Cs01, 0.0);
    measures_t0.insert(MetricId::CrossGammaRatesCredit, 5.0);
    let val_t0 = ValuationResult::stamped(
        "METRICS-XFACTOR",
        as_of_t0,
        Money::new(100.0, Currency::USD),
    )
    .with_measures(measures_t0);
    let val_t1 = ValuationResult::stamped(
        "METRICS-XFACTOR",
        as_of_t1,
        Money::new(150.0, Currency::USD),
    );

    let metrics_based = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("metrics-based attribution should succeed");

    assert!(
        metrics_based.cross_factor_pnl.amount().abs() > 1e-6,
        "metrics-based attribution should surface an explicit rates-credit cross term",
    );
    let metrics_detail = metrics_based
        .cross_factor_detail
        .as_ref()
        .expect("metrics-based cross-factor detail should be present");
    assert!(metrics_detail.by_pair.contains_key("Rates×Credit"));
    assert!((metrics_detail.total.amount() - metrics_based.cross_factor_pnl.amount()).abs() < 1e-9);
}
