//! Independent accrued-interest, quote, yield and effective-assumption checks.

use finstack_quant_core::market_data::{context::MarketContext, term_structures::DiscountCurve};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::generate_tranche_cashflows;
use finstack_quant_valuations::{
    instruments::PricingOptions,
    instruments::{
        fixed_income::structured_credit::{
            calculate_tranche_metrics, DefaultModelSpec, PrepaymentModelSpec, StructuredCredit,
        },
        Instrument,
    },
    metrics::MetricId,
};
use time::macros::date;

fn deal_and_market() -> (StructuredCredit, MarketContext) {
    let mut deal = StructuredCredit::example().expect("example");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
    let market = MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(deal.closing_date)
            .knots([(0.0, 1.0), (20.0, (-0.03_f64 * 20.0).exp())])
            .build()
            .expect("discount curve"),
    );
    (deal, market)
}

#[test]
fn production_structured_metrics_seasoned_accrued_uses_opening_balance() {
    let (deal, market) = deal_and_market();
    let as_of = date!(2024 - 02 - 15);
    let result = deal
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Accrued],
            PricingOptions::default(),
        )
        .expect("metrics");
    assert!((result.measures["accrued"] - 750_000.0).abs() < 1e-6);
}

#[test]
fn production_structured_metrics_unrequested_yield_reprices_the_cashflows() {
    let (deal, market) = deal_and_market();
    let result = deal
        .value_tranche_with_metrics(
            deal.tranches.tranches[0].id.as_str(),
            &market,
            deal.closing_date,
            &[],
        )
        .expect("tranche result");
    // The yield compounds at the note's quarterly coupon frequency and
    // measures time in its Act/360 day count: an independent bisection on
    // the same flows reproduces the model dirty value.
    let ytm = result.ytm.expect("a performing note carries a yield");
    let expected = quarterly_yield(&deal, &market, deal.closing_date, result.dirty_price);
    assert!((ytm - expected).abs() < 1e-7, "{ytm} vs {expected}");
}

/// Quarterly-compounded yield of `deal`'s first tranche reproducing
/// `dirty_price_pct` of its current balance at buyer settlement, with time in
/// the note's own day count (Act/360 here): solved by bisection.
fn quarterly_yield(
    deal: &StructuredCredit,
    market: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
    dirty_price_pct: f64,
) -> f64 {
    use finstack_quant_core::dates::DayCountContext;
    let tranche = &deal.tranches.tranches[0];
    let flows =
        generate_tranche_cashflows(deal, tranche.id.as_str(), market, as_of).expect("flows");
    let settlement = deal.quote_settlement_date.unwrap_or(as_of);
    let target = dirty_price_pct / 100.0 * tranche.current_balance.amount();
    let pv_at = |y: f64| -> f64 {
        flows
            .cashflows
            .iter()
            .filter(|(date, _)| *date > settlement)
            .map(|(date, amount)| {
                let t = tranche
                    .day_count
                    .year_fraction(settlement, *date, DayCountContext::default())
                    .expect("year fraction");
                amount.amount() * (1.0 + y / 4.0).powf(-4.0 * t)
            })
            .sum::<f64>()
    };
    let (mut lo, mut hi) = (-0.5_f64, 1.0_f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if pv_at(mid) > target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

#[test]
fn production_structured_metrics_rates_match_effective_model() {
    let (mut deal, market) = deal_and_market();
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.36);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.07);
    let result = deal
        .price_with_metrics(
            &market,
            deal.closing_date,
            &[MetricId::CPR, MetricId::CDR],
            PricingOptions::default(),
        )
        .expect("metrics");
    assert!((result.measures["cpr"] - 0.36).abs() < 1e-12);
    assert!((result.measures["cdr"] - 0.07).abs() < 1e-12);
}

#[test]
fn production_structured_metrics_spread_duration_uses_quoted_value() {
    let (deal, market) = deal_and_market();
    let metrics = calculate_tranche_metrics(
        &deal,
        deal.tranches.tranches[0].id.as_str(),
        &market,
        deal.closing_date,
        Some(75.0),
    )
    .expect("quoted metrics");
    let expected = -metrics.cs01 / (75_000_000.0 * 1e-4);
    assert!(
        (metrics.spread_duration - expected).abs() < 1e-10,
        "{} versus {expected}",
        metrics.spread_duration
    );
}

#[test]
fn production_structured_metrics_clean_quote_adds_accrued_once() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::calculate_tranche_z_spread;
    let (mut deal, market) = deal_and_market();
    let as_of = date!(2024 - 02 - 15);
    deal.instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price_pct = Some(99.25);
    let flows =
        generate_tranche_cashflows(&deal, deal.tranches.tranches[0].id.as_str(), &market, as_of)
            .expect("flows");
    let expected = calculate_tranche_z_spread(
        &flows.cashflows,
        &market.get_discount("USD-OIS").expect("curve"),
        Money::from((100_000_000_i64, Currency::USD)),
        as_of,
    )
    .expect("independent dirty target")
        * 1e-4;
    let result = deal
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ZSpread],
            PricingOptions::default(),
        )
        .expect("canonical clean quote accepted");
    assert!(
        (result.measures["z_spread"] - expected).abs() < 1e-9,
        "registry={} versus independent target={expected}",
        result.measures["z_spread"]
    );
}

#[test]
fn production_structured_metrics_settlement_crosses_coupon_once() {
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
        calculate_tranche_oas, OasConfig,
    };
    let (mut deal, market) = deal_and_market();
    let as_of = date!(2024 - 02 - 15);
    let settlement = date!(2024 - 05 - 15);
    deal.quote_settlement_date = Some(settlement);
    let tranche_id = deal.tranches.tranches[0].id.as_str();
    let flows = generate_tranche_cashflows(&deal, tranche_id, &market, as_of).expect("flows");
    let curve = market.get_discount("USD-OIS").expect("curve");
    let dirty: f64 = flows
        .cashflows
        .iter()
        .filter(|(date, _)| *date > settlement)
        .map(|(date, amount)| {
            amount.amount() * curve.df_between_dates(settlement, *date).expect("df")
        })
        .sum();
    let accrued = 100_000_000.0 * 0.06 * 44.0 / 360.0;
    let clean = (dirty - accrued) / 1_000_000.0;
    let metrics =
        calculate_tranche_metrics(&deal, tranche_id, &market, as_of, Some(clean)).expect("summary");
    assert!(metrics.z_spread_bp.abs() < 1e-6, "{}", metrics.z_spread_bp);
    assert!((metrics.target_price_pct - clean).abs() < 1e-10);
    assert!((metrics.spread_duration + metrics.cs01 / (dirty * 1e-4)).abs() < 1e-10);
    let result = deal
        .value_tranche_with_metrics(tranche_id, &market, as_of, &[])
        .expect("mandatory metrics");
    assert!((result.accrued.amount() - accrued).abs() < 1e-6);
    let ytm = result.ytm.expect("a performing note carries a yield");
    let expected = quarterly_yield(&deal, &market, as_of, result.dirty_price);
    assert!((ytm - expected).abs() < 1e-7, "{ytm} vs {expected}");
    assert!((result.clean_price - clean).abs() < 1e-9);
    let oas = calculate_tranche_oas(
        &deal,
        tranche_id,
        &market,
        as_of,
        clean,
        &OasConfig {
            stochastic_rates: false,
            stochastic_credit: false,
            ..Default::default()
        },
    )
    .expect("deterministic OAS");
    assert!(oas.oas.abs() < 1e-8, "{}", oas.oas);
    assert!((oas.model_price - clean).abs() < 1e-8);
    deal.instrument_pricing_overrides
        .market_quotes
        .quoted_dirty_price_currency = Some(dirty);
    let quoted = deal
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ZSpread, MetricId::Accrued],
            PricingOptions::default(),
        )
        .expect("dirty target");
    assert!(quoted.measures["z_spread"].abs() < 1e-8);
    assert!((quoted.measures["accrued"] - accrued).abs() < 1e-6);
}

#[test]
fn production_structured_metrics_opening_balance_and_unadjusted_boundary() {
    let (mut deal, market) = deal_and_market();
    deal.first_payment_date = date!(2024 - 06 - 30); // Sunday; payment adjusts to Friday June 28.
    deal.tranches.tranches[0].current_balance =
        finstack_quant_core::money::Money::new(80_000_000.0, deal.pool.get_currency())
            .expect("balance");
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.9);
    let as_of = date!(2024 - 05 - 15);
    let details =
        generate_tranche_cashflows(&deal, deal.tranches.tranches[0].id.as_str(), &market, as_of)
            .expect("flows");
    let first = &details.accrual_periods[0];
    assert_eq!(first.start, deal.closing_date);
    assert_eq!(first.end, date!(2024 - 06 - 30));
    assert_eq!(first.payment_date, date!(2024 - 06 - 28));
    assert!((first.opening_balance.amount() - 80_000_000.0).abs() < 1e-8);
    let expected = 80_000_000.0 * 0.06 * 135.0 / 360.0;
    assert!((first.accrued(as_of).expect("accrued") - expected).abs() < 1e-6);
    assert_eq!(first.accrued(first.payment_date).expect("paid coupon"), 0.0);
    let next = &details.accrual_periods[1];
    assert_eq!(next.start, date!(2024 - 06 - 30));
}

#[test]
fn production_structured_metrics_credit_model_risks_move_pv() {
    let (mut deal, market) = deal_and_market();
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.18);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.03);
    deal.credit_model.recovery_spec.rate = 0.55;
    let result = deal
        .price_with_metrics(
            &market,
            deal.closing_date,
            &[
                MetricId::Prepayment01,
                MetricId::Default01,
                MetricId::Recovery01,
            ],
            PricingOptions::default(),
        )
        .expect("credit-model risks");
    for metric in ["prepayment01", "default01", "recovery01"] {
        assert!(
            result.measures[metric].abs() > 1e-3,
            "{metric} must move PV"
        );
    }
}

#[test]
fn production_structured_metrics_discount_margin_settles_clean_price_at_quote_date() {
    use finstack_quant_cashflows::builder::FloatingRateSpec;
    use finstack_quant_core::{
        dates::{DayCount, Tenor},
        market_data::{scalars::ScalarTimeSeries, term_structures::ForwardCurve},
        types::CurveId,
    };
    use finstack_quant_valuations::instruments::fixed_income::loan_terms::RateSpec;
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::calculate_tranche_discount_margin;
    use rust_decimal_macros::dec;
    let (mut deal, market) = deal_and_market();
    let as_of = date!(2024 - 02 - 15);
    let settlement = date!(2024 - 05 - 15);
    deal.quote_settlement_date = Some(settlement);
    deal.tranches.tranches[0].coupon = RateSpec::Floating(FloatingRateSpec {
        forward_curve_id: CurveId::new("SOFR-3M"),
        spread_bp: dec!(100),
        gearing: dec!(1),
        gearing_includes_spread: true,
        index_floor_bp: None,
        all_in_floor_bp: None,
        all_in_cap_bp: None,
        index_cap_bp: None,
        overnight_index_constraints: Default::default(),
        reset_frequency: Tenor::quarterly(),
        index_tenor: None,
        reset_lag_days: 0,
        fixing_calendar_id: None,
        compounding: None,
        overnight_basis: None,
        fallback: Default::default(),
    });
    let market = market
        .insert(
            ForwardCurve::builder("SOFR-3M", 0.25)
                .base_date(deal.closing_date)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 0.05), (20.0, 0.05)])
                .build()
                .expect("forward"),
        )
        .insert_series(
            ScalarTimeSeries::new("FIXING:SOFR-3M", vec![(deal.closing_date, 0.05)], None)
                .expect("fixing"),
        );
    let id = deal.tranches.tranches[0].id.as_str();
    let flows = generate_tranche_cashflows(&deal, id, &market, as_of).expect("floating flows");
    let curve = market.get_discount("USD-OIS").expect("curve");
    let dirty: f64 = flows
        .cashflows
        .iter()
        .filter(|(date, _)| *date > settlement)
        .map(|(date, amount)| {
            amount.amount() * curve.df_between_dates(settlement, *date).expect("df")
        })
        .sum();
    // The solver takes a clean price per CURRENT face; strip the accrued
    // interest the buyer pays at settlement from the independent dirty value.
    let accrued: f64 = flows
        .accrual_periods
        .iter()
        .map(|period| period.accrued(settlement).expect("accrued"))
        .sum();
    let current = deal.tranches.tranches[0].current_balance.amount();
    let clean_pct = (dirty - accrued) / current * 100.0;
    let dm = calculate_tranche_discount_margin(&deal, id, &market, as_of, clean_pct).expect("DM");
    assert!(dm.abs() < 1e-10, "{dm}");
    assert!(calculate_tranche_discount_margin(&deal, id, &market, as_of, 0.0).is_err());
}

#[test]
fn production_structured_metrics_deferred_coupon_is_not_current_accrual() {
    let (mut deal, market) = deal_and_market();
    deal.tranches.tranches[0].deferred_interest =
        finstack_quant_core::money::Money::new(1_000_000.0, deal.pool.get_currency())
            .expect("deferred");
    let result = deal
        .value_tranche_with_metrics(
            deal.tranches.tranches[0].id.as_str(),
            &market,
            date!(2024 - 02 - 15),
            &[],
        )
        .expect("metrics");
    assert!((result.accrued.amount() - 750_000.0).abs() < 1e-6);
    let matured = deal
        .value_tranche_with_metrics(
            deal.tranches.tranches[0].id.as_str(),
            &market,
            deal.maturity + time::Duration::days(10),
            &[],
        )
        .expect("a matured note values at zero");
    assert!(
        matured.ytm.is_none(),
        "matured notes have no meaningful yield and must not fabricate one"
    );
    assert_eq!(matured.dirty_price, 0.0);
}
