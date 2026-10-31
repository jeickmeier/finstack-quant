"""Cross-host check that a full-grid sensitivity result has no tornado.

The WASM twin is ``finstack-quant-wasm/tests/facade/tornado_full_grid_parity.test.mjs``.
Both read the same pinned full-grid result: Python gets ``None``, JavaScript ``null``.

Regenerate with ``UPDATE_TORNADO_FULL_GRID=1 uv run --no-sync pytest <this file>``.
"""

from __future__ import annotations

import json
import os
from pathlib import Path

import pytest

from finstack_quant.statements import ModelBuilder
from finstack_quant.statements_analytics import (
    ParameterSpec,
    SensitivityConfig,
    generate_tornado_entries,
    run_sensitivity,
)

FIXTURE = Path(__file__).parent / "fixtures" / "tornado_full_grid.json"


def _result(mode: str) -> object:
    builder = ModelBuilder("m")
    builder.periods("2025Q1..Q2", "2025Q1")
    builder.value("revenue", {"2025Q1": 100.0, "2025Q2": 110.0})
    builder.compute("profit", "revenue * 0.5")
    config = SensitivityConfig(
        mode,
        [ParameterSpec.with_percentages("revenue", "2025Q2", 110.0, [-10.0, 10.0])],
        ["profit"],
    )
    return run_sensitivity(builder.build(), config)


def test_full_grid_pinned_for_both_hosts_has_no_tornado() -> None:
    result = _result("full_grid")
    if os.environ.get("UPDATE_TORNADO_FULL_GRID"):
        FIXTURE.write_text(json.dumps(json.loads(result.to_json()), indent=2) + "\n")
    assert generate_tornado_entries(FIXTURE.read_text(), "profit", "2025Q2") is None


def test_one_at_a_time_run_still_ranks() -> None:
    entries = generate_tornado_entries(_result("diagonal"), "profit", "2025Q2")
    assert entries is not None
    assert [entry.parameter_id for entry in entries] == ["revenue@2025Q2"]


def test_full_grid_still_rejects_a_bad_period() -> None:
    with pytest.raises(ValueError, match=r"(?i)period"):
        generate_tornado_entries(_result("full_grid"), "profit", "not-a-period")
