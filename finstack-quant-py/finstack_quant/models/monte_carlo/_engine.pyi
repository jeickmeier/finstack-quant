"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/monte_carlo/engine.rs``."""

from __future__ import annotations

from typing import Any
from finstack_quant.models.monte_carlo._results import MoneyEstimate, PathSummary

def simulate_paths(spec: dict[str, Any] | str) -> PathSummary:
    """
    Simulate paths of any built-in process on a shared time grid.

    Binds Rust ``monte_carlo::simulate::simulate_paths``. The spec is plain
    data validated in Rust: it selects the process, the discretization scheme,
    the time grid and the random streams. The same spec always reproduces the
    same paths bit for bit, on any thread count.

    Parameters
    ----------
    spec : dict or str
        ``PathSimulationSpec`` as a dict or its JSON text, with keys:

        - ``process`` : dict tagged by ``"type"`` plus that process's
          parameters. Rates, yields and volatilities are annualized decimals
          (``0.05`` for 5%); times are in years.

          - ``"gbm"`` : ``r``, ``q``, ``sigma``. State ``[spot]``.
          - ``"gbm_with_dividends"`` : ``params`` (GBM parameters) and
            ``dividends``, a list of ``[time, {"cash": amount}]`` or
            ``[time, {"proportional": fraction}]``. State ``[spot]``.
          - ``"multi_gbm"`` : ``assets`` (list of GBM parameters) and optional
            ``correlation`` (row-major ``n x n``). State ``[spot_0, ...]``.
          - ``"brownian"`` : ``mu``, ``sigma``. State ``[x]``.
          - ``"multi_brownian"`` : ``mus``, ``sigmas``, optional
            ``correlation``. State ``[x_0, ...]``.
          - ``"multi_ou"`` : ``kappas``, ``thetas``, ``sigmas``, optional
            ``correlation``. State ``[x_0, ...]``.
          - ``"hull_white_1f"`` : ``kappa``, ``volatility``
            (``{"times": [...], "values": [...]}``), ``theta_curve``,
            ``theta_times``. State ``[short_rate]``.
          - ``"cir"`` : ``kappa``, ``theta``, ``sigma``. State ``[short_rate]``.
          - ``"cir_plus_plus"`` : ``params`` (CIR parameters),
            ``shift_curve``, ``shift_times``. State ``[short_rate]``.
          - ``"heston"`` : ``r``, ``q``, ``kappa``, ``theta``, ``sigma_v``,
            ``rho``, ``v0``. State ``[spot, variance]``; the starting variance
            must equal ``v0``.
          - ``"schwartz_smith"`` : ``kappa``, ``sigma_x``, ``mu_y``,
            ``sigma_y``, ``rho_xy``, optional ``lambda_x``. State ``[x, y]``.
          - ``"local_vol"`` : ``r``, ``q`` and ``surface``, a Dupire local
            volatility grid ``{"expiries": [...], "strikes": [...],
            "local_vols": [...]}`` (row-major, expiry as the slow axis) such
            as ``json.loads(LocalVolSurface.to_json())`` from
            :mod:`finstack_quant.models.volatility`. State ``[spot]``. Use a
            fine time grid: the scheme freezes the volatility over each step.
          - ``"lmm"`` : ``num_forwards``, ``num_factors`` (2 or 3),
            ``tenors`` (``num_forwards + 1`` year fractions),
            ``accrual_factors``, ``displacements``, ``vol_times``,
            ``vol_values`` (per volatility period, per forward, three factor
            loadings) and ``initial_forwards``. State ``[forward_0, ...]``,
            simple forward rates under the terminal measure;
            ``initial_state`` must equal ``initial_forwards``, and the time
            grid must contain every fixing date and volatility breakpoint
            inside the horizon.
          - ``"rough_bergomi"`` : ``r``, ``q``, ``hurst`` (``{"h": H}`` with
            ``H`` in ``(0, 1)``), ``eta``, ``rho`` and ``xi``, the forward
            variance curve ``{"interpolation": "linear" |
            "constant_intervals", "times": [...], "values": [...]}``. State
            ``[spot]``.
          - ``"rough_heston"`` : ``r``, ``q``, ``hurst`` (``H`` in
            ``(0, 0.5)``), ``kappa``, ``theta``, ``sigma_v``, ``rho``, ``v0``.
            State ``[spot, variance]``; the starting variance must equal
            ``v0``. At most 8,000 steps.
          - ``"cheyette_rough"`` : ``kappa``, ``sigma_base`` (base volatility
            curve, same shape as ``xi``), ``hurst``, ``eta``, ``rho``,
            ``phi_times`` and ``phi_values`` (initial forward curve). State
            ``[x, y]``, normally started at ``[0, 0]``; the short rate is
            ``x`` plus the initial forward rate.

        - ``scheme`` : ``"default"`` (the process's canonical scheme — its
          exact transition where one exists, quadratic-exponential for
          square-root variance, Euler otherwise; used when omitted),
          ``"euler"``, ``"log_euler"`` or ``"milstein"``. ``"log_euler"`` and
          ``"milstein"`` apply to ``"gbm"`` and ``"multi_gbm"`` only, except
          that ``"local_vol"`` also accepts ``"log_euler"`` (its default);
          ``"lmm"``, ``"rough_bergomi"``, ``"rough_heston"`` and
          ``"cheyette_rough"`` accept ``"default"`` only.
        - ``initial_state`` : list of float, the state at time zero in the
          process's state layout.
        - ``time_grid`` : ``{"type": "uniform", "expiry": years, "num_steps": n}``
          or ``{"type": "times", "times": [0.0, ...]}`` with strictly
          increasing year fractions.
        - ``num_paths`` : int in ``[1, 100_000]``, the number of independent
          random streams.
        - ``seed`` : int, root seed of the Philox generator.
        - ``antithetic`` : bool, default ``False``. When ``True`` each stream's
          path is followed by its antithetic partner, driven by the negated
          normal draws.
        - ``fbm`` : dict, optional. Generator of the fractional noise that
          ``"rough_bergomi"`` and ``"cheyette_rough"`` consume:
          ``{"type": "volterra"}`` (used when omitted; the Riemann-Liouville
          Volterra process of the published models, uniform grids only),
          ``{"type": "cholesky"}`` (exact fractional Brownian motion, at most
          8,000 steps) or ``{"type": "windowed_conditional",
          "near_field_size": n}`` (approximate fractional Brownian motion
          with an ``n``-step window; ``n`` optional). The last two give a
          different model with the same forwards.

    Returns
    -------
    PathSummary
        Every simulated state, including the initial state at time zero, in
        stream order.

    Raises
    ------
    ValueError
        If ``spec`` is not valid ``PathSimulationSpec`` data (unknown key or
        tag, missing field, wrong type); a process parameter or correlation
        matrix is out of range; the scheme is not available for the process;
        ``fbm`` is set for a process that does not consume fractional noise;
        ``initial_state`` has the wrong length or lies outside the process's
        domain (for example a non-positive GBM spot); the time grid is
        invalid; ``num_paths`` is outside ``[1, 100_000]``; the output would
        exceed ``64_000_000`` stored values; or a simulated state is
        non-finite.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import simulate_paths
    >>> spec = {
    ...     "process": {"type": "gbm", "r": 0.05, "q": 0.0, "sigma": 0.2},
    ...     "initial_state": [100.0],
    ...     "time_grid": {"type": "uniform", "expiry": 1.0, "num_steps": 2},
    ...     "num_paths": 3,
    ...     "seed": 7,
    ... }
    >>> paths = simulate_paths(spec)
    >>> (paths.num_paths, paths.times, paths.factor_names)
    (3, [0.0, 0.5, 1.0], ['spot'])
    >>> heston = simulate_paths({
    ...     **spec,
    ...     "process": {
    ...         "type": "heston",
    ...         "r": 0.04,
    ...         "q": 0.01,
    ...         "kappa": 2.0,
    ...         "theta": 0.05,
    ...         "sigma_v": 0.3,
    ...         "rho": -0.7,
    ...         "v0": 0.03,
    ...     },
    ...     "initial_state": [100.0, 0.03],
    ...     "antithetic": True,
    ... })
    >>> (heston.num_simulated_paths, heston.factor_names)
    (6, ['spot', 'variance'])
    """
    ...

def heston_satisfies_feller(kappa: float, theta: float, vol_of_vol: float) -> bool:
    """
    Test the inclusive Feller condition ``2 * kappa * theta >= vol_of_vol**2``.

    This is the Monte Carlo engine's own predicate, so the answer at the
    boundary matches :func:`price_heston`.
    Inputs are not validated: non-finite values typically yield ``False``.

    Parameters
    ----------
    kappa : float
        Mean-reversion speed of the variance process per year.
    theta : float
        Long-run variance level in squared-volatility units.
    vol_of_vol : float
        Annualized volatility of the variance process.

    Returns
    -------
    bool
        ``True`` when ``2 * kappa * theta >= vol_of_vol**2``.

    Sources
    -------
    - Heston (1993): see docs/REFERENCES.md#heston-1993

    Notes
    -----
    This helper does not raise; non-finite inputs typically yield ``False``.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import heston_satisfies_feller
    >>> heston_satisfies_feller(2.0, 0.04, 0.3)
    True
    >>> heston_satisfies_feller(1.0, 0.045, 0.3)
    True
    >>> heston_satisfies_feller(1.0, 0.04, 0.5)
    False
    """
    ...

def price_heston(
    spot: float,
    strike: float,
    expiry: float,
    params: dict[str, Any] | str,
    is_call: bool,
    settings: dict[str, Any] | str | None = None,
) -> MoneyEstimate:
    """
    Monte Carlo European option under Heston stochastic volatility.

    Simulates spot and variance with Andersen's quadratic-exponential (QE)
    discretization, which stays stable when the Feller condition is violated.
    Mirrors the Fourier :func:`finstack_quant.models.heston_price`: the same
    market and model inputs, with Monte Carlo settings in place of the
    Fourier settings.

    Parameters
    ----------
    spot : float
        Initial spot price.
    strike : float
        Strike price.
    expiry : float
        Time to maturity in years.
    params : dict or str
        Serde form of the Rust ``HestonPricingParams``, as a dict or JSON text.
        Every key is required: ``r`` (continuously compounded risk-free rate,
        decimal); ``q`` (continuous dividend yield, decimal); ``v0`` (initial
        variance, not volatility); ``kappa`` (mean-reversion speed of
        variance); ``theta`` (long-run variance level); ``sigma_v``
        (volatility of variance, ``sigma`` in Heston notation); ``rho``
        (correlation between spot and variance Brownian motions in
        ``[-1, 1]``).
    is_call : bool
        ``True`` for a call paying ``max(S_T - K, 0)``, ``False`` for a put
        paying ``max(K - S_T, 0)``.
    settings : dict or str, optional
        Serde form of the Rust ``HestonMcSettings``, as a dict or JSON text.
        Every key is optional and an omitted key (or ``settings=None``) takes
        the registry binding default: ``num_paths`` (independent path
        estimators in ``[2, 10_000_000]``, each antithetic pair counts once;
        default ``100_000``); ``seed`` (RNG seed, default ``42``);
        ``num_steps`` (time steps per path, default ``252``); ``currency``
        (ISO currency code stamped on the result).

    Returns
    -------
    MoneyEstimate
        Discounted Monte Carlo price with stderr and confidence bands.

    Raises
    ------
    ValueError
        If ``params`` or ``settings`` has an unknown key or a wrongly typed
        value, ``params`` misses a key, the Heston parameters, expiry, path
        count, step count, or the discount factor fail validation, or a
        simulated discounted payoff is non-finite. Violating the Feller
        condition does not raise.

    Sources
    -------
    - Heston (1993): see docs/REFERENCES.md#heston-1993
    - Andersen QE (2008): see docs/REFERENCES.md#andersen-2008-heston-qe

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import price_heston
    >>> params = {"r": 0.05, "q": 0.0, "v0": 0.04, "kappa": 2.0, "theta": 0.04, "sigma_v": 0.3, "rho": -0.7}
    >>> r = price_heston(100, 100, 1.0, params, True, {"num_paths": 5000})
    >>> r.num_paths
    5000
    >>> put = price_heston(100, 100, 1.0, params, False, {"num_paths": 5000})
    >>> put.mean.amount > 0
    True
    """
    ...
