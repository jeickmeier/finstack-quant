"""Cross-host golden for ``ApplicationReport.effective_as_of``.

The WASM twin is
``finstack-quant-wasm/tests/facade/application_report_effective_as_of_parity.test.mjs``:
both hosts read the same pinned reports and compare to the same dates.
"""

from __future__ import annotations

import datetime
import json
from pathlib import Path

import pytest

from finstack_quant.scenarios import ApplicationReport

CASES = json.loads((Path(__file__).parent / "fixtures" / "application_report_effective_as_of.json").read_text())


@pytest.mark.parametrize("case", CASES, ids=[case["name"] for case in CASES])
def test_matches_the_date_pinned_for_both_hosts(case: dict[str, object]) -> None:
    report = ApplicationReport.from_json(json.dumps(case["report"]))
    assert report.effective_as_of(case["as_of"]) == datetime.date.fromisoformat(str(case["effective"]))


def test_rejects_a_string_that_is_not_a_date() -> None:
    report = ApplicationReport.from_json(json.dumps(CASES[0]["report"]))
    with pytest.raises(ValueError, match=r"(?i)date"):
        report.effective_as_of("not-a-date")
