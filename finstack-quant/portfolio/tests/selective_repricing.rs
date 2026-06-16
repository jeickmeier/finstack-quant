//! Integration tests for the dependency index and selective repricing API.

mod common;

use common::*;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::valuation::{
    revalue_affected, value_portfolio, PortfolioValuationOptions,
    PORTFOLIO_SELECTIVE_REPRICING_EXTENSION_KEY,
};
use finstack_quant_portfolio::MarketFactorKey;
use finstack_quant_portfolio::{Portfolio, PortfolioBuilder};
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::RatesCurveKind;
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
fn selective_reprice_verify_full_eval_mode_matches_full_reprice() {
    let portfolio = build_two_curve_portfolio();
    let mut config = FinstackConfig::default();
    config.extensions.insert(
        PORTFOLIO_SELECTIVE_REPRICING_EXTENSION_KEY,
        serde_json::json!({ "verify_full_eval": true }),
    );
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

    assert!(
        (full_val.total_base_ccy.amount() - selective_val.total_base_ccy.amount()).abs() < 1e-10
    );
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
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    UnresolvableInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl UnresolvableInstrument {
    fn new() -> Self {
        Self {
            attributes: finstack_quant_valuations::instruments::Attributes::default(),
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
        Err(finstack_quant_core::Error::Validation(
            "stub: unresolvable deps".into(),
        ))
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
