//! Shared fixtures for scenarios Criterion targets.
//!
//! Built once outside `b.iter`. Apply helpers clone the market (and optional
//! model / instruments) so fixture construction is not folded into engine cost.

#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{build_periods, Date};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::hierarchy::MarketDataHierarchy;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{
    BaseCorrelationCurve, DiscountCurve, ForwardCurve, HazardCurve, InflationCurve, PriceCurve,
    PriceCurveKind,
};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_models::credit::pool::CorrelationStructure;
use finstack_quant_scenarios::{
    ApplicationReport, Compounding, ExecutionContext, OperationSpec, RateBindingSpec,
    ScenarioEngine, ScenarioSpec,
};
use finstack_quant_statements::types::{AmountOrScalar, NodeSpec, NodeType};
use finstack_quant_statements::FinancialModelSpec;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
use finstack_quant_valuations::instruments::{Bond, Instrument};
use indexmap::{indexmap, IndexMap};
use std::sync::Arc;
use time::macros::date;
use time::Month;

pub const AS_OF: Date = date!(2025 - 01 - 01);
pub const USD_SOFR: &str = "USD_SOFR";
pub const EUR_ESTR: &str = "EUR_ESTR";
pub const USD_SOFR_3M: &str = "USD_SOFR_3M";
pub const USD_CPI: &str = "USD_CPI";
pub const WTI: &str = "WTI";
pub const VIX: &str = "VIX";
pub const SPX_VOL: &str = "SPX_VOL";
pub const CDX_IG_VOL: &str = "CDX_IG_VOL";
pub const CDX_IG: &str = "CDX_IG";
pub const CDX_IG_HAZARD: &str = "CDX_IG_HAZARD";
pub const CDX_HY_HAZARD: &str = "CDX_HY_HAZARD";

pub fn spec(id: &str, operations: Vec<OperationSpec>) -> ScenarioSpec {
    ScenarioSpec {
        id: id.into(),
        name: None,
        description: None,
        operations,
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    }
}

fn discount_knots() -> [(f64, f64); 8] {
    [
        (0.0, 1.0),
        (0.25, 0.99),
        (0.5, 0.98),
        (1.0, 0.96),
        (2.0, 0.92),
        (5.0, 0.82),
        (10.0, 0.65),
        (30.0, 0.35),
    ]
}

pub fn discount_curve(id: &str) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(AS_OF)
        .knots(discount_knots())
        .interp(InterpStyle::MonotoneConvex)
        .build()
        .unwrap()
}

pub fn lean_market() -> MarketContext {
    MarketContext::new().insert(discount_curve(USD_SOFR))
}

fn vol_surface(id: &str, atm: f64) -> VolSurface {
    VolSurface::builder(id)
        .expiries(&[0.25, 0.5, 1.0])
        .strikes(&[90.0, 100.0, 110.0])
        .row(&[atm + 0.05, atm, atm + 0.02])
        .row(&[atm + 0.04, atm - 0.01, atm + 0.01])
        .row(&[atm + 0.03, atm - 0.02, atm])
        .build()
        .unwrap()
}

/// Full book-style market covering every adapter family the engine can shock.
pub fn full_market() -> MarketContext {
    let fx_provider = Arc::new({
        let p = SimpleFxProvider::new();
        p.set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("valid rate");
        p.set_quote(Currency::GBP, Currency::USD, 1.25)
            .expect("valid rate");
        p.set_quote(Currency::JPY, Currency::USD, 0.0067)
            .expect("valid rate");
        p
    });

    let hazard_ig = HazardCurve::builder(CDX_IG_HAZARD)
        .base_date(AS_OF)
        .recovery_rate(0.40)
        .knots([
            (0.0, 0.0),
            (1.0, 0.01),
            (3.0, 0.015),
            (5.0, 0.02),
            (10.0, 0.025),
        ])
        .par_spreads([(1.0, 60.0), (3.0, 90.0), (5.0, 120.0), (10.0, 150.0)])
        .build()
        .unwrap();

    let hazard_hy = HazardCurve::builder(CDX_HY_HAZARD)
        .base_date(AS_OF)
        .recovery_rate(0.30)
        .knots([
            (0.0, 0.0),
            (1.0, 0.05),
            (3.0, 0.06),
            (5.0, 0.07),
            (10.0, 0.08),
        ])
        .par_spreads([(1.0, 350.0), (3.0, 420.0), (5.0, 490.0), (10.0, 560.0)])
        .build()
        .unwrap();

    let forward = ForwardCurve::builder(USD_SOFR_3M, 0.25)
        .base_date(AS_OF)
        .knots([(0.0, 0.04), (1.0, 0.038), (2.0, 0.036), (5.0, 0.035)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let inflation = InflationCurve::builder(USD_CPI)
        .base_date(AS_OF)
        .base_cpi(100.0)
        .knots([(0.0, 100.0), (1.0, 102.0), (5.0, 110.0), (10.0, 122.0)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let commodity = PriceCurve::builder(WTI)
        .base_date(AS_OF)
        .spot_price(70.0)
        .knots([(0.0, 70.0), (1.0, 72.0), (2.0, 74.0), (5.0, 78.0)])
        .build()
        .unwrap();

    let vol_index = PriceCurve::builder(VIX)
        .kind(PriceCurveKind::VolIndex)
        .base_date(AS_OF)
        .spot_price(18.0)
        .knots([(0.0, 18.0), (0.25, 19.0), (1.0, 20.0), (2.0, 21.0)])
        .build()
        .unwrap();

    MarketContext::new()
        .insert(discount_curve(USD_SOFR))
        .insert(discount_curve(EUR_ESTR))
        .insert(forward)
        .insert(inflation)
        .insert(commodity)
        .insert(vol_index)
        .insert_fx(FxMatrix::new(fx_provider))
        .insert_surface(vol_surface(SPX_VOL, 0.20))
        .insert_surface(vol_surface(CDX_IG_VOL, 0.30))
        .insert(
            BaseCorrelationCurve::builder(CDX_IG)
                .knots(vec![(3.0, 0.30), (7.0, 0.50), (10.0, 0.60)])
                .build()
                .unwrap(),
        )
        .insert(hazard_ig)
        .insert(hazard_hy)
        .insert_price(
            "SPY",
            MarketScalar::Price(Money::new(450.0, Currency::USD).expect("valid money fixture")),
        )
        .insert_price(
            "QQQ",
            MarketScalar::Price(Money::new(380.0, Currency::USD).expect("valid money fixture")),
        )
        .insert_price(
            "EWU",
            MarketScalar::Price(Money::new(32.0, Currency::USD).expect("valid money fixture")),
        )
}

/// `n` hazard curves under `Credit/USD/*`, plus one shared discount curve.
pub fn hierarchy_hazard_market(n_curves: usize) -> MarketContext {
    let ids: Vec<String> = (0..n_curves).map(|i| format!("USD_HAZARD_{i}")).collect();
    let mut builder = MarketDataHierarchy::builder();
    for id in &ids {
        builder = builder
            .add_node(&format!("Credit/USD/{id}"))
            .curve_ids(&[id.as_str()]);
    }
    let hierarchy = builder.build().unwrap();

    let mut market = MarketContext::new().insert(discount_curve(USD_SOFR));
    for (i, id) in ids.iter().enumerate() {
        let spread = 60.0 + i as f64;
        market.insert_mut(
            HazardCurve::builder(id.as_str())
                .base_date(AS_OF)
                .recovery_rate(0.40)
                .knots([(1.0, 0.01), (5.0, 0.02)])
                .par_spreads([(1.0, spread), (5.0, spread + 60.0)])
                .build()
                .unwrap(),
        );
    }
    market.set_hierarchy(hierarchy);
    market
}

/// `n` discount curves under `Rates/USD/*`, plus the same ids in the market.
pub fn hierarchy_market(n_curves: usize) -> MarketContext {
    let ids: Vec<String> = (0..n_curves).map(|i| format!("USD_DISC_{i}")).collect();
    let mut builder = MarketDataHierarchy::builder();
    for id in &ids {
        builder = builder
            .add_node(&format!("Rates/USD/{id}"))
            .curve_ids(&[id.as_str()]);
    }
    let hierarchy = builder.build().unwrap();

    let mut market = MarketContext::new();
    for id in &ids {
        market.insert_mut(discount_curve(id));
    }
    market.set_hierarchy(hierarchy);
    market
}

pub fn financial_model() -> FinancialModelSpec {
    let period_plan = build_periods("2025Q1..2026Q4", None).unwrap();
    let periods = period_plan.periods;
    let mut model = FinancialModelSpec::new("test_model", periods.clone());

    let mut revenue_values = IndexMap::new();
    for (i, period) in periods.iter().enumerate() {
        revenue_values.insert(
            period.id,
            AmountOrScalar::Scalar(1_000_000.0 * (1.0 + i as f64 * 0.05)),
        );
    }
    model.add_node(NodeSpec::new("Revenue", NodeType::Value).with_values(revenue_values));

    let mut cogs_values = IndexMap::new();
    for (i, period) in periods.iter().enumerate() {
        cogs_values.insert(
            period.id,
            AmountOrScalar::Scalar(600_000.0 * (1.0 + i as f64 * 0.04)),
        );
    }
    model.add_node(NodeSpec::new("COGS", NodeType::Value).with_values(cogs_values));

    let mut rate_values = IndexMap::new();
    for period in &periods {
        rate_values.insert(period.id, AmountOrScalar::Scalar(0.045));
    }
    model.add_node(NodeSpec::new("InterestRate", NodeType::Value).with_values(rate_values));

    model
}

pub fn interest_rate_bindings(
) -> IndexMap<finstack_quant_statements::types::NodeId, RateBindingSpec> {
    indexmap! {
        "InterestRate".into() => RateBindingSpec {
            node_id: "InterestRate".into(),
            curve_id: USD_SOFR.into(),
            tenor: "1Y".to_string(),
            compounding: Compounding::Continuous,
            day_count: None,
        },
    }
}

pub fn sample_bond(id: &str, maturity_year_offset: i32) -> Bond {
    let maturity =
        Date::from_calendar_date(2025 + maturity_year_offset, Month::January, 1).unwrap();
    let mut bond = Bond::fixed(
        id,
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        AS_OF,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        USD_SOFR,
    )
    .unwrap();
    bond.attributes
        .meta
        .insert("sector".to_string(), "financials".to_string());
    bond
}

pub fn sample_bonds(n: usize) -> Vec<Bond> {
    (0..n)
        .map(|i| sample_bond(&format!("BENCH-BOND-{i}"), 1 + (i % 10) as i32))
        .collect()
}

pub fn boxed_bonds(bonds: &[Bond]) -> Vec<Box<dyn Instrument>> {
    bonds
        .iter()
        .cloned()
        .map(|bond| Box::new(bond) as Box<dyn Instrument>)
        .collect()
}

pub fn structured_credit_with_corr() -> StructuredCredit {
    let mut deal = StructuredCredit::example();
    deal.with_correlation(CorrelationStructure::flat(0.20, -0.15).expect("valid correlation"));
    deal
}

pub fn compose_specs(count: usize) -> Vec<ScenarioSpec> {
    (0..count)
        .map(|i| ScenarioSpec {
            id: format!("scenario_{i}"),
            name: Some(format!("Test Scenario {i}")),
            description: Some(format!("Benchmark scenario number {i}")),
            operations: vec![
                OperationSpec::CurveParallelBp {
                    curve_kind: finstack_quant_scenarios::CurveKind::Discount,
                    curve_id: USD_SOFR.into(),
                    discount_curve_id: None,
                    bp: (i as f64 + 1.0) * 10.0,
                },
                OperationSpec::EquityPricePct {
                    ids: vec!["SPY".into()],
                    pct: -(i as f64 + 1.0) * 2.0,
                },
            ],
            priority: (i % 3) as i32,
            resolution_mode: Default::default(),
            hazard_bump_mode: Default::default(),
        })
        .collect()
}

pub fn apply_market(
    engine: &ScenarioEngine,
    scenario: &ScenarioSpec,
    market: &MarketContext,
) -> ApplicationReport {
    let mut market = market.clone();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: AS_OF,
    };
    engine.apply(scenario, &mut ctx).expect("apply")
}

pub fn apply_with_model(
    engine: &ScenarioEngine,
    scenario: &ScenarioSpec,
    market: &MarketContext,
    model: &FinancialModelSpec,
    rate_bindings: Option<IndexMap<finstack_quant_statements::types::NodeId, RateBindingSpec>>,
) -> ApplicationReport {
    let mut market = market.clone();
    let mut model = model.clone();
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings,
        calendar: None,
        as_of: AS_OF,
    };
    engine.apply(scenario, &mut ctx).expect("apply")
}

pub fn apply_with_instruments(
    engine: &ScenarioEngine,
    scenario: &ScenarioSpec,
    market: &MarketContext,
    bonds: &[Bond],
) -> ApplicationReport {
    let mut market = market.clone();
    let mut instruments = boxed_bonds(bonds);
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: Some(&mut instruments),
        rate_bindings: None,
        calendar: None,
        as_of: AS_OF,
    };
    engine.apply(scenario, &mut ctx).expect("apply")
}

pub fn apply_with_mixed_instruments(
    engine: &ScenarioEngine,
    scenario: &ScenarioSpec,
    market: &MarketContext,
    bonds: &[Bond],
    structured: &StructuredCredit,
) -> ApplicationReport {
    let mut market = market.clone();
    let mut instruments = boxed_bonds(bonds);
    instruments.push(Box::new(structured.clone()));
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: Some(&mut instruments),
        rate_bindings: None,
        calendar: None,
        as_of: AS_OF,
    };
    engine.apply(scenario, &mut ctx).expect("apply")
}
