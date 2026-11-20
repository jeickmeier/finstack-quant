//! Unified DV01 over a built-in swap.

use finstack_quant_valuations::metrics::sensitivities::dv01::{
    Dv01CalculatorConfig, UnifiedDv01Calculator,
};

/// A curve that serves as both the discount and the projection curve of a
/// self-discounting swap must be bumped exactly ONCE per direction, or
/// parallel DV01 doubles (a 1bp request delivers a 2bp shock).
///
/// `MarketContext` stores one `CurveStorage` enum variant per `CurveId`, so
/// `get_discount(id)` and `get_forward(id)` are mutually exclusive and
/// `collect_curves` can never emit the same id under both roles. This test
/// pins that invariant: if curve storage ever becomes dual-role, the
/// duplicate would reach the in-place bump loop (where same-id bumps stack
/// additively) and this test fails before DV01 silently doubles.
#[test]
fn collect_curves_never_duplicates_a_dual_role_curve_id() {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
    use time::macros::date;

    // The OIS projection uses the discount evaluator for overnight compounding.
    let mut swap = InterestRateSwap::example().expect("valid example swap");
    swap.float_leg.compounding =
        finstack_quant_cashflows::builder::FloatingLegCompounding::CompoundedInArrears {
            lookback_days: 0,
        };
    swap.float_leg.forward_curve_id = swap.fixed_leg.discount_curve_id.clone();

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(date!(2024 - 01 - 02))
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (10.0, 0.70)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("valid discount curve");
    let market = MarketContext::new()
        .try_insert(curve)
        .expect("insert market curve");

    let calc = UnifiedDv01Calculator::new(Dv01CalculatorConfig::parallel_combined());
    let curves = calc
        .collect_curves(&swap, &market)
        .expect("dual-role curve must still resolve for DV01");

    let mut ids: Vec<&str> = curves.iter().map(|(id, _)| id.as_str()).collect();
    ids.sort_unstable();
    let unique = {
        let mut deduped = ids.clone();
        deduped.dedup();
        deduped
    };
    assert_eq!(
        ids, unique,
        "collect_curves must never emit duplicate curve ids (each physical \
         curve is bumped once per direction): {ids:?}"
    );
    assert_eq!(
        curves.len(),
        1,
        "self-discounting swap has exactly one physical rate curve, got {curves:?}"
    );
}

#[test]
fn seasoned_discount_key_rate_is_anchored_at_valuation() {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::bumps::BumpSpec;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::instruments::rates::deposit::Deposit;
    use finstack_quant_valuations::metrics::MetricId;
    use time::macros::date;

    let base = date!(2024 - 01 - 03);
    let valuation = date!(2025 - 01 - 02);
    let payment = date!(2026 - 01 - 02);
    let mut deposit = Deposit::example().expect("deposit");
    deposit.maturity = payment;
    deposit.calendar_id = None;
    let curve = DiscountCurve::builder(deposit.discount_curve_id.clone())
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.93), (3.0, 0.87)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let market = MarketContext::new().try_insert(curve).expect("market");
    let conditioned = market.condition(valuation).expect("condition at valuation");
    let bump = BumpSpec::triangular_key_rate_bucket_bp(Some(0.5), 1.0, Some(2.0), 1.0);
    let mut seasoned_bumped = market.clone();
    seasoned_bumped
        .apply_curve_bump_in_place(&deposit.discount_curve_id, bump)
        .expect("apply seasoned shock");
    let mut conditioned_bumped = conditioned.clone();
    conditioned_bumped
        .apply_curve_bump_in_place(&deposit.discount_curve_id, bump)
        .expect("apply conditioned shock");
    let pv = |market| {
        deposit
            .value(market, valuation)
            .expect("deposit PV")
            .amount()
    };
    assert!(pv(&seasoned_bumped) > pv(&market));
    assert!(pv(&conditioned_bumped) < pv(&conditioned));

    let result = deposit
        .price_with_metrics(
            &market,
            valuation,
            &[MetricId::BucketedDv01],
            Default::default(),
        )
        .expect("key-rate DV01");
    let key = format!("bucketed_dv01::{}::1y", deposit.discount_curve_id);
    assert!(
        result.measures[key.as_str()] < 0.0,
        "{key} = {}",
        result.measures[key.as_str()]
    );
}
