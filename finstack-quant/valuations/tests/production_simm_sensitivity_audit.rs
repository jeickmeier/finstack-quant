//! SIMM sign and percentage-unit adapters.

use finstack_quant_core::{currency::Currency, market_data::context::MarketContext, money::Money};
use finstack_quant_margin::Marginable;
use finstack_quant_valuations::instruments::{
    EquityTotalReturnSwap, FIIndexTotalReturnSwap, InterestRateSwap, TrsSide,
};

#[test]
fn equity_delta_is_one_percent_of_the_directional_trade_exposure() {
    let mut instrument = EquityTotalReturnSwap::example().expect("equity TRS");
    instrument.notional = Money::from((1_000_000_i64, Currency::USD));
    instrument.side = TrsSide::ReceiveTotalReturn;
    let as_of = instrument.schedule.start;
    let market = MarketContext::new();
    let long = instrument
        .simm_sensitivities(&market, as_of)
        .expect("long sensitivity");
    assert_eq!(long.total_equity_delta(), 10_000.0);
    instrument.side = TrsSide::PayTotalReturn;
    let short = instrument
        .simm_sensitivities(&market, as_of)
        .expect("short sensitivity");
    assert_eq!(short.total_equity_delta(), -10_000.0);
}

#[test]
fn payer_fixed_rate_proxy_has_positive_rate_sensitivity() {
    let mut swap = InterestRateSwap::example_standard().expect("IRS");
    swap.side = finstack_quant_valuations::instruments::rates::irs::PayReceive::Pay;
    let payer = swap
        .simm_sensitivities(&MarketContext::new(), swap.fixed.start)
        .expect("payer");
    assert!(payer.total_ir_delta() > 0.0);
    swap.side = finstack_quant_valuations::instruments::rates::irs::PayReceive::Receive;
    let receiver = swap
        .simm_sensitivities(&MarketContext::new(), swap.fixed.start)
        .expect("receiver");
    assert_eq!(receiver.total_ir_delta(), -payer.total_ir_delta());
}

#[test]
fn fi_index_margin_requires_the_same_duration_as_its_risk_metric() {
    let mut instrument = FIIndexTotalReturnSwap::example().expect("FI TRS");
    instrument.underlying.duration_id = None;
    assert!(instrument
        .simm_sensitivities(&MarketContext::new(), instrument.schedule.start)
        .is_err());
}
