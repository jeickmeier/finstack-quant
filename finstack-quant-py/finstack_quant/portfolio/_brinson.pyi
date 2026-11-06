"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/brinson.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd

class BrinsonPeriodResult:
    """
    Single-period Brinson-Fachler attribution result.

    Returned by :func:`brinson_fachler`. Sector effects preserve the input
    sector order; the three effect totals sum to
    :attr:`total_excess_return`.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import brinson_fachler
    >>> sectors = [
    ...     {
    ...         "sector": "A",
    ...         "portfolio_weight": 1.0,
    ...         "benchmark_weight": 1.0,
    ...         "portfolio_return": 0.02,
    ...         "benchmark_return": 0.01,
    ...     }
    ... ]
    >>> brinson_fachler(json.dumps(sectors)).total_excess_return
    0.01
    """

    @property
    def sectors(self) -> list[dict[str, object]]:
        """
        Per-sector effects as records, in the order supplied.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_allocation(self) -> float:
        """
        Sum of allocation effects across sectors.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_selection(self) -> float:
        """
        Sum of selection effects across sectors.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_interaction(self) -> float:
        """
        Sum of interaction effects across sectors.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def portfolio_return(self) -> float:
        """
        Portfolio total return for the period.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def benchmark_return(self) -> float:
        """
        Benchmark total return for the period.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_excess_return(self) -> float:
        """
        Active return; equals the sum of the three effect totals.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-sector effects as a pandas DataFrame.

        Columns: ``sector``, ``allocation``, ``selection``, ``interaction``, ``total``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> BrinsonPeriodResult:
        """
        Deserialize a ``BrinsonPeriodResult`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        BrinsonPeriodResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``BrinsonPeriodResult`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import BrinsonPeriodResult
        >>> doc = '{"sectors": [], "total_allocation": 0.0, "total_selection": 0.0, "total_interaction": 0.0, "portfolio_return": 0.0, "benchmark_return": 0.0, "total_excess_return": 0.0}'
        >>> BrinsonPeriodResult.from_json(doc).total_excess_return
        0.0
        """
        ...

class CarinoLinkedAttribution:
    """
    Carino-linked multi-period Brinson attribution result.

    Returned by :func:`carino_link`. The linked per-sector effects sum to the
    geometrically compounded active return exactly.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import brinson_fachler, carino_link
    >>> period = [
    ...     {
    ...         "sector": "A",
    ...         "portfolio_weight": 1.0,
    ...         "benchmark_weight": 1.0,
    ...         "portfolio_return": 0.02,
    ...         "benchmark_return": 0.01,
    ...     }
    ... ]
    >>> result = carino_link([brinson_fachler(json.dumps(p)) for p in [period, period]])
    >>> round(result.linked_selection, 4)
    0.0203
    """

    @property
    def periods(self) -> list[dict[str, object]]:
        """
        Per-period decompositions as records, in chronological order.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def portfolio_return_compounded(self) -> float:
        """
        Geometrically compounded portfolio return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def benchmark_return_compounded(self) -> float:
        """
        Geometrically compounded benchmark return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_sectors(self) -> list[dict[str, object]]:
        """
        Per-sector Carino-smoothed effects summed across periods.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_allocation(self) -> float:
        """
        Sum of per-sector linked allocation effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_selection(self) -> float:
        """
        Sum of per-sector linked selection effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_interaction(self) -> float:
        """
        Sum of per-sector linked interaction effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Linked per-sector effects as a pandas DataFrame.

        Columns: ``sector``, ``allocation``, ``selection``, ``interaction``, ``total``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> CarinoLinkedAttribution:
        """
        Deserialize a ``CarinoLinkedAttribution`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        CarinoLinkedAttribution
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``CarinoLinkedAttribution`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import CarinoLinkedAttribution
        >>> doc = '{"periods": [], "portfolio_return_compounded": 0.0, "benchmark_return_compounded": 0.0, "linked_sectors": [], "linked_allocation": 0.0, "linked_selection": 0.0, "linked_interaction": 0.0}'
        >>> CarinoLinkedAttribution.from_json(doc).linked_allocation
        0.0
        """
        ...

def brinson_fachler(sectors_json: str | dict[str, Any] | list[Any] | pd.DataFrame) -> BrinsonPeriodResult:
    """
    Compute single-period Brinson-Fachler attribution from sector JSON.

    Parameters
    ----------
    sectors_json : str | dict | list | pandas.DataFrame
        JSON array of ``SectorPeriod`` objects with ``sector``,
        ``portfolio_weight``, ``benchmark_weight``, ``portfolio_return``, and
        ``benchmark_return`` fields. Returns are simple decimal returns for the
        period (e.g. ``0.02`` for +2%).

    Returns
    -------
    BrinsonPeriodResult
        Typed result with per-sector effects, effect totals, and
        ``to_dataframe()`` / ``to_json()`` exits; use
        serialize with ``to_json()`` for the wire string.

    Raises
    ------
    PortfolioError
        If sector weights do not sum to one or returns are invalid.
    ValueError
        If ``sectors_json`` is malformed.

    Sources
    -------
    See ``docs/REFERENCES.md#brinson-fachler-1985``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import brinson_fachler
    >>> sectors = [
    ...     {
    ...         "sector": "A",
    ...         "portfolio_weight": 1.0,
    ...         "benchmark_weight": 1.0,
    ...         "portfolio_return": 0.02,
    ...         "benchmark_return": 0.01,
    ...     }
    ... ]
    >>> brinson_fachler(json.dumps(sectors)).total_excess_return
    0.01
    """
    ...

def carino_link(periods_json: str | dict[str, Any] | list[Any] | pd.DataFrame) -> CarinoLinkedAttribution:
    """
    Carino-link already-computed single-period Brinson-Fachler results.

    Binds Rust ``carino_link``, like :func:`campisi_carino_link` and
    :func:`grid_carino_link`. To link raw sector inputs, attribute each period
    first: ``carino_link([brinson_fachler(p) for p in periods])``. The same
    input the WASM ``carinoLink`` takes.

    Parameters
    ----------
    periods_json : str | dict | list | pandas.DataFrame
        Chronological list of typed ``BrinsonPeriodResult`` objects (as
        returned by :func:`brinson_fachler`) or their JSON form (for example
        ``json.loads(brinson_fachler(...).to_json())`` per period) with identical
        sector ordering in every period.

    Returns
    -------
    CarinoLinkedAttribution
        Typed result with linked per-sector effects and compounded returns;
        use serialize with ``to_json()`` for the wire string.

    Raises
    ------
    PortfolioError
        If the sequence is empty or changes sector ordering, or a period
        return is non-finite or at most ``-1``.
    ValueError
        If ``periods_json`` is malformed.

    Sources
    -------
    See ``docs/REFERENCES.md#carino-1999``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import brinson_fachler, carino_link
    >>> period = [
    ...     {
    ...         "sector": "A",
    ...         "portfolio_weight": 1.0,
    ...         "benchmark_weight": 1.0,
    ...         "portfolio_return": 0.02,
    ...         "benchmark_return": 0.01,
    ...     }
    ... ]
    >>> result = json.loads(brinson_fachler(json.dumps(period)).to_json())
    >>> linked = carino_link(json.dumps([result, result]))
    >>> round(linked.linked_selection, 4)
    0.0203
    """
    ...

def brinson_linked(periods: str | list[list[dict[str, Any]]]) -> CarinoLinkedAttribution:
    """
    Attribute each period with Brinson-Fachler and Carino-link the results.

    Binds Rust ``brinson_linked``: the multi-period Brinson-Fachler report in
    one call, identical to ``carino_link([brinson_fachler(p) for p in periods])``.
    The same input the WASM ``brinsonLinked`` takes.

    Parameters
    ----------
    periods : str | list[list[dict]]
        Chronological periods, each a list of ``SectorPeriod`` objects
        (``sector``, ``portfolio_weight``, ``benchmark_weight``,
        ``portfolio_return``, ``benchmark_return``), or the JSON of that
        nested list. Weights are decimal fractions summing to one on each
        side; returns are decimal period returns (``0.015`` is +1.5 %).
        Every period lists the same sectors in the same order; a sector
        absent on one side keeps a zero weight.

    Returns
    -------
    CarinoLinkedAttribution
        Per-period sector allocation, selection and interaction
        (``periods``), linked effects by sector (``linked_sectors``) and by
        effect (``linked_allocation``, ``linked_selection``,
        ``linked_interaction``), and the compounded portfolio and benchmark
        returns. The linked effects sum to the compounded active return.

    Raises
    ------
    PortfolioError
        If there are no periods, a period fails Brinson-Fachler (no sectors,
        a non-finite input, or weights not summing to one; the message names
        the zero-based period), the sector ordering changes between periods,
        or a period return is at most ``-1``.
    ValueError
        If ``periods`` is malformed JSON or a row has an unknown field.

    Sources
    -------
    See ``docs/REFERENCES.md#brinson-fachler-1985`` and
    ``docs/REFERENCES.md#carino-1999``.

    Examples
    --------
    >>> from finstack_quant.portfolio import brinson_linked
    >>> def row(sector, wp, wb, rp, rb):
    ...     return {
    ...         "sector": sector,
    ...         "portfolio_weight": wp,
    ...         "benchmark_weight": wb,
    ...         "portfolio_return": rp,
    ...         "benchmark_return": rb,
    ...     }
    >>> periods = [
    ...     [row("Tech", 0.7, 0.5, 0.10, 0.06), row("Energy", 0.3, 0.5, 0.04, 0.05)],
    ...     [row("Tech", 0.6, 0.5, 0.02, 0.03), row("Energy", 0.4, 0.5, -0.01, 0.00)],
    ... ]
    >>> report = brinson_linked(periods)
    >>> active = report.portfolio_return_compounded - report.benchmark_return_compounded
    >>> linked = report.linked_allocation + report.linked_selection + report.linked_interaction
    >>> abs(linked - active) < 1e-12
    True
    >>> [s["sector"] for s in report.linked_sectors]
    ['Tech', 'Energy']
    """
    ...
