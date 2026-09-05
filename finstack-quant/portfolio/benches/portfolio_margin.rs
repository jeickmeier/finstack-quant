//! Portfolio-layer SIMM aggregation over netting sets.
//!
//! Sensitivity extraction is mocked so the timed path is the aggregator's
//! Rayon fan-out, merge, and per-set SIMM rollup — the work that lives in
//! this crate rather than instrument pricing.

use std::any::Any;
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_margin::{Marginable, NettingSetId, SimmSensitivities};
use finstack_quant_portfolio::types::DUMMY_ENTITY_ID;
use finstack_quant_portfolio::{
    Entity, Portfolio, PortfolioMarginAggregator, Position, PositionUnit,
};
use finstack_quant_valuations::instruments::{Instrument, MarketDependencies};
use finstack_quant_valuations::pricer::InstrumentType;
use time::macros::date;

#[derive(Clone)]
struct BenchMarginableInstrument {
    id: String,
    netting_set_id: NettingSetId,
    attributes: Attributes,
    ir_delta: f64,
    mtm: Money,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    BenchMarginableInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl Instrument for BenchMarginableInstrument {
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

impl Marginable for BenchMarginableInstrument {
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
        Ok(None)
    }
}

fn margin_portfolio(n_positions: usize, n_netting_sets: usize, as_of: Date) -> Portfolio {
    let mut builder = Portfolio::builder("MARGIN_BENCH")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new(DUMMY_ENTITY_ID));

    for index in 0..n_positions {
        let netting_set = NettingSetId::bilateral(
            format!("CP_{}", index % n_netting_sets),
            format!("CSA_{}", index % n_netting_sets),
        );
        let instrument = BenchMarginableInstrument {
            id: format!("MGN_{index}"),
            netting_set_id: netting_set,
            attributes: Attributes::default(),
            ir_delta: 1_000.0 + index as f64,
            mtm: Money::new(100_000.0 + index as f64, Currency::USD).expect("valid money fixture"),
        };
        let position = Position::new(
            format!("POS_{index}"),
            DUMMY_ENTITY_ID,
            instrument.id.clone(),
            Arc::new(instrument),
            1.0,
            PositionUnit::Notional(None),
        )
        .expect("bench: margin position");
        builder = builder.position(position);
    }
    builder.build().expect("bench: margin portfolio")
}

fn bench_margin_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_margin_aggregation");
    group.sample_size(10);
    let as_of = date!(2025 - 01 - 01);
    let market = MarketContext::new();

    for &n_positions in &[256_usize, 1_024] {
        let portfolio = margin_portfolio(n_positions, 4, as_of);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{n_positions}pos_x_4sets")),
            &n_positions,
            |b, _| {
                b.iter(|| {
                    let mut aggregator = PortfolioMarginAggregator::from_portfolio(&portfolio);
                    aggregator
                        .calculate(
                            std::hint::black_box(&portfolio),
                            std::hint::black_box(&market),
                            as_of,
                        )
                        .expect("bench: margin calculate")
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_margin_aggregation);
criterion_main!(benches);
