//! The exported working behind an attribution must reproduce its numbers.
//!
//! `PnlAttribution` carries the two endpoint present values and, per method,
//! the ordered steps that produced each factor bucket. These tests rebuild the
//! reported buckets from those rows alone: waterfall step values chain from
//! `pv_t0`, and each sensitivity row's `sensitivity × market_move` (or its
//! repriced value) gives the row's P&L.

use finstack_quant_attribution::{
    attribute_pnl, default_waterfall_order, AttributionFactor, AttributionMethod,
    AttributionRequest, ExecutionPolicy, PnlAttribution, TaylorAttributionConfig,
};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{create_date, Date};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use std::sync::Arc;
use time::Month;

/// Relative-plus-absolute closeness for amounts rebuilt from exported rows.
fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-7 * (1.0 + expected.abs())
}

fn endpoints(attribution: &PnlAttribution) -> (f64, f64, f64) {
    let pv_t0 = attribution.pv_t0.expect("pv_t0 exported").amount();
    let pv_t1 = attribution.pv_t1.expect("pv_t1 exported").amount();
    let mtm = attribution
        .mark_to_market_pnl
        .expect("mark-to-market exported")
        .amount();
    assert!(
        close(pv_t1 - pv_t0, mtm),
        "mark_to_market_pnl {mtm} must equal pv_t1 − pv_t0 = {}",
        pv_t1 - pv_t0
    );
    // Period cash receipts the total-return convention adds to `total_pnl`.
    let cash = attribution.total_pnl.amount() - mtm;
    (pv_t0, pv_t1, cash)
}

/// Rebuild every waterfall bucket from `waterfall_steps`.
pub(super) fn assert_waterfall_steps_reconcile(attribution: &PnlAttribution) {
    let AttributionMethod::Waterfall(order) = &attribution.meta.method else {
        panic!("expected a waterfall attribution");
    };
    let (pv_t0, pv_t1, cash) = endpoints(attribution);
    let steps = &attribution.waterfall_steps;
    assert_eq!(steps.len(), order.len(), "one row per applied factor");
    assert!(
        close(steps[0].pv_before.amount(), pv_t0),
        "the first step must start from pv_t0"
    );

    for (index, step) in steps.iter().enumerate() {
        assert_eq!(step.step_index, index);
        assert_eq!(step.factor, order[index], "rows follow the applied order");
        assert!(
            close(
                step.pv_after.amount() - step.pv_before.amount(),
                step.step_pnl.amount()
            ),
            "{:?}: step_pnl must equal pv_after − pv_before",
            step.factor
        );
        if let Some(next) = steps.get(index + 1) {
            assert_eq!(step.pv_after, next.pv_before, "step values must chain");
        }
        let (bucket, expected) = match step.factor {
            AttributionFactor::Carry => (attribution.carry, step.step_pnl.amount() + cash),
            AttributionFactor::RatesCurves => {
                (attribution.rates_curves_pnl, step.step_pnl.amount())
            }
            AttributionFactor::CreditCurves => {
                (attribution.credit_curves_pnl, step.step_pnl.amount())
            }
            AttributionFactor::InflationCurves => {
                (attribution.inflation_curves_pnl, step.step_pnl.amount())
            }
            AttributionFactor::Correlations => {
                (attribution.correlations_pnl, step.step_pnl.amount())
            }
            AttributionFactor::Fx => (attribution.fx_pnl, step.step_pnl.amount()),
            AttributionFactor::Volatility => (attribution.vol_pnl, step.step_pnl.amount()),
            AttributionFactor::ModelParameters => {
                (attribution.model_params_pnl, step.step_pnl.amount())
            }
            AttributionFactor::MarketScalars => {
                (attribution.market_scalars_pnl, step.step_pnl.amount())
            }
        };
        assert!(
            close(bucket.amount(), expected),
            "{:?}: bucket {} must be rebuilt from its step ({expected})",
            step.factor,
            bucket.amount()
        );
    }

    // The chain ends at pv_t1 less whatever the waterfall left unexplained.
    let last = steps.last().expect("at least one step");
    assert!(
        close(
            last.pv_after.amount() + attribution.residual.amount(),
            pv_t1
        ),
        "final step value {} + residual {} must reach pv_t1 {pv_t1}",
        last.pv_after.amount(),
        attribution.residual.amount()
    );
    let step_sum: f64 = steps.iter().map(|step| step.step_pnl.amount()).sum();
    assert!(
        close(
            step_sum + cash + attribution.residual.amount(),
            attribution.total_pnl.amount()
        ),
        "step P&Ls + period cash + residual must sum to total_pnl"
    );
}

/// Rebuild every Taylor or metrics-based bucket from `sensitivity_steps`.
pub(super) fn assert_sensitivity_steps_reconcile(attribution: &PnlAttribution) {
    let (pv_t0, pv_t1, cash) = endpoints(attribution);
    assert!(
        !attribution.sensitivity_steps.is_empty(),
        "a sensitivity-based attribution must export its factor rows"
    );

    let mut rates = 0.0;
    let mut credit = 0.0;
    let mut vol = 0.0;
    let mut fx = 0.0;
    let mut inflation = 0.0;
    let mut correlations = 0.0;
    let mut scalars = 0.0;
    let mut model_params = 0.0;
    let mut carry = 0.0;

    for step in &attribution.sensitivity_steps {
        let explained = step.explained_pnl.amount();
        if !step.buckets.is_empty() {
            assert!(step.move_unit.is_some(), "{}: unit stated", step.factor);
            assert!(step.sensitivity.is_none() && step.repriced_pv.is_none());
            let rebuilt: f64 = step
                .buckets
                .iter()
                .map(|bucket| bucket.sensitivity.amount() * bucket.market_move)
                .sum();
            assert!(
                close(rebuilt, explained),
                "{}: Σ bucket sensitivity × move = {rebuilt}, reported {explained}",
                step.factor
            );
        } else if let (Some(sensitivity), Some(market_move)) = (step.sensitivity, step.market_move)
        {
            assert!(step.move_unit.is_some(), "{}: unit stated", step.factor);
            assert!(step.repriced_pv.is_none());
            let rebuilt = sensitivity.amount() * market_move;
            assert!(
                close(rebuilt, explained),
                "{}: sensitivity × move = {rebuilt}, reported {explained}",
                step.factor
            );
        } else if let Some(market_move) = step.market_move {
            // Second-order only: the move is exported, the P&L is all gamma.
            assert!(market_move.is_finite() && step.move_unit.is_some());
            assert!(step.gamma_pnl.is_some(), "{}: gamma reported", step.factor);
            assert_eq!(explained, 0.0, "{}: no first-order amount", step.factor);
        } else {
            let repriced = step
                .repriced_pv
                .unwrap_or_else(|| panic!("{}: row exports no working", step.factor))
                .amount();
            let rebuilt = if step.factor == "Theta" {
                repriced - pv_t0 + cash
            } else {
                pv_t1 - repriced
            };
            assert!(
                close(rebuilt, explained),
                "{}: repriced working gives {rebuilt}, reported {explained}",
                step.factor
            );
        }

        let contribution = explained + step.gamma_pnl.map_or(0.0, |gamma| gamma.amount());
        let bucket = match step.factor.as_str() {
            "Theta" => &mut carry,
            "Fx" => &mut fx,
            "Inflation" | "InflationConvexity" => &mut inflation,
            "Correlations" => &mut correlations,
            "MarketScalars" | "Spot" | "SpotGamma" => &mut scalars,
            "ModelParameters" => &mut model_params,
            "Rates" | "RatesConvexity" => &mut rates,
            "Credit" | "CreditGamma" => &mut credit,
            "Vol" | "Volga" => &mut vol,
            name if name.starts_with("Rates:") || name.starts_with("Forward:") => &mut rates,
            name if name.starts_with("Credit:") => &mut credit,
            name if name.starts_with("Vol:") => &mut vol,
            other => panic!("unexpected sensitivity row '{other}'"),
        };
        *bucket += contribution;
    }

    // Metrics-based carry is read directly from the T0 carry metrics and has
    // no row; every other method's carry is its `Theta` row.
    if matches!(attribution.meta.method, AttributionMethod::MetricsBased) {
        carry = attribution.carry.amount();
    }
    for (name, rebuilt, reported) in [
        ("carry", carry, attribution.carry),
        ("rates_curves_pnl", rates, attribution.rates_curves_pnl),
        ("credit_curves_pnl", credit, attribution.credit_curves_pnl),
        ("vol_pnl", vol, attribution.vol_pnl),
        ("fx_pnl", fx, attribution.fx_pnl),
        (
            "inflation_curves_pnl",
            inflation,
            attribution.inflation_curves_pnl,
        ),
        (
            "correlations_pnl",
            correlations,
            attribution.correlations_pnl,
        ),
        (
            "market_scalars_pnl",
            scalars,
            attribution.market_scalars_pnl,
        ),
        (
            "model_params_pnl",
            model_params,
            attribution.model_params_pnl,
        ),
    ] {
        assert!(
            close(rebuilt, reported.amount()),
            "{name}: rows sum to {rebuilt}, reported {}",
            reported.amount()
        );
    }
}

fn flat_curve(base_date: Date, rate: f64) -> DiscountCurve {
    let knots: Vec<(f64, f64)> = (0..=20)
        .map(|i| {
            let t = f64::from(i) * 0.5;
            (t, (-rate * t).exp())
        })
        .collect();
    DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(knots)
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("flat discount curve")
}

/// A 5% semi-annual bond attributed across its 2025-07-01 coupon date, with
/// rates rising 25bp, so both the market move and the period cash matter.
fn attribute_bond_over_coupon(method: AttributionMethod) -> PnlAttribution {
    let as_of_t0 = create_date(2025, Month::June, 27).expect("t0");
    let as_of_t1 = create_date(2025, Month::July, 3).expect("t1");
    let bond = Bond::fixed(
        "US-BOND-AUDIT",
        Money::new(1_000_000.0, Currency::USD).expect("notional"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("coupon"),
        create_date(2025, Month::January, 1).expect("issue"),
        create_date(2030, Month::January, 1).expect("maturity"),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    let instrument: Arc<dyn Instrument> = Arc::new(bond);
    let market_t0 = MarketContext::new().insert(flat_curve(as_of_t0, 0.0400));
    let market_t1 = MarketContext::new().insert(flat_curve(as_of_t1, 0.0425));
    let config = FinstackConfig::default();
    attribute_pnl(
        &method,
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Serial,
            ..AttributionRequest::new(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &config,
            )
        },
    )
    .expect("attribution")
}

fn json_roundtrip(attribution: &PnlAttribution) -> PnlAttribution {
    let json = serde_json::to_string(attribution).expect("serialize");
    serde_json::from_str(&json).expect("deserialize")
}

#[test]
fn waterfall_steps_chain_from_pv_t0_and_rebuild_every_bucket() {
    let mut attribution =
        attribute_bond_over_coupon(AttributionMethod::Waterfall(default_waterfall_order()));
    let (_, _, cash) = endpoints(&attribution);
    assert!(
        cash > 20_000.0,
        "the period must contain the semi-annual coupon, got cash {cash}"
    );
    assert_eq!(attribution.waterfall_steps.len(), 9);
    assert!(attribution.sensitivity_steps.is_empty());
    assert!(
        attribution.waterfall_steps[1].step_pnl.amount() < 0.0,
        "the rates step must lose money when rates rise"
    );
    assert_waterfall_steps_reconcile(&attribution);

    // The rows survive the wire unchanged.
    let restored = json_roundtrip(&attribution);
    assert_eq!(restored.waterfall_steps, attribution.waterfall_steps);
    assert_eq!(restored.pv_t0, attribution.pv_t0);
    assert_eq!(restored.pv_t1, attribution.pv_t1);

    // Position scaling (here a short of 2.5 units) keeps the rows reconciled.
    attribution.scale(-2.5).expect("scale");
    assert_waterfall_steps_reconcile(&attribution);
    attribution
        .validate_currencies()
        .expect("step amounts share the attribution currency");
}

#[test]
fn taylor_sensitivity_steps_rebuild_every_bucket() {
    let mut attribution =
        attribute_bond_over_coupon(AttributionMethod::Taylor(TaylorAttributionConfig {
            include_gamma: true,
            ..TaylorAttributionConfig::default()
        }));
    assert!(attribution.waterfall_steps.is_empty());

    let rates = attribution
        .sensitivity_steps
        .iter()
        .find(|step| step.factor == "Rates:USD-OIS")
        .expect("rates row");
    assert_eq!(rates.buckets.len(), 11, "one bucket per key-rate tenor");
    assert_eq!(
        rates.move_unit,
        Some(finstack_quant_attribution::MoveUnit::BasisPoint)
    );
    assert!(
        rates
            .buckets
            .iter()
            .all(|bucket| close(bucket.market_move, 25.0)),
        "a flat +25bp zero-rate move must be observed at every tenor"
    );
    assert!(rates.explained_pnl.amount() < 0.0);
    assert!(
        rates.gamma_pnl.expect("gamma requested").amount() > 0.0,
        "a long bond has positive rate convexity"
    );
    let theta = attribution
        .sensitivity_steps
        .iter()
        .find(|step| step.factor == "Theta")
        .expect("theta row");
    assert!(theta.repriced_pv.is_some());
    assert_sensitivity_steps_reconcile(&attribution);

    let restored = json_roundtrip(&attribution);
    assert_eq!(restored.sensitivity_steps, attribution.sensitivity_steps);

    attribution.scale(-2.5).expect("scale");
    assert_sensitivity_steps_reconcile(&attribution);
    attribution
        .validate_currencies()
        .expect("row amounts share the attribution currency");
}

#[test]
fn parallel_attribution_exports_endpoints_without_steps() {
    let attribution = attribute_bond_over_coupon(AttributionMethod::Parallel);
    let (pv_t0, pv_t1, cash) = endpoints(&attribution);
    assert!(pv_t0 > 0.0 && pv_t1 > 0.0);
    assert!(close(attribution.total_pnl.amount(), pv_t1 - pv_t0 + cash));
    assert!(attribution.waterfall_steps.is_empty());
    assert!(attribution.sensitivity_steps.is_empty());
}

#[test]
fn payload_without_audit_fields_still_deserializes() {
    let attribution =
        attribute_bond_over_coupon(AttributionMethod::Waterfall(default_waterfall_order()));
    let mut value = serde_json::to_value(&attribution).expect("serialize");
    let object = value.as_object_mut().expect("object");
    for field in ["pv_t0", "pv_t1", "waterfall_steps"] {
        assert!(
            object.remove(field).is_some(),
            "{field} must be on the wire"
        );
    }
    assert!(
        !object.contains_key("sensitivity_steps"),
        "an empty row list is omitted from the wire"
    );
    let legacy: PnlAttribution = serde_json::from_value(value).expect("legacy payload");
    assert!(legacy.pv_t0.is_none() && legacy.pv_t1.is_none());
    assert!(legacy.waterfall_steps.is_empty() && legacy.sensitivity_steps.is_empty());
    assert_eq!(legacy.total_pnl, attribution.total_pnl);
}
