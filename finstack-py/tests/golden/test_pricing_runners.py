"""Unit tests for the shared pricing runner's market-resolution logic."""

from __future__ import annotations

import pytest

from tests.golden.runners.pricing_common import _resolve_market


def _minimal_market_dict() -> dict:
    """Minimal valid MarketContext JSON shape (empty curves, no surfaces)."""
    return {
        "version": 2,
        "curves": [],
        "fx": None,
        "surfaces": [],
        "prices": {},
        "series": [],
        "inflation_indices": [],
        "dividends": [],
        "credit_indices": [],
        "fx_delta_vol_surfaces": [],
        "vol_cubes": [],
        "collateral": {},
    }


def _minimal_envelope_dict() -> dict:
    """Minimal valid CalibrationEnvelope JSON shape (no steps, no initial market)."""
    return {
        "schema": "finstack.calibration",
        "plan": {
            "id": "test_envelope",
            "quote_sets": {},
            "steps": [],
            "settings": {},
        },
    }


def test_resolve_market_snapshot_only() -> None:
    market = _resolve_market({"kind": "snapshot", "data": _minimal_market_dict()})
    assert market is not None


def test_resolve_market_envelope_only() -> None:
    market = _resolve_market({"kind": "envelope", "envelope": _minimal_envelope_dict()})
    assert market is not None


def test_resolve_market_rejects_unknown_kind() -> None:
    with pytest.raises(ValueError, match=r"snapshot.*envelope|market\.kind"):
        _resolve_market({"kind": "bogus"})
