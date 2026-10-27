"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/grid_attribution.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd

class GridAttributionResult:
    """
    Single-period hierarchical duration-cell x sector grid attribution result.

    Returned by :func:`grid_attribution`. ``total_curve``, ``total_sector``
    and ``total_selection`` sum to :attr:`active_return` to floating-point
    precision for well-conditioned inputs.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import grid_attribution
    >>> portfolio = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.02}]
    >>> benchmark = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.01}]
    >>> result = grid_attribution(json.dumps(portfolio), json.dumps(benchmark))
    >>> round(result.total_selection, 4)
    0.01
    """

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
    def active_return(self) -> float:
        """
        Active return; portfolio minus benchmark total return.

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
    def curve_effects(self) -> list[dict[str, object]]:
        """
        Per-cell curve effects as records, in first-appearance order.

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
    def sector_effects(self) -> list[dict[str, object]]:
        """
        Per-(cell, sector) allocation effects as records.

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
    def total_curve(self) -> float:
        """
        Sum of the per-cell curve effects.

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
    def total_sector(self) -> float:
        """
        Sum of the sector allocation effects.

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
        Sum of the selection effects.

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
        Per-cell curve effects as a pandas DataFrame.

        Columns: ``cell``, ``portfolio_weight``, ``benchmark_weight``, ``benchmark_cell_return``, ``curve_effect``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.
        The primary frame is the duration-cell axis; the two (cell,
        sector) tables come from :meth:`to_sector_effects_dataframe` and
        the ``selection_effect`` column of :meth:`to_sector_effects_dataframe`.

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

    def to_sector_effects_dataframe(self) -> pd.DataFrame:
        """
        Per-(cell, sector) allocation effects as a pandas DataFrame.

        Columns: ``cell``, ``sector``, ``allocation_effect``, ``selection_effect``. The schema is pinned, so an empty result keeps the
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
    def from_json(json: str) -> GridAttributionResult:
        """
        Deserialize a ``GridAttributionResult`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        GridAttributionResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``GridAttributionResult`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import GridAttributionResult
        >>> doc = '{"portfolio_return": 0.0, "benchmark_return": 0.0, "active_return": 0.0, "curve_effects": [], "sector_effects": [], "total_curve": 0.0, "total_sector": 0.0, "total_selection": 0.0}'
        >>> GridAttributionResult.from_json(doc).active_return
        0.0
        """
        ...

class GridCarinoLinkedResult:
    """
    Multi-period Carino-linked hierarchical grid attribution result.

    Returned by :func:`grid_carino_link`. Only the three top-level effects
    are linked; their sum reconstructs the compounded active return exactly.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import grid_attribution, grid_carino_link
    >>> portfolio = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.02}]
    >>> benchmark = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.01}]
    >>> period = json.loads(grid_attribution(json.dumps(portfolio), json.dumps(benchmark)).to_json())
    >>> result = grid_carino_link(json.dumps([period, period]))
    >>> round(result.linked_selection, 4)
    0.0203
    """

    @property
    def periods(self) -> list[dict[str, object]]:
        """
        Per-period single-period results as records, in chronological order.

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
    def linked_curve(self) -> float:
        """
        Sum of per-period Carino-scaled curve effects.

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
    def linked_sector(self) -> float:
        """
        Sum of per-period Carino-scaled sector allocation effects.

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
        Sum of per-period Carino-scaled selection effects.

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
        Single-row pandas DataFrame view of the linked totals.

        Columns: ``portfolio_return_compounded``, ``benchmark_return_compounded``, ``linked_curve``, ``linked_sector``, ``linked_selection``. The schema is pinned, so an empty result keeps the
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
    def from_json(json: str) -> GridCarinoLinkedResult:
        """
        Deserialize a ``GridCarinoLinkedResult`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        GridCarinoLinkedResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``GridCarinoLinkedResult`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import GridCarinoLinkedResult
        >>> doc = '{"periods": [], "portfolio_return_compounded": 0.0, "benchmark_return_compounded": 0.0, "linked_curve": 0.0, "linked_sector": 0.0, "linked_selection": 0.0}'
        >>> GridCarinoLinkedResult.from_json(doc).linked_curve
        0.0
        """
        ...

def grid_attribution(
    portfolio_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    benchmark_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
) -> GridAttributionResult:
    """
    Compute a single-period hierarchical duration-cell x sector grid attribution.

    Binds Rust ``finstack_quant_portfolio::grid_attribution`` (Dynkin, Hyman
    & Vankudre 1998, Appendix A): decomposes active return into a per-cell
    curve (positioning) effect, a within-cell sector allocation effect, and
    a security-selection residual per (cell, sector).

    Parameters
    ----------
    portfolio_json : str | dict | list | pandas.DataFrame
        JSON array of ``GridPosition`` objects (``cell``, ``sector``,
        ``weight``, ``total_return``) for the portfolio side; weights must
        sum to ``1.0`` within ``1e-6``.
    benchmark_json : str | dict | list | pandas.DataFrame
        JSON array of ``GridPosition`` objects for the benchmark side; same
        weight-sum requirement.

    Returns
    -------
    GridAttributionResult
        Typed result whose ``total_curve``,
        ``total_sector`` and ``total_selection`` sum to ``active_return`` to
        floating-point precision for well-conditioned inputs; among
        *accepted* inputs (those that clear the near-zero-net-weight check
        below), the reconciliation residual grows the closer any bucket's
        net weight sits to that check's own rejection boundary — see the
        Rust module docs' measured residuals for magnitudes.

    Raises
    ------
    PortfolioError
        If any weight or return is non-finite, either side's weights don't
        sum to ``1.0`` within tolerance, or a (cell) or (cell, sector)
        bucket has positions on a side but nets to a weight that is zero, or
        numerically near zero (within ``1e-6`` relative to its own gross
        weight), which is undefined-or-explosive to attribute (the error
        names the bucket and the side).
    ValueError
        If either JSON argument is malformed or carries unknown fields.

    Sources
    -------
    See ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import grid_attribution
    >>> portfolio = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.02}]
    >>> benchmark = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.01}]
    >>> result = grid_attribution(json.dumps(portfolio), json.dumps(benchmark))
    >>> round(result.total_selection, 4)
    0.01
    """
    ...

def grid_carino_link(periods_json: str | dict[str, Any] | list[Any] | pd.DataFrame) -> GridCarinoLinkedResult:
    """
    Carino-link multi-period hierarchical grid attribution results.

    Binds Rust ``finstack_quant_portfolio::grid_carino_link`` (Carino 1999):
    applies Carino smoothing to a chronological sequence of single-period
    :func:`grid_attribution` results so the three top-level effects
    (``linked_curve``, ``linked_sector``, ``linked_selection``) sum exactly
    to the geometrically compounded active return. Only the three top-level
    effects are linked; per-cell / per-(cell, sector) multi-period linking
    is out of scope.

    Parameters
    ----------
    periods_json : str | dict | list | pandas.DataFrame
        Chronological list of typed ``GridAttributionResult`` objects (as
        returned by :func:`grid_attribution`) or their JSON form, each the
        wire output of :func:`grid_attribution` (or
        ``GridAttributionResult.to_json()``).

    Returns
    -------
    GridCarinoLinkedResult
        Typed result with the three linked effects and compounded returns;
        use serialize with ``to_json()`` for the wire string.

    Raises
    ------
    PortfolioError
        If ``periods_json`` is empty; any consumed return or top-level effect
        is non-finite; ``active_return`` disagrees with the portfolio-minus-
        benchmark return; the three effect totals do not reconcile to
        ``active_return`` within an overflow-safe scaled-L1 tolerance; a
        return-identity or reconciliation residual is non-finite; or any
        per-period or compounded return is at or below -100% (outside the
        Carino domain).
    ValueError
        If ``periods_json`` is malformed or does not match the
        ``GridAttributionResult`` schema.

    Sources
    -------
    See ``docs/REFERENCES.md#carino-1999`` and
    ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import grid_attribution, grid_carino_link
    >>> portfolio = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.02}]
    >>> benchmark = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.01}]
    >>> period = json.loads(grid_attribution(json.dumps(portfolio), json.dumps(benchmark)).to_json())
    >>> result = grid_carino_link(json.dumps([period, period]))
    >>> round(result.linked_selection, 4)
    0.0203
    """
    ...
