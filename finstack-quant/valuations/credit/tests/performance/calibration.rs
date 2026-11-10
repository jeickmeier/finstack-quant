//! Test-only PV budgets for the frozen five-knot calibration control.
//!
//! The calibration crate exports the canonical contracts and final market.
//! Reading that fixture at run time lets an archived baseline test binary and
//! a candidate binary price the same two calibrated curves, without adding a
//! production calibration dependency or retaining an old pricing path.

use super::CdsTranchePricer;
use crate::cds_tranche::CdsTranche;
use finstack_quant_core::canonical::content_hash;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

const BENCHMARK: &str = "calibration_base_correlation_full_plan_5_knots";
const INPUT_FIXTURE: &str = "valuation_b4_calibration.json";
const GAUSSIAN_CAP_BUDGET: f64 = 2e-10;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CalibrationInputs {
    schema_version: u32,
    benchmark: String,
    envelope_hash: String,
    as_of: Date,
    market: MarketContext,
    cases: Vec<CalibrationCase>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CalibrationCase {
    id: String,
    role: String,
    quote_id: Option<String>,
    instrument: CdsTranche,
}

fn gamma(operations: usize) -> f64 {
    let scaled_epsilon = operations as f64 * f64::EPSILON;
    assert!(scaled_epsilon < 1.0);
    scaled_epsilon / (1.0 - scaled_epsilon)
}

fn case_record(case: &CalibrationCase, inputs: &CalibrationInputs) -> Value {
    let pricer = CdsTranchePricer::new();
    let tranche = &case.instrument;
    let as_of = inputs.as_of;
    let market = &inputs.market;
    tranche.validate().expect("canonical calibration contract");
    assert_eq!(tranche.realized_loss, 0.0);
    let notional = tranche.notional.amount();
    assert!(notional.is_finite() && notional > 0.0);
    let coupon = tranche.coupon_bp / 10_000.0;
    assert!(coupon.is_finite());
    let projection = pricer
        .prepare_projection_inputs(tranche, market, as_of)
        .expect("actual calibration projection inputs");
    let el_wd = projection
        .el_curve(&pricer, projection.index.base_correlation_curve())
        .expect("actual calibration EL/WD curve");
    let index = projection.index;
    let settlement = projection.valuation_date;
    let periods = projection.periods;
    assert!(index.issuer_credit_curves().is_none());
    assert_eq!(index.num_constituents(), 125);
    assert_eq!(periods.len(), el_wd.len());
    assert!(!periods.is_empty());
    assert!(
        periods
            .windows(2)
            .all(|pair| pair[0].accrual_end <= pair[1].accrual_end)
    );
    let effective = pricer.calculate_effective_structure(tranche, index.recovery_rate());
    let width = (tranche.detach_pct - tranche.attach_pct) / 100.0;
    assert!(width > 0.0 && effective.pool_factor > 0.0);
    // Each EL or WD is a difference of two capped pool expectations, then
    // scaled from pool notional to the original tranche notional. The frozen
    // control has no realized losses or recovery erosion.
    let period_error_bound = 2.0 * GAUSSIAN_CAP_BUDGET * effective.pool_factor / width;
    let discount = market
        .get_discount(&tranche.discount_curve_id)
        .expect("frozen discount curve");
    let mut coefficient_sum = 0.0;
    let mut absolute_cashflow_scale = 0.0;
    let mut suppression_discount_sum = 0.0;
    let records: Vec<_> = periods
        .iter()
        .zip(&el_wd)
        .map(|(period, point)| {
            assert!(period.accrual_end > as_of);
            assert!(point.el_fraction.is_finite() && point.wd_fraction.is_finite());
            assert!(point.el_fraction >= 0.0 && point.wd_fraction >= 0.0);
            assert!(point.el_fraction + point.wd_fraction <= 1.0 + 1e-12);
            let protection_start = period.accrual_start.max(as_of).min(period.accrual_end);
            let prior_time = if protection_start <= as_of {
                0.0
            } else {
                pricer
                    .years_from_base(&index, protection_start)
                    .expect("protection-start time")
            };
            let end_time = pricer
                .years_from_base(&index, period.accrual_end)
                .expect("accrual-end time");
            let default_fraction =
                pricer.within_period_default_fraction(&index, prior_time, end_time);
            let remaining_accrual = tranche
                .day_count
                .year_fraction(
                    protection_start,
                    period.accrual_end,
                    DayCountContext::default(),
                )
                .expect("remaining accrual fraction");
            let days = (period.accrual_end - protection_start)
                .whole_days()
                .max(0);
            let default_date = protection_start
                .checked_add(time::Duration::days(
                    (default_fraction * days as f64).round() as i64,
                ))
                .unwrap_or(period.accrual_end)
                .clamp(as_of, period.accrual_end);
            let discount_payment = discount
                .df_between_dates(as_of, period.payment_date)
                .expect("payment discount factor");
            let discount_default = discount
                .df_between_dates(as_of, default_date)
                .expect("default discount factor");
            assert!((0.0..=1.0).contains(&default_fraction));
            assert!(remaining_accrual >= 0.0 && period.accrual_year_fraction >= 0.0);
            assert!(discount_payment > 0.0 && discount_default > 0.0);
            // Protection uses an EL difference (two component errors).
            // Scheduled premium uses previous EL + WD (two errors), and
            // accrual-on-default uses two consecutive EL/WD points (four).
            // max(0, x) is non-expansive, including at a cashflow boundary.
            let protection_coefficient = 2.0 * discount_default;
            let premium_coefficient = coupon.abs()
                * discount_payment
                * (2.0 * period.accrual_year_fraction
                    + 4.0 * (1.0 - default_fraction) * remaining_accrual);
            let coefficient = protection_coefficient + premium_coefficient;
            coefficient_sum += coefficient;
            absolute_cashflow_scale += discount_default
                + coupon.abs()
                    * discount_payment
                    * (period.accrual_year_fraction + 2.0 * remaining_accrual);
            suppression_discount_sum += discount_payment + discount_default;
            let raw_caps = pricer
                .paired_capped_expectations_for_test(tranche, &index, period.accrual_end)
                .expect("actual paired projection caps");
            assert!(raw_caps.iter().all(|cap| cap.is_finite() && *cap >= 0.0));
            json!({
                "accrual_start": period.accrual_start,
                "accrual_end": period.accrual_end,
                "payment_date": period.payment_date,
                "protection_start": protection_start,
                "default_date": default_date,
                "accrual": period.accrual_year_fraction,
                "remaining_accrual": remaining_accrual,
                "default_fraction": default_fraction,
                "discount_payment": discount_payment,
                "discount_default": discount_default,
                "protection_coefficient": protection_coefficient,
                "premium_coefficient": premium_coefficient,
                "pv_coefficient": coefficient,
                "el": point.el_fraction,
                "wd": point.wd_fraction,
                "raw_cap_source": "actual_projection_pair",
                "raw_cap_order": ["loss_attach", "recovery_attach", "loss_detach", "recovery_detach"],
                "raw_caps": raw_caps,
                "raw_cap_bits": raw_caps.map(f64::to_bits),
            })
        })
        .collect();
    // Upfront is fixed by the contract and introduces no cap-error term,
    // but its discounted magnitude belongs in the floating-point ledger.
    let upfront_discounted_abs =
        tranche
            .upfront
            .filter(|(date, _)| *date >= as_of)
            .map_or(0.0, |(date, amount)| {
                amount.amount().abs()
                    * discount
                        .df_between_dates(as_of, date)
                        .expect("upfront discount factor")
            });
    absolute_cashflow_scale += upfront_discounted_abs / notional;
    // Same conservative arithmetic ledger as the frozen tranche panel,
    // covering each production/reconstruction period and fixed setup.
    let operations = 128 * periods.len() + 128;
    let pv_roundoff_allowance = gamma(operations) * notional * absolute_cashflow_scale;
    // Production omits premium/protection rows with magnitude <= EPSILON
    // currency units. This absolute cutoff needs its own additive allowance;
    // it is not a relative floating-point rounding error.
    let pv_row_suppression_allowance = f64::EPSILON * suppression_discount_sum;
    let pv_error_bound = notional * period_error_bound * coefficient_sum
        + pv_roundoff_allowance
        + pv_row_suppression_allowance;
    let normalized_pv_error_bound = pv_error_bound / notional;
    assert!(normalized_pv_error_bound.is_finite() && normalized_pv_error_bound > 0.0);
    let actual_pv = pricer
        .price_tranche(tranche, market, as_of)
        .expect("calibration control PV");
    assert_eq!(actual_pv.currency(), tranche.notional.currency());
    let actual_pv = actual_pv.amount();
    let normalized_pv = actual_pv / notional;
    assert!(normalized_pv.is_finite());
    json!({
        "id": case.id,
        "role": case.role,
        "quote_id": case.quote_id,
        "settlement": settlement,
        "notional": notional,
        "actual_pv": actual_pv,
        "actual_pv_bits": actual_pv.to_bits(),
        "normalized_pv": normalized_pv,
        "normalized_pv_bits": normalized_pv.to_bits(),
        "cap_error_bound": GAUSSIAN_CAP_BUDGET,
        "period_error_bound": period_error_bound,
        "pv_error_bound": pv_error_bound,
        "normalized_pv_error_bound": normalized_pv_error_bound,
        "normalized_comparison_limit": 2.0 * normalized_pv_error_bound,
        "pv_coefficient_sum": coefficient_sum,
        "pv_roundoff_allowance": pv_roundoff_allowance,
        "pv_row_suppression_allowance": pv_row_suppression_allowance,
        "suppression_discount_sum": suppression_discount_sum,
        "roundoff_operation_budget": operations,
        "absolute_cashflow_scale": absolute_cashflow_scale,
        "upfront_discounted_abs": upfront_discounted_abs,
        "pool_factor": effective.pool_factor,
        "original_tranche_width": width,
        "coupon_decimal": coupon,
        "prior_el": pricer.calculate_prior_tranche_loss(tranche),
        "prior_wd": pricer.calculate_prior_tranche_writedown(tranche, index.recovery_rate()),
        "periods": records,
    })
}

#[test]
#[ignore = "manual B4/BC6 calibration budget export; root freezes the input fixture"]
fn base_correlation_pv_budget_export() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(INPUT_FIXTURE);
    let bytes = std::fs::read(path).expect("root must freeze the calibration exporter fixture");
    let fixture: Value = serde_json::from_slice(&bytes).expect("calibration fixture JSON");
    let inputs: CalibrationInputs =
        serde_json::from_value(fixture.clone()).expect("calibration fixture schema");
    assert_eq!(inputs.schema_version, 1);
    assert_eq!(inputs.benchmark, BENCHMARK);
    let expected_ids = [
        "CDX-IG-46-0-3",
        "CDX-IG-46-3-7",
        "CDX-IG-46-7-10",
        "CDX-IG-46-10-15",
        "CDX-IG-46-15-30",
        "calibration_off_grid_0_5",
        "calibration_off_grid_5_12",
    ];
    assert_eq!(inputs.cases.len(), expected_ids.len());
    for (i, (case, expected_id)) in inputs.cases.iter().zip(expected_ids).enumerate() {
        assert_eq!(case.id, expected_id);
        assert_eq!(case.role, if i < 5 { "quoted" } else { "off_grid" });
        assert_eq!(case.quote_id.as_deref(), (i < 5).then_some(expected_id));
        if i >= 5 {
            assert!(case.instrument.upfront.is_none());
        }
    }
    let cases: Vec<_> = inputs
        .cases
        .iter()
        .map(|case| case_record(case, &inputs))
        .collect();
    let tranche = &inputs.cases[0].instrument;
    let index = inputs
        .market
        .get_credit_index(&tranche.credit_index_id)
        .expect("frozen calibrated index");
    let curve = index.base_correlation_curve();
    assert_eq!(curve.len(), 5);
    assert!(curve.validate_shape().is_monotonic);
    assert!(
        curve
            .correlations()
            .iter()
            .all(|rho| (0.0..0.999).contains(rho))
    );
    let record = json!({
        "schema_version": 1,
        "benchmark": BENCHMARK,
        "fixture_hash": content_hash(&fixture).expect("fixture hash"),
        "envelope_hash": inputs.envelope_hash,
        "contract_hash": content_hash(&json!({"as_of": inputs.as_of, "cases": inputs.cases})).expect("contract hash"),
        "market_hash": content_hash(&inputs.market).expect("market hash"),
        "knots": {
            "detachment_percent": curve.detachment_points(),
            "correlation_decimal": curve.correlations(),
        },
        "prerequisite_curve_hashes": {
            "discount": content_hash(inputs.market.get_discount(&tranche.discount_curve_id).expect("discount").as_ref()).expect("discount hash"),
            "hazard": content_hash(index.index_credit_curve().as_ref()).expect("hazard hash"),
        },
        "allowance_convention": "b_i = pv_error_bound / tranche_notional; same-curve kernel and total calibrated-output differences must each be <= 2*b_i; freeze theta0 budgets before candidate application",
        "cases": cases,
    });
    eprintln!("FQ_CALIBRATION_PV_BUDGET {record}");
}
