"""Native carry and breakeven ladder declarations."""

from __future__ import annotations

from collections.abc import Sequence
from datetime import date
from typing import Any, Literal, TypedDict

import pandas as pd
import polars as pl

from finstack_quant.core.dates import Tenor
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.table import ArrowTable
from finstack_quant.valuations.instruments._instruments.bond import Bond

class BondBreakevenConfig(TypedDict):
    """Select the native bond breakeven target and solve convention.

    Examples
    --------
    >>> config = {"target": "z_spread", "mode": "linear"}
    >>> config["target"]
    'z_spread'
    """

    target: Literal["z_spread", "ytm", "oas"]
    mode: Literal["linear", "iterative"]

class BondBreakevenRow:
    """Carry and signed breakeven for one requested calendar tenor.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import BondBreakevenRow
    >>> hasattr(BondBreakevenRow, "horizon")
    True
    """

    @property
    def horizon(self) -> Tenor:
        """Read horizon from the native result.

        Returns
        -------
        Tenor
            Calendar tenor selected by the caller.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def carry(self) -> float:
        """Read carry from the native result.

        Returns
        -------
        float
            Carry and roll-down in native bond currency.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def sensitivity(self) -> float:
        """Read sensitivity from the native result.

        Returns
        -------
        float
            Signed change in native currency per basis point at the base date.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def breakeven_bp(self) -> float:
        """Read breakeven bp from the native result.

        Returns
        -------
        float
            Signed basis-point move that offsets carry.

        Notes
        -----
        This accessor does not raise.
        """
        ...

class BondBreakevenLadder:
    """Ordered bond carry and breakeven rows for one market snapshot.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import BondBreakevenLadder
    >>> hasattr(BondBreakevenLadder, "as_of")
    True
    """

    @property
    def as_of(self) -> date:
        """Read as of from the native result.

        Returns
        -------
        date
            Valuation date anchoring each horizon.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def instrument_id(self) -> str:
        """Read instrument id from the native result.

        Returns
        -------
        str
            Identifier of the priced bond.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def currency(self) -> str:
        """Read currency from the native result.

        Returns
        -------
        str
            ISO currency of carry and sensitivity.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def rows(self) -> list[BondBreakevenRow]:
        """Read rows from the native result.

        Returns
        -------
        list[BondBreakevenRow]
            Independent rows in input tenor order.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """Present the requested horizons in an :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Ordered ``horizon``, ``carry``, ``sensitivity``, ``breakeven_bp``, and ``currency`` columns.

        Raises
        ------
        ValueError
            If the native table cannot be represented as Arrow.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """Present the requested horizons in an independent pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Ordered ``horizon``, ``carry``, ``sensitivity``, ``breakeven_bp``, and ``currency`` columns.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        ValueError
            If the native table cannot be represented by pandas.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """Present the requested horizons in a Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Ordered ``horizon``, ``carry``, ``sensitivity``, ``breakeven_bp``, and ``currency`` columns.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        ValueError
            If the native table cannot be represented by Polars.
        """
        ...

    def to_pyarrow(self) -> Any:
        """Present the requested horizons in a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Ordered ``horizon``, ``carry``, ``sensitivity``, ``breakeven_bp``, and ``currency`` columns.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        ValueError
            If the native table cannot be represented by PyArrow.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...

def bond_breakeven_ladder(
    bond: Bond,
    market: MarketContext | str,
    as_of: date | str,
    horizons: Sequence[Tenor | str],
    config: BondBreakevenConfig,
) -> BondBreakevenLadder:
    """Measure carry and signed basis-point breakeven for each horizon.

    Parameters
    ----------
    bond : Bond
        Typed bond whose native pricing conventions and cashflows are used.
    market : MarketContext or str
        Market snapshot containing the pricing curves and risk inputs.
    as_of : date or str
        Valuation date or ISO date string anchoring the horizons.
    horizons : Sequence[Tenor or str]
        Nonempty ordered positive calendar tenors, such as "1M" and "1Y".
    config : BondBreakevenConfig
        Select yield, Z-spread for a vanilla bond, or linear OAS for an
        option-bearing bond. Spread targets require instrument spread CS01.

    Returns
    -------
    BondBreakevenLadder
        Native carry and sensitivity with signed basis-point shifts.

    Raises
    ------
    ValueError
        If horizons, config, pricing or requested sensitivities are invalid.
    KeyError
        If required market data are absent.

    Examples
    --------
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.valuations.instruments import Bond, bond_breakeven_ladder
    >>> bond = Bond.example()
    >>> market = MarketContext().insert(
    ...     DiscountCurve(
    ...         bond.discount_curve_id,
    ...         bond.issue_date,
    ...         [(0.0, 1.0), (10.0, 0.70)],
    ...         interp="log_linear",
    ...         extrapolation="extend",
    ...         day_count="act_365f",
    ...     )
    ... )
    >>> ladder = bond_breakeven_ladder(
    ...     bond,
    ...     market,
    ...     bond.issue_date,
    ...     ["3M"],
    ...     {"target": "z_spread", "mode": "linear"},
    ... )
    >>> ladder.to_pandas()["horizon"].tolist()
    ['3M']
    """
    ...
