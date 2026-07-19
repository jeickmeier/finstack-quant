//! Integration tests for the dependency index and selective repricing API.

mod common;

use common::*;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::fx::{
    FxConversionPolicy, FxMatrix, FxProvider, FxQuery, SimpleFxProvider,
};
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::valuation::{
    revalue_affected, value_portfolio, PortfolioValuationOptions,
};
use finstack_quant_portfolio::MarketFactorKey;
use finstack_quant_portfolio::{Portfolio, PortfolioBuilder};
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::{
    Attributes, Instrument, MarketDependencies, PricingOptions, RatesCurveKind,
};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::InstrumentType;
use finstack_quant_valuations::results::ValuationResult;
use std::any::Any;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const EUR_KNOTS: [(f64, f64); 3] = [(0.0, 1.0), (1.0, 0.97), (5.0, 0.85)];
const USD_KNOTS: [(f64, f64); 3] = [(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)];
const USD_BUMPED_KNOTS: [(f64, f64); 3] = [(0.0, 1.0), (1.0, 0.975), (5.0, 0.88)];

fn usd_discount_key() -> MarketFactorKey {
    MarketFactorKey::curve("USD".into(), RatesCurveKind::Discount)
}

fn make_curve(id: &str, knots: &[(f64, f64)]) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(base_date())
        .knots(knots.to_vec())
        .interp(InterpStyle::Linear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build()
        .unwrap()
}

fn make_market(usd_knots: &[(f64, f64)], eur_knots: &[(f64, f64)]) -> MarketContext {
    MarketContext::new()
        .insert(make_curve("USD", usd_knots))
        .insert(make_curve("EUR", eur_knots))
}

fn two_curve_market() -> MarketContext {
    make_market(&USD_KNOTS, &EUR_KNOTS)
}

fn bumped_usd_market() -> MarketContext {
    make_market(&USD_BUMPED_KNOTS, &EUR_KNOTS)
}

fn make_deposit(id: &str, curve_id: &str, notional: f64) -> Deposit {
    make_deposit_ccy(id, curve_id, notional, Currency::USD)
}

fn make_deposit_ccy(id: &str, curve_id: &str, notional: f64, currency: Currency) -> Deposit {
    Deposit::builder()
        .id(id.into())
        .notional(Money::new(notional, currency))
        .start_date(base_date())
        .maturity(base_date() + time::Duration::days(90))
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id(curve_id.into())
        .quote_rate_opt(Some(rust_decimal::Decimal::try_from(0.045).unwrap()))
        .build()
        .unwrap()
}

struct StaticFx {
    rate: f64,
}

impl FxProvider for StaticFx {
    fn rate(
        &self,
        _from: Currency,
        _to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_quant_core::Result<f64> {
        Ok(self.rate)
    }
}

fn fx_matrix(rate: f64) -> FxMatrix {
    FxMatrix::new(Arc::new(StaticFx { rate }))
}

fn triangulated_fx_matrix(eur_usd: f64) -> FxMatrix {
    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quotes(&[
            (Currency::EUR, Currency::USD, eur_usd),
            (Currency::USD, Currency::JPY, 150.0),
        ])
        .expect("valid triangulation quotes");
    FxMatrix::new(provider)
}

fn market_with_fx(
    usd_knots: &[(f64, f64)],
    eur_knots: &[(f64, f64)],
    eur_usd: f64,
) -> MarketContext {
    make_market(usd_knots, eur_knots).insert_fx(fx_matrix(eur_usd))
}

fn build_two_curve_portfolio() -> Portfolio {
    let dep_usd = make_deposit("DEP_USD", "USD", 1_000_000.0);
    let dep_eur = make_deposit("DEP_EUR", "EUR", 500_000.0);

    let pos_usd = Position::new(
        "POS_USD",
        "ENTITY_A",
        "DEP_USD",
        Arc::new(dep_usd),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let pos_eur = Position::new(
        "POS_EUR",
        "ENTITY_A",
        "DEP_EUR",
        Arc::new(dep_eur),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    PortfolioBuilder::new("TEST")
        .base_ccy(Currency::USD)
        .as_of(base_date())
        .entity(Entity::new("ENTITY_A"))
        .position(pos_usd)
        .position(pos_eur)
        .build()
        .unwrap()
}

// ---------------------------------------------------------------------------
// Dependency Index Tests
// ---------------------------------------------------------------------------

#[test]
fn dependency_index_built_by_builder() {
    let portfolio = build_two_curve_portfolio();
    let index = portfolio.dependency_index();

    assert!(!index.is_empty(), "index should contain factor keys");
    assert!(
        index.factor_count() >= 2,
        "at least USD + EUR discount curves"
    );
}

#[test]
fn dependency_index_rebuilt_after_mutation() {
    let mut portfolio = build_two_curve_portfolio();
    let count_before = portfolio.dependency_index().factor_count();

    let dep3 = make_deposit("DEP_GBP", "GBP", 250_000.0);
    let pos3 = Position::new(
        "POS_GBP",
        "ENTITY_A",
        "DEP_GBP",
        Arc::new(dep3),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    portfolio.add_position(pos3).unwrap();

    assert!(
        portfolio.dependency_index().factor_count() > count_before,
        "new GBP curve should appear in the index"
    );
}

#[test]
fn positions_for_key_returns_correct_indices() {
    let portfolio = build_two_curve_portfolio();
    let index = portfolio.dependency_index();

    let usd_indices = index.positions_for_key(&usd_discount_key());
    assert_eq!(usd_indices.len(), 1);
    assert_eq!(
        portfolio.positions()[usd_indices[0]].position_id.as_str(),
        "POS_USD"
    );

    let eur_key = MarketFactorKey::curve("EUR".into(), RatesCurveKind::Discount);
    let eur_indices = index.positions_for_key(&eur_key);
    assert_eq!(eur_indices.len(), 1);
    assert_eq!(
        portfolio.positions()[eur_indices[0]].position_id.as_str(),
        "POS_EUR"
    );
}

#[test]
fn affected_positions_deduplicates() {
    let portfolio = build_two_curve_portfolio();

    let key = usd_discount_key();
    let indices = portfolio
        .dependency_index()
        .affected_positions(&[key.clone(), key]);
    assert_eq!(indices.len(), 1);
}

#[test]
fn empty_portfolio_has_empty_index() {
    let portfolio = Portfolio::builder("EMPTY")
        .base_ccy(Currency::USD)
        .as_of(base_date())
        .build()
        .unwrap();
    assert!(portfolio.dependency_index().is_empty());
    assert_eq!(portfolio.dependency_index().factor_count(), 0);
}

// ---------------------------------------------------------------------------
// Selective Repricing Parity Tests
// ---------------------------------------------------------------------------

#[test]
fn selective_reprice_matches_full_reprice_when_one_curve_changes() {
    let portfolio = build_two_curve_portfolio();
    let config = FinstackConfig::default();
    let options = PortfolioValuationOptions::default();

    let base_market = two_curve_market();
    let bumped_market = bumped_usd_market();

    let base_val = value_portfolio(&portfolio, &base_market, &config, &Default::default()).unwrap();
    let full_val =
        value_portfolio(&portfolio, &bumped_market, &config, &Default::default()).unwrap();

    let selective_val = revalue_affected(
        &portfolio,
        &bumped_market,
        &config,
        &options,
        &base_val,
        &[usd_discount_key()],
    )
    .unwrap();

    let full_total = full_val.total_base_ccy.amount();
    let selective_total = selective_val.total_base_ccy.amount();

    assert!(
        (full_total - selective_total).abs() < 1e-10,
        "total mismatch: full={full_total}, selective={selective_total}"
    );

    for (pid, full_pv) in &full_val.position_values {
        let sel_pv = selective_val
            .get_position_value(pid.as_str())
            .unwrap_or_else(|| panic!("missing position {pid}"));
        assert!(
            (full_pv.value_base.amount() - sel_pv.value_base.amount()).abs() < 1e-10,
            "position {pid} mismatch"
        );
    }
}

#[test]
fn selective_reprice_no_changes_returns_prior() {
    let portfolio = build_two_curve_portfolio();
    let config = FinstackConfig::default();
    let options = PortfolioValuationOptions::default();

    let market = two_curve_market();
    let base_val = value_portfolio(&portfolio, &market, &config, &Default::default()).unwrap();

    let nonexistent_key = MarketFactorKey::curve("JPY".into(), RatesCurveKind::Discount);

    let result = revalue_affected(
        &portfolio,
        &market,
        &config,
        &options,
        &base_val,
        &[nonexistent_key],
    )
    .unwrap();

    assert!(
        (result.total_base_ccy.amount() - base_val.total_base_ccy.amount()).abs() < 1e-14,
        "no-change reprice should return identical total"
    );
}

#[test]
fn selective_reprice_eur_position_unchanged_when_usd_bumped() {
    let portfolio = build_two_curve_portfolio();
    let config = FinstackConfig::default();
    let options = PortfolioValuationOptions::default();

    let base_market = two_curve_market();
    let bumped_market = bumped_usd_market();

    let base_val = value_portfolio(&portfolio, &base_market, &config, &Default::default()).unwrap();

    let selective_val = revalue_affected(
        &portfolio,
        &bumped_market,
        &config,
        &options,
        &base_val,
        &[usd_discount_key()],
    )
    .unwrap();

    let base_eur = base_val.get_position_value("POS_EUR").unwrap();
    let sel_eur = selective_val.get_position_value("POS_EUR").unwrap();
    assert!(
        (base_eur.value_base.amount() - sel_eur.value_base.amount()).abs() < 1e-14,
        "EUR position should be untouched when only USD curve moves"
    );

    let base_usd = base_val.get_position_value("POS_USD").unwrap();
    let sel_usd = selective_val.get_position_value("POS_USD").unwrap();
    assert!(
        (base_usd.value_base.amount() - sel_usd.value_base.amount()).abs() > 1e-6,
        "USD position should change when USD curve moves"
    );
}

#[test]
fn entity_totals_consistent_after_selective_reprice() {
    let portfolio = build_two_curve_portfolio();
    let config = FinstackConfig::default();
    let options = PortfolioValuationOptions::default();

    let base_market = two_curve_market();
    let bumped_market = bumped_usd_market();

    let base_val = value_portfolio(&portfolio, &base_market, &config, &Default::default()).unwrap();
    let full_val =
        value_portfolio(&portfolio, &bumped_market, &config, &Default::default()).unwrap();

    let selective_val = revalue_affected(
        &portfolio,
        &bumped_market,
        &config,
        &options,
        &base_val,
        &[usd_discount_key()],
    )
    .unwrap();

    for (entity_id, full_money) in &full_val.by_entity {
        let sel_money = selective_val
            .get_entity_value(entity_id.as_str())
            .unwrap_or_else(|| panic!("missing entity {entity_id}"));
        assert!(
            (full_money.amount() - sel_money.amount()).abs() < 1e-10,
            "entity {entity_id} total mismatch: full={}, selective={}",
            full_money.amount(),
            sel_money.amount()
        );
    }
}

#[test]
fn base_then_selective_reprice_round_trip() {
    let portfolio = build_two_curve_portfolio();
    let config = FinstackConfig::default();
    let options = PortfolioValuationOptions::default();

    let base_market = two_curve_market();
    let bumped_market = bumped_usd_market();

    let base = value_portfolio(&portfolio, &base_market, &config, &Default::default()).unwrap();
    let bumped = revalue_affected(
        &portfolio,
        &bumped_market,
        &config,
        &options,
        &base,
        &[usd_discount_key()],
    )
    .unwrap();

    assert!(
        (base.total_base_ccy.amount() - bumped.total_base_ccy.amount()).abs() > 1e-6,
        "bumped total should differ from base"
    );
}

#[test]
fn selective_reprice_fx_change_reprices_native_non_base_positions() {
    let dep_eur = make_deposit_ccy("DEP_EUR", "EUR", 1_000_000.0, Currency::EUR);
    let pos_eur = Position::new(
        "POS_EUR",
        "ENTITY_A",
        "DEP_EUR",
        Arc::new(dep_eur),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let portfolio = PortfolioBuilder::new("TEST")
        .base_ccy(Currency::USD)
        .as_of(base_date())
        .entity(Entity::new("ENTITY_A"))
        .position(pos_eur)
        .build()
        .unwrap();
    let config = FinstackConfig::default();
    let options = PortfolioValuationOptions::default();
    let base_market = market_with_fx(&USD_KNOTS, &EUR_KNOTS, 1.10);
    let bumped_market = market_with_fx(&USD_KNOTS, &EUR_KNOTS, 1.20);

    let base_val = value_portfolio(&portfolio, &base_market, &config, &options).unwrap();
    let full_val = value_portfolio(&portfolio, &bumped_market, &config, &options).unwrap();
    let selective_val = revalue_affected(
        &portfolio,
        &bumped_market,
        &config,
        &options,
        &base_val,
        &[MarketFactorKey::fx(Currency::EUR, Currency::USD)],
    )
    .unwrap();

    assert!(
        (full_val.total_base_ccy.amount() - selective_val.total_base_ccy.amount()).abs() < 1e-10,
        "B-3 selective FX repricing should match full repricing: full={}, selective={}",
        full_val.total_base_ccy.amount(),
        selective_val.total_base_ccy.amount()
    );
    assert!(
        (base_val.total_base_ccy.amount() - selective_val.total_base_ccy.amount()).abs() > 1e-6,
        "FX-only selective repricing should not reuse the stale prior base value"
    );
}

// ---------------------------------------------------------------------------
// Unresolved Position Tests
// ---------------------------------------------------------------------------

/// Stub instrument whose `market_dependencies()` always fails.
#[derive(Clone)]
struct UnresolvableInstrument {
    attributes: finstack_quant_valuations::instruments::Attributes,
    fail_dependencies: bool,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    UnresolvableInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl UnresolvableInstrument {
    fn new() -> Self {
        Self {
            attributes: finstack_quant_valuations::instruments::Attributes::default(),
            fail_dependencies: true,
        }
    }

    fn with_empty_dependencies() -> Self {
        Self {
            attributes: finstack_quant_valuations::instruments::Attributes::default(),
            fail_dependencies: false,
        }
    }
}

impl Instrument for UnresolvableInstrument {
    fn id(&self) -> &str {
        "UNRESOLVABLE"
    }
    fn key(&self) -> finstack_quant_valuations::pricer::InstrumentType {
        finstack_quant_valuations::pricer::InstrumentType::Deposit
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn attributes(&self) -> &finstack_quant_valuations::instruments::Attributes {
        &self.attributes
    }
    fn attributes_mut(&mut self) -> &mut finstack_quant_valuations::instruments::Attributes {
        &mut self.attributes
    }
    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }
    fn base_value(
        &self,
        _market: &finstack_quant_core::market_data::context::MarketContext,
        _as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        Ok(Money::new(0.0, Currency::USD))
    }
    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
    {
        if self.fail_dependencies {
            Err(finstack_quant_core::Error::Validation(
                "stub: unresolvable deps".into(),
            ))
        } else {
            Ok(finstack_quant_valuations::instruments::MarketDependencies::new())
        }
    }
}

#[test]
fn unresolved_positions_always_included_in_affected() {
    let dep = make_deposit("DEP_USD", "USD", 1_000_000.0);
    let pos_resolved = Position::new(
        "POS_RESOLVED",
        "ENTITY_A",
        "DEP_USD",
        Arc::new(dep),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let pos_unresolvable = Position::new(
        "POS_UNRESOLVABLE",
        "ENTITY_A",
        "UNRESOLVABLE",
        Arc::new(UnresolvableInstrument::new()),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("TEST")
        .base_ccy(Currency::USD)
        .as_of(base_date())
        .entity(Entity::new("ENTITY_A"))
        .position(pos_resolved)
        .position(pos_unresolvable)
        .build()
        .unwrap();

    let index = portfolio.dependency_index();
    assert_eq!(
        index.unresolved().len(),
        1,
        "one position should be unresolved"
    );

    let unrelated_key = MarketFactorKey::curve("JPY".into(), RatesCurveKind::Discount);
    let affected = index.affected_positions(&[unrelated_key]);
    assert!(
        affected.contains(&1),
        "unresolved position index should appear in affected set even for unrelated keys"
    );
}

#[test]
fn empty_compatibility_dependencies_are_conservatively_unresolved() {
    let position = Position::new(
        "POS_DEFAULT_DEPS",
        "ENTITY_A",
        "DEFAULT_DEPS",
        Arc::new(UnresolvableInstrument::with_empty_dependencies()),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let portfolio = PortfolioBuilder::new("DEFAULT_DEPS")
        .base_ccy(Currency::USD)
        .as_of(base_date())
        .entity(Entity::new("ENTITY_A"))
        .position(position)
        .build()
        .unwrap();

    let index = portfolio.dependency_index();
    assert_eq!(index.unresolved(), &[0]);
    assert_eq!(
        index.affected_positions(&[MarketFactorKey::curve(
            "USD-OIS".into(),
            RatesCurveKind::Discount,
        )]),
        vec![0],
    );
}

// ---------------------------------------------------------------------------
// Exact-profile and workload-shape acceptance tests
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct DependencyProbeInstrument {
    id: String,
    value: Money,
    discount_curve_id: Option<String>,
    fx_pair: Option<(Currency, Currency)>,
    value_fx_pair: Option<(Currency, Currency)>,
    base_calls: Arc<AtomicUsize>,
    pv_only_calls: Arc<AtomicUsize>,
    metric_calls: Arc<AtomicUsize>,
    attributes: Attributes,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    DependencyProbeInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl Instrument for DependencyProbeInstrument {
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
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        self.base_calls.fetch_add(1, Ordering::SeqCst);
        self.resolved_value(market, as_of)
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
        options: PricingOptions,
    ) -> finstack_quant_core::Result<ValuationResult> {
        if metrics.is_empty() {
            self.pv_only_calls.fetch_add(1, Ordering::SeqCst);
        } else {
            self.metric_calls.fetch_add(1, Ordering::SeqCst);
        }
        let config = options.config.as_deref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "selective test expected the executor request config".to_string(),
            )
        })?;
        Ok(ValuationResult::stamped_with_config(
            self.id(),
            as_of,
            self.resolved_value(market, as_of)?,
            config,
        ))
    }

    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        let mut dependencies = MarketDependencies::new();
        if let Some(curve_id) = &self.discount_curve_id {
            dependencies.add_discount_curve(curve_id.clone());
        }
        if let Some((base, quote)) = self.fx_pair {
            dependencies.add_fx_pair(base, quote);
        }
        Ok(dependencies)
    }
}

impl DependencyProbeInstrument {
    fn resolved_value(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let Some((base, quote)) = self.value_fx_pair else {
            return Ok(self.value);
        };
        let rate = market
            .fx_required()?
            .rate(FxQuery::new(base, quote, as_of))?
            .rate;
        Ok(Money::new(
            self.value.amount() * rate,
            self.value.currency(),
        ))
    }
}

#[derive(Clone)]
struct DependencyProbe {
    base_calls: Arc<AtomicUsize>,
    pv_only_calls: Arc<AtomicUsize>,
    metric_calls: Arc<AtomicUsize>,
}

fn dependency_probe_instrument(
    id: impl Into<String>,
    value: Money,
    discount_curve_id: Option<String>,
    fx_pair: Option<(Currency, Currency)>,
    value_fx_pair: Option<(Currency, Currency)>,
) -> (Arc<dyn Instrument>, DependencyProbe) {
    let base_calls = Arc::new(AtomicUsize::new(0));
    let pv_only_calls = Arc::new(AtomicUsize::new(0));
    let metric_calls = Arc::new(AtomicUsize::new(0));
    let instrument = DependencyProbeInstrument {
        id: id.into(),
        value,
        discount_curve_id,
        fx_pair,
        value_fx_pair,
        base_calls: Arc::clone(&base_calls),
        pv_only_calls: Arc::clone(&pv_only_calls),
        metric_calls: Arc::clone(&metric_calls),
        attributes: Attributes::new(),
    };
    (
        Arc::new(instrument),
        DependencyProbe {
            base_calls,
            pv_only_calls,
            metric_calls,
        },
    )
}

fn build_dependency_probe_portfolio(position_count: usize) -> (Portfolio, Vec<DependencyProbe>) {
    let mut builder = PortfolioBuilder::new(format!("SELECTIVE_{position_count}"))
        .base_ccy(Currency::USD)
        .as_of(base_date())
        .entity(Entity::new("ENTITY_A"));
    let mut probes = Vec::with_capacity(position_count);

    for index in 0..position_count {
        let instrument_id = format!("PROBE_{index:04}");
        let (instrument, probe) = dependency_probe_instrument(
            instrument_id.clone(),
            Money::new((index + 1) as f64, Currency::USD),
            Some(format!("CURVE_{index:04}")),
            None,
            None,
        );
        builder = builder.position(
            Position::new(
                format!("POSITION_{index:04}"),
                "ENTITY_A",
                instrument_id,
                instrument,
                1.0,
                PositionUnit::Units,
            )
            .expect("valid dependency-probe position"),
        );
        probes.push(probe);
    }

    (
        builder.build().expect("valid dependency-probe portfolio"),
        probes,
    )
}

fn selective_pv_options() -> PortfolioValuationOptions {
    PortfolioValuationOptions {
        strict_risk: true,
        metrics: finstack_quant_portfolio::valuation::RequestedMetrics::Only(Vec::new()),
    }
}

fn reset_probes(probes: &[DependencyProbe]) {
    for probe in probes {
        probe.base_calls.store(0, Ordering::SeqCst);
        probe.pv_only_calls.store(0, Ordering::SeqCst);
        probe.metric_calls.store(0, Ordering::SeqCst);
    }
}

fn total_pv_only_calls(probes: &[DependencyProbe]) -> usize {
    probes
        .iter()
        .map(|probe| probe.pv_only_calls.load(Ordering::SeqCst))
        .sum()
}

fn total_metric_calls(probes: &[DependencyProbe]) -> usize {
    probes
        .iter()
        .map(|probe| probe.metric_calls.load(Ordering::SeqCst))
        .sum()
}

fn curve_keys(count: usize) -> Vec<MarketFactorKey> {
    (0..count)
        .map(|index| {
            MarketFactorKey::curve(format!("CURVE_{index:04}").into(), RatesCurveKind::Discount)
        })
        .collect()
}

fn assert_same_valuation(
    expected: &finstack_quant_portfolio::valuation::PortfolioValuation,
    actual: &finstack_quant_portfolio::valuation::PortfolioValuation,
) {
    assert_eq!(actual.as_of, expected.as_of);
    assert_eq!(actual.total_base_ccy, expected.total_base_ccy);
    assert_eq!(actual.by_entity, expected.by_entity);
    assert_eq!(
        actual.position_values.keys().collect::<Vec<_>>(),
        expected.position_values.keys().collect::<Vec<_>>()
    );
    for (position_id, expected_value) in &expected.position_values {
        let actual_value = &actual.position_values[position_id];
        assert_eq!(actual_value.value_native, expected_value.value_native);
        assert_eq!(actual_value.value_base, expected_value.value_base);
        assert_eq!(
            actual_value.risk_metrics_complete,
            expected_value.risk_metrics_complete
        );
    }
}

#[test]
fn selective_dirty_fractions_match_full_repricing_and_exact_call_counts() {
    let (portfolio, probes) = build_dependency_probe_portfolio(100);
    let market = MarketContext::new();
    let config = FinstackConfig::default();
    let options = selective_pv_options();
    let prior = value_portfolio(&portfolio, &market, &config, &options).expect("base valuation");

    for dirty_count in [0usize, 1, 25, 50, 100] {
        reset_probes(&probes);
        let changed = if dirty_count == 0 {
            vec![MarketFactorKey::curve(
                "UNCHANGED_CURVE".into(),
                RatesCurveKind::Discount,
            )]
        } else {
            curve_keys(dirty_count)
        };
        let selective = revalue_affected(&portfolio, &market, &config, &options, &prior, &changed)
            .expect("selective valuation");
        assert_eq!(
            total_pv_only_calls(&probes),
            dirty_count,
            "the authoritative dirty set should control the number of PV calls"
        );

        let full = value_portfolio(&portfolio, &market, &config, &options)
            .expect("full comparison valuation");
        assert_same_valuation(&full, &selective);
    }
}

#[test]
fn reordered_prior_position_values_force_full_repricing() {
    let (portfolio, probes) = build_dependency_probe_portfolio(4);
    let market = MarketContext::new();
    let config = FinstackConfig::default();
    let options = selective_pv_options();
    let mut prior =
        value_portfolio(&portfolio, &market, &config, &options).expect("base valuation");
    prior.position_values.swap_indices(0, 1);
    reset_probes(&probes);

    let result = revalue_affected(
        &portfolio,
        &market,
        &config,
        &options,
        &prior,
        &[MarketFactorKey::curve(
            "UNCHANGED_CURVE".into(),
            RatesCurveKind::Discount,
        )],
    )
    .expect("reordered-prior valuation");

    assert_eq!(
        total_pv_only_calls(&probes),
        portfolio.positions().len(),
        "selective reuse requires the executor's exact position order"
    );
    let full =
        value_portfolio(&portfolio, &market, &config, &options).expect("full comparison valuation");
    assert_same_valuation(&full, &result);
}

#[test]
fn selective_63_and_64_affected_positions_preserve_results_and_call_counts() {
    let (portfolio, probes) = build_dependency_probe_portfolio(128);
    let market = MarketContext::new();
    let config = FinstackConfig::default();
    let options = selective_pv_options();
    let prior = value_portfolio(&portfolio, &market, &config, &options).expect("base valuation");

    for dirty_count in [63usize, 64] {
        reset_probes(&probes);
        let selective = revalue_affected(
            &portfolio,
            &market,
            &config,
            &options,
            &prior,
            &curve_keys(dirty_count),
        )
        .expect("threshold selective valuation");
        assert_eq!(total_pv_only_calls(&probes), dirty_count);

        let full = value_portfolio(&portfolio, &market, &config, &options)
            .expect("full threshold comparison");
        assert_same_valuation(&full, &selective);
    }
}

#[test]
fn selective_profile_change_forces_a_full_metric_reprice() {
    let (portfolio, probes) = build_dependency_probe_portfolio(4);
    let market = MarketContext::new();
    let config = FinstackConfig::default();
    let prior = value_portfolio(&portfolio, &market, &config, &selective_pv_options())
        .expect("PV-only prior");
    reset_probes(&probes);

    let target_options = PortfolioValuationOptions {
        strict_risk: true,
        metrics: finstack_quant_portfolio::valuation::RequestedMetrics::Only(vec![MetricId::Dv01]),
    };
    let result = revalue_affected(
        &portfolio,
        &market,
        &config,
        &target_options,
        &prior,
        &[MarketFactorKey::curve(
            "UNCHANGED_CURVE".into(),
            RatesCurveKind::Discount,
        )],
    )
    .expect("profile-changing selective request");

    assert_eq!(
        total_metric_calls(&probes),
        portfolio.positions().len(),
        "a PV-only prior cannot satisfy an exact metric profile"
    );
    assert!(result
        .position_values
        .values()
        .all(|value| value.risk_metrics_complete));
}

#[test]
fn selective_instrument_replacement_with_same_position_id_forces_reprice() {
    let config = FinstackConfig::default();
    let market = MarketContext::new();
    let options = selective_pv_options();
    let (base_instrument, _) = dependency_probe_instrument(
        "BASE_INSTRUMENT",
        Money::new(100.0, Currency::USD),
        Some("CURVE_A".to_string()),
        None,
        None,
    );
    let base_portfolio = PortfolioBuilder::new("BASE")
        .base_ccy(Currency::USD)
        .as_of(base_date())
        .entity(Entity::new("ENTITY_A"))
        .position(
            Position::new(
                "SAME_POSITION",
                "ENTITY_A",
                "BASE_INSTRUMENT",
                base_instrument,
                1.0,
                PositionUnit::Units,
            )
            .expect("base position"),
        )
        .build()
        .expect("base portfolio");
    let prior =
        value_portfolio(&base_portfolio, &market, &config, &options).expect("base valuation");

    let (replacement, replacement_probe) = dependency_probe_instrument(
        "BASE_INSTRUMENT",
        Money::new(250.0, Currency::USD),
        Some("CURVE_B".to_string()),
        None,
        None,
    );
    let replacement_portfolio = PortfolioBuilder::new("REPLACEMENT")
        .base_ccy(Currency::USD)
        .as_of(base_date())
        .entity(Entity::new("ENTITY_A"))
        .position(
            Position::new(
                "SAME_POSITION",
                "ENTITY_A",
                "BASE_INSTRUMENT",
                replacement,
                1.0,
                PositionUnit::Units,
            )
            .expect("replacement position"),
        )
        .build()
        .expect("replacement portfolio");

    let result = revalue_affected(
        &replacement_portfolio,
        &market,
        &config,
        &options,
        &prior,
        &[MarketFactorKey::curve(
            "UNCHANGED_CURVE".into(),
            RatesCurveKind::Discount,
        )],
    )
    .expect("replacement valuation");

    assert_eq!(
        replacement_probe.pv_only_calls.load(Ordering::SeqCst),
        1,
        "portfolio-state identity must change when economics change under the same position and instrument IDs"
    );
    assert_eq!(
        result.position_values["SAME_POSITION"].value_base.amount(),
        250.0
    );
}

#[test]
fn selective_portfolio_shape_change_forces_full_reprice() {
    let (base_portfolio, _) = build_dependency_probe_portfolio(3);
    let (expanded_portfolio, expanded_probes) = build_dependency_probe_portfolio(4);
    let market = MarketContext::new();
    let config = FinstackConfig::default();
    let options = selective_pv_options();
    let prior =
        value_portfolio(&base_portfolio, &market, &config, &options).expect("base valuation");
    reset_probes(&expanded_probes);

    let selective = revalue_affected(
        &expanded_portfolio,
        &market,
        &config,
        &options,
        &prior,
        &[MarketFactorKey::curve(
            "UNCHANGED_CURVE".into(),
            RatesCurveKind::Discount,
        )],
    )
    .expect("shape-changing revaluation");

    assert_eq!(
        total_pv_only_calls(&expanded_probes),
        expanded_portfolio.positions().len()
    );
    let full = value_portfolio(&expanded_portfolio, &market, &config, &options)
        .expect("full shape comparison");
    assert_same_valuation(&full, &selective);
}

#[test]
fn selective_fx_change_reprices_only_fx_dependent_instrument_and_refreshes_all_conversions() {
    let (fx_dependent, fx_probe) = dependency_probe_instrument(
        "FX_DEPENDENT",
        Money::new(100.0, Currency::EUR),
        None,
        Some((Currency::EUR, Currency::USD)),
        None,
    );
    let (conversion_only, conversion_probe) = dependency_probe_instrument(
        "CONVERSION_ONLY",
        Money::new(200.0, Currency::EUR),
        Some("UNRELATED_STATIC_INPUT".to_string()),
        None,
        None,
    );
    let (triangulated_cross, triangulated_probe) = dependency_probe_instrument(
        "TRIANGULATED_CROSS",
        Money::new(50.0, Currency::USD),
        None,
        Some((Currency::EUR, Currency::JPY)),
        Some((Currency::EUR, Currency::JPY)),
    );
    let portfolio = PortfolioBuilder::new("FX_SELECTIVE")
        .base_ccy(Currency::USD)
        .as_of(base_date())
        .entity(Entity::new("ENTITY_A"))
        .position(
            Position::new(
                "FX_DEPENDENT_POSITION",
                "ENTITY_A",
                "FX_DEPENDENT",
                fx_dependent,
                1.0,
                PositionUnit::Units,
            )
            .expect("FX-dependent position"),
        )
        .position(
            Position::new(
                "CONVERSION_ONLY_POSITION",
                "ENTITY_A",
                "CONVERSION_ONLY",
                conversion_only,
                1.0,
                PositionUnit::Units,
            )
            .expect("conversion-only position"),
        )
        .position(
            Position::new(
                "TRIANGULATED_CROSS_POSITION",
                "ENTITY_A",
                "TRIANGULATED_CROSS",
                triangulated_cross,
                1.0,
                PositionUnit::Units,
            )
            .expect("triangulated-cross position"),
        )
        .build()
        .expect("FX selective portfolio");
    let config = FinstackConfig::default();
    let options = selective_pv_options();
    let base_market = MarketContext::new().insert_fx(triangulated_fx_matrix(1.10));
    let stressed_market = MarketContext::new().insert_fx(triangulated_fx_matrix(1.20));
    let prior =
        value_portfolio(&portfolio, &base_market, &config, &options).expect("base FX valuation");
    fx_probe.pv_only_calls.store(0, Ordering::SeqCst);
    conversion_probe.pv_only_calls.store(0, Ordering::SeqCst);
    triangulated_probe.pv_only_calls.store(0, Ordering::SeqCst);

    let selective = revalue_affected(
        &portfolio,
        &stressed_market,
        &config,
        &options,
        &prior,
        &[MarketFactorKey::fx(Currency::EUR, Currency::USD)],
    )
    .expect("selective FX valuation");
    assert_eq!(fx_probe.pv_only_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        triangulated_probe.pv_only_calls.load(Ordering::SeqCst),
        1,
        "any changed FX quote can alter an instrument's triangulated cross"
    );
    assert_eq!(
        conversion_probe.pv_only_calls.load(Ordering::SeqCst),
        0,
        "conversion-only FX changes should reuse native PV"
    );

    let full = value_portfolio(&portfolio, &stressed_market, &config, &options)
        .expect("full FX comparison");
    assert_same_valuation(&full, &selective);
}
