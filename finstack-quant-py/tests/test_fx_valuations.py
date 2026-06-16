"""Direct FX valuation binding smoke tests."""

from __future__ import annotations

import json

import pytest

from finstack_quant.core.market_data import MarketContext
from finstack_quant.valuations import fx


def _fx_spot_spec() -> dict[str, object]:
    return {
        "id": "EURUSD-SPOT",
        "base_currency": "EUR",
        "quote_currency": "USD",
        "settlement": "2025-01-17",
        "spot_rate": 1.20,
        "notional": {"amount": 1_000_000.0, "currency": "EUR"},
        "attributes": {},
    }


def _fx_forward_spec() -> dict[str, object]:
    return {
        "id": "EURUSD-FWD",
        "base_currency": "EUR",
        "quote_currency": "USD",
        "maturity": "2025-06-15",
        "notional": {"amount": 1_000_000.0, "currency": "EUR"},
        "contract_rate": 1.12,
        "domestic_discount_curve_id": "USD-OIS",
        "foreign_discount_curve_id": "EUR-OIS",
        "attributes": {},
    }


def test_fx_namespace_exports_direct_classes() -> None:
    expected = {
        "FxSpot",
        "FxForward",
        "FxSwap",
        "Ndf",
        "FxOption",
        "FxDigitalOption",
        "FxTouchOption",
        "FxBarrierOption",
        "FxVarianceSwap",
        "QuantoOption",
    }
    assert expected.issubset(set(fx.__all__))
    for name in expected:
        assert hasattr(fx, name)


def test_fx_spot_kwargs_roundtrip_and_default_price() -> None:
    spot = fx.FxSpot(**_fx_spot_spec())

    payload = json.loads(spot.to_json())
    assert payload["type"] == "fx_spot"
    assert payload["spec"]["id"] == "EURUSD-SPOT"
    assert spot.validate() is None

    result = json.loads(spot.price(MarketContext(), "2025-01-15"))
    assert result["value"]["currency"] == "USD"
    assert float(result["value"]["amount"]) == pytest.approx(1_200_000.0)


def test_fx_forward_from_json_validates_economics() -> None:
    spec = _fx_forward_spec()
    spec["contract_rate"] = -1.0

    with pytest.raises(ValueError, match="contract_rate"):
        fx.FxForward(spec)

    with pytest.raises(ValueError, match="expected instrument type"):
        fx.FxForward.from_json(json.dumps({"type": "fx_spot", "spec": _fx_spot_spec()}))


def test_fx_option_classes_expose_greek_methods() -> None:
    for cls_name in (
        "FxOption",
        "FxDigitalOption",
        "FxTouchOption",
        "FxBarrierOption",
        "QuantoOption",
    ):
        cls = getattr(fx, cls_name)
        for method in (
            "delta",
            "gamma",
            "vega",
            "theta",
            "rho",
            "foreign_rho",
            "vanna",
            "volga",
            "greeks",
        ):
            assert hasattr(cls, method), f"{cls_name} missing {method}"
