#[allow(clippy::expect_used, clippy::unwrap_used, dead_code, unused_imports)]
mod test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/test_utils.rs"
    ));
}

use crate::instruments::exotics::barrier_option::BarrierType;
use crate::instruments::fx::fx_barrier_option::FxBarrierOption;
use crate::instruments::{Attributes, Instrument, OptionType};
use crate::models::closed_form::barrier::{
    barrier_rebate_continuous, BarrierParams, BarrierType as AnalyticalBarrierType,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use test_utils::{date, flat_discount_with_tenor, flat_vol_surface};

#[test]
fn test_fx_barrier_rebate_added_to_closed_form_price() {
    let as_of = date(2024, 1, 1);
    let expiry = date(2025, 1, 1);
    let spot = 1.10;
    let strike = 1.10;
    let barrier = 1.20;
    let rebate = Money::new(0.02, Currency::USD);

    let dom_curve = flat_discount_with_tenor("USD-OIS", as_of, 0.03, 5.0);
    let for_curve = flat_discount_with_tenor("EUR-OIS", as_of, 0.01, 5.0);
    let expiries = [0.25, 0.5, 1.0, 2.0, 5.0];
    let strikes = [0.9, 1.0, 1.1, 1.2, 1.3];
    let vol_surface = flat_vol_surface("EURUSD-VOL", &expiries, &strikes, 0.15);

    let market = MarketContext::new()
        .insert(dom_curve)
        .insert(for_curve)
        .insert_surface(vol_surface)
        .insert_price("EURUSD-SPOT", MarketScalar::Unitless(spot));

    let base_option = FxBarrierOption::builder()
        .id(InstrumentId::new("FXBAR-REBATE-BASE"))
        .strike(strike)
        .barrier(Money::new(barrier, Currency::USD))
        .option_type(OptionType::Call)
        .barrier_type(BarrierType::UpAndOut)
        .expiry(expiry)
        .notional(Money::new(1_000_000.0, Currency::EUR)) // Notional in foreign currency
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .day_count(DayCount::Act365F)
        .use_gobet_miri(false)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .fx_spot_id("EURUSD-SPOT".into())
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .attributes(Attributes::new())
        .build()
        .expect("Base FX barrier option should build");

    let mut rebate_option = base_option.clone();
    rebate_option.rebate = Some(rebate);

    let base_pv = base_option.value(&market, as_of).expect("Base PV");
    let rebate_pv = rebate_option.value(&market, as_of).expect("Rebate PV");

    let t = DayCount::Act365F
        .year_fraction(as_of, expiry, DayCountContext::default())
        .expect("Year fraction");
    let r_dom = market.get_discount("USD-OIS").expect("USD curve").zero(t);
    let r_for = market.get_discount("EUR-OIS").expect("EUR curve").zero(t);
    let sigma = market
        .get_surface("EURUSD-VOL")
        .expect("FX vol")
        .value_clamped(t, strike);

    let params = BarrierParams::new(spot, barrier, barrier, t, r_dom, r_for, sigma);
    let expected_rebate =
        barrier_rebate_continuous(&params, rebate.amount(), AnalyticalBarrierType::UpAndOut)
            * base_option.notional.amount();

    let rebate_delta = rebate_pv.amount() - base_pv.amount();
    // Allow small tolerance for floating point differences
    assert!(
        (rebate_delta - expected_rebate).abs() < 0.01,
        "Rebate PV mismatch: {} vs {}",
        rebate_delta,
        expected_rebate
    );
}
