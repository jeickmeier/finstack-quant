use super::dcf::{
    evaluate_corporate_dcf, evaluate_dcf, evaluate_dcf_from_results, resolve_exit_multiple_metric,
};
use super::path::evaluate_dcf_monte_carlo;
use super::sensitivity::{
    DcfRateShocks, DcfSensitivityEndpoint, DcfSensitivityGridSpec, DcfSensitivityShocks,
    DcfTerminalShocks, dcf_sensitivity_grid, dcf_sensitivity_on_results,
};
use super::types::{
    CorporateDcfRequest, DcfInputs, DcfOptions, DcfProjection, ExitMultipleBump, TerminalFcffBasis,
};
use super::wacc::{WaccInputs, dcf_sensitivity, wacc};
use super::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::wire::DecimalWire;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::AmountOrScalar;
use finstack_quant_statements::types::FinancialModelSpec;
use finstack_quant_valuations::instruments::equity::dcf_equity::{
    TerminalValueSpec, ValuationDiscounts,
};

#[test]
fn dated_dcf_matches_independent_value_and_rejects_another_cut() {
    use time::macros::date;

    let as_of = date!(2024 - 12 - 31);
    let forecast = PeriodId::annual(2025).unwrap();
    let mut builder = ModelBuilder::new("dated-dcf-pin");
    builder.periods("2025..2025", None).unwrap();
    builder.value_money("ufcf", &[(forecast, Money::from((100_i64, Currency::USD)))]);
    builder.with_meta("currency", serde_json::json!("USD"));
    builder.source_snapshot("authored-forecast-2024-12-31");
    let model = builder.build().unwrap();
    let mut request = CorporateDcfRequest {
        cut: StatementValuationCut {
            as_of,
            observed_through: as_of,
            currency: Currency::USD,
        },
        inputs: DcfInputs {
            wacc: 0.10,
            terminal_value: TerminalValueSpec::GordonGrowth {
                stable_growth_rate: 0.02,
            },
            ufcf_node: "ufcf".to_string(),
            net_debt_override: Some(50.0),
        },
        projection: DcfProjection {
            terminal_date: date!(2026 - 01 - 01),
            normalized_annual_fcff: Some(TerminalFcffBasis::Authored {
                amount: Money::from((100_i64, Currency::USD)),
            }),
            forward_remainder_node: None,
            stable_growth_ceiling: None,
        },
        options: DcfOptions::default(),
    };
    let results = request.cut.evaluate(&model, None).unwrap();
    let direct = evaluate_corporate_dcf(&model, &request, None).unwrap();
    let reused = evaluate_dcf_from_results(&model, &results, &request).unwrap();
    assert_eq!(direct.enterprise_value, reused.enterprise_value);
    assert!((reused.enterprise_value.amount() - 1_249.78).abs() < 0.005);
    assert!((reused.equity_value.amount() - 1_199.78).abs() < 0.005);
    request.projection.stable_growth_ceiling = Some(0.025);
    let points = dcf_sensitivity_grid(
        &model,
        &results,
        &request,
        &DcfSensitivityGridSpec {
            wacc_values: vec![0.09, 0.11],
            terminal_values: vec![0.01, 0.02, 0.03],
        },
    )
    .unwrap();
    assert_eq!(points.cells.len(), 2);
    assert!(points.cells.iter().all(|row| row.len() == 3));
    assert!(matches!(
        points.cells[0][2],
        DcfSensitivityEndpoint::Unavailable { level: 0.03, .. }
    ));
    let DcfSensitivityEndpoint::Priced { change, .. } = points.cells[0][1] else {
        panic!("admitted 9% WACC and 2% terminal growth must price");
    };
    assert!((change.amount() - 178.56).abs() < 0.02);

    let shock = DcfRateShocks {
        low: DecimalWire("0.01".parse().unwrap()),
        high: DecimalWire("0.01".parse().unwrap()),
    };
    let sensitivity = dcf_sensitivity_on_results(
        &model,
        &results,
        &request,
        &DcfSensitivityShocks {
            wacc: shock,
            terminal: DcfTerminalShocks::StableGrowth(shock),
        },
    )
    .unwrap();
    assert_eq!(sensitivity.pairs[0].parameter_id, "wacc");
    let growth = &sensitivity.pairs[1];
    let DcfSensitivityEndpoint::Priced { level, change, .. } = growth.low else {
        panic!("the admitted opposite terminal endpoint must price");
    };
    assert!((level - 0.01).abs() < 1e-15);
    assert!((change.amount() + 138.86).abs() < 0.02);
    assert!(matches!(
        growth.high,
        DcfSensitivityEndpoint::Unavailable { .. }
    ));

    use finstack_quant_statements::evaluator::{
        MonteCarloConfig, MonteCarloContext, ObservationCutoff,
    };
    let market = MarketContext::new();
    let context = Some(MonteCarloContext {
        market: &market,
        as_of,
        cutoff: ObservationCutoff::Through(as_of),
    });
    let config = MonteCarloConfig {
        n_paths: 3,
        seed: 7,
        metrics: vec!["ufcf".to_owned()],
        percentiles: vec![0.10, 0.90],
        include_path_data: true,
    };
    let paths = Evaluator::new()
        .evaluate_monte_carlo(&model, &config, context)
        .unwrap();
    let distribution = evaluate_dcf_monte_carlo(&model, &results, &paths, &request).unwrap();
    assert_eq!(distribution.paths.len(), 3);
    assert!(
        distribution
            .paths
            .iter()
            .all(|path| path.enterprise_value == reused.enterprise_value)
    );
    assert!(
        distribution
            .quantiles
            .iter()
            .all(|point| point.equity_value == reused.equity_value)
    );
    let no_paths = Evaluator::new()
        .evaluate_monte_carlo(
            &model,
            &MonteCarloConfig {
                include_path_data: false,
                ..config
            },
            context,
        )
        .unwrap();
    assert!(evaluate_dcf_monte_carlo(&model, &results, &no_paths, &request).is_err());
    request.cut.observed_through = date!(2024 - 12 - 30);
    assert!(evaluate_dcf_from_results(&model, &results, &request).is_err());
}

/// `DcfOptions` is the host input for DCF options: a partial document
/// fills the rest from `Default`, and an unknown key is rejected.
#[test]
fn dcf_options_partial_json_fills_defaults() {
    let options: DcfOptions = serde_json::from_str(
        r#"{"exit_multiple_metric_node": "ebitda", "exit_multiple_bump": {"relative": 0.1}}"#,
    )
    .expect("partial options parse");
    let defaults = DcfOptions::default();
    assert_eq!(options.exit_multiple_metric_node.as_deref(), Some("ebitda"));
    assert!(matches!(
        options.exit_multiple_bump,
        ExitMultipleBump::Relative(bump) if (bump - 0.1).abs() < 1e-12
    ));
    assert_eq!(options.mid_year_convention, defaults.mid_year_convention);
    assert!((options.wacc_sensitivity_bump - defaults.wacc_sensitivity_bump).abs() < 1e-12);
    assert!((options.max_stable_growth_rate - defaults.max_stable_growth_rate).abs() < 1e-12);

    let empty: DcfOptions = serde_json::from_str("{}").expect("empty options parse");
    assert!(empty.exit_multiple_metric_node.is_none());
    assert!(serde_json::from_str::<DcfOptions>(r#"{"exit_multiple_node": "ebitda"}"#).is_err());
}

#[test]
fn evaluate_dcf_requires_explicit_currency_metadata() {
    let model = {
        let mut model_builder_1 = ModelBuilder::new("dcf-missing-currency");
        model_builder_1
            .periods("2025Q1..Q2", None)
            .expect("valid periods");
        model_builder_1.value(
            "ufcf",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        );
        model_builder_1.value(
            "total_debt",
            &[(
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                AmountOrScalar::scalar(50_000.0),
            )],
        );
        model_builder_1.value(
            "cash",
            &[(
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                AmountOrScalar::scalar(10_000.0),
            )],
        );
        model_builder_1.build().expect("valid model")
    };

    let result = evaluate_dcf(
        &model,
        &DcfInputs {
            wacc: 0.10,
            terminal_value: TerminalValueSpec::GordonGrowth {
                stable_growth_rate: 0.02,
            },
            ufcf_node: "ufcf".to_string(),
            net_debt_override: None,
        },
        &DcfOptions::default(),
        None,
        None,
    );
    assert!(result.is_err(), "currency metadata must be explicit");
}

#[test]
fn evaluate_dcf_requires_balance_sheet_inputs_without_override() {
    let model = {
        let mut model_builder_1 = ModelBuilder::new("dcf-missing-balance-sheet");
        model_builder_1
            .periods("2025Q1..Q2", None)
            .expect("valid periods");
        model_builder_1.value(
            "ufcf",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        );
        model_builder_1.with_meta("currency", serde_json::json!("USD"));
        model_builder_1.build().expect("valid model")
    };

    let result = evaluate_dcf(
        &model,
        &DcfInputs {
            wacc: 0.10,
            terminal_value: TerminalValueSpec::GordonGrowth {
                stable_growth_rate: 0.02,
            },
            ufcf_node: "ufcf".to_string(),
            net_debt_override: None,
        },
        &DcfOptions::default(),
        None,
        None,
    );
    assert!(
        result.is_err(),
        "missing debt and cash inputs must fail without an override"
    );
}

#[test]
fn valuation_discount_reconciles_the_equity_bridge() {
    let model = sensitivity_model();
    let gordon = || TerminalValueSpec::GordonGrowth {
        stable_growth_rate: 0.02,
    };
    let plain = evaluate_dcf(
        &model,
        &DcfInputs {
            wacc: 0.10,
            terminal_value: gordon(),
            ufcf_node: "ufcf".to_string(),
            net_debt_override: Some(200.0),
        },
        &DcfOptions::default(),
        None,
        None,
    )
    .expect("plain DCF");
    assert_eq!(plain.valuation_discount.amount(), 0.0);

    let discounted = evaluate_dcf(
        &model,
        &DcfInputs {
            wacc: 0.10,
            terminal_value: gordon(),
            ufcf_node: "ufcf".to_string(),
            net_debt_override: Some(200.0),
        },
        &DcfOptions {
            valuation_discounts: Some(ValuationDiscounts {
                dlom: Some(0.25),
                ..ValuationDiscounts::default()
            }),
            ..DcfOptions::default()
        },
        None,
        None,
    )
    .expect("discounted DCF");
    let bridge = discounted.enterprise_value.amount()
        - discounted.net_debt.amount()
        - discounted.valuation_discount.amount();
    assert!((bridge - discounted.equity_value.amount()).abs() < 0.01);
    let pre_discount = discounted.enterprise_value.amount() - discounted.net_debt.amount();
    assert!((discounted.valuation_discount.amount() - 0.25 * pre_discount).abs() < 0.01);
}

/// The exported working reproduces every reported DCF number: rows sum to
/// the explicit PV, explicit plus terminal PV is EV, the terminal PV is the
/// undiscounted terminal value over its compounding, and the bridge nets
/// to net debt.
#[test]
fn dcf_detail_reconciles_to_enterprise_and_equity_value() {
    let model = sensitivity_model();
    for mid_year_convention in [false, true] {
        let result = evaluate_dcf(
            &model,
            &DcfInputs {
                wacc: 0.10,
                terminal_value: TerminalValueSpec::GordonGrowth {
                    stable_growth_rate: 0.02,
                },
                ufcf_node: "ufcf".to_string(),
                net_debt_override: Some(200.0),
            },
            &DcfOptions {
                mid_year_convention,
                ..DcfOptions::default()
            },
            None,
            None,
        )
        .expect("DCF");
        let wacc = result.wacc.expect("wacc");
        assert_eq!(wacc, 0.10);
        let dcf = result.dcf_instrument.as_ref().expect("instrument");
        assert_eq!(result.periods.len(), dcf.flows.len());
        for (row, (date, amount)) in result.periods.iter().zip(&dcf.flows) {
            assert_eq!(row.date, *date);
            assert_eq!(row.free_cash_flow, amount.amount());
            let factor = (1.0 + wacc).powf(-row.discount_years);
            assert!((row.discount_factor - factor).abs() < 1e-15);
            assert!((row.present_value - row.free_cash_flow * row.discount_factor).abs() < 1e-9);
        }
        let rows_pv: f64 = result.periods.iter().map(|row| row.present_value).sum();
        // The owner sums the same rows with compensation.
        let owner_pv = dcf.calculate_pv_explicit_flows().expect("valid PV");
        assert!((rows_pv - owner_pv).abs() < 1e-12);
        let pv_explicit = result.pv_explicit.expect("pv_explicit").amount();
        assert!((rows_pv - pv_explicit).abs() < 0.01);
        assert!(
            (rows_pv + result.terminal_value_pv.amount() - result.enterprise_value.amount()).abs()
                < 0.02
        );

        let terminal_value = result.terminal_value.expect("terminal value").amount();
        let terminal_years = result.terminal_discount_years.expect("terminal tenor");
        assert!(
            (terminal_value / (1.0 + wacc).powf(terminal_years)
                - result.terminal_value_pv.amount())
            .abs()
                < 0.01
        );
        assert!(terminal_value > result.terminal_value_pv.amount());

        // A flat net-debt override is reported as gross debt, zero cash.
        let bridge = result.equity_bridge.as_ref().expect("bridge");
        assert_eq!((bridge.total_debt, bridge.cash), (200.0, 0.0));
        assert!((bridge.total_debt - bridge.cash - result.net_debt.amount()).abs() < 0.01);
        assert!((bridge.net_adjustment() - result.net_debt.amount()).abs() < 0.01);
        assert!(
            (result.enterprise_value.amount()
                - bridge.net_adjustment()
                - result.equity_value.amount())
            .abs()
                < 0.02
        );

        // The detail crosses the wire; the instrument does not.
        let json = serde_json::to_string(&result).expect("serialize");
        let back: CorporateValuationResult = serde_json::from_str(&json).expect("round trip");
        assert_eq!(back.periods, result.periods);
        assert_eq!(back.pv_explicit, result.pv_explicit);
        assert_eq!(back.terminal_value, result.terminal_value);
        assert!(back.dcf_instrument.is_none());
    }
}

/// JSON written before the DCF detail fields existed still loads.
#[test]
fn corporate_valuation_result_reads_json_without_detail() {
    let legacy = r#"{"equity_value":{"amount":"90","currency":"USD"},
        "enterprise_value":{"amount":"100","currency":"USD"},
        "net_debt":{"amount":"10","currency":"USD"},
        "other_equity_claims":{"amount":"0","currency":"USD"},
        "valuation_discount":{"amount":"0","currency":"USD"},
        "terminal_value_pv":{"amount":"60","currency":"USD"},
        "equity_value_per_share":null,"diluted_shares":null}"#;
    let result: CorporateValuationResult = serde_json::from_str(legacy).expect("legacy");
    assert!(result.periods.is_empty());
    assert!(result.pv_explicit.is_none() && result.equity_bridge.is_none());
}

fn sensitivity_model() -> FinancialModelSpec {
    {
        let mut model_builder_1 = ModelBuilder::new("dcf-sensitivity");
        model_builder_1
            .periods("2025..2027", None)
            .expect("valid periods");
        model_builder_1.value_money(
            "ufcf",
            &[
                (
                    PeriodId::annual(2025).expect("valid annual period"),
                    Money::from((100_i64, Currency::USD)),
                ),
                (
                    PeriodId::annual(2026).expect("valid annual period"),
                    Money::from((110_i64, Currency::USD)),
                ),
                (
                    PeriodId::annual(2027).expect("valid annual period"),
                    Money::from((120_i64, Currency::USD)),
                ),
            ],
        );
        model_builder_1.with_meta("currency", serde_json::json!("USD"));
        model_builder_1.build().expect("valid model")
    }
}

#[test]
fn dcf_sensitivity_ranks_wacc_and_growth_as_deltas() {
    let model = sensitivity_model();
    let result = dcf_sensitivity(
        &model,
        0.10,
        TerminalValueSpec::GordonGrowth {
            stable_growth_rate: 0.02,
        },
        "ufcf",
        Some(0.0),
        &DcfOptions::default(),
    )
    .expect("sensitivity runs");

    assert_eq!(result.entries.len(), 2, "wacc and terminal growth");
    assert_eq!(
        result.baseline_enterprise_value.currency().to_string(),
        "USD"
    );
    assert!(result.baseline_enterprise_value.amount() > 0.0);

    let wacc_entry = result
        .entries
        .iter()
        .find(|entry| entry.parameter_id == "wacc")
        .expect("wacc entry present");
    // A lower WACC raises EV; a higher WACC lowers it.
    assert!(wacc_entry.downside > 0.0, "{wacc_entry:?}");
    assert!(wacc_entry.upside < 0.0, "{wacc_entry:?}");

    let growth_entry = result
        .entries
        .iter()
        .find(|entry| entry.parameter_id == "terminal_growth")
        .expect("growth entry present");
    // Growth moves EV the other way round.
    assert!(growth_entry.downside < 0.0, "{growth_entry:?}");
    assert!(growth_entry.upside > 0.0, "{growth_entry:?}");

    // Widest absolute swing first.
    let swings: Vec<f64> = result.entries.iter().map(|e| e.swing().abs()).collect();
    assert!(swings[0] >= swings[1], "entries must be sorted: {swings:?}");

    // 10% WACC less a 100 bp bump stays clear of the 2% growth floor.
    assert!(!result.wacc_down_clamped);
    assert!((result.wacc_down - 0.09).abs() < 1e-12);
    assert!(!result.terminal_growth_up_clamped);
}

#[test]
fn dcf_sensitivity_clamps_shocks_against_the_growth_denominator() {
    let model = sensitivity_model();
    let options = DcfOptions {
        // A 300 bp bump on a 50 bp WACC-to-growth spread would invert the
        // terminal denominator in both directions.
        wacc_sensitivity_bump: 0.03,
        max_stable_growth_rate: 0.06,
        ..Default::default()
    };
    let result = dcf_sensitivity(
        &model,
        0.06,
        TerminalValueSpec::GordonGrowth {
            stable_growth_rate: 0.055,
        },
        "ufcf",
        Some(0.0),
        &options,
    )
    .expect("sensitivity runs");

    assert!(result.wacc_down_clamped, "wacc down-shock must be clamped");
    // Floored at growth + epsilon = 0.055 + 0.005.
    assert!(
        (result.wacc_down - 0.060).abs() < 1e-12,
        "{:?}",
        result.wacc_down
    );

    assert!(
        result.terminal_growth_up_clamped,
        "growth up-shock must be clamped"
    );
    // Capped at wacc - epsilon = 0.06 - 0.005.
    assert_eq!(result.terminal_growth_up, Some(0.055));

    assert!(
        result.entries.iter().all(|e| e.swing().is_finite()),
        "clamping must keep every swing finite: {:?}",
        result.entries
    );
}

#[test]
fn dcf_sensitivity_shocks_the_exit_multiple() {
    let model = sensitivity_model();
    let result = dcf_sensitivity(
        &model,
        0.10,
        TerminalValueSpec::ExitMultiple {
            terminal_metric: Some(150.0),
            multiple: 8.0,
        },
        "ufcf",
        Some(0.0),
        &DcfOptions::default(),
    )
    .expect("sensitivity runs");

    assert_eq!(result.entries.len(), 2, "wacc and exit multiple");
    assert!(result.terminal_growth_up.is_none());
    let exit = result
        .entries
        .iter()
        .find(|entry| entry.parameter_id == "exit_multiple")
        .expect("exit multiple entry present");
    assert!(exit.downside < 0.0 && exit.upside > 0.0, "{exit:?}");
}

fn inputs(
    equity_weight: &str,
    cost_of_equity: f64,
    debt_weight: &str,
    cost_of_debt: f64,
    tax_rate: f64,
) -> WaccInputs {
    WaccInputs {
        equity_weight: DecimalWire(equity_weight.parse().expect("equity fraction")),
        cost_of_equity,
        debt_weight: DecimalWire(debt_weight.parse().expect("debt fraction")),
        cost_of_debt,
        tax_rate,
    }
}

#[test]
fn wacc_blends_equity_and_after_tax_debt() {
    // 0.6 * 0.115 + 0.4 * 0.06 * 0.75 = 0.069 + 0.018 = 0.087
    let rate = wacc(&inputs("0.6", 0.115, "0.4", 0.06, 0.25)).expect("valid inputs");
    assert!((rate - 0.087).abs() < 1e-12, "{rate}");

    // An all-equity firm prices at the cost of equity.
    let unlevered = wacc(&inputs("1.0", 0.115, "0.0", 0.06, 0.25)).expect("valid inputs");
    assert!((unlevered - 0.115).abs() < 1e-12, "{unlevered}");

    // A zero tax rate removes the shield.
    let untaxed = wacc(&inputs("0.6", 0.115, "0.4", 0.06, 0.0)).expect("valid inputs");
    assert!((untaxed - 0.093).abs() < 1e-12, "{untaxed}");
}

#[test]
fn wacc_rejects_invalid_weights_and_rates() {
    assert!(
        wacc(&inputs("0.6", 0.115, "0.3", 0.06, 0.25)).is_err(),
        "weights must sum to 1"
    );
    assert!(
        wacc(&inputs("1.2", 0.115, "-0.2", 0.06, 0.25)).is_err(),
        "weights must be non-negative"
    );
    assert!(
        wacc(&inputs("0.6", 0.115, "0.4", 0.06, 1.5)).is_err(),
        "tax rate must be in [0, 1]"
    );
    assert!(
        wacc(&inputs("0.6", f64::NAN, "0.4", 0.06, 0.25)).is_err(),
        "inputs must be finite"
    );
}

#[test]
fn wacc_checks_exact_complements_and_component_cost_domains() {
    assert!(
        wacc(&inputs(
            "0.6000000000000000000000000001",
            0.115,
            "0.4",
            0.06,
            0.25
        ))
        .is_err()
    );
    assert!(
        wacc(&inputs(
            "0.6000000000000000000000000001",
            0.115,
            "0.3999999999999999999999999999",
            0.06,
            0.25
        ))
        .is_ok()
    );
    assert!(wacc(&inputs("1", 0.1, "0", -1.0, 0.25)).is_err());
}

fn exit_multiple_model() -> FinancialModelSpec {
    {
        let mut model_builder_1 = ModelBuilder::new("exit-multiple-node");
        model_builder_1
            .periods("2025..2027", None)
            .expect("valid periods");
        model_builder_1.value_money(
            "ufcf",
            &[
                (
                    PeriodId::annual(2025).expect("valid annual period"),
                    Money::from((100_i64, Currency::USD)),
                ),
                (
                    PeriodId::annual(2026).expect("valid annual period"),
                    Money::from((110_i64, Currency::USD)),
                ),
                (
                    PeriodId::annual(2027).expect("valid annual period"),
                    Money::from((120_i64, Currency::USD)),
                ),
            ],
        );
        model_builder_1.value(
            "ebitda",
            &[
                (
                    PeriodId::annual(2025).expect("valid annual period"),
                    AmountOrScalar::amount(100.0, Currency::USD).unwrap(),
                ),
                (
                    PeriodId::annual(2026).expect("valid annual period"),
                    AmountOrScalar::amount(125.0, Currency::USD).unwrap(),
                ),
                (
                    PeriodId::annual(2027).expect("valid annual period"),
                    AmountOrScalar::amount(150.0, Currency::USD).unwrap(),
                ),
            ],
        );
        model_builder_1.with_meta("currency", serde_json::json!("USD"));
        model_builder_1.build().expect("valid model")
    }
}

#[test]
fn exit_multiple_metric_node_uses_last_forecast_period() {
    let model = exit_multiple_model();
    let options = DcfOptions {
        exit_multiple_metric_node: Some("ebitda".into()),
        ..Default::default()
    };
    let result = evaluate_dcf(
        &model,
        &DcfInputs {
            wacc: 0.10,
            terminal_value: TerminalValueSpec::ExitMultiple {
                // Explicit metric is ignored when the node is set.
                terminal_metric: None,
                multiple: 8.0,
            },
            ufcf_node: "ufcf".to_string(),
            net_debt_override: Some(0.0),
        },
        &options,
        None,
        None,
    )
    .expect("DCF evaluation");

    let dcf = result.dcf_instrument.expect("instrument");
    let tv = dcf.calculate_terminal_value().expect("terminal value");
    assert!(
        (tv - 1_200.0).abs() < 1e-9,
        "last-forecast EBITDA 150 × 8x must yield TV 1200, got {tv}"
    );
}

#[test]
fn exit_multiple_metric_requires_complete_trailing_year() {
    use finstack_quant_core::dates::{FiscalConfig, build_fiscal_periods, build_periods};
    for plan in [
        build_periods("2024Q1..Q4", Some("2024Q2")).unwrap(),
        build_fiscal_periods(
            "FY2024Q1..FY2024Q4",
            FiscalConfig::new(3, 1).unwrap(),
            Some("FY2024Q2"),
        )
        .unwrap(),
    ] {
        let vals: Vec<_> = plan
            .iter()
            .enumerate()
            .map(|(i, p)| {
                (
                    p.id,
                    AmountOrScalar::amount((i + 1) as f64 * 10., Currency::USD).unwrap(),
                )
            })
            .collect();
        let mut model = {
            let mut model_builder_1 = ModelBuilder::new("ltm");
            model_builder_1.periods_explicit(plan).unwrap();
            model_builder_1.value("ebitda", &vals);
            model_builder_1.with_meta("currency", serde_json::json!("USD"));
            model_builder_1.build().unwrap()
        };
        let results = Evaluator::new().evaluate(&model).unwrap();
        let terminal = TerminalValueSpec::ExitMultiple {
            terminal_metric: Some(999.),
            multiple: 8.,
        };
        let resolved =
            resolve_exit_multiple_metric(&model, &results, terminal.clone(), Some("ebitda"))
                .unwrap();
        let TerminalValueSpec::ExitMultiple {
            terminal_metric, ..
        } = resolved
        else {
            unreachable!()
        };
        assert_eq!(terminal_metric, Some(100.));
        model.periods = finstack_quant_core::dates::PeriodPlan::try_new(
            model.periods.get_calendar(),
            model.periods[1..].to_vec(),
        )
        .unwrap();
        assert!(
            resolve_exit_multiple_metric(&model, &results, terminal.clone(), Some("ebitda"))
                .is_err()
        );
        assert!(resolve_exit_multiple_metric(&model, &results, terminal, None).is_ok());
        for bad in [0.0, -5.0, f64::NAN] {
            let unset = TerminalValueSpec::ExitMultiple {
                terminal_metric: Some(bad),
                multiple: 8.,
            };
            assert!(
                resolve_exit_multiple_metric(&model, &results, unset, None).is_err(),
                "terminal_metric {bad} without a metric node must be rejected"
            );
        }
    }
}

#[test]
fn exit_multiple_metric_node_missing_errors() {
    let model = exit_multiple_model();
    let options = DcfOptions {
        exit_multiple_metric_node: Some("missing_ebitda".into()),
        ..Default::default()
    };
    let result = evaluate_dcf(
        &model,
        &DcfInputs {
            wacc: 0.10,
            terminal_value: TerminalValueSpec::ExitMultiple {
                terminal_metric: Some(150.0),
                multiple: 8.0,
            },
            ufcf_node: "ufcf".to_string(),
            net_debt_override: Some(0.0),
        },
        &options,
        None,
        None,
    );
    assert!(
        result.is_err(),
        "missing exit-multiple metric node must fail"
    );
}

#[test]
fn exit_multiple_explicit_metric_used_when_node_is_none() {
    let model = exit_multiple_model();
    let result = evaluate_dcf(
        &model,
        &DcfInputs {
            wacc: 0.10,
            terminal_value: TerminalValueSpec::ExitMultiple {
                terminal_metric: Some(150.0),
                multiple: 8.0,
            },
            ufcf_node: "ufcf".to_string(),
            net_debt_override: Some(0.0),
        },
        &DcfOptions::default(),
        None,
        None,
    )
    .expect("DCF evaluation");

    let dcf = result.dcf_instrument.expect("instrument");
    let tv = dcf.calculate_terminal_value().expect("terminal value");
    assert!(
        (tv - 1_200.0).abs() < 1e-9,
        "explicit metric 150 × 8 = 1200, got {tv}"
    );
}
