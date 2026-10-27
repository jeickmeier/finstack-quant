"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/monte_carlo/pricers.rs``."""

from __future__ import annotations
from finstack_quant.models.monte_carlo._results import LrmGreeks, MoneyEstimate

class EuropeanPricer:
    """
    European-option Monte Carlo pricer under GBM (exact time-stepping).

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import EuropeanPricer
    >>> EuropeanPricer(num_paths=1000, seed=1).price_gbm(100, 100, 0.05, 0.0, 0.2, 1.0, True).num_paths
    1000
    """

    def __init__(
        self,
        num_paths: int | None = None,
        seed: int | None = None,
        use_parallel: bool | None = None,
    ) -> None:
        """
        Create a European-option pricer.

        Parameters
        ----------
        num_paths : int, optional
            Independent path count in ``[2, 10_000_000]``. Defaults to the
            registry default (``100_000``); the upper limit is checked when pricing.
        seed : int, optional
            RNG seed. Defaults to the registry default (``42``).
        use_parallel : bool, optional
            Parallel accumulation flag. Defaults to the registry default.

        Raises
        ------
        ValueError
            If the path count is less than two or embedded defaults cannot be loaded.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import EuropeanPricer
        >>> EuropeanPricer(500, 9).seed
        9
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Number of Monte Carlo paths this pricer will simulate.

        Returns
        -------
        int
            Number of Monte Carlo paths.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import EuropeanPricer
        >>> EuropeanPricer(1234).num_paths
        1234
        """
        ...

    @property
    def seed(self) -> int:
        """
        Seed value used for path generation.

        Returns
        -------
        int
            Seed value used for path generation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import EuropeanPricer
        >>> EuropeanPricer(seed=55).seed
        55
        """
        ...

    @property
    def use_parallel(self) -> bool:
        """
        Whether path accumulation runs on the rayon pool.

        Returns
        -------
        bool
            Parallel flag as passed to ``__init__``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def price_gbm(
        self,
        spot: float,
        strike: float,
        rate: float,
        div_yield: float,
        vol: float,
        expiry: float,
        is_call: bool,
        num_steps: int | None = None,
        currency: str | None = None,
    ) -> MoneyEstimate:
        """
        Monte Carlo present value of a European option under exact GBM.

        Parameters
        ----------
        spot : float
            Spot price.
        strike : float
            Strike price.
        rate : float
            Risk-free rate (continuously compounded decimal).
        div_yield : float
            Dividend yield (continuously compounded decimal).
        vol : float
            Volatility (decimal).
        expiry : float
            Time to maturity in years.
        is_call : bool
            ``True`` for a call payoff, ``False`` for a put.
        num_steps : int, optional
            Time steps. Defaults to the registry default (``1``).
            Exact GBM is unbiased for any step size, so a European payoff
            only needs one step.
        currency : str, optional
            ISO currency code. Defaults to USD.

        Returns
        -------
        MoneyEstimate
            Monte Carlo price with stderr and confidence bands.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import EuropeanPricer
        >>> EuropeanPricer(800, 0).price_gbm(100, 100, 0.05, 0.0, 0.2, 1.0, True, num_steps=52).num_paths
        800
        >>> EuropeanPricer(800, 0).price_gbm(100, 100, 0.05, 0.0, 0.2, 1.0, False).mean.amount > 0
        True

        Raises
        ------
        ValueError
            If ``rate`` or ``div_yield`` is non-finite, ``vol`` is negative or non-finite,
            ``expiry`` is non-finite or not strictly positive, ``num_steps`` is zero, the
            configured path count is zero or exceeds ``10_000_000``,
            ``currency`` is unknown, or discounting produces a non-finite value.
        TypeError
            If a non-``None`` ``currency`` is neither a string nor a ``Currency`` instance.

        """
        ...

class PathDependentPricer:
    """
    Path-dependent Monte Carlo pricer (Asian-style exotics on GBM).

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import PathDependentPricer
    >>> PathDependentPricer(600, 2).price_asian(100, 100, 0.05, 0.0, 0.2, 1.0, True).num_paths
    600
    """

    def __init__(
        self,
        num_paths: int | None = None,
        seed: int | None = None,
        use_parallel: bool | None = None,
        antithetic: bool | None = None,
        use_sobol: bool | None = None,
        use_brownian_bridge: bool | None = None,
    ) -> None:
        """
        Create a path-dependent pricer.

        Parameters
        ----------
        num_paths : int, optional
            Path budget of at least two. Without Sobol, counts independent
            estimators, with each antithetic pair counted once; with Sobol,
            counts points across independent scrambles. Defaults to the registry default.
        seed : int, optional
            RNG seed. Defaults to the registry default.
        use_parallel : bool, optional
            Parallel accumulation flag. Defaults to the registry default.
            Incompatible with ``use_sobol=True``.
        antithetic : bool, optional
            Pair each path with its sign-flipped counterpart. Defaults to the
            registry default.
        use_sobol : bool, optional
            Drive paths from a Sobol quasi-random sequence. Enabling Sobol
            also switches on the Brownian bridge unless
            ``use_brownian_bridge`` is passed explicitly. Defaults to the
            registry default.
        use_brownian_bridge : bool, optional
            Brownian-bridge path construction (Sobol only). Defaults to the
            registry default.

        Raises
        ------
        ValueError
            If the path budget is less than two, embedded defaults cannot be loaded, or
            the configuration is inconsistent (``use_sobol`` with
            ``use_parallel``).

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import PathDependentPricer
        >>> PathDependentPricer(100, 1, use_parallel=True).num_paths
        100
        >>> PathDependentPricer(100, 1, use_parallel=False, use_sobol=True).use_brownian_bridge
        True
        """
        ...

    def price_asian(
        self,
        spot: float,
        strike: float,
        rate: float,
        div_yield: float,
        vol: float,
        expiry: float,
        is_call: bool,
        num_steps: int | None = None,
        currency: str | None = None,
    ) -> MoneyEstimate:
        """
        Price an arithmetic Asian option (post-initial fixings at every step).

        Parameters
        ----------
        spot : float
            Spot price.
        strike : float
            Strike price.
        rate : float
            Risk-free rate (continuously compounded decimal).
        div_yield : float
            Dividend yield (continuously compounded decimal).
        vol : float
            Volatility (decimal).
        expiry : float
            Maturity in years.
        is_call : bool
            ``True`` for a call on the arithmetic average, ``False`` for a put.
        num_steps : int, optional
            Steps. Defaults to the registry default. The default fixing
            schedule is steps ``1..=num_steps`` and excludes the initial spot
            at step ``0``.
        currency : str, optional
            ISO currency code. Defaults to USD.

        Returns
        -------
        MoneyEstimate
            Monte Carlo price with stderr and confidence bands.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import PathDependentPricer
        >>> pricer = PathDependentPricer(400, 0)
        >>> pricer.price_asian(100, 100, 0.05, 0.0, 0.2, 1.0, True, num_steps=12).num_paths
        400
        >>> pricer.price_asian(100, 100, 0.05, 0.0, 0.2, 1.0, False, num_steps=12).mean.amount > 0
        True

        Raises
        ------
        ValueError
            If ``rate`` or ``div_yield`` is non-finite, ``vol`` is negative or non-finite,
            ``expiry`` is non-finite or not strictly positive, ``num_steps`` is zero, the
            configured path count is zero or exceeds ``10_000_000``,
            ``currency`` is unknown, or discounting produces a non-finite value.
        TypeError
            If a non-``None`` ``currency`` is neither a string nor a ``Currency`` instance.

        """
        ...

    def price_with_lrm_greeks(
        self,
        spot: float,
        strike: float,
        rate: float,
        div_yield: float,
        vol: float,
        expiry: float,
        is_call: bool,
        num_steps: int | None = None,
        currency: str | None = None,
    ) -> LrmGreeks:
        """
        Price an arithmetic Asian option with likelihood-ratio delta and vega.

        The price, delta and vega come from one set of GBM paths: the Greeks
        weight each discounted payoff by the score of the path density
        (Glasserman 2003, section 7.3), so no bumped re-simulation is needed.
        The payoff is the one :meth:`price_asian`
        price: unit notional, fixings at steps ``1..=num_steps``.

        Parameters
        ----------
        spot : float
            Finite, strictly positive spot price at time zero.
        strike : float
            Strike price in the same units as ``spot``.
        rate : float
            Risk-free rate (continuously compounded decimal).
        div_yield : float
            Dividend yield (continuously compounded decimal).
        vol : float
            Annualized volatility (decimal), strictly positive.
        expiry : float
            Maturity in years.
        is_call : bool
            ``True`` for a call on the arithmetic average, ``False`` for a put.
        num_steps : int, optional
            Time-grid steps, each an averaging date. Defaults to the registry
            value ``convenience.greeks.lrm_num_steps`` (32), not the 252 steps
            of :meth:`price_asian`: every path is kept in memory, so
            ``num_paths * (num_steps + 1)`` may not exceed ``4_000_000``. A
            default pricer (100,000 paths) therefore accepts at most 39 steps.
        currency : str, optional
            ISO currency code. Defaults to USD.

        Returns
        -------
        LrmGreeks
            Price in ``currency``, delta per unit of spot and vega per
            volatility point (``0.01``), each with its standard error and 95%
            confidence interval.

        Raises
        ------
        ValueError
            If ``spot``, ``vol`` or ``expiry`` is not finite and strictly
            positive, ``rate`` or ``div_yield`` is non-finite, ``num_steps``
            is zero, the pricer uses Sobol or antithetic sampling,
            ``num_paths`` exceeds ``100_000``, ``num_paths * (num_steps + 1)``
            exceeds ``4_000_000``, or ``currency`` is unknown.
        TypeError
            If a non-``None`` ``currency`` is neither a string nor a ``Currency`` instance.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import PathDependentPricer
        >>> greeks = PathDependentPricer().price_with_lrm_greeks(100, 100, 0.04, 0.01, 0.25, 1.0, True)
        >>> 0.0 < greeks.delta.mean < 1.0
        True
        >>> greeks.vega.stderr > 0.0
        True
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Number of Monte Carlo paths this pricer will simulate.

        Returns
        -------
        int
            Number of Monte Carlo paths.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import PathDependentPricer
        >>> PathDependentPricer(777).num_paths
        777
        """
        ...

    @property
    def seed(self) -> int:
        """
        Seed value used for path generation.

        Returns
        -------
        int
            Seed value used for path generation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import PathDependentPricer
        >>> PathDependentPricer(seed=44).seed
        44
        """
        ...

    @property
    def use_parallel(self) -> bool:
        """
        Whether path generation runs on the rayon pool.

        Returns
        -------
        bool
            Parallel flag. This accessor does not raise.
        """
        ...

    @property
    def antithetic(self) -> bool:
        """
        Whether each path is paired with its sign-flipped counterpart.

        Returns
        -------
        bool
            Antithetic flag. This accessor does not raise.
        """
        ...

    @property
    def use_sobol(self) -> bool:
        """
        Whether paths are driven by a Sobol quasi-random sequence.

        Returns
        -------
        bool
            Sobol flag. This accessor does not raise.
        """
        ...

    @property
    def use_brownian_bridge(self) -> bool:
        """
        Whether Brownian-bridge construction is enabled (Sobol only).

        Returns
        -------
        bool
            Brownian-bridge flag. This accessor does not raise.
        """
        ...

class LsmcPricer:
    """
    Longstaff–Schwartz Monte Carlo pricer for Bermudan options under GBM.

    Exercise is decided on the discrete grid ``1..=num_steps``, not as a
    continuous American. Immediate exercise at valuation (``t = 0``) floors
    the reported price at intrinsic.

    Independent paths must be in ``[1, 10_000_000]`` and time steps in
    ``[1, 100_000]``. Each pricing pass retains at most ``64_000_000`` spot
    values, including time zero and both rows of every antithetic pair:
    ``num_paths * (2 if antithetic else 1) * (num_steps + 1)``. Construction
    checks the storage limit without antithetic partners; pricing checks
    the selected pairing before allocating paths.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import LsmcPricer
    >>> LsmcPricer(300, 0, num_steps=10).price_american(100, 100, 0.05, 0.0, 0.3, 1.0, False).num_paths
    300
    """

    def __init__(
        self,
        num_paths: int | None = None,
        seed: int | None = None,
        use_parallel: bool | None = None,
        num_steps: int | None = None,
        basis: str | None = None,
        basis_degree: int | None = None,
        antithetic: bool | None = None,
    ) -> None:
        """
        Create a Longstaff–Schwartz Monte Carlo pricer for early exercise.

        Parameters
        ----------
        num_paths : int, optional
            Independent path count in ``[2, 10_000_000]``. Defaults to the
            registry default; subject to the retained-storage limit above.
        seed : int, optional
            RNG seed. Defaults to the registry default.
        use_parallel : bool, optional
            Parallel path generation flag. Defaults to the registry default.
        num_steps : int, optional
            Time-grid steps and exercise dates in ``[1, 100_000]``. Defaults
            to the registry value; subject to the retained-storage limit above.
        basis : str, optional
            Regression basis family. One of ``"laguerre"``,
            ``"polynomial"``, or ``"normalized_polynomial"``. Defaults to
            the registry default.
        basis_degree : int, optional
            Polynomial/Laguerre degree. Defaults to the registry default.
            Must be positive; for ``"laguerre"`` it must additionally be
            in ``[1, 4]``.
        antithetic : bool, optional
            Pair each path with its sign-flipped counterpart (``Z`` and
            ``-Z``). Defaults to the registry default (``True``). Both rows
            count toward the retained-storage limit when pricing.

        Raises
        ------
        ValueError
            If ``basis`` is not a recognized family or ``basis_degree`` is
            out of range; the independent path or step count is outside the
            documented range; or the paths without antithetic partners exceed
            ``64_000_000`` retained spot values including time zero.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import LsmcPricer
        >>> LsmcPricer(50, 3).num_paths
        50
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Number of Monte Carlo paths this pricer will simulate.

        Returns
        -------
        int
            Number of Monte Carlo paths.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def seed(self) -> int:
        """
        Seed value used for path generation.

        Returns
        -------
        int
            Seed value used for path generation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def use_parallel(self) -> bool:
        """
        Whether path generation runs on the rayon pool.

        Returns
        -------
        bool
            Parallel flag as passed to ``__init__``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def antithetic(self) -> bool:
        """
        Whether each path is paired with its sign-flipped counterpart.

        Returns
        -------
        bool
            Antithetic flag as passed to ``__init__`` or the registry default.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def basis(self) -> str:
        """
        Regression basis family name.

        Returns
        -------
        str
            One of ``"laguerre"``, ``"polynomial"``,
            ``"normalized_polynomial"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def basis_degree(self) -> int:
        """
        Configured polynomial/Laguerre degree.

        Returns
        -------
        int
            Degree value used in the regression basis.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def price_american(
        self,
        spot: float,
        strike: float,
        rate: float,
        div_yield: float,
        vol: float,
        expiry: float,
        is_call: bool,
        currency: str | None = None,
        num_steps: int | None = None,
        basis: str | None = None,
        basis_degree: int | None = None,
    ) -> MoneyEstimate:
        """
        Price a Bermudan option via LSMC on the grid ``1..=num_steps``.

        Immediate exercise at valuation floors the reported price at
        ``max(strike - spot, 0)``. If that floor binds, stderr and the 95%
        CI collapse to the intrinsic value.

        Parameters
        ----------
        spot : float
            Spot price.
        strike : float
            Strike price.
        rate : float
            Risk-free rate (continuously compounded decimal).
        div_yield : float
            Dividend yield (continuously compounded decimal).
        vol : float
            Volatility (decimal).
        expiry : float
            Maturity in years.
        is_call : bool
            ``True`` for a call, ``False`` for a put.
        currency : str, optional
            ISO currency code. Defaults to USD.
        num_steps : int, optional
            Per-call exercise-grid override in ``[1, 100_000]``; defaults to
            the instance ``num_steps``. Subject to the retained-storage limit
            described by :class:`LsmcPricer`.
        basis : str, optional
            Per-call override of the regression basis family; defaults to
            the instance ``basis``.
        basis_degree : int, optional
            Per-call override of the basis degree; defaults to the instance
            ``basis_degree``.

        Returns
        -------
        MoneyEstimate
            LSMC price with stderr and confidence bands.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import LsmcPricer
        >>> LsmcPricer(200, 0, num_steps=8).price_american(100, 100, 0.05, 0.0, 0.25, 1.0, False).num_paths
        200

        Raises
        ------
        ValueError
            If ``strike`` is non-finite or non-positive, or is no greater than ``1e-14``
            with the normalized-polynomial basis; ``rate`` or ``div_yield``
            is non-finite; ``vol`` is negative or non-finite; ``expiry`` is
            non-finite or not strictly positive;
            the time-step or independent path count is outside the documented
            range; retained spots exceed ``64_000_000`` values including time
            zero and both rows of each antithetic pair; or ``currency`` is unknown.
        TypeError
            If a non-``None`` ``currency`` is neither a string nor a ``Currency`` instance.

        """
        ...

    def price_american_unbiased(
        self,
        spot: float,
        strike: float,
        rate: float,
        div_yield: float,
        vol: float,
        expiry: float,
        is_call: bool,
        pricing_seed: int,
        currency: str | None = None,
        num_steps: int | None = None,
        basis: str | None = None,
        basis_degree: int | None = None,
    ) -> MoneyEstimate:
        """
        Two-pass unbiased American option price.

        Mitigates the in-sample upward bias of single-pass LSMC by fitting
        the regression on a training path set seeded by the pricer's ``seed``
        and pricing on an independent path set seeded by ``pricing_seed``.

        Parameters
        ----------
        spot : float
            Spot price.
        strike : float
            Strike price.
        rate : float
            Risk-free rate (continuously compounded decimal).
        div_yield : float
            Dividend yield (continuously compounded decimal).
        vol : float
            Volatility (decimal).
        expiry : float
            Maturity in years.
        is_call : bool
            ``True`` for a call, ``False`` for a put.
        pricing_seed : int
            Seed for the pricing pass; must differ from the pricer's training
            seed (passing the same value reintroduces the in-sample bias and
            is rejected).
        currency : str, optional
            ISO currency code. Defaults to USD.
        num_steps : int, optional
            Per-call exercise-grid override in ``[1, 100_000]``; defaults to
            the instance ``num_steps``. Subject to the retained-storage limit
            described by :class:`LsmcPricer`.
        basis : str, optional
            Per-call override of the regression basis family; defaults to
            the instance ``basis``.
        basis_degree : int, optional
            Per-call override of the basis degree; defaults to the instance
            ``basis_degree``.

        Returns
        -------
        MoneyEstimate
            Out-of-sample price with stderr and confidence bands.

        Raises
        ------
        ValueError
            If ``pricing_seed`` equals the pricer's training seed; the time-step
            or independent path count is outside the documented range; or each
            pass exceeds ``64_000_000`` retained spot values including time zero
            and both rows of each antithetic pair. The input validation errors
            documented by :meth:`price_american` also apply.
        """
        ...
