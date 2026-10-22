"""Typed instruments skip the JSON round trip but prepare identically.

Pricing entry points clone a typed instance's inner Rust instrument instead of
serializing it to an envelope and parsing it back. The envelope path is
unchanged, so each typed call must match the same call on ``to_json()``
exactly: results, cashflows, VaR, structured-credit metrics, composite legs,
statement debt, scenario and horizon runs, portfolio positions, optimization
candidates, attribution, and error messages.
"""

from __future__ import annotations

from collections.abc import Callable
import datetime
import json

import pytest

from finstack_quant import statements
from finstack_quant.attribution import attribute_pnl, attribute_pnl_many
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, StubKind, Tenor
from finstack_quant.core.market_data import DiscountCurve, ForwardCurve, MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Bps, Rate
from finstack_quant.portfolio import CandidatePosition, Portfolio
from finstack_quant.scenarios import OperationSpec, ScenarioSpec, apply_scenario, compute_horizon_return
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
    tranche_metrics,
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


def _market(as_of: datetime.date = AS_OF, df_5y: float = 0.82) -> MarketContext:
    return (
        MarketContext()
        .insert(DiscountCurve("USD-OIS", as_of, [(0.0, 1.0), (1.0, 0.96), (5.0, df_5y), (10.0, 0.67)]))
        .insert(
            ForwardCurve(
                "USD-SOFR-3M",
                0.25,
                as_of,
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
            lambda tid=tranche_id: tranche_metrics(deal, tid, _market(), AS_OF),
            lambda tid=tranche_id: tranche_metrics(envelope, tid, _market(), AS_OF),
        )
        assert kind == "ok"


def test_composite_leg_typed_equal_envelope() -> None:
    bond = _fixed_bond()
    kind, _ = _assert_same(
        lambda: composite.CompositeLegSpec("B1", bond, 1.0),
        lambda: composite.CompositeLegSpec("B1", bond.to_json(), 1.0),
    )
    assert kind == "ok"


@pytest.mark.parametrize("cls", [Bond, TermLoan, EquityOption], ids=lambda cls: cls.__name__)
def test_statement_add_debt_typed_equal_envelope(cls: type) -> None:
    def build(instrument: object) -> object:
        builder = statements.ModelBuilder("deal")
        builder.add_debt("DEBT", instrument)
        builder.periods("2025Q1..Q2", None)
        builder.value("x", [("2025Q1", 1.0)])
        return builder.build()

    instrument = cls.example()
    kind, _ = _assert_same(lambda: build(instrument), lambda: build(instrument.to_json()))
    # Equity options are not capital-structure debt; the rejection must match too.
    assert kind == ("ValueError" if cls is EquityOption else "ok")


@pytest.mark.parametrize("cls", EXAMPLE_TYPES, ids=lambda cls: cls.__name__)
def test_scenario_and_horizon_typed_equal_envelope(cls: type) -> None:
    instrument = cls.example()
    envelope = instrument.to_json()
    roll = ScenarioSpec("roll", [OperationSpec.time_roll_forward("1M", roll_mode="calendar_days")])
    shock = ScenarioSpec("up25", [OperationSpec.curve_parallel_bp("discount", "USD-OIS", 25.0)])
    _assert_same(
        lambda: apply_scenario(roll, _market(), AS_OF, instruments=[instrument]),
        lambda: apply_scenario(roll, _market(), AS_OF, instruments=[envelope]),
    )
    _assert_same(
        lambda: compute_horizon_return(instrument, _market(), AS_OF, shock),
        lambda: compute_horizon_return(envelope, _market(), AS_OF, shock),
    )


@pytest.mark.parametrize("cls", EXAMPLE_TYPES, ids=lambda cls: cls.__name__)
def test_portfolio_position_and_candidate_typed_equal_envelope(cls: type) -> None:
    instrument = cls.example()
    envelope = instrument.to_json()
    kind, _ = _assert_same(
        lambda: Portfolio.builder("book", "USD", AS_OF).position("P1", instrument, 2.0).build(),
        lambda: Portfolio.builder("book", "USD", AS_OF).position("P1", envelope, 2.0).build(),
    )
    assert kind == "ok"
    kind, _ = _assert_same(
        lambda: CandidatePosition("C1", "ACME", instrument, max_weight=0.5),
        lambda: CandidatePosition("C1", "ACME", envelope, max_weight=0.5),
    )
    assert kind == "ok"


@pytest.mark.parametrize("cls", EXAMPLE_TYPES, ids=lambda cls: cls.__name__)
def test_attribution_typed_equal_envelope(cls: type) -> None:
    instrument = cls.example()
    envelope = instrument.to_json()
    t1 = datetime.date(2024, 1, 22)
    market_t1 = _market(t1, df_5y=0.81)
    _assert_same(
        lambda: attribute_pnl(instrument, _market(), market_t1, AS_OF, t1, "parallel"),
        lambda: attribute_pnl(envelope, _market(), market_t1, AS_OF, t1, "parallel"),
    )


def test_attribution_many_typed_equal_envelope() -> None:
    instruments = [_fixed_bond(), _frn()]
    t1 = datetime.date(2024, 1, 22)
    market_t1 = _market(t1, df_5y=0.81)
    typed = attribute_pnl_many(instruments, _market(), market_t1, AS_OF, t1, "parallel")
    via_json = attribute_pnl_many([i.to_json() for i in instruments], _market(), market_t1, AS_OF, t1, "parallel")
    assert typed.equals(via_json)


@pytest.mark.parametrize("make", [_fixed_bond, _frn], ids=["fixed", "frn"])
def test_priced_scenario_horizon_attribution_typed_equal_envelope(make: Callable[[], Bond]) -> None:
    """Most examples need curves `_market` lacks; these bonds price on it, so results are compared."""
    bond = make()
    envelope = bond.to_json()
    roll = ScenarioSpec("roll", [OperationSpec.time_roll_forward("1M", roll_mode="calendar_days")])
    shock = ScenarioSpec("up25", [OperationSpec.curve_parallel_bp("discount", "USD-OIS", 25.0)])
    t1 = datetime.date(2024, 1, 22)
    market_t1 = _market(t1, df_5y=0.81)
    for typed, via_json in [
        (
            lambda: apply_scenario(roll, _market(), AS_OF, instruments=[bond]),
            lambda: apply_scenario(roll, _market(), AS_OF, instruments=[envelope]),
        ),
        (
            lambda: compute_horizon_return(bond, _market(), AS_OF, shock),
            lambda: compute_horizon_return(envelope, _market(), AS_OF, shock),
        ),
        (
            lambda: attribute_pnl(bond, _market(), market_t1, AS_OF, t1, "parallel"),
            lambda: attribute_pnl(envelope, _market(), market_t1, AS_OF, t1, "parallel"),
        ),
    ]:
        kind, _ = _assert_same(typed, via_json)
        assert kind == "ok"
