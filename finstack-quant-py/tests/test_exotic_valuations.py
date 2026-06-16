"""Direct exotic valuation binding smoke tests."""

from __future__ import annotations

import json

import pytest

from finstack_quant.valuations import exotics


def _asian_option_spec() -> dict[str, object]:
    return {
        "id": "ASIAN-TEST",
        "underlying_ticker": "SPX",
        "strike": 100.0,
        "option_type": "call",
        "averaging_method": "arithmetic",
        "fixing_dates": ["2026-03-31", "2026-06-30"],
        "expiry": "2026-06-30",
        "notional": {"amount": 1_000_000.0, "currency": "USD"},
        "discount_curve_id": "USD-OIS",
        "spot_id": "SPX-SPOT",
        "vol_surface_id": "EQ-VOL",
        "div_yield_id": None,
        "pricing_overrides": {},
        "day_count": "Act365F",
        "attributes": {},
    }


def test_exotics_namespace_exports_direct_classes() -> None:
    expected = {"AsianOption", "BarrierOption", "LookbackOption", "Basket"}

    assert expected.issubset(set(exotics.__all__))
    for name in expected:
        assert hasattr(exotics, name)


def test_asian_option_kwargs_roundtrip() -> None:
    option = exotics.AsianOption(**_asian_option_spec())

    payload = json.loads(option.to_json())
    assert payload["type"] == "asian_option"
    assert payload["spec"]["id"] == "ASIAN-TEST"
    assert option.validate() is None


def test_asian_option_constructor_rejects_invalid_schema() -> None:
    spec = _asian_option_spec()
    del spec["strike"]

    with pytest.raises(ValueError, match="strike"):
        exotics.AsianOption(spec)
