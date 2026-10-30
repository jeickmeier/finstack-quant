"""Cross-host goldens for ``models.credit.merton_report``.

The WASM twin is ``finstack-quant-wasm/tests/facade/merton_report_parity.test.mjs``:
both hosts read the same inputs and compare to the same pinned reports, so the KMV
default point and the default master scale cannot differ between them.

Regenerate with ``UPDATE_MERTON_PARITY=1 uv run --no-sync pytest <this file>``.
"""

from __future__ import annotations

import json
import os
from pathlib import Path

import pytest

from finstack_quant.models.credit import MertonModel, MertonReport, merton_report
from finstack_quant.models.credit.pd import MasterScale
from tests.test_statements_wasm_parity import assert_matches

FIXTURES = Path(__file__).parent / "fixtures"
INPUTS = json.loads((FIXTURES / "merton_report_parity_inputs.json").read_text())
EXPECTED_PATH = FIXTURES / "merton_report_parity_expected.json"


def _request(name: str) -> dict[str, object]:
    return {"source": INPUTS["sources"][name], **INPUTS["terms"]}


def test_pins_reports_every_host_must_match() -> None:
    actual = {name: json.loads(merton_report(_request(name)).to_json()) for name in INPUTS["sources"]}
    if os.environ.get("UPDATE_MERTON_PARITY"):
        EXPECTED_PATH.write_text(json.dumps(actual, indent=2, sort_keys=True) + "\n")
    assert_matches(actual, json.loads(EXPECTED_PATH.read_text()), "merton_report")


def test_equity_source_defaults_to_the_kmv_default_point() -> None:
    report = merton_report(_request("equity"))
    source = INPUTS["sources"]["equity"]
    point = MertonModel.kmv_default_point(source["short_term_debt"], source["long_term_debt"])
    assert report.default_point == point
    assert report.model.debt_barrier == point


def test_grades_come_from_the_default_master_scale() -> None:
    report = merton_report(_request("model"))
    scale = MasterScale.default_calibration()
    assert [row.grade for row in report.grades] == [scale.map_pd(pd).grade for pd in report.default_probabilities]


def test_rejected_measure_keeps_its_message_beside_the_rest() -> None:
    report = merton_report(_request("first_passage"))
    assert report.debt_spread is None
    assert "Terminal only" in report.errors["debt_spread"]
    assert report.implied_spread is not None
    frame = report.to_dataframe()
    assert list(frame["horizon"]) == report.horizons
    assert frame["debt_spread"].isna().all()


def test_rejects_a_non_positive_horizon_and_an_unknown_field() -> None:
    with pytest.raises(ValueError, match="horizons"):
        merton_report({**_request("model"), "horizons": [0.0]})
    with pytest.raises(ValueError, match="unknown field"):
        merton_report({**_request("model"), "extra": 1})


def test_wire_round_trip() -> None:
    report = merton_report(_request("equity"))
    assert MertonReport.from_json(report.to_json()).to_json() == report.to_json()
