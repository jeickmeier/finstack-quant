"""``Portfolio`` loads from JSON text with ``from_json``, and a bad spec fails the same way on every route."""

from __future__ import annotations

from collections.abc import Callable
import copy
import datetime as dt
import json
import pickle
from typing import Any

import pytest

from finstack_quant.portfolio import Portfolio, PortfolioError

AS_OF = dt.date(2025, 1, 2)


def _equity(instrument_id: str) -> dict[str, Any]:
    spec = {
        "id": instrument_id,
        "ticker": instrument_id,
        "currency": "USD",
        "quantity": 1.0,
        "quoted_spot": 100.0,
        "spot_id": None,
        "div_yield_id": None,
        "discrete_dividends": [],
        "discount_curve_id": "USD",
        "attributes": {"tags": [], "meta": {}},
    }
    return {"schema": "finstack_quant.instrument/1", "instrument": {"type": "equity", "spec": spec}}


def _portfolio() -> Portfolio:
    return (
        Portfolio
        .builder("book", "USD", AS_OF)
        .entity("ACME")
        .position("P1", _equity("EQ-1"), 10.0, entity_id="ACME")
        .tag("desk", "equities")
        .build()
    )


def _spec() -> dict[str, Any]:
    return _portfolio().to_spec()


def _duplicate_position(spec: dict[str, Any]) -> None:
    spec["positions"].append(copy.deepcopy(spec["positions"][0]))


# A bad spec, how to make it from a good one, and the exception its loader raises.
BAD_SPECS: list[tuple[str, Callable[[dict[str, Any]], Any], type[Exception]]] = [
    ("unknown-field", lambda spec: spec.update(zzz=1), ValueError),
    ("missing-base-currency", lambda spec: spec.pop("base_currency"), ValueError),
    ("unknown-entity", lambda spec: spec["positions"][0].update(entity_id="NOPE"), KeyError),
    ("quantity-as-text", lambda spec: spec["positions"][0].update(quantity="abc"), ValueError),
    ("positions-not-a-list", lambda spec: spec.update(positions={}), ValueError),
    ("unknown-currency", lambda spec: spec.update(base_currency="ZZZZ"), ValueError),
    ("duplicate-position", _duplicate_position, PortfolioError),
    ("invalid-date", lambda spec: spec.update(as_of="2025-02-30"), ValueError),
    ("unknown-instrument-type", lambda spec: spec["positions"][0]["instrument_spec"].update(type="zzz"), ValueError),
]
BAD_IDS = [label for label, _, _ in BAD_SPECS]


def _bad(mutate: Callable[[dict[str, Any]], Any]) -> dict[str, Any]:
    spec = _spec()
    mutate(spec)
    return spec


def test_from_json_rebuilds_the_portfolio_that_to_json_wrote() -> None:
    portfolio = _portfolio()

    rebuilt = Portfolio.from_json(portfolio.to_json())

    assert rebuilt == portfolio
    assert rebuilt.position_ids == ["P1"]
    assert json.loads(rebuilt.to_json()) == portfolio.to_spec()


def test_pickling_goes_through_from_json() -> None:
    portfolio = _portfolio()

    loader, arguments = portfolio.__reduce__()

    assert loader == Portfolio.from_json
    assert arguments == (portfolio.to_json(),)
    assert pickle.loads(pickle.dumps(portfolio)) == portfolio  # noqa: S301 - own serialized portfolio


@pytest.mark.parametrize(("label", "mutate", "expected"), BAD_SPECS, ids=BAD_IDS)
def test_from_json_rejects_a_bad_spec(
    label: str, mutate: Callable[[dict[str, Any]], Any], expected: type[Exception]
) -> None:
    with pytest.raises(expected) as raised:
        Portfolio.from_json(json.dumps(_bad(mutate)))
    assert type(raised.value) is expected, label


def test_from_json_rejects_text_that_is_not_json() -> None:
    with pytest.raises(ValueError, match="EOF while parsing"):
        Portfolio.from_json('{"positions":')
