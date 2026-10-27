"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/fi_attribution.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
from finstack_quant.portfolio._attribution import ReconciliationReport

class FiAttributionResult:
    """
    Single-period Campisi benchmark-relative attribution result.

    Returned by :func:`campisi_attribution`. The five effect totals sum to
    :attr:`active_return`; sector effects preserve first-seen order.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import campisi_attribution
    >>> snap = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.02,
    ...         "yield_annual": 0.04,
    ...         "modified_duration": 5.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> result = campisi_attribution(json.dumps(snap), json.dumps(snap), '{"period_years":0.25}')
    >>> round(result.active_return, 6)
    0.0
    """

    @property
    def sectors(self) -> list[dict[str, object]]:
        """
        Per-sector benchmark-relative effects as records.

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
    def portfolio_components(self) -> dict[str, object]:
        """
        Portfolio-side absolute Campisi split (carry, treasury, spread, selection, total).

        Returns
        -------
        dict[str, object]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def benchmark_components(self) -> dict[str, object]:
        """
        Benchmark-side absolute Campisi split (carry, treasury, spread, selection, total).

        Returns
        -------
        dict[str, object]
            JSON-shaped view of the canonical Rust field.

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
    def total_allocation(self) -> float:
        """
        Sum of sector allocation effects.

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
    def total_active_carry(self) -> float:
        """
        Sum of sector active carry effects.

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
    def total_active_treasury(self) -> float:
        """
        Sum of sector active treasury effects.

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
    def total_active_spread(self) -> float:
        """
        Sum of sector active spread effects.

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
        Sum of sector selection effects.

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

        Columns: ``sector``, ``portfolio_weight``, ``benchmark_weight``, ``portfolio_return``, ``benchmark_return``, ``allocation``, ``active_carry``, ``active_treasury``, ``active_spread``, ``selection``, ``total_active``. The schema is pinned, so an empty result keeps the
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

    def reconciliation_check(self, tolerance: float) -> ReconciliationReport:
        """
        Reconcile the five Campisi effect totals against the active return.

        Binds the Rust method
        ``finstack_quant_portfolio::FiAttributionResult::reconciliation_check``.
        For a result held as JSON, rebuild it first with
        :meth:`FiAttributionResult.from_json`.

        Parameters
        ----------
        tolerance : float
            Absolute tolerance in return units (``1e-10`` is appropriate for
            return-space values).

        Returns
        -------
        ReconciliationReport
            Typed report with ``total_residual``, ``is_reconciled`` and
            ``tolerance`` (residual and tolerance in return units).

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.portfolio import campisi_attribution
        >>> snap = [
        ...     {
        ...         "sector": "GOVT",
        ...         "weight": 1.0,
        ...         "total_return": 0.02,
        ...         "yield_annual": 0.04,
        ...         "modified_duration": 5.0,
        ...         "spread_duration": 0.0,
        ...         "spread": 0.0,
        ...         "delta_treasury_yield": -0.001,
        ...         "delta_spread": 0.0,
        ...     }
        ... ]
        >>> result = campisi_attribution(json.dumps(snap), json.dumps(snap), '{"period_years":0.25}')
        >>> result.reconciliation_check(1e-10).is_reconciled
        True
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
    def from_json(json: str) -> FiAttributionResult:
        """
        Deserialize a ``FiAttributionResult`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        FiAttributionResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``FiAttributionResult`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import FiAttributionResult
        >>> doc = '{"sectors": [], "portfolio_components": {"carry": 0.0, "treasury": 0.0, "spread": 0.0, "selection": 0.0, "total": 0.0}, "benchmark_components": {"carry": 0.0, "treasury": 0.0, "spread": 0.0, "selection": 0.0, "total": 0.0}, "portfolio_return": 0.0, "benchmark_return": 0.0, "active_return": 0.0, "total_allocation": 0.0, "total_active_carry": 0.0, "total_active_treasury": 0.0, "total_active_spread": 0.0, "total_selection": 0.0}'
        >>> FiAttributionResult.from_json(doc).active_return
        0.0
        """
        ...

class FiCarinoLinkedResult:
    """
    Multi-period Carino-linked Campisi attribution result.

    Returned by :func:`campisi_carino_link`. The five linked totals sum to
    the geometrically compounded active return.

    Examples
    --------
    >>> from finstack_quant.portfolio import FiCarinoLinkedResult
    >>> doc = (
    ...     '{"periods": [], "portfolio_return_compounded": 0.0404,'
    ...     ' "benchmark_return_compounded": 0.0201, "linked_sectors": [],'
    ...     ' "linked_allocation": 0.0, "linked_active_carry": 0.0,'
    ...     ' "linked_active_treasury": 0.0, "linked_active_spread": 0.0,'
    ...     ' "linked_selection": 0.0}'
    ... )
    >>> FiCarinoLinkedResult.from_json(doc).portfolio_return_compounded
    0.0404
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
    def linked_sectors(self) -> list[dict[str, object]]:
        """
        Per-sector linked effects as records.

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
        Sum of linked allocation effects.

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
    def linked_active_carry(self) -> float:
        """
        Sum of linked active carry effects.

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
    def linked_active_treasury(self) -> float:
        """
        Sum of linked active treasury effects.

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
    def linked_active_spread(self) -> float:
        """
        Sum of linked active spread effects.

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
        Sum of linked selection effects.

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

        Columns: ``sector``, ``allocation``, ``active_carry``, ``active_treasury``, ``active_spread``, ``selection``, ``total_active``. The schema is pinned, so an empty result keeps the
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
    def from_json(json: str) -> FiCarinoLinkedResult:
        """
        Deserialize a ``FiCarinoLinkedResult`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        FiCarinoLinkedResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``FiCarinoLinkedResult`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import FiCarinoLinkedResult
        >>> doc = '{"periods": [], "portfolio_return_compounded": 0.0, "benchmark_return_compounded": 0.0, "linked_sectors": [], "linked_allocation": 0.0, "linked_active_carry": 0.0, "linked_active_treasury": 0.0, "linked_active_spread": 0.0, "linked_selection": 0.0}'
        >>> FiCarinoLinkedResult.from_json(doc).linked_selection
        0.0
        """
        ...

def campisi_attribution(
    portfolio_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    benchmark_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    config_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
) -> FiAttributionResult:
    """
    Compute single-period Campisi fixed-income benchmark attribution.

    Binds Rust ``finstack_quant_portfolio::campisi_attribution``.

    Parameters
    ----------
    portfolio_json : str | dict | list | pandas.DataFrame
        JSON array of ``FiPositionSnapshot`` objects with ``sector``,
        ``weight``, ``total_return``, ``yield_annual``, ``modified_duration``,
        ``spread_duration``, ``spread``, ``delta_treasury_yield``, and
        ``delta_spread`` fields (decimals; durations in years). Unknown fields
        are rejected and weights must sum to one. ``spread_duration`` must be
        the canonical quote-reproducing Z-spread duration; ``spread`` and
        ``delta_spread`` must use the matching ``z_spread`` basis. OAS,
        G-spread, and discount-margin values are incompatible. The direct JSON
        shape has no metric-ID provenance, so this binding cannot detect a
        mislabeled numeric spread basis.
    benchmark_json : str | dict | list | pandas.DataFrame
        JSON array of ``FiPositionSnapshot`` objects for the benchmark, subject
        to the same quote-reproducing Z-spread basis contract.
    config_json : str | dict | list | pandas.DataFrame
        JSON object whose only field is ``period_years`` (e.g. ``0.25``). It is
        required — there is no default — and unknown keys are rejected. The
        spread convention is not configurable: the Campisi spread effect
        ``-SD * delta_spread`` is identical under the absolute and
        Duration-Times-Spread conventions, so a mode switch could only relabel
        an identical result.

    Returns
    -------
    FiAttributionResult
        Typed result with per-sector effects, the five effect totals, and
        ``to_dataframe()`` / ``to_json()`` exits; use
        serialize with ``to_json()`` for the wire string.

    Raises
    ------
    PortfolioError
        If weights do not sum to one, inputs are non-finite, ``period_years``
        is not finite and positive, either side is empty, or a sector present
        on either side has ``|net sector weight| <= 1e-6 * gross absolute
        sector weight``. The spread *level* is unconstrained: zero and negative
        Z-spreads are accepted, because the spread effect never divides by the
        level. Spread-basis provenance cannot be validated from the numeric JSON
        shape.
    ValueError
        If any JSON argument is malformed.

    Sources
    -------
    See ``docs/REFERENCES.md#campisi-2000``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import campisi_attribution
    >>> portfolio = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.02,
    ...         "yield_annual": 0.04,
    ...         "modified_duration": 5.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> benchmark = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.01,
    ...         "yield_annual": 0.03,
    ...         "modified_duration": 4.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> result = campisi_attribution(json.dumps(portfolio), json.dumps(benchmark), '{"period_years":0.25}')
    >>> round(result.active_return, 4)
    0.01
    """
    ...

def campisi_carino_link(periods_json: str | dict[str, Any] | list[Any] | pd.DataFrame) -> FiCarinoLinkedResult:
    """
    Carino-link already-computed single-period Campisi attribution results.

    Binds Rust ``finstack_quant_portfolio::campisi_carino_link``.

    Each period result already carries its own applied ``period_years``, so
    periods of *different* lengths link correctly here (e.g. act/365 calendar
    months: 31/365, 28/365, 31/365). Attribute each period with
    :func:`campisi_attribution` first:
    ``campisi_carino_link([campisi_attribution(p, b, c) for p, b, c in periods])``.

    Parameters
    ----------
    periods_json : list[FiAttributionResult] | str | list[dict]
        Chronological list of typed ``FiAttributionResult`` objects (as
        returned by :func:`campisi_attribution`) or their JSON form
        (``FiAttributionResult.to_json()``). Every period must carry the same
        sector ordering. Unknown fields are rejected.

    Returns
    -------
    FiCarinoLinkedResult
        Typed result whose five linked totals sum to the geometrically
        compounded active return; use :func:`campisi_carino_link` for
        the raw wire string.

    Raises
    ------
    PortfolioError
        If ``periods_json`` is empty; sector orderings differ; a consumed
        top-level return/effect, per-sector linked effect, or sector
        ``total_active`` is non-finite; ``active_return`` disagrees with the
        portfolio-minus-benchmark return; a sector ``total_active`` disagrees
        with its five effects; sector effects do not reconcile to their
        declared top-level totals; the five totals do not reconcile to
        ``active_return`` within the overflow-safe scaled-L1 tolerance; a
        reconciliation residual is non-finite; or a return is at or below
        -100% (outside the Carino domain).
    ValueError
        If ``periods_json`` is malformed or does not match the
        ``FiAttributionResult`` schema.

    Sources
    -------
    See ``docs/REFERENCES.md#carino-1999``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import campisi_attribution, campisi_carino_link
    >>> portfolio = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.02,
    ...         "yield_annual": 0.04,
    ...         "modified_duration": 5.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> benchmark = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.01,
    ...         "yield_annual": 0.03,
    ...         "modified_duration": 4.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> period = campisi_attribution(json.dumps(portfolio), json.dumps(benchmark), '{"period_years":0.25}')
    >>> linked = campisi_carino_link([period, period])
    >>> round(linked.linked_selection, 6)
    0.013195
    """
    ...
