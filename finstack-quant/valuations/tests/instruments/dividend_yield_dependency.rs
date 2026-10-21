//! Dividend-yield identifiers are market scalars, not time series.

use finstack_quant_core::types::PriceId;
use finstack_quant_valuations::instruments::equity::autocallable::Autocallable;
use finstack_quant_valuations::instruments::equity::cliquet_option::CliquetOption;
use finstack_quant_valuations::instruments::exotics::asian_option::AsianOption;
use finstack_quant_valuations::instruments::exotics::barrier_option::BarrierOption;
use finstack_quant_valuations::instruments::exotics::lookback_option::LookbackOption;
use finstack_quant_valuations::instruments::Instrument;

fn assert_dividend_yield_is_market_scalar<T: Instrument>(
    instrument: &T,
    dividend_id: &PriceId,
    label: &str,
) {
    let deps = instrument
        .market_dependencies()
        .unwrap_or_else(|err| panic!("{label}: market dependencies: {err}"));
    assert!(
        deps.market_scalar_ids
            .contains(&dividend_id.as_str().to_string()),
        "{label}: dividend yield must appear in market_scalar_ids"
    );
    assert!(
        deps.series_ids.is_empty(),
        "{label}: dividend yield must not be recorded as a series"
    );
}

#[test]
fn dividend_yield_dependency_is_a_market_scalar() {
    let dividend_id = PriceId::new("SPX-DIV");

    let mut asian = AsianOption::example().expect("asian example");
    asian.div_yield_id = Some(dividend_id.clone());
    assert_dividend_yield_is_market_scalar(&asian, &dividend_id, "asian");

    let mut barrier = BarrierOption::example().expect("barrier example");
    barrier.div_yield_id = Some(dividend_id.clone());
    assert_dividend_yield_is_market_scalar(&barrier, &dividend_id, "barrier");

    let mut lookback = LookbackOption::example().expect("lookback example");
    lookback.div_yield_id = Some(dividend_id.clone());
    assert_dividend_yield_is_market_scalar(&lookback, &dividend_id, "lookback");

    let mut cliquet = CliquetOption::example().expect("cliquet example");
    cliquet.div_yield_id = Some(dividend_id.clone());
    assert_dividend_yield_is_market_scalar(&cliquet, &dividend_id, "cliquet");

    let mut autocall = Autocallable::example().expect("autocallable example");
    autocall.div_yield_id = Some(dividend_id.clone());
    assert_dividend_yield_is_market_scalar(&autocall, &dividend_id, "autocallable");
}
