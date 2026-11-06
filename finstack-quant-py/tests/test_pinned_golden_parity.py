"""Cross-host goldens: Rust-owned tables both hosts compare to one pinned fixture.

The WASM twin is ``finstack-quant-wasm/tests/facade/pinned_golden_parity.test.mjs``:
it reads the same fixtures through the same table.

Regenerate with ``UPDATE_PINNED_GOLDENS=1 uv run --no-sync pytest <this file>``.
"""

from __future__ import annotations

from collections.abc import Callable
import json
import os
from pathlib import Path
from typing import Any

import pytest

from finstack_quant.valuations.instruments import (
    bond_quote_set_field_units,
    equity_metrics_field_units,
    metric_metadata,
    pool_report_field_units,
    price_driver_fields,
    scenario_table_field_units,
    simulation_field_units,
    tranche_metric_field_units,
    tranche_solve_field_units,
)

FIXTURES = Path(__file__).parent / "fixtures"

# fixture -> (value computed from the pinned fixture, fixture key holding the
# expected value; ``None`` when the whole fixture is the expected value)
GOLDENS: dict[str, tuple[Callable[[Any], Any], str | None]] = {
    "tranche_metric_field_units.json": (lambda _: tranche_metric_field_units("EUR"), None),
    "tranche_solve_field_units.json": (lambda _: tranche_solve_field_units(), None),
    "bond_quote_set_field_units.json": (lambda _: bond_quote_set_field_units("EUR"), None),
    "pool_report_field_units.json": (lambda _: pool_report_field_units("EUR"), None),
    "scenario_table_field_units.json": (lambda _: scenario_table_field_units("EUR"), None),
    "simulation_field_units.json": (lambda _: simulation_field_units("EUR"), None),
    "equity_metrics_field_units.json": (lambda _: equity_metrics_field_units("EUR"), None),
    "price_driver_fields.json": (lambda _: price_driver_fields(), None),
    "metric_coordinates.json": (
        lambda pinned: [entry["coordinates"] for entry in metric_metadata(pinned["keys"])],
        "coordinates",
    ),
}


@pytest.mark.parametrize("fixture", GOLDENS)
def test_matches_the_value_pinned_for_both_hosts(fixture: str) -> None:
    compute, key = GOLDENS[fixture]
    path = FIXTURES / fixture
    pinned = json.loads(path.read_text())
    actual = compute(pinned)
    if os.environ.get("UPDATE_PINNED_GOLDENS"):
        pinned = actual if key is None else {**pinned, key: actual}
        path.write_text(json.dumps(pinned, indent=2) + "\n")
    assert actual == (pinned if key is None else pinned[key])


def test_tranche_metric_units_reject_an_unknown_currency() -> None:
    with pytest.raises(ValueError, match=r"(?i)currency"):
        tranche_metric_field_units("NOT-A-CURRENCY")
