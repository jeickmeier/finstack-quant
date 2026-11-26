"""``price_instrument`` on a typed instrument equals the instrument's own ``price`` and ``metric``.

Every typed instrument class is priced through both routes and the outcomes
are compared: the result's ``to_json()`` text, the metric value, or the type
and message of the exception. The markets are an empty ``MarketContext`` and
the ones stored in the cross-host golden that
``tests/test_valuations_wasm_parity.py`` writes.
"""

from __future__ import annotations

from collections.abc import Callable
import inspect
import json
from typing import Any

import pytest

from finstack_quant.core.market_data import MarketContext
from finstack_quant.valuations import ValuationResult, instruments as ins
from tests import test_valuations_wasm_parity as parity_cases
from tests.tests_typed_helpers import build_irs

GOLDEN = json.loads(parity_cases.GOLDEN.read_text())


def _typed_classes() -> list[str]:
    """Exported instrument classes with ``example()`` and a ``price`` method that takes ``market``."""
    names = []
    for name in ins.__all__:
        cls = getattr(ins, name)
        price = getattr(cls, "price", None)
        if (
            inspect.isclass(cls)
            and hasattr(cls, "example")
            and callable(price)
            and "market" in inspect.signature(price).parameters
        ):
            names.append(name)
    return names


TYPED_CLASSES = _typed_classes()

# Market label -> (market, valuation date). Every stored market is one the
# parity test builds; each typed example is priced on all of them.
MARKETS: dict[str, tuple[MarketContext, str]] = {
    "empty": (MarketContext(), "2024-01-15"),
    "rates": (MarketContext.from_json(GOLDEN["rates"]["market"]), "2024-01-15"),
    "credit": (MarketContext.from_json(GOLDEN["credit"]["market"]), parity_cases.AS_OF),
    "equity": (MarketContext.from_json(GOLDEN["equity"]["market"]), "2024-01-15"),
    "convertible": (MarketContext.from_json(GOLDEN["convertible"]["market"]), parity_cases.AS_OF),
    "structured": (MarketContext.from_json(GOLDEN["structured"]["market"]), GOLDEN["structured"]["as_of"]),
    "g_spread": (MarketContext.from_json(GOLDEN["g_spread"]["market"]), GOLDEN["g_spread"]["as_of"]),
    "default_risk": (MarketContext.from_json(GOLDEN["default_risk"]["market"]), GOLDEN["default_risk"]["as_of"]),
}

# Label -> (instrument, market label, model): the instruments the parity test
# prices, each on the market it is priced on there.
PRICED: dict[str, tuple[Any, str, str]] = {
    "InterestRateSwap": (build_irs(), "rates", "default"),
    "CreditDefaultSwap": (ins.CreditDefaultSwap.example(), "credit", "hazard_rate"),
    "EquityOption": (ins.EquityOption.example(), "equity", "default"),
    "ConvertibleBond": (ins.ConvertibleBond.example(), "convertible", "default"),
    "StructuredCredit": (ins.StructuredCredit.from_json(GOLDEN["structured"]["deal"]), "structured", "default"),
    "Bond/government_curve": (ins.Bond.from_json(GOLDEN["g_spread"]["bond"]), "g_spread", "default"),
    "Bond/own_curve": (ins.Bond.from_json(GOLDEN["g_spread"]["own_curve_bond"]), "g_spread", "default"),
    "Bond/no_government_curve": (ins.Bond.from_json(GOLDEN["g_spread"]["plain_bond"]), "g_spread", "default"),
    "Bond/not_hazard_priced": (
        ins.Bond.from_json(GOLDEN["default_risk"]["not_hazard_priced"]),
        "default_risk",
        "default",
    ),
    **{
        f"{name}/hazard": (getattr(ins, name).from_json(case["json"]), "default_risk", "default")
        for name, case in GOLDEN["default_risk"]["cases"].items()
    },
}


def _metric_ids(instrument: Any) -> list[str]:
    """Every metric id the standard registry applies to the instrument's type."""
    type_tag = json.loads(instrument.to_json())["instrument"]["type"]
    return [metric for group in ins.metrics_for_instrument(type_tag).values() for metric in group]


def _outcome(call: Callable[[], Any]) -> tuple[str, ...]:
    """What a pricing call produced: a result's JSON, a value, or the exception's type and message."""
    try:
        value = call()
    except Exception as error:  # noqa: BLE001 - every failure a route can raise is part of the comparison
        return ("raised", type(error).__name__, str(error))
    if isinstance(value, ValuationResult):
        return ("result", value.to_json())
    return ("value", repr(value))


def _option_cases(metric_id: str | None) -> dict[str, dict[str, Any]]:
    """The keyword sets each route is called with."""
    cases: dict[str, dict[str, Any]] = {
        "plain": {},
        "explain": {"extras": {"explain": True}},
        "empty_overrides": {"extras": {"metric_pricing_overrides": {}}},
    }
    if metric_id is not None:
        cases["metric"] = {"metrics": [metric_id]}
    return cases


def _assert_price_routes_agree(instrument: Any, market: MarketContext, as_of: str, where: str, **kw: Any) -> tuple:
    """Both ``price`` routes give one outcome; returns it."""
    method = _outcome(lambda: instrument.price(market, as_of, **kw))
    function = _outcome(lambda: ins.price_instrument(instrument, market, as_of, **kw))
    assert function == method, f"{where}: price_instrument differs from the typed price for {kw}"
    return function


def _assert_metric_routes_agree(
    instrument: Any, market: MarketContext, as_of: str, metric_id: str, where: str, **kw: Any
) -> tuple:
    """``metric(id)`` and ``price_instrument(metrics=[id])[id]`` give one outcome; returns it."""
    method = _outcome(lambda: instrument.metric(market, as_of, metric_id, **kw))
    function = _outcome(lambda: ins.price_instrument(instrument, market, as_of, metrics=[metric_id], **kw)[metric_id])
    assert function == method, f"{where}: price_instrument differs from the typed metric {metric_id!r} for {kw}"
    return function


def test_the_sweep_covers_every_typed_instrument_class() -> None:
    assert len(TYPED_CLASSES) == 77
    for name in TYPED_CLASSES:
        assert "market" in inspect.signature(getattr(ins, name).metric).parameters, name


@pytest.mark.parametrize("name", TYPED_CLASSES)
def test_price_instrument_equals_the_typed_methods_on_every_market(name: str) -> None:
    instrument = getattr(ins, name).example()
    metric_ids = _metric_ids(instrument)
    metric_id = metric_ids[0] if metric_ids else None
    for label, (market, as_of) in MARKETS.items():
        where = f"{name} on the {label} market"
        for kw in _option_cases(metric_id).values():
            _assert_price_routes_agree(instrument, market, as_of, where, **kw)
        if metric_id is not None:
            _assert_metric_routes_agree(instrument, market, as_of, metric_id, where)


@pytest.mark.parametrize("label", PRICED)
def test_price_instrument_equals_the_typed_methods_on_a_pricing_market(label: str) -> None:
    instrument, market_label, model = PRICED[label]
    market, as_of = MARKETS[market_label]
    metric_ids = _metric_ids(instrument)
    plain = _assert_price_routes_agree(instrument, market, as_of, label, model=model)
    assert plain[0] == "result", f"{label} must price on its market: {plain}"
    # One route run twice: a pricer that is not repeatable would make the comparison meaningless.
    assert _outcome(lambda: ins.price_instrument(instrument, market, as_of, model=model)) == plain
    for kw in _option_cases(None).values():
        _assert_price_routes_agree(instrument, market, as_of, label, model=model, **kw)
    _assert_price_routes_agree(instrument, market, as_of, label, model=model, metrics=metric_ids)
    for metric_id in metric_ids:
        _assert_price_routes_agree(instrument, market, as_of, label, model=model, metrics=[metric_id])
        _assert_metric_routes_agree(instrument, market, as_of, metric_id, label, model=model)
