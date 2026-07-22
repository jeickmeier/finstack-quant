//! PV ↔ reported-cashflow reconciliation for `XccySwap`.
//!
//! `base_value` and `raw_cashflow_schedule` project floating coupons through
//! two different code paths (direct accrual-window projection vs the generic
//! `FloatingCouponSpec` builder). These tests pin them together: discounting
//! the instrument's own reported coupon flows on each leg's discount curve and
//! converting at the valuation-date spot must reproduce the priced PV. Any
//! silent projection drift between the two paths (spread handling, day count,
//! payment dating, reset-lag treatment) breaks this reconciliation.

use finstack_quant_cashflows::CashflowScheduleSource;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_valuations::instruments::rates::xccy_swap::{NotionalExchange, XccySwap};

use super::fixtures::*;

const EURUSD_SPOT: f64 = 1.10;

/// Discount the schedule's coupon (`FloatReset`) flows per currency on that
/// currency's discount curve and convert to USD at the valuation-date spot.
fn coupon_pv_from_schedule(
    swap: &XccySwap,
    market: &finstack_quant_core::market_data::context::MarketContext,
    as_of: finstack_quant_core::dates::Date,
) -> f64 {
    let schedule = swap
        .raw_cashflow_schedule(market, as_of)
        .expect("schedule builds");
    let usd_disc = market.get_discount("USD-OIS").expect("usd curve");
    let eur_disc = market.get_discount("EUR-OIS").expect("eur curve");

    let mut pv_usd = 0.0;
    for flow in schedule.get_flows() {
        if flow.kind != CFKind::FloatReset || flow.date <= as_of {
            continue;
        }
        match flow.amount.currency() {
            Currency::USD => {
                let df = usd_disc.df_between_dates(as_of, flow.date).expect("usd df");
                pv_usd += flow.amount.amount() * df;
            }
            Currency::EUR => {
                let df = eur_disc.df_between_dates(as_of, flow.date).expect("eur df");
                pv_usd += flow.amount.amount() * df * EURUSD_SPOT;
            }
            other => panic!("unexpected flow currency {other}"),
        }
    }
    pv_usd
}

/// Unlagged swap with a nonzero spread on the pay leg: the priced PV
/// (no notional exchange, so coupons only) must equal the PV of the
/// instrument's own reported coupon schedule.
#[test]
fn coupon_schedule_reconciles_with_priced_pv_unlagged() {
    let base = d(2025, 1, 2);
    let maturity = d(2027, 1, 2);

    let mut eur_leg = leg_eur_pay(base, maturity);
    eur_leg.spread_bp = rust_decimal::Decimal::new(25, 0); // 25 bp

    let swap = XccySwap::new(
        "XCCY-RECON",
        leg_usd_receive(base, maturity),
        eur_leg,
        Currency::USD,
    )
    .with_notional_exchange(NotionalExchange::None);

    let market = market_with_fx();
    let priced = swap.value(&market, base).expect("prices").amount();
    let from_schedule = coupon_pv_from_schedule(&swap, &market, base);

    assert!(
        (priced - from_schedule).abs() < 1e-6 * priced.abs().max(1.0),
        "priced PV and reported-schedule PV must agree (unlagged): \
         priced={priced}, schedule={from_schedule}"
    );
}

/// Same reconciliation with a reset lag on both legs. The 2-day lag pushes
/// the first period's fixing date before the valuation date, so BOTH paths
/// must consume the same recorded fixing (deliberately off-curve at 4% vs the
/// 2% forward: a path that projected the curve instead of using the fixing
/// misses by ~$5k and fails loudly), while every later period projects the
/// window-consistent forward.
#[test]
fn coupon_schedule_reconciles_with_priced_pv_with_reset_lag_and_fixing() {
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;

    let base = d(2025, 1, 2);
    let maturity = d(2027, 1, 2);

    let mut usd_leg = leg_usd_receive(base, maturity);
    usd_leg.reset_lag_days = Some(2);
    let mut eur_leg = leg_eur_pay(base, maturity);
    eur_leg.reset_lag_days = Some(2);
    eur_leg.spread_bp = rust_decimal::Decimal::new(25, 0);

    let swap = XccySwap::new("XCCY-RECON-LAG", usd_leg, eur_leg, Currency::USD)
        .with_notional_exchange(NotionalExchange::None);

    // Off-curve fixings for the first period's lagged reset (2024-12-30 both
    // calendars): 4% USD / 3% EUR against 2% / 1.5% curve forwards.
    let usd_fixing =
        ScalarTimeSeries::new("FIXING:USD-SOFR-3M", vec![(d(2024, 12, 30), 0.04)], None)
            .expect("usd fixing series");
    let eur_fixing =
        ScalarTimeSeries::new("FIXING:EUR-EURIBOR-3M", vec![(d(2024, 12, 30), 0.03)], None)
            .expect("eur fixing series");
    let market = market_with_fx()
        .insert_series(usd_fixing)
        .insert_series(eur_fixing);

    let priced = swap.value(&market, base).expect("prices").amount();
    let from_schedule = coupon_pv_from_schedule(&swap, &market, base);

    assert!(
        (priced - from_schedule).abs() < 1e-6 * priced.abs().max(1.0),
        "priced PV and reported-schedule PV must agree with a lagged, seasoned \
         first fixing: priced={priced}, schedule={from_schedule}"
    );
}
