"""Structured-credit tranche analytics are exported from the instruments module.

These take a tranche id, so they are not reachable through
``price_instrument``. The surface mirrors the five WASM
``structuredCreditTranche*`` entry points.
"""

from __future__ import annotations

from datetime import date
import json
import math

import pytest

from finstack_quant.core.market_data import (
    DiscountCurve,
    ForwardCurve,
    MarketContext,
    ScalarTimeSeries,
)
from finstack_quant.valuations import instruments
from tests.tests_typed_helpers import canonical_structured_credit_json

SC_ENTRY_POINTS = (
    "structured_credit_tranche_breakeven_cdr",
    "structured_credit_tranche_discount_margin",
    "structured_credit_tranche_metrics",
    "structured_credit_tranche_oas",
    "structured_credit_tranche_scenario_table",
)

# Both stochastic dimensions off collapses the OAS to a single deterministic
# scenario (see `OasConfig::num_paths`), which keeps this fast. Every field is
# required when a config is supplied; values track `OasConfig::default`.
_DETERMINISTIC_OAS_CONFIG_JSON = json.dumps({
    "num_paths": 1,
    "stochastic_rates": False,
    "stochastic_credit": False,
    "hw1f_mean_reversion": 0.05,
    "hw1f_sigma": 0.01,
    "prepay_beta": 7.0,
    "credit_loading": 0.3,
    "seed": 42,
    "tolerance": 1e-7,
})


def _approx(value: float) -> object:
    """Round-trip tolerance sized for serde_json's float reparse, nothing looser.

    A parse -> reserialize cycle can move an f64 by about 1 ULP, so ``rel=1e-15``
    (a few ULP) with ``abs=0`` covers it while keeping zeros exact.
    ``pytest.approx``'s default ``rel=1e-6`` would have accepted a PV of
    1,000,000.0 coming back as 1,000,000.9.
    """
    return pytest.approx(value, rel=1e-15, abs=0)


def _deal_json() -> str:
    return canonical_structured_credit_json(calendar_id="NOT_A_CALENDAR")


def _valid_deal_json() -> str:
    return canonical_structured_credit_json()


def _market() -> MarketContext:
    as_of = date(2024, 1, 1)
    market = (
        MarketContext()
        .insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
        .insert(
            ForwardCurve(
                "SOFR-3M",
                0.25,
                as_of,
                [(0.0, 0.04), (10.0, 0.04)],
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

    metrics = instruments.structured_credit_tranche_metrics(
        _valid_deal_json(),
        "CLONOTES-A",
        market,
        "2024-01-01",
        market_price_pct=None,
    )

    assert math.isfinite(metrics.pv)
    assert metrics.pv > 0.0
    assert metrics.z_spread_bp == pytest.approx(0.0, abs=1e-4)


def test_tranche_metrics_returns_typed_wrapper() -> None:
    """The metrics entry point returns ``TrancheMetrics``, not a JSON string."""
    metrics = instruments.structured_credit_tranche_metrics(_valid_deal_json(), "CLONOTES-A", _market(), "2024-01-01")

    assert isinstance(metrics, instruments.TrancheMetrics)
    assert not isinstance(metrics, str)

    # The wire payload is still one call away and survives a round trip.
    wire = metrics.to_json()
    payload = json.loads(wire)
    reparsed = json.loads(instruments.TrancheMetrics.from_json(wire).to_json())
    assert reparsed.keys() == payload.keys()
    for key, expected in payload.items():
        if isinstance(expected, float):
            assert reparsed[key] == _approx(expected)
        else:
            assert reparsed[key] == expected


def test_tranche_oas_returns_typed_wrapper() -> None:
    """The OAS entry point returns ``OasResult``, not a JSON string."""
    market = _market()
    deal_json = _valid_deal_json()
    # Solve against the deal's own model price so the Brent search starts
    # inside its bracket; this test is about the return type, not the level.
    model_price_pct = instruments.structured_credit_tranche_metrics(
        deal_json, "CLONOTES-A", market, "2024-01-01"
    ).price_pct

    result = instruments.structured_credit_tranche_oas(
        deal_json,
        "CLONOTES-A",
        market,
        "2024-01-01",
        model_price_pct,
        _DETERMINISTIC_OAS_CONFIG_JSON,
    )

    assert isinstance(result, instruments.OasResult)
    assert not isinstance(result, str)
    # The solver echoes back the price it was handed; nothing computes on it,
    # so this is an exact identity rather than a numerical comparison.
    assert result.market_price == model_price_pct

    wire = result.to_json()
    payload = json.loads(wire)
    reparsed = json.loads(instruments.OasResult.from_json(wire).to_json())
    assert reparsed.keys() == payload.keys()
    for key, expected in payload.items():
        if isinstance(expected, float):
            assert reparsed[key] == _approx(expected)
        else:
            assert reparsed[key] == expected


def test_tranche_scenario_table_returns_typed_wrapper() -> None:
    """The scenario-table entry point returns ``ScenarioTable``, not JSON."""
    grid_json = json.dumps({"cprs": [0.10], "cdrs": [0.02], "severities": [0.40]})

    table = instruments.structured_credit_tranche_scenario_table(
        _valid_deal_json(), "CLONOTES-A", _market(), "2024-01-01", grid_json
    )

    assert isinstance(table, instruments.ScenarioTable)
    assert not isinstance(table, str)
    assert table.tranche_id == "CLONOTES-A"
    assert len(table.cells()) == 1

    wire = table.to_json()
    payload = json.loads(wire)
    reparsed = json.loads(instruments.ScenarioTable.from_json(wire).to_json())
    assert reparsed.keys() == payload.keys()
    assert reparsed["tranche_id"] == payload["tranche_id"]
    assert len(reparsed["cells"]) == len(payload["cells"])
    for got, want in zip(reparsed["cells"], payload["cells"], strict=True):
        assert got == _approx(want)


def test_unknown_tranche_raises_key_error_not_panic() -> None:
    """A bad tranche id surfaces as a typed, tranche-specific lookup error.

    Missing identifiers map to ``KeyError`` per the binding error contract.
    """
    with pytest.raises(KeyError, match="NO_SUCH_TRANCHE"):
        instruments.structured_credit_tranche_breakeven_cdr(
            _valid_deal_json(), "NO_SUCH_TRANCHE", _market(), "2024-01-01"
        )


def test_invalid_deal_reports_an_actionable_error() -> None:
    """Validation failures must name what is wrong, not fail opaquely.

    Uses the real tranche id from the registry-generated deal. This
    previously passed ``CLASS_A``, which does not exist, and relied on the
    missing-calendar error firing before the tranche was ever resolved. Once
    breakeven CDR began resolving the tranche up front — to scale its
    writedown threshold to the tranche face (SC-m03) — the unknown id
    surfaced first, which is the better failure. Naming a real tranche keeps
    this test on the deal-validation behaviour it is actually about.
    """
    with pytest.raises(KeyError, match=r"(?i)calendar") as excinfo:
        instruments.structured_credit_tranche_breakeven_cdr(_deal_json(), "CLONOTES-A", MarketContext(), "2024-01-01")
    message = str(excinfo.value)
    assert message.strip(), "the error message must not be empty"


def test_malformed_json_raises_rather_than_panics() -> None:
    """Garbage input must be rejected cleanly at the boundary."""
    with pytest.raises(ValueError, match=r"(?i)json|parse|expected"):
        instruments.structured_credit_tranche_breakeven_cdr(
            "{not valid json", "CLONOTES-A", MarketContext(), "2024-01-01"
        )
