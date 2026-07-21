"""Structured-credit tranche analytics are exported from the instruments module.

These take a tranche id, so they are not reachable through
``price_instrument_with_metrics``. The surface mirrors the five WASM
``structuredCreditTranche*`` entry points.
"""

from __future__ import annotations

from datetime import date
import json
import math
from pathlib import Path

import pytest

from finstack_quant.core.market_data import (
    DiscountCurve,
    ForwardCurve,
    MarketContext,
    ScalarTimeSeries,
)
from finstack_quant.valuations import instruments

SC_ENTRY_POINTS = (
    "structured_credit_tranche_breakeven_cdr",
    "structured_credit_tranche_discount_margin",
    "structured_credit_tranche_metrics",
    "structured_credit_tranche_oas",
    "structured_credit_tranche_scenario_table",
)

FIXTURE = (
    Path(__file__).resolve().parents[2]
    / "finstack-quant"
    / "valuations"
    / "tests"
    / "instruments"
    / "json_examples"
    / "structured_credit_full.json"
)


def _deal_json() -> str:
    return json.dumps(json.loads(FIXTURE.read_text())["instrument"])


def _valid_deal_json() -> str:
    instrument = json.loads(FIXTURE.read_text())["instrument"]
    instrument["spec"]["payment_calendar_id"] = "nyse"
    return json.dumps(instrument)


def _market() -> MarketContext:
    as_of = date(2024, 1, 1)
    market = (
        MarketContext()
        .insert(DiscountCurve.flat("USD-SOFR-DISC", as_of, 0.04))
        .insert(
            ForwardCurve(
                "SOFR-3M",
                0.25,
                [(0.0, 0.04), (10.0, 0.04)],
                as_of,
                day_count="act_360",
            )
        )
    )
    market.insert_series(ScalarTimeSeries("FIXING:SOFR-3M", [(date(2023, 12, 28), 0.04)]))
    return market


@pytest.mark.parametrize("name", SC_ENTRY_POINTS)
def test_entry_point_is_exported(name: str) -> None:
    """Each entry point is present and callable."""
    assert hasattr(instruments, name), f"{name} is not exported from the instruments module"
    assert callable(getattr(instruments, name))


@pytest.mark.parametrize("name", SC_ENTRY_POINTS)
def test_entry_point_reports_the_right_module(name: str) -> None:
    """``__module__`` must place these in the instruments namespace."""
    assert getattr(instruments, name).__module__ == "finstack_quant.valuations.instruments", (
        f"{name} reports the wrong __module__"
    )


def test_wasm_parity_surface_is_matched() -> None:
    """Python exposes exactly the five WASM ``structuredCreditTranche*`` names."""
    exported = {n for n in instruments.__all__ if n.startswith("structured_credit_tranche_")}
    assert exported == set(SC_ENTRY_POINTS), f"Python structured-credit surface diverged from WASM: {sorted(exported)}"


def test_tranche_metrics_happy_path_uses_model_price() -> None:
    """A valid fixture deal returns finite metrics at its own model price."""
    market = _market()

    metrics = json.loads(
        instruments.structured_credit_tranche_metrics(
            _valid_deal_json(),
            "SENIOR",
            market,
            "2024-01-01",
            market_price_pct=None,
        )
    )

    assert math.isfinite(metrics["pv"])
    assert metrics["pv"] > 0.0
    assert metrics["z_spread_bp"] == pytest.approx(0.0, abs=1e-5)


def test_unknown_tranche_raises_value_error_not_panic() -> None:
    """A bad tranche id surfaces as a typed, tranche-specific error."""
    with pytest.raises(ValueError, match="NO_SUCH_TRANCHE"):
        instruments.structured_credit_tranche_breakeven_cdr(
            _valid_deal_json(), "NO_SUCH_TRANCHE", _market(), "2024-01-01"
        )


def test_invalid_deal_reports_an_actionable_error() -> None:
    """Validation failures must name what is wrong, not fail opaquely."""
    with pytest.raises(ValueError, match="calendar") as excinfo:
        instruments.structured_credit_tranche_breakeven_cdr(_deal_json(), "CLASS_A", MarketContext(), "2024-01-01")
    message = str(excinfo.value)
    assert message.strip(), "the error message must not be empty"


def test_malformed_json_raises_rather_than_panics() -> None:
    """Garbage input must be rejected cleanly at the boundary."""
    with pytest.raises(ValueError, match=r"(?i)json|parse|expected"):
        instruments.structured_credit_tranche_breakeven_cdr("{not valid json", "CLASS_A", MarketContext(), "2024-01-01")
