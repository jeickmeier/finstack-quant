"""Hull-White one-factor parameters and closed-form pricing kernels.

Product-independent equations of the Hull-White (1990) one-factor short-rate
model: the parameter set (:class:`HullWhiteParams`) and the scalar kernels for
convexity adjustments, bond-price volatility, zero-coupon bond options, the
caplet normal-vol proxy and cap/floor pricing. Quote preparation and fitting
live in :mod:`finstack_quant.calibration`.

Units: ``kappa`` is a mean-reversion speed in inverse years; ``sigma`` is a
short-rate volatility in absolute rate units per square-root year (``0.01`` =
100 bp/sqrt(yr)); times are year fractions; strikes are decimal rates.

Examples
--------
>>> from finstack_quant.models.rates.hull_white import hw_bond_vol
>>> round(hw_bond_vol(0.05, 0.01, 0.0, 1.0, 2.0), 6)
0.009515
"""

from __future__ import annotations

from typing import Any, Sequence

from finstack_quant.core.market_data import DiscountCurve

__all__ = [
    "HullWhiteParams",
    "hw1f_cap_floor_price",
    "hw1f_caplet_forward_rate_normal_vol",
    "hw1f_convexity_adjustment",
    "hw1f_zcb_option_price",
    "hw_bond_vol",
]

class HullWhiteParams:
    """Hull-White one-factor parameters with a piecewise-constant volatility schedule.

    Parameters
    ----------
    kappa : float
        Mean-reversion speed in inverse years; finite and positive.
    sigma : float
        Constant short-rate volatility in absolute rate units per
        square-root year; finite and positive. Use :meth:`piecewise` for a
        term structure of volatility.

    Raises
    ------
    ValueError
        If ``kappa`` or ``sigma`` is non-finite or not strictly positive.

    Examples
    --------
    >>> from finstack_quant.models.rates.hull_white import HullWhiteParams
    >>> params = HullWhiteParams(0.05, 0.01)
    >>> (params.kappa, params.sigma(0.5), params.times)
    (0.05, 0.01, [0.0])
    """

    def __init__(self, kappa: float, sigma: float) -> None: ...
    @classmethod
    def piecewise(cls, kappa: float, times: Sequence[float], values: Sequence[float]) -> HullWhiteParams:
        """Build parameters with a piecewise-constant volatility schedule.

        Parameters
        ----------
        kappa : float
            Mean-reversion speed in inverse years; finite and positive.
        times : Sequence[float]
            Strictly increasing knot times in years starting at ``0.0``.
        values : Sequence[float]
            Non-negative volatility per knot (absolute rate units per
            square-root year) applying from ``times[i]`` until the next knot,
            flat after the last knot.

        Returns
        -------
        HullWhiteParams
            Validated parameters.

        Raises
        ------
        ValueError
            If the schedule is empty, ragged, does not start at ``0.0``, is not
            strictly increasing, holds a negative value, or ``kappa`` is invalid.

        Examples
        --------
        >>> from finstack_quant.models.rates.hull_white import HullWhiteParams
        >>> HullWhiteParams.piecewise(0.05, [0.0, 1.0], [0.01, 0.012]).sigma(1.5)
        0.012
        """
        ...

    @property
    def kappa(self) -> float:
        """
        Mean-reversion speed of the short rate, in inverse years (1/y).

        Returns
        -------
        float
            The Hull-White ``kappa``; larger values pull the short rate back
            to its time-dependent mean faster (``0.05`` is a ~20-year
            half-life scale).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def times(self) -> list[float]:
        """
        Knot times of the piecewise-constant volatility schedule.

        Returns
        -------
        list[float]
            Non-decreasing year fractions measured from the model base date;
            an empty list for a flat-volatility parameterisation.

        Notes
        -----
        This accessor does not raise; it returns the stored schedule.
        """
        ...

    @property
    def values(self) -> list[float]:
        """
        Short-rate volatility applying on each knot interval.

        Returns
        -------
        list[float]
            Absolute (normal) volatilities in rate units per square root of a
            year, aligned with :attr:`times`; ``0.01`` means 100 bp per
            square-root year. A single-element list for a flat schedule.

        Notes
        -----
        This accessor does not raise; it returns the stored schedule.
        """
        ...

    def sigma(self, t: float) -> float:
        """Return the short-rate volatility applying at time ``t``.

        Parameters
        ----------
        t : float
            Model time in years.

        Returns
        -------
        float
            Volatility in absolute rate units per square-root year.

        Notes
        -----
        This method does not raise; a time outside the knot schedule clamps
        to the first or last knot value.

        Examples
        --------
        >>> from finstack_quant.models.rates.hull_white import HullWhiteParams
        >>> HullWhiteParams(0.05, 0.01).sigma(3.0)
        0.01
        """
        ...

    def state_variance(self, t: float) -> float:
        """Return the variance of the centered short-rate state at ``t``.

        Parameters
        ----------
        t : float
            Model time in years.

        Returns
        -------
        float
            ``Var[x(t)]`` in rate units squared.

        Raises
        ------
        ValueError
            If the integration interval is invalid.

        Examples
        --------
        >>> from finstack_quant.models.rates.hull_white import HullWhiteParams
        >>> round(HullWhiteParams(0.05, 0.01).state_variance(1.0), 8)
        9.516e-05
        """
        ...

    def state_covariance(self, left_time: float, right_time: float) -> float:
        """Return the covariance of the centered short-rate state at two times.

        Parameters
        ----------
        left_time : float
            First model time in years.
        right_time : float
            Second model time in years.

        Returns
        -------
        float
            Covariance in rate units squared; ``0.0`` when the earlier time is
            non-positive.

        Raises
        ------
        ValueError
            If the earlier-time variance cannot be evaluated.

        Examples
        --------
        >>> from finstack_quant.models.rates.hull_white import HullWhiteParams
        >>> HullWhiteParams(0.05, 0.01).state_covariance(0.0, 1.0)
        0.0
        """
        ...

    def bond_vol(self, t: float, expiry: float, maturity: float) -> float:
        """Return the ``maturity`` zero-coupon bond price volatility over ``[t, expiry]``.

        Parameters
        ----------
        t : float
            Valuation time in years (``>= 0``).
        expiry : float
            Option expiry in years (``>= t``).
        maturity : float
            Bond maturity in years (``>= expiry``).

        Returns
        -------
        float
            Total (not annualized) lognormal bond-price volatility.

        Raises
        ------
        ValueError
            If the time ordering ``t <= expiry <= maturity`` is violated or a
            time is negative/non-finite.

        Examples
        --------
        >>> from finstack_quant.models.rates.hull_white import HullWhiteParams, hw_bond_vol
        >>> round(HullWhiteParams(0.05, 0.01).bond_vol(0.0, 1.0, 2.0), 12) == round(
        ...     hw_bond_vol(0.05, 0.01, 0.0, 1.0, 2.0), 12
        ... )
        True
        """
        ...

    def to_json(self) -> str:
        """Serialize to the canonical JSON wire format.

        Returns
        -------
        str
            JSON document with ``kappa`` and the ``volatility`` schedule.

        Raises
        ------
        ValueError
            If the parameters cannot be serialized to JSON (raised as
            ``"HullWhiteParams serialization failed"``).

        Examples
        --------
        >>> from finstack_quant.models.rates.hull_white import HullWhiteParams
        >>> params = HullWhiteParams(0.05, 0.01)
        >>> HullWhiteParams.from_json(params.to_json()) == params
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> HullWhiteParams:
        """Deserialize parameters produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        HullWhiteParams
            The reconstructed parameters.

        Raises
        ------
        ValueError
            If the payload is malformed or fails validation.

        Examples
        --------
        >>> from finstack_quant.models.rates.hull_white import HullWhiteParams
        >>> HullWhiteParams.from_json('{"kappa":0.05,"volatility":{"times":[0.0],"values":[0.01]}}').kappa
        0.05
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

def hw1f_convexity_adjustment(kappa: float, sigma: float, t_settle: float, t_end: float) -> float:
    """Approximate the simple-deposit futures-minus-forward bias under Hull-White.

    Parameters
    ----------
    kappa : float
        Finite non-negative mean-reversion speed in inverse years; zero uses
        the Ho-Lee limit.
    sigma : float
        Finite non-negative short-rate volatility in absolute decimal rate
        units per square-root year.
    t_settle : float
        Settlement/fixing time in years using the deposit day-count convention.
    t_end : float
        End of the accrual period on the same day-count basis; ``t_end -
        t_settle`` is the deposit accrual fraction.

    Returns
    -------
    float
        Approximate futures-minus-forward bias in decimal rate units; ``0.0``
        for non-positive fixing time or accrual. Non-finite inputs or negative
        ``kappa``/``sigma`` return ``NaN`` before degenerate-period handling.

    Notes
    -----
    This low-rate, small-variance approximation does not perform the exact
    conversion from a simple forward rate to a futures quote. The Ho-Lee
    limit is ``0.5 * sigma**2 * t_settle * (2*t_end - t_settle)``.
    This helper does not raise.

    Examples
    --------
    >>> from finstack_quant.models.rates.hull_white import hw1f_convexity_adjustment
    >>> round(hw1f_convexity_adjustment(0.0, 0.01, 1.0, 2.0), 8)
    0.00015
    """
    ...

def hw_bond_vol(kappa: float, sigma: float, t: float, expiry: float, maturity: float) -> float:
    """Return the zero-coupon bond price volatility for constant Hull-White parameters.

    Parameters
    ----------
    kappa : float
        Mean-reversion speed in inverse years.
    sigma : float
        Short-rate volatility in absolute rate units per square-root year.
    t : float
        Valuation time in years.
    expiry : float
        Option expiry in years (``>= t``).
    maturity : float
        Bond maturity in years (``>= expiry``).

    Returns
    -------
    float
        Total (not annualized) lognormal bond-price volatility
        ``B(expiry, maturity) * sigma * sqrt(var)``.

    Notes
    -----
    This helper does not raise; a negative variance window is floored at zero.

    Examples
    --------
    >>> from finstack_quant.models.rates.hull_white import hw_bond_vol
    >>> round(hw_bond_vol(0.05, 0.01, 0.0, 1.0, 2.0), 6)
    0.009515
    """
    ...

def hw1f_zcb_option_price(
    p0_expiry: float,
    p0_maturity: float,
    strike: float,
    bond_vol: float,
    is_call: bool,
) -> float:
    """Return the Jamshidian closed-form price of a European zero-coupon bond option.

    Parameters
    ----------
    p0_expiry : float
        Discount factor to the option expiry.
    p0_maturity : float
        Discount factor to the bond maturity.
    strike : float
        Strike bond price per unit face.
    bond_vol : float
        Total bond-price volatility from :func:`hw_bond_vol` or
        :meth:`HullWhiteParams.bond_vol`.
    is_call : bool
        ``True`` for a call on the bond, ``False`` for a put.

    Returns
    -------
    float
        Present value per unit face, floored at zero.

    Notes
    -----
    This helper does not raise; the intrinsic value is returned when
    ``bond_vol`` is (numerically) zero.

    Examples
    --------
    >>> from finstack_quant.models.rates.hull_white import hw1f_zcb_option_price
    >>> call = hw1f_zcb_option_price(0.98, 0.94, 0.96, 0.03, True)
    >>> put = hw1f_zcb_option_price(0.98, 0.94, 0.96, 0.03, False)
    >>> round(call - put - (0.94 - 0.96 * 0.98), 12)
    0.0
    """
    ...

def hw1f_caplet_forward_rate_normal_vol(kappa: float, sigma: float, t_fix: float, accrual: float) -> float:
    """Return the annualized normal volatility of a forward rate implied by Hull-White.

    Parameters
    ----------
    kappa : float
        Mean-reversion speed in inverse years.
    sigma : float
        Short-rate volatility in absolute rate units per square-root year.
    t_fix : float
        Caplet fixing time in years (``> 0``).
    accrual : float
        Accrual period of the underlying rate in years (``> 0``).

    Returns
    -------
    float
        Normal vol in absolute rate units per square-root year; ``0.0`` when
        any input is non-positive.

    Notes
    -----
    This helper does not raise.

    Examples
    --------
    >>> from finstack_quant.models.rates.hull_white import hw1f_caplet_forward_rate_normal_vol
    >>> 0.0 < hw1f_caplet_forward_rate_normal_vol(0.05, 0.01, 1.0, 0.25) < 0.01
    True
    """
    ...

def hw1f_cap_floor_price(
    kappa: float,
    sigma: float,
    periods: Sequence[tuple[float, float, float]],
    strike: float,
    is_cap: bool,
    discount_curve: DiscountCurve,
    forward_curve: DiscountCurve | None = None,
) -> float:
    """Price a cap or floor as a sum of Hull-White caplets/floorlets.

    Parameters
    ----------
    kappa : float
        Mean-reversion speed in inverse years; finite and positive.
    sigma : float
        Short-rate volatility in absolute rate units per square-root year;
        finite and positive.
    periods : Sequence[tuple[float, float, float]]
        One ``(t_fix, t_pay, accrual)`` triple per caplet, all in years.
    strike : float
        Cap/floor strike as a decimal rate.
    is_cap : bool
        ``True`` prices a cap, ``False`` a floor.
    discount_curve : DiscountCurve
        Discounting curve queried at ``t_pay``.
    forward_curve : DiscountCurve or None, default None
        Projection curve for the forward rates; ``None`` reuses
        ``discount_curve`` (single-curve).

    Returns
    -------
    float
        Present value per unit notional (``NaN`` if a discount factor is
        non-positive).

    Raises
    ------
    ValueError
        If ``kappa`` or ``sigma`` is non-finite or not strictly positive.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.market_data import DiscountCurve
    >>> from finstack_quant.models.rates.hull_white import hw1f_cap_floor_price
    >>> curve = DiscountCurve.flat("USD-OIS", datetime.date(2025, 1, 1), 0.03)
    >>> cap = hw1f_cap_floor_price(0.05, 0.01, [(1.0, 2.0, 1.0)], 0.03, True, curve)
    >>> floor = hw1f_cap_floor_price(0.05, 0.01, [(1.0, 2.0, 1.0)], 0.03, False, curve)
    >>> cap > 0.0 and floor > 0.0
    True
    """
    ...
