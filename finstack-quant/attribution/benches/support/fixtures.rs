//! Shared fixtures for attribution Criterion targets.
//!
//! Built once outside `b.iter` so curve construction is not folded into the
//! measured attribution cost.
//!
//! Compiled independently into each bench binary via `#[path]`, so items used
//! by only one target look unused to the other.
#![allow(dead_code)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use finstack_quant_attribution::{
    AttributionEnvelope, AttributionMethod, AttributionSpec, CreditFactorDetailOptions,
    ReturnContributionFactor, ReturnContributionPosition, ReturnContributionSpec,
    ReturnContributionWeighting,
};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::{CurveState, MarketContext, MarketContextState};
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, HazardCurve, InflationCurve,
};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::equity::spot::Equity;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use finstack_quant_valuations::instruments::Instrument;
use std::collections::BTreeMap;
use std::sync::Arc;
use time::Month;

pub const USD_OIS: &str = "USD-OIS";
pub const EUR_OIS: &str = "EUR-OIS";
pub const BASE_RATE: f64 = 0.04;

pub fn calendar_date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
        .expect("valid date")
}

/// Flat continuously-compounded discount curve.
pub fn build_flat_curve(curve_id: &str, as_of: Date, rate: f64) -> DiscountCurve {
    let tenors = [0.0_f64, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 20.0, 30.0];
    let knots: Vec<(f64, f64)> = tenors.iter().map(|&t| (t, (-rate * t).exp())).collect();
    DiscountCurve::builder(curve_id)
        .base_date(as_of)
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

pub fn sample_bond(id: &str, maturity_year_offset: i32) -> Bond {
    let issue = calendar_date(2025, 1, 1);
    let maturity =
        Date::from_calendar_date(2025 + maturity_year_offset, Month::January, 1).unwrap();
    Bond::fixed(
        id,
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        USD_OIS,
    )
    .unwrap()
}

pub fn sample_bond_idx(idx: usize) -> Bond {
    sample_bond(&format!("BENCH-BOND-{idx}"), 1 + (idx % 10) as i32)
}

pub fn sample_eur_bond(id: &str, maturity_year_offset: i32) -> Bond {
    let issue = calendar_date(2025, 1, 1);
    let maturity =
        Date::from_calendar_date(2025 + maturity_year_offset, Month::January, 1).unwrap();
    Bond::fixed(
        id,
        Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        EUR_OIS,
    )
    .unwrap()
}

/// T0/T1 markets for a vanilla USD discount move.
pub struct BondMarkets {
    pub market_t0: MarketContext,
    pub market_t1: MarketContext,
    pub as_of_t0: Date,
    pub as_of_t1: Date,
    pub config: FinstackConfig,
}

impl BondMarkets {
    pub fn new(shift_bp: f64) -> Self {
        let as_of_t0 = calendar_date(2025, 1, 15);
        let as_of_t1 = calendar_date(2025, 1, 16);
        Self {
            market_t0: MarketContext::new().insert(build_flat_curve(USD_OIS, as_of_t0, BASE_RATE)),
            market_t1: MarketContext::new().insert(build_flat_curve(
                USD_OIS,
                as_of_t1,
                BASE_RATE + shift_bp / 10_000.0,
            )),
            as_of_t0,
            as_of_t1,
            config: FinstackConfig::default(),
        }
    }
}

/// Book-shaped market: unused credit / inflation / FX / spot families sit
/// alongside the bond's discount curve so extract/restore cost is visible.
pub fn rich_markets(shift_bp: f64) -> BondMarkets {
    let lean = BondMarkets::new(shift_bp);
    let hazard_t0 = HazardCurve::builder("ACME-HAZ")
        .base_date(lean.as_of_t0)
        .knots([(0.0, 0.0050), (1.0, 0.0055), (5.0, 0.0060)])
        .recovery_rate(0.40)
        .build()
        .unwrap();
    let hazard_t1 = HazardCurve::builder("ACME-HAZ")
        .base_date(lean.as_of_t1)
        .knots([(0.0, 0.0055), (1.0, 0.0060), (5.0, 0.0068)])
        .recovery_rate(0.40)
        .build()
        .unwrap();
    let cpi_t0 = InflationCurve::builder("USD-CPI")
        .base_date(lean.as_of_t0)
        .base_cpi(100.0)
        .knots([(0.0, 100.0), (1.0, 102.0), (5.0, 110.0)])
        .build()
        .unwrap();
    let cpi_t1 = InflationCurve::builder("USD-CPI")
        .base_date(lean.as_of_t1)
        .base_cpi(100.0)
        .knots([(0.0, 100.2), (1.0, 102.4), (5.0, 111.0)])
        .build()
        .unwrap();
    let fx_t0 = FxMatrix::new(Arc::new(StaticFx { eur_usd: 1.10 }));
    let fx_t1 = FxMatrix::new(Arc::new(StaticFx { eur_usd: 1.12 }));

    BondMarkets {
        market_t0: lean
            .market_t0
            .insert(hazard_t0)
            .insert(cpi_t0)
            .insert_fx(fx_t0)
            .insert_price(
                "AAPL-SPOT",
                MarketScalar::Price(Money::new(180.0, Currency::USD).expect("valid money fixture")),
            ),
        market_t1: lean
            .market_t1
            .insert(hazard_t1)
            .insert(cpi_t1)
            .insert_fx(fx_t1)
            .insert_price(
                "AAPL-SPOT",
                MarketScalar::Price(Money::new(185.0, Currency::USD).expect("valid money fixture")),
            ),
        as_of_t0: lean.as_of_t0,
        as_of_t1: lean.as_of_t1,
        config: lean.config,
    }
}

pub struct StaticFx {
    pub eur_usd: f64,
}

impl FxProvider for StaticFx {
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_quant_core::Result<f64> {
        if from == to {
            return Ok(1.0);
        }
        if from == Currency::EUR && to == Currency::USD {
            return Ok(self.eur_usd);
        }
        if from == Currency::USD && to == Currency::EUR {
            return Ok(1.0 / self.eur_usd);
        }
        Ok(1.0)
    }
}

/// EUR bond + EUR-OIS + EUR/USD FX, for translation and FX-factor benches.
pub fn eur_fx_markets(shift_bp: f64) -> BondMarkets {
    let as_of_t0 = calendar_date(2025, 1, 15);
    let as_of_t1 = calendar_date(2025, 1, 16);
    let fx_t0 = FxMatrix::new(Arc::new(StaticFx { eur_usd: 1.10 }));
    let fx_t1 = FxMatrix::new(Arc::new(StaticFx { eur_usd: 1.12 }));
    BondMarkets {
        market_t0: MarketContext::new()
            .insert(build_flat_curve(EUR_OIS, as_of_t0, BASE_RATE))
            .insert_fx(fx_t0),
        market_t1: MarketContext::new()
            .insert(build_flat_curve(
                EUR_OIS,
                as_of_t1,
                BASE_RATE + shift_bp / 10_000.0,
            ))
            .insert_fx(fx_t1),
        as_of_t0,
        as_of_t1,
        config: FinstackConfig::default(),
    }
}

pub fn sample_equity() -> Arc<dyn Instrument> {
    Arc::new(
        Equity::new("AAPL", "AAPL", Currency::USD)
            .with_price_id("AAPL-SPOT")
            .with_shares(100.0),
    )
}

pub fn equity_markets() -> BondMarkets {
    let as_of_t0 = calendar_date(2025, 1, 15);
    let as_of_t1 = calendar_date(2025, 1, 16);
    let disc_t0 = build_flat_curve(USD_OIS, as_of_t0, BASE_RATE);
    let disc_t1 = build_flat_curve(USD_OIS, as_of_t1, BASE_RATE);
    BondMarkets {
        market_t0: MarketContext::new().insert(disc_t0).insert_price(
            "AAPL-SPOT",
            MarketScalar::Price(Money::new(180.0, Currency::USD).expect("valid money fixture")),
        ),
        market_t1: MarketContext::new().insert(disc_t1).insert_price(
            "AAPL-SPOT",
            MarketScalar::Price(Money::new(185.0, Currency::USD).expect("valid money fixture")),
        ),
        as_of_t0,
        as_of_t1,
        config: FinstackConfig::default(),
    }
}

/// `n` distinct flat discount curves in one context — extract/restore scaling.
pub fn multi_curve_market(n: usize) -> (MarketContext, MarketContext, Date) {
    let as_of = calendar_date(2025, 1, 15);
    let mut t0 = MarketContext::new();
    let mut t1 = MarketContext::new();
    for i in 0..n {
        let id = format!("USD-OIS-{i}");
        t0.insert_mut(build_flat_curve(&id, as_of, BASE_RATE));
        t1.insert_mut(build_flat_curve(&id, as_of, BASE_RATE + 0.0001));
    }
    (t0, t1, as_of)
}

pub fn return_contribution_spec(n: usize, brinson: bool) -> ReturnContributionSpec {
    let sectors = ["rates", "credit", "equity", "fx"];
    let regions = ["us", "eu", "em"];
    let positions = (0..n)
        .map(|i| {
            let mut groups = BTreeMap::new();
            groups.insert("sector".to_owned(), sectors[i % sectors.len()].to_owned());
            groups.insert("region".to_owned(), regions[i % regions.len()].to_owned());
            let mv = 1_000.0 + (i as f64) * 10.0;
            ReturnContributionPosition {
                id: format!("POS-{i:04}"),
                market_value: Some(mv),
                weight: None,
                period_return: 0.01 + (i as f64) * 1e-6,
                groups,
                benchmark_weight: brinson.then_some(1.0 / n as f64),
                benchmark_return: brinson.then_some(0.008),
            }
        })
        .collect();
    ReturnContributionSpec {
        as_of: "2025-01-16".to_owned(),
        positions,
        factors: vec![
            ReturnContributionFactor {
                factor: "rates".to_owned(),
                exposure: 0.4,
                factor_return: 0.005,
            },
            ReturnContributionFactor {
                factor: "credit".to_owned(),
                exposure: 0.3,
                factor_return: 0.004,
            },
            ReturnContributionFactor {
                factor: "equity".to_owned(),
                exposure: 0.3,
                factor_return: 0.012,
            },
        ],
        weighting: ReturnContributionWeighting::Gross,
    }
}

pub fn parallel_spec_envelope(shift_bp: f64) -> AttributionEnvelope {
    let markets = BondMarkets::new(shift_bp);
    let bond = sample_bond("BENCH-BOND-SPEC", 5);
    let market_t0 = MarketContextState::from(&markets.market_t0);
    let market_t1 = MarketContextState::from(&markets.market_t1);
    AttributionEnvelope::new(AttributionSpec {
        instrument: InstrumentJson::Bond(bond),
        market_t0,
        market_t1,
        as_of_t0: markets.as_of_t0,
        as_of_t1: markets.as_of_t1,
        method: AttributionMethod::Parallel,
        config: None,
        model_params_t0: None,
        credit_factor_model: None,
        credit_factor_detail_options: CreditFactorDetailOptions::default(),
        full_cross_attribution: false,
    })
}

pub fn market_state(as_of: Date, rate: f64, curve_id: &str) -> MarketContextState {
    MarketContextState {
        schema_version: finstack_quant_core::wire::SchemaVersion::CURRENT,
        curves: vec![CurveState::Discount(build_flat_curve(
            curve_id, as_of, rate,
        ))],
        fx: None,
        surfaces: vec![],
        prices: BTreeMap::new(),
        series: vec![],
        inflation_indices: vec![],
        dividends: vec![],
        credit_indices: vec![],
        collateral: BTreeMap::new(),
        fx_delta_vol_surfaces: vec![],
        hierarchy: None,
        vol_cubes: vec![],
    }
}
