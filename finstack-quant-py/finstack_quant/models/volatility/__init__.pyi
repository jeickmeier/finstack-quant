"""Product-independent volatility models, evaluators, fitting, and convention conversion.

FX delta-quoted smiles are stored as
:class:`finstack_quant.core.market_data.FxDeltaVolSurface` and evaluated with
:func:`get_fx_delta_vol` or :func:`materialize_fx_delta_surface`. Quote
construction from ATM / risk-reversal / butterfly rows is a Rust-only
``FxVolSurfaceBuilder``; Python callers construct the quote artifact directly.

Examples
--------
>>> from finstack_quant.models.volatility import SabrParameters
>>> SabrParameters.rates_default().beta
0.5
"""

from __future__ import annotations

from typing import Any, Literal, Optional, TypedDict

import pandas as pd

from finstack_quant.core.market_data import FxDeltaVolSurface, VolCube, VolSurface

__all__ = [
    "ArbitrageReport",
    "SabrCalibrator",
    "SabrModel",
    "SabrParameters",
    "SabrSmile",
    "SviParams",
    "calibrate_svi",
    "check_butterfly_grid",
    "check_calendar_spread_grid",
    "check_local_vol_density_grid",
    "check_surface_grid",
    "convert_atm_volatility",
    "delta_to_strike",
    "get_cube_normal_vol",
    "get_cube_normal_vol_clamped",
    "get_cube_vol",
    "get_cube_vol_clamped",
    "get_fx_delta_pillar_vols",
    "get_fx_delta_vol",
    "get_surface_vol",
    "get_surface_vol_clamped",
    "materialize_cube_expiry_slice",
    "materialize_cube_expiry_slice_normal",
    "materialize_cube_tenor_slice",
    "materialize_cube_tenor_slice_normal",
    "materialize_fx_delta_surface",
    "strike_to_delta",
    "surface_to_dataframe",
]

# SABR volatility smile

class SabrParameters:
    """
    SABR parameters ``(alpha, beta, nu, rho)`` with optional ``shift``.

    Enforces ``alpha > 0``, ``beta in [0, 1]``, ``nu >= 0``, ``rho in
    [-1, 1]``, and ``shift > 0`` when supplied.

    Sources
    -------
    - Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr

    Examples
    --------
    >>> from finstack_quant.models.volatility import SabrParameters
    >>> params = SabrParameters(0.2, 0.5, 0.3, -0.2)
    >>> (params.alpha, params.beta, params.nu, params.rho, params.is_shifted())
    (0.2, 0.5, 0.3, -0.2, False)

    """

    def __init__(
        self,
        alpha: float,
        beta: float,
        nu: float,
        rho: float,
        shift: float | None = None,
    ) -> None:
        """
        Create a validated SABR volatility-smile parameter set.

        Parameters
        ----------
        alpha : float
            Positive initial volatility level as a decimal (Black vol for
            ``beta > 0``, absolute normal vol in rate units for ``beta == 0``).
        beta : float
            CEV elasticity constrained to the closed interval ``[0, 1]``.
        nu : float
            Non-negative volatility-of-volatility parameter per square-root year.
        rho : float
            Instantaneous forward/volatility correlation in ``[-1, 1]``.
        shift : float or None, default None
            Optional positive shifted-lognormal displacement in the forward's
            rate units (``0.03`` = 3% shift) for negative-rate strikes and
            forwards; ``None`` uses the unshifted formula.

        Raises
        ------
        ValueError
            If any parameter is non-finite; ``alpha`` is non-positive;
            ``beta`` is outside ``[0, 1]``; ``nu`` is negative; ``rho`` is
            outside ``[-1, 1]``; or a supplied ``shift`` is non-positive.
        """
        ...
    @staticmethod
    def equity_default() -> SabrParameters:
        """
        Equity-standard defaults ``(alpha=0.20, beta=1.0, nu=0.30, rho=-0.20)``.

        Returns
        -------
        SabrParameters
            Default equity SABR parameters.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.volatility import SabrParameters
        >>> params = SabrParameters.equity_default()
        >>> (params.alpha, params.beta, params.nu, params.rho)
        (0.2, 1.0, 0.3, -0.2)
        """
        ...

    @staticmethod
    def rates_default() -> SabrParameters:
        """
        Rates-standard defaults ``(alpha=0.02, beta=0.5, nu=0.30, rho=0.0)``.

        Returns
        -------
        SabrParameters
            Default rates SABR parameters.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.volatility import SabrParameters
        >>> params = SabrParameters.rates_default()
        >>> (params.alpha, params.beta, params.nu, params.rho)
        (0.02, 0.5, 0.3, 0.0)
        """
        ...

    @property
    def alpha(self) -> float:
        """
        SABR alpha level parameter.

        Returns
        -------
        float
            Alpha parameter value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def beta(self) -> float:
        """
        SABR beta elasticity parameter in ``[0, 1]``.

        Returns
        -------
        float
            Beta parameter value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def nu(self) -> float:
        """
        Volatility-of-volatility parameter.

        Returns
        -------
        float
            Nu parameter value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rho(self) -> float:
        """
        Forward/volatility correlation parameter in ``[-1, 1]``.

        Returns
        -------
        float
            Rho parameter value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def shift(self) -> float | None:
        """
        Optional positive shift used for negative-rate smiles.

        Returns
        -------
        float or None
            Shift value, or ``None`` if not set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def is_shifted(self) -> bool:
        """
        ``True`` when parameters include a non-zero shift (negative-rate support).

        Returns
        -------
        bool
            ``True`` if a non-zero shift is present.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.
        """
        ...

class SabrModel:
    """
    Hagan-2002 SABR stochastic-volatility smile model.

    Sources
    -------
    - Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr

    Examples
    --------
    >>> from finstack_quant.models.volatility import SabrModel, SabrParameters
    >>> model = SabrModel(SabrParameters.rates_default())
    >>> (round(model.implied_vol(0.02, 0.02, 1.0), 6), model.supports_negative_rates())
    (0.142511, False)

    """

    def __init__(self, params: SabrParameters) -> None:
        """
        Create a SABR model from validated parameters.

        Parameters
        ----------
        params : SabrParameters
            SABR parameter set (alpha, beta, nu, rho, optional shift).

        Raises
        ------
        ValueError
            If parameters violate SABR constraints.
        """
        ...

    def implied_vol(self, forward: float, strike: float, t: float) -> float:
        """
        Normal or Black implied volatility under the Hagan-2002 expansion.

        Parameters
        ----------
        forward : float
            Forward price at expiry.
        strike : float
            Strike price.
        t : float
            Time to expiry in years.

        Returns
        -------
        float
            Normal decimal rate per square-root year when beta is below 1e-4;
            otherwise dimensionless annual Black volatility.

        Raises
        ------
        ValueError
            If an input is non-finite, ``t`` is non-positive, unshifted
            non-normal SABR receives a non-positive forward or strike, shifted
            SABR has a non-positive effective forward or strike, or the
            calculation produces an invalid volatility.
        """
        ...

    @property
    def params(self) -> SabrParameters:
        """
        Parameters used by this model.

        Returns
        -------
        SabrParameters
            The SABR parameter set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def supports_negative_rates(self) -> bool:
        """
        Return ``True`` for beta approximately zero or a positive shift.

        Returns
        -------
        bool
            ``True`` if beta is below 1e-4 (normal dynamics) or a positive
            displacement enables negative-rate smiles.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.
        """
        ...

class SabrSmile:
    """
    Volatility smile generator for a fixed ``(forward, t)`` pair.

    Sources
    -------
    - Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr

    Examples
    --------
    >>> from finstack_quant.models.volatility import SabrParameters, SabrSmile
    >>> smile = SabrSmile(SabrParameters.equity_default(), 100.0, 1.0)
    >>> (round(smile.atm_vol(), 6), len(smile.generate_smile([90.0, 100.0, 110.0])))
    (0.20081, 3)

    """

    def __init__(
        self,
        params: SabrParameters,
        forward: float,
        t: float,
    ) -> None:
        """
        Create a smile helper for one forward and expiry.

        Parameters
        ----------
        params : SabrParameters
            SABR parameter set.
        forward : float
            Forward price at expiry.
        t : float
            Time to expiry in years.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    def atm_vol(self) -> float:
        """
        Return the ATM implied volatility.

        Returns
        -------
        float
            Normal decimal rate per square-root year when beta is below 1e-4;
            otherwise dimensionless annual Black volatility.

        Raises
        ------
        ValueError
            If expiry or forward/model coordinates are invalid.
        """
        ...

    def implied_vol(self, strike: float) -> float:
        """
        Return implied volatility at ``strike``.

        Parameters
        ----------
        strike : float
            Strike price.

        Returns
        -------
        float
            Implied vol as a decimal.

        Raises
        ------
        ValueError
            If the stored forward, ``strike``, or expiry is outside the model's
            valid domain, or the calculation produces an invalid volatility.
        RuntimeError
            If the native smile calculation returns no value for ``strike``.
        """
        ...

    def generate_smile(self, strikes: list[float]) -> list[float]:
        """
        Return implied volatilities for all supplied strikes.

        Parameters
        ----------
        strikes : list[float]
            Strike grid.

        Returns
        -------
        list[float]
            Implied vols aligned with ``strikes``.

        Raises
        ------
        ValueError
            If the stored forward, any supplied strike, or expiry is outside
            the model's valid domain, or a volatility calculation is invalid.
        """
        ...

    def arbitrage_diagnostics(
        self,
        strikes: list[float],
        r: float = 0.0,
        q: float = 0.0,
    ) -> dict[str, Any]:
        """
        Butterfly + monotonicity arbitrage diagnostics on ``strikes``.

        Returns a dict with ``arbitrage_free``, ``butterfly_violations``,
        and ``monotonicity_violations``.

        Parameters
        ----------
        strikes : list[float]
            Strike grid to test.
        r : float, default 0.0
            Risk-free rate (decimal).
        q : float, default 0.0
            Dividend yield (decimal).

        Returns
        -------
        dict[str, Any]
            Diagnostics dict with ``arbitrage_free``, ``butterfly_violations``,
            and ``monotonicity_violations``.

        Raises
        ------
        ValueError
            If the stored forward, a strike, or expiry is outside the model's
            valid domain, or smile generation produces an invalid volatility.
        """
        ...

    def to_dataframe(self, strikes: list[float]) -> pd.DataFrame:
        """
        Tabulate the smile on a strike grid.

        Parameters
        ----------
        strikes : list[float]
            Strikes at which to evaluate the smile.

        Returns
        -------
        pandas.DataFrame
            Columns ``strike``, ``vol`` (decimal Black vol, or absolute normal
            vol when ``beta == 0``) and ``log_moneyness`` (``ln(K / F)``).

        Raises
        ------
        ValueError
            If any strike or the stored forward / expiry is outside the model
            domain.

        Examples
        --------
        >>> from finstack_quant.models.volatility import SabrParameters, SabrSmile
        >>> smile = SabrSmile(SabrParameters.equity_default(), 100.0, 1.0)
        >>> list(smile.to_dataframe([90.0, 100.0, 110.0]).columns)
        ['strike', 'vol', 'log_moneyness']
        """
        ...

    @property
    def forward(self) -> float:
        """
        Forward this smile is anchored on.

        Returns
        -------
        float
            Forward rate or price in the same units as the strikes passed to the smile; SABR vols are quoted relative to it.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def t(self) -> float:
        """
        Time to expiry in years this smile is anchored on.

        Returns
        -------
        float
            Year fraction from valuation to expiry (decimal years), used as the ``T`` in the SABR expansion.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class SabrCalibrator:
    """
    SABR calibrator (Levenberg-Marquardt with beta fixed).

    Sources
    -------
    - Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr

    Examples
    --------
    >>> from finstack_quant.models.volatility import SabrCalibrator
    >>> calibrator = SabrCalibrator()
    >>> calibrator.with_tolerance(1e-6) is calibrator
    False

    """

    def __init__(self) -> None:
        """
        Create a SABR calibrator with the library default tolerance and iteration cap.

        Use :meth:`high_precision` for a tighter production fit, or
        :meth:`with_tolerance` to override the residual tolerance.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @staticmethod
    def high_precision() -> SabrCalibrator:
        """
        Return a calibrator with tighter tolerance for production fits.

        Returns
        -------
        SabrCalibrator
            Calibrator with high-precision tolerance.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.volatility import SabrCalibrator
        >>> calibrator = SabrCalibrator.high_precision()
        >>> calibrator.with_tolerance(1e-6) is calibrator
        False
        """
        ...

    def with_tolerance(self, tolerance: float) -> SabrCalibrator:
        """
        Return a copy with an overridden maximum relative quote error.

        Parameters
        ----------
        tolerance : float
            Positive finite maximum relative error of any final volatility quote;
            1e-4 permits 0.01% of each quote. Calibration raises ``ValueError``
            for invalid settings and ``RuntimeError`` when no fit meets this budget.

        Returns
        -------
        SabrCalibrator
            New calibrator instance sharing other settings.

        Notes
        -----
        This builder returns a copy with the field set and does not raise.

        """
        ...

    def with_max_iterations(self, max_iterations: int) -> SabrCalibrator:
        """
        Return a copy with an overridden solver iteration cap.

        Parameters
        ----------
        max_iterations : int
            Positive cap on Levenberg-Marquardt iterations before the fit is
            reported as non-converged.

        Returns
        -------
        SabrCalibrator
            New calibrator instance sharing other settings. Does not raise.

        Examples
        --------
        >>> from finstack_quant.models.volatility import SabrCalibrator
        >>> "max_iterations=5000" in repr(SabrCalibrator().with_max_iterations(5000))
        True
        """
        ...

    def calibrate(
        self,
        forward: float,
        strikes: list[float],
        market_vols: list[float],
        t: float,
        beta: float,
    ) -> SabrParameters:
        """
        Fit ``(alpha, nu, rho)`` to market vols with ``beta`` fixed.

        Parameters
        ----------
        forward : float
            Forward at expiry.
        strikes : list[float]
            Strike grid aligned with ``market_vols``.
        market_vols : list[float]
            Positive finite market vols: decimal rate per square-root year for beta=0,
            dimensionless annual Black volatility otherwise.
        t : float
            Expiry in years.
        beta : float
            Fixed SABR beta in ``[0, 1]``.

        Returns
        -------
        SabrParameters
            Calibrated :class:`SabrParameters`.

        Raises
        ------
        ValueError
            If inputs or settings are invalid.
        RuntimeError
            If no fitted candidate satisfies the per-quote error budget.
        """
        ...

    def with_shift(self, shift: float | str | None) -> SabrCalibrator:
        """
        Return a copy with an overridden displacement policy.

        Parameters
        ----------
        shift : float | str | None
            ``None`` fits the quotes as-is; a float is a fixed additive shift in
            the forward's units (decimal rate or price); ``"auto"`` picks the
            smallest standardized shift (1-4%) that leaves 10bp of headroom above
            the most negative forward or strike, or none when every input is
            non-negative. The shift used is stored on the fitted
            :class:`SabrParameters`.

        Returns
        -------
        SabrCalibrator
            New calibrator instance sharing other settings.

        Raises
        ------
        ValueError
            If ``shift`` is neither ``None``, a float, nor ``"auto"``.

        Examples
        --------
        >>> from finstack_quant.models.volatility import SabrCalibrator
        >>> "shift='auto'" in repr(SabrCalibrator().with_shift("auto"))
        True
        """
        ...

    def with_atm_pinning(self, atm_pinning: bool) -> SabrCalibrator:
        """
        Return a copy with exact ATM pinning enabled or disabled.

        Parameters
        ----------
        atm_pinning : bool
            When ``True``, ``alpha`` is solved analytically so the model
            reproduces the ATM volatility interpolated from the quotes exactly,
            and only ``nu`` and ``rho`` are fitted to the smile.

        Returns
        -------
        SabrCalibrator
            New calibrator instance sharing other settings. Does not raise.

        Examples
        --------
        >>> from finstack_quant.models.volatility import SabrCalibrator
        >>> "atm_pinning=True" in repr(SabrCalibrator().with_atm_pinning(True))
        True
        """
        ...

def get_surface_vol(surface: VolSurface, expiry: float, strike: float) -> float:
    """Evaluate a stored surface with checked grid bounds.

    Parameters
    ----------
    surface : VolSurface
        Data-only core surface artifact.
    expiry : float
        Positive option expiry in years inside the grid.
    strike : float
        Secondary-axis coordinate in stored units.

    Returns
    -------
    float
        Interpolated annualized volatility as a decimal.

    Raises
    ------
    ValueError
        If a coordinate is non-finite or outside the checked grid.

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolSurface
    >>> from finstack_quant.models.volatility import get_surface_vol
    >>> get_surface_vol(VolSurface("V", [1.0], [100.0], [0.2]), 1.0, 100.0)
    0.2
    """
    ...

def get_surface_vol_clamped(surface: VolSurface, expiry: float, strike: float) -> float:
    """Evaluate a stored surface with flat coordinate extrapolation.

    Parameters
    ----------
    surface : VolSurface
        Data-only core surface artifact.
    expiry : float
        Finite expiry in years, clamped to the stored axis.
    strike : float
        Finite secondary coordinate, clamped to the stored axis.

    Returns
    -------
    float
        Interpolated volatility, or nan for a non-finite input.

    This function does not raise; invalid finite model outputs are returned as nan.

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolSurface
    >>> from finstack_quant.models.volatility import get_surface_vol_clamped
    >>> get_surface_vol_clamped(VolSurface("V", [1.0], [100.0], [0.2]), 2.0, 120.0)
    0.2
    """
    ...

def get_cube_vol(cube: VolCube, expiry: float, tenor: float, strike: float) -> float:
    """Evaluate Black volatility from a stored SABR cube.

    Parameters
    ----------
    cube : VolCube
        Data-only SABR parameter and forward grid.
    expiry : float
        Positive expiry in years inside the grid.
    tenor : float
        Positive underlying tenor in years inside the grid.
    strike : float
        Finite strike in stored forward units.

    Returns
    -------
    float
        Annualized Black volatility as a decimal.

    Raises
    ------
    ValueError
        If coordinates or SABR evaluation are invalid.

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolCube
    >>> from finstack_quant.models.volatility import get_cube_vol
    >>> c = VolCube("C", [1.0], [5.0], [{"alpha": 0.03, "beta": 0.5, "rho": -0.2, "nu": 0.4}], [0.03])
    >>> get_cube_vol(c, 1.0, 5.0, 0.03) > 0
    True
    """
    ...

def get_cube_vol_clamped(cube: VolCube, expiry: float, tenor: float, strike: float) -> float:
    """Evaluate Black cube volatility with coordinate clamping.

    Parameters
    ----------
    cube : VolCube
        Data-only SABR parameter and forward grid.
    expiry : float
        Finite expiry in years, clamped to the grid.
    tenor : float
        Finite tenor in years, clamped to the grid.
    strike : float
        Finite strike in stored forward units.

    Returns
    -------
    float
        Annualized volatility, or nan when undefined.

    This function does not raise; invalid coordinates or model outputs return nan.

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolCube
    >>> from finstack_quant.models.volatility import get_cube_vol_clamped
    >>> c = VolCube("C", [1.0], [5.0], [{"alpha": 0.03, "beta": 0.5, "rho": -0.2, "nu": 0.4}], [0.03])
    >>> get_cube_vol_clamped(c, 2.0, 10.0, 0.03) > 0
    True
    """
    ...

def get_cube_normal_vol(cube: VolCube, expiry: float, tenor: float, strike: float) -> float:
    """Evaluate normal volatility from a stored SABR cube.
    Returns an **absolute** normal (Bachelier) volatility in the cube's rate
    units (e.g. ``0.0075`` for 75 bp on decimal rates), not a Black decimal vol.


    Parameters
    ----------
    cube : VolCube
        Data-only SABR parameter and forward grid.
    expiry : float
        Positive expiry in years inside the grid.
    tenor : float
        Positive tenor in years inside the grid.
    strike : float
        Finite strike in absolute rate units.

    Returns
    -------
    float
        Annualized normal volatility in absolute rate units.

    Raises
    ------
    ValueError
        If coordinates or the shifted-SABR domain are invalid.

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolCube
    >>> from finstack_quant.models.volatility import get_cube_normal_vol
    >>> c = VolCube("C", [1.0], [5.0], [{"alpha": 0.03, "beta": 0.5, "rho": -0.2, "nu": 0.4}], [0.03])
    >>> get_cube_normal_vol(c, 1.0, 5.0, 0.03) > 0
    True
    """
    ...

def get_cube_normal_vol_clamped(cube: VolCube, expiry: float, tenor: float, strike: float) -> float:
    """Evaluate normal cube volatility with coordinate clamping.

    Parameters
    ----------
    cube : VolCube
        Data-only SABR parameter and forward grid.
    expiry : float
        Finite expiry in years, clamped to the grid.
    tenor : float
        Finite tenor in years, clamped to the grid.
    strike : float
        Finite strike in absolute rate units.

    Returns
    -------
    float
        Normal volatility, or nan when the domain is invalid.

    This function does not raise; invalid coordinates or model outputs return nan.

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolCube
    >>> from finstack_quant.models.volatility import get_cube_normal_vol_clamped
    >>> c = VolCube("C", [1.0], [5.0], [{"alpha": 0.03, "beta": 0.5, "rho": -0.2, "nu": 0.4}], [0.03])
    >>> get_cube_normal_vol_clamped(c, 2.0, 10.0, 0.03) > 0
    True
    """
    ...

def materialize_cube_tenor_slice(cube: VolCube, tenor: float, strikes: list[float]) -> VolSurface:
    """Materialize a lognormal cube tenor slice.

    Parameters
    ----------
    cube : VolCube
        Source SABR cube.
    tenor : float
        Finite tenor in years, clamped to the grid.
    strikes : list[float]
        Non-empty finite strike grid.

    Returns
    -------
    VolSurface
        Data-only expiry-by-strike Black surface.

    Raises
    ------
    ValueError
        If inputs or evaluation are invalid.

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolCube
    >>> from finstack_quant.models.volatility import materialize_cube_tenor_slice
    >>> c = VolCube("C", [1.0], [5.0], [{"alpha": 0.03, "beta": 0.5, "rho": -0.2, "nu": 0.4}], [0.03])
    >>> materialize_cube_tenor_slice(c, 5.0, [0.02, 0.03]).grid_shape
    (1, 2)
    """
    ...

def materialize_cube_tenor_slice_normal(cube: VolCube, tenor: float, strikes: list[float]) -> VolSurface:
    """Materialize a normal-volatility cube tenor slice.

    Parameters
    ----------
    cube : VolCube
        Source SABR cube.
    tenor : float
        Finite tenor in years, clamped to the grid.
    strikes : list[float]
        Non-empty finite strike grid.

    Returns
    -------
    VolSurface
        Data-only expiry-by-strike normal surface.

    Raises
    ------
    ValueError
        If inputs or evaluation are invalid.

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolCube
    >>> from finstack_quant.models.volatility import materialize_cube_tenor_slice_normal
    >>> c = VolCube("C", [1.0], [5.0], [{"alpha": 0.03, "beta": 0.5, "rho": -0.2, "nu": 0.4}], [0.03])
    >>> materialize_cube_tenor_slice_normal(c, 5.0, [0.02, 0.03]).quote_type
    'normal'
    """
    ...

def materialize_cube_expiry_slice(cube: VolCube, expiry: float, strikes: list[float]) -> VolSurface:
    """Materialize a lognormal cube expiry slice.

    Parameters
    ----------
    cube : VolCube
        Source SABR cube.
    expiry : float
        Finite expiry in years, clamped to the grid.
    strikes : list[float]
        Non-empty finite strike grid.

    Returns
    -------
    VolSurface
        Data-only tenor-axis Black surface.

    Raises
    ------
    ValueError
        If inputs or evaluation are invalid.

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolCube
    >>> from finstack_quant.models.volatility import materialize_cube_expiry_slice
    >>> c = VolCube("C", [1.0], [5.0], [{"alpha": 0.03, "beta": 0.5, "rho": -0.2, "nu": 0.4}], [0.03])
    >>> materialize_cube_expiry_slice(c, 1.0, [0.02, 0.03]).grid_shape
    (1, 2)
    """
    ...

def materialize_cube_expiry_slice_normal(cube: VolCube, expiry: float, strikes: list[float]) -> VolSurface:
    """Materialize a normal-volatility cube expiry slice.

    Parameters
    ----------
    cube : VolCube
        Source SABR cube.
    expiry : float
        Finite expiry in years, clamped to the grid.
    strikes : list[float]
        Non-empty finite strike grid.

    Returns
    -------
    VolSurface
        Data-only tenor-axis normal surface.

    Raises
    ------
    ValueError
        If inputs or evaluation are invalid.

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolCube
    >>> from finstack_quant.models.volatility import materialize_cube_expiry_slice_normal
    >>> c = VolCube("C", [1.0], [5.0], [{"alpha": 0.03, "beta": 0.5, "rho": -0.2, "nu": 0.4}], [0.03])
    >>> materialize_cube_expiry_slice_normal(c, 1.0, [0.02, 0.03]).quote_type
    'normal'
    """
    ...

def get_fx_delta_pillar_vols(surface: FxDeltaVolSurface, expiry_index: int) -> tuple[float, float, float]:
    """Recover ATM, 25-delta put, and 25-delta call vols.

    Parameters
    ----------
    surface : FxDeltaVolSurface
        Data-only FX delta quote artifact.
    expiry_index : int
        Zero-based stored expiry index.

    Returns
    -------
    tuple[float, float, float]
        ATM, put, and call annualized decimal volatilities.

    Raises
    ------
    ValueError
        If the index is outside the surface.

    Examples
    --------
    >>> from finstack_quant.core.market_data import FxDeltaVolSurface
    >>> from finstack_quant.models.volatility import get_fx_delta_pillar_vols
    >>> tuple(
    ...     round(value, 6)
    ...     for value in get_fx_delta_pillar_vols(FxDeltaVolSurface("FX", [1.0], [0.12], [0.01], [0.002]), 0)
    ... )
    (0.12, 0.117, 0.127)
    """
    ...

def get_fx_delta_vol(surface: FxDeltaVolSurface, expiry: float, strike: float, forward: float) -> float:
    """Evaluate an FX delta-quoted surface.

    Parameters
    ----------
    surface : FxDeltaVolSurface
        Data-only FX delta quote artifact.
    expiry : float
        Positive option expiry in years.
    strike : float
        Positive strike in FX quote units.
    forward : float
        Positive FX forward in quote units.

    Returns
    -------
    float
        Interpolated annualized Black volatility.

    Raises
    ------
    ValueError
        If inputs or reconstructed wing vols are invalid.

    Examples
    --------
    >>> from finstack_quant.core.market_data import FxDeltaVolSurface
    >>> from finstack_quant.models.volatility import get_fx_delta_vol
    >>> s = FxDeltaVolSurface("FX", [1.0], [0.12], [0.01], [0.002])
    >>> get_fx_delta_vol(s, 1.0, 1.1, 1.1) > 0
    True
    """
    ...

def materialize_fx_delta_surface(
    surface: FxDeltaVolSurface,
    spot: float,
    domestic_rate: float,
    foreign_rate: float,
) -> VolSurface:
    """Materialize FX delta quotes as a strike-axis surface.

    Parameters
    ----------
    surface : FxDeltaVolSurface
        Source FX delta quote artifact.
    spot : float
        Positive spot FX rate.
    domestic_rate : float
        Continuously compounded domestic decimal rate.
    foreign_rate : float
        Continuously compounded foreign decimal rate.

    Returns
    -------
    VolSurface
        Data-only expiry-by-strike surface.

    Raises
    ------
    ValueError
        If market inputs or smile nodes are invalid.

    Examples
    --------
    >>> from finstack_quant.core.market_data import FxDeltaVolSurface
    >>> from finstack_quant.models.volatility import materialize_fx_delta_surface
    >>> s = FxDeltaVolSurface("FX", [1.0], [0.12], [0.01], [0.002])
    >>> materialize_fx_delta_surface(s, 1.1, 0.03, 0.02).grid_shape
    (1, 3)
    """
    ...

def delta_to_strike(delta: float, forward: float, vol: float, expiry: float) -> float:
    """Convert premium-unadjusted forward call delta to strike.

    Parameters
    ----------
    delta : float
        Forward call delta in the open interval from zero to one.
    forward : float
        Positive forward in strike units.
    vol : float
        Positive annualized Black decimal volatility.
    expiry : float
        Positive option expiry in years.

    Returns
    -------
    float
        Strike in forward units.

    This function does not raise; IEEE non-finite results propagate for invalid inputs.

    Examples
    --------
    >>> from finstack_quant.models.volatility import delta_to_strike
    >>> round(delta_to_strike(0.25, 1.1, 0.12, 1.0), 6)
    1.201354
    """
    ...

def strike_to_delta(strike: float, forward: float, vol: float, expiry: float) -> float:
    """Convert strike to premium-unadjusted forward call delta.

    Parameters
    ----------
    strike : float
        Positive strike in forward units.
    forward : float
        Positive forward in strike units.
    vol : float
        Positive annualized Black decimal volatility.
    expiry : float
        Positive option expiry in years.

    Returns
    -------
    float
        Forward call delta as a decimal probability.

    This function does not raise; IEEE non-finite results propagate for invalid inputs.

    Examples
    --------
    >>> from finstack_quant.models.volatility import strike_to_delta
    >>> round(strike_to_delta(1.1, 1.1, 0.12, 1.0), 6)
    0.523922
    """
    ...

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
    def elapsed_us(self) -> int:
        """
        Wall-clock microseconds spent on the check suite (non-deterministic).

        Returns
        -------
        int
            Elapsed run time in microseconds. Non-deterministic: it varies run to run and must not be used in golden comparisons.

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
        Violation counts keyed by severity name, every severity present.

        Returns
        -------
        dict[Literal["negligible", "minor", "major", "critical"], int]
            Counts per severity label; all four keys are always present, with ``0`` where no violation of that severity occurred.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_type(self) -> dict[_ArbitrageTypeName, int]:
        """
        Violation counts keyed by check type, every ``ArbitrageType`` present.

        Returns
        -------
        dict[_ArbitrageTypeName, int]
            Counts per ``ArbitrageType`` name (butterfly, calendar, and so on); every check type is present, with ``0`` where clean.

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

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...

def check_butterfly_grid(
    strikes: list[float],
    expiries: list[float],
    vols: list[list[float]],
    forward_prices: list[float],
    tolerance: float = 1e-6,
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
        Tolerance in total-variance units. Default ``1e-6``.

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
    tolerance: float = 1e-6,
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
        Tolerance in total-variance units. Default ``1e-6``.

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
    tolerance: float = 1e-6,
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
        Tolerance in total-variance units. Default ``1e-6``.

    Returns
    -------
    ArbitrageReport
        Typed report with ``total_violations``, ``passed``, ``by_severity``,
        ``by_type``, ``violations``, ``elapsed_us`` and ``to_dataframe()``.

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

def surface_to_dataframe(surface: VolSurface) -> pd.DataFrame:
    """
    Tabulate a core ``VolSurface`` grid as a DataFrame.

    Parameters
    ----------
    surface : VolSurface
        Data-only surface, e.g. from :func:`materialize_cube_tenor_slice` or
        :func:`materialize_fx_delta_surface`.

    Returns
    -------
    pandas.DataFrame
        Index ``expiry`` (years), one column per strike / secondary-axis
        node, values in the surface's ``quote_type`` units (decimal Black vol
        or absolute normal vol).

    Raises
    ------
    ValueError
        If the stored grid length does not match ``expiries x strikes``.

    Examples
    --------
    >>> from finstack_quant.core.market_data import FxDeltaVolSurface
    >>> from finstack_quant.models.volatility import materialize_fx_delta_surface, surface_to_dataframe
    >>> s = FxDeltaVolSurface("FX", [1.0], [0.12], [0.01], [0.002])
    >>> surface_to_dataframe(materialize_fx_delta_surface(s, 1.1, 0.03, 0.02)).shape
    (1, 3)
    """
    ...

_VolatilityConvention = Literal["normal", "lognormal"] | dict[str, dict[str, float]]

def convert_atm_volatility(
    vol: float,
    from_convention: _VolatilityConvention,
    to_convention: _VolatilityConvention,
    forward_rate: float,
    time_to_expiry: float,
) -> float:
    """
    Convert an ATM volatility quote between normal, lognormal and shifted-lognormal conventions.

    Prices are equated at the money (strike = forward) and the target vol is
    solved deterministically.

    Parameters
    ----------
    vol : float
        Input volatility in the source convention: decimal Black vol for
        ``"lognormal"`` / shifted-lognormal, absolute vol in the forward's rate
        units for ``"normal"`` (``0.0075`` = 75 bp on decimal rates). Positive.
    from_convention : str or dict
        Convention *vol* is quoted in: ``"normal"`` (absolute/Bachelier vol),
        ``"lognormal"`` (decimal Black vol), or
        ``{"shifted_lognormal": {"shift": s}}`` with ``shift`` in the
        forward's rate units (for example ``0.03`` for a 3% shift on decimal
        rates).
    to_convention : str or dict
        Convention the result is returned in; accepts the same three forms as
        *from_convention*. Passing the same convention returns *vol*
        unchanged up to solver tolerance.
    forward_rate : float
        ATM forward rate or price used as both forward and strike; must be
        positive for lognormal conventions, ``forward + shift > 0`` for
        shifted ones.
    time_to_expiry : float
        Time to expiry in years (non-negative); ``0`` returns ``vol`` unchanged.

    Returns
    -------
    float
        Volatility in the target convention.

    Raises
    ------
    ValueError
        If ``vol`` / ``time_to_expiry`` / a convention is invalid, or the
        forward is outside the convention domain.
    RuntimeError
        If the price-matching solver fails to converge.

    Examples
    --------
    >>> from finstack_quant.models.volatility import convert_atm_volatility
    >>> ln = convert_atm_volatility(0.01, "normal", "lognormal", 0.05, 1.0)
    >>> round(convert_atm_volatility(ln, "lognormal", "normal", 0.05, 1.0), 10)
    0.01
    """
    ...

class SviParams:
    """
    Gatheral SVI total-variance smile parameters.

    ``w(k) = a + b * (rho * (k - m) + sqrt((k - m)^2 + sigma^2))`` in
    log-moneyness ``k = ln(K / F)``. Construction validates the
    Gatheral-Jacquier necessary no-arbitrage conditions.

    Examples
    --------
    >>> from finstack_quant.models.volatility import SviParams
    >>> p = SviParams(0.04, 0.1, -0.3, 0.0, 0.2)
    >>> round(p.implied_vol(0.0, 1.0), 6)
    0.244949

    Sources
    -------
    - Gatheral (2004): see docs/REFERENCES.md#gatheral-2004-svi
    - Gatheral-Jacquier (2014): see docs/REFERENCES.md#gatheral-jacquier-2014-svi
    """

    def __init__(self, a: float, b: float, rho: float, m: float, sigma: float) -> None:
        """
        Create validated SVI parameters.

        Parameters
        ----------
        a : float
            Overall total-variance level.
        b : float
            Wing slope (``>= 0``).
        rho : float
            Rotation / asymmetry in ``(-1, 1)``.
        m : float
            Log-moneyness translation of the minimum-variance point.
        sigma : float
            Vertex smoothing (``> 0``).

        Raises
        ------
        ValueError
            If a parameter is non-finite or violates the no-arbitrage
            conditions (``b >= 0``, ``sigma > 0``, ``|rho| < 1``, non-negative
            minimum variance, Roger Lee moment bound).

        Examples
        --------
        >>> from finstack_quant.models.volatility import SviParams
        >>> SviParams(0.04, 0.1, -0.3, 0.0, 0.2).b
        0.1
        """
        ...

    @property
    def a(self) -> float:
        """
        Overall total-variance level.

        Returns
        -------
        float
            SVI ``a``: the vertical shift of the total-variance curve, in total-variance units (``vol^2 * T``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def b(self) -> float:
        """
        Wing slope of the SVI total-variance smile.

        Returns
        -------
        float
            SVI ``b`` (non-negative): controls the slope of both wings in total variance per unit log-moneyness.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rho(self) -> float:
        """
        Rotation / asymmetry.

        Returns
        -------
        float
            SVI ``rho`` in ``(-1, 1)``: the skew parameter rotating the smile; negative values tilt variance toward low strikes.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def m(self) -> float:
        """
        Log-moneyness translation of the minimum-variance point.

        Returns
        -------
        float
            SVI ``m``: horizontal shift in log-moneyness ``k`` locating the smile minimum.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def sigma(self) -> float:
        """
        Vertex smoothing.

        Returns
        -------
        float
            SVI ``sigma`` (positive): curvature at the vertex in log-moneyness units; larger values flatten the smile's minimum.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def total_variance(self, k: float) -> float:
        """
        Total implied variance ``w(k) = vol^2 * T`` at log-moneyness ``k``.

        Parameters
        ----------
        k : float
            Log-moneyness ``ln(K / F)``.

        Returns
        -------
        float
            Dimensionless total variance. Does not raise.
        """
        ...

    def implied_vol(self, k: float, t: float) -> float:
        """
        Black implied volatility ``sqrt(w(k) / t)`` at log-moneyness ``k``.

        Parameters
        ----------
        k : float
            Log-moneyness ``ln(K / F)``.
        t : float
            Time to expiry in years (``> 0``).

        Returns
        -------
        float
            Annualized Black volatility as a decimal.

        Raises
        ------
        ValueError
            If ``t <= 0`` or the total variance at ``k`` is negative.
        """
        ...

    def durrleman_g(self, k: float) -> float:
        """
        Durrleman ``g(k)`` butterfly density function at log-moneyness ``k``.

        Parameters
        ----------
        k : float
            Log-moneyness ``ln(K / F)``.

        Returns
        -------
        float
            ``g(k)``; non-negative everywhere means butterfly-arbitrage-free.
            Does not raise.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON (``a, b, rho, m, sigma``).

        Returns
        -------
        str
            JSON object. Does not raise.
        """
        ...

    @staticmethod
    def from_json(json: str) -> SviParams:
        """
        Deserialize from JSON; validation runs on load.

        Parameters
        ----------
        json : str
            JSON object with ``a, b, rho, m, sigma``.

        Returns
        -------
        SviParams
            Validated parameters.

        Raises
        ------
        ValueError
            On malformed JSON or invalid parameters.

        Examples
        --------
        >>> from finstack_quant.models.volatility import SviParams
        >>> SviParams.from_json(SviParams(0.04, 0.1, -0.3, 0.0, 0.2).to_json()).a
        0.04
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...

def calibrate_svi(
    strikes: list[float],
    vols: list[float],
    forward: float,
    expiry: float,
) -> SviParams:
    """
    Calibrate SVI parameters to a market smile (Gatheral 2004).

    Parameters
    ----------
    strikes : list[float]
        Positive strikes (at least five).
    vols : list[float]
        Black implied vols (decimal) aligned one-for-one with ``strikes``.
    forward : float
        Positive forward at ``expiry``.
    expiry : float
        Positive time to expiry in years.

    Returns
    -------
    SviParams
        Fitted, validated parameters.

    Raises
    ------
    ValueError
        If lengths differ, fewer than five quotes are supplied, an input is
        outside its domain, or the fit violates the no-arbitrage conditions.
    RuntimeError
        If the optimizer fails to converge or the fit RMSE exceeds the
        acceptance threshold.

    Examples
    --------
    >>> from finstack_quant.models.volatility import calibrate_svi
    >>> strikes = [80.0, 90.0, 95.0, 100.0, 105.0, 110.0, 120.0]
    >>> vols = [0.30, 0.25, 0.22, 0.20, 0.21, 0.23, 0.28]
    >>> p = calibrate_svi(strikes, vols, 100.0, 1.0)
    >>> abs(p.implied_vol(0.0, 1.0) - 0.20) < 0.02
    True

    Sources
    -------
    - Gatheral (2004): see docs/REFERENCES.md#gatheral-2004-svi
    """
    ...
