//! Spread duration tests for bonds.
//!
//! Bond spread duration is always the quote-reproducing Z-spread sensitivity,
//! even when canonical CS01 uses a hazard/par-CDS curve. These tests pin that
//! basis and the no-credit fallback.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::constants::ONE_BASIS_POINT;
use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_z_spread;
use finstack_quant_valuations::instruments::fixed_income::bond::{
    Bond, BondSettlementConvention, CallPut, CallPutSchedule, CashflowSpec,
};
use finstack_quant_valuations::instruments::{Instrument, InstrumentPricingOverrides};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

/// Build a 5Y fixed-rate bullet corporate bond quoted at 96.5.
fn bullet_5y(credit: bool) -> (Bond, MarketContext) {
    let as_of = date!(2025 - 01 - 06);
    let mut bond = Bond::fixed(
        "SD-BULLET",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        date!(2030 - 01 - 06),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond builds");
    bond.cashflow_spec = CashflowSpec::fixed_rate(
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        Tenor::annual(),
        DayCount::Act365F,
    )
    .expect("finite coupon");
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(96.5);

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (7.0, 0.80)])
        .build()
        .expect("curve builds");
    let mut market = MarketContext::new().insert(disc);

    if credit {
        bond.credit_curve_id = Some(CurveId::new("USD-CREDIT"));
        let hazard = crate::test_support::credit::calibrated_hazard_curve(
            &market,
            as_of,
            "USD-CREDIT",
            "USD-CREDIT-ENTITY",
            "USD-OIS",
        )
        .expect("hazard calibration succeeds");
        market = market.insert(hazard);
    }
    (bond, market)
}

struct SpreadMetrics {
    duration_mod: f64,
    spread_duration: f64,
    z_spread_duration: f64,
    cs01_duration: f64,
}

fn priced(credit: bool) -> SpreadMetrics {
    let (bond, market) = bullet_5y(credit);
    let as_of = date!(2025 - 01 - 06);
    let credit_risk_metric = MetricId::Cs01;
    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::DurationMod,
                MetricId::ZSpread,
                credit_risk_metric.clone(),
                MetricId::SpreadDuration,
            ],
            crate::test_support::credit::pricing_options(),
        )
        .expect("bond must emit spread_duration");
    let duration_mod = *result
        .measures
        .get("duration_mod")
        .expect("duration_mod measure");
    let spread_duration = *result
        .measures
        .get("spread_duration")
        .expect("spread_duration measure");
    let z_spread = *result.measures.get("z_spread").expect("z_spread measure");
    let cs01 = *result
        .measures
        .get(credit_risk_metric.as_str())
        .expect("credit risk measure");
    let base_pv =
        price_from_z_spread(&bond, &market, as_of, z_spread).expect("base z-spread reprice");
    let bumped_pv = price_from_z_spread(&bond, &market, as_of, z_spread + ONE_BASIS_POINT)
        .expect("bumped z-spread reprice");

    SpreadMetrics {
        duration_mod,
        spread_duration,
        z_spread_duration: -(bumped_pv - base_pv) / (base_pv * ONE_BASIS_POINT),
        cs01_duration: -cs01 / (base_pv * ONE_BASIS_POINT),
    }
}

/// A fixed-rate bullet with no credit curve takes the z-spread CS01 fallback,
/// so spread duration must reproduce modified duration (both measure the same
/// yield-basis sensitivity) — and must be **positive**, confirming the bond
/// CS01 path shares the structured-credit sign convention.
#[test]
fn fixed_rate_bullet_spread_duration_matches_modified_duration() {
    let metrics = priced(false);

    assert!(
        metrics.spread_duration > 0.0,
        "spread_duration must be positive for a long bond, got {}",
        metrics.spread_duration
    );
    assert!(
        (3.0..6.0).contains(&metrics.spread_duration),
        "a 5Y bullet's spread duration must be a sane number of years, got {}",
        metrics.spread_duration
    );

    let z_basis_error = (metrics.spread_duration - metrics.z_spread_duration).abs();
    assert!(
        z_basis_error < 1e-10,
        "no-credit spread duration must equal the direct Z-spread bump: \
         metric={}, expected={}, error={z_basis_error:.3e}",
        metrics.spread_duration,
        metrics.z_spread_duration
    );

    let rel = (metrics.spread_duration - metrics.duration_mod).abs() / metrics.duration_mod;
    assert!(
        rel < 0.01,
        "spread_duration ({}) must track duration_mod ({}) for a fixed-rate bullet; \
         relative difference {rel:.3e}",
        metrics.spread_duration,
        metrics.duration_mod
    );
}

/// A hazard curve changes canonical CS01 to the par-CDS bump basis, but must
/// not change spread duration away from the quote-reproducing Z-spread basis.
#[test]
fn credit_curve_bond_spread_duration_stays_on_z_spread_basis() {
    let metrics = priced(true);

    assert!(
        metrics.spread_duration > 0.0,
        "spread_duration must be positive for a long bond, got {}",
        metrics.spread_duration
    );
    assert!(
        (metrics.spread_duration - metrics.z_spread_duration).abs() < 1e-10,
        "hazard-curve bond spread duration must equal the direct Z-spread bump: \
         metric={}, z_basis={}",
        metrics.spread_duration,
        metrics.z_spread_duration
    );
    assert!(
        (metrics.spread_duration - metrics.cs01_duration).abs() > 1e-3,
        "fixture must distinguish Z-spread duration ({}) from par-CDS CS01 duration ({})",
        metrics.spread_duration,
        metrics.cs01_duration
    );
}

#[test]
fn callable_bond_spread_duration_uses_quote_reproducing_workout_path() {
    use finstack_quant_cashflows::accrual::accrued_interest_amount;
    use finstack_quant_cashflows::traits::CashflowProvider;

    let as_of = date!(2025 - 01 - 06);
    let quote_date = date!(2025 - 01 - 09);
    let call_date = date!(2027 - 01 - 06);
    let notional = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
    let clean_quote = 105.0;
    let mut bond = Bond::fixed(
        "SD-CALLABLE",
        notional,
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        date!(2023 - 01 - 06),
        date!(2032 - 01 - 06),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond builds");
    bond.cashflow_spec = CashflowSpec::fixed_rate(
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        Tenor::annual(),
        DayCount::Act365F,
    )
    .expect("finite coupon");
    bond.settlement_convention = Some(BondSettlementConvention {
        settlement_days: 3,
        ..Default::default()
    });
    bond.instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.01);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: call_date,
            end_date: call_date,
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(clean_quote);

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (10.0, 0.65)])
        .build()
        .expect("curve builds");
    let market = MarketContext::new().insert(disc);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ZSpread, MetricId::SpreadDuration],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("callable spread metrics");
    let z_spread = result.measures["z_spread"];
    let spread_duration = result.measures["spread_duration"];

    // Independently construct the known worst (call) path. This deliberately
    // does not call price_from_z_spread or the production workout helper.
    let schedule = bond
        .cashflow_schedule(&market, as_of)
        .expect("cashflow schedule");
    let accrued_at_quote = accrued_interest_amount(&schedule, quote_date, &bond.accrual_config())
        .expect("settlement accrued");
    let dirty_quote = clean_quote * notional.amount() / 100.0 + accrued_at_quote;
    let accrued_at_call = accrued_interest_amount(&schedule, call_date, &bond.accrual_config())
        .expect("call-date accrued");
    let mut workout_flows: Vec<(Date, Money)> = bond
        .dated_cashflows(&market, as_of)
        .expect("contractual flows")
        .into_iter()
        .filter(|(date, _)| *date > quote_date && *date <= call_date)
        .collect();
    workout_flows.push((
        call_date,
        Money::new(notional.amount() + accrued_at_call, notional.currency())
            .expect("valid money fixture"),
    ));

    let manual_pv = |spread: f64| -> f64 {
        workout_flows
            .iter()
            .map(|(date, amount)| {
                let t = DayCount::Act365F
                    .year_fraction(quote_date, *date, DayCountContext::default())
                    .expect("year fraction");
                let df = market
                    .get_discount("USD-OIS")
                    .expect("discount curve")
                    .df_between_dates(quote_date, *date)
                    .expect("discount factor");
                let base_rate = df.powf(-1.0 / t) - 1.0;
                let shifted_df = (1.0 + base_rate + spread).powf(-t);
                amount.amount() * shifted_df
            })
            .sum()
    };
    let base_pv = manual_pv(z_spread);
    let bumped_pv = manual_pv(z_spread + ONE_BASIS_POINT);
    let expected_duration = -(bumped_pv - base_pv) / (base_pv * ONE_BASIS_POINT);

    assert!(
        (base_pv - dirty_quote).abs() / notional.amount() < 1e-7,
        "independent workout-path PV must reproduce the dirty quote: \
         base={base_pv}, quote={dirty_quote}"
    );
    assert!(
        (spread_duration - expected_duration).abs() < 1e-10,
        "spread duration must bump the same quote-reproducing workout path: \
         metric={spread_duration}, expected={expected_duration}"
    );
}
