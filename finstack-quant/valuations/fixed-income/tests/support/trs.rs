//! Builder for fixed-income index TRS test fixtures (also `#[path]`-included
//! by the umbrella crate's cross-family TRS tests).

use super::trs_market::{as_of_date, d};
use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::{
    currency::Currency,
    dates::{Date, DayCount},
    money::Money,
};
use finstack_quant_valuations_core::instruments::{FinancingLegSpec, PayReceive, TrsScheduleSpec};
use finstack_quant_valuations_fixed_income::IndexUnderlyingParams;
use finstack_quant_valuations_fixed_income::fi_trs::FiIndexTotalReturnSwap;
use rust_decimal::Decimal;

/// Builder for test FI index TRS with sensible defaults.
pub struct TestFIIndexTrsBuilder {
    id: String,
    notional: Money,
    index_id: String,
    yield_id: Option<String>,
    duration_id: Option<String>,
    discount_curve_id: String,
    forward_curve_id: String,
    spread_bp: f64,
    start: Date,
    end: Date,
    side: PayReceive,
    initial_level: Option<f64>,
}

impl Default for TestFIIndexTrsBuilder {
    fn default() -> Self {
        Self {
            id: "TEST-FI-TRS-001".into(),
            notional: Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
            index_id: "HY-INDEX".into(),
            yield_id: Some("HY-INDEX-YIELD".into()),
            duration_id: Some("HY-INDEX-DURATION".into()),
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: 100.0,
            // Keep `start` strictly after `as_of` so theta (which rolls the
            // valuation date forward) doesn't cross the start date.
            start: as_of_date() + time::Duration::days(2),
            end: d(2026, 1, 2),
            side: PayReceive::Receive,
            initial_level: None,
        }
    }
}

impl TestFIIndexTrsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn notional(mut self, notional: Money) -> Self {
        self.notional = notional;
        self
    }

    pub fn spread_bp(mut self, spread: f64) -> Self {
        self.spread_bp = spread;
        self
    }

    pub fn tenor_months(mut self, months: i32) -> Self {
        self.end = as_of_date() + time::Duration::days((months * 30) as i64);
        self
    }

    pub fn side(mut self, side: PayReceive) -> Self {
        self.side = side;
        self
    }

    pub fn yield_id(mut self, id: Option<String>) -> Self {
        self.yield_id = id;
        self
    }

    pub fn duration_id(mut self, id: Option<String>) -> Self {
        self.duration_id = id;
        self
    }

    pub fn build(self) -> FiIndexTotalReturnSwap {
        let mut underlying =
            IndexUnderlyingParams::new(self.index_id.clone(), self.notional.currency());

        if let Some(y_id) = self.yield_id {
            underlying = underlying.with_yield_id(y_id);
        }
        if let Some(d_id) = self.duration_id {
            underlying = underlying.with_duration_id(d_id);
        }

        let financing = FinancingLegSpec::new(
            self.discount_curve_id,
            self.forward_curve_id,
            Decimal::try_from(self.spread_bp).expect("valid spread_bp"),
            DayCount::Act360,
        );

        let schedule =
            TrsScheduleSpec::from_params(self.start, self.end, ScheduleParams::quarterly_act360());

        let mut builder = FiIndexTotalReturnSwap::builder()
            .id(self.id.into())
            .notional(self.notional)
            .underlying(underlying)
            .financing_leg(financing)
            .schedule(schedule)
            .side(self.side);

        if let Some(level) = self.initial_level {
            builder = builder.initial_level(level);
        }

        builder.build().unwrap()
    }
}
