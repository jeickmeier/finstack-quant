"""Cross-host golden for ``InstrumentCashflowEnvelope.calendar_year_ladder_by_currency``.

The WASM twin is ``finstack-quant-wasm/tests/facade/currency_ladder_parity.test.mjs``:
both hosts read the same pinned envelope and compare to the same ladders.

Regenerate with ``UPDATE_CURRENCY_LADDER_PARITY=1 uv run --no-sync pytest <this file>``.
"""

from __future__ import annotations

import json
import os
from pathlib import Path

from finstack_quant.valuations.instruments import InstrumentCashflowEnvelope

FIXTURE = Path(__file__).parent / "fixtures" / "currency_ladder.json"


def _flow(date: str, currency: str, kind: str, amount: float, native_pv: float) -> dict[str, object]:
    return {
        "date": date,
        "amount": amount,
        "currency": currency,
        "kind": kind,
        "accrual_factor": 0.0,
        "year_fraction": 1.0,
        "rate": None,
        "discount_factor": 1.0,
        "discount_curve_id": "USD-OIS",
        "native_pv": native_pv,
        "pv": native_pv * 2.0,
    }


ENVELOPE = {
    "instrument_id": "XCCY",
    "currency": "USD",
    "model": "discounting",
    "as_of": "2025-01-15",
    "discount_curve_id": "USD-OIS",
    "flows": [
        _flow("2026-01-15", "EUR", "fixed", 10.0, 9.0),
        _flow("2026-06-15", "USD", "fixed", 20.0, 18.0),
        _flow("2027-01-15", "EUR", "notional", 100.0, 80.0),
    ],
    "total_pv": 214.0,
    "reconciles_with_base_value": True,
}


def test_ladders_match_the_records_pinned_for_both_hosts() -> None:
    actual = InstrumentCashflowEnvelope.from_json(json.dumps(ENVELOPE)).calendar_year_ladder_by_currency()
    if os.environ.get("UPDATE_CURRENCY_LADDER_PARITY"):
        FIXTURE.write_text(json.dumps({"envelope": ENVELOPE, "ladders": actual}, indent=2) + "\n")
    assert actual == json.loads(FIXTURE.read_text())["ladders"]
    assert [ladder["currency"] for ladder in actual] == ["EUR", "USD"]
