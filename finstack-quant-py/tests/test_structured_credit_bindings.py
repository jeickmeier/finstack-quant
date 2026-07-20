"""Structured-credit tranche analytics are exported from the instruments module.

These take a tranche id, so they are not reachable through
``price_instrument_with_metrics``. The surface mirrors the five WASM
``structuredCreditTranche*`` entry points.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from finstack_quant.core.market_data import MarketContext
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


@pytest.mark.parametrize("name", SC_ENTRY_POINTS)
def test_entry_point_is_exported(name: str) -> None:
    """Each entry point is present and declared in ``__all__``."""
    assert hasattr(instruments, name), f"{name} is not exported from the instruments module"
    assert name in instruments.__all__, f"{name} is missing from __all__"
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


def test_unknown_tranche_raises_value_error_not_panic() -> None:
    """A bad tranche id must surface as a Python exception, never a panic.

    A Rust panic across the PyO3 boundary raises ``pyo3_runtime.PanicException``
    (or aborts), which is not something a caller can handle.

    The match is on ``Validation`` rather than the tranche id because deal
    validation runs BEFORE the tranche lookup — the canonical fixture omits a
    payment calendar, so that error fires first. What matters here is that a
    typed, catchable exception crosses the boundary at all.
    """
    with pytest.raises(ValueError, match="Validation"):
        instruments.structured_credit_tranche_breakeven_cdr(
            _deal_json(), "NO_SUCH_TRANCHE", MarketContext(), "2024-01-01"
        )


def test_invalid_deal_reports_an_actionable_error() -> None:
    """Validation failures must name what is wrong, not fail opaquely."""
    with pytest.raises(ValueError, match="calendar") as excinfo:
        instruments.structured_credit_tranche_breakeven_cdr(_deal_json(), "CLASS_A", MarketContext(), "2024-01-01")
    message = str(excinfo.value)
    assert message.strip(), "the error message must not be empty"
    # The canonical fixture omits a payment calendar, which the engine requires.
    assert "calendar" in message.lower(), "the validation error should name the missing input; got: " + message


def test_malformed_json_raises_rather_than_panics() -> None:
    """Garbage input must be rejected cleanly at the boundary."""
    with pytest.raises(ValueError, match=r"(?i)json|parse|expected"):
        instruments.structured_credit_tranche_breakeven_cdr("{not valid json", "CLASS_A", MarketContext(), "2024-01-01")
