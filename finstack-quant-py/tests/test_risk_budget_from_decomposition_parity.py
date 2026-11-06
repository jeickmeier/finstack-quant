"""Cross-host golden for ``evaluate_risk_budget_from_decomposition``.

The WASM twin is
``finstack-quant-wasm/tests/facade/risk_budget_from_decomposition_parity.test.mjs``:
both hosts read the same pinned decomposition and compare to the same result.

Regenerate with ``UPDATE_RISK_BUDGET_PARITY=1 uv run --no-sync pytest <this file>``.
"""

from __future__ import annotations

import json
import os
from pathlib import Path

import pytest

from finstack_quant.models.factor.risk import (
    PositionRiskDecomposition,
    evaluate_risk_budget,
    evaluate_risk_budget_from_decomposition,
    position_component_var,
    position_risk_report,
)

FIXTURE = Path(__file__).parent / "fixtures" / "risk_budget_from_decomposition.json"
TARGETS = [0.5, 0.5]


def _decomposition() -> PositionRiskDecomposition:
    return position_risk_report({
        "position_ids": ["A", "B"],
        "source": {"kind": "parametric", "weights": [0.6, 0.4], "covariance": [[0.04, 0.0], [0.0, 0.01]]},
    }).decomposition


def test_result_matches_the_record_pinned_for_both_hosts() -> None:
    decomposition = _decomposition()
    actual = json.loads(evaluate_risk_budget_from_decomposition(decomposition, TARGETS, 1.1).to_json())
    if os.environ.get("UPDATE_RISK_BUDGET_PARITY"):
        FIXTURE.write_text(
            json.dumps(
                {"decomposition": json.loads(decomposition.to_json()), "targets": TARGETS, "result": actual},
                indent=2,
            )
            + "\n"
        )
    pinned = json.loads(FIXTURE.read_text())
    assert actual == pinned["result"]
    replayed = PositionRiskDecomposition.from_json(json.dumps(pinned["decomposition"]))
    assert json.loads(evaluate_risk_budget_from_decomposition(replayed, TARGETS, 1.1).to_json()) == pinned["result"]


def test_matches_the_join_a_caller_would_write() -> None:
    decomposition = _decomposition()
    joined = evaluate_risk_budget(
        ["A", "B"],
        [position_component_var(decomposition, "A"), position_component_var(decomposition, "B")],
        TARGETS,
        decomposition.portfolio_var,
    )
    assert evaluate_risk_budget_from_decomposition(decomposition, TARGETS).to_json() == joined.to_json()


def test_rejects_targets_that_do_not_cover_every_position() -> None:
    with pytest.raises(ValueError, match="target_var_pct length"):
        evaluate_risk_budget_from_decomposition(_decomposition(), [1.0])
