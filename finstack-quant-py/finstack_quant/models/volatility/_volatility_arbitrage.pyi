"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/volatility_arbitrage.rs``."""

from __future__ import annotations

from typing import Any, Literal, TypedDict
import pandas as pd

_ArbitrageTypeName = Literal[
    "butterfly",
    "calendar_spread",
    "local_vol_density",
    "svi_moment_bound",
    "svi_butterfly_condition",
    "svi_calendar_spread",
]

class _ViolationLocation(TypedDict):
    strike: float
    expiry: float
    adjacent_expiry: float | None

class _ArbitrageViolation(TypedDict):
    """Serde form of the Rust ``ArbitrageViolation``."""

    violation_type: _ArbitrageTypeName
    location: _ViolationLocation
    severity: Literal["negligible", "minor", "major", "critical"]
    magnitude: float
    description: str
    suggested_fix: float | None

class ArbitrageReport:
    """
    Aggregated model-free arbitrage report for a volatility grid.

    Returned by :func:`check_surface_grid`. Picklable and JSON round-trippable;
    ``to_dataframe()`` gives one row per violation.

    Examples
    --------
    >>> from finstack_quant.models.volatility import check_surface_grid
    >>> strikes, expiries = [90.0, 100.0, 110.0], [1.0, 2.0]
    >>> vols, forwards = [[0.2, 0.2, 0.2], [0.2, 0.2, 0.2]], [100.0, 100.0]
    >>> report = check_surface_grid(strikes, expiries, vols, forwards)
    >>> (report.passed, report.total_violations)
    (True, 0)
    """

    @property
    def vol_surface_id(self) -> str:
        """
        Identifier of the checked surface (``"grid"`` for array inputs).

        Returns
        -------
        str
            The surface identifier carried through from the input, or the literal ``"grid"`` when the check ran on raw arrays.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def passed(self) -> bool:
        """
        ``True`` when no violation above ``negligible`` severity was found.

        Returns
        -------
        bool
            ``True`` if every arbitrage check passed or produced only negligible-severity violations, ``False`` otherwise.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_violations(self) -> int:
        """
        Number of violations of any severity.

        Returns
        -------
        int
            Count of all violation rows, including negligible ones; equals ``len(violations)``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def violations(self) -> list[_ArbitrageViolation]:
        """
        Violation rows as serde dicts, critical first.

        Returns
        -------
        list[_ArbitrageViolation]
            One dict per detected violation, ordered by descending severity; empty when the surface is arbitrage-free.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_severity(self) -> dict[Literal["negligible", "minor", "major", "critical"], int]:
        """
        Violation counts keyed by severity name; only severities that occur.

        Returns
        -------
        dict[Literal["negligible", "minor", "major", "critical"], int]
            Counts per severity label (the Rust ``counts_by_severity`` map); a severity with no violation is absent, so a clean surface gives ``{}``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_type(self) -> dict[_ArbitrageTypeName, int]:
        """
        Violation counts keyed by check type; only check types that occur.

        Returns
        -------
        dict[_ArbitrageTypeName, int]
            Counts per ``ArbitrageType`` name (the Rust ``counts_by_type`` map); a clean check type is absent, so a clean surface gives ``{}``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per violation.

        Returns
        -------
        pandas.DataFrame
            Columns ``violation_type``, ``severity``, ``strike``, ``expiry``,
            ``adjacent_expiry``, ``magnitude``, ``suggested_fix``,
            ``description``; empty when the surface passed. Does not raise.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON (the Rust ``ArbitrageReport`` wire form).

        Returns
        -------
        str
            JSON document. Does not raise.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ArbitrageReport:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Serialized report.

        Returns
        -------
        ArbitrageReport
            Reconstructed report.

        Raises
        ------
        ValueError
            On malformed JSON.

        Examples
        --------
        >>> from finstack_quant.models.volatility import ArbitrageReport, check_surface_grid
        >>> r = check_surface_grid([90.0, 100.0, 110.0], [1.0, 2.0], [[0.2] * 3, [0.2] * 3], [100.0])
        >>> ArbitrageReport.from_json(r.to_json()).passed
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct this value for ``pickle`` through its canonical JSON form.

        Returns
        -------
        tuple[Any, tuple[str]]
            The bound ``from_json`` reconstruction callable and its JSON payload.

        Raises
        ------
        ValueError
            If serialization fails or the JSON payload cannot reconstruct the
            stored value, including non-finite floats unsupported by its schema.
        """
        ...

def check_butterfly_grid(
    strikes: list[float],
    expiries: list[float],
    vols: list[list[float]],
    forward_prices: list[float],
    tolerance: float = 1e-10,
) -> list[_ArbitrageViolation]:
    """
    Check butterfly arbitrage via Durrleman's ``g(k)`` density condition.

    Parameters
    ----------
    strikes : list[float]
        Monotonically increasing strike grid.
    expiries : list[float]
        Monotonically increasing expiry grid in years.
    vols : list[list[float]]
        Implied volatilities shaped ``[n_expiries][n_strikes]`` (decimal, e.g.
        ``0.20`` for 20%).
    forward_prices : list[float]
        Forward prices per expiry, or a single value broadcast across expiries.
    tolerance : float, optional
        Tolerance in total-variance units. Default ``1e-10`` (Rust ``DEFAULT_ARBITRAGE_TOLERANCE``, the same default as every Rust arbitrage check); pass a looser value to screen noisy market quotes.

    Returns
    -------
    list[_ArbitrageViolation]
        One serde dict per violation with keys ``violation_type``,
        ``location`` (``strike``, ``expiry``, ``adjacent_expiry``),
        ``severity``, ``magnitude``, ``description``, ``suggested_fix``.

    Raises
    ------
    ValueError
        If grid dimensions are inconsistent or inputs are non-finite.

    Sources
    -------
    See ``docs/REFERENCES.md#dupire-1994`` for local-volatility density context.

    Examples
    --------
    >>> from finstack_quant.models.volatility import check_butterfly_grid
    >>> strikes, expiries = [90.0, 100.0, 110.0], [1.0, 2.0]
    >>> vols, forwards = [[0.2, 0.2, 0.2], [0.2, 0.2, 0.2]], [100.0, 100.0]
    >>> check_butterfly_grid(strikes, expiries, vols, forwards)
    []

    """
    ...

def check_calendar_spread_grid(
    strikes: list[float],
    expiries: list[float],
    vols: list[list[float]],
    forward_prices: list[float],
    tolerance: float = 1e-10,
) -> list[_ArbitrageViolation]:
    """
    Check calendar-spread arbitrage (total-variance monotonicity in log-moneyness).

    Parameters
    ----------
    strikes : list[float]
        Monotonically increasing strike grid.
    expiries : list[float]
        Monotonically increasing expiry grid in years.
    vols : list[list[float]]
        Implied vols shaped ``[n_expiries][n_strikes]``.
    forward_prices : list[float]
        Forward prices per expiry or one broadcast value.
    tolerance : float, optional
        Tolerance in total-variance units. Default ``1e-10`` (Rust ``DEFAULT_ARBITRAGE_TOLERANCE``, the same default as every Rust arbitrage check); pass a looser value to screen noisy market quotes.

    Returns
    -------
    list[_ArbitrageViolation]
        Violation dicts with the same schema as :func:`check_butterfly_grid`.

    Raises
    ------
    ValueError
        If grid dimensions are inconsistent or inputs are non-finite.

    Examples
    --------
    >>> from finstack_quant.models.volatility import check_calendar_spread_grid
    >>> strikes, expiries = [90.0, 100.0, 110.0], [1.0, 2.0]
    >>> vols, forwards = [[0.2, 0.2, 0.2], [0.2, 0.2, 0.2]], [100.0, 100.0]
    >>> check_calendar_spread_grid(strikes, expiries, vols, forwards)
    []

    """
    ...

def check_local_vol_density_grid(
    strikes: list[float],
    expiries: list[float],
    vols: list[list[float]],
    forward_prices: list[float],
) -> list[_ArbitrageViolation]:
    """
    Check Dupire local-volatility density positivity on the implied-vol grid.

    Parameters
    ----------
    strikes : list[float]
        Monotonically increasing strike grid.
    expiries : list[float]
        Monotonically increasing expiry grid in years.
    vols : list[list[float]]
        Implied vols shaped ``[n_expiries][n_strikes]``.
    forward_prices : list[float]
        One finite positive forward per expiry; no single-value broadcasting.

    Returns
    -------
    list[_ArbitrageViolation]
        Violation dicts with the same schema as :func:`check_butterfly_grid`.

    Raises
    ------
    ValueError
        If grid dimensions are inconsistent, inputs are non-finite, or forwards are non-positive.

    Sources
    -------
    See ``docs/REFERENCES.md#dupire-1994``.

    Examples
    --------
    >>> from finstack_quant.models.volatility import check_local_vol_density_grid
    >>> strikes, expiries = [90.0, 100.0, 110.0], [1.0, 2.0]
    >>> vols, forwards = [[0.2, 0.2, 0.2], [0.2, 0.2, 0.2]], [100.0, 100.0]
    >>> check_local_vol_density_grid(strikes, expiries, vols, forwards)
    []

    """
    ...

def check_surface_grid(
    strikes: list[float],
    expiries: list[float],
    vols: list[list[float]],
    forward_prices: list[float],
    tolerance: float = 1e-10,
) -> ArbitrageReport:
    """
    Run butterfly, calendar-spread, and local-vol density checks together.

    Parameters
    ----------
    strikes : list[float]
        Monotonically increasing strike grid.
    expiries : list[float]
        Monotonically increasing expiry grid in years.
    vols : list[list[float]]
        Implied vols shaped ``[n_expiries][n_strikes]``.
    forward_prices : list[float]
        One forward price to broadcast or one price per expiry.
    tolerance : float, optional
        Tolerance in total-variance units. Default ``1e-10`` (Rust ``DEFAULT_ARBITRAGE_TOLERANCE``, the same default as every Rust arbitrage check); pass a looser value to screen noisy market quotes.

    Returns
    -------
    ArbitrageReport
        Typed report with ``total_violations``, ``passed``, ``by_severity``,
        ``by_type``, ``violations`` and ``to_dataframe()``.

    Raises
    ------
    ValueError
        If the forward-price shape or grid inputs are invalid.

    Examples
    --------
    >>> from finstack_quant.models.volatility import check_surface_grid
    >>> strikes, expiries = [90.0, 100.0, 110.0], [1.0, 2.0]
    >>> vols, forwards = [[0.2, 0.2, 0.2], [0.2, 0.2, 0.2]], [100.0, 100.0]
    >>> check_surface_grid(strikes, expiries, vols, forwards).passed
    True

    """
    ...
