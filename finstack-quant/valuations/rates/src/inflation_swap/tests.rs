use super::yoy::YoYInflationSwap;
use super::zero_coupon::InflationSwap;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, DayCountContext, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{InflationIndex, InflationObservation};
use finstack_quant_core::market_data::scalars::{InflationLag, SeriesInterpolation};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, InflationCurve};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{Attributes, CurveId, InstrumentId};
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::PayReceive;
use rust_decimal::Decimal;
use time::Month;

fn d(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).expect("valid date")
}

fn sample_inflation_curve(base_date: Date) -> InflationCurve {
    InflationCurve::builder("US-CPI")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Flat)
        .knots([(0.0, 100.0), (1.0, 110.0), (2.0, 121.0)])
        .build()
        .expect("inflation curve should build")
}

fn sample_discount_curve(base_date: Date) -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (2.0, 1.0)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve should build")
}

fn flat_historical_index() -> InflationIndex {
    let knowledge_date = d(2025, Month::January, 1);
    let fixture = finstack_quant_valuations_test_support::inflation::simple_index(
        "US-CPI",
        knowledge_date,
        100.0,
        Currency::USD,
    );
    InflationIndex::new(
        "US-CPI",
        fixture
            .observations()
            .iter()
            .map(|observation| InflationObservation {
                value: 100.0,
                ..*observation
            })
            .collect(),
        Currency::USD,
        knowledge_date,
    )
    .expect("inflation index should build")
    .with_expected_releases(fixture.get_expected_releases().to_vec())
    .expect("explicit synthetic releases")
}

fn sample_swap(start_date: Date, maturity: Date) -> InflationSwap {
    InflationSwap::builder()
        .id(InstrumentId::new("INFL-SWAP"))
        .notional(Money::from((1_000_000_i64, Currency::USD)))
        .start_date(start_date)
        .maturity(maturity)
        .fixed_rate(Decimal::ZERO)
        .inflation_index_id(CurveId::new("US-CPI"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .lag(InflationLag::None)
        .interpolation(SeriesInterpolation::Step)
        .attributes(Attributes::new())
        .build()
        .expect("swap should build")
}

#[test]
fn inflation_theta_collects_settlement_cash_and_adjusted_final_payment() {
    use finstack_quant_valuations_core::instruments::PricingOptions;
    use finstack_quant_valuations_core::metrics::MetricId;
    use time::macros::date;
    let as_of = date!(2025 - 01 - 31);
    let maturity = date!(2025 - 02 - 01);
    let market = MarketContext::new()
        .try_insert(sample_discount_curve(as_of))
        .expect("insert market curve")
        .try_insert(
            InflationCurve::builder("US-CPI")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .interp(InterpStyle::LogLinear)
                .extrapolation(ExtrapolationPolicy::Flat)
                .knots([(0.0, 310.0), (1.0, 310.0)])
                .build()
                .expect("CPI curve"),
        )
        .expect("insert market curve")
        .insert_inflation_index(
            finstack_quant_valuations_test_support::inflation::simple_index(
                "US-CPI",
                as_of,
                300.0,
                Currency::USD,
            ),
        );
    let mut swap = sample_swap(date!(2025 - 01 - 01), maturity);
    swap.base_cpi = Some(300.0);
    for (calendar, convention, days) in [
        (None, BusinessDayConvention::Unadjusted, 1.0),
        (
            Some("weekends_only".into()),
            BusinessDayConvention::Following,
            3.0,
        ),
    ] {
        swap.calendar_id = calendar;
        swap.business_day_convention = convention;
        swap.metric_pricing_overrides.theta_period =
            Some(Tenor::new(1, finstack_quant_core::dates::TenorUnit::Weeks).expect("week"));
        let result = swap
            .price_with_metrics(
                &market,
                as_of,
                &[
                    MetricId::Theta,
                    MetricId::ThetaCarry,
                    MetricId::ThetaPeriodDays,
                ],
                PricingOptions::default(),
            )
            .expect("theta");
        assert!((result.value.amount() - 1_000_000.0 / 30.0).abs() < 1e-8);
        assert!(result.metric(MetricId::Theta).expect("theta").abs() < 1e-8);
        assert!(
            (result.metric(MetricId::ThetaCarry).expect("carry") - result.value.amount()).abs()
                < 1e-8
        );
        assert_eq!(result.metric(MetricId::ThetaPeriodDays), Some(days));
    }
    let mut yoy = YoYInflationSwap::example().expect("YoY swap");
    yoy.start_date = date!(2024 - 02 - 01);
    yoy.maturity = maturity;
    yoy.fixed_rate = Decimal::ZERO;
    yoy.base_cpi = Some(300.0);
    yoy.lag = Some(InflationLag::None);
    yoy.calendar_id = Some("weekends_only".into());
    yoy.business_day_convention = BusinessDayConvention::Following;
    yoy.metric_pricing_overrides.theta_period = swap.metric_pricing_overrides.theta_period;
    let result = yoy
        .price_with_metrics(
            &market,
            as_of,
            &[
                MetricId::Theta,
                MetricId::ThetaCarry,
                MetricId::ThetaPeriodDays,
            ],
            PricingOptions::default(),
        )
        .expect("YoY theta");
    assert!(result.metric(MetricId::Theta).expect("theta").abs() < 1e-8);
    assert!(
        (result.metric(MetricId::ThetaCarry).expect("carry") - result.value.amount()).abs() < 1e-8
    );
    assert_eq!(result.metric(MetricId::ThetaPeriodDays), Some(3.0));
}

#[test]
fn projected_index_ratio_uses_inflation_curve_anchor_date() {
    let curve_base = d(2024, Month::January, 1);
    let discount_base = d(2024, Month::June, 1);
    let start = d(2024, Month::March, 1);
    let maturity = d(2025, Month::March, 1);
    let swap = sample_swap(start, maturity);
    let inflation_curve = sample_inflation_curve(curve_base);
    let market = MarketContext::new()
        .try_insert(sample_discount_curve(discount_base))
        .expect("insert market curve")
        .try_insert(inflation_curve.clone())
        .expect("insert market curve")
        .insert_inflation_index(flat_historical_index());

    let ratio = swap
        .projected_index_ratio(&market, discount_base)
        .expect("ratio should compute");
    let expected = inflation_curve.cpi_on_date(maturity).expect("maturity CPI") / 100.0;

    assert!(
        (ratio - expected).abs() < 1e-10,
        "expected ratio {expected}, got {ratio}"
    );
}

#[test]
fn yoy_swap_prices_with_curve_only_market() {
    let as_of = d(2025, Month::January, 1);
    let market = MarketContext::new()
        .try_insert(sample_discount_curve(as_of))
        .expect("insert market curve")
        .try_insert(sample_inflation_curve(as_of))
        .expect("insert market curve")
        .insert_inflation_index(flat_historical_index());
    let swap = YoYInflationSwap::builder()
        .id(InstrumentId::new("YOY-CURVE-ONLY"))
        .notional(Money::from((1_000_000_i64, Currency::USD)))
        .start_date(as_of)
        .maturity(d(2027, Month::January, 1))
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .frequency(Tenor::annual())
        .inflation_index_id(CurveId::new("US-CPI"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .lag(InflationLag::None)
        .base_cpi(100.0)
        .interpolation(SeriesInterpolation::Step)
        .attributes(Attributes::new())
        .build()
        .expect("yoy swap should build");

    let pv = swap
        .npv_raw(&market, as_of)
        .expect("curve-only market should price");

    assert!(pv.is_finite());
}

#[test]
fn matured_but_unpaid_swap_retains_value_until_adjusted_payment_date() {
    let start = d(2024, Month::January, 15);
    let maturity = d(2025, Month::January, 18);
    let as_of = d(2025, Month::January, 19);
    let mut swap = sample_swap(start, maturity);
    swap.business_day_convention = BusinessDayConvention::Following;
    swap.calendar_id = Some("nyse".into());

    let market = MarketContext::new()
        .try_insert(sample_discount_curve(as_of))
        .expect("insert market curve")
        .try_insert(sample_inflation_curve(d(2024, Month::January, 1)))
        .expect("insert market curve")
        .insert_inflation_index(
            InflationIndex::new(
                "US-CPI",
                vec![
                    InflationObservation {
                        reference_month: start.replace_day(1).unwrap(),
                        value: 100.0,
                        available_on: start,
                    },
                    InflationObservation {
                        reference_month: maturity.replace_day(1).unwrap(),
                        value: 110.0,
                        available_on: maturity,
                    },
                ],
                Currency::USD,
                as_of,
            )
            .expect("inflation index should build"),
        );

    let pv = swap.value(&market, as_of).expect("value should compute");
    assert!(
        pv.amount().abs() > 0.0,
        "swap should retain value between contractual maturity and adjusted payment date"
    );
}

#[test]
fn zero_coupon_swap_cashflow_provider_emits_two_maturity_flows() {
    let as_of = d(2025, Month::January, 1);
    let maturity = d(2027, Month::January, 1);
    let market = MarketContext::new()
        .try_insert(sample_discount_curve(as_of))
        .expect("insert market curve")
        .try_insert(sample_inflation_curve(as_of))
        .expect("insert market curve")
        .insert_inflation_index(flat_historical_index());
    let swap = InflationSwap::builder()
        .id(InstrumentId::new("INFL-CF"))
        .notional(Money::from((1_000_000_i64, Currency::USD)))
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .inflation_index_id(CurveId::new("US-CPI"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .lag(InflationLag::None)
        .interpolation(SeriesInterpolation::Step)
        .attributes(Attributes::new())
        .build()
        .expect("swap should build");

    let flows = swap
        .dated_cashflows(&market, as_of)
        .expect("contractual schedule should build");
    let schedule = swap
        .cashflow_schedule(&market, as_of)
        .expect("classified inflation schedule");

    assert_eq!(flows.len(), 2, "zc inflation swap should emit both legs");
    assert!(flows.iter().all(|(date, _)| *date == maturity));
    assert!(
        flows[0].1.amount() < 0.0,
        "pay-fixed swap should pay fixed leg"
    );
    assert!(
        flows[1].1.amount() > 0.0,
        "pay-fixed swap should receive inflation leg"
    );
    assert_eq!(schedule.get_flows()[0].get_kind(), CFKind::Fixed);
    assert_eq!(schedule.get_flows()[1].get_kind(), CFKind::InflationCoupon);
    assert!(
        schedule
            .get_flows()
            .iter()
            .all(|flow| flow.get_accrual().is_some())
    );
}

/// YoY `npv_raw` must discount with DF(as_of → pay) = DF(pay)/DF(as_of)
/// (curve-base relative), not DF(yf(as_of, pay)) which silently assumes
/// the curve base equals as_of.
#[test]
fn yoy_npv_raw_discounts_relative_to_as_of_on_rebased_curve() {
    let curve_base = d(2024, Month::January, 1);
    let as_of = d(2025, Month::January, 1);
    let maturity = d(2026, Month::January, 1);

    // Non-trivial DF shape so DF(pay)/DF(as_of) != DF(yf(as_of, pay)).
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(curve_base)
        .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.88), (3.0, 0.80)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve should build");
    // Flat CPI so the inflation leg is exactly zero.
    let infl = InflationCurve::builder("US-CPI")
        .base_date(curve_base)
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Flat)
        .knots([(0.0, 100.0), (3.0, 100.0)])
        .build()
        .expect("inflation curve should build");
    let market = MarketContext::new()
        .try_insert(disc.clone())
        .expect("insert market curve")
        .try_insert(infl)
        .expect("insert market curve")
        .insert_inflation_index(flat_historical_index());

    let swap = YoYInflationSwap::builder()
        .id(InstrumentId::new("YOY-REBASE"))
        .notional(Money::from((1_000_000_i64, Currency::USD)))
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .frequency(Tenor::annual())
        .inflation_index_id(CurveId::new("US-CPI"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .day_count(DayCount::Act365F)
        .side(PayReceive::Receive)
        .lag(InflationLag::None)
        .interpolation(SeriesInterpolation::Step)
        .attributes(Attributes::new())
        .build()
        .expect("yoy swap should build");

    let pv = swap.npv_raw(&market, as_of).expect("npv should compute");

    // Inflation leg is 0 (flat CPI); fixed leg = N * r * accrual * DF.
    let accrual = DayCount::Act365F
        .year_fraction(as_of, maturity, DayCountContext::default())
        .expect("accrual");
    let df_expected = disc.df_between_dates(as_of, maturity).expect("relative df");
    let expected = 1_000_000.0 * 0.02 * accrual * df_expected;
    assert!(
        (pv - expected).abs() < 1e-6,
        "expected relative-DF PV {expected}, got {pv}"
    );

    // Sanity: the buggy absolute-time DF would differ materially.
    let t_wrong = DayCount::Act365F
        .year_fraction(as_of, maturity, DayCountContext::default())
        .expect("yf");
    let df_wrong = disc.df(t_wrong).expect("df");
    assert!(
        (df_expected - df_wrong).abs() > 1e-3,
        "test setup must distinguish the two discounting conventions"
    );
}

/// Realized CPI fixings dated after `as_of` must not be consulted
/// (look-ahead bias): pricing must match the curve-projection PV.
#[test]
fn yoy_cpi_value_ignores_fixings_after_as_of() {
    let as_of = d(2025, Month::January, 1);
    let maturity = d(2026, Month::January, 1);

    let disc = sample_discount_curve(as_of);
    let infl = InflationCurve::builder("US-CPI")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Flat)
        .knots([(0.0, 100.0), (2.0, 110.0)])
        .build()
        .expect("inflation curve should build");

    let market_no_index = MarketContext::new()
        .try_insert(disc.clone())
        .expect("insert market curve")
        .try_insert(infl.clone())
        .expect("insert market curve")
        .insert_inflation_index(flat_historical_index());

    // A future version cannot be admitted to the earlier information snapshot.
    assert!(
        InflationIndex::new(
            "US-CPI",
            vec![InflationObservation {
                reference_month: maturity,
                value: 500.0,
                available_on: maturity,
            }],
            Currency::USD,
            as_of,
        )
        .is_err()
    );
    let market_with_future_fixing = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(infl)
        .expect("insert market curve")
        .insert_inflation_index(flat_historical_index());

    let swap = YoYInflationSwap::builder()
        .id(InstrumentId::new("YOY-LOOKAHEAD"))
        .notional(Money::from((1_000_000_i64, Currency::USD)))
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .frequency(Tenor::annual())
        .inflation_index_id(CurveId::new("US-CPI"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .lag(InflationLag::None)
        .interpolation(SeriesInterpolation::Step)
        .attributes(Attributes::new())
        .build()
        .expect("yoy swap should build");

    let pv_clean = swap
        .npv_raw(&market_no_index, as_of)
        .expect("clean pv should compute");
    let pv_with_future = swap
        .npv_raw(&market_with_future_fixing, as_of)
        .expect("pv with future fixing should compute");

    assert!(
        (pv_clean - pv_with_future).abs() < 1e-6,
        "future-dated fixings must not affect PV (look-ahead bias): \
         clean={pv_clean}, with_future_fixing={pv_with_future}"
    );
}

#[test]
fn yoy_swap_cashflow_provider_emits_two_flows_per_period() {
    let as_of = d(2025, Month::January, 1);
    let market = MarketContext::new()
        .try_insert(sample_discount_curve(as_of))
        .expect("insert market curve")
        .try_insert(sample_inflation_curve(as_of))
        .expect("insert market curve")
        .insert_inflation_index(flat_historical_index());
    let swap = YoYInflationSwap::builder()
        .id(InstrumentId::new("YOY-CF"))
        .notional(Money::from((1_000_000_i64, Currency::USD)))
        .start_date(as_of)
        .maturity(d(2027, Month::January, 1))
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .frequency(Tenor::annual())
        .inflation_index_id(CurveId::new("US-CPI"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .lag(InflationLag::None)
        .interpolation(SeriesInterpolation::Step)
        .attributes(Attributes::new())
        .build()
        .expect("yoy swap should build");

    let flows = swap
        .dated_cashflows(&market, as_of)
        .expect("yoy contractual schedule should build");
    let schedule = swap
        .cashflow_schedule(&market, as_of)
        .expect("classified yoy schedule");

    assert_eq!(
        flows.len(),
        4,
        "two annual periods should emit fixed and inflation rows"
    );
    assert_eq!(
        flows
            .iter()
            .filter(|(_, money)| money.amount() < 0.0)
            .count(),
        2
    );
    assert_eq!(
        flows
            .iter()
            .filter(|(_, money)| money.amount() > 0.0)
            .count(),
        2
    );
    assert_eq!(
        schedule
            .get_flows()
            .iter()
            .filter(|flow| flow.get_kind() == CFKind::Fixed)
            .count(),
        2
    );
    assert_eq!(
        schedule
            .get_flows()
            .iter()
            .filter(|flow| flow.get_kind() == CFKind::InflationCoupon)
            .count(),
        2
    );
    assert!(
        schedule
            .get_flows()
            .iter()
            .all(|flow| flow.get_accrual().is_some())
    );
}

#[test]
fn yoy_swap_registers_par_rate_and_convexity_and_rejects_degenerate_par_rate() {
    use finstack_quant_valuations_core::metrics::MetricId;

    let as_of = d(2025, Month::January, 1);
    let market = MarketContext::new()
        .try_insert(sample_discount_curve(as_of))
        .expect("insert market curve")
        .try_insert(sample_inflation_curve(as_of))
        .expect("insert market curve")
        .insert_inflation_index(flat_historical_index());
    let mut swap = YoYInflationSwap::builder()
        .id(InstrumentId::new("YOY-METRICS"))
        .notional(Money::from((1_000_000_i64, Currency::USD)))
        .start_date(as_of)
        .maturity(d(2027, Month::January, 1))
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .frequency(Tenor::annual())
        .inflation_index_id(CurveId::new("US-CPI"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .lag(InflationLag::None)
        .interpolation(SeriesInterpolation::Step)
        .attributes(Attributes::new())
        .build()
        .expect("yoy swap");

    let res = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParRate, MetricId::InflationConvexity],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .expect("YoY ParRate and InflationConvexity should succeed");
    assert!(res.measures[&MetricId::ParRate].is_finite());
    assert!(res.measures[&MetricId::InflationConvexity].is_finite());

    let expired = d(2028, Month::January, 1);
    assert!(swap.par_rate(&market, expired).is_err());
    assert!(
        swap.cashflow_schedule(&market, d(2027, Month::January, 1))
            .expect("schedule on maturity")
            .get_flows()
            .is_empty()
    );

    swap.base_cpi = Some(f64::NAN);
    assert!(swap.validate().is_err());
}
