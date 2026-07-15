//! Pricing tests for real estate assets.

use crate::finstack_quant_test_utils::date;
use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, StubKind, Tenor};
use finstack_quant_core::dates::{DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::equity::real_estate::{
    LeveredRealEstateEquity, RealEstateAsset, RealEstateValuationMethod,
};
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, RateSpec, TermLoan,
};
use finstack_quant_valuations::instruments::{Attributes, Bond, Instrument, InstrumentJson};

fn build_flat_discount_curve(
    id: &str,
    as_of: finstack_quant_core::dates::Date,
    rate: f64,
) -> DiscountCurve {
    // Simple flat curve with exp(-r t) discount factors.
    let knots = [
        (0.0, 1.0),
        (1.0, (-rate).exp()),
        (5.0, (-rate * 5.0).exp()),
        (30.0, (-rate * 30.0).exp()),
    ];
    DiscountCurve::builder(id)
        .base_date(as_of)
        .knots(knots)
        .build()
        .expect("flat discount curve should build")
}

#[test]
fn test_real_estate_dcf_pricing() {
    let valuation_date = date(2025, 1, 1);
    let noi1 = date(2026, 1, 1);
    let noi2 = date(2027, 1, 1);

    let asset = RealEstateAsset::builder()
        .id(InstrumentId::new("RE-DCF"))
        .currency(Currency::USD)
        .valuation_date(valuation_date)
        .valuation_method(RealEstateValuationMethod::Dcf)
        .noi_schedule(vec![(noi1, 100.0), (noi2, 100.0)])
        .discount_rate_opt(Some(0.10))
        .terminal_cap_rate_opt(Some(0.08))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let ctx = MarketContext::new();
    let pv = asset.value(&ctx, valuation_date).expect("npv");

    let t1 = DayCount::Act365F
        .year_fraction(valuation_date, noi1, DayCountContext::default())
        .unwrap();
    let t2 = DayCount::Act365F
        .year_fraction(valuation_date, noi2, DayCountContext::default())
        .unwrap();
    let pv_flows = 100.0 / (1.0_f64 + 0.10).powf(t1) + 100.0 / (1.0_f64 + 0.10).powf(t2);
    let terminal_value = 100.0 / 0.08;
    let pv_terminal = terminal_value / (1.0_f64 + 0.10).powf(t2);
    let expected = pv_flows + pv_terminal;

    // Allow small tolerance for floating point differences
    assert!(
        (pv.amount() - expected).abs() < 0.01,
        "PV={} vs expected={}",
        pv.amount(),
        expected
    );
}

#[test]
fn test_real_estate_direct_cap_pricing() {
    let valuation_date = date(2025, 1, 1);
    let noi1 = date(2026, 1, 1);

    let asset = RealEstateAsset::builder()
        .id(InstrumentId::new("RE-CAP"))
        .currency(Currency::USD)
        .valuation_date(valuation_date)
        .valuation_method(RealEstateValuationMethod::DirectCap)
        .noi_schedule(vec![(noi1, 120.0)])
        .cap_rate_opt(Some(0.06))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let ctx = MarketContext::new();
    let pv = asset.value(&ctx, valuation_date).expect("npv");

    let expected = 120.0 / 0.06;
    assert!((pv.amount() - expected).abs() < 1e-10);
}

#[test]
fn test_real_estate_direct_cap_uses_first_future_noi_when_not_stabilized() {
    let valuation_date = date(2025, 1, 1);
    let noi1 = date(2026, 1, 1);
    let noi2 = date(2027, 1, 1);

    let asset = RealEstateAsset::builder()
        .id(InstrumentId::new("RE-CAP-FIRST-NOI"))
        .currency(Currency::USD)
        .valuation_date(valuation_date)
        .valuation_method(RealEstateValuationMethod::DirectCap)
        .noi_schedule(vec![(noi1, 100.0), (noi2, 200.0)])
        .cap_rate_opt(Some(0.10))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let ctx = MarketContext::new();
    let pv = asset.value(&ctx, valuation_date).expect("npv");
    assert!((pv.amount() - (100.0 / 0.10)).abs() < 1e-10);
}

#[test]
fn test_real_estate_terminal_growth_applies_to_exit_value() {
    let valuation_date = date(2025, 1, 1);
    let noi1 = date(2026, 1, 1);

    let asset = RealEstateAsset::builder()
        .id(InstrumentId::new("RE-DCF-TV-GROWTH"))
        .currency(Currency::USD)
        .valuation_date(valuation_date)
        .valuation_method(RealEstateValuationMethod::Dcf)
        .noi_schedule(vec![(noi1, 100.0)])
        .discount_rate_opt(Some(0.10))
        .terminal_cap_rate_opt(Some(0.08))
        .terminal_growth_rate_opt(Some(0.02))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let ctx = MarketContext::new();
    let pv = asset.value(&ctx, valuation_date).expect("npv");

    let t1 = DayCount::Act365F
        .year_fraction(valuation_date, noi1, DayCountContext::default())
        .unwrap();
    let pv_flow = 100.0 / (1.0_f64 + 0.10).powf(t1);
    let terminal_value = (100.0 * 1.02) / 0.08;
    let pv_terminal = terminal_value / (1.0_f64 + 0.10).powf(t1);
    let expected = pv_flow + pv_terminal;
    assert!((pv.amount() - expected).abs() < 0.01);
}

/// DCF always discounts at the property `discount_rate`;
/// loading the named curve in the market context must not change the PV.
#[test]
fn test_real_estate_dcf_pv_identical_with_and_without_curve() {
    let valuation_date = date(2025, 1, 1);
    let noi1 = date(2026, 1, 1);

    let asset = RealEstateAsset::builder()
        .id(InstrumentId::new("RE-DCF-CURVE"))
        .currency(Currency::USD)
        .valuation_date(valuation_date)
        .valuation_method(RealEstateValuationMethod::Dcf)
        .noi_schedule(vec![(noi1, 100.0)])
        .discount_rate_opt(Some(0.10))
        .terminal_cap_rate_opt(Some(0.08))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let curve_rate = 0.05;
    let disc = build_flat_discount_curve("USD-OIS", valuation_date, curve_rate);
    let ctx_curve = MarketContext::new().insert(disc);
    let ctx_bare = MarketContext::new();

    let pv_curve = asset.value(&ctx_curve, valuation_date).expect("npv");
    let pv_bare = asset.value(&ctx_bare, valuation_date).expect("npv");
    assert!(
        (pv_curve.amount() - pv_bare.amount()).abs() < 1e-9,
        "PV must not depend on curve presence: with={}, without={}",
        pv_curve.amount(),
        pv_bare.amount()
    );

    // And the discount_rate-based PV must reconstruct exactly.
    let t1 = DayCount::Act365F
        .year_fraction(valuation_date, noi1, DayCountContext::default())
        .unwrap();
    let pv_flow = 100.0 / (1.0_f64 + 0.10).powf(t1);
    let terminal_value = 100.0 / 0.08;
    let pv_terminal = terminal_value / (1.0_f64 + 0.10).powf(t1);
    let expected = pv_flow + pv_terminal;

    assert!((pv_bare.amount() - expected).abs() < 0.01);
}

#[test]
fn test_real_estate_value_uses_as_of_for_filtering_flows() {
    let valuation_date = date(2025, 1, 1);
    let as_of = date(2026, 6, 1);
    let noi1 = date(2026, 1, 1);
    let noi2 = date(2027, 1, 1);

    let asset = RealEstateAsset::builder()
        .id(InstrumentId::new("RE-ASOF"))
        .currency(Currency::USD)
        .valuation_date(valuation_date)
        .valuation_method(RealEstateValuationMethod::Dcf)
        .noi_schedule(vec![(noi1, 100.0), (noi2, 100.0)])
        .discount_rate_opt(Some(0.10))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let ctx = MarketContext::new();
    let pv = asset.value(&ctx, as_of).expect("npv");

    // NOI1 is before as_of and should be filtered out.
    let t2 = DayCount::Act365F
        .year_fraction(as_of, noi2, DayCountContext::default())
        .unwrap();
    let expected = 100.0 / (1.0_f64 + 0.10).powf(t2);
    assert!((pv.amount() - expected).abs() < 0.01);
}

#[test]
fn test_real_estate_appraisal_override() {
    let valuation_date = date(2025, 1, 1);
    let noi1 = date(2026, 1, 1);

    let asset = RealEstateAsset::builder()
        .id(InstrumentId::new("RE-APPRAISAL"))
        .currency(Currency::USD)
        .valuation_date(valuation_date)
        .valuation_method(RealEstateValuationMethod::Dcf)
        .noi_schedule(vec![(noi1, 100.0)])
        .discount_rate_opt(Some(0.10))
        .appraisal_value_opt(Some(Money::new(1_500.0, Currency::USD)))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let ctx = MarketContext::new();
    let pv = asset.value(&ctx, valuation_date).expect("npv");

    assert_eq!(pv.amount(), 1_500.0);
}

#[test]
fn test_real_estate_custom_metrics_compute() {
    use finstack_quant_valuations::metrics::MetricId;

    let valuation_date = date(2025, 1, 1);
    let noi1 = date(2026, 1, 1);

    let asset = RealEstateAsset::builder()
        .id(InstrumentId::new("RE-METRICS"))
        .currency(Currency::USD)
        .valuation_date(valuation_date)
        .valuation_method(RealEstateValuationMethod::Dcf)
        .noi_schedule(vec![(noi1, 100.0)])
        .purchase_price_opt(Some(Money::new(1_000.0, Currency::USD)))
        .discount_rate_opt(Some(0.10))
        .terminal_cap_rate_opt(Some(0.10))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let market = MarketContext::new();
    let as_of = valuation_date;

    let metrics = [
        MetricId::custom("real_estate::going_in_cap_rate"),
        MetricId::custom("real_estate::exit_cap_rate"),
        MetricId::custom("real_estate::unlevered_multiple"),
    ];
    let result = asset
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("price_with_metrics");

    assert!(result
        .measures
        .contains_key(&MetricId::custom("real_estate::going_in_cap_rate")));
    assert!(result
        .measures
        .contains_key(&MetricId::custom("real_estate::exit_cap_rate")));
    assert!(result
        .measures
        .contains_key(&MetricId::custom("real_estate::unlevered_multiple")));
}

#[test]
fn test_real_estate_unlevered_metrics_include_acquisition_cost_line_items() {
    use finstack_quant_valuations::metrics::MetricId;

    let as_of = date(2025, 1, 1);
    let noi1 = date(2026, 1, 1);

    let asset = RealEstateAsset::builder()
        .id(InstrumentId::new("RE-METRICS-ACQ-COSTS"))
        .currency(Currency::USD)
        .valuation_date(as_of)
        .valuation_method(RealEstateValuationMethod::Dcf)
        .noi_schedule(vec![(noi1, 100.0)])
        .purchase_price_opt(Some(Money::new(1_000.0, Currency::USD)))
        .acquisition_costs(vec![Money::new(100.0, Currency::USD)])
        .discount_rate_opt(Some(0.0))
        .terminal_cap_rate_opt(Some(0.10))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let market = MarketContext::new();
    let metrics = [
        MetricId::custom("real_estate::unlevered_multiple"),
        MetricId::custom("real_estate::unlevered_cash_on_cash_first"),
    ];
    let result = asset
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("price_with_metrics");

    let multiple = *result
        .measures
        .get(&MetricId::custom("real_estate::unlevered_multiple"))
        .expect("unlevered multiple present");
    let coc = *result
        .measures
        .get(&MetricId::custom(
            "real_estate::unlevered_cash_on_cash_first",
        ))
        .expect("cash-on-cash present");

    // Inflows: NOI_1 (100) + terminal sale (100/0.10 = 1000) = 1100
    // Outflow: purchase (1000) + acquisition line items (100) = 1100
    assert!((multiple - 1.0).abs() < 1e-10, "multiple={multiple}");

    // First-period cash-on-cash uses denominator purchase + acquisition total.
    assert!((coc - (100.0 / 1_100.0)).abs() < 1e-12, "coc={coc}");
}

#[test]
fn test_real_estate_terminal_only_sale_price_is_allowed() {
    let as_of = date(2025, 1, 1);
    let noi1 = date(2026, 1, 1);

    // Sale happens before the first NOI date, so there are no flows on/before horizon.
    let sale_date = date(2025, 6, 1);

    let sale_price = Money::new(1_000.0, Currency::USD);
    let disposition_cost_pct = 0.10; // 10%
    let disposition_costs = vec![Money::new(50.0, Currency::USD)];
    let net_sale = sale_price.amount() * (1.0 - disposition_cost_pct) - 50.0;

    let asset = RealEstateAsset::builder()
        .id(InstrumentId::new("RE-TERMINAL-ONLY"))
        .currency(Currency::USD)
        .valuation_date(as_of)
        .valuation_method(RealEstateValuationMethod::Dcf)
        .noi_schedule(vec![(noi1, 100.0)])
        .discount_rate_opt(Some(0.10))
        .sale_date_opt(Some(sale_date))
        .sale_price_opt(Some(sale_price))
        .disposition_cost_pct_opt(Some(disposition_cost_pct))
        .disposition_costs(disposition_costs)
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("asset build");

    let market = MarketContext::new(); // curve-free, uses discount_rate
    let pv = asset.value(&market, as_of).expect("npv");

    let t = DayCount::Act365F
        .year_fraction(as_of, sale_date, DayCountContext::default())
        .unwrap();
    let expected = net_sale / (1.0_f64 + 0.10).powf(t);

    assert!(
        (pv.amount() - expected).abs() < 0.01,
        "PV={} vs expected={}",
        pv.amount(),
        expected
    );
}

#[test]
fn test_real_estate_sensitivities_metrics_compute_and_have_expected_signs() {
    use finstack_quant_valuations::metrics::MetricId;

    let as_of = date(2025, 1, 1);
    let noi1 = date(2026, 1, 1);
    let noi2 = date(2027, 1, 1);

    let asset = RealEstateAsset::builder()
        .id(InstrumentId::new("RE-SENS"))
        .currency(Currency::USD)
        .valuation_date(as_of)
        .valuation_method(RealEstateValuationMethod::Dcf)
        .noi_schedule(vec![(noi1, 100.0), (noi2, 100.0)])
        .discount_rate_opt(Some(0.10))
        .terminal_cap_rate_opt(Some(0.08))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("asset build");

    let market = MarketContext::new(); // curve-free so discount_rate is used

    let metrics = [
        MetricId::custom("real_estate::cap_rate_sensitivity"),
        MetricId::custom("real_estate::discount_rate_sensitivity"),
    ];
    let result = asset
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("price_with_metrics");

    let d_v_d_cap = *result
        .measures
        .get(&MetricId::custom("real_estate::cap_rate_sensitivity"))
        .expect("cap rate sens present");
    let d_v_d_r = *result
        .measures
        .get(&MetricId::custom("real_estate::discount_rate_sensitivity"))
        .expect("discount rate sens present");

    // Higher cap rates / discount rates should reduce value.
    assert!(d_v_d_cap < 0.0, "cap sensitivity should be negative");
    assert!(
        d_v_d_r < 0.0,
        "discount-rate sensitivity should be negative"
    );
}

#[test]
fn test_levered_real_estate_equity_value_is_asset_minus_debt() {
    let as_of = date(2025, 1, 1);
    let noi1 = date(2026, 1, 1);
    let noi2 = date(2027, 1, 1);

    let asset = RealEstateAsset::builder()
        .id(InstrumentId::new("RE-ASSET"))
        .currency(Currency::USD)
        .valuation_date(as_of)
        .valuation_method(RealEstateValuationMethod::Dcf)
        .noi_schedule(vec![(noi1, 100.0), (noi2, 100.0)])
        .purchase_price_opt(Some(Money::new(1_000.0, Currency::USD)))
        .discount_rate_opt(Some(0.10))
        .terminal_cap_rate_opt(Some(0.10))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("asset build");

    let loan = TermLoan::builder()
        .id("TL-RE-001".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(600.0, Currency::USD))
        .issue_date(as_of)
        .maturity(noi2)
        .rate(RateSpec::Fixed { rate_bp: 500 }) // 5%
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .instrument_pricing_overrides(Default::default())
        .attributes(Default::default())
        .build()
        .expect("loan build");

    let bond = Bond::example().unwrap();

    let levered = LeveredRealEstateEquity::builder()
        .id(InstrumentId::new("RE-EQ-L"))
        .currency(Currency::USD)
        .asset(asset.clone())
        .financing(vec![
            InstrumentJson::TermLoan(loan.clone()),
            InstrumentJson::Bond(bond.clone()),
        ])
        .exit_date_opt(Some(noi2))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("levered build");

    let disc_ois = build_flat_discount_curve("USD-OIS", as_of, 0.05);
    let disc_tsy = build_flat_discount_curve("USD-TREASURY", as_of, 0.05);
    let market = MarketContext::new().insert(disc_ois).insert(disc_tsy);

    let pv_asset = asset.value(&market, as_of).expect("asset pv").amount();
    let pv_fin = loan.value(&market, as_of).expect("loan pv").amount()
        + bond.value(&market, as_of).expect("bond pv").amount();
    let pv_eq = levered.value(&market, as_of).expect("eq pv").amount();

    let diff = pv_eq - (pv_asset - pv_fin);
    assert!(
        // Money amounts may be rounded to currency minor units in different pricing paths.
        diff.abs() < 1e-2,
        "expected pv_eq == pv_asset - pv_financing (diff={diff}); pv_asset={pv_asset}, pv_fin={pv_fin}, pv_eq={pv_eq}"
    );
}

#[test]
fn test_levered_real_estate_equity_custom_metrics_compute() {
    use finstack_quant_valuations::metrics::MetricId;

    let as_of = date(2025, 1, 1);
    let noi1 = date(2026, 1, 1);
    let noi2 = date(2027, 1, 1);

    let asset = RealEstateAsset::builder()
        .id(InstrumentId::new("RE-ASSET-2"))
        .currency(Currency::USD)
        .valuation_date(as_of)
        .valuation_method(RealEstateValuationMethod::Dcf)
        .noi_schedule(vec![(noi1, 120.0), (noi2, 120.0)])
        .purchase_price_opt(Some(Money::new(1_000.0, Currency::USD)))
        .discount_rate_opt(Some(0.10))
        .terminal_cap_rate_opt(Some(0.10))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("asset build");

    let loan = TermLoan::builder()
        .id("TL-RE-002".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(700.0, Currency::USD))
        .issue_date(as_of)
        .maturity(noi2)
        .rate(RateSpec::Fixed { rate_bp: 600 }) // 6%
        .frequency(Tenor::annual())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .instrument_pricing_overrides(Default::default())
        .attributes(Default::default())
        .build()
        .expect("loan build");

    let levered = LeveredRealEstateEquity::builder()
        .id(InstrumentId::new("RE-EQ-L-2"))
        .currency(Currency::USD)
        .asset(asset)
        .financing(vec![InstrumentJson::TermLoan(loan)])
        .exit_date_opt(Some(noi2))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("levered build");

    let market = MarketContext::new().insert(build_flat_discount_curve("USD-OIS", as_of, 0.05));

    let metrics = [
        MetricId::custom("real_estate::levered_irr"),
        MetricId::custom("real_estate::equity_multiple"),
        MetricId::custom("real_estate::ltv"),
        MetricId::custom("real_estate::ltv_at_origination"),
        MetricId::custom("real_estate::dscr_min"),
        MetricId::custom("real_estate::dscr_min_interest_only"),
        MetricId::custom("real_estate::debt_payoff_at_exit"),
    ];

    let result = levered
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("price_with_metrics");

    for m in metrics {
        let v = *result.measures.get(&m).expect("metric present");
        assert!(v.is_finite(), "metric {} should be finite", m.as_str());
    }
}

#[test]
fn test_levered_real_estate_sensitivities_metrics_compute() {
    use finstack_quant_valuations::metrics::MetricId;

    let as_of = date(2025, 1, 1);
    let noi1 = date(2026, 1, 1);
    let noi2 = date(2027, 1, 1);

    let asset = RealEstateAsset::builder()
        .id(InstrumentId::new("RE-ASSET-SENS-L"))
        .currency(Currency::USD)
        .valuation_date(as_of)
        .valuation_method(RealEstateValuationMethod::Dcf)
        .noi_schedule(vec![(noi1, 120.0), (noi2, 120.0)])
        .purchase_price_opt(Some(Money::new(1_000.0, Currency::USD)))
        .discount_rate_opt(Some(0.10))
        .terminal_cap_rate_opt(Some(0.09))
        .day_count(DayCount::Act365F)
        // Keep the asset curve ID distinct so the asset remains curve-free (discount_rate is used),
        // while the financing instruments can still use USD-OIS from the market.
        .discount_curve_id(CurveId::new("USD-RE-DISC"))
        .attributes(Attributes::new())
        .build()
        .expect("asset build");

    let loan = TermLoan::builder()
        .id("TL-RE-SENS".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(700.0, Currency::USD))
        .issue_date(as_of)
        .maturity(noi2)
        .rate(RateSpec::Fixed { rate_bp: 600 }) // 6%
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .instrument_pricing_overrides(Default::default())
        .attributes(Default::default())
        .build()
        .expect("loan build");

    let levered = LeveredRealEstateEquity::builder()
        .id(InstrumentId::new("RE-EQ-SENS-L"))
        .currency(Currency::USD)
        .asset(asset)
        .financing(vec![InstrumentJson::TermLoan(loan)])
        .exit_date_opt(Some(noi2))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("levered build");

    // Provide USD-OIS for financing PV, but keep the asset curve absent (USD-RE-DISC not in market).
    let market = MarketContext::new().insert(build_flat_discount_curve("USD-OIS", as_of, 0.05));

    let metrics = [
        MetricId::custom("real_estate::cap_rate_sensitivity"),
        MetricId::custom("real_estate::discount_rate_sensitivity"),
    ];
    let result = levered
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("price_with_metrics");

    for m in metrics {
        let v = *result.measures.get(&m).expect("metric present");
        assert!(v.is_finite(), "metric {} should be finite", m.as_str());
    }
}
