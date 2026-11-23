use super::capital_structure::*;
use super::spec::*;
use crate::types::{NodeSpec, NodeType};
use finstack_quant_core::contract::ContractError;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
use indexmap::IndexMap;

use crate::types::AmountOrScalar;
use finstack_quant_core::dates::{Date, Period, PeriodId};
use time::Month;

fn period(id: PeriodId, is_actual: bool) -> Period {
    let range = finstack_quant_core::dates::build_periods(&format!("{id}..{id}"), None)
        .expect("single-period range");
    let mut p = range.into_periods().into_iter().next().expect("one period");
    p.is_actual = is_actual;
    p
}

fn model_with_periods(periods: Vec<Period>) -> FinancialModelSpec {
    let mut model = FinancialModelSpec::new(
        "timeline",
        finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            periods,
        )
        .expect("valid period plan"),
    );
    // A trivial node so the model is otherwise valid.
    let first = model.periods.first().expect("period").id;
    model.add_node(
        NodeSpec::new("revenue", NodeType::Value).with_values(
            [(first, AmountOrScalar::scalar(100.0))]
                .into_iter()
                .collect(),
        ),
    );
    model
}

/// The legitimate layout — actuals then forecasts, in order — must pass.
#[test]
fn contiguous_actuals_then_forecasts_is_accepted() {
    let mut model = model_with_periods(vec![
        period(
            PeriodId::quarter(2024, 1).expect("valid period fixture"),
            true,
        ),
        period(
            PeriodId::quarter(2024, 2).expect("valid period fixture"),
            true,
        ),
        period(
            PeriodId::quarter(2024, 3).expect("valid period fixture"),
            false,
        ),
        period(
            PeriodId::quarter(2024, 4).expect("valid period fixture"),
            false,
        ),
    ]);
    model
        .validate_semantics()
        .expect("actuals-then-forecasts in order is valid");
}

/// An actual after a forecast anchors that forecast on a later value.
#[test]
fn actual_after_forecast_is_rejected() {
    let mut model = model_with_periods(vec![
        period(
            PeriodId::quarter(2024, 1).expect("valid period fixture"),
            true,
        ),
        period(
            PeriodId::quarter(2024, 2).expect("valid period fixture"),
            false,
        ),
        period(
            PeriodId::quarter(2024, 3).expect("valid period fixture"),
            true,
        ),
        period(
            PeriodId::quarter(2024, 4).expect("valid period fixture"),
            false,
        ),
    ]);
    let err = model
        .validate_semantics()
        .expect_err("an actual after a forecast is look-ahead and must be rejected");
    assert!(
        err.to_string().contains("prefix"),
        "expected the actuals-prefix diagnostic: {err}"
    );
}

/// Out-of-order periods are rejected regardless of actual/forecast flags.
#[test]
fn out_of_order_periods_are_rejected() {
    let rows = vec![
        period(PeriodId::quarter(2024, 2).unwrap(), true),
        period(PeriodId::quarter(2024, 1).unwrap(), true),
    ];
    let err = finstack_quant_core::dates::PeriodPlan::try_new(
        finstack_quant_core::dates::PeriodCalendar::Gregorian,
        rows,
    )
    .expect_err("descending periods must be rejected at construction");
    assert!(err.to_string().contains("ascending"), "{err}");
}

/// Both swap sides are supported; signed cashflows and the declared IRS
/// kind determine receipts and payments independently of coupon signs.
#[test]
fn receive_swap_in_capital_structure_is_accepted() {
    use finstack_quant_valuations::instruments::PayReceive;

    let mut swap = InterestRateSwap::example().expect("example swap");
    swap.side = PayReceive::Receive;
    let mut model = model_with_periods(vec![period(
        PeriodId::quarter(2024, 1).expect("valid period fixture"),
        true,
    )]);
    model.capital_structure = Some(CapitalStructureSpec {
        debt_instruments: vec![DebtInstrumentSpec {
            id: "IRS-RCV".to_string(),
            spec: FinancialStatementInstrument::InterestRateSwap(swap),
        }],
        meta: IndexMap::new(),
        reporting_currency: None,
        fx_policy: None,
        waterfall: None,
    });
    model
        .validate_semantics()
        .expect("Receive swaps are supported");
}

/// A bond can use the default waterfall. Explicit prepayment requires an
/// engine that can rebuild the instrument's schedule after each prepayment.
#[test]
fn bond_accepts_default_waterfall_and_rejects_explicit_sweep() {
    use crate::capital_structure::{EcfSweepSpec, PaymentPriority, WaterfallSpec};

    let bond = Bond::fixed(
        finstack_quant_core::types::InstrumentId::new("BOND-SWEEP"),
        finstack_quant_core::money::Money::from((
            1_000_000_i64,
            finstack_quant_core::currency::Currency::USD,
        )),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        Date::from_calendar_date(2025, Month::January, 1).expect("valid date"),
        Date::from_calendar_date(2030, Month::January, 1).expect("valid date"),
        finstack_quant_core::dates::StubKind::ShortFront,
        finstack_quant_core::types::CurveId::new("USD-OIS"),
    )
    .expect("bond");
    let mut model = model_with_periods(vec![period(
        PeriodId::quarter(2025, 1).expect("valid period fixture"),
        true,
    )]);
    let default_waterfall: WaterfallSpec =
        serde_json::from_value(serde_json::json!({ "available_cash_node": "cash" }))
            .expect("waterfall with omitted payment order");
    assert_eq!(default_waterfall, WaterfallSpec::default());
    let mut capital = CapitalStructureSpec {
        debt_instruments: vec![DebtInstrumentSpec {
            id: "BOND-SWEEP".to_string(),
            spec: FinancialStatementInstrument::Bond(bond),
        }],
        meta: IndexMap::new(),
        reporting_currency: None,
        fx_policy: None,
        waterfall: Some(default_waterfall.clone()),
    };
    capital
        .validate()
        .expect("a bond can use the default scheduled-payment waterfall");
    capital.waterfall = Some(WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Sweep,
            PaymentPriority::Equity,
        ],
        ecf_sweep: Some(EcfSweepSpec {
            cash_measure: "ebitda".into(),
            deductions: vec![
                crate::capital_structure::EcfTerm::PaidCashInterest,
                crate::capital_structure::EcfTerm::PaidFees,
                crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
            ],
            credits: Vec::new(),
            sweep_percentage: 0.5,
            facilities: crate::capital_structure::FacilitySelection::AllEligible,
            payment_lag_days: None,
        }),
        ..default_waterfall
    });
    model.capital_structure = Some(capital);
    let err = model
        .validate_semantics()
        .expect_err("Bond + Sweep must be a build error");
    assert!(
        err.to_string().contains("bond"),
        "expected the Bond+sweep diagnostic: {err}"
    );
}

/// A prepayment names facilities of the capital structure, and cannot
/// repay at par a loan whose agreement prices its own payoff.
#[test]
fn prepayment_facilities_must_exist_and_be_repayable_at_par() {
    use crate::capital_structure::{
        FacilitySelection, PaymentPriority, PrepaymentSpec, WaterfallSpec,
    };

    let loan = |id: &str, callable: bool| {
        let mut loan = if callable {
            TermLoan::example_callable().expect("callable loan")
        } else {
            TermLoan::example().expect("loan")
        };
        loan.id = finstack_quant_core::types::InstrumentId::new(id);
        DebtInstrumentSpec {
            id: id.to_string(),
            spec: FinancialStatementInstrument::TermLoan(loan),
        }
    };
    let capital = |facilities: FacilitySelection| CapitalStructureSpec {
        debt_instruments: vec![loan("TL-PAR", false), loan("TL-CALL", true)],
        meta: IndexMap::new(),
        reporting_currency: None,
        fx_policy: None,
        waterfall: Some(WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::VoluntaryPrepayment,
                PaymentPriority::Equity,
            ],
            voluntary_prepayment: Some(PrepaymentSpec {
                amount: "prepay".into(),
                facilities,
            }),
            ..WaterfallSpec::default()
        }),
    };

    capital(FacilitySelection::Named(vec!["TL-PAR".into()]))
        .validate()
        .expect("a loan without payoff terms is repaid at par");

    let error = capital(FacilitySelection::Named(vec!["TL-GHOST".into()]))
        .validate()
        .expect_err("unknown facility")
        .to_string();
    assert!(error.contains("'TL-GHOST'"), "{error}");
    assert!(error.contains("voluntary_prepayment"), "{error}");

    for facilities in [
        FacilitySelection::AllEligible,
        FacilitySelection::Named(vec!["TL-CALL".into()]),
    ] {
        let error = capital(facilities)
            .validate()
            .expect_err("callable loan repaid at par")
            .to_string();
        assert!(error.contains("'TL-CALL'"), "{error}");
        assert!(error.contains("call schedule"), "{error}");
    }
}

/// A PIK election is applied through a term loan's coupon terms, so it
/// may only name a term loan of the structure.
#[test]
fn pik_elections_may_only_name_term_loans() {
    use crate::capital_structure::{PikElectionSpec, PikToggleSpec, WaterfallSpec};

    let bond = Bond::fixed(
        finstack_quant_core::types::InstrumentId::new("BOND-PIK"),
        finstack_quant_core::money::Money::from((
            1_000_000_i64,
            finstack_quant_core::currency::Currency::USD,
        )),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        Date::from_calendar_date(2025, Month::January, 1).expect("valid date"),
        Date::from_calendar_date(2030, Month::January, 1).expect("valid date"),
        finstack_quant_core::dates::StubKind::ShortFront,
        finstack_quant_core::types::CurveId::new("USD-OIS"),
    )
    .expect("bond");
    let mut loan = TermLoan::example().expect("loan");
    loan.id = finstack_quant_core::types::InstrumentId::new("TL-PIK");
    let capital = |target: &str| CapitalStructureSpec {
        debt_instruments: vec![
            DebtInstrumentSpec {
                id: "BOND-PIK".to_string(),
                spec: FinancialStatementInstrument::Bond(bond.clone()),
            },
            DebtInstrumentSpec {
                id: "TL-PIK".to_string(),
                spec: FinancialStatementInstrument::TermLoan(loan.clone()),
            },
        ],
        meta: IndexMap::new(),
        reporting_currency: None,
        fx_policy: None,
        waterfall: Some(WaterfallSpec {
            pik_toggle: Some(PikToggleSpec {
                liquidity_metric: "cash".into(),
                threshold: 1.0,
                elections: vec![PikElectionSpec {
                    instrument_id: target.into(),
                    pik_fraction: rust_decimal::Decimal::ONE,
                    min_coupon_periods: 0,
                    notice_days: 0,
                }],
            }),
            ..WaterfallSpec::default()
        }),
    };
    capital("TL-PIK").validate().expect("a term loan target");
    let error = capital("TL-GHOST")
        .validate()
        .expect_err("an unknown loan")
        .to_string();
    assert!(error.contains("'TL-GHOST'"), "{error}");
    let error = capital("BOND-PIK")
        .validate()
        .expect_err("a bond target")
        .to_string();
    assert!(error.contains("'BOND-PIK'"), "{error}");
    assert!(error.contains("pik_toggle"), "{error}");
}

/// The supported `Pay` side must continue to pass validation.
#[test]
fn pay_swap_in_capital_structure_is_accepted() {
    let swap = InterestRateSwap::example().expect("example swap");
    let mut model = model_with_periods(vec![period(
        PeriodId::quarter(2024, 1).expect("valid period fixture"),
        true,
    )]);
    model.capital_structure = Some(CapitalStructureSpec {
        debt_instruments: vec![DebtInstrumentSpec {
            id: "IRS-PAY".to_string(),
            spec: FinancialStatementInstrument::InterestRateSwap(swap),
        }],
        meta: IndexMap::new(),
        reporting_currency: None,
        fx_policy: None,
        waterfall: None,
    });
    model
        .validate_semantics()
        .expect("a Pay swap is supported and must pass");
}

/// The capital-structure checks run on the spec alone, so a host can
/// validate a `CapitalStructureSpec` without building a model around it.
#[test]
fn capital_structure_spec_validates_standalone() {
    let spec: CapitalStructureSpec = serde_json::from_value(serde_json::json!({
        "waterfall": {
            "priority_of_payments": ["fees", "interest", "amortization", "equity", "sweep"],
            "available_cash_node": "cash",
        }
    }))
    .expect("shape parses");
    let err = spec.validate().expect_err("equity must rank last");
    assert!(err.to_string().contains("last entry"), "{err}");

    let mut swap = InterestRateSwap::example().expect("example swap");
    swap.side = finstack_quant_valuations::instruments::PayReceive::Receive;
    let spec = CapitalStructureSpec {
        debt_instruments: vec![DebtInstrumentSpec {
            id: "IRS-RCV".to_string(),
            spec: FinancialStatementInstrument::InterestRateSwap(swap),
        }],
        meta: IndexMap::new(),
        reporting_currency: None,
        fx_policy: None,
        waterfall: None,
    };
    spec.validate().expect("Receive swaps are supported");
}

/// `from_json` decodes and then validates: a document that decodes but
/// is not runnable fails with its validation kind, and a valid one loads.
#[test]
fn from_json_runs_semantic_validation() {
    let err = FinancialModelSpec::from_json(
        r#"{"id": "m", "periods": {"calendar": "gregorian", "periods": []}, "nodes": {}, "schema_version": 1}"#,
    )
    .expect_err("empty timeline is rejected");
    let ContractError::Report(report) = &err else {
        panic!("expected a located diagnostic, got {err:?}");
    };
    assert_eq!(report.diagnostics[0].code, "statements/build");
    assert_eq!(report.diagnostics[0].pointer.as_deref(), Some("/periods"));
    assert!(err.to_string().contains("at least one period"), "{err}");

    let err = FinancialModelSpec::from_json("{\"id\": ").expect_err("malformed JSON is rejected");
    assert!(
        matches!(&err, ContractError::Report(report) if report.diagnostics[0].code == "contract/parse-error"),
        "{err:?}"
    );

    let model = model_with_periods(vec![period(
        PeriodId::quarter(2024, 1).expect("valid period fixture"),
        true,
    )]);
    let json = serde_json::to_string(&model).expect("serialize");
    let parsed = FinancialModelSpec::from_json(&json).expect("valid model parses");
    assert_eq!(parsed.nodes.len(), 1);
}

#[test]
fn financial_statement_instrument_conversion_rejects_unsupported_types() {
    let unsupported = InstrumentJson::Equity(
        finstack_quant_valuations::instruments::equity::spot::Equity::example().expect("example"),
    );
    let error = FinancialStatementInstrument::try_from(unsupported)
        .expect_err("equity is not a capital-structure debt instrument");
    assert!(error.to_string().contains("equity"));
}
