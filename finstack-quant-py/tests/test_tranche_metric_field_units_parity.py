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

from finstack_quant.valuations.instruments import tranche_metric_field_units

FIXTURE = Path(__file__).parent / "fixtures" / "tranche_metric_field_units.json"


def test_units_match_the_records_pinned_for_both_hosts() -> None:
    actual = tranche_metric_field_units("EUR")
    if os.environ.get("UPDATE_TRANCHE_UNITS_PARITY"):
        FIXTURE.write_text(json.dumps(actual, indent=2) + "\n")
    assert actual == json.loads(FIXTURE.read_text())


def test_rejects_an_unknown_currency() -> None:
    with pytest.raises(ValueError, match=r"(?i)currency"):
        tranche_metric_field_units("NOT-A-CURRENCY")
