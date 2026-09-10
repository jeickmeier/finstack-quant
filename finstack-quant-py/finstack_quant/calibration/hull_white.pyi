"""Hull-White short-rate calibration to swaption and cap/floor quotes.

Scalar fits solve a constant mean reversion and volatility pair; the piecewise
bootstrap solves one volatility per quote maturity at a fixed mean reversion.

Examples:
--------
>>> from finstack_quant.calibration.hull_white import SwaptionQuote
>>> SwaptionQuote(1.0, 10.0, 0.0085).volatility
0.0085
"""

from typing import Any

from finstack_quant.calibration import CalibrationReport
from finstack_quant.core.market_data import DiscountCurve

__all__ = [
    "CapFloorCalibrationConfig",
    "CapFloorQuote",
    "HullWhiteCalibrationParams",
    "HullWhiteParams",
    "PiecewiseSigmaCalibrationConfig",
    "SwaptionQuote",
    "bootstrap_hull_white_sigma_schedule_to_cap_floors",
    "calibrate_hull_white_to_cap_floors",
    "calibrate_hull_white_to_swaptions",
]

class SwaptionQuote:
    """An at-the-money swaption volatility quote used to fit Hull-White.

    Examples:
    --------
    >>> from finstack_quant.calibration.hull_white import SwaptionQuote
    >>> SwaptionQuote(1.0, 10.0, 0.0085).tenor
    10.0

    """

    def __init__(
        self,
        expiry: float,
        tenor: float,
        volatility: float,
        is_normal_vol: bool = True,
    ) -> None:
        """Build a swaption quote.

        Parameters
        ----------
        expiry : float
            Option expiry in years from the curve base date.
        tenor : float
            Underlying swap tenor in years.
        volatility : float
            Quoted volatility: decimal per annum for normal (Bachelier) quotes
            (``0.0085`` for 85 bp), annualized decimal for lognormal quotes.
        is_normal_vol : bool, default True
            True when ``volatility`` is a normal (Bachelier) volatility.

        Raises
        ------
        ValueError
            If ``expiry``, ``tenor`` or ``volatility`` is not strictly positive.

        """

    @property
    def expiry(self) -> float:
        """
        Option expiry in years.

        This property does not raise.

        Returns
        -------
        float
            Option expiry in years.
        """

    @property
    def tenor(self) -> float:
        """
        Underlying swap tenor in years.

        This property does not raise.

        Returns
        -------
        float
            Underlying swap tenor in years.
        """

    @property
    def volatility(self) -> float:
        """
        Quoted volatility in the units implied by ``is_normal_vol``.

        This property does not raise.

        Returns
        -------
        float
            Quoted volatility in the units implied by ``is_normal_vol``.
        """

    @property
    def is_normal_vol(self) -> bool:
        """
        Whether the quote is a normal (Bachelier) volatility.

        This property does not raise.

        Returns
        -------
        bool
            Whether the quote is a normal (Bachelier) volatility.
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this quote.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> SwaptionQuote:
        """
        Rebuild a quote from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        SwaptionQuote
            The decoded quote.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration.hull_white import SwaptionQuote
        >>> SwaptionQuote.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class CapFloorQuote:
    """A cap or floor flat-volatility quote used to fit Hull-White.

    Examples:
    --------
    >>> from finstack_quant.calibration.hull_white import CapFloorQuote
    >>> CapFloorQuote(5.0, 0.04, 0.0090).is_cap
    True

    """

    def __init__(
        self,
        maturity: float,
        strike: float,
        volatility: float,
        is_cap: bool = True,
        is_normal_vol: bool = True,
    ) -> None:
        """Build a cap or floor quote.

        Parameters
        ----------
        maturity : float
            Cap or floor maturity in years from the curve base date.
        strike : float
            Strike rate as a decimal per annum.
        volatility : float
            Quoted flat volatility: decimal per annum for normal quotes,
            annualized decimal for lognormal quotes.
        is_cap : bool, default True
            True for a cap, False for a floor.
        is_normal_vol : bool, default True
            True when ``volatility`` is a normal (Bachelier) volatility.

        Raises
        ------
        ValueError
            If ``maturity`` or ``volatility`` is not strictly positive.

        """

    @property
    def maturity(self) -> float:
        """
        Cap or floor maturity in years.

        This property does not raise.

        Returns
        -------
        float
            Cap or floor maturity in years.
        """

    @property
    def strike(self) -> float:
        """
        Strike rate, decimal per annum.

        This property does not raise.

        Returns
        -------
        float
            Strike rate, decimal per annum.
        """

    @property
    def volatility(self) -> float:
        """
        Quoted flat volatility in the units implied by ``is_normal_vol``.

        This property does not raise.

        Returns
        -------
        float
            Quoted flat volatility in the units implied by ``is_normal_vol``.
        """

    @property
    def is_cap(self) -> bool:
        """
        True for a cap, False for a floor.

        This property does not raise.

        Returns
        -------
        bool
            True for a cap, False for a floor.
        """

    @property
    def is_normal_vol(self) -> bool:
        """
        Whether the quote is a normal (Bachelier) volatility.

        This property does not raise.

        Returns
        -------
        bool
            Whether the quote is a normal (Bachelier) volatility.
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this quote.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> CapFloorQuote:
        """
        Rebuild a quote from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        CapFloorQuote
            The decoded quote.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration.hull_white import CapFloorQuote
        >>> CapFloorQuote.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class HullWhiteCalibrationParams:
    """Scalar Hull-White parameters: constant mean reversion and volatility.

    Examples:
    --------
    >>> from finstack_quant.calibration.hull_white import HullWhiteCalibrationParams
    >>> HullWhiteCalibrationParams(0.03, 0.01).kappa
    0.03

    """

    def __init__(self, kappa: float, sigma: float) -> None:
        """Build scalar Hull-White parameters.

        Parameters
        ----------
        kappa : float
            Mean-reversion speed per annum.
        sigma : float
            Short-rate volatility, decimal per annum (``0.01`` for 100 bp).

        Raises
        ------
        ValueError
            If ``kappa`` or ``sigma`` is not strictly positive.

        """

    @property
    def kappa(self) -> float:
        """
        Mean-reversion speed per annum.

        This property does not raise.

        Returns
        -------
        float
            Mean-reversion speed per annum.
        """

    @property
    def sigma(self) -> float:
        """
        Short-rate volatility, decimal per annum.

        This property does not raise.

        Returns
        -------
        float
            Short-rate volatility, decimal per annum.
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of these parameters.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> HullWhiteCalibrationParams:
        """
        Rebuild parameters from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        HullWhiteCalibrationParams
            The decoded parameters.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration.hull_white import HullWhiteCalibrationParams
        >>> HullWhiteCalibrationParams.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class HullWhiteParams:
    """Hull-White parameters with a piecewise-constant volatility schedule.

    Examples:
    --------
    >>> from finstack_quant.calibration.hull_white import HullWhiteParams
    >>> p = HullWhiteParams.from_json('{"kappa":0.03,"volatility":{"times":[0.0],"values":[0.01]}}')
    >>> p.sigma_at(0.5)
    0.01

    """

    @property
    def kappa(self) -> float:
        """
        Mean-reversion speed per annum.

        This property does not raise.

        Returns
        -------
        float
            Mean-reversion speed per annum.
        """

    @property
    def times(self) -> list[float]:
        """
        Right endpoints of the volatility intervals, in years, increasing.

        This property does not raise.

        Returns
        -------
        list[float]
            Right endpoints of the volatility intervals, in years, increasing.
        """

    @property
    def values(self) -> list[float]:
        """
        Volatility on each interval, decimal per annum.

        This property does not raise.

        Returns
        -------
        list[float]
            Volatility on each interval, decimal per annum.
        """

    def sigma_at(self, time: float) -> float:
        """Volatility in force at a given time.

        Parameters
        ----------
        time : float
            Time in years from the curve base date. Values outside the
            calibrated grid clamp to the first or last interval.

        This lookup is a piecewise-constant read and does not raise.

        Returns
        -------
        float
            Piecewise-constant volatility at ``time``, decimal per annum; the
            last interval's value applies beyond the final knot.

        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of these parameters.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> HullWhiteParams:
        """
        Rebuild parameters from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        HullWhiteParams
            The decoded parameters.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration.hull_white import HullWhiteParams
        >>> HullWhiteParams.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class CapFloorCalibrationConfig:
    """Settings for the scalar Hull-White fit to cap/floor quotes.

    Examples:
    --------
    >>> from finstack_quant.calibration.hull_white import CapFloorCalibrationConfig
    >>> CapFloorCalibrationConfig(1e-4, fixed_kappa=0.03).fixed_kappa
    0.03

    """

    def __init__(
        self,
        fit_tolerance: float,
        frequency: str = "semi_annual",
        fixed_kappa: float | None = None,
        initial_guess: HullWhiteCalibrationParams | None = None,
    ) -> None:
        """Build a cap/floor calibration configuration.

        Parameters
        ----------
        fit_tolerance : float
            Required positive maximum absolute implied normal-vol error, in
            decimal rate units (0.0001 is one normal-vol basis point). This
            acceptance budget is independent of numerical solver tolerance.
        frequency : str, default "semi_annual"
            Caplet payment frequency: ``"annual"``, ``"semi_annual"`` or
            ``"quarterly"``.
        fixed_kappa : float | None, default None
            Mean reversion held fixed during the fit, per annum. Required when
            only one quote is supplied.
        initial_guess : HullWhiteCalibrationParams | None, default None
            Solver starting point; Rust defaults when None.

        Raises
        ------
        ValueError
            If ``frequency`` is not recognized. Numeric constraints are validated
            when calibration runs.

        """

    @property
    def fit_tolerance(self) -> float:
        """Maximum accepted implied normal-vol quote error.

        Returns
        -------
        float
            Positive absolute error budget in decimal rate volatility units.

        This property does not raise.
        """

    @property
    def frequency(self) -> str:
        """
        Caplet payment frequency.

        This property does not raise.

        Returns
        -------
        str
            Caplet payment frequency.
        """

    @property
    def fixed_kappa(self) -> float | None:
        """
        Mean reversion held fixed during the fit, or None.

        This property does not raise.

        Returns
        -------
        float | None
            Mean reversion held fixed during the fit, or None.
        """

    @property
    def initial_guess(self) -> HullWhiteCalibrationParams | None:
        """
        Solver starting point, or None.

        This property does not raise.

        Returns
        -------
        HullWhiteCalibrationParams | None
            Solver starting point, or None.
        """

    def __reduce__(self) -> tuple[Any, tuple[float, str, float | None, HullWhiteCalibrationParams | None]]: ...
    def __repr__(self) -> str: ...

class PiecewiseSigmaCalibrationConfig:
    """Settings for the piecewise-sigma Hull-White bootstrap.

    Examples:
    --------
    >>> from finstack_quant.calibration.hull_white import PiecewiseSigmaCalibrationConfig
    >>> PiecewiseSigmaCalibrationConfig(0.03, 1e-4, 0.05, 1e-4).sigma_max
    0.05

    """

    def __init__(
        self,
        fixed_kappa: float,
        sigma_min: float,
        sigma_max: float,
        fit_tolerance: float,
        frequency: str = "semi_annual",
    ) -> None:
        """Build a piecewise-sigma bootstrap configuration.

        Parameters
        ----------
        fixed_kappa : float
            Mean reversion held fixed across every interval, per annum.
        sigma_min : float
            Lower bracket for each interval's volatility, decimal per annum.
        sigma_max : float
            Upper bracket for each interval's volatility, decimal per annum.
        fit_tolerance : float
            Required positive maximum absolute implied normal-vol error, in
            decimal rate units (0.0001 is one normal-vol basis point). This
            acceptance budget is independent of numerical solver tolerance.
        frequency : str, default "semi_annual"
            Caplet payment frequency: ``"annual"``, ``"semi_annual"`` or
            ``"quarterly"``.

        Raises
        ------
        ValueError
            If ``fixed_kappa`` or ``sigma_min`` is not strictly positive,
            ``sigma_min`` is not below ``sigma_max``, or ``frequency`` is not
            recognized.

        """

    @property
    def fit_tolerance(self) -> float:
        """Maximum accepted implied normal-vol quote error.

        Returns
        -------
        float
            Positive absolute error budget in decimal rate volatility units.

        This property does not raise.
        """

    @property
    def fixed_kappa(self) -> float:
        """
        Mean reversion held fixed across intervals, per annum.

        This property does not raise.

        Returns
        -------
        float
            Mean reversion held fixed across intervals, per annum.
        """

    @property
    def sigma_min(self) -> float:
        """
        Lower volatility bracket, decimal per annum.

        This property does not raise.

        Returns
        -------
        float
            Lower volatility bracket, decimal per annum.
        """

    @property
    def sigma_max(self) -> float:
        """
        Upper volatility bracket, decimal per annum.

        This property does not raise.

        Returns
        -------
        float
            Upper volatility bracket, decimal per annum.
        """

    @property
    def frequency(self) -> str:
        """
        Caplet payment frequency.

        This property does not raise.

        Returns
        -------
        str
            Caplet payment frequency.
        """

    def __reduce__(self) -> tuple[Any, tuple[float, float, float, float, str]]: ...
    def __repr__(self) -> str: ...

def calibrate_hull_white_to_swaptions(
    discount: DiscountCurve,
    quotes: list[SwaptionQuote],
    fit_tolerance: float,
    frequency: str = "semi_annual",
    initial_guess: HullWhiteCalibrationParams | None = None,
) -> tuple[HullWhiteCalibrationParams, CalibrationReport]:
    """Fit scalar Hull-White parameters to at-the-money swaption quotes.

    Parameters
    ----------
    discount : DiscountCurve
        Discount curve the swap annuities and forwards are read from.
    quotes : list[SwaptionQuote]
        At least two swaption quotes, one per free parameter.
    fit_tolerance : float
        Required positive maximum absolute reconstructed quote error. Normal
        quotes use decimal rate volatility (0.0001 is one basis point); Black
        quotes use relative volatility. A poor fit returns a report with
        ``success=False``; callers must inspect it before using the parameters.
    frequency : str, default "semi_annual"
        Fixed-leg payment frequency: ``"annual"``, ``"semi_annual"`` or
        ``"quarterly"``.
    initial_guess : HullWhiteCalibrationParams | None, default None
        Solver starting point; Rust defaults when None.

    Returns
    -------
    tuple[HullWhiteCalibrationParams, CalibrationReport]
        Fitted parameters and the fit report, whose residuals are in volatility
        units.

    Raises
    ------
    ValueError
        If fewer than two quotes are supplied, ``frequency`` is invalid, or
        ``discount`` is not a ``DiscountCurve``, or ``fit_tolerance`` is not
        finite and positive.
    RuntimeError
        If the solver fails to converge.

    Examples:
    --------
    >>> from finstack_quant.calibration.hull_white import (
    ...     SwaptionQuote,
    ...     calibrate_hull_white_to_swaptions,
    ... )
    >>> import datetime
    >>> from finstack_quant.core.market_data import DiscountCurve
    >>> curve = DiscountCurve.flat("USD-OIS", datetime.date(2025, 1, 1), 0.03)
    >>> params, report = calibrate_hull_white_to_swaptions(
    ...     curve,
    ...     [SwaptionQuote(1.0, 5.0, 0.0085), SwaptionQuote(5.0, 5.0, 0.0080)],
    ...     fit_tolerance=1e-4,
    ... )
    >>> params.sigma > 0.0
    True

    """

def calibrate_hull_white_to_cap_floors(
    discount: DiscountCurve,
    quotes: list[CapFloorQuote],
    config: CapFloorCalibrationConfig,
    forward: DiscountCurve | None = None,
) -> tuple[HullWhiteCalibrationParams, CalibrationReport]:
    """Fit scalar Hull-White parameters to cap/floor quotes.

    Parameters
    ----------
    discount : DiscountCurve
        Discounting curve.
    quotes : list[CapFloorQuote]
        Cap/floor quotes; a single quote requires ``config.fixed_kappa``.
    forward : DiscountCurve | None, default None
        Curve projecting the caplet forwards; ``discount`` when None.
    config : CapFloorCalibrationConfig
        Required quote-fit budget, frequency, fixed kappa and initial guess.
        Reports with ``success=False`` must not be treated as accepted fits.

    Returns
    -------
    tuple[HullWhiteCalibrationParams, CalibrationReport]
        Fitted parameters and the fit report.

    Raises
    ------
    ValueError
        If no quotes are supplied, a single quote is given without
        ``fixed_kappa``, or a curve argument is not a ``DiscountCurve``.
    RuntimeError
        If the solver fails to converge.

    Examples:
    --------
    >>> from finstack_quant.calibration.hull_white import (
    ...     CapFloorCalibrationConfig,
    ...     CapFloorQuote,
    ...     calibrate_hull_white_to_cap_floors,
    ... )
    >>> import datetime
    >>> from finstack_quant.core.market_data import DiscountCurve
    >>> curve = DiscountCurve.flat("USD-OIS", datetime.date(2025, 1, 1), 0.03)
    >>> params, report = calibrate_hull_white_to_cap_floors(
    ...     curve,
    ...     [CapFloorQuote(5.0, 0.03, 0.009)],
    ...     config=CapFloorCalibrationConfig(1e-4, fixed_kappa=0.03),
    ... )
    >>> params.kappa
    0.03

    """

def bootstrap_hull_white_sigma_schedule_to_cap_floors(
    discount: DiscountCurve,
    quotes: list[CapFloorQuote],
    config: PiecewiseSigmaCalibrationConfig,
    forward: DiscountCurve | None = None,
) -> tuple[HullWhiteParams, CalibrationReport]:
    """Bootstrap a piecewise-constant Hull-White volatility schedule.

    Parameters
    ----------
    discount : DiscountCurve
        Discounting curve.
    quotes : list[CapFloorQuote]
        Cap/floor quotes with strictly increasing maturities; each maturity adds
        one volatility interval.
    config : PiecewiseSigmaCalibrationConfig
        Fixed mean reversion, volatility brackets and payment frequency.
    forward : DiscountCurve | None, default None
        Curve projecting the caplet forwards; ``discount`` when None.

    Returns
    -------
    tuple[HullWhiteParams, CalibrationReport]
        Piecewise parameters and the fit report.

    Raises
    ------
    ValueError
        If no quotes are supplied, the configuration bounds are invalid, or a
        curve argument is not a ``DiscountCurve``.
    RuntimeError
        If an interval's volatility cannot be bracketed or solved.

    Examples:
    --------
    >>> from finstack_quant.calibration.hull_white import (
    ...     CapFloorQuote,
    ...     PiecewiseSigmaCalibrationConfig,
    ...     bootstrap_hull_white_sigma_schedule_to_cap_floors,
    ... )
    >>> import datetime
    >>> from finstack_quant.core.market_data import DiscountCurve
    >>> curve = DiscountCurve.flat("USD-OIS", datetime.date(2025, 1, 1), 0.03)
    >>> params, report = bootstrap_hull_white_sigma_schedule_to_cap_floors(
    ...     curve,
    ...     [CapFloorQuote(2.0, 0.03, 0.009), CapFloorQuote(5.0, 0.03, 0.010)],
    ...     PiecewiseSigmaCalibrationConfig(0.03, 1e-4, 0.05, 1e-4),
    ... )
    >>> len(params.values)
    2

    """
