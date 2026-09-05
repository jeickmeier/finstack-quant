//! Integration tests for portfolio margin aggregation.
//!
//! Tests quantity scaling, netting-set aggregation and FX conversion through
//! `PortfolioMarginAggregator::calculate`.

mod common;

use common::base_date;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_margin::{Marginable, NettingSetId, SimmSensitivities};
use finstack_quant_portfolio::types::DUMMY_ENTITY_ID;
use finstack_quant_portfolio::{
    Entity, Portfolio, PortfolioMarginAggregator, PortfolioMarginResult, Position, PositionUnit,
};
use finstack_quant_valuations::instruments::{Instrument, MarketDependencies};
use finstack_quant_valuations::pricer::InstrumentType;
use std::any::Any;
use std::sync::Arc;

fn test_date() -> finstack_quant_core::dates::Date {
    base_date()
}

// B-6 fixture: mock marginable instrument reporting UNIT (per-1-notional) SIMM
// sensitivities, unit MTM, and an optional unit clearing-IM exposure base.

#[derive(Clone)]
struct TestMarginableInstrument {
    id: String,
    netting_set_id: NettingSetId,
    attributes: Attributes,
    ir_delta: f64,
    mtm: Money,
    im_exposure_base: Option<Money>,
}

impl TestMarginableInstrument {
    fn new(id: &str, netting_set_id: NettingSetId, ir_delta: f64, mtm: Money) -> Self {
        Self {
            id: id.to_string(),
            netting_set_id,
            attributes: Attributes::default(),
            ir_delta,
            mtm,
            im_exposure_base: None,
        }
    }

    fn with_im_exposure_base(mut self, im_exposure_base: Money) -> Self {
        self.im_exposure_base = Some(im_exposure_base);
        self
    }
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    TestMarginableInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl Instrument for TestMarginableInstrument {
    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> InstrumentType {
        InstrumentType::Irs
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
        _market: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        Ok(self.mtm)
    }

    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        Ok(MarketDependencies::new())
    }

    fn as_marginable(&self) -> Option<&dyn Marginable> {
        Some(self)
    }
}

impl Marginable for TestMarginableInstrument {
    fn id(&self) -> &str {
        &self.id
    }

    fn margin_spec(&self) -> Option<&finstack_quant_margin::OtcMarginSpec> {
        None
    }

    fn netting_set_id(&self) -> Option<NettingSetId> {
        Some(self.netting_set_id.clone())
    }

    fn simm_sensitivities(
        &self,
        _market: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<SimmSensitivities> {
        let mut sensitivities = SimmSensitivities::new(self.mtm.currency());
        sensitivities.add_ir_delta(self.mtm.currency(), "5Y", self.ir_delta);
        Ok(sensitivities)
    }

    fn mtm_for_vm(
        &self,
        _market: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        Ok(self.mtm)
    }

    fn im_exposure_base(
        &self,
        _market: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<Option<Money>> {
        Ok(self.im_exposure_base)
    }
}

/// Build a single-position portfolio around `instrument` and run margin.
fn run_margin(
    instrument: Arc<TestMarginableInstrument>,
    quantities: &[f64],
) -> PortfolioMarginResult {
    let as_of = test_date();
    let mut builder = Portfolio::builder("portfolio")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new(DUMMY_ENTITY_ID));
    for (i, &quantity) in quantities.iter().enumerate() {
        let position_instrument: Arc<dyn Instrument> =
            Arc::<TestMarginableInstrument>::clone(&instrument);
        let position = Position::new(
            format!("pos-{i}"),
            DUMMY_ENTITY_ID,
            instrument.id.clone(),
            position_instrument,
            quantity,
            PositionUnit::Notional(None),
        )
        .expect("position should build");
        builder = builder.position(position);
    }
    let portfolio = builder.build().expect("portfolio should build");
    let mut aggregator =
        PortfolioMarginAggregator::from_portfolio(&portfolio).expect("consistent margin terms");
    let result = aggregator
        .calculate(&portfolio, &MarketContext::new(), as_of)
        .expect("margin run should succeed");
    // MO-16 is expected here: this fixture's netting set carries no CSA, so
    // its VM is the unadjusted gross MTM. That is recorded rather than passed
    // off as a netted call amount. Every other degradation is a real failure.
    let unexpected: Vec<_> = result
        .degraded_positions
        .iter()
        .filter(|(_, message)| !message.starts_with("MO-16:"))
        .collect();
    assert!(
        unexpected.is_empty(),
        "no positions should degrade: {unexpected:?}"
    );
    result
}

// B-6: IM must scale with position quantity exactly as VM does.

#[test]
fn b6_simm_im_scales_with_position_quantity() {
    let instrument = Arc::new(TestMarginableInstrument::new(
        "irs-1",
        NettingSetId::bilateral("BANK", "CSA"),
        20_000.0,
        Money::new(1.0, Currency::USD).expect("valid money fixture"),
    ));

    let unit = run_margin(Arc::clone(&instrument), &[1.0]);
    let scaled = run_margin(instrument, &[10.0]);

    // VM regression: quantity scaling already applied on the VM path.
    assert!(
        (scaled.total_variation_margin.amount() - 10.0 * unit.total_variation_margin.amount())
            .abs()
            < 1e-9,
        "VM(qty=10) should be 10x VM(qty=1): got {} vs {}",
        scaled.total_variation_margin.amount(),
        unit.total_variation_margin.amount()
    );

    // B-6: SIMM IM must scale by the held quantity too.
    assert!(
        unit.total_initial_margin.amount() > 0.0,
        "unit IM must be positive"
    );
    assert!(
        (scaled.total_initial_margin.amount() - 10.0 * unit.total_initial_margin.amount()).abs()
            < 1e-6 * unit.total_initial_margin.amount(),
        "B-6: SIMM IM(qty=10) should be 10x IM(qty=1): got {} vs {}",
        scaled.total_initial_margin.amount(),
        unit.total_initial_margin.amount()
    );
}

#[test]
fn b6_clearing_im_scales_with_position_quantity() {
    let instrument = Arc::new(
        TestMarginableInstrument::new(
            "irs-cleared",
            NettingSetId::cleared("LCH"),
            0.0,
            Money::new(1.0, Currency::USD).expect("valid money fixture"),
        )
        .with_im_exposure_base(Money::new(100.0, Currency::USD).expect("valid money fixture")),
    );

    let unit = run_margin(Arc::clone(&instrument), &[1.0]);
    let short = run_margin(Arc::clone(&instrument), &[-1.0]);
    let offset = run_margin(Arc::clone(&instrument), &[1.0, -1.0]);
    assert_eq!(short.total_initial_margin, unit.total_initial_margin);
    assert!(
        (offset.total_initial_margin.amount() - 2.0 * unit.total_initial_margin.amount()).abs()
            < 1e-9
    );
    assert!(short.by_netting_set.values().all(|set| set.is_approximate));
    let scaled = run_margin(instrument, &[10.0]);

    assert!(
        unit.total_initial_margin.amount() > 0.0,
        "unit IM must be positive"
    );
    assert!(
        (scaled.total_initial_margin.amount() - 10.0 * unit.total_initial_margin.amount()).abs()
            < 1e-6 * unit.total_initial_margin.amount(),
        "B-6: clearing IM(qty=10) should be 10x IM(qty=1): got {} vs {}",
        scaled.total_initial_margin.amount(),
        unit.total_initial_margin.amount()
    );
}

#[test]
fn b6_short_position_nets_simm_sensitivities() {
    // +q and -q of the same instrument in one netting set: SIMM sensitivities
    // are signed and must net to ~0 IM; net signed MTM (VM input) is also 0.
    let instrument = Arc::new(TestMarginableInstrument::new(
        "irs-1",
        NettingSetId::bilateral("BANK", "CSA"),
        20_000.0,
        Money::new(1.0, Currency::USD).expect("valid money fixture"),
    ));

    let result = run_margin(instrument, &[5.0, -5.0]);

    assert!(
        result.total_initial_margin.amount().abs() < 1e-9,
        "B-6: long+short SIMM sensitivities must net to zero IM, got {}",
        result.total_initial_margin.amount()
    );
    assert!(
        result.total_variation_margin.amount().abs() < 1e-9,
        "net signed MTM of offsetting positions should be zero, got {}",
        result.total_variation_margin.amount()
    );
}

// Aggregation through `PortfolioMarginAggregator::calculate`.

/// Build a portfolio with one unit position per instrument and run margin
/// against `market`, asserting nothing degraded beyond the expected MO-16.
fn run_margin_for(
    instruments: &[Arc<TestMarginableInstrument>],
    market: &MarketContext,
) -> PortfolioMarginResult {
    let as_of = test_date();
    let mut builder = Portfolio::builder("portfolio")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new(DUMMY_ENTITY_ID));
    for (i, instrument) in instruments.iter().enumerate() {
        let position_instrument: Arc<dyn Instrument> =
            Arc::<TestMarginableInstrument>::clone(instrument);
        let position = Position::new(
            format!("pos-{i}"),
            DUMMY_ENTITY_ID,
            instrument.id.clone(),
            position_instrument,
            1.0,
            PositionUnit::Units,
        )
        .expect("position should build");
        builder = builder.position(position);
    }
    let portfolio = builder.build().expect("portfolio should build");
    let mut aggregator =
        PortfolioMarginAggregator::from_portfolio(&portfolio).expect("consistent margin terms");
    let result = aggregator
        .calculate(&portfolio, market, as_of)
        .expect("margin run should succeed");
    let unexpected: Vec<_> = result
        .degraded_positions
        .iter()
        .filter(|(_, message)| !message.starts_with("MO-16:"))
        .collect();
    assert!(
        unexpected.is_empty(),
        "no positions should degrade: {unexpected:?}"
    );
    result
}

fn usd_instrument(
    id: &str,
    netting_set_id: NettingSetId,
    mtm: f64,
) -> Arc<TestMarginableInstrument> {
    Arc::new(TestMarginableInstrument::new(
        id,
        netting_set_id,
        20_000.0,
        Money::new(mtm, Currency::USD).expect("valid money fixture"),
    ))
}

#[test]
fn calculate_sums_netting_sets_into_portfolio_totals() {
    let bilateral = NettingSetId::bilateral("BANK_A", "CSA_001");
    let other = NettingSetId::bilateral("BANK_B", "CSA_002");
    let result = run_margin_for(
        &[
            usd_instrument("irs-a", bilateral.clone(), 1_000_000.0),
            usd_instrument("irs-b", other.clone(), 500_000.0),
        ],
        &MarketContext::new(),
    );

    assert_eq!(result.base_currency, Currency::USD);
    assert_eq!(result.by_netting_set.len(), 2);
    assert_eq!(result.total_positions, 2);
    // MO-16 (no CSA) degradations are the only positions without a margin figure.
    assert_eq!(
        result.positions_without_margin,
        result.degraded_positions.len()
    );

    let sets = [
        &result.by_netting_set[&bilateral],
        &result.by_netting_set[&other],
    ];
    let im: f64 = sets.iter().map(|s| s.initial_margin.amount()).sum();
    let vm: f64 = sets.iter().map(|s| s.variation_margin.amount()).sum();
    let total: f64 = sets.iter().map(|s| s.total_margin.amount()).sum();
    assert!(im > 0.0, "SIMM IM must be positive");
    assert!((result.total_initial_margin.amount() - im).abs() < 1e-9);
    assert!((result.total_variation_margin.amount() - vm).abs() < 1e-9);
    assert!((result.total_margin.amount() - total).abs() < 1e-9);
    assert_eq!(result.total_variation_margin.amount(), 1_500_000.0);
}

#[test]
fn calculate_nets_negative_vm_but_only_positive_vm_adds_to_total_margin() {
    let owed = NettingSetId::bilateral("BANK_A", "CSA_001");
    let receivable = NettingSetId::bilateral("BANK_B", "CSA_002");
    let result = run_margin_for(
        &[
            usd_instrument("irs-owed", owed, 300_000.0),
            usd_instrument("irs-receivable", receivable, -500_000.0),
        ],
        &MarketContext::new(),
    );

    // VM nets across netting sets; total margin only adds positive VM.
    assert_eq!(result.total_variation_margin.amount(), -200_000.0);
    assert!(
        (result.total_margin.amount() - (result.total_initial_margin.amount() + 300_000.0)).abs()
            < 1e-9
    );
}

#[test]
fn calculate_converts_foreign_currency_netting_set_to_base() {
    let usd_set = NettingSetId::bilateral("GS", "CSA_USD");
    let eur_set = NettingSetId::bilateral("DB", "CSA_EUR");
    let eur = Arc::new(TestMarginableInstrument::new(
        "irs-eur",
        eur_set.clone(),
        20_000.0,
        Money::new(2_000_000.0, Currency::EUR).expect("valid money fixture"),
    ));
    let usd_only = run_margin_for(
        &[usd_instrument("irs-usd", usd_set.clone(), 500_000.0)],
        &MarketContext::new(),
    );
    let mixed = run_margin_for(
        &[usd_instrument("irs-usd", usd_set.clone(), 500_000.0), eur],
        &common::market_with_eur_and_fx(1.10),
    );

    // Every stored row and every total is in the base currency.
    assert_eq!(mixed.by_netting_set.len(), 2);
    for margin in mixed.by_netting_set.values() {
        assert_eq!(margin.initial_margin.currency(), Currency::USD);
        assert_eq!(margin.variation_margin.currency(), Currency::USD);
        assert_eq!(margin.total_margin.currency(), Currency::USD);
    }
    // VM: 500k USD + 2M EUR * 1.10.
    assert!((mixed.total_variation_margin.amount() - (500_000.0 + 2_200_000.0)).abs() < 1e-6);
    // The EUR set's IM is the USD set's IM scaled by the spot rate (same unit
    // IR delta, sensitivities rebased before the SIMM run).
    let usd_im = usd_only.by_netting_set[&usd_set].initial_margin.amount();
    let eur_im = mixed.by_netting_set[&eur_set].initial_margin.amount();
    assert!(usd_im > 0.0);
    assert!(
        (eur_im - 1.10 * usd_im).abs() < 1e-6 * usd_im,
        "EUR IM {eur_im} should be 1.10x USD IM {usd_im}"
    );
    assert!((mixed.total_initial_margin.amount() - (usd_im + eur_im)).abs() < 1e-6);
}
