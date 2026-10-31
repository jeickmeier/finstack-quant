"""Cross-host golden for term-loan outputs bound through both hosts.

The WASM twin is ``finstack-quant-wasm/tests/facade/term_loan_cross_host.test.mjs``:
both hosts price the shared example term loan on the shared market and compare
to the same pinned values.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from finstack_quant.valuations.instruments import instrument_cashflows, price_instrument

ROOT = Path(__file__).parents[2]
INSTRUMENT = (ROOT / "finstack-quant/valuations/tests/instruments/json_examples/term_loan.json").read_text()
PINNED = json.loads((Path(__file__).parent / "fixtures" / "term_loan_cross_host.json").read_text())


def test_oid_eir_rate_matches_the_value_pinned_for_both_hosts() -> None:
    result = price_instrument(
        INSTRUMENT,
        json.dumps(PINNED["market"]),
        PINNED["as_of"],
        PINNED["model"],
        ["oid_eir_rate"],
    )
    assert result.get_metric("oid_eir_rate") == pytest.approx(PINNED["oid_eir_rate"], rel=1e-12)


def test_cashflow_balances_match_the_values_pinned_for_both_hosts() -> None:
    envelope = instrument_cashflows(
        INSTRUMENT,
        json.dumps(PINNED["market"]),
        PINNED["as_of"],
        PINNED["model"],
    )
    actual = [[row["beginning_balance"], row["ending_balance"]] for row in envelope.flows]
    assert len(actual) == len(PINNED["balances"])
    for row, pinned in zip(actual, PINNED["balances"], strict=True):
        assert row == pytest.approx(pinned, rel=1e-12, abs=1e-6)
