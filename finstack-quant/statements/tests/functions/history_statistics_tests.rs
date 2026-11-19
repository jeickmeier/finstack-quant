//! Historical statistics, cumulative functions and EWM over canonical period
//! slots (STATEMENTS-EVALUATION-004).

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::{
    CellStatus, EvalWarning, Evaluator, StatementResult, UnavailableReason,
};

fn month(index: u8) -> PeriodId {
    PeriodId::month(2025, index).expect("valid period fixture")
}

fn usd(amount: &str) -> Money {
    Money::from_decimal_str(amount, Currency::USD).expect("amount")
}

/// Six months of a scalar series and a USD series. `x` is 10, 20, gap, 20,
/// 50, 30 and `cash` is 0.10, 0.20, gap, 0.20, 0.50, 0.30: March has no
/// observation in either.
fn evaluate(formulas: &[(&str, &str)]) -> finstack_quant_statements::Result<StatementResult> {
    let mut builder = ModelBuilder::new("history");
    builder.periods("2025M01..2025M06", None)?;
    let observed = [(1, 10.0), (2, 20.0), (4, 20.0), (5, 50.0), (6, 30.0)];
    builder.value_scalar("x", &observed.map(|(index, value)| (month(index), value)));
    builder.value_money(
        "cash",
        &observed.map(|(index, value)| (month(index), usd(&format!("{:.2}", value / 100.0)))),
    );
    for (node, formula) in formulas {
        builder.compute(*node, *formula)?;
    }
    Evaluator::new().evaluate(&builder.build()?)
}

fn one(formula: &str) -> StatementResult {
    evaluate(&[("out", formula)]).unwrap_or_else(|error| panic!("{formula}: {error}"))
}

fn scalar(formula: &str, index: u8) -> Option<f64> {
    let result = one(formula);
    let cell = result.get_cell("out", &month(index)).expect("cell");
    match cell.status {
        CellStatus::Value => cell.numeric(),
        CellStatus::Unavailable => {
            assert_eq!(
                cell.reason,
                Some(UnavailableReason::MissingInput),
                "{formula}"
            );
            None
        }
        other => panic!("{formula}: unexpected status {other:?}"),
    }
}

fn close(actual: Option<f64>, expected: f64) {
    let actual = actual.expect("value");
    assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
}

#[test]
fn rolling_windows_cover_exact_slots_and_need_their_minimum_count() {
    // The window is the last three calendar slots, not the last three
    // observations: in April it is February, the March gap and April.
    assert_eq!(scalar("rolling_mean(x, 3)", 4), None);
    close(scalar("rolling_mean(x, 3, 2)", 4), 20.0);
    close(scalar("rolling_sum(x, 3, 2)", 4), 40.0);
    close(scalar("rolling_count(x, 3, 1)", 4), 2.0);
    // Exactly at the minimum count, and one below it.
    close(scalar("rolling_max(x, 3, 2)", 3), 20.0);
    assert_eq!(scalar("rolling_max(x, 4, 3)", 3), None);
    // A full window.
    close(scalar("rolling_mean(x, 3)", 6), 100.0 / 3.0);
    close(scalar("rolling_min(x, 3)", 6), 20.0);
    close(scalar("rolling_median(x, 3)", 6), 30.0);
    // Before the model start there are no slots to count.
    assert_eq!(scalar("rolling_mean(x, 3)", 2), None);

    for (formula, cause) in [
        ("rolling_mean(x, 0)", "greater than 0"),
        ("rolling_mean(x, 3, 0)", "from 1 through the window"),
        ("rolling_mean(x, 3, 4)", "from 1 through the window"),
        ("rolling_mean(x, 2.5)", "integer"),
    ] {
        let error = evaluate(&[("out", formula)]).expect_err(formula);
        assert!(error.to_string().contains(cause), "{formula}: {error}");
    }
}

#[test]
fn sample_statistics_use_n_minus_one_and_need_two_observations() {
    // April to June: 20, 50, 30 has sample variance 700 / 3.
    close(scalar("rolling_var(x, 3)", 6), 700.0 / 3.0);
    close(scalar("rolling_std(x, 3)", 6), (700.0_f64 / 3.0).sqrt());
    // One observation in the window is not a sample.
    assert_eq!(scalar("rolling_std(x, 2, 1)", 4), None);
    assert_eq!(scalar("std(x)", 1), None);
    // Full history through February: 10, 20.
    close(scalar("var(x)", 2), 50.0);
    close(scalar("median(x)", 2), 15.0);
}

#[test]
fn full_history_statistics_report_the_sample_count_when_there_are_gaps() {
    let result = one("std(x)");
    // Through June: 10, 20, 20, 50, 30 over six slots.
    let expected = (920.0_f64 / 4.0).sqrt();
    assert!((result.get("out", &month(6)).unwrap() - expected).abs() < 1e-12);
    let gapped = |index: u8| {
        result
            .meta
            .warnings
            .iter()
            .find_map(|warning| match warning {
                EvalWarning::GappedSample {
                    node_id,
                    period,
                    observations,
                    slots,
                    ..
                } if node_id == "out" && *period == month(index) => Some((*observations, *slots)),
                _ => None,
            })
    };
    assert_eq!(gapped(2), None, "no gap through February");
    assert_eq!(gapped(6), Some((5, 6)));
}

#[test]
fn money_statistics_are_exact_and_keep_their_currency() {
    let result = evaluate(&[
        ("total", "rolling_sum(cash, 3)"),
        ("average", "rolling_mean(cash, 2)"),
        ("spread", "rolling_std(cash, 3)"),
        ("running", "cumsum(cash)"),
        ("middle", "quantile(cash, 0.5)"),
    ])
    .unwrap();
    assert_eq!(result.get_money("total", &month(6)), Some(usd("1.00")));
    assert_eq!(result.get_money("average", &month(6)), Some(usd("0.40")));
    assert_eq!(result.get_money("running", &month(2)), Some(usd("0.30")));
    let spread = result.get_money("spread", &month(6)).expect("money");
    assert_eq!(spread.currency(), Currency::USD);
    assert!((spread.amount() - (0.07_f64 / 3.0).sqrt()).abs() < 1e-12);
    assert_eq!(result.get_money("middle", &month(6)), Some(usd("0.20")));
    // Variance has squared units and is rejected for money when the model
    // is built.
    assert!(evaluate(&[("bad", "rolling_var(cash, 3)")]).is_err());
}

#[test]
fn cumulative_functions_need_a_complete_prefix() {
    close(scalar("cumsum(x)", 2), 30.0);
    close(scalar("cumprod(x)", 2), 200.0);
    close(scalar("cummin(x)", 2), 10.0);
    close(scalar("cummax(x)", 2), 20.0);
    // The March gap makes every later prefix incomplete.
    for function in ["cumsum", "cumprod", "cummin", "cummax"] {
        for index in [3, 4, 6] {
            assert_eq!(
                scalar(&format!("{function}(x)"), index),
                None,
                "{function} M{index}"
            );
        }
    }
    // A recovered gap completes the prefix again.
    close(scalar("cumsum(coalesce(x, 0))", 6), 130.0);
    let error = evaluate(&[("out", "cumprod(x * 1e200)")]).expect_err("overflow");
    assert!(error.to_string().contains("overflows"), "{error}");
}

#[test]
fn rank_uses_exact_competition_ties() {
    // Through June the observations are 10, 20, 20, 50, 30.
    close(scalar("rank(x)", 2), 2.0);
    // April ties February at 20: both share rank 2 ascending.
    close(scalar("rank(x)", 4), 2.0);
    close(scalar("rank(x)", 5), 4.0);
    close(scalar("rank(x)", 6), 4.0);
    // Descending: 50 leads, 30 follows, the tied 20s share rank 3.
    close(scalar("rank(x, 0)", 6), 2.0);
    close(scalar("rank(x, 0)", 4), 1.0);
    // The current value is unavailable in March.
    assert_eq!(scalar("rank(x)", 3), None);
    // R-7 interpolation over the available observations.
    close(scalar("quantile(x, 0.5)", 6), 20.0);
    close(scalar("quantile(x, 0.25)", 2), 12.5);
    close(scalar("quantile(x, 1)", 6), 50.0);
    for formula in ["quantile(x, 1.5)", "quantile(x, 0 - 0.1)"] {
        assert!(evaluate(&[("out", formula)]).is_err(), "{formula}");
    }
}

#[test]
fn ewm_decays_once_per_slot_and_carries_the_estimate_across_a_gap() {
    // alpha 0.5: January 10; February (10 + 20) / 2.
    close(scalar("ewm_mean(x, 0.5)", 1), 10.0);
    close(scalar("ewm_mean(x, 0.5)", 2), 15.0);
    // March has no observation: the estimate carries.
    close(scalar("ewm_mean(x, 0.5)", 3), 15.0);
    // April: the old weight decayed twice, so the new observation weighs
    // 0.5 / (0.25 + 0.5) = 2/3.
    close(scalar("ewm_mean(x, 0.5)", 4), 15.0 + (2.0 / 3.0) * 5.0);
    // Money keeps its currency.
    let result = one("ewm_mean(cash, 0.5)");
    let estimate = result.get_money("out", &month(2)).expect("money");
    assert_eq!(estimate.currency(), Currency::USD);
    assert!((estimate.amount() - 0.15).abs() < 1e-12);

    for formula in ["ewm_mean(x, 0)", "ewm_mean(x, 1.5)", "ewm_std(x, 0.5, 2)"] {
        assert!(evaluate(&[("out", formula)]).is_err(), "{formula}");
    }
}

#[test]
fn corrected_ewm_dispersion_needs_two_observations() {
    // One observation: the corrected statistic is unavailable, the
    // uncorrected one is zero.
    for function in ["ewm_std", "ewm_var"] {
        assert_eq!(scalar(&format!("{function}(x, 0.5)"), 1), None);
        assert_eq!(scalar(&format!("{function}(x, 0.5, 1)"), 1), None);
        assert_eq!(scalar(&format!("{function}(x, 0.5, x > 0)"), 1), None);
        close(scalar(&format!("{function}(x, 0.5, 0)"), 1), 0.0);
        close(scalar(&format!("{function}(x, 0.5, x < 0)"), 1), 0.0);
    }
    // Two observations 10 and 20 at alpha 0.5: biased variance 25, squared
    // weights 0.5, corrected variance 50.
    close(scalar("ewm_var(x, 0.5, 0)", 2), 25.0);
    close(scalar("ewm_var(x, 0.5)", 2), 50.0);
    close(scalar("ewm_std(x, 0.5)", 2), 50.0_f64.sqrt());
}
