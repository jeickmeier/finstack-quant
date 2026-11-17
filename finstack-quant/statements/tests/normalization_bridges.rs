//! Ordered normalization bridges to a separately named adjusted metric
//! (STATEMENTS-EVALUATION-015).
#![allow(clippy::expect_used)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{PeriodId, PeriodKind};
use finstack_quant_core::money::Money;
use finstack_quant_statements::adjustments::{
    Adjustment, AdjustmentCap, CapBaseMode, NormalizationConfig, merge_into_results, normalize,
};
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::{
    CheckCategory, CheckOutcome, CheckSuite, FormulaCheckMode, FormulaCheckSpec, PeriodScope,
    Severity,
};
use finstack_quant_statements::evaluator::{
    CellStatus, Evaluator, ResultCell, RollupSpec, StatementResult, UnavailableReason,
};
use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec, RollupMethod};
use indexmap::IndexMap;

fn q(quarter: u8) -> PeriodId {
    PeriodId::quarter(2025, quarter).expect("valid quarter")
}

fn usd(amount: &str) -> AmountOrScalar {
    AmountOrScalar::Amount(Money::from_decimal_str(amount, Currency::USD).expect("amount"))
}

fn usd_series(values: &[(u8, &str)]) -> Vec<(PeriodId, AmountOrScalar)> {
    values
        .iter()
        .map(|(quarter, amount)| (q(*quarter), usd(amount)))
        .collect()
}

/// Two quarters: EBITDA of USD 100 and USD -50, revenue of USD 1,000 in the
/// first quarter only, and D&A.
fn usd_model() -> (FinancialModelSpec, StatementResult) {
    let mut builder = ModelBuilder::new("normalization");
    builder.periods("2025Q1..Q2", None).expect("valid periods");
    builder.value("ebitda", &usd_series(&[(1, "100"), (2, "-50")]));
    builder.value("revenue", &usd_series(&[(1, "1000")]));
    builder.value("d_and_a", &usd_series(&[(1, "15"), (2, "15")]));
    builder.value("ebit", &usd_series(&[(1, "85"), (2, "-65")]));
    builder.value_scalar("headcount", &[(q(1), 10.0), (q(2), 12.0)]);
    for node in ["ebitda", "revenue", "d_and_a", "ebit", "headcount"] {
        builder.rollup(node, RollupMethod::Sum).expect("known node");
    }
    let model = builder.build().expect("valid model");
    let results = Evaluator::new().evaluate(&model).expect("evaluation");
    (model, results)
}

fn fixed(id: &str, amounts: &[(u8, &str)]) -> Adjustment {
    Adjustment::fixed(id, id, usd_series(amounts).into_iter().collect())
}

fn target_cap(fraction: f64, base_mode: CapBaseMode) -> AdjustmentCap {
    AdjustmentCap::Reference {
        base_node: "ebitda".into(),
        fraction,
        base_mode,
    }
}

fn config(adjustments: Vec<Adjustment>) -> NormalizationConfig {
    adjustments.into_iter().fold(
        NormalizationConfig::new("ebitda", "adjusted_ebitda"),
        |config, adjustment| config.add_adjustment(adjustment).expect("unique id"),
    )
}

/// The record's example: reported USD 100, plus USD 20, plus USD 40 capped
/// at 30% of the target.
fn capped_example(base_mode: CapBaseMode) -> NormalizationConfig {
    config(vec![
        fixed("first", &[(1, "20")]),
        fixed("second", &[(1, "40")]).with_cap(target_cap(0.30, base_mode)),
    ])
}

#[test]
fn a_reported_cap_gives_150_and_a_progressive_cap_gives_156() {
    let (_, results) = usd_model();

    let reported = &normalize(&results, &capped_example(CapBaseMode::Reported)).expect("bridge")[0];
    assert_eq!(reported.base_value, Some(usd("100")));
    let second = &reported.adjustments[1];
    assert_eq!(second.raw_amount, usd("40"));
    assert_eq!(second.cap_base, Some(usd("100")));
    assert_eq!(second.cap_limit, Some(usd("30")));
    assert_eq!(second.capped_amount, usd("30"));
    assert!(second.is_capped);
    assert_eq!(reported.adjustments[0].adjusted_value, usd("120"));
    assert_eq!(second.adjusted_value, usd("150"));
    assert_eq!(reported.final_value, Some(usd("150")));

    let progressive =
        &normalize(&results, &capped_example(CapBaseMode::Progressive)).expect("bridge")[0];
    let second = &progressive.adjustments[1];
    assert_eq!(second.cap_base, Some(usd("120")));
    assert_eq!(second.cap_limit, Some(usd("36")));
    assert_eq!(second.capped_amount, usd("36"));
    assert_eq!(progressive.final_value, Some(usd("156")));
    // The first adjustment has no cap, so it records none.
    assert_eq!(progressive.adjustments[0].cap_limit, None);
    assert!(!progressive.adjustments[0].is_capped);
}

#[test]
fn a_cap_keeps_the_sign_gives_no_room_on_a_negative_base_and_may_exceed_one() {
    let (_, results) = usd_model();
    // A deduction capped at 10% of reported EBITDA stays a deduction.
    let deduction = config(vec![
        fixed("d", &[(1, "-40"), (2, "-40")]).with_cap(target_cap(0.10, CapBaseMode::Reported)),
    ]);
    let rows = normalize(&results, &deduction).expect("bridge");
    assert_eq!(rows[0].adjustments[0].capped_amount, usd("-10"));
    assert_eq!(rows[0].final_value, Some(usd("90")));
    // Q2 EBITDA is negative: no cap room, so nothing is applied.
    assert_eq!(rows[1].adjustments[0].cap_base, Some(usd("-50")));
    assert_eq!(rows[1].adjustments[0].cap_limit, Some(usd("0")));
    assert_eq!(rows[1].adjustments[0].capped_amount, usd("0"));
    assert_eq!(rows[1].final_value, Some(usd("-50")));

    // A fraction above 100% is a valid cap.
    let wide = config(vec![
        fixed("a", &[(1, "140")]).with_cap(target_cap(1.25, CapBaseMode::Reported)),
    ]);
    let row = &normalize(&results, &wide).expect("bridge")[0];
    assert_eq!(row.adjustments[0].cap_limit, Some(usd("125")));
    assert_eq!(row.final_value, Some(usd("225")));

    // An absolute cap is a typed amount; an omitted fixed amount is zero.
    let absolute = config(vec![
        fixed("a", &[(1, "40")]).with_cap(AdjustmentCap::Absolute { amount: usd("25") }),
    ]);
    let rows = normalize(&results, &absolute).expect("bridge");
    assert_eq!(rows[0].final_value, Some(usd("125")));
    assert_eq!(rows[1].adjustments[0].raw_amount, usd("0"));
    assert_eq!(rows[1].final_value, Some(usd("-50")));
}

#[test]
fn an_unavailable_input_leaves_the_period_unavailable_with_a_reason() {
    let (_, mut results) = usd_model();
    // Revenue has no Q2 observation: a percentage of it has no value.
    let percentage = config(vec![Adjustment::percentage("p", "p", "revenue", 0.01)]);
    let rows = normalize(&results, &percentage).expect("bridge");
    assert_eq!(rows[0].final_value, Some(usd("110")));
    assert_eq!(rows[1].final_value, None);
    assert_eq!(rows[1].unavailable, Some(UnavailableReason::MissingInput));
    assert_eq!(rows[1].base_value, Some(usd("-50")));
    assert!(rows[1].adjustments.is_empty());

    // A cap base without a value.
    let cap_base = config(vec![fixed("a", &[(2, "5")]).with_cap(
        AdjustmentCap::Reference {
            base_node: "revenue".into(),
            fraction: 0.5,
            base_mode: CapBaseMode::Reported,
        },
    )]);
    let rows = normalize(&results, &cap_base).expect("bridge");
    assert_eq!(rows[1].unavailable, Some(UnavailableReason::MissingInput));

    // The target itself: the reason of its cell is kept.
    results.set_cell(
        "ebitda",
        q(1),
        ResultCell::unavailable(UnavailableReason::HiddenByCutoff),
    );
    let rows = normalize(&results, &config(vec![fixed("a", &[(1, "5")])])).expect("bridge");
    assert_eq!(rows[0].base_value, None);
    assert_eq!(rows[0].unavailable, Some(UnavailableReason::HiddenByCutoff));

    // Publication writes the unavailable cell with its reason.
    merge_into_results(&mut results, &config(vec![fixed("a", &[(1, "5")])])).expect("publish");
    let cell = results.get_cell("adjusted_ebitda", &q(1)).expect("cell");
    assert_eq!(cell.status, CellStatus::Unavailable);
    assert_eq!(cell.reason, Some(UnavailableReason::HiddenByCutoff));
    assert_eq!(
        results.get_money("adjusted_ebitda", &q(2)).map(Into::into),
        Some(usd("-50"))
    );
}

#[test]
fn the_configuration_is_validated_before_any_period_is_calculated() {
    let (_, results) = usd_model();
    let rejected = |config: NormalizationConfig, needle: &str| {
        let error = normalize(&results, &config).expect_err(needle).to_string();
        assert!(error.contains(needle), "{needle}: {error}");
    };
    let annual = PeriodId::annual(2025).expect("valid year");

    // A period key outside the plan, even though no period would read it.
    rejected(
        config(vec![Adjustment::fixed(
            "a",
            "a",
            IndexMap::from([(annual, usd("1"))]),
        )]),
        "not a period",
    );
    // Units and currencies.
    rejected(
        config(vec![Adjustment::fixed(
            "a",
            "a",
            IndexMap::from([(q(1), AmountOrScalar::scalar(1.0))]),
        )]),
        "units",
    );
    let eur = AmountOrScalar::Amount(Money::from_decimal_str("1", Currency::EUR).expect("amount"));
    rejected(
        config(vec![Adjustment::fixed(
            "a",
            "a",
            IndexMap::from([(q(1), eur)]),
        )]),
        "units",
    );
    rejected(
        config(vec![Adjustment::percentage("a", "a", "headcount", 0.1)]),
        "different units",
    );
    rejected(
        config(vec![
            fixed("a", &[(1, "1")]).with_cap(AdjustmentCap::Absolute { amount: eur }),
        ]),
        "units",
    );
    // References.
    rejected(
        config(vec![Adjustment::percentage("a", "a", "ghost", 0.1)]),
        "'ghost'",
    );
    rejected(NormalizationConfig::new("ghost", "adjusted"), "'ghost'");
    // Numbers.
    rejected(
        config(vec![Adjustment::percentage("a", "a", "revenue", f64::NAN)]),
        "finite",
    );
    rejected(
        config(vec![
            fixed("a", &[(1, "1")]).with_cap(target_cap(-0.1, CapBaseMode::Reported)),
        ]),
        "non-negative",
    );
    rejected(
        config(vec![
            fixed("a", &[(1, "1")]).with_cap(AdjustmentCap::Absolute { amount: usd("-1") }),
        ]),
        "non-negative",
    );
    // Identifiers and the output node.
    let mut duplicate = config(vec![fixed("a", &[(1, "1")])]);
    duplicate.adjustments.push(fixed("a", &[(1, "1")]));
    rejected(duplicate, "Duplicate");
    rejected(NormalizationConfig::new("ebitda", "ebitda"), "new id");
    rejected(NormalizationConfig::new("ebitda", ""), "new id");

    // Results without a period plan.
    let mut hand_built = results.clone();
    hand_built.periods = None;
    assert!(normalize(&hand_built, &config(vec![])).is_err());
}

/// One scalar quarter with reported value 1.
fn scalar_results() -> StatementResult {
    let mut builder = ModelBuilder::new("scalar");
    builder.periods("2025Q1..Q1", None).expect("valid periods");
    builder.value_scalar("x", &[(q(1), 1.0)]);
    Evaluator::new()
        .evaluate(&builder.build().expect("valid model"))
        .expect("evaluation")
}

#[test]
fn an_infinite_intermediate_with_a_finite_total_publishes_nothing() {
    let mut results = scalar_results();
    let step = |id: &str, value: f64| {
        Adjustment::fixed(
            id,
            id,
            IndexMap::from([(q(1), AmountOrScalar::scalar(value))]),
        )
    };
    // The four amounts cancel exactly, but the running value overflows.
    let overflowing = [
        step("a", f64::MAX),
        step("b", f64::MAX),
        step("c", -f64::MAX),
        step("d", -f64::MAX),
    ]
    .into_iter()
    .fold(NormalizationConfig::new("x", "adjusted_x"), |config, a| {
        config.add_adjustment(a).expect("unique id")
    });
    assert!(normalize(&results, &overflowing).is_err());
    let before = results.clone();
    assert!(merge_into_results(&mut results, &overflowing).is_err());
    assert_eq!(results.cells, before.cells);
    assert!(results.normalizations.is_empty());

    // Scalar accumulation is compensated across scales.
    let cancelling = [
        step("a", 1e16),
        step("b", 1.0),
        step("c", -1e16),
        step("d", 1.0),
    ]
    .into_iter()
    .fold(NormalizationConfig::new("x", "adjusted_x"), |config, a| {
        config.add_adjustment(a).expect("unique id")
    });
    let row = &normalize(&results, &cancelling).expect("bridge")[0];
    assert_eq!(row.final_value, Some(AmountOrScalar::scalar(3.0)));
}

#[test]
fn publication_needs_a_new_node_id_and_records_the_configuration() {
    let (_, mut results) = usd_model();
    let bridge = capped_example(CapBaseMode::Reported);
    let rows = merge_into_results(&mut results, &bridge).expect("publish");
    assert_eq!(rows[0].final_value, Some(usd("150")));
    assert_eq!(
        results.get_money("adjusted_ebitda", &q(1)).map(Into::into),
        Some(usd("150"))
    );
    // The reported line is unchanged and the result names the bridge.
    assert_eq!(
        results.get_money("ebitda", &q(1)).map(Into::into),
        Some(usd("100"))
    );
    assert_eq!(results.normalizations.get("adjusted_ebitda"), Some(&bridge));
    assert!(results.provenance.is_some());

    // A second publication under the same id, an existing node and a result
    // with no source run are all rejected.
    for (output, needle) in [
        ("adjusted_ebitda", "already hold"),
        ("revenue", "already hold"),
    ] {
        let mut again = bridge.clone();
        again.output_node = output.into();
        let error = merge_into_results(&mut results, &again)
            .expect_err("collision")
            .to_string();
        assert!(error.contains(needle), "{error}");
    }
    let (_, mut unlinked) = usd_model();
    unlinked.provenance = None;
    assert!(merge_into_results(&mut unlinked, &bridge).is_err());
    assert!(!unlinked.cells.contains_key("adjusted_ebitda"));
}

#[test]
fn included_depreciation_is_added_back_once() {
    let (_, results) = usd_model();
    let bridge = NormalizationConfig::new("ebit", "ebitda_from_ebit")
        .add_adjustment(Adjustment::percentage("d_and_a", "D&A", "d_and_a", 1.0))
        .expect("unique id");
    let rows = normalize(&results, &bridge).expect("bridge");
    assert_eq!(rows[0].final_value, Some(usd("100")));
    assert_eq!(rows[1].final_value, Some(usd("-50")));
    // The same add-back cannot be listed twice.
    assert!(
        bridge
            .add_adjustment(Adjustment::percentage("d_and_a", "D&A", "d_and_a", 1.0))
            .is_err()
    );
}

#[test]
fn a_check_on_the_adjusted_basis_references_the_adjusted_node() {
    let (model, mut results) = usd_model();
    merge_into_results(&mut results, &capped_example(CapBaseMode::Reported)).expect("publish");
    let check = |id: &str, formula: &str| FormulaCheckSpec {
        id: id.into(),
        name: id.into(),
        category: CheckCategory::CreditReasonableness,
        severity: Severity::Error,
        mode: FormulaCheckMode::Predicate,
        scope: PeriodScope::AllPeriods,
        formula: formula.into(),
        eligibility: Some("ebitda > 0 * ebitda".into()),
        message_template: "{period}".into(),
        tolerance: None,
    };
    // Eight times D&A is USD 120: the floor holds only on the adjusted
    // basis (150 against a reported 100). Q2 EBITDA is negative and out of
    // scope.
    let suite = CheckSuite::builder("margins")
        .add_check(check("reported", "ebitda >= d_and_a * 8"))
        .add_check(check("adjusted", "adjusted_ebitda >= d_and_a * 8"))
        .build()
        .expect("unique ids");
    let report = suite.run(&model, &results).expect("checks run");
    assert_eq!(report.results[0].outcome, CheckOutcome::Fail);
    assert_eq!(report.results[1].outcome, CheckOutcome::Pass);
}

#[test]
fn a_rolled_result_is_normalized_after_the_rollup() {
    let (model, mut quarterly) = usd_model();
    let spec = RollupSpec {
        frequency: PeriodKind::SemiAnnual,
        methods: IndexMap::new(),
    };
    let mut half = quarterly.rollup(&model, &spec).expect("rollup");
    let h1: PeriodId = "2025H1".parse().expect("half-year id");
    let bridge = NormalizationConfig::new("ebitda", "adjusted_ebitda")
        .add_adjustment(Adjustment::fixed(
            "a",
            "a",
            IndexMap::from([(h1, usd("10"))]),
        ))
        .expect("unique id");
    merge_into_results(&mut half, &bridge).expect("publish on the rolled result");
    assert_eq!(
        half.get_money("adjusted_ebitda", &h1).map(Into::into),
        Some(usd("60"))
    );

    // A published node is not a model node: it cannot be rolled up.
    merge_into_results(&mut quarterly, &capped_example(CapBaseMode::Reported)).expect("publish");
    let error = quarterly
        .rollup(&model, &spec)
        .expect_err("published node has no method")
        .to_string();
    assert!(error.contains("adjusted_ebitda"), "{error}");
}
