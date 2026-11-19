"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/curves/hazard.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Optional, Sequence, Union
import pandas as pd
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import DateLike, TimeOrDate

class HazardCurve:
    """
    Credit hazard-rate curve for default-probability modelling.

    Stores piecewise-constant hazard rates ``(t, lambda)`` at strictly positive
    ending times ``t`` in years from ``base_date`` with ``lambda`` as an annual
    default intensity (decimal). Each ``lambda`` applies to the segment *ending*
    at its knot, so ``sp(t) = exp(-integral of lambda)``. Query methods accept a
    year fraction or a date (converted with the curve day count by Rust).

    Examples
    --------
    >>> from finstack_quant.core.market_data import HazardCurve
    >>> curve = HazardCurve(
    ...     "ACME-HZD", "2025-01-01", [(1.0, 0.02), (5.0, 0.03)], recovery_rate=0.4, day_count="act_365f"
    ... )
    >>> round(curve.sp(1.0), 6)
    0.980199
    >>> round(curve.conditional_default_prob("2025-01-01", "2026-01-01"), 6)
    0.019801
    >>> curve.knot_points
    [(1.0, 0.02), (5.0, 0.03)]

    """

    def __init__(
        self,
        id: str,
        base_date: DateLike,
        knots: Sequence[tuple[float, float]],
        *,
        recovery_rate: float,
        day_count: Union[DayCount, str],
        calendar_id: Optional[str] = None,
        issuer: Optional[str] = None,
        seniority: Optional[str] = None,
        currency: Optional[Union[Currency, str]] = None,
    ) -> None:
        """
        Construct a hazard curve from ``(time_years, hazard_rate)`` knots.

        Parameters
        ----------
        id : str
            Unique curve identifier (e.g. ``"ACME-HZD"``).
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        knots : Sequence[tuple[float, float]]
            ``(time_years, hazard_rate)`` pairs with strictly positive
            increasing ending times ``t > 0``; hazard rates are annual default
            intensities as decimals (``0.02`` is 2% per year).
        recovery_rate : float
            Recovery on default as a decimal fraction in ``[0, 1]`` (keyword-only).
        day_count : DayCount | str
            Explicit day-count label or object (keyword-only and required).
        calendar_id : str, optional
            Registered holiday calendar of the curve's time axis (required with
            ``day_count="bus_252"``).
        issuer : str, optional
            Issuer name metadata.
        seniority : str, optional
            ``"senior_secured"``, ``"senior"``, ``"subordinated"`` or ``"junior"``.
        currency : Currency or str, optional
            Currency of the protection leg (metadata).

        Raises
        ------
        ValueError
            If a knot has non-positive/non-increasing time or negative/non-finite
            hazard rate, ``recovery_rate`` is outside ``[0, 1]``, ``day_count``
            is invalid or lacks a required calendar, or a label is unknown.
        TypeError
            If ``recovery_rate`` or ``day_count`` is omitted.

        Examples
        --------
        >>> from finstack_quant.core.market_data import HazardCurve
        >>> curve = HazardCurve(
        ...     "ACME-HZD", "2025-01-01", [(1.0, 0.02)], recovery_rate=0.4, day_count="act_365f", seniority="senior"
        ... )
        >>> curve.seniority
        'senior'

        """
        ...

    @staticmethod
    def flat(
        id: str,
        base_date: DateLike,
        hazard_rate: float,
        recovery_rate: float,
        day_count: Union[DayCount, str],
    ) -> HazardCurve:
        """
        Construct a flat (constant-intensity) hazard curve.

        Parameters
        ----------
        id : str
            Unique curve identifier.
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        hazard_rate : float
            Constant annual default intensity as a decimal (``0.02`` is 2%).
        recovery_rate : float
            Recovery on default as a decimal fraction in ``[0, 1]``.
        day_count : DayCount | str
            Explicit supported curve day-count convention.

        Returns
        -------
        HazardCurve
            Curve with ``sp(t) == exp(-hazard_rate * t)``.

        Raises
        ------
        ValueError
            If ``hazard_rate`` is non-finite or negative, ``recovery_rate``
            is outside ``[0, 1]``, or ``day_count`` is invalid or requires an
            absent calendar.

        Examples
        --------
        >>> from finstack_quant.core.market_data import HazardCurve
        >>> round(HazardCurve.flat("ACME", "2025-01-01", 0.02, 0.4, "act_365f").sp(5.0), 6)
        0.904837

        """
        ...

    @staticmethod
    def from_survival_probs(
        id: str,
        base_date: DateLike,
        points: Sequence[tuple[float, float]],
        recovery_rate: float,
        day_count: Union[DayCount, str],
    ) -> HazardCurve:
        """
        Construct a hazard curve from survival-probability pillars.

        Parameters
        ----------
        id : str
            Unique curve identifier.
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        points : Sequence[tuple[float, float]]
            ``(time_years, survival_probability)`` pillars with probabilities
            in ``(0, 1]`` and non-increasing in time. An optional ``t = 0``
            pillar must be ``1.0``; at least one ``t > 0`` pillar is required.
        recovery_rate : float
            Recovery on default as a decimal fraction in ``[0, 1]``.
        day_count : DayCount | str
            Explicit supported curve day-count convention.

        Returns
        -------
        HazardCurve
            Piecewise-constant hazard curve reproducing every pillar exactly.

        Raises
        ------
        ValueError
            If ``points`` is empty or lacks a positive time, a probability is
            outside ``(0, 1]`` or increases with time, ``recovery_rate`` is
            outside ``[0, 1]``, or ``day_count`` is invalid.

        Examples
        --------
        >>> from finstack_quant.core.market_data import HazardCurve
        >>> curve = HazardCurve.from_survival_probs("ACME", "2025-01-01", [(1.0, 0.98), (5.0, 0.90)], 0.4, "act_365f")
        >>> round(curve.sp(5.0), 6)
        0.9

        """
        ...

    @staticmethod
    def from_json(json: str) -> HazardCurve:
        """
        Deserialize a curve from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        HazardCurve
            The constructed hazard curve, ready for survival-probability queries.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import HazardCurve
        >>> curve = HazardCurve.flat("ACME", "2025-01-01", 0.02, 0.4, "act_365f")
        >>> HazardCurve.from_json(curve.to_json()) == curve
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the curve cannot be serialized.
        """
        ...

    def sp(self, t: TimeOrDate) -> float:
        """
        Survival probability at a year fraction or date.

        Parameters
        ----------
        t : float, datetime.date or str
            Non-negative year fraction from ``base_date``, or a date on or
            after ``base_date`` converted with the curve day count.

        Returns
        -------
        float
            Probability in ``[0, 1]``; ``1.0`` at ``t = 0``.

        Raises
        ------
        ValueError
            If ``t < 0``, ``t`` is non-finite, or a date precedes ``base_date``.
        """
        ...

    def hazard_rate(self, t: TimeOrDate) -> float:
        """
        Instantaneous hazard rate (decimal per year) at a year fraction or date.

        Parameters
        ----------
        t : float, datetime.date or str
            Non-negative year fraction from ``base_date``, or a date on or
            after ``base_date`` converted with the curve day count.

        Returns
        -------
        float
            Hazard rate of the segment containing ``t``.

        Raises
        ------
        ValueError
            If ``t < 0``, ``t`` is non-finite, or a date precedes ``base_date``.
        """
        ...

    def survival_at_dates(self, dates: Sequence[DateLike]) -> list[float]:
        """
        Survival probabilities on several dates.

        Parameters
        ----------
        dates : Sequence[datetime.date or str]
            Target dates; a date on or before ``base_date`` gives ``1.0``.

        Returns
        -------
        list[float]
            One survival probability per input date, in order.

        Raises
        ------
        ValueError
            If any year fraction cannot be computed.
        """
        ...

    def default_prob(self, t1: float, t2: float) -> float:
        """
        Probability of default in ``[t1, t2]``: ``sp(t1) - sp(t2)``.

        Parameters
        ----------
        t1 : float
            Start year fraction.
        t2 : float
            End year fraction; must not precede ``t1``.

        Returns
        -------
        float
            Default probability.

        Raises
        ------
        ValueError
            If ``t2 < t1``.
        """
        ...

    def conditional_default_prob(self, start: DateLike, end: DateLike) -> float:
        """Default probability conditional on survival to the start date.

        Integrates the piecewise-constant annual hazard directly, preserving
        small probabilities even when unconditional survival has underflowed.
        This query assumes survival to ``start``; it does not determine whether
        an issuer has actually defaulted.

        Parameters
        ----------
        start : datetime.date or str
            Conditioning date, converted with the curve day count. Dates before
            ``base_date`` are clamped to time zero.
        end : datetime.date or str
            End date on or after ``start``. Equal dates give zero; the final
            hazard rate extends beyond the last knot.

        Returns
        -------
        float
            Conditional default probability as a decimal fraction in [0, 1].

        Raises
        ------
        ValueError
            If a date is malformed, ``end`` precedes ``start``, or the curve
            day count cannot compute either date's year fraction.
        TypeError
            If either argument is not a supported date or ISO date string.
        """
        ...

    def with_recovery_rate(self, recovery_rate: float) -> HazardCurve:
        """
        Copy of this curve with a different recovery-rate metadata value.

        Survival probabilities are unchanged.

        Parameters
        ----------
        recovery_rate : float
            New recovery as a decimal fraction in ``[0, 1]``.

        Returns
        -------
        HazardCurve
            The constructed hazard curve, ready for survival-probability queries.

        Raises
        ------
        ValueError
            If ``recovery_rate`` is outside ``[0, 1]``.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export knots as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``t`` (years) and ``hazard_rate`` (decimal); one row per knot.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this curve is registered and looked up under.

        Returns
        -------
        str
            The curve id supplied at construction (for example ``"USD-OIS"``); market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """
        Valuation date the curve's time axis is measured from.

        Returns
        -------
        datetime.date
            The base date; a year fraction ``t`` on this curve means ``t`` years after this date under :attr:`day_count`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def recovery_rate(self) -> float:
        """
        Assumed recovery on the reference obligation.

        Returns
        -------
        float
            Recovery as a decimal fraction of notional (``0.4`` is 40%); loss given default is ``1 - recovery_rate``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def knots(self) -> list[float]:
        """Return the knot coordinates on the curve time axis.

        Returns
        -------
        list[float]
            Years from the base date, in increasing order.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def hazard_rates(self) -> list[float]:
        """Return the hazard intensities at the knots.

        Returns
        -------
        list[float]
            Continuous decimal intensities per year (0.02 is 2% per year).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def day_count(self) -> str:
        """
        Day-count convention converting calendar dates to curve time.

        Returns
        -------
        str
            Lower-case ISDA day-count label such as ``"act_365f"`` or ``"act_360"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def calendar_id(self) -> Optional[str]:
        """
        Registered holiday calendar of the curve's time axis.

        Returns
        -------
        str or None
            Canonical calendar id (for example ``"brbd"`` on a ``"bus_252"``
            curve), or ``None`` when the day count needs no calendar.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> Optional[Currency]:
        """
        Settlement currency recorded for the credit.

        Returns
        -------
        Optional[Currency]
            ISO-4217 currency code of the protection leg, or ``None`` when the curve carries no currency metadata.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def issuer(self) -> Optional[str]:
        """
        Reference entity the hazard curve describes.

        Returns
        -------
        Optional[str]
            Free-form issuer label used for reporting and curve selection, or ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def seniority(self) -> Optional[str]:
        """
        Capital-structure seniority of the reference obligation.

        Returns
        -------
        Optional[str]
            One of ``"senior_secured"``, ``"senior"``, ``"subordinated"`` or ``"junior"``, or ``None`` when unset; it typically drives the recovery assumption.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def hazard_calibration(self) -> Optional[dict[str, Any]]:
        """
        Calibration recipe stamped on this curve by the calibration engine.

        The dict is the serde form of the Rust ``HazardCalibrationRecipe``, the
        same object found under ``"hazard_calibration"`` in :meth:`to_json`:
        ``hazard_params`` (the hazard step parameters), ``calibration_inputs``
        and ``spread_risk_inputs`` (each entry holds the typed CDS ``quote``
        with its ``pillar_date`` and year-fraction ``pillar_time``) and
        ``calibration_config`` (solver and validation policy of the original
        solve).

        Returns
        -------
        Optional[dict[str, Any]]
            The recipe, or ``None`` for a curve that was built directly rather than
            produced by a calibration step.

        Raises
        ------
        ValueError
            If the recipe cannot be converted to Python objects.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...
