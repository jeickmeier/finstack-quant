"""The LBO evaluator: the accepted worked example, its refusals, and its JSON round trips.

The model and request are the shared cross-host inputs ``lbo_model`` and
``lbo_request`` (``tests/fixtures/statements_parity_inputs.json``): the
worked example of STATEMENTS-ANALYTICS-VALUATION-03. Entry at 8x on 100 of
EBITDA is 800; fees of 20 make uses 820; debt proceeds of 500 leave 320 of
sponsor cash. Exit at 9x on 120 is 1,080 against 400 of net debt.
"""

from __future__ import annotations

from collections.abc import Callable
import copy
import datetime as dt
import json
from pathlib import Path
import pickle
from typing import Any

import pytest

from finstack_quant.statements import FinancialModelSpec
from finstack_quant.statements_analytics import (
    LboCheckMappings,
    LboRequest,
    LboResult,
    evaluate_lbo,
)

INPUTS: dict[str, Any] = json.loads((Path(__file__).parent / "fixtures" / "statements_parity_inputs.json").read_text())


def _usd(amount: int) -> dict[str, str]:
    return {"amount": str(amount), "currency": "USD"}


def _model(edit: Callable[[dict[str, Any]], Any] | None = None) -> FinancialModelSpec:
    document = copy.deepcopy(INPUTS["lbo_model"])
    if edit is not None:
        edit(document)
    return FinancialModelSpec.from_json(json.dumps(document))


def _request(edit: Callable[[dict[str, Any]], Any] | None = None) -> LboRequest:
    document = copy.deepcopy(INPUTS["lbo_request"])
    if edit is not None:
        edit(document)
    return LboRequest.from_json(json.dumps(document))


def _commit(amount: int) -> Callable[[dict[str, Any]], Any]:
    return lambda request: request["funding"].update(sponsor_commitment=_usd(amount))


def _suite(formula: str) -> dict[str, Any]:
    """The shared one-check suite, with its predicate replaced."""
    suite = copy.deepcopy(INPUTS["lbo_check_suite"])
    suite["formula_checks"][0]["formula"] = formula
    return suite


def test_sizing_reports_the_required_sponsor_cash_and_no_funded_return() -> None:
    result = evaluate_lbo(_model(), _request())

    assert (result.entry_date, result.exit_valuation_date) == (dt.date(2025, 12, 31), dt.date(2028, 12, 31))
    assert (result.entry_earnings.amount, result.entry_enterprise_value.amount) == (100.0, 800.0)
    assert result.funding.cash_uses.amount == 820.0
    assert (result.funding.debt_cash_proceeds.amount, result.funding.opening_debt_face.amount) == (500.0, 520.0)
    assert result.funding.required_sponsor_cash.amount == 320.0
    assert result.funding.committed_cash_sources is None
    assert result.funding.funding_gap is None
    assert (result.exit_earnings.amount, result.exit.enterprise_value.amount) == (120.0, 1080.0)
    assert (result.exit.net_debt.amount, result.exit.other_claims.amount) == (400.0, 0.0)
    assert result.exit.common_equity_residual.amount == 680.0
    assert result.exit.sponsor_proceeds.amount == 680.0
    assert result.exit.sized_moic == 2.125
    assert result.exit.funded_moic is None
    assert result.checks is None


def test_a_short_commitment_shows_the_gap_and_withholds_the_funded_return() -> None:
    result = evaluate_lbo(_model(), _request(_commit(300)))

    assert result.funding.committed_cash_sources.amount == 800.0
    assert result.funding.funding_gap.amount == 20.0
    assert result.exit.sized_moic == 2.125
    assert result.exit.funded_moic is None


def test_a_full_commitment_closes_the_gap_and_reports_the_funded_return() -> None:
    result = evaluate_lbo(_model(), _request(_commit(320)))

    assert result.funding.funding_gap.amount == 0.0
    assert result.exit.funded_moic == 2.125


def test_a_negative_exit_residual_is_kept_signed_and_proceeds_are_floored_at_zero() -> None:
    def distressed(request: dict[str, Any]) -> None:
        _commit(320)(request)
        request["exit"]["multiple"] = "3"

    result = evaluate_lbo(_model(), _request(distressed))

    assert result.exit.enterprise_value.amount == 360.0
    assert result.exit.common_equity_residual.amount == -40.0
    assert result.exit.sponsor_proceeds.amount == 0.0
    assert (result.exit.sized_moic, result.exit.funded_moic) == (0.0, 0.0)


def test_the_run_records_the_model_snapshot_and_observation_cut() -> None:
    run = evaluate_lbo(_model(), _request()).run

    assert run["model_id"] == "dated-buyout"
    assert run["source_snapshot"] == "training-borrower-2025-close"
    assert run["observation_cutoff"] == {"through": "2025-12-31"}
    assert run["valuation_date"] == "2025-12-31"
    assert run["model_content_hash"].startswith("sha256:")


def test_request_and_result_round_trip_through_json_and_pickle() -> None:
    request = _request(_commit(320))
    result = evaluate_lbo(_model(), request)
    wire = json.loads(result.to_json())

    assert json.loads(LboRequest.from_json(request.to_json()).to_json()) == json.loads(request.to_json())
    assert json.loads(LboResult.from_json(result.to_json()).to_json()) == wire
    assert json.loads(pickle.loads(pickle.dumps(result)).to_json()) == wire  # noqa: S301 - own serialized result
    assert list(wire) == [
        "run",
        "entry_date",
        "exit_valuation_date",
        "entry_earnings",
        "entry_enterprise_value",
        "funding",
        "exit_earnings",
        "exit",
        "checks",
    ]
    assert wire["exit"]["funded_moic"] == result.exit.funded_moic
    with pytest.raises(ValueError, match="unknown field"):
        LboResult.from_json(json.dumps({**wire, "zzz": 1}))


def test_a_0_11_config_or_result_document_is_refused() -> None:
    # `evaluate_lbo` and `LboResult` keep the names the 0.11 evaluator used, with a new contract.
    # An old document must fail, never be valued.
    old_config = {
        "entry_multiple": 8.0,
        "entry_metric_node": "ebitda",
        "entry_period": "2025",
        "transaction_fees": 0.0,
        "sources": [{"name": "debt", "amount": 500.0}],
        "exit_multiple": 9.0,
        "exit_metric_node": "ebitda",
        "exit_net_debt_node": "net_debt",
        "exit_period": "2028",
    }
    with pytest.raises(TypeError, match="LboRequest"):
        evaluate_lbo(_model(), old_config)
    with pytest.raises(ValueError, match="unknown field `entry_multiple`"):
        LboRequest(**old_config)
    with pytest.raises(ValueError, match="unknown field `entry_multiple`"):
        LboRequest.from_json(json.dumps(old_config))
    old_result = {"entry_enterprise_value": _usd(800), "entry_metric": 100.0, "moic": 2.0, "checks": None}
    with pytest.raises(ValueError, match="unknown field `entry_metric`"):
        LboResult.from_json(json.dumps(old_result))


def _without_availability_dates(model: dict[str, Any]) -> None:
    for node in model["nodes"].values():
        node.pop("availability_dates", None)


def _availability_the_day_after(model: dict[str, Any]) -> None:
    for node in model["nodes"].values():
        for period in node.get("availability_dates", {}):
            node["availability_dates"][period] = "2026-01-01"


# What is wrong, the model edit, the request edit, and the message the evaluator gives.
REFUSALS: list[tuple[str, Callable[[dict[str, Any]], Any] | None, Callable[[dict[str, Any]], Any] | None, str]] = [
    (
        "funding-date-is-not-the-entry-date",
        None,
        lambda request: request["funding"].update(date="2025-12-30"),
        "matching entry and funding dates before the exit date",
    ),
    (
        "exit-before-entry",
        None,
        lambda request: request.update(exit_valuation_date="2025-06-30"),
        "matching entry and funding dates before the exit date",
    ),
    (
        "exit-date-is-not-the-period-end",
        None,
        lambda request: request.update(exit_valuation_date="2028-12-30"),
        "must close on the matching actual entry or projected exit date",
    ),
    (
        "no-source-snapshot",
        lambda model: model.pop("source_snapshot"),
        None,
        "needs an identified source snapshot",
    ),
    (
        "no-availability-dates",
        _without_availability_dates,
        None,
        "has no availability date for its 2025 observation",
    ),
    (
        "observation-published-after-the-entry-date",
        _availability_the_day_after,
        None,
        "must be monetary in USD",
    ),
    (
        "zero-multiple",
        None,
        lambda request: request["entry"].update(multiple="0"),
        "multiples must be positive exact decimals",
    ),
    (
        "commitment-in-another-currency",
        None,
        lambda request: request["funding"].update(sponsor_commitment={"amount": "320", "currency": "EUR"}),
        "sponsor cash commitment must be nonnegative in USD",
    ),
    (
        "unknown-exit-debt-node",
        None,
        lambda request: request.update(exit_debt_node="nope"),
        "Node 'nope' at 2028 must be monetary in USD",
    ),
]


@pytest.mark.parametrize(
    ("edit_model", "edit_request", "message"),
    [refusal[1:] for refusal in REFUSALS],
    ids=[refusal[0] for refusal in REFUSALS],
)
def test_the_evaluator_refuses_a_request_or_model_outside_the_dated_contract(
    edit_model: Callable[[dict[str, Any]], Any] | None,
    edit_request: Callable[[dict[str, Any]], Any] | None,
    message: str,
) -> None:
    with pytest.raises(ValueError, match=message):
        evaluate_lbo(_model(edit_model), _request(edit_request))


def test_a_required_check_suite_blocks_the_result_and_an_optional_one_is_reported() -> None:
    def checked(formula: str, required: bool) -> Callable[[dict[str, Any]], Any]:
        def edit(request: dict[str, Any]) -> None:
            _commit(320)(request)
            request["checks"] = {"suite": {"kind": "spec", "spec": _suite(formula)}, "required": required}

        return edit

    with pytest.raises(ValueError, match="Required LBO check suite returned Fail"):
        evaluate_lbo(_model(), _request(checked("term_amortization > 1000", required=True)))

    reported = evaluate_lbo(_model(), _request(checked("term_amortization > 1000", required=False)))
    assert reported.exit.funded_moic == 2.125
    assert json.loads(reported.checks.to_json())["outcome"] == "fail"

    passed = evaluate_lbo(_model(), _request(checked("term_amortization >= 0", required=True)))
    assert json.loads(passed.checks.to_json())["outcome"] == "pass"


def test_the_lbo_arm_runs_the_lbo_model_check_suite_from_node_mappings() -> None:
    model = FinancialModelSpec.from_json(json.dumps(INPUTS["lbo_checked_model"]))
    request = LboRequest.from_json(json.dumps(INPUTS["lbo_checked_request"]))

    result = evaluate_lbo(model, request)

    report = json.loads(result.checks.to_json())
    assert report["outcome"] == "pass"
    assert report["summary"]["total_checks"] == 9
    assert report["summary"]["warnings"] == 5
    assert result.exit.sized_moic == pytest.approx(880 / 310)


def test_a_typed_check_mapping_may_sit_in_place_of_its_dict_and_json_text_is_refused() -> None:
    model = FinancialModelSpec.from_json(json.dumps(INPUTS["lbo_checked_model"]))
    document = copy.deepcopy(INPUTS["lbo_checked_request"])
    from_dict = evaluate_lbo(model, LboRequest(**document))

    document["checks"]["suite"]["mappings"] = LboCheckMappings.from_json(
        json.dumps(document["checks"]["suite"]["mappings"])
    )
    typed = evaluate_lbo(model, LboRequest(**document))
    assert json.loads(typed.checks.to_json()) == json.loads(from_dict.checks.to_json())
    assert typed.exit.sized_moic == from_dict.exit.sized_moic

    # JSON text is refused; its `json.loads` form is the dict.
    checks_text = json.dumps(INPUTS["lbo_checked_request"]["checks"])
    with pytest.raises(ValueError, match="invalid LboRequest: invalid type: string"):
        LboRequest(**{**document, "checks": checks_text})


@pytest.mark.parametrize(
    ("suite", "message"),
    [
        ({"name": "lbo", "builtin_checks": [], "formula_checks": []}, "missing field `kind`"),
        ({"kind": "zzz"}, "unknown variant `zzz`"),
        ({"kind": "spec"}, "missing field `spec`"),
        (
            {**INPUTS["lbo_checked_request"]["checks"]["suite"], "zzz": 1},
            "unknown field `zzz`",
        ),
    ],
    ids=["untagged", "unknown-kind", "spec-without-spec", "unknown-field"],
)
def test_the_check_suite_choice_must_be_tagged(suite: dict[str, Any], message: str) -> None:
    document = copy.deepcopy(INPUTS["lbo_request"])
    document["checks"] = {"suite": suite, "required": False}
    with pytest.raises(ValueError, match=message):
        LboRequest.from_json(json.dumps(document))
