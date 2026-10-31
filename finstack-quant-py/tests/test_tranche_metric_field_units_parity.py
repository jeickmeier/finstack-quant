"""Cross-host golden for ``valuations.instruments.tranche_metric_field_units``.

The WASM twin is
``finstack-quant-wasm/tests/facade/tranche_metric_field_units_parity.test.mjs``:
both hosts compare to the same pinned records.

Regenerate with ``UPDATE_TRANCHE_UNITS_PARITY=1 uv run --no-sync pytest <this file>``.
"""

from __future__ import annotations

import json
import os
from pathlib import Path

import pytest

from finstack_quant.valuations.instruments import (
    pool_report_field_units,
    tranche_metric_field_units,
    tranche_solve_field_units,
)

FIXTURE = Path(__file__).parent / "fixtures" / "tranche_metric_field_units.json"
SOLVE_FIXTURE = Path(__file__).parent / "fixtures" / "tranche_solve_field_units.json"
POOL_REPORT_FIXTURE = Path(__file__).parent / "fixtures" / "pool_report_field_units.json"


def test_units_match_the_records_pinned_for_both_hosts() -> None:
    actual = tranche_metric_field_units("EUR")
    if os.environ.get("UPDATE_TRANCHE_UNITS_PARITY"):
        FIXTURE.write_text(json.dumps(actual, indent=2) + "\n")
    assert actual == json.loads(FIXTURE.read_text())


def test_rejects_an_unknown_currency() -> None:
    with pytest.raises(ValueError, match=r"(?i)currency"):
        tranche_metric_field_units("NOT-A-CURRENCY")


def test_solve_units_match_the_records_pinned_for_both_hosts() -> None:
    actual = tranche_solve_field_units()
    if os.environ.get("UPDATE_TRANCHE_UNITS_PARITY"):
        SOLVE_FIXTURE.write_text(json.dumps(actual, indent=2) + "\n")
    assert actual == json.loads(SOLVE_FIXTURE.read_text())


def test_pool_report_units_match_the_records_pinned_for_both_hosts() -> None:
    actual = pool_report_field_units("EUR")
    if os.environ.get("UPDATE_TRANCHE_UNITS_PARITY"):
        POOL_REPORT_FIXTURE.write_text(json.dumps(actual, indent=2) + "\n")
    assert actual == json.loads(POOL_REPORT_FIXTURE.read_text())
