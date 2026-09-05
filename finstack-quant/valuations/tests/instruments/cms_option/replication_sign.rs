//! CMS static replication against independent lognormal density integration.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::rates::cms_option::replication_pricer::CmsReplicationPricer;
use finstack_quant_valuations::instruments::rates::cms_option::CmsOption;
use finstack_quant_valuations::instruments::OptionType;
use finstack_quant_valuations::pricer::Pricer;
use rust_decimal::Decimal;
use time::Month;

fn single_curve_market(as_of: Date, r: f64, v: f64) -> MarketContext {
    let mut mkt = MarketContext::new();

    let ois_knots: Vec<(f64, f64)> = [
        0.0, 0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 12.0, 15.0, 20.0, 30.0,
    ]
    .iter()
    .map(|&t| (t, (-r * t).exp()))
    .collect();

    mkt = mkt.insert(
        DiscountCurve::builder(CurveId::new("USD-FLAT"))
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots(ois_knots)
            .build()
            .unwrap(),
    );

    // Separate forward curve for multi-curve regressions.
    let fwd_knots = vec![(0.0, r), (30.0, r)];
    mkt = mkt.insert(
        ForwardCurve::builder(CurveId::new("USD-FLAT-FWD"), 0.25)
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots(fwd_knots)
            .build()
            .unwrap(),
    );

    // Flat vol surface
    let strikes = vec![
        0.005, 0.01, 0.015, 0.02, 0.025, 0.03, 0.035, 0.04, 0.05, 0.06, 0.07, 0.08, 0.10,
    ];
    let expiries = vec![0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 15.0, 20.0];
    let flat_row = vec![v; strikes.len()];
    let mut builder = VolSurface::builder(CurveId::new("USD-FLAT-VOL"))
        .expiries(&expiries)
        .strikes(&strikes);
    for _ in 0..expiries.len() {
        builder = builder.row(&flat_row);
    }
    mkt = mkt.insert_surface(builder.build().unwrap());
    mkt
}

/// Create a single-period CMS option using the single-curve market.
fn single_curve_cms(
    fixing: Date,
    payment: Date,
    strike_rate: f64,
    cms_tenor: f64,
    option_type: OptionType,
) -> CmsOption {
    CmsOption {
        id: InstrumentId::new("CMS-TEST"),
        strike: Decimal::try_from(strike_rate).expect("valid strike"),
        cms_tenor,
        fixing_dates: vec![fixing],
        payment_dates: vec![payment],
        accrual_fractions: vec![1.0],
        option_type,
        notional: Money::new(1.0, Currency::USD).expect("valid money fixture"),
        day_count: DayCount::Act365F,
        swap_convention: None,
        swap_fixed_frequency: Some(Tenor::semi_annual()),
        swap_float_frequency: Some(Tenor::quarterly()),
        // Same day count for both legs so forward rate equals OIS rate exactly.
        swap_day_count: Some(DayCount::Act365F),
        swap_float_day_count: Some(DayCount::Act365F),
        // Both legs use the same "USD-FLAT" curve so the single-curve path is taken.
        discount_curve_id: CurveId::new("USD-FLAT"),
        forward_curve_id: CurveId::new("USD-FLAT"),
        vol_surface_id: CurveId::new("USD-FLAT-VOL"),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

/// Price a `CmsOption` using `CmsReplicationPricer` directly.
fn replication_price(inst: &CmsOption, mkt: &MarketContext, as_of: Date) -> f64 {
    CmsReplicationPricer::new()
        .price_dyn(inst, mkt, as_of)
        .expect("replication pricing should succeed")
        .value
        .amount()
}

// Reference integrates the payoff itself, without differentiating the annuity
// or using hedge-option prices. Hagan (2003), equations 2.7–2.9.
fn density_price(inst: &CmsOption, market: &MarketContext, as_of: Date, vol: f64) -> f64 {
    use finstack_quant_core::dates::{DateExt, DayCountContext};
    use finstack_quant_core::math::integration::gauss_legendre_integrate_adaptive;
    let reference = inst.reference_swap();
    let start = reference
        .reference_swap_start(inst.fixing_dates[0])
        .unwrap();
    let end = start.add_months((inst.cms_tenor * 12.0).round() as i32);
    let (forward, _) = reference
        .forward_rate_and_annuity(market, as_of, start, end)
        .unwrap();
    let years = |a, b| {
        DayCount::Act365F
            .year_fraction(a, b, DayCountContext::default())
            .unwrap()
    };
    let t = years(as_of, inst.fixing_dates[0]);
    let delay = years(start, inst.payment_dates[0]);
    let df = market
        .get_discount("USD-FLAT")
        .unwrap()
        .df_between_dates(as_of, inst.payment_dates[0])
        .unwrap();
    let strike: f64 = inst.strike.to_string().parse().unwrap();
    let sign = if inst.option_type == OptionType::Call {
        1.0
    } else {
        -1.0
    };
    if vol == 0.0 {
        return df * (sign * (forward - strike)).max(0.0);
    }
    let m = reference.payments_per_year();
    let weight = |rate: f64| {
        // Direct par-annuity formula, evaluated only at strictly positive rates.
        let annuity = -(-(inst.cms_tenor * m) * (rate / m).ln_1p()).exp_m1() / rate;
        (1.0 + rate / m).powf(-m * delay) / annuity
    };
    let sigma_t = vol * t.sqrt();
    let density = |z: f64| (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let rate = |z: f64| forward * (sigma_t * z - 0.5 * sigma_t * sigma_t).exp();
    let integrate = |payoff: bool| {
        let integrand = |z| {
            let s = rate(z);
            weight(s)
                * density(z)
                * if payoff {
                    (sign * (s - strike)).max(0.0)
                } else {
                    1.0
                }
        };
        let kink = if strike > 0.0 {
            ((strike / forward).ln() + 0.5 * sigma_t * sigma_t) / sigma_t
        } else {
            0.0
        };
        let split = kink.clamp(-12.0, 12.0);
        gauss_legendre_integrate_adaptive(integrand, -12.0, split, 16, 1e-13, 20).unwrap()
            + gauss_legendre_integrate_adaptive(integrand, split, 12.0, 16, 1e-13, 20).unwrap()
    };
    df * integrate(true) / integrate(false)
}

#[test]
fn replication_matches_independent_density_across_strikes_and_payment_lags() {
    let as_of = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    for (fixing_year, tenor, vol) in [(2026, 10.0, 0.2), (2030, 20.0, 0.8)] {
        let fixing = Date::from_calendar_date(fixing_year, Month::January, 2).unwrap();
        for month in [Month::April, Month::December] {
            let payment = Date::from_calendar_date(fixing_year, month, 2).unwrap();
            let market = single_curve_market(as_of, 0.03, vol);
            for strike in [-0.01, 0.0, 0.00001, 0.02, 0.03, 0.04, 0.15] {
                for kind in [OptionType::Call, OptionType::Put] {
                    let inst = single_curve_cms(fixing, payment, strike, tenor, kind);
                    let actual = replication_price(&inst, &market, as_of);
                    let expected = density_price(&inst, &market, as_of, vol);
                    assert!(
                        (actual - expected).abs() < 2e-9,
                        "T={fixing_year} K={strike} vol={vol} {kind:?}: {actual} vs {expected}"
                    );
                }
            }
        }
    }
}

#[test]
fn replication_zero_volatility_preserves_intrinsic_for_itm_and_sub_bp_strikes() {
    let as_of = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let fixing = Date::from_calendar_date(2026, Month::January, 2).unwrap();
    let payment = Date::from_calendar_date(2026, Month::April, 2).unwrap();
    for vol in [0.0, 1e-8] {
        let market = single_curve_market(as_of, 0.03, vol);
        for strike in [-0.01, 0.0, 0.00001, 0.02, 0.03, 0.04] {
            for kind in [OptionType::Call, OptionType::Put] {
                let inst = single_curve_cms(fixing, payment, strike, 10.0, kind);
                let expected = density_price(&inst, &market, as_of, 0.0);
                assert!((replication_price(&inst, &market, as_of) - expected).abs() < 1e-10);
            }
        }
    }
}

#[test]
fn replication_cap_floor_parity_has_strike_independent_cms_forward() {
    let as_of = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let fixing = Date::from_calendar_date(2030, Month::January, 2).unwrap();
    let payment = Date::from_calendar_date(2030, Month::April, 2).unwrap();
    let market = single_curve_market(as_of, 0.03, 0.5);
    let df = market
        .get_discount("USD-FLAT")
        .unwrap()
        .df_between_dates(as_of, payment)
        .unwrap();
    let mut implied = Vec::new();
    for strike in [0.00001, 0.01, 0.03, 0.06, 0.15] {
        let cap = single_curve_cms(fixing, payment, strike, 20.0, OptionType::Call);
        let floor = single_curve_cms(fixing, payment, strike, 20.0, OptionType::Put);
        implied.push(
            strike
                + (replication_price(&cap, &market, as_of)
                    - replication_price(&floor, &market, as_of))
                    / df,
        );
    }
    for value in &implied {
        assert!((value - implied[0]).abs() < 1e-9);
    }
}
