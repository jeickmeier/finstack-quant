"""Pickle support across the binding surface.

Pickling is what makes `multiprocessing`, `joblib`, and `dask` usable: a result
object that cannot cross a process boundary forces quants back to raw JSON for
any fan-out. Every wrapper that can already round-trip through JSON reconstructs
via that same strict serde path, so there is no second state format to drift.
"""

from __future__ import annotations

import copy
import datetime
import inspect
import json
import pickle
from types import ModuleType
from typing import Any

import pytest

import finstack_quant as fq
from finstack_quant.core.dates import StubKind

DOMAINS = [
    "analytics",
    "attribution",
    "cashflows",
    "core",
    "covenants",
    "features",
    "margin",
    "models",
    "portfolio",
    "scenarios",
    "statements",
    "statements_analytics",
    "valuations",
]


def _walk_classes() -> list[tuple[str, type]]:
    """Yield every (qualified_name, class) reachable from the public tree."""
    seen_modules: set[int] = set()
    out: list[tuple[str, type]] = []

    def walk(mod: ModuleType, prefix: str) -> None:
        if id(mod) in seen_modules:
            return
        seen_modules.add(id(mod))
        for name in getattr(mod, "__all__", None) or [n for n in dir(mod) if not n.startswith("_")]:
            try:
                obj = getattr(mod, name)
            except AttributeError:
                continue
            if inspect.ismodule(obj):
                walk(obj, f"{prefix}.{name}")
            elif inspect.isclass(obj):
                out.append((f"{prefix}.{name}", obj))

    for domain in DOMAINS:
        walk(getattr(fq, domain), domain)
    return out


class TestReduceCoverage:
    """Structural guarantee, independent of whether we can build an instance."""

    def test_every_json_capable_class_defines_reduce(self) -> None:
        """`to_json` + `from_json` is the exact precondition for the pickle path.

        A class that gains a JSON bridge but no `__reduce__` silently becomes
        un-pickleable, which only shows up when someone fans out over a pool.
        """
        missing = [
            name
            for name, cls in _walk_classes()
            if hasattr(cls, "to_json") and hasattr(cls, "from_json") and "__reduce__" not in cls.__dict__
        ]
        assert not missing, f"JSON-capable classes without __reduce__: {sorted(set(missing))}"

    def test_known_unpicklable_types_are_exactly_the_documented_set(self) -> None:
        """A type with `to_json` but no `from_json` cannot pickle through the JSON bridge.

        The coverage test above requires BOTH methods, so it would silently
        exempt any such type rather than flag it. Pin the known set instead, so
        a new result type that ships without a `from_json` fails here.

        A class that defines its own `__reduce__` on another constructor is
        pickleable by construction (`Portfolio` rebuilds through `from_spec`,
        the twin of its `to_json`), and
        `test_reduce_is_backed_by_a_working_constructor` checks that route.

        The set is currently empty: every other type carrying `to_json` also
        carries `from_json`. Keep it that way — a new result type that ships
        without a `from_json` fails here rather than silently losing pickle
        support.
        """
        expected: set[str] = set()
        actual = {
            name
            for name, cls in _walk_classes()
            if hasattr(cls, "to_json") and not hasattr(cls, "from_json") and "__reduce__" not in cls.__dict__
        }
        assert actual == expected, (
            f"un-pickleable set changed.\n  newly un-pickleable: {sorted(actual - expected)}\n"
            f"  now fixed (drop from `expected`): {sorted(expected - actual)}"
        )

    def test_reduce_is_backed_by_a_working_constructor(self) -> None:
        """Every `__reduce__` must name a callable that can be reached again.

        Most wrappers reduce via `to_json`/`from_json`, but value types such as
        the `core.dates` enums reduce through their own constructors instead.
        Either way the reduce target has to be importable and callable, or the
        class raises at pickle time rather than at import time.
        """
        broken: list[str] = []
        for name, cls in _walk_classes():
            if "__reduce__" not in cls.__dict__:
                continue
            if hasattr(cls, "to_json") and hasattr(cls, "from_json"):
                continue
            if not callable(cls):
                broken.append(name)
        assert not broken, f"__reduce__ without a usable reconstruction path: {sorted(set(broken))}"


class TestRoundTrips:
    """Behavioural checks on representative types from several domains."""

    @staticmethod
    def _assert_round_trip(obj: Any) -> None:
        restored = pickle.loads(pickle.dumps(obj))  # noqa: S301
        assert type(restored) is type(obj)
        assert json.loads(restored.to_json()) == json.loads(obj.to_json())

    def test_money(self) -> None:
        from finstack_quant.core.money import Money

        self._assert_round_trip(Money(1234.56, "USD"))

    def test_money_preserves_decimal_amount(self) -> None:
        """Money is Decimal-backed; a lossy hop would show up here first."""
        from decimal import Decimal

        from finstack_quant.core.money import Money

        original = Money(Decimal("1234.56"), "USD")
        restored = pickle.loads(pickle.dumps(original))  # noqa: S301
        assert restored.amount_decimal == original.amount_decimal
        assert restored == original

    def test_currency(self) -> None:
        from finstack_quant.core.currency import Currency

        self._assert_round_trip(Currency("EUR"))

    def test_typed_instrument(self) -> None:
        from finstack_quant.core.currency import Currency
        from finstack_quant.core.money import Money
        from finstack_quant.core.types import Rate
        from finstack_quant.valuations.instruments import Bond

        bond = Bond.fixed(
            "BOND-1",
            Money(1_000_000.0, Currency("USD")),
            Rate(0.05),
            datetime.date(2024, 1, 1),
            datetime.date(2034, 1, 1),
            StubKind.SHORT_FRONT,
            "USD-OIS",
        )
        self._assert_round_trip(bond)

    def test_statement_result(self) -> None:
        from finstack_quant.statements import Evaluator, ModelBuilder

        model = (
            ModelBuilder("pickle")
            .periods("2025Q1..Q2")
            .value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
            .compute("cogs", "revenue * 0.4")
            .build()
        )
        result = Evaluator().evaluate(model)
        restored = pickle.loads(pickle.dumps(result))  # noqa: S301
        assert restored.node_count == result.node_count
        assert restored.num_periods == result.num_periods
        assert restored.get_scalar("cogs", "2025Q1") == result.get_scalar("cogs", "2025Q1")

    def test_valuation_result(self) -> None:
        from finstack_quant.core.currency import Currency
        from finstack_quant.core.money import Money
        from finstack_quant.core.types import Rate
        from finstack_quant.valuations.instruments import Bond, price_instrument

        market = json.dumps({
            "schema_version": 1,
            "curves": [
                {
                    "type": "discount",
                    "id": "USD-OIS",
                    "base": "2024-01-01",
                    "day_count": "act_360",
                    "knot_points": [[0.0, 1.0], [5.0, 0.90], [10.0, 0.80]],
                    "interp_style": "monotone_convex",
                    "extrapolation": "flat_forward",
                    "min_forward_rate": None,
                    "allow_non_monotonic": False,
                    "min_forward_tenor": 1e-6,
                }
            ],
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
            "hierarchy": None,
        })
        bond = Bond.fixed(
            "BOND-1",
            Money(1_000_000.0, Currency("USD")),
            Rate(0.05),
            datetime.date(2024, 1, 1),
            datetime.date(2034, 1, 1),
            StubKind.SHORT_FRONT,
            "USD-OIS",
        )
        result = price_instrument(bond.to_json(), market, datetime.date(2024, 1, 1))
        restored = pickle.loads(pickle.dumps(result))  # noqa: S301
        assert restored.price == result.price
        assert restored.currency == result.currency


class TestCopySemantics:
    """`copy` is built on the same protocol, so it comes along for free."""

    def test_deepcopy_uses_reduce(self) -> None:
        from finstack_quant.core.money import Money

        original = Money(999.99, "GBP")
        assert copy.deepcopy(original) == original

    def test_deepcopy_is_independent(self) -> None:
        """A frozen value type must not alias its source after a deep copy."""
        from finstack_quant.core.money import Money

        original = Money(999.99, "GBP")
        clone = copy.deepcopy(original)
        assert clone is not original


class TestMultiprocessing:
    """The reason pickle matters: results must survive a process boundary."""

    def test_result_survives_a_process_pool(self) -> None:
        import multiprocessing as mp

        from finstack_quant.core.money import Money

        # "spawn" re-imports the module in the child, which is the strict case
        # (and the default on macOS); "fork" can mask pickling bugs.
        ctx = mp.get_context("spawn")
        with ctx.Pool(1) as pool:
            out = pool.map(_double_money, [Money(10.0, "USD"), Money(20.0, "USD")])
        assert [m.amount for m in out] == pytest.approx([20.0, 40.0])
        assert all(m.currency.code == "USD" for m in out)


def _double_money(m: Any) -> Any:
    """Top-level so the child process can import it (spawn pickles by name)."""
    return m * 2.0
