//! Regression coverage for using margin calculators through `Marginable` trait objects.

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::Result;
use finstack_margin::metrics::{InitialMarginMetric, TotalMarginMetric, VariationMarginMetric};
use finstack_margin::{ImCalculator, Marginable, SimmCalculator, SimmSensitivities, SimmVersion};

#[derive(Clone)]
struct StandaloneMarginable {
    id: String,
    mtm: Money,
    sensitivities: SimmSensitivities,
}

impl StandaloneMarginable {
    fn new(mtm: Money, sensitivities: SimmSensitivities) -> Self {
        Self {
            id: "MARGINABLE-ONLY".to_string(),
            mtm,
            sensitivities,
        }
    }
}

impl Marginable for StandaloneMarginable {
    fn id(&self) -> &str {
        &self.id
    }

    fn margin_spec(&self) -> Option<&finstack_margin::OtcMarginSpec> {
        None
    }

    fn netting_set_id(&self) -> Option<finstack_margin::NettingSetId> {
        None
    }

    fn simm_sensitivities(
        &self,
        _market: &MarketContext,
        _as_of: Date,
    ) -> Result<SimmSensitivities> {
        Ok(self.sensitivities.clone())
    }

    fn mtm_for_vm(&self, _market: &MarketContext, _as_of: Date) -> Result<Money> {
        Ok(self.mtm)
    }
}

#[test]
fn simm_calculator_accepts_standalone_marginable_trait_objects() {
    let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry should load");
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).expect("valid date");
    let market = MarketContext::new();

    let mut sensitivities = SimmSensitivities::new(Currency::USD);
    sensitivities.add_ir_delta(Currency::USD, "5y", 50_000.0);
    sensitivities.add_equity_delta("AAPL", 100_000.0);

    let instrument = StandaloneMarginable::new(
        Money::new(1_000_000.0, Currency::USD),
        sensitivities.clone(),
    );

    let expected = calc.calculate_from_sensitivities(&sensitivities, Currency::USD);
    let actual = calc
        .calculate(&instrument, &market, as_of)
        .expect("SIMM calculation should succeed");

    assert!((actual.amount.amount() - expected.0).abs() < 1e-2);
    assert!(actual.breakdown.contains_key("IR_Delta"));
    assert!(actual.breakdown.contains_key("Equity_Delta"));
}

#[test]
fn margin_metrics_accept_standalone_marginable_trait_objects() {
    let as_of = Date::from_calendar_date(2024, time::Month::January, 1).expect("valid date");
    let market = MarketContext::new();

    let mut sensitivities = SimmSensitivities::new(Currency::USD);
    sensitivities.add_ir_delta(Currency::USD, "5y", 50_000.0);

    let instrument =
        StandaloneMarginable::new(Money::new(1_000_000.0, Currency::USD), sensitivities);
    let trade: &dyn Marginable = &instrument;

    let im = InitialMarginMetric::new()
        .calculate(trade, &market, as_of)
        .expect("IM metric should support trait objects");
    let vm = VariationMarginMetric::new()
        .calculate(trade, &market, as_of)
        .expect("VM metric should support trait objects");
    let total = TotalMarginMetric::new()
        .calculate(trade, &market, as_of)
        .expect("total margin metric should support trait objects");

    assert_eq!(im.amount.amount(), 0.0);
    assert_eq!(vm.delivery_amount.amount(), 1_000_000.0);
    assert_eq!(total.total_margin.amount(), 1_000_000.0);
}
