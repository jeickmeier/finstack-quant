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
    use finstack_quant_core::types::CurveId;
    use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
    use time::macros::date;

    // Self-discounting swap: projection curve id == discount curve id.
    let mut swap = InterestRateSwap::example().expect("valid example swap");
    swap.float_leg.forward_curve_id = CurveId::new("USD-OIS");

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(date!(2024 - 01 - 02))
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (10.0, 0.70)])
        .build()
        .expect("valid discount curve");
    let market = MarketContext::new().insert(curve);

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
