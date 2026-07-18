"""Deterministic synthetic data generators for example notebooks."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from datetime import date
import random
from typing import Final, NamedTuple

import pandas as pd

from finstack_quant.statements import FinancialModelSpec, ModelBuilder

DEMO_PL_PERIODS: Final = (
    "2024Q1",
    "2024Q2",
    "2024Q3",
    "2024Q4",
    "2025Q1",
    "2025Q2",
    "2025Q3",
    "2025Q4",
)
DEMO_PL_REVENUE: Final = (100.0, 104.0, 108.0, 112.0, 116.0, 120.0, 124.0, 128.0)
DEMO_PL_COGS: Final = (55.0, 57.0, 59.0, 61.0, 63.0, 65.0, 67.0, 69.0)
DEMO_PL_OPEX: Final = (18.0, 18.0, 19.0, 19.0, 20.0, 20.0, 21.0, 21.0)

WALK_START: Final = date(2024, 1, 1)


class RandomWalkPanel(NamedTuple):
    """A deterministic synthetic price/return panel.

    Attributes:
        dates: Business days, one per period.
        prices: One price list per series, each of length ``n_periods``.
        returns: One simple-return list per series, each of length ``n_periods - 1``.
        names: Series names, aligned with ``prices`` and ``returns``.
    """

    dates: list[date]
    prices: list[list[float]]
    returns: list[list[float]]
    names: list[str]


def _per_series(value: float | Sequence[float], n_series: int, label: str) -> list[float]:
    if isinstance(value, (int, float)):
        return [float(value)] * n_series
    values = [float(v) for v in value]
    if len(values) != n_series:
        msg = f"Expected {n_series} {label} values, got {len(values)}"
        raise ValueError(msg)
    return values


def random_walk_panel(
    n_periods: int,
    n_series: int,
    *,
    seed: int,
    start: float = 100.0,
    drift: float | Sequence[float] = 0.0,
    vol: float | Sequence[float] = 0.01,
    names: Sequence[str] | None = None,
) -> RandomWalkPanel:
    """Generate a deterministic multiplicative random-walk price panel.

    Each series evolves as ``p[t] = p[t-1] * (1 + drift + N(0, vol))`` on a
    business-day axis starting at 2024-01-01. Series are generated one at a
    time from a single :class:`random.Random` seeded with *seed*, so output is
    reproducible for a given ``(seed, n_periods, n_series)``.

    Args:
        n_periods: Number of business days (and prices) per series.
        n_series: Number of series to generate.
        seed: Seed for the internal ``random.Random``.
        start: Starting price, shared by every series.
        drift: Per-period drift; a scalar applied to all series, or one value
            per series.
        vol: Per-period return standard deviation; scalar or one value per series.
        names: Series names. Defaults to ``["S0", "S1", ...]``.

    Returns:
        Dates plus per-series prices, returns, and names.

    Raises:
        ValueError: If sizes are too small, or a per-series ``drift``/``vol``/
            ``names`` sequence does not have ``n_series`` entries.

    Examples:
        >>> panel = random_walk_panel(5, 2, seed=42, names=["A", "B"])
        >>> len(panel.dates), len(panel.prices[0]), len(panel.returns[0])
        (5, 5, 4)
    """
    if n_periods < 2:
        msg = f"n_periods must be at least 2, got {n_periods}"
        raise ValueError(msg)
    if n_series < 1:
        msg = f"n_series must be at least 1, got {n_series}"
        raise ValueError(msg)

    drifts = _per_series(drift, n_series, "drift")
    vols = _per_series(vol, n_series, "vol")
    if names is None:
        labels = [f"S{i}" for i in range(n_series)]
    else:
        labels = list(names)
        if len(labels) != n_series:
            msg = f"Expected {n_series} names, got {len(labels)}"
            raise ValueError(msg)

    dates = [ts.date() for ts in pd.bdate_range(WALK_START, periods=n_periods)]

    rng = random.Random(seed)
    prices: list[list[float]] = []
    returns: list[list[float]] = []
    for series_drift, series_vol in zip(drifts, vols, strict=True):
        path = [start]
        rets: list[float] = []
        for _ in range(n_periods - 1):
            step = series_drift + rng.gauss(0.0, series_vol)
            rets.append(step)
            path.append(path[-1] * (1.0 + step))
        prices.append(path)
        returns.append(rets)

    return RandomWalkPanel(dates=dates, prices=prices, returns=returns, names=labels)


def demo_pl_builder(
    model_id: str = "demo-pl",
    *,
    periods: Sequence[str] | None = None,
    revenue: Sequence[float] | None = None,
    cogs: Sequence[float] | None = None,
    opex: Sequence[float] | None = None,
    actual_through: str | None = None,
    currency: str = "USD",
    margins: bool = True,
    values: Mapping[str, Sequence[float]] | None = None,
    computes: Mapping[str, str] | None = None,
) -> ModelBuilder:
    """Build the shared quarterly P&L ``ModelBuilder`` without calling ``build``.

    Lines are always ``revenue``, ``cogs``, ``gross_profit`` (computed),
    ``opex``, and ``ebitda`` (computed). Currency metadata is stamped so
    downstream DCF and tear-sheet helpers work. All derived lines are computed
    in-model via ``compute``; nothing is precomputed in Python.

    Use this variant when the caller must extend the builder before building,
    e.g. attaching a ``ForecastSpec``; otherwise prefer :func:`demo_pl_model`.

    Args:
        model_id: Model identifier.
        periods: Period identifiers. Defaults to eight quarters, 2024Q1 through
            2025Q4.
        revenue: One value per period. A list shorter than *periods* populates
            only the leading periods, leaving the tail for a forecast to supply.
        cogs: Cost of goods sold, same length rules as *revenue*.
        opex: Operating expense, same length rules as *revenue*.
        actual_through: Last actual period. Defaults to the last period of the
            first half.
        currency: ISO 4217 code stamped as model ``currency`` metadata.
        margins: When true, also compute ``gross_margin`` and ``ebitda_margin``
            in percent.
        values: Extra value lines, ``{line_id: values}``, added after ``ebitda``.
        computes: Extra computed lines, ``{line_id: formula}``, added last.

    Returns:
        A configured, unbuilt builder.

    Raises:
        ValueError: If *periods* is empty or a value list is longer than it.
    """
    ids = list(periods) if periods is not None else list(DEMO_PL_PERIODS)
    if not ids:
        msg = "periods must not be empty"
        raise ValueError(msg)

    def _pairs(line_id: str, vals: Sequence[float]) -> list[tuple[str, float]]:
        if len(vals) > len(ids):
            msg = f"{line_id} has {len(vals)} values but only {len(ids)} periods"
            raise ValueError(msg)
        return [(period, float(v)) for period, v in zip(ids, vals, strict=False)]

    builder = ModelBuilder(model_id)
    builder.periods(
        f"{ids[0]}..{ids[-1]}",
        actual_through if actual_through is not None else ids[(len(ids) + 1) // 2 - 1],
    )
    builder.with_meta("currency", f'"{currency}"')

    builder.value("revenue", _pairs("revenue", revenue if revenue is not None else DEMO_PL_REVENUE))
    builder.value("cogs", _pairs("cogs", cogs if cogs is not None else DEMO_PL_COGS))
    builder.compute("gross_profit", "revenue - cogs")
    builder.value("opex", _pairs("opex", opex if opex is not None else DEMO_PL_OPEX))
    builder.compute("ebitda", "gross_profit - opex")

    for line_id, vals in (values or {}).items():
        builder.value(line_id, _pairs(line_id, vals))
    if margins:
        builder.compute("gross_margin", "gross_profit / revenue * 100")
        builder.compute("ebitda_margin", "ebitda / revenue * 100")
    for line_id, formula in (computes or {}).items():
        builder.compute(line_id, formula)
    return builder


def demo_pl_model(
    model_id: str = "demo-pl",
    *,
    periods: Sequence[str] | None = None,
    revenue: Sequence[float] | None = None,
    cogs: Sequence[float] | None = None,
    opex: Sequence[float] | None = None,
    actual_through: str | None = None,
    currency: str = "USD",
    margins: bool = True,
    values: Mapping[str, Sequence[float]] | None = None,
    computes: Mapping[str, str] | None = None,
) -> FinancialModelSpec:
    """Build the shared quarterly P&L model spec.

    Thin terminal wrapper over :func:`demo_pl_builder`; see it for parameter
    documentation.

    Returns:
        The built statements model spec.
    """
    return demo_pl_builder(
        model_id,
        periods=periods,
        revenue=revenue,
        cogs=cogs,
        opex=opex,
        actual_through=actual_through,
        currency=currency,
        margins=margins,
        values=values,
        computes=computes,
    ).build()
