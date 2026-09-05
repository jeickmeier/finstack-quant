use super::*;
use crate::instruments::fixed_income::bond::{
    Bond, BondSettlementConvention, CallPut, CallPutSchedule,
};
use crate::instruments::{Instrument, PricingOptions};
use crate::metrics::{standard_registry, MetricCalculator, MetricContext, MetricId};
use crate::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricerRegistry, PricingError,
};
use crate::results::ValuationResult;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use std::sync::{Arc, Mutex};
use time::macros::date;

struct LinearTreeOasPricer {
    call_dates: Arc<Mutex<Vec<Date>>>,
}

impl LinearTreeOasPricer {
    fn price(
        &self,
        instrument: &dyn Instrument,
        as_of: Date,
    ) -> std::result::Result<f64, PricingError> {
        self.call_dates
            .lock()
            .expect("call-date recorder should not be poisoned")
            .push(as_of);
        let bond = expect_inst::<Bond>(instrument, InstrumentType::Bond)?;
        let oas = bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_oas
            .unwrap_or(0.0);
        Ok(1_000.0 - 10_000.0 * oas)
    }
}

impl Pricer for LinearTreeOasPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Bond, ModelKey::Tree)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        _market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        Ok(ValuationResult::stamped(
            instrument.id(),
            as_of,
            Money::new(self.price(instrument, as_of)?, Currency::USD).expect("valid test amount"),
        ))
    }

    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        _market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<f64, PricingError> {
        self.price(instrument, as_of)
    }
}

struct FixedYtmCalculator;

impl MetricCalculator for FixedYtmCalculator {
    fn calculate(&self, _context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        Ok(0.123_456)
    }
}

#[test]
fn asset_swap_forward_paths_use_discount_factor_implied_rates() {
    let base = date!(2025 - 01 - 01);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .build()
        .expect("discount curve should build");
    let fwd = ForwardCurve::builder("USD-3M", 0.25)
        .base_date(base)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .knots([(0.0, 0.01), (1.0, 0.21)])
        .build()
        .expect("forward curve should build");
    let schedule = [base, date!(2025 - 07 - 01), date!(2026 - 01 - 01)];
    let mut expected_float_pv = 0.0;
    let mut integrated_float_pv = 0.0;
    for dates in schedule.windows(2) {
        let t1 = fwd
            .day_count()
            .year_fraction(base, dates[0], DayCountContext::default())
            .expect("valid start time");
        let t2 = fwd
            .day_count()
            .year_fraction(base, dates[1], DayCountContext::default())
            .expect("valid end time");
        let yf = fwd
            .day_count()
            .year_fraction(dates[0], dates[1], DayCountContext::default())
            .expect("valid accrual fraction");
        let df = disc
            .df_on_date_curve(dates[1])
            .expect("valid discount factor");
        expected_float_pv += fwd.rate_between(t1, t2).expect("valid term forward") * yf * df;
        integrated_float_pv += fwd.rate_period(t1, t2) * yf * df;
    }

    let (float_pv, fixed_ann, _) = asset_swap_forward_components(
        &disc,
        &fwd,
        finstack_quant_core::dates::DayCount::Act360,
        None,
        &schedule,
        0.0,
    )
    .expect("asset-swap components should succeed");
    let (par_rate, par_ann) = par_rate_and_annuity_from_forward(
        &disc,
        &fwd,
        finstack_quant_core::dates::DayCount::Act360,
        None,
        &schedule,
        0.0,
    )
    .expect("forward par rate should succeed");

    assert!((expected_float_pv - integrated_float_pv).abs() > 1e-6);
    assert!((float_pv - expected_float_pv).abs() < 1e-14);
    assert!((par_ann - fixed_ann).abs() < 1e-14);
    assert!((par_rate - expected_float_pv / fixed_ann).abs() < 1e-14);
}

#[test]
fn overnight_asset_swap_forward_paths_use_observation_average() {
    let base = date!(2025 - 01 - 01);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .build()
        .expect("discount curve should build");
    let fwd = ForwardCurve::builder("USD-SOFR", 1.0 / 360.0)
        .base_date(base)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .knots([(0.0, 0.01), (1.0, 0.21)])
        .build()
        .expect("forward curve should build");
    let schedule = [base, date!(2026 - 01 - 01)];
    let t2 = fwd
        .day_count()
        .year_fraction(base, schedule[1], DayCountContext::default())
        .expect("valid end time");
    let yf = t2;
    let df = disc
        .df_on_date_curve(schedule[1])
        .expect("valid discount factor");
    let expected_float_pv = fwd.rate_period(0.0, t2) * yf * df;
    let term_float_pv = fwd.rate_between(0.0, t2).expect("valid term forward") * yf * df;

    let (float_pv, _, _) = asset_swap_forward_components(
        &disc,
        &fwd,
        finstack_quant_core::dates::DayCount::Act360,
        None,
        &schedule,
        0.0,
    )
    .expect("asset-swap components should succeed");

    assert!((expected_float_pv - term_float_pv).abs() > 1e-6);
    assert!((float_pv - expected_float_pv).abs() < 1e-14);
}

/// 31 CFR Part 356, Appendix B, section II.C long-first-period example.
///
/// 8.5% note issued 1990-03-01, first payment 1990-11-15, maturity
/// 1995-05-15, priced at an 8.53% Treasury yield: 99.805118 per 100.
#[test]
fn treasury_actual_matches_cfr_long_first_coupon_example() {
    let as_of = date!(1990 - 03 - 01);
    let coupon = 8.50 / 2.0;
    let fractional_coupon = coupon * 75.0 / 181.0;
    let flows = vec![
        (
            date!(1990 - 11 - 15),
            Money::new(coupon + fractional_coupon, Currency::USD).expect("valid test amount"),
        ),
        (
            date!(1991 - 05 - 15),
            Money::new(coupon, Currency::USD).expect("valid test amount"),
        ),
        (
            date!(1991 - 11 - 15),
            Money::new(coupon, Currency::USD).expect("valid test amount"),
        ),
        (
            date!(1992 - 05 - 15),
            Money::new(coupon, Currency::USD).expect("valid test amount"),
        ),
        (
            date!(1992 - 11 - 15),
            Money::new(coupon, Currency::USD).expect("valid test amount"),
        ),
        (
            date!(1993 - 05 - 15),
            Money::new(coupon, Currency::USD).expect("valid test amount"),
        ),
        (
            date!(1993 - 11 - 15),
            Money::new(coupon, Currency::USD).expect("valid test amount"),
        ),
        (
            date!(1994 - 05 - 15),
            Money::new(coupon, Currency::USD).expect("valid test amount"),
        ),
        (
            date!(1994 - 11 - 15),
            Money::new(coupon, Currency::USD).expect("valid test amount"),
        ),
        (
            date!(1995 - 05 - 15),
            Money::new(100.0 + coupon, Currency::USD).expect("valid test amount"),
        ),
    ];

    let price = price_from_ytm_compounded_params(
        DayCount::ActActIsma,
        Tenor::semi_annual(),
        &flows,
        as_of,
        0.0853,
        YieldCompounding::TreasuryActual,
    )
    .expect("Treasury Appendix B price");

    assert!(
        (price - 99.805118).abs() < 5e-7,
        "CFR long-first price mismatch: {price}"
    );
}

#[test]
fn treasury_actual_zero_coupon_round_trips() {
    let as_of = date!(2025 - 01 - 01);
    let flows = vec![(date!(2025 - 09 - 01), Money::from((100_i64, Currency::USD)))];
    let frequency = Tenor::semi_annual();
    let day_count = DayCount::Act365F;
    let expected_yield = 0.05;
    let price = price_from_ytm_compounded_params(
        day_count,
        frequency,
        &flows,
        as_of,
        expected_yield,
        YieldCompounding::TreasuryActual,
    )
    .expect("price");

    let solved = crate::instruments::fixed_income::bond::pricing::ytm_solver::solve_ytm(
        &flows,
        as_of,
        Money::new(price, Currency::USD).expect("valid test amount"),
        crate::instruments::fixed_income::bond::pricing::ytm_solver::YtmPricingSpec {
            day_count,
            notional: Money::from((100_i64, Currency::USD)),
            coupon_rate: 0.0,
            compounding: YieldCompounding::TreasuryActual,
            frequency,
        },
    )
    .expect("yield");
    assert!((solved - expected_yield).abs() < 1e-11);
}

#[test]
fn compute_quotes_returns_zeroes_for_effectively_zero_notional() {
    let as_of = date!(2025 - 01 - 01);
    let bond = Bond::fixed(
        "QE-NEAR-ZERO-NOTIONAL",
        Money::new(1e-12, Currency::USD).expect("valid test amount"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid test rate"),
        as_of,
        date!(2030 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.8)])
        .build()
        .expect("curve");

    let quotes = compute_quotes(
        &bond,
        &MarketContext::new().insert(curve),
        as_of,
        BondQuoteInput::CleanPricePct(99.0),
        crate::instruments::PricingOptions::default()
            .with_model(crate::pricer::ModelKey::Discounting),
    )
    .expect("quote conversion");

    assert_eq!(quotes.clean_price_currency, 0.0);
    assert_eq!(quotes.clean_price_pct, 0.0);
    assert_eq!(quotes.dirty_price_currency, 0.0);
    assert!(quotes.ytm.is_none());
}

#[test]
fn z_spread_input_rejects_option_bond_without_quoted_workout_price() {
    use crate::instruments::fixed_income::bond::{CallPut, CallPutSchedule};

    let as_of = date!(2025 - 01 - 01);
    let mut bond = Bond::fixed(
        "QE-ZSPREAD-CALLABLE",
        Money::from((100_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid test rate"),
        as_of,
        date!(2030 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.settlement_convention = None;
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2027 - 01 - 01),
            end_date: date!(2027 - 01 - 01),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: Vec::new(),
    });
    let market = MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .build()
            .expect("curve"),
    );

    let direct_error = price_from_z_spread(&bond, &market, as_of, 0.01)
        .expect_err("direct Z-spread pricing must not ignore callability");
    assert!(direct_error
        .to_string()
        .contains("requires an explicit quoted clean price"));

    let quote_error = compute_quotes(
        &bond,
        &market,
        as_of,
        BondQuoteInput::ZSpread(0.01),
        crate::instruments::PricingOptions::default().with_model(crate::pricer::ModelKey::Tree),
    )
    .expect_err("Z-spread quote normalization must not ignore callability");
    assert!(quote_error
        .to_string()
        .contains("requires an explicit quoted clean price"));
}

#[test]
fn quote_engine_ytw_input_uses_callable_workout_inverse() {
    let as_of = date!(2025 - 01 - 15);
    let mut bond = Bond::fixed(
        "QE-YTW-CALLABLE",
        Money::from((100_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid test rate"),
        as_of,
        date!(2030 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.settlement_convention = None;
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2027 - 01 - 15),
            end_date: date!(2027 - 01 - 15),
            price_pct_of_par: 70.0,
            make_whole: None,
        }],
        puts: Vec::new(),
    });
    let market = MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .build()
            .expect("curve"),
    );
    let target_ytw = 0.05;
    let expected =
        price_from_ytw(&bond, &market, as_of, target_ytw).expect("direct callable YTW inversion");
    let maturity_price = price_from_ytm(
        &bond,
        &bond
            .pricing_dated_cashflows(&market, as_of)
            .expect("maturity flows"),
        as_of,
        target_ytw,
    )
    .expect("maturity price");
    assert!(expected + 20.0 < maturity_price);

    let quotes = compute_quotes(
        &bond,
        &market,
        as_of,
        BondQuoteInput::Ytw(target_ytw),
        crate::instruments::PricingOptions::default().with_model(crate::pricer::ModelKey::Tree),
    )
    .expect("callable YTW quote conversion");

    assert!((quotes.dirty_price_currency - expected).abs() < 1e-10);
}

#[test]
fn compute_quotes_preserves_custom_tree_and_metric_registries_for_oas_roundtrip() {
    let as_of = date!(2025 - 01 - 15);
    let mut bond = Bond::fixed(
        "QE-CUSTOM-TREE-OAS",
        Money::from((1000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.0).expect("valid test rate"),
        as_of,
        date!(2030 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("valid bond");
    bond.settlement_convention = Some(BondSettlementConvention {
        settlement_days: 2,
        ..Default::default()
    });
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start_date: date!(2028 - 01 - 15),
            end_date: date!(2028 - 01 - 15),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: Vec::new(),
    });
    assert_eq!(bond.default_model(), ModelKey::Tree);

    let market = MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .build()
            .expect("discount curve"),
    );
    let quote_date =
        crate::instruments::fixed_income::bond::pricing::settlement::settlement_date(&bond, as_of)
            .expect("settlement date");
    assert_ne!(quote_date, as_of);

    let call_dates = Arc::new(Mutex::new(Vec::new()));
    let mut pricers = PricerRegistry::new();
    pricers
        .register(LinearTreeOasPricer {
            call_dates: Arc::clone(&call_dates),
        })
        .expect("unique custom Tree pricer");

    let mut metrics = standard_registry().clone();
    metrics
        .replace_metric(
            MetricId::Ytm,
            Arc::new(FixedYtmCalculator),
            &[InstrumentType::Bond],
        )
        .expect("replace YTM for this test");

    let target_oas = 0.002;
    let quotes = compute_quotes(
        &bond,
        &market,
        as_of,
        BondQuoteInput::Oas(target_oas),
        PricingOptions::default()
            .with_registry(Arc::new(pricers))
            .with_metric_registry(Arc::new(metrics)),
    )
    .expect("custom Tree OAS quote roundtrip");

    assert!((quotes.dirty_price_currency - 980.0).abs() < 1.0e-12);
    assert!((quotes.oas.expect("OAS metric") - target_oas).abs() < 1.0e-10);
    assert_eq!(quotes.ytm, Some(0.123_456));

    let recorded = call_dates
        .lock()
        .expect("call-date recorder should not be poisoned");
    assert_eq!(
        recorded.first(),
        Some(&quote_date),
        "OAS input normalization must use the selected pricer at settlement"
    );
    assert!(
        recorded.contains(&as_of),
        "base PV must use the selected pricer at the valuation date"
    );
    assert!(
        recorded.iter().filter(|date| **date == quote_date).count() > 1,
        "OAS solver trials must retain the selected pricer at settlement"
    );
}
