//! Builder for equity TRS test fixtures (also `#[path]`-included by the
//! umbrella crate's cross-family TRS tests).

use super::trs_market::{as_of_date, d};
use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::{
    currency::Currency,
    dates::{Date, DayCount},
    money::Money,
    types::PriceId,
};
use finstack_quant_valuations_core::instruments::{FinancingLegSpec, PayReceive, TrsScheduleSpec};
use finstack_quant_valuations_equity::EquityUnderlyingParams;
use finstack_quant_valuations_equity::equity_trs::{EquityTotalReturnSwap, TrsDividendSettlement};
use rust_decimal::Decimal;

/// Builder for test equity TRS with sensible defaults.
pub struct TestEquityTrsBuilder {
    id: String,
    notional: Money,
    spot_id: String,
    div_yield_id: Option<PriceId>,
    discount_curve_id: String,
    forward_curve_id: String,
    spread_bp: f64,
    start: Date,
    end: Date,
    side: PayReceive,
    initial_level: Option<f64>,
}

impl Default for TestEquityTrsBuilder {
    fn default() -> Self {
        Self {
            id: "TEST-EQ-TRS-001".into(),
            notional: Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
            spot_id: "SPX-SPOT".into(),
            div_yield_id: Some(finstack_quant_core::types::PriceId::new("SPX-DIV-YIELD")),
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: 25.0,
            // Keep `start` strictly after `as_of` so theta (which rolls the
            // valuation date forward) doesn't cross the start date.
            start: as_of_date() + time::Duration::days(2),
            end: d(2026, 1, 2),
            side: PayReceive::Receive,
            initial_level: None,
        }
    }
}

impl TestEquityTrsBuilder {
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

    pub fn div_yield_id(mut self, id: Option<PriceId>) -> Self {
        self.div_yield_id = id;
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

    pub fn initial_level(mut self, level: f64) -> Self {
        self.initial_level = Some(level);
        self
    }

    pub fn build(self) -> EquityTotalReturnSwap {
        let mut underlying =
            EquityUnderlyingParams::new("TEST-EQ", self.spot_id, self.notional.currency());
        if let Some(div_id) = self.div_yield_id {
            underlying = underlying.with_div_yield_id(div_id);
        }

        let financing = FinancingLegSpec::new(
            self.discount_curve_id,
            self.forward_curve_id,
            Decimal::try_from(self.spread_bp).expect("valid spread_bp"),
            DayCount::Act360,
        );

        let schedule =
            TrsScheduleSpec::from_params(self.start, self.end, ScheduleParams::quarterly_act360());

        let mut builder = EquityTotalReturnSwap::builder()
            .id(self.id.into())
            .notional(self.notional)
            .underlying(underlying)
            .financing_leg(financing)
            .schedule(schedule)
            .side(self.side)
            .dividend_settlement(TrsDividendSettlement::OnDividendDate);

        if let Some(level) = self.initial_level {
            builder = builder.initial_level(level);
        }

        builder.build().unwrap()
    }
}
