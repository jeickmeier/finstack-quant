"""Typed instruments skip the JSON round trip but prepare identically.

Pricing entry points clone a typed instance's inner Rust instrument instead of
serializing it to an envelope and parsing it back. The envelope path is
unchanged, so each typed call must match the same call on ``to_json()``
exactly: results, cashflows, VaR, structured-credit metrics, composite legs,
and error messages.
"""

from __future__ import annotations

from collections.abc import Callable
import datetime
import json

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, StubKind, Tenor
from finstack_quant.core.market_data import DiscountCurve, ForwardCurve, MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Bps, Rate
from finstack_quant.valuations import composite
from finstack_quant.valuations.instruments import (
    AssetBackedFacility,
    Bond,
    CapFloor,
    CdsIndex,
    CdsOption,
    CdsTranche,
    ConvertibleBond,
    CreditDefaultSwap,
    EquityOption,
    FxForward,
    FxOption,
    InterestRateSwap,
    MarketHistory,
    RevolvingCredit,
    StructuredCredit,
    Swaption,
    TermLoan,
    calculate_var,
    instrument_cashflows,
    price_instrument,
    structured_credit_tranche_metrics,
)

AS_OF = datetime.date(2024, 1, 15)

EXAMPLE_TYPES = [
    AssetBackedFacility,
    Bond,
    CapFloor,
    CdsIndex,
    CdsOption,
    CdsTranche,
    ConvertibleBond,
    CreditDefaultSwap,
    EquityOption,
    FxForward,
    FxOption,
    InterestRateSwap,
    RevolvingCredit,
    StructuredCredit,
    Swaption,
    TermLoan,
]


def _market() -> MarketContext:
    return (
        MarketContext()
        .insert(DiscountCurve("USD-OIS", AS_OF, [(0.0, 1.0), (1.0, 0.96), (5.0, 0.82), (10.0, 0.67)]))
        .insert(
            ForwardCurve(
                "USD-SOFR-3M",
                0.25,
                AS_OF,
                [(0.0, 0.04), (5.0, 0.042), (10.0, 0.045)],
                day_count="act_360",
            )
        )
    )


def _fixed_bond() -> Bond:
    return Bond.fixed(
        "B1",
        Money(1_000_000.0, Currency("USD")),
        Rate(0.05),
        AS_OF,
        datetime.date(2029, 1, 15),
        StubKind.NONE,
        "USD-OIS",
    )


def _frn() -> Bond:
    return Bond.floating(
        "FRN1",
        Money(1_000_000.0, Currency("USD")),
        "USD-SOFR-3M",
        Bps(150),
        datetime.date(2024, 3, 15),
        datetime.date(2029, 1, 15),
        Tenor.quarterly(),
        DayCount.ACT_360,
        "USD-OIS",
    )


def _snapshot(value: object) -> object:
    """Comparable form of a result: its wire JSON without the wall-clock stamp."""
    payload = json.loads(value.to_json())  # type: ignore[attr-defined]
    if isinstance(payload, dict) and isinstance(payload.get("meta"), dict):
        payload["meta"].pop("timestamp", None)
    return payload


def _outcome(call: Callable[[], object]) -> tuple[str, object]:
    """Snapshot a call's result, or its exception type and message."""
    try:
        return ("ok", _snapshot(call()))
    except Exception as exc:  # noqa: BLE001 - the error itself is compared
        return (type(exc).__name__, str(exc))


def _assert_same(typed: Callable[[], object], via_json: Callable[[], object]) -> tuple[str, object]:
    typed_outcome = _outcome(typed)
    assert typed_outcome == _outcome(via_json)
    return typed_outcome


@pytest.mark.parametrize("cls", EXAMPLE_TYPES, ids=lambda cls: cls.__name__)
def test_price_and_cashflows_typed_equal_envelope(cls: type) -> None:
    instrument = cls.example()
    envelope = instrument.to_json()
    market = _market()
    _assert_same(
        lambda: price_instrument(instrument, market, AS_OF),
        lambda: price_instrument(envelope, market, AS_OF),
    )
    _assert_same(
        lambda: instrument_cashflows(instrument, market, AS_OF, "discounting"),
        lambda: instrument_cashflows(envelope, market, AS_OF, "discounting"),
    )


@pytest.mark.parametrize(
    ("make", "metrics"),
    [
        (_fixed_bond, ["dirty_price", "ytm", "dv01", "z_spread", "theta", "bucketed_dv01"]),
        (_frn, ["dirty_price", "dv01", "discount_margin", "theta", "bucketed_dv01"]),
    ],
    ids=["fixed", "frn"],
)
def test_bond_metrics_typed_equal_envelope(make: Callable[[], Bond], metrics: list[str]) -> None:
    bond = make()
    kind, _ = _assert_same(
        lambda: price_instrument(bond, _market(), AS_OF, metrics=metrics),
        lambda: price_instrument(bond.to_json(), _market(), AS_OF, metrics=metrics),
    )
    assert kind == "ok"


def test_bond_cashflows_typed_equal_envelope() -> None:
    bond = _fixed_bond()
    kind, _ = _assert_same(
        lambda: instrument_cashflows(bond, _market(), AS_OF, "discounting"),
        lambda: instrument_cashflows(bond.to_json(), _market(), AS_OF, "discounting"),
    )
    assert kind == "ok"


def test_metric_pricing_overrides_merge_identically() -> None:
    overrides = {"bump_config": {"rate_bump_bp": 2.5}, "theta_period": {"count": 1, "unit": "weeks"}}
    bond = _fixed_bond()
    kind, _ = _assert_same(
        lambda: price_instrument(bond, _market(), AS_OF, metrics=["dv01", "theta"], metric_pricing_overrides=overrides),
        lambda: price_instrument(
            bond.to_json(), _market(), AS_OF, metrics=["dv01", "theta"], metric_pricing_overrides=overrides
        ),
    )
    assert kind == "ok"
    kind, _ = _assert_same(
        lambda: price_instrument(bond, _market(), AS_OF, metric_pricing_overrides='{"bogus": 1}'),
        lambda: price_instrument(bond.to_json(), _market(), AS_OF, metric_pricing_overrides='{"bogus": 1}'),
    )
    assert kind == "ValueError"


def test_calculate_var_typed_equal_envelope() -> None:
    shift = lambda s: [  # noqa: E731
        {"factor": {"type": "discount_rate", "curve_id": "USD-OIS", "tenor_years": 5.0}, "shift": s}
    ]
    history = MarketHistory(
        AS_OF,
        3,
        [
            {"date": "2024-01-12", "shifts": shift(0.0010)},
            {"date": "2024-01-11", "shifts": shift(-0.0005)},
            {"date": "2024-01-10", "shifts": shift(0.0020)},
        ],
    )
    instruments = [_fixed_bond(), _frn()]
    kind, _ = _assert_same(
        lambda: calculate_var(instruments, _market(), history, AS_OF),
        lambda: calculate_var([i.to_json() for i in instruments], _market(), history, AS_OF),
    )
    assert kind == "ok"


def test_structured_credit_metrics_typed_equal_envelope() -> None:
    deal = StructuredCredit.example()
    envelope = deal.to_json()
    tranche_ids = [t["id"] for t in json.loads(envelope)["instrument"]["spec"]["tranches"]["tranches"]]
    assert tranche_ids
    for tranche_id in tranche_ids:
        kind, _ = _assert_same(
            lambda tid=tranche_id: structured_credit_tranche_metrics(deal, tid, _market(), AS_OF),
            lambda tid=tranche_id: structured_credit_tranche_metrics(envelope, tid, _market(), AS_OF),
        )
        assert kind == "ok"


def test_composite_leg_typed_equal_envelope() -> None:
    bond = _fixed_bond()
    kind, _ = _assert_same(
        lambda: composite.CompositeLegSpec("B1", bond, 1.0),
        lambda: composite.CompositeLegSpec("B1", bond.to_json(), 1.0),
    )
    assert kind == "ok"
