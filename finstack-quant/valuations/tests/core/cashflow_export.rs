//! The cashflow exporter over built-in instruments: reconciliation,
//! rejection of non-decomposable values and the explain trace.

use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::cashflow_export::*;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
use finstack_quant_valuations::instruments::fx::fx_swap::FxSwap;
use finstack_quant_valuations::instruments::json_loader::{InstrumentEnvelope, InstrumentJson};
use finstack_quant_valuations::instruments::pricing_overrides::ScenarioPricingOverrides;
use finstack_quant_valuations::pricer::ModelKey;
use std::sync::Arc;
use time::Month;

/// Parse a canonical instrument envelope and export its cashflows as JSON.
fn instrument_cashflows_json(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
) -> finstack_quant_core::Result<String> {
    let instrument =
        finstack_quant_valuations::pricer::parse_boxed_instrument_from_json(instrument_json, None)?;
    let envelope = instrument_cashflows(instrument.as_instrument(), market, as_of, model)?;
    Ok(serde_json::to_string(&envelope).expect("serialize cashflow envelope"))
}

fn serialize_bond(bond: &Bond) -> String {
    let envelope = InstrumentEnvelope {
        schema: finstack_quant_valuations::instruments::envelope::InstrumentSchema::CURRENT,
        instrument: InstrumentJson::Bond(bond.clone()),
    };
    serde_json::to_string(&envelope).expect("serialize bond envelope")
}

#[test]
fn cashflow_export_rejects_exercise_contingent_bond_rows() {
    let bond = Bond::example_callable().expect("callable example");
    for model in ["discounting", "hazard_rate"] {
        let err = instrument_cashflows_json(
            &serialize_bond(&bond),
            &MarketContext::new(),
            "2025-01-01",
            model,
        )
        .expect_err("static rows cannot represent exercise-contingent value");

        assert!(
            err.to_string().contains("exercise-contingent value"),
            "unexpected error for {model}: {err}"
        );
    }
}

#[test]
fn revolving_credit_credit_risk_export_fails_instead_of_returning_false_reconciliation() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let maturity = Date::from_calendar_date(2026, Month::January, 1).expect("date");
    let facility = RevolvingCredit::builder()
        .id("RC-CASHFLOW-CREDIT".into())
        .commitment(Money::from((1_000_000_i64, Currency::USD)))
        .drawn(Money::from((1_000_000_i64, Currency::USD)))
        .issue_date(as_of)
        .maturity(maturity)
        .rate(RateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act365F)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act365F)
        .frequency(Tenor::annual())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .credit_curve_id("USD-HZ".into())
        .recovery_rate(0.4)
        .build()
        .expect("facility");
    let market = MarketContext::new()
        .insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 1.0)])
                .build()
                .expect("discount curve"),
        )
        .insert(
            HazardCurve::builder("USD-HZ")
                .base_date(as_of)
                .recovery_rate(0.4)
                .knots([(1.0, 0.20), (5.0, 0.20)])
                .build()
                .expect("hazard curve"),
        );
    let json = serde_json::to_string(&InstrumentEnvelope::new(InstrumentJson::RevolvingCredit(
        facility,
    )))
    .expect("serialize");

    let err = instrument_cashflows_json(&json, &market, "2025-01-01", "discounting")
        .expect_err("generic rows cannot represent the recovery leg");
    assert!(
        err.to_string()
            .contains("model-specific cashflow decomposition"),
        "unexpected error: {err}"
    );
}

#[test]
fn malformed_instrument_precedes_model_and_date_errors() {
    let mut deal = StructuredCredit::example().expect("example");
    deal.cleanup_call_decimal = Some(-0.5);
    let json = serde_json::to_string(&InstrumentEnvelope::new(InstrumentJson::StructuredCredit(
        Box::new(deal),
    )))
    .expect("serialize structured credit");

    let err = instrument_cashflows_json(&json, &MarketContext::new(), "not-a-date", "not-a-model")
        .expect_err("instrument validation must win");

    assert!(
        err.to_string().contains("cleanup_call_decimal"),
        "unexpected error ordering: {err}"
    );
}

#[test]
fn mixed_currency_fx_swap_rows_use_native_curves_and_reporting_currency_pv() {
    let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("date");
    let swap = FxSwap::example().expect("example");
    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 1.10)])
        .expect("fx quote");
    let market = MarketContext::new()
        .insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 0.95)])
                .build()
                .expect("usd curve"),
        )
        .insert(
            DiscountCurve::builder("EUR-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 0.97)])
                .build()
                .expect("eur curve"),
        )
        .insert_fx(FxMatrix::new(provider, None));
    let instrument = InstrumentEnvelope {
        schema: finstack_quant_valuations::instruments::envelope::InstrumentSchema::CURRENT,
        instrument: InstrumentJson::FxSwap(swap),
    };
    let json = serde_json::to_string(&instrument).expect("serialize fx swap");

    let payload = instrument_cashflows_json(&json, &market, "2024-01-01", "discounting")
        .expect("mixed-currency cashflow export");
    let envelope: InstrumentCashflowEnvelope =
        serde_json::from_str(&payload).expect("parse envelope");

    assert_eq!(envelope.currency, Currency::USD);
    assert!(envelope.reconciles_with_base_value);
    assert!(envelope.flows.iter().any(|row| {
        row.currency == Currency::EUR && row.discount_curve_id.as_str() == "EUR-OIS"
    }));
    assert!(envelope.flows.iter().any(|row| {
        row.currency == Currency::USD && row.discount_curve_id.as_str() == "USD-OIS"
    }));
    for row in &envelope.flows {
        if row.currency == Currency::EUR {
            assert_eq!(row.fx_rate, Some(1.10));
            assert!((row.native_pv * 1.10 - row.pv).abs() < 0.01);
        } else {
            assert!(row.fx_rate.is_none());
        }
    }
}

#[test]
fn discounting_reconciles_with_schedule_pv_for_fixed_bond() {
    use finstack_quant_valuations::instruments::common_impl::helpers::schedule_pv_raw;

    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("date");
    let bond = Bond::fixed(
        "BOND-DISC-RECONCILE",
        Money::from((1_000_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");

    // Use as_of strictly after issue so the initial notional outflow is
    // unambiguously in the past and excluded by both the schedule-based
    // engine and `base_value`. Note this makes the instrument *seasoned*
    // (as_of != curve base_date), which is exactly the case where a wrong
    // time origin in the exporter's discount factors would surface.
    let as_of_date = Date::from_calendar_date(2025, Month::July, 1).expect("date");

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80)])
        .build()
        .expect("discount curve");
    let market = MarketContext::new().insert(disc);

    let json = serialize_bond(&bond);
    let payload = instrument_cashflows_json(&json, &market, "2025-07-01", "discounting")
        .expect("cashflows envelope");
    let envelope: InstrumentCashflowEnvelope =
        serde_json::from_str(&payload).expect("parse envelope");

    // The raw and Money paths share the core relative-discounting kernel;
    // the raw path is used here to avoid Money rounding in reconciliation.
    let discount_curve_id = bond
        .market_dependencies()
        .expect("deps")
        .curves
        .discount_curves
        .first()
        .cloned()
        .expect("bond should declare a discount curve");
    let reference = schedule_pv_raw(&bond, &market, as_of_date, &discount_curve_id, false)
        .expect("schedule pv");

    let diff = (envelope.total_pv - reference).abs();
    assert!(
        diff < 1e-2,
        "total_pv {} should reconcile with schedule PV {} (diff={})",
        envelope.total_pv,
        reference,
        diff,
    );
    assert_eq!(envelope.model, "discounting");
    assert_eq!(envelope.currency, Currency::USD);
    assert!(envelope.reconciles_with_base_value);
    assert!(!envelope.flows.is_empty());
    for row in &envelope.flows {
        assert!(row.survival_probability.is_none());
        assert!(row.discount_factor > 0.0);
        assert!(row.fx_rate.is_none());
        if row.date > as_of_date {
            assert!((row.pv - row.native_pv).abs() < 1e-9);
        }
    }
    assert!(envelope.scenario_price_shock_decimal.is_none());

    // Every coupon row carries the inputs it was computed from.
    let coupons: Vec<_> = envelope
        .flows
        .iter()
        .filter(|row| row.kind == CFKind::Fixed)
        .collect();
    assert!(!coupons.is_empty());
    for row in coupons {
        let start = row.accrual_start.expect("accrual start");
        let end = row.accrual_end.expect("accrual end");
        assert!(start < end && end <= row.date);
        assert!(row.accrual_day_count.is_some());
        let notional = row.accrual_notional.expect("accrual notional");
        let rate = row.rate.expect("coupon rate");
        assert!((notional - 1_000_000.0).abs() < 1e-9);
        assert!((notional * rate * row.accrual_factor - row.amount).abs() < 1e-6);
    }
}

#[test]
fn explained_price_carries_reconciling_trace_and_provenance() {
    use finstack_quant_core::explain::{ExplainOpts, TraceEntry};
    use finstack_quant_valuations::instruments::PricingOptions;
    use finstack_quant_valuations::metrics::MetricId;

    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("date");
    let mut bond = Bond::fixed(
        "BOND-EXPLAIN",
        Money::from((1_000_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.scenario_pricing_overrides =
        ScenarioPricingOverrides::default().with_scenario_price_shock_decimal(-0.10);
    bond.metric_pricing_overrides.bump_config.rate_bump_bp = Some(5.0);
    let as_of = Date::from_calendar_date(2025, Month::July, 1).expect("date");
    let market = MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(issue)
            .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80)])
            .build()
            .expect("discount curve"),
    );

    let plain = bond
        .price_with_metrics(&market, as_of, &[], PricingOptions::default())
        .expect("plain price");
    assert!(plain.explanation.is_none());
    let provenance = plain.provenance.expect("registry stamps provenance");
    assert_eq!(provenance.model, ModelKey::Discounting);
    assert_eq!(provenance.requested_as_of, as_of);
    assert_eq!(
        provenance.market_dependencies,
        bond.market_dependencies().expect("deps")
    );
    assert_eq!(provenance.scenario_price_shock_decimal, Some(-0.10));
    assert!(provenance.sensitivity_bumps.is_none());

    let explained = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            PricingOptions::default().with_explain(ExplainOpts::enabled()),
        )
        .expect("explained price");
    assert_eq!(explained.value, plain.value);
    let bumps = explained
        .provenance
        .as_ref()
        .and_then(|p| p.sensitivity_bumps.as_ref())
        .expect("metric request stamps bumps");
    assert_eq!(bumps.rate_bump_bp, 5.0);
    assert_eq!(bumps.vol_bump_decimal, 0.01);

    let trace = explained.explanation.expect("trace");
    assert_eq!(trace.trace_type, "pricing");
    let mut total = 0.0;
    for entry in &trace.entries {
        let TraceEntry::CashflowPV {
            pv_amount,
            curve_id,
            survival_probability,
            ..
        } = entry
        else {
            panic!("unexpected entry {entry:?}");
        };
        assert_eq!(curve_id, "USD-OIS");
        assert!(survival_probability.is_none());
        total += pv_amount;
    }
    assert!(!trace.entries.is_empty());
    assert!((total - explained.value.amount()).abs() < 1e-2);
}

#[test]
fn trace_names_the_reason_when_no_decomposition_exists() {
    use finstack_quant_core::explain::{ExplainOpts, TraceEntry};

    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("date");
    let bond = Bond::fixed(
        "BOND-TREE",
        Money::from((1_000_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    let priced = finstack_quant_valuations::results::ValuationResult::stamped(
        "BOND-TREE",
        issue,
        Money::from((1_000_000_i64, Currency::USD)),
    );

    let trace = pricing_trace(
        &bond,
        &MarketContext::new(),
        ModelKey::Tree,
        &priced,
        ExplainOpts::enabled(),
    );

    assert!(matches!(
        trace.entries.as_slice(),
        [TraceEntry::ComputationStep { name, description, .. }]
            if name == "cashflow_trace_unavailable" && description.contains("tree")
    ));
}

#[test]
fn scenario_price_shock_scales_cashflow_rows_and_total_once() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("date");
    let mut bond = Bond::fixed(
        "BOND-CASHFLOW-SCENARIO",
        Money::from((1_000_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    let as_of = Date::from_calendar_date(2025, Month::July, 1).expect("date");
    let market = MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(issue)
            .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80)])
            .build()
            .expect("discount curve"),
    );

    let baseline: InstrumentCashflowEnvelope = serde_json::from_str(
        &instrument_cashflows_json(&serialize_bond(&bond), &market, "2025-07-01", "discounting")
            .expect("baseline cashflows"),
    )
    .expect("baseline envelope");

    bond.scenario_pricing_overrides =
        ScenarioPricingOverrides::default().with_scenario_price_shock_decimal(-0.10);
    let shocked: InstrumentCashflowEnvelope = serde_json::from_str(
        &instrument_cashflows_json(&serialize_bond(&bond), &market, "2025-07-01", "discounting")
            .expect("shocked cashflows"),
    )
    .expect("shocked envelope");

    assert_eq!(shocked.as_of, as_of);
    assert_eq!(shocked.scenario_price_shock_decimal, Some(-0.10));
    assert!((shocked.total_pv - baseline.total_pv * 0.90).abs() < 1e-8);
    assert!(shocked.reconciles_with_base_value);
    for (baseline_row, shocked_row) in baseline.flows.iter().zip(&shocked.flows) {
        assert!((shocked_row.pv - baseline_row.pv * 0.90).abs() < 1e-8);
        assert_eq!(shocked_row.native_pv, baseline_row.native_pv);
    }
}

#[test]
fn seasoned_instrument_discount_factors_use_correct_time_origin() {
    // Failure mode: `discount.df(year_fraction)` measures `year_fraction`
    // from `as_of`, but `DiscountCurve::df` expects time from the curve's
    // own `base_date`. When `as_of != base_date` (a seasoned instrument)
    // every exported discount factor and PV lands on the wrong time origin,
    // and `total_pv` no longer reconciles with `base_value`
    // (`Instrument::value`), which uses date-based `df_between_dates`.
    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("date");
    let bond = Bond::fixed(
        "BOND-SEASONED-TIME-ORIGIN",
        Money::from((1_000_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");

    // as_of strictly after the curve base_date => seasoned instrument.
    let as_of_date = Date::from_calendar_date(2025, Month::July, 1).expect("date");

    // Curve base_date == issue, deliberately != as_of.
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80)])
        .build()
        .expect("discount curve");
    let market = MarketContext::new().insert(disc.clone());

    // base_value: the canonical price from Instrument::value, which the
    // discounting kernel computes via df_between_dates(as_of, date).
    let base_value = bond
        .value(&market, as_of_date)
        .expect("bond base value")
        .amount();

    let json = serialize_bond(&bond);
    let payload = instrument_cashflows_json(&json, &market, "2025-07-01", "discounting")
        .expect("cashflows envelope");
    let envelope: InstrumentCashflowEnvelope =
        serde_json::from_str(&payload).expect("parse envelope");

    // total_pv must reconcile with base_value within rounding.
    let diff = (envelope.total_pv - base_value).abs();
    assert!(
        diff < 1e-2,
        "seasoned-instrument total_pv {} must reconcile with base_value {} (diff={}); \
         a wrong time origin in the exported discount factors breaks reconciliation",
        envelope.total_pv,
        base_value,
        diff,
    );

    // Each exported discount factor must equal df_between_dates(as_of, date).
    for row in &envelope.flows {
        if row.year_fraction < 0.0 {
            continue;
        }
        let expected_df = disc
            .df_between_dates(as_of_date, row.date)
            .expect("df_between_dates");
        assert!(
            (row.discount_factor - expected_df).abs() < 1e-12,
            "discount_factor for flow {} is {}, expected df_between_dates(as_of, date)={}",
            row.date,
            row.discount_factor,
            expected_df,
        );
    }

    // The reconciliation flag must be honest: it is only `true` here
    // because the export actually reconciles.
    assert!(envelope.reconciles_with_base_value);
}

#[test]
fn rejects_unsupported_model_for_equity_option_style_instrument() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("date");
    let maturity = Date::from_calendar_date(2026, Month::January, 15).expect("date");
    let bond = Bond::fixed(
        "BOND-BAD-MODEL",
        Money::from((1_000_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    let json = serialize_bond(&bond);
    let market = MarketContext::new();

    let err = instrument_cashflows_json(&json, &market, "2025-01-15", "monte_carlo_gbm")
        .expect_err("monte_carlo_gbm should reject bond");
    let msg = err.to_string();
    assert!(
        msg.contains("monte_carlo_gbm") || msg.contains("not priced") || msg.contains("supported"),
        "error should explain unsupported model: {msg}"
    );
}

// NOTE: A hazard_rate reconciliation test would require an instrument that
// (a) declares a canonical credit-curve dependency AND (b) has a
// registered `HazardRate` pricer. The existing CDS and bond-with-hazard
// pathways satisfy both via the instrument JSON layer, which is exercised
// by the workspace-wide test suite (see `tests/instruments/cds/`). The
// per-flow PV formula here is byte-identical to the one validated by
// `period_pv.rs::test_periodized_pv_credit_adjusted_matches_detailed_engine`.

/// Outstanding principal a row moves, read only from exported fields.
fn principal_effect(row: &CashflowRow) -> f64 {
    if let Some(delta) = row.principal_delta {
        return delta;
    }
    match row.kind {
        CFKind::Pik => row.amount,
        CFKind::Amortization
        | CFKind::PrePayment
        | CFKind::DefaultedNotional
        | CFKind::Notional => -row.amount,
        _ => 0.0,
    }
}

/// Each row rolls its opening balance to its closing balance, and each principal
/// row opens where the previous one closed. Rows that move nothing need not
/// chain: a coupon paid after its period's balance date opens at the later balance.
fn assert_balances_roll(envelope: &InstrumentCashflowEnvelope, opening: f64, closing: f64) {
    let mut previous: Option<f64> = None;
    for row in &envelope.flows {
        let begin = row.beginning_balance.expect("beginning balance");
        let end = row.ending_balance.expect("ending balance");
        assert!(
            (begin + principal_effect(row) - end).abs() < 1e-6,
            "{} {:?}: {begin} + {} != {end}",
            row.date,
            row.kind,
            principal_effect(row)
        );
        if principal_effect(row) == 0.0 {
            continue;
        }
        if let Some(previous) = previous {
            assert!(
                (previous - begin).abs() < 1e-6,
                "{} opens at {begin}",
                row.date
            );
        }
        previous = Some(end);
    }
    let first = envelope.flows.first().expect("flows");
    assert!((first.beginning_balance.expect("begin") - opening).abs() < 1e-6);
    assert!((previous.expect("flows") - closing).abs() < 1e-6);
    if closing == 0.0 {
        assert_eq!(
            previous,
            Some(0.0),
            "a repaid balance closes at exactly zero"
        );
    }
}

fn balance_market(as_of: Date) -> MarketContext {
    MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, 0.97), (10.0, 0.70)])
            .build()
            .expect("curve"),
    )
}

fn balance_bond(id: &str) -> Bond {
    Bond::fixed(
        id,
        Money::from((1_000_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("rate"),
        Date::from_calendar_date(2025, Month::January, 15).expect("date"),
        Date::from_calendar_date(2030, Month::January, 15).expect("date"),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond")
}

fn export(instrument: &dyn Instrument, as_of: Date) -> InstrumentCashflowEnvelope {
    instrument_cashflows(
        instrument,
        &balance_market(as_of),
        &as_of.to_string(),
        "discounting",
    )
    .expect("envelope")
}

#[test]
fn bullet_bond_balances_hold_until_redemption() {
    let as_of = Date::from_calendar_date(2026, Month::March, 1).expect("date");
    let envelope = export(&balance_bond("BOND-BULLET"), as_of);
    assert_balances_roll(&envelope, 1_000_000.0, 0.0);
    for row in envelope
        .flows
        .iter()
        .filter(|row| row.kind == CFKind::Fixed)
    {
        assert_eq!(row.beginning_balance, Some(1_000_000.0));
        assert_eq!(row.ending_balance, Some(1_000_000.0));
    }
}

#[test]
fn issue_funding_row_keeps_the_opening_balance() {
    // The opening notional already holds the funded principal, so the funding
    // cash moves nothing (record CORE-CONVENTIONS-015 #5).
    let as_of = Date::from_calendar_date(2025, Month::January, 15).expect("date");
    let envelope = export(&balance_bond("BOND-FUNDING"), as_of);
    let funding = &envelope.flows[0];
    assert_eq!(funding.kind, CFKind::Notional);
    assert_eq!(funding.principal_delta, Some(0.0));
    assert_eq!(funding.beginning_balance, Some(1_000_000.0));
    assert_eq!(funding.ending_balance, Some(1_000_000.0));
    assert_balances_roll(&envelope, 1_000_000.0, 0.0);
}

#[test]
fn amortizing_term_loan_balances_match_the_schedule_path() {
    let json = include_str!("../instruments/json_examples/term_loan.json");
    let instrument =
        finstack_quant_valuations::pricer::parse_boxed_instrument_from_json(json, None)
            .expect("example loan");
    let as_of = Date::from_calendar_date(2025, Month::February, 1).expect("date");
    let envelope = export(instrument.as_instrument(), as_of);
    let opening = envelope.flows[0].beginning_balance.expect("begin");
    // 2.5% of remaining a quarter from 10m, four quarters paid before as_of.
    assert!(
        (opening - 10_000_000.0 * 0.975_f64.powi(4)).abs() < 1e-3,
        "{opening}"
    );
    assert_balances_roll(&envelope, opening, 0.0);
    // A coupon shows the principal it accrued on, even when it pays on a
    // business-day-adjusted date after the amortization of its accrual end.
    for row in envelope
        .flows
        .iter()
        .filter(|row| row.kind == CFKind::Fixed)
    {
        let accrued_on = row.accrual_notional.expect("accrual notional");
        assert!(
            (row.beginning_balance.expect("begin") - accrued_on).abs() < 1e-6,
            "{}",
            row.date
        );
        assert_eq!(row.beginning_balance, row.ending_balance);
    }
    let amortization: Vec<_> = envelope
        .flows
        .iter()
        .filter(|row| row.kind == CFKind::Amortization)
        .collect();
    assert!(!amortization.is_empty());
    assert!(
        amortization
            .iter()
            .all(|row| row.ending_balance < row.beginning_balance)
    );
}

#[test]
fn pik_bond_balance_capitalises_every_coupon() {
    let mut bond = balance_bond("BOND-PIK");
    let finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec::Fixed(spec) =
        &mut bond.cashflow_spec
    else {
        panic!("fixed bond");
    };
    spec.coupon_type = finstack_quant_cashflows::builder::CouponType::Pik;
    let as_of = Date::from_calendar_date(2025, Month::January, 16).expect("date");
    let envelope = export(&bond, as_of);
    assert_balances_roll(&envelope, 1_000_000.0, 0.0);

    let pik: Vec<_> = envelope
        .flows
        .iter()
        .filter(|row| row.kind == CFKind::Pik)
        .collect();
    assert!(!pik.is_empty());
    for row in &pik {
        assert!(row.ending_balance.expect("end") > row.beginning_balance.expect("begin"));
    }
    // The redemption repays the capitalised balance, not the original face.
    let redeemed = envelope
        .flows
        .iter()
        .find(|row| row.kind == CFKind::Notional)
        .expect("redemption");
    assert_eq!(
        redeemed.beginning_balance,
        pik.last().expect("pik").ending_balance
    );
    assert!((redeemed.amount - redeemed.beginning_balance.expect("begin")).abs() < 1e-6);
    assert!(redeemed.amount > 1_000_000.0);
}

#[test]
fn revolver_balances_follow_draws_and_repayments() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let maturity = Date::from_calendar_date(2027, Month::January, 1).expect("date");
    let draw = Date::from_calendar_date(2025, Month::June, 2).expect("date");
    let repay = Date::from_calendar_date(2026, Month::March, 2).expect("date");
    let mut facility = RevolvingCredit::example().expect("example facility");
    facility.issue_date = issue;
    facility.maturity = maturity;
    facility.rate = RateSpec::Fixed { rate: 0.05 };
    facility.drawn = Money::new(10_000_000.0, Currency::USD).expect("money");
    facility.commitment = Money::new(50_000_000.0, Currency::USD).expect("money");
    facility.draw_repay_spec = DrawRepaySpec::Deterministic(vec![
        finstack_quant_valuations::instruments::fixed_income::revolving_credit::DrawRepayEvent {
            date: draw,
            amount: Money::new(5_000_000.0, Currency::USD).expect("money"),
            is_draw: true,
        },
        finstack_quant_valuations::instruments::fixed_income::revolving_credit::DrawRepayEvent {
            date: repay,
            amount: Money::new(2_000_000.0, Currency::USD).expect("money"),
            is_draw: false,
        },
    ]);
    let as_of = issue;
    let envelope = export(&facility, as_of);
    assert_balances_roll(&envelope, 10_000_000.0, 0.0);
    let at = |date: Date| {
        envelope
            .flows
            .iter()
            .find(|row| row.date == date && row.kind == CFKind::Notional)
            .expect("principal row")
    };
    assert_eq!(at(draw).ending_balance, Some(15_000_000.0));
    assert_eq!(at(repay).ending_balance, Some(13_000_000.0));
}

#[test]
fn balances_stay_empty_for_instruments_without_an_outstanding_principal() {
    let as_of = Date::from_calendar_date(2025, Month::January, 2).expect("date");
    let terms = CashflowExportTerms::from_schedule(
        finstack_quant_cashflows::schedule_from_dated_flows(
            vec![(
                Date::from_calendar_date(2025, Month::June, 2).expect("date"),
                Money::from((100_i64, Currency::USD)),
            )],
            CFKind::Fixed,
            DayCount::Act360,
            finstack_quant_cashflows::ScheduleBuildOpts::default(),
        )
        .expect("schedule"),
    );
    assert!(
        terms.get_flow_balances().is_none(),
        "not requested at {as_of}"
    );
    let no_issue_date = terms
        .with_schedule_balances()
        .expect("no issue date is not an error");
    assert!(no_issue_date.get_flow_balances().is_none());
}
