//! Shared, test-only fixture descriptors and exact numerical capture helpers.
//!
//! Included by the integration panel and both owner-local unit-test panels.

#![allow(dead_code)]

use super::fi::bond::{Bond, CallPut, CallPutSchedule};
use super::fi::term_loan::{LoanCall, LoanCallSchedule, LoanCallType, TermLoan};
use finstack_quant_cashflows::accrual::{AccrualConfig, AccrualIndex, accrued_interest_amount};
use finstack_quant_cashflows::builder::{AmortizationSpec, CashFlowSchedule, CouponType, RateSpec};
use finstack_quant_core::canonical::{CANONICAL_VERSION, content_hash};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, Rate};
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_test_support::test_helpers::flat_discount_curve;
use serde_json::{Value, json};
use time::macros::date;

pub(super) const AI_BOUND_USD: f64 = 1e-7;
pub(super) const IMMEDIATE_CALL_BOUND_USD: f64 = 1.0;
pub(super) const WINDOWED_BOND_ID: &str = "ccm_i3_consumer/windowed_bond_hw_100";
pub(super) const CALLABLE_LOAN_ID: &str = "ccm_i3_consumer/callable_loan_100";
pub(super) const IMMEDIATE_LOAN_ID: &str = "ccm_i3_consumer/immediate_par_loan";
pub(super) const PIK_LOAN_ID: &str = "ccm_i3_consumer/pik_loan_claims";

pub(super) fn close(actual: f64, expected: f64, bound: f64) {
    assert!(actual.is_finite() && expected.is_finite());
    assert!(
        (actual - expected).abs() <= bound,
        "actual {actual}, expected {expected}, absolute bound {bound} USD"
    );
}

pub(super) fn scalar(value: f64) -> Value {
    assert!(value.is_finite());
    json!({"value": value, "f64_bits_hex": format!("{:016x}", value.to_bits())})
}

pub(super) fn money(value: Money) -> Value {
    let decimal = value.amount_decimal();
    json!({
        "wire": value,
        "currency": value.currency().to_string(),
        "decimal": decimal.to_string(),
        "decimal_mantissa": decimal.mantissa().to_string(),
        "decimal_scale": decimal.scale(),
        "decimal_bytes": decimal.serialize(),
        "float": scalar(value.amount()),
    })
}

pub(super) fn schedule(schedule: &CashFlowSchedule) -> Value {
    schedule.validate().expect("consumer schedule validates");
    let rows: Vec<_> = schedule
        .get_flows()
        .iter()
        .enumerate()
        .map(|(ordinal, flow)| {
            json!({
                "ordinal": ordinal, "date": flow.date.to_string(), "kind": flow.kind,
                "amount": money(flow.amount), "reset_date": flow.reset_date,
                "principal_date": flow.principal_date,
                "principal_delta": flow.principal_delta.map(money),
                "accrual_factor": scalar(flow.accrual_factor), "rate": flow.rate.map(scalar),
                "accrual": flow.accrual,
                "projected_index_rate": flow.accrual.as_ref().and_then(|a| a.projected_index_rate).map(scalar),
            })
        })
        .collect();
    let fixings: Vec<_> = schedule.get_meta().projected_fixings.iter().map(|fixing| {
        json!({"series_id": fixing.series_id, "date": fixing.date.to_string(), "value": fixing.value.map(scalar)})
    }).collect();
    json!({
        "wire": schedule, "rows": rows, "notional": schedule.get_notional(),
        "meta": schedule.get_meta(), "normalized_fixings_with_bits": fixings,
        "flow_count": schedule.get_flows().len(), "coupon_count": schedule.coupons().count(),
    })
}

pub(super) fn bond_market() -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(date!(2025 - 01 - 01))
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.88),
            (10.0, 0.70),
            (30.0, 0.40),
        ])
        .interp(InterpStyle::MonotoneConvex)
        .build()
        .expect("windowed bond market");
    MarketContext::new().insert(curve)
}

pub(super) fn windowed_bond() -> Bond {
    let mut bond = Bond::fixed(
        "CCM-I3-WINDOWED-BOND",
        Money::from((100_i64, Currency::USD)),
        Rate::from_decimal(0.05).expect("coupon"),
        date!(2025 - 01 - 01),
        date!(2035 - 01 - 01),
        StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("windowed bond");
    bond.instrument_pricing_overrides = InstrumentPricingOverrides::default()
        .with_hw1f_sigma(0.01)
        .with_hw1f_mean_reversion(0.03)
        .with_tree_steps(100);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: date!(2030 - 01 - 01),
            end: date!(2035 - 01 - 01),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: Vec::new(),
    });
    bond
}

pub(super) fn loan_market(as_of: Date) -> MarketContext {
    MarketContext::new().insert(flat_discount_curve(0.05, as_of, "USD-OIS"))
}

pub(super) fn callable_loan() -> TermLoan {
    TermLoan::builder()
        .id(InstrumentId::new("CCM-I3-CALLABLE-LOAN"))
        .currency(Currency::USD)
        .notional_limit(Money::from((10_000_000_i64, Currency::USD)))
        .issue_date(date!(2025 - 01 - 01))
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate: 0.06 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .business_day_convention(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .credit_curve_id_opt(None)
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .oid_eir_opt(None)
        .call_schedule_opt(Some(LoanCallSchedule {
            calls: vec![LoanCall {
                date: date!(2027 - 01 - 01),
                price_pct_of_par: 101.0,
                call_type: LoanCallType::Hard,
            }],
        }))
        .settlement_days(1)
        .instrument_pricing_overrides(
            InstrumentPricingOverrides::default()
                .with_hw1f_sigma(0.01)
                .with_tree_steps(100),
        )
        .attributes(Default::default())
        .build()
        .expect("callable loan")
}

pub(super) fn immediate_loan() -> TermLoan {
    let mut loan = callable_loan();
    loan.id = InstrumentId::new("CCM-I3-IMMEDIATE-LOAN");
    loan.settlement_days = 0;
    loan.instrument_pricing_overrides.model_config.hw1f_sigma = Some(0.0);
    loan.call_schedule = Some(LoanCallSchedule {
        calls: vec![LoanCall {
            date: date!(2025 - 02 - 15),
            price_pct_of_par: 100.0,
            call_type: LoanCallType::Hard,
        }],
    });
    loan
}

pub(super) fn pik_loan(amortizing: bool) -> TermLoan {
    let mut loan = callable_loan();
    loan.id = InstrumentId::new(if amortizing {
        "CCM-I3-PIK-AMORT"
    } else {
        "CCM-I3-PIK"
    });
    loan.maturity = date!(2027 - 01 - 01);
    loan.notional_limit = Money::from((100_i64, Currency::USD));
    loan.rate = RateSpec::Fixed { rate: 0.10 };
    loan.frequency = Tenor::annual();
    loan.day_count = DayCount::Thirty360;
    loan.business_day_convention = BusinessDayConvention::Unadjusted;
    loan.coupon_type = CouponType::Pik;
    loan.amortization = if amortizing {
        AmortizationSpec::PercentOfRemainingPerPeriod { pct: 0.5 }
    } else {
        AmortizationSpec::None
    };
    loan.call_schedule = None;
    loan.settlement_days = 0;
    loan
}

// The fixtures start coupons on Jan/Jul 1, so the US 30/360 month-end-February
// exception cannot apply. This is an independent integer-days calculation.
fn thirty360_days(start: Date, end: Date) -> f64 {
    let first = start.day().min(30);
    let last = if end.day() == 31 && first == 30 {
        30
    } else {
        end.day()
    };
    f64::from(
        360 * (end.year() - start.year())
            + 30 * (i32::from(end.month() as u8) - i32::from(start.month() as u8))
            + i32::from(last)
            - i32::from(first),
    )
}

pub(super) fn expected_linear(schedule: &CashFlowSchedule, date: Date, include_pik: bool) -> f64 {
    schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.kind.is_coupon() || (include_pik && flow.kind == CFKind::Pik))
        .map(|flow| {
            let accrual = flow.accrual.as_ref().expect("contractual accrual");
            if date < accrual.start || date >= flow.date {
                return 0.0;
            }
            let end = date.min(accrual.end);
            let (elapsed, total) = match accrual.day_count {
                DayCount::Act360 | DayCount::Act365F => (
                    (end - accrual.start).whole_days() as f64,
                    (accrual.end - accrual.start).whole_days() as f64,
                ),
                DayCount::Thirty360 => (
                    thirty360_days(accrual.start, end),
                    thirty360_days(accrual.start, accrual.end),
                ),
                other => panic!("unsupported independent consumer convention {other:?}"),
            };
            // A PIK row's value is its explicit capitalization, not cash.
            let value = if flow.kind == CFKind::Pik {
                flow.principal_delta.expect("PIK delta").amount()
            } else {
                flow.amount.amount()
            };
            value * elapsed / total
        })
        .sum()
}

pub(super) fn queries(schedule: &CashFlowSchedule, cfg: &AccrualConfig, dates: &[Date]) -> Value {
    let index = AccrualIndex::build(schedule, cfg).expect("consumer accrual index");
    let mut total = 0.0;
    let records: Vec<_> = dates
        .iter()
        .enumerate()
        .map(|(ordinal, &date)| {
            let value = index.accrued_at(date).expect("consumer accrued query");
            let direct =
                accrued_interest_amount(schedule, date, cfg).expect("direct accrued query");
            let expected = expected_linear(schedule, date, cfg.include_pik);
            close(value, expected, AI_BOUND_USD);
            assert_eq!(
                value.to_bits(),
                direct.to_bits(),
                "direct and prepared query differ"
            );
            total += value;
            let active_rows: Vec<_> = schedule
                .get_flows()
                .iter()
                .enumerate()
                .filter_map(|(row, flow)| {
                    let eligible =
                        flow.kind.is_coupon() || (cfg.include_pik && flow.kind == CFKind::Pik);
                    let accrual = flow.accrual.as_ref()?;
                    (eligible && accrual.start <= date && date < flow.date).then_some(row)
                })
                .collect();
            json!({"ordinal": ordinal, "date": date.to_string(), "value": scalar(value),
            "direct_value": scalar(direct), "expected": scalar(expected),
            "absolute_error_usd": (value - expected).abs(), "bound_usd": AI_BOUND_USD,
            "expected_active_schedule_rows": active_rows})
        })
        .collect();
    json!({"config": cfg, "queries": records, "query_count": dates.len(),
        "outer_sum_in_input_order": scalar(total), "date_provenance": "caller-supplied diagnostic dates"})
}

pub(super) fn export_destination() -> std::path::PathBuf {
    std::env::var_os("CCM_CASHFLOW_CONSUMER_EXPORT")
        .map(std::path::PathBuf::from)
        .expect("set CCM_CASHFLOW_CONSUMER_EXPORT to an explicit output directory")
}

pub(super) fn emit(record: Value) {
    let directory = export_destination();
    let fixture = record["fixture_id"].as_str().expect("fixture identifier");
    let panel = record["panel"].as_str().unwrap_or("public_controls");
    let filename = format!("{}__{panel}.json", fixture.replace('/', "_"));
    let document = json!({
        "schema": "ccm-cashflow-consumer-v1", "package": env!("CARGO_PKG_NAME"),
        "package_version": env!("CARGO_PKG_VERSION"),
        "canonical_version": CANONICAL_VERSION,
        "record_hash": content_hash(&record).expect("consumer record hash"),
        "source_label": std::env::var("CCM_CASHFLOW_CONSUMER_SOURCE").ok(),
        "capture_scope": "untimed numerical diagnostics; no latency or memory measurement",
        "candidate_requirement": "exact same-host C0 values, Decimal encodings, ordered dates, claims and float bits",
        "record": record,
    });
    std::fs::create_dir_all(&directory).expect("consumer export directory");
    std::fs::write(
        directory.join(filename),
        serde_json::to_vec_pretty(&document).expect("consumer capture JSON"),
    )
    .expect("consumer capture write");
}
