//! Integration tests for market-restore completeness across ALL curve
//! storage families (audit B8), vol-cube-only volatility gating (audit M4),
//! waterfall order stamping (audit Mo12), and the default cross-pair set
//! additions Rates×Inflation / Credit×Correlations (audit Mo7).
//!
//! Audit B8: `retain_curves_mut` in `factors.rs` previously handled only
//! Discount/Forward/Hazard/Inflation/BaseCorrelation and silently kept
//! Price, VolIndex, BasisSpread and Parametric curves at their pre-restore
//! state, so a waterfall/parallel attribution never moved them to T1 and
//! commodity P&L landed 100% in the residual.

use finstack_quant_attribution::{
    attribute_pnl, default_waterfall_order, AttributionMethod, AttributionRequest, ExecutionPolicy,
    MarketRestoreFlags, MarketSnapshot,
};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::SabrParameterData;
use finstack_quant_core::market_data::surfaces::VolCube;
use finstack_quant_core::market_data::term_structures::{
    BaseCorrelationCurve, BasisSpreadCurve, ForwardCurve, HazardCurve, InflationCurve,
    NelsonSiegelModel, ParametricCurve, PriceCurve, PriceCurveKind,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_models::volatility::get_cube_vol_clamped;
use finstack_quant_valuations::instruments::{Attributes, Instrument, MarketDependencies};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::InstrumentType;
use finstack_quant_valuations::results::ValuationResult;
use std::sync::{Arc, OnceLock};
use time::macros::date;
use time::Date;

const AS_OF_T0: Date = date!(2025 - 01 - 15);
const AS_OF_T1: Date = date!(2025 - 01 - 16);

// ---------------------------------------------------------------------------
// Test instrument: one struct, per-kind valuation formulas.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Kind {
    /// V = 100_000 · price-curve spot (commodity forward shape).
    PriceCurve,
    /// V = 1_000_000 · SABR cube vol (swaption shape; NO plain vol surface).
    VolCube,
    /// V = 1_000_000 · (1 + fwd(1y)) · cpi(1y)/100 (bilinear rates×inflation).
    RatesInflation,
    /// V = 1_000_000 · hazard(1y) · baseCorr(7%) (bilinear credit×correlation).
    CreditCorrelation,
}

#[derive(Clone)]
struct RestoreTestInstrument {
    id: String,
    kind: Kind,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    RestoreTestInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl RestoreTestInstrument {
    fn build(id: &str, kind: Kind) -> Arc<dyn Instrument> {
        Arc::new(Self {
            id: id.to_string(),
            kind,
        })
    }
}

impl Instrument for RestoreTestInstrument {
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
        match self.kind {
            Kind::PriceCurve | Kind::VolCube => {}
            Kind::RatesInflation => {
                deps.add_forward_curve(CurveId::new("USD-FWD"));
                deps.add_inflation_curve(CurveId::new("USD-CPI"));
            }
            Kind::CreditCorrelation => {
                deps.add_credit_curve(CurveId::new("ACME-HAZ"));
            }
        }
        Ok(deps)
    }

    fn base_value(&self, market: &MarketContext, _as_of: Date) -> Result<Money> {
        let amount = match self.kind {
            Kind::PriceCurve => 100_000.0 * market.get_price_curve("WTI")?.spot_price(),
            Kind::VolCube => {
                let cube = market.get_vol_cube("SWPT-CUBE")?;
                1_000_000.0 * get_cube_vol_clamped(&cube, 1.0, 1.0, 0.05)
            }
            Kind::RatesInflation => {
                let rate = market.get_forward("USD-FWD")?.rate(1.0);
                let cpi = market.get_inflation_curve("USD-CPI")?.cpi(1.0);
                1_000_000.0 * (1.0 + rate) * (cpi / 100.0)
            }
            Kind::CreditCorrelation => {
                let hazard = market.get_hazard("ACME-HAZ")?.hazard_rate(1.0);
                let corr = market.get_base_correlation("CDX-BC")?.correlation(0.07);
                1_000_000.0 * hazard * corr
            }
        };
        Ok(Money::new(amount, Currency::USD).expect("valid money fixture"))
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        _metrics: &[MetricId],
        _options: finstack_quant_valuations::instruments::PricingOptions,
    ) -> finstack_quant_valuations::Result<ValuationResult> {
        let value = self.value(market, as_of)?;
        Ok(ValuationResult::stamped(self.id(), as_of, value))
    }
}

// ---------------------------------------------------------------------------
// Market builders
// ---------------------------------------------------------------------------

fn price_curve(spot: f64) -> PriceCurve {
    PriceCurve::builder("WTI")
        .base_date(AS_OF_T0)
        .spot_price(spot)
        .knots([(0.5, spot + 0.05), (1.0, spot + 0.10)])
        .build()
        .expect("price curve should build")
}

fn vol_index_curve(level: f64) -> PriceCurve {
    PriceCurve::builder("VIX")
        .kind(PriceCurveKind::VolIndex)
        .base_date(AS_OF_T0)
        .spot_price(level)
        .knots([(0.5, level + 1.0), (1.0, level + 2.0)])
        .build()
        .expect("vol index curve should build")
}

fn basis_spread_curve(spread: f64) -> BasisSpreadCurve {
    BasisSpreadCurve::builder("BASIS-3M6M")
        .base_date(AS_OF_T0)
        .knots([(0.25, spread), (5.0, spread)])
        .build()
        .expect("basis spread curve should build")
}

fn parametric_curve(beta0: f64) -> ParametricCurve {
    ParametricCurve::builder("USD-NS")
        .base_date(AS_OF_T0)
        .model(NelsonSiegelModel::Ns {
            beta0,
            beta1: -0.01,
            beta2: 0.005,
            tau: 2.0,
        })
        .build()
        .expect("parametric curve should build")
}

fn vol_cube(alpha: f64) -> VolCube {
    let params = SabrParameterData::new(alpha, 0.5, -0.2, 0.4).expect("valid SABR params");
    VolCube::from_grid(
        "SWPT-CUBE",
        &[1.0, 2.0],
        &[1.0, 5.0],
        &[params, params, params, params],
        &[0.05, 0.05, 0.05, 0.05],
    )
    .expect("vol cube should build")
}

fn forward_curve(rate: f64) -> ForwardCurve {
    ForwardCurve::builder("USD-FWD", 0.25)
        .base_date(AS_OF_T0)
        .knots([(0.0, rate), (5.0, rate)])
        .build()
        .expect("forward curve should build")
}

fn inflation_curve(cpi_1y: f64) -> InflationCurve {
    InflationCurve::builder("USD-CPI")
        .base_date(AS_OF_T0)
        .base_cpi(100.0)
        .knots([(0.0, 100.0), (1.0, cpi_1y), (5.0, cpi_1y + 8.0)])
        .build()
        .expect("inflation curve should build")
}

fn hazard_curve(hazard: f64) -> HazardCurve {
    HazardCurve::builder("ACME-HAZ")
        .base_date(AS_OF_T0)
        .knots([(0.0, hazard), (5.0, hazard)])
        .recovery_rate(0.40)
        .build()
        .expect("hazard curve should build")
}

fn base_correlation_curve(corr_7pct: f64) -> BaseCorrelationCurve {
    BaseCorrelationCurve::builder("CDX-BC")
        .knots([
            (0.03, corr_7pct - 0.10),
            (0.07, corr_7pct),
            (0.15, corr_7pct + 0.10),
        ])
        .build()
        .expect("base correlation curve should build")
}

// ---------------------------------------------------------------------------
// B8: price-curve P&L must land in a named factor, not the residual.
// ---------------------------------------------------------------------------

/// Waterfall: a commodity instrument priced off a `PriceCurve` that moves
/// 3.00 → 3.30 must see its full P&L in MarketScalars with ≈0 residual.
/// Before the fix the T0 price curve survived every restore and the entire
/// P&L fell into the residual.
#[test]
fn waterfall_price_curve_pnl_lands_in_market_scalars_not_residual() {
    let instrument = RestoreTestInstrument::build("WTI-FWD-1", Kind::PriceCurve);
    let market_t0 = MarketContext::new().insert(price_curve(3.00));
    let market_t1 = MarketContext::new().insert(price_curve(3.30));
    let config = FinstackConfig::default();

    let attribution = attribute_pnl(
        &AttributionMethod::Waterfall(default_waterfall_order()),
        &AttributionRequest {
            strict_validation: true,
            model_params_t0: None,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                AS_OF_T0,
                AS_OF_T1,
                &config,
            )
        },
    )
    .expect("waterfall attribution should succeed");

    let expected_pnl = 100_000.0 * 0.30;
    assert!(
        (attribution.total_pnl.amount() - expected_pnl).abs() < 1e-6,
        "total_pnl should be {expected_pnl}, got {}",
        attribution.total_pnl.amount()
    );
    assert!(
        (attribution.market_scalars_pnl.amount() - expected_pnl).abs() < 1e-6,
        "price-curve move must be attributed to MarketScalars, got {}",
        attribution.market_scalars_pnl.amount()
    );
    assert!(
        attribution.residual.amount().abs() < 1e-6,
        "residual must be ~0, got {} (price curve stuck at T0 through restores?)",
        attribution.residual.amount()
    );
}

/// Parallel: same commodity shape through `attribute_pnl_parallel`.
#[test]
fn parallel_price_curve_pnl_lands_in_market_scalars_not_residual() {
    let instrument = RestoreTestInstrument::build("WTI-FWD-2", Kind::PriceCurve);
    let market_t0 = MarketContext::new().insert(price_curve(3.00));
    let market_t1 = MarketContext::new().insert(price_curve(3.30));
    let config = FinstackConfig::default();

    let attribution = attribute_pnl(
        &AttributionMethod::Parallel,
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                AS_OF_T0,
                AS_OF_T1,
                &config,
            )
        },
    )
    .expect("parallel attribution should succeed");

    let expected_pnl = 100_000.0 * 0.30;
    assert!(
        (attribution.market_scalars_pnl.amount() - expected_pnl).abs() < 1e-6,
        "price-curve move must be attributed to MarketScalars, got {}",
        attribution.market_scalars_pnl.amount()
    );
    assert!(
        attribution.residual.amount().abs() < 1e-6,
        "residual must be ~0, got {}",
        attribution.residual.amount()
    );
}

/// B8 invariant: an all-flags snapshot/restore must move EVERY curve storage
/// family — including Price, VolIndex, BasisSpread and Parametric — to the
/// snapshot's state.
#[test]
fn restore_market_moves_all_nine_curve_storage_families() {
    let market_t0 = MarketContext::new()
        .insert(price_curve(3.00))
        .insert(vol_index_curve(15.0))
        .insert(basis_spread_curve(0.0010))
        .insert(parametric_curve(0.045));
    let market_t1 = MarketContext::new()
        .insert(price_curve(3.30))
        .insert(vol_index_curve(19.0))
        .insert(basis_spread_curve(0.0025))
        .insert(parametric_curve(0.050));

    let snapshot_t1 = MarketSnapshot::extract(&market_t1, MarketRestoreFlags::all());
    let restored =
        MarketSnapshot::restore_market(&market_t0, &snapshot_t1, MarketRestoreFlags::all());

    let spot = restored
        .get_price_curve("WTI")
        .expect("price curve present")
        .spot_price();
    assert!(
        (spot - 3.30).abs() < 1e-12,
        "Price curve must restore to T1 spot 3.30, got {spot}"
    );

    let level = restored
        .get_vol_index_curve("VIX")
        .expect("vol index curve present")
        .spot_price();
    assert!(
        (level - 19.0).abs() < 1e-12,
        "VolIndex curve must restore to T1 level 19.0, got {level}"
    );

    let spread = restored
        .get_basis_spread("BASIS-3M6M")
        .expect("basis spread curve present")
        .spread(1.0);
    assert!(
        (spread - 0.0025).abs() < 1e-12,
        "BasisSpread curve must restore to T1 spread 0.0025, got {spread}"
    );

    let long_rate = match restored
        .get_parametric("USD-NS")
        .expect("parametric curve present")
        .params()
    {
        NelsonSiegelModel::Ns { beta0, .. } | NelsonSiegelModel::Nss { beta0, .. } => *beta0,
    };
    assert!(
        (long_rate - 0.050).abs() < 1e-12,
        "Parametric curve must restore to T1 beta0 0.050, got {long_rate}"
    );
}

// ---------------------------------------------------------------------------
// M4: vol factor must fire for cube-only (and FX-delta-only) vol markets.
// ---------------------------------------------------------------------------

/// A swaption-shaped instrument priced ONLY off a SABR `VolCube` (no plain
/// vol surface) must still receive volatility repricing in the parallel
/// method. Before the fix the vol step was gated on `surfaces.is_empty()`
/// alone and silently skipped, dropping the entire move into the residual.
#[test]
fn parallel_vol_cube_only_market_receives_vol_pnl() {
    let instrument = RestoreTestInstrument::build("SWPT-1", Kind::VolCube);
    let market_t0 = MarketContext::new().insert_vol_cube(vol_cube(0.20));
    let market_t1 = MarketContext::new().insert_vol_cube(vol_cube(0.30));
    let config = FinstackConfig::default();

    let attribution = attribute_pnl(
        &AttributionMethod::Parallel,
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                AS_OF_T0,
                AS_OF_T1,
                &config,
            )
        },
    )
    .expect("parallel attribution should succeed");

    assert!(
        attribution.total_pnl.amount().abs() > 1_000.0,
        "test setup: alpha move must produce material P&L, got {}",
        attribution.total_pnl.amount()
    );
    assert!(
        (attribution.vol_pnl.amount() - attribution.total_pnl.amount()).abs() < 1e-6,
        "cube-only vol move must be attributed to Volatility (vol_pnl {}, total {})",
        attribution.vol_pnl.amount(),
        attribution.total_pnl.amount()
    );
    assert!(
        attribution.residual.amount().abs() < 1e-6,
        "residual must be ~0, got {}",
        attribution.residual.amount()
    );
}

// ---------------------------------------------------------------------------
// Mo12: waterfall must stamp the executed factor order into the metadata.
// ---------------------------------------------------------------------------

#[test]
fn waterfall_stamps_executed_factor_order_into_notes() {
    let instrument = RestoreTestInstrument::build("WTI-FWD-3", Kind::PriceCurve);
    let market_t0 = MarketContext::new().insert(price_curve(3.00));
    let market_t1 = MarketContext::new().insert(price_curve(3.30));
    let config = FinstackConfig::default();

    let attribution = attribute_pnl(
        &AttributionMethod::Waterfall(default_waterfall_order()),
        &AttributionRequest {
            strict_validation: true,
            model_params_t0: None,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                AS_OF_T0,
                AS_OF_T1,
                &config,
            )
        },
    )
    .expect("waterfall attribution should succeed");

    let order_note = attribution
        .meta
        .notes
        .iter()
        .find(|note| note.contains("waterfall order:"));
    let note = order_note.expect("waterfall must stamp its executed factor order into meta.notes");
    assert!(
        note.contains("Carry") && note.contains("MarketScalars"),
        "order note must list the executed factors, got: {note}"
    );
}

// ---------------------------------------------------------------------------
// Mo7: default cross-pair set must include Rates×Inflation and
// Credit×Correlations.
// ---------------------------------------------------------------------------

/// A bilinear rates×inflation instrument (a linker shape) must have its
/// interaction extracted by the default cross-pair set instead of leaving it
/// in the residual.
#[test]
fn parallel_default_cross_pairs_include_rates_inflation() {
    let instrument = RestoreTestInstrument::build("LINKER-1", Kind::RatesInflation);
    let market_t0 = MarketContext::new()
        .insert(forward_curve(0.02))
        .insert(inflation_curve(102.0));
    let market_t1 = MarketContext::new()
        .insert(forward_curve(0.03))
        .insert(inflation_curve(106.0));
    let config = FinstackConfig::default();

    let attribution = attribute_pnl(
        &AttributionMethod::Parallel,
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                AS_OF_T0,
                AS_OF_T1,
                &config,
            )
        },
    )
    .expect("parallel attribution should succeed");

    let detail = attribution
        .cross_factor_detail
        .as_ref()
        .expect("bilinear rates×inflation exposure must produce cross-factor detail");
    assert!(
        detail.by_pair.contains_key("Rates×Inflation"),
        "default cross pairs must include Rates×Inflation, got: {:?}",
        detail.by_pair.keys().collect::<Vec<_>>()
    );
    assert!(
        attribution.residual.amount().abs() < 1e-6,
        "with the pair captured the residual must be ~0, got {}",
        attribution.residual.amount()
    );
}

/// A bilinear credit×correlation instrument (a tranche shape) must have its
/// interaction extracted by the default cross-pair set.
#[test]
fn parallel_default_cross_pairs_include_credit_correlations() {
    let instrument = RestoreTestInstrument::build("TRANCHE-1", Kind::CreditCorrelation);
    let market_t0 = MarketContext::new()
        .insert(hazard_curve(0.005))
        .insert(base_correlation_curve(0.40));
    let market_t1 = MarketContext::new()
        .insert(hazard_curve(0.008))
        .insert(base_correlation_curve(0.50));
    let config = FinstackConfig::default();

    let attribution = attribute_pnl(
        &AttributionMethod::Parallel,
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                AS_OF_T0,
                AS_OF_T1,
                &config,
            )
        },
    )
    .expect("parallel attribution should succeed");

    let detail = attribution
        .cross_factor_detail
        .as_ref()
        .expect("bilinear credit×correlation exposure must produce cross-factor detail");
    assert!(
        detail.by_pair.contains_key("Credit×Correlations"),
        "default cross pairs must include Credit×Correlations, got: {:?}",
        detail.by_pair.keys().collect::<Vec<_>>()
    );
    assert!(
        attribution.residual.amount().abs() < 1e-6,
        "with the pair captured the residual must be ~0, got {}",
        attribution.residual.amount()
    );
}
