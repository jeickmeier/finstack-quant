"""Stub declarations mirroring ``finstack-quant-py/src/bindings/margin/xva.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
from finstack_quant.core.market_data import DiscountCurve, HazardCurve
from finstack_quant.margin._im import SimmCalculator, SimmSensitivities

class FundingConfig:
    """
    Funding cost/benefit configuration for FVA and MVA calculation.

    Supplying ``im_profile`` turns on MVA: :func:`compute_bilateral_xva` then
    prices the funding cost of posted initial margin and includes it in
    ``XvaResult.total_xva``.

    Parameters
    ----------
    funding_spread_bp : float
        Non-negative finite funding cost spread in basis points.
    funding_benefit_bp : float | None, optional
        Non-negative finite funding benefit in bp, no greater than the cost
        spread; ``None`` for symmetric funding.
    im_profile : ImProfile | None, optional
        Valid expected initial-margin profile driving MVA; ``None`` disables MVA.
    margin_funding_spread_bp : float | None, optional
        Non-negative finite spread applied to posted IM; ``None`` reuses
        ``funding_spread_bp``.

    Returns
    -------
    FundingConfig
        Funding parameters.

    Raises
    ------
    ValueError
        If a spread is negative or non-finite, the benefit exceeds the funding
        cost, or ``im_profile`` is invalid.

    Examples
    --------
    >>> FundingConfig(50.0, None).funding_spread_bp
    50.0
    """

    def __init__(
        self,
        funding_spread_bp: float,
        funding_benefit_bp: float | None = None,
        im_profile: ImProfile | None = None,
        margin_funding_spread_bp: float | None = None,
    ) -> None:
        """
        Initialize FVA and MVA funding parameters.

        Parameters
        ----------
        funding_spread_bp : float
            Non-negative finite funding cost spread in basis points.
        funding_benefit_bp : float | None
            Non-negative finite funding benefit in basis points, no greater
            than the cost spread; ``None`` uses the funding cost spread.
        im_profile : ImProfile | None
            Valid expected initial-margin profile driving MVA, or ``None``.
        margin_funding_spread_bp : float | None
            Non-negative finite IM funding spread in bp, or ``None`` to reuse
            ``funding_spread_bp``.

        Raises
        ------
        ValueError
            If a spread is negative or non-finite, the benefit exceeds the
            funding cost, or ``im_profile`` is invalid.
        """
        ...

    @staticmethod
    def from_json(json: str) -> FundingConfig:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical JSON for a ``FundingConfig``.

        Returns
        -------
        FundingConfig
            Parsed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> original = FundingConfig(50.0, 30.0)
        >>> FundingConfig.from_json(original.to_json()).to_json() == original.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON accepted by ``from_json``.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(FundingConfig(50.0, 30.0).to_json(), str)
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @property
    def funding_spread_bp(self) -> float:
        """
        Funding spread in basis points.

        Returns
        -------
        float
            Spread in bp.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> FundingConfig(10.0).funding_spread_bp
        10.0
        """
        ...

    @property
    def funding_benefit_bp(self) -> float | None:
        """
        Funding benefit spread in basis points (or None).

        Returns
        -------
        float or None
            Benefit bp if asymmetric.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> FundingConfig(10.0, 8.0).funding_benefit_bp
        8.0
        """
        ...

    @property
    def im_profile(self) -> ImProfile | None:
        """
        Expected initial-margin profile driving MVA (or None).

        Returns
        -------
        ImProfile or None
            The IM profile when MVA is enabled.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> FundingConfig(10.0).im_profile is None
        True
        """
        ...

    @property
    def margin_funding_spread_bp(self) -> float | None:
        """
        IM funding spread in basis points (or None).

        Returns
        -------
        float or None
            Explicit IM funding spread, when overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> FundingConfig(10.0, None, None, 6.0).margin_funding_spread_bp
        6.0
        """
        ...

    def effective_margin_spread_bp(self) -> float:
        """
        Effective IM funding spread in basis points.

        Falls back to ``funding_spread_bp`` when
        ``margin_funding_spread_bp`` is ``None``.

        Returns
        -------
        float
            Effective IM funding spread in bp.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> FundingConfig(10.0).effective_margin_spread_bp()
        10.0
        """
        ...

    def effective_benefit_bp(self) -> float:
        """
        Effective funding benefit spread in basis points.

        Returns
        -------
        float
            Effective benefit bp.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> isinstance(FundingConfig(1.0).effective_benefit_bp(), float)
        True
        """
        ...

    def __repr__(self) -> str: ...

class ExposureDiagnostics:
    """
    Diagnostics from exposure simulation.

    Counters an exposure engine attaches to an ``ExposureProfile`` (via its
    ``diagnostics`` argument): how many market-roll and valuation failures
    occurred over how many time points.

    Parameters
    ----------
    market_roll_failures : int, default 0
        Number of market-roll failures.
    valuation_failures : int, default 0
        Total instrument valuation failures.
    total_time_points : int, default 0
        Total time grid points evaluated.

    Returns
    -------
    ExposureDiagnostics
        Counters for simulation health.

    Examples
    --------
    >>> ExposureDiagnostics(valuation_failures=2, total_time_points=40).valuation_failures
    2
    """

    def __init__(
        self,
        market_roll_failures: int = 0,
        valuation_failures: int = 0,
        total_time_points: int = 0,
    ) -> None:
        """
        Create a diagnostics record from its three counters.

        Parameters
        ----------
        market_roll_failures : int, default 0
            Number of market-roll failures.
        valuation_failures : int, default 0
            Total instrument valuation failures.
        total_time_points : int, default 0
            Total time grid points evaluated.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ExposureDiagnostics:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical JSON for a ``ExposureDiagnostics``.

        Returns
        -------
        ExposureDiagnostics
            Parsed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> original = ExposureDiagnostics(1, 2, 3)
        >>> ExposureDiagnostics.from_json(original.to_json()).to_json() == original.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON accepted by ``from_json``.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(ExposureDiagnostics(1, 2, 3).to_json(), str)
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @property
    def market_roll_failures(self) -> int:
        """
        Number of market-roll failures.

        Returns
        -------
        int
            Failure count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def valuation_failures(self) -> int:
        """
        Total instrument valuation failures.

        Returns
        -------
        int
            Failure count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_time_points(self) -> int:
        """
        Total time grid points evaluated.

        Returns
        -------
        int
            Point count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...

class ExposureProfile:
    """
    Exposure profile at each time grid point.

    Parameters
    ----------
    times : list[float]
        Exposure or IM observation times in years from the valuation date.
    mtm_values : list[float]
        Portfolio MtM at each time.
    epe : list[float]
        Expected positive exposure series.
    ene : list[float]
        Expected negative exposure series.
    diagnostics : ExposureDiagnostics | None, optional
        Engine failure counters to attach; ``None`` when built by hand.

    Returns
    -------
    ExposureProfile
        Profile vectors.

    Examples
    --------
    >>> p = ExposureProfile([0.0, 1.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0])
    >>> len(p)
    2
    """

    def __init__(
        self,
        times: list[float],
        mtm_values: list[float],
        epe: list[float],
        ene: list[float],
        diagnostics: ExposureDiagnostics | None = None,
    ) -> None:
        """
        Create aligned MtM, EPE, and ENE vectors on an exposure time grid.

        Parameters
        ----------
        times : list[float]
            Finite, nonnegative, strictly increasing times in years. An explicit
            zero-time point is allowed. Before the first point, XVA holds
            its exposure constant; it does not assume zero opening exposure.
        mtm_values : list[float]
            Portfolio mark-to-market amounts at the corresponding times.
        epe : list[float]
            Expected positive exposure amounts at the corresponding times.
        ene : list[float]
            Expected negative exposure amounts at the corresponding times.
        diagnostics : ExposureDiagnostics | None, optional
            Engine failure counters to attach.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied and
        checked by ``validate()`` or the XVA entry points.
        """
        ...

    @staticmethod
    def from_dataframe(frame: pd.DataFrame) -> ExposureProfile:
        """
        Build a profile from the frame ``to_dataframe`` emits.

        Parameters
        ----------
        frame : pd.DataFrame
            Columns ``mtm_values``, ``epe``, ``ene`` indexed by time in
            years.

        Returns
        -------
        ExposureProfile
            Profile with the index as ``times`` and no diagnostics.

        Raises
        ------
        ValueError
            If a column is missing or non-numeric.
        TypeError
            If ``frame`` is not a pandas ``DataFrame``.

        Examples
        --------
        >>> original = ExposureProfile([0.5, 1.0], [1.0, 2.0], [1.0, 2.0], [0.0, 0.0])
        >>> ExposureProfile.from_dataframe(original.to_dataframe()).epe
        [1.0, 2.0]
        """
        ...

    @property
    def diagnostics(self) -> ExposureDiagnostics | None:
        """
        Engine diagnostics attached to the profile.

        Returns
        -------
        ExposureDiagnostics or None
            Counters supplied at construction (or carried through
            ``from_json``), else ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> p = ExposureProfile([1.0], [0.0], [0.0], [0.0], ExposureDiagnostics(0, 1, 1))
        >>> p.diagnostics.valuation_failures
        1
        """
        ...

    @staticmethod
    def from_json(json: str) -> ExposureProfile:
        """
        Parse this object from a JSON object or JSON string.

        Parameters
        ----------
        json : str
            JSON string.

        Returns
        -------
        ExposureProfile
            Parsed profile.

        Raises
        ------
        ValueError
            Invalid JSON.

        Examples
        --------
        >>> original = ExposureProfile([0.0, 1.0], [0.0, 2.0], [0.0, 2.0], [0.0, 0.0])
        >>> restored = ExposureProfile.from_json(original.to_json())
        >>> restored.mtm_values
        [0.0, 2.0]
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this object to a JSON-compatible dict.

        Returns
        -------
        str
            Pretty-printed JSON.

        Raises
        ------
        ValueError
            Serialization error.

        Examples
        --------
        >>> '"times"' in ExposureProfile([1.0], [0.0], [0.0], [0.0]).to_json()
        True
        """
        ...

    def validate(self) -> None:
        """
        Validate internal consistency.

        Returns
        -------
        None
            Returns ``None`` when the vectors are consistent.

        Raises
        ------
        ValueError
            If the profile is empty, lengths differ, times are not strictly
            increasing and positive, or an amount is non-finite.

        Examples
        --------
        >>> ExposureProfile([1.0], [0.0], [0.0], [0.0]).validate()
        """
        ...

    @property
    def times(self) -> list[float]:
        """
        Exposure or IM observation times in years from the valuation date.

        Returns
        -------
        list[float]
            Times.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ExposureProfile([0.0, 1.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]).times
        [0.0, 1.0]
        """
        ...

    @property
    def mtm_values(self) -> list[float]:
        """
        Portfolio MtM values at each time point.

        Returns
        -------
        list[float]
            MtM path.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ExposureProfile([0.0], [1.0], [0.0], [0.0]).mtm_values
        [1.0]
        """
        ...

    @property
    def epe(self) -> list[float]:
        """
        Expected Positive Exposure at each time point.

        Returns
        -------
        list[float]
            EPE series.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ExposureProfile([0.0], [0.0], [2.0], [0.0]).epe
        [2.0]
        """
        ...

    @property
    def ene(self) -> list[float]:
        """
        Expected Negative Exposure at each time point.

        Returns
        -------
        list[float]
            ENE series.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ExposureProfile([0.0], [0.0], [0.0], [1.0]).ene
        [1.0]
        """
        ...

    def __len__(self) -> int:
        """Number of time points.

        Returns
        -------
        int
            Length of time grid.

        Examples
        --------
        >>> len(ExposureProfile([0.0, 1.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]))
        2
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a pandas DataFrame with time (years) as index.

        Columns: ``mtm_values``, ``epe``, ``ene``; ``from_dataframe`` accepts
        this frame back.

        Returns
        -------
        pd.DataFrame
            Exposure profile as a DataFrame.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str: ...

class XvaResult:
    """
    Result of XVA calculations (CVA, DVA, FVA, MVA, exposure profiles).

    Adjustments compose as ``total_xva = CVA - DVA + FVA + MVA``; uncomputed
    optional legs contribute zero.

    Parameters
    ----------
    (Produced by XVA engine; also loadable via ``from_json``.)

    Returns
    -------
    XvaResult
        XVA amounts and profiles.

    Examples
    --------
    >>> doc = (
    ...     '{"cva":1.0,"total_xva":1.0,"epe_profile":[[0.0,2.0]],'
    ...     '"ene_profile":[[0.0,0.0]],"max_epe":2.0,'
    ...     '"effective_epe_profile":[[0.0,2.0]],"effective_epe":2.0}'
    ... )
    >>> result = XvaResult.from_json(doc)
    >>> (result.cva, result.total_xva)
    (1.0, 1.0)
    """

    @staticmethod
    def from_json(json: str) -> XvaResult:
        """
        Parse this object from a JSON object or JSON string.

        Parameters
        ----------
        json : str
            JSON string.

        Returns
        -------
        XvaResult
            Parsed result.

        Raises
        ------
        ValueError
            Invalid JSON.

        Examples
        --------
        >>> doc = (
        ...     '{"cva":1.0,"total_xva":1.0,"epe_profile":[[0.0,2.0]],'
        ...     '"ene_profile":[[0.0,0.0]],"max_epe":2.0,'
        ...     '"effective_epe_profile":[[0.0,2.0]],"effective_epe":2.0}'
        ... )
        >>> result = XvaResult.from_json(doc)
        >>> (result.max_epe, result.effective_epe)
        (2.0, 2.0)
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this object to a JSON-compatible dict.

        Returns
        -------
        str
            Pretty-printed JSON.

        Raises
        ------
        ValueError
            Serialization error.

        """
        ...

    @property
    def cva(self) -> float:
        """
        Unilateral CVA (positive = cost).

        Returns
        -------
        float
            CVA amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def dva(self) -> float | None:
        """
        DVA (own-default benefit, or None).

        Returns
        -------
        float or None
            DVA if computed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fva(self) -> float | None:
        """
        FVA (net funding cost/benefit, or None).

        Returns
        -------
        float or None
            FVA if computed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mva(self) -> float | None:
        """
        MVA (funding cost of posted initial margin, or None).

        ``None`` unless the run supplied ``FundingConfig.im_profile``.

        Returns
        -------
        float or None
            MVA if computed; positive is a cost to the desk.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_xva(self) -> float:
        """
        All-in adjustment = CVA − DVA + FVA + MVA.

        Uncomputed legs contribute zero. This is the quantity subtracted from
        the risk-free value of the netting set.

        Returns
        -------
        float
            Total XVA.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def max_epe(self) -> float:
        """
        Maximum expected positive exposure across the profile, in reporting currency.

        Returns
        -------
        float
            Maximum expected positive exposure across the profile, in reporting currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def effective_epe(self) -> float:
        """
        Effective EPE (time-weighted average, regulatory metric).

        Returns
        -------
        float
            Effective EPE.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def epe_profile(self) -> list[tuple[float, float]]:
        """
        EPE profile as list of (time, value) tuples.

        Returns
        -------
        list[tuple[float, float]]
            (time, EPE) pairs.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def ene_profile(self) -> list[tuple[float, float]]:
        """
        ENE profile as list of (time, value) tuples.

        Returns
        -------
        list[tuple[float, float]]
            (time, ENE) pairs.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def effective_epe_profile(self) -> list[tuple[float, float]]:
        """
        Effective EPE profile as list of (time, value) tuples.

        Returns
        -------
        list[tuple[float, float]]
            (time, effective EPE) pairs.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cva_rows(self) -> list[dict[str, float]]:
        """
        Per-time-bucket terms of the CVA sum.

        Returns
        -------
        list[dict[str, float]]
            One dict per exposure-profile node, in time order, with ``time``
            (years), ``discount_factor``, ``survival_probability`` (of the
            counterparty), ``marginal_default_probability``, ``exposure_mid``
            (bucket-average EPE), ``discount_factor_mid``, ``survival_weight``
            (bucket-average own survival; ``1.0`` when unilateral),
            ``loss_given_default`` and ``contribution``. Each contribution is
            ``loss_given_default * exposure_mid * marginal_default_probability
            * discount_factor_mid * survival_weight``; the contributions sum
            to ``cva``. Empty only for a result parsed from JSON written
            before the field existed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> doc = (
        ...     '{"cva":1.0,"total_xva":1.0,"epe_profile":[[0.0,2.0]],'
        ...     '"ene_profile":[[0.0,0.0]],"max_epe":2.0,'
        ...     '"effective_epe_profile":[[0.0,2.0]],"effective_epe":2.0}'
        ... )
        >>> XvaResult.from_json(doc).cva_rows
        []
        """
        ...

    @property
    def dva_rows(self) -> list[dict[str, float]] | None:
        """
        Per-time-bucket terms of the DVA sum.

        Returns
        -------
        list[dict[str, float]] | None
            ``None`` when ``dva`` is ``None``. Otherwise rows with the same keys
            as ``cva_rows``, where the defaulting party is the institution,
            ``exposure_mid`` is the bucket-average ENE net of posted initial
            margin and ``survival_weight`` is the counterparty's bucket-average
            survival. The contributions sum to ``dva``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> doc = (
        ...     '{"cva":1.0,"total_xva":1.0,"epe_profile":[[0.0,2.0]],'
        ...     '"ene_profile":[[0.0,0.0]],"max_epe":2.0,'
        ...     '"effective_epe_profile":[[0.0,2.0]],"effective_epe":2.0}'
        ... )
        >>> XvaResult.from_json(doc).dva_rows is None
        True
        """
        ...

    @property
    def fva_rows(self) -> list[dict[str, float]] | None:
        """
        Per-time-bucket terms of the FVA sum.

        Returns
        -------
        list[dict[str, float]] | None
            ``None`` when ``fva`` is ``None``. Otherwise rows with ``time`` and
            ``dt`` (years), ``discount_factor``, ``epe_mid``, ``ene_mid``,
            ``discount_factor_mid``, ``joint_survival_mid``,
            ``funding_spread`` and ``funding_benefit_spread`` (decimal per
            year) and ``contribution`` =
            ``(epe_mid * funding_spread - ene_mid * funding_benefit_spread)
            * discount_factor_mid * dt * joint_survival_mid``. The
            contributions sum to ``fva``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> doc = (
        ...     '{"cva":1.0,"total_xva":1.0,"epe_profile":[[0.0,2.0]],'
        ...     '"ene_profile":[[0.0,0.0]],"max_epe":2.0,'
        ...     '"effective_epe_profile":[[0.0,2.0]],"effective_epe":2.0}'
        ... )
        >>> XvaResult.from_json(doc).fva_rows is None
        True
        """
        ...

    @property
    def meta(self) -> dict[str, Any]:
        """
        Policy metadata stamped by the computing layer.

        Returns
        -------
        dict[str, Any]
            ``numeric_mode``, ``rounding`` (the active rounding context),
            ``fx_policy_applied`` (or ``None``), ``parallel`` and
            ``timestamp``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> doc = (
        ...     '{"cva":1.0,"total_xva":1.0,"epe_profile":[[0.0,2.0]],'
        ...     '"ene_profile":[[0.0,0.0]],"max_epe":2.0,'
        ...     '"effective_epe_profile":[[0.0,2.0]],"effective_epe":2.0}'
        ... )
        >>> "numeric_mode" in XvaResult.from_json(doc).meta
        True
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the XVA components as a single-row pandas DataFrame.

        Columns: ``cva``, ``dva``, ``fva``, ``mva``, ``total_xva``,
        ``max_epe``, ``effective_epe`` -- all in the netting set's currency
        units, matching the properties of the same name. Uncomputed legs
        (``dva`` / ``fva`` / ``mva``) are ``NaN`` rather than absent, so the
        frame keeps its schema across netting sets.

        This is the default export; the time-indexed exposure profiles are a
        separate table -- see :meth:`to_profiles_dataframe`.

        Returns
        -------
        pd.DataFrame
            Single-row DataFrame, so a portfolio of netting sets stacks with
            ``pd.concat``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_profiles_dataframe(self) -> pd.DataFrame:
        """
        Export exposure profiles as a pandas DataFrame.

        Columns: ``epe``, ``ene``, ``effective_epe`` -- indexed
        by time in years.

        Returns
        -------
        pd.DataFrame
            Profile DataFrame.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str: ...

class ImDecayProfile:
    """
    Deterministic IM decay profile for MVA (Green 2015, ch. 10).

    Approximates the expected initial-margin path ``E[IM(t)] = IM(0) *
    factor(t)`` used by :func:`im_profile_from_simm`. Constructed via the
    static factories below; there is no direct constructor.

    Parameters
    ----------
    (Use ``constant()``, ``linear_to_maturity()``, or ``sqrt_time()``.)

    Returns
    -------
    ImDecayProfile
        Decay profile applied to today's IM.

    Examples
    --------
    >>> ImDecayProfile.constant().factor(5.0)
    1.0
    """

    @staticmethod
    def constant() -> ImDecayProfile:
        """
        IM stays at today's level for the whole horizon.

        Returns
        -------
        ImDecayProfile
            Decay profile with ``factor(t) == 1`` for all ``t``.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> ImDecayProfile.constant().factor(10.0)
        1.0
        """
        ...

    @staticmethod
    def linear_to_maturity(maturity_years: float) -> ImDecayProfile:
        """
        IM decays linearly to zero at ``maturity_years``.

        Parameters
        ----------
        maturity_years : float
            Portfolio maturity ``T`` in years; must be positive and finite.

        Returns
        -------
        ImDecayProfile
            Decay profile with ``factor(t) = max(1 - t/T, 0)``.

        Raises
        ------
        ValueError
            If ``maturity_years`` is non-positive or non-finite.

        Examples
        --------
        >>> ImDecayProfile.linear_to_maturity(2.0).factor(1.0)
        0.5
        """
        ...

    @staticmethod
    def sqrt_time(maturity_years: float) -> ImDecayProfile:
        """
        IM decays like the square root of remaining time to ``maturity_years``.

        Parameters
        ----------
        maturity_years : float
            Portfolio maturity ``T`` in years; must be positive and finite.

        Returns
        -------
        ImDecayProfile
            Decay profile with ``factor(t) = sqrt(max(1 - t/T, 0))``.

        Raises
        ------
        ValueError
            If ``maturity_years`` is non-positive or non-finite.

        Examples
        --------
        >>> round(ImDecayProfile.sqrt_time(2.0).factor(1.0), 6)
        0.707107
        """
        ...

    def factor(self, t: float) -> float:
        """
        IM decay multiplier applied at time ``t`` in the MVA profile.

        Parameters
        ----------
        t : float
            Time in years from the valuation date.

        Returns
        -------
        float
            Decay factor, always in ``[0, 1]`` for ``t >= 0``.

        Raises
        ------
        (None)
            This is a pure arithmetic evaluation; it never raises.

        Examples
        --------
        >>> ImDecayProfile.constant().factor(3.0)
        1.0
        """
        ...

    @staticmethod
    def from_json(json: str) -> ImDecayProfile:
        """
        Parse this object from a JSON object or JSON string.

        Parameters
        ----------
        json : str
            JSON representation produced by ``to_json``.

        Returns
        -------
        ImDecayProfile
            Parsed decay profile.

        Raises
        ------
        ValueError
            If the payload is not valid JSON or does not match the schema.

        Examples
        --------
        >>> ImDecayProfile.from_json(ImDecayProfile.constant().to_json())
        ImDecayProfile(constant)
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this object to a JSON-compatible dict.

        Returns
        -------
        str
            Pretty-printed JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> isinstance(ImDecayProfile.constant().to_json(), str)
        True
        """
        ...

    def __repr__(self) -> str: ...

class ImProfile:
    """
    Expected initial-margin profile ``E[IM(t)]`` on a time grid.

    Values are in the aggregation currency chosen when the profile was
    built (e.g. the ``currency`` argument of :func:`im_profile_from_simm`).

    Parameters
    ----------
    times : list[float]
        Time points in years from the valuation date; strictly increasing
        and positive.
    im_values : list[float]
        Expected IM at each time point; non-negative and finite.

    Returns
    -------
    ImProfile
        IM profile as constructed; not validated until ``validate()`` is
        called or the profile is passed to ``compute_mva`` or
        ``im_profile_from_simm``.

    Examples
    --------
    >>> ImProfile([1.0, 2.0], [100.0, 50.0]).times
    [1.0, 2.0]
    """

    def __init__(self, times: list[float], im_values: list[float]) -> None:
        """
        Construct from time and IM vectors.

        Values are stored as given and are not validated at construction
        time. Call ``validate()`` explicitly, or rely on downstream
        functions (``compute_mva``, ``im_profile_from_simm``) to reject an
        inconsistent profile when it is used.

        Parameters
        ----------
        times : list[float]
            Time points in years; strictly increasing and positive.
        im_values : list[float]
            Expected IM at each time point; non-negative and finite.

        Raises
        ------
        (None)
            Construction never raises; values are stored unchecked and
            validated only by ``validate()`` or downstream consumers.

        Examples
        --------
        >>> ImProfile([1.0, 2.0], [100.0, 50.0]).times
        [1.0, 2.0]
        """
        ...

    @staticmethod
    def from_json(json: str) -> ImProfile:
        """
        Parse this object from a JSON object or JSON string.

        Parameters
        ----------
        json : str
            JSON representation produced by ``to_json``.

        Returns
        -------
        ImProfile
            Parsed IM profile.

        Raises
        ------
        ValueError
            If the payload is not valid JSON or does not match the schema.

        Examples
        --------
        >>> ImProfile.from_json(ImProfile([1.0], [1.0]).to_json()).times
        [1.0]
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this object to a JSON-compatible dict.

        Returns
        -------
        str
            Pretty-printed JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.

        Examples
        --------
        >>> isinstance(ImProfile([1.0], [1.0]).to_json(), str)
        True
        """
        ...

    def validate(self) -> None:
        """
        Validate internal consistency.

        Raises
        ------
        ValueError
            If the profile is empty, lengths differ, times are not
            strictly increasing and positive, or IM values are
            negative/non-finite.

        Examples
        --------
        >>> ImProfile([1.0], [1.0]).validate()
        """
        ...

    @property
    def times(self) -> list[float]:
        """
        Exposure or IM observation times in years from the valuation date.

        Returns
        -------
        list[float]
            Strictly increasing, positive time points.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ImProfile([1.0, 2.0], [1.0, 2.0]).times
        [1.0, 2.0]
        """
        ...

    @property
    def im_values(self) -> list[float]:
        """
        Expected IM at each time point.

        Returns
        -------
        list[float]
            Non-negative IM values in the profile's aggregation currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ImProfile([1.0], [100.0]).im_values
        [100.0]
        """
        ...

    def __len__(self) -> int:
        """
        Number of time points.

        Returns
        -------
        int
            Length of ``times`` (and ``im_values``).

        Examples
        --------
        >>> len(ImProfile([1.0, 2.0], [1.0, 2.0]))
        2
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a pandas ``DataFrame`` with time (years) as index.

        Returns
        -------
        pandas.DataFrame
            Single column ``im``, indexed by time in years.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.

        Examples
        --------
        >>> df = ImProfile([1.0, 2.0], [100.0, 50.0]).to_dataframe()
        >>> list(df.columns)
        ['im']
        """
        ...

    def __repr__(self) -> str: ...

class MvaResult:
    """
    Result of an MVA computation.

    All monetary quantities are ``float`` in the IM profile's currency.
    Returned by :func:`compute_mva`.

    Parameters
    ----------
    (Returned by ``compute_mva``; not directly instantiated.)

    Returns
    -------
    MvaResult
        MVA value, average IM, and echoed IM profile.

    Examples
    --------
    >>> result = MvaResult.from_json('{"mva":1.0,"average_im":100.0,"im_profile":[[1.0,100.0]]}')
    >>> (result.mva, result.average_im)
    (1.0, 100.0)
    """

    @staticmethod
    def from_json(json: str) -> MvaResult:
        """
        Parse this object from a JSON object or JSON string.

        Parameters
        ----------
        json : str
            JSON representation produced by ``to_json``.

        Returns
        -------
        MvaResult
            Parsed MVA result.

        Raises
        ------
        ValueError
            If the payload is not valid JSON or does not match the schema.

        Examples
        --------
        >>> result = MvaResult.from_json('{"mva":1.0,"average_im":100.0,"im_profile":[[1.0,100.0]]}')
        >>> result.im_profile
        [(1.0, 100.0)]
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this object to a JSON-compatible dict.

        Returns
        -------
        str
            Pretty-printed JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def mva(self) -> float:
        """
        MVA (positive = lifetime funding cost of posting IM).

        Returns
        -------
        float
            MVA amount in the IM profile's currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def average_im(self) -> float:
        """
        Time-weighted average IM over the profile horizon.

        Returns
        -------
        float
            ``(1/T) * integral_0^T IM(t) dt`` under the same trapezoid
            convention as ``mva``, in the IM profile's currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def im_profile(self) -> list[tuple[float, float]]:
        """
        IM profile used, as ``(time, value)`` tuples.

        Returns
        -------
        list[tuple[float, float]]
            ``(time_years, im_value)`` pairs echoing the input profile.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the IM profile as a pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Single column ``im``, indexed by time in years.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str: ...

def im_profile_from_simm(
    calculator: SimmCalculator,
    sensitivities: SimmSensitivities,
    currency: str,
    decay: ImDecayProfile,
    time_grid: list[float],
) -> ImProfile:
    """
    Build a deterministic IM profile from current SIMM sensitivities.

    Computes ``IM(t) = SIMM(sensitivities) * decay(t)`` on ``time_grid``,
    where the base IM is the full cross-risk-class ISDA SIMM aggregate of
    ``sensitivities`` (validated first, so an unknown tenor raises).

    Parameters
    ----------
    calculator : SimmCalculator
        SIMM calculator fixing the version/registry parameters.
    sensitivities : SimmSensitivities
        Current portfolio sensitivities in SIMM buckets.
    currency : str
        Aggregation currency for the SIMM total.
    decay : ImDecayProfile
        Deterministic decay profile applied to the base IM.
    time_grid : list[float]
        Strictly increasing, positive year fractions.

    Returns
    -------
    ImProfile
        Expected IM profile ``E[IM(t)]`` on ``time_grid``.

    Raises
    ------
    ValueError
        If ``currency`` is not a known currency code, or ``sensitivities``,
        ``decay`` or ``time_grid`` fails validation.

    Examples
    --------
    >>> sens = SimmSensitivities("USD")
    >>> sens.add_ir_delta("USD", "5Y", 50_000.0)
    >>> calc = SimmCalculator("v2_6")
    >>> decay = ImDecayProfile.linear_to_maturity(4.0)
    >>> profile = im_profile_from_simm(calc, sens, "USD", decay, [1.0, 2.0, 4.0])
    >>> profile.times
    [1.0, 2.0, 4.0]
    """
    ...

def compute_mva(
    im_profile: ImProfile,
    funding_spread_curve: list[tuple[float, float]] | pd.Series,
    discount_curve: DiscountCurve,
    survival_curve: HazardCurve | None = None,
) -> MvaResult:
    """
    Compute MVA over an expected-IM profile.

    Integrates ``spread(t) * IM(t) * DF(t) * S(t)`` over the profile's time
    grid using the same midpoint/trapezoid convention as the CVA engine,
    with a ``t = 0`` bucket edge (``DF(0) = 1``, ``S(0) = 1``). IM is
    treated as flat (left-constant) before the first grid point.

    Parameters
    ----------
    im_profile : ImProfile
        Expected IM profile ``E[IM(t)]`` (from ``im_profile_from_simm`` or
        the stochastic engine's mean per-path IM).
    funding_spread_curve : list[tuple[float, float]] | pd.Series
        ``(time_years, spread_bp)`` pairs in basis points, or a ``Series`` of
        spreads in bp indexed by time in years; linearly interpolated with
        flat extrapolation, and a single point means a flat spread.
    discount_curve : DiscountCurve
        Risk-free discount curve.
    survival_curve : HazardCurve or None, optional
        Optional bank (own) hazard curve; when ``None``, survival
        probability ``S(t)`` is treated as 1 for all ``t``.

    Returns
    -------
    MvaResult
        MVA, time-weighted average IM, and the echoed IM profile.

    Raises
    ------
    ValueError
        If the profile or spread curve fails validation, or if any curve
        evaluation returns a non-finite value.

    Examples
    --------
    >>> from datetime import date
    >>> from finstack_quant.core.market_data import DiscountCurve
    >>> profile = ImProfile([1.0, 2.0], [1_000_000.0, 1_000_000.0])
    >>> result = compute_mva(
    ...     profile,
    ...     [(0.0, 50.0)],
    ...     DiscountCurve.flat("USD-OIS", date(2025, 1, 1), 0.0),
    ... )
    >>> round(result.mva, 2)
    10000.0
    """
    ...

def compute_bilateral_xva(
    exposure_profile: ExposureProfile,
    counterparty_hazard_curve: HazardCurve,
    own_hazard_curve: HazardCurve,
    discount_curve: DiscountCurve,
    counterparty_recovery_rate: float,
    own_recovery_rate: float,
    funding: FundingConfig | None = None,
) -> XvaResult:
    """
    Compute bilateral XVA: CVA, DVA, FVA, MVA, and the all-in adjustment.

    All legs are weighted by joint (first-to-default) survival, so the credit
    and funding components are not double-counted. MVA is computed only when
    ``funding`` carries an ``im_profile``; that posted IM also reduces ENE for
    bilateral DVA.

    The result reports ``total_xva = CVA - DVA + FVA + MVA``; uncomputed
    optional legs contribute zero.

    Parameters
    ----------
    exposure_profile : ExposureProfile
        EPE/ENE profile from exposure simulation.
    counterparty_hazard_curve : HazardCurve
        Hazard curve for the counterparty's credit.
    own_hazard_curve : HazardCurve
        Hazard curve for the institution's own credit.
    discount_curve : DiscountCurve
        Risk-free discount curve for present-valuing.
    counterparty_recovery_rate : float
        Recovery rate on counterparty default, in ``[0, 1]``.
    own_recovery_rate : float
        Recovery rate on own default, in ``[0, 1]``.
    funding : FundingConfig | None
        Funding configuration driving FVA and, when it carries an
        ``im_profile``, MVA. ``None`` computes the credit legs only.

    Returns
    -------
    XvaResult
        CVA, DVA, FVA, MVA, ``total_xva``, and the exposure/regulatory
        profiles.

    Raises
    ------
    ValueError
        If the exposure profile is empty or inconsistent, a recovery rate is
        outside ``[0, 1]``, funding inputs are invalid, the IM and exposure
        horizons differ, or a curve evaluation is non-finite.

    Examples
    --------
    >>> import datetime as dt
    >>> from finstack_quant.core.market_data import DiscountCurve, HazardCurve
    >>> profile = ExposureProfile([1.0, 2.0], [1e6, 1e6], [1e6, 1e6], [0.0, 0.0])
    >>> df = DiscountCurve(
    ...     "USD-OIS",
    ...     dt.date(2025, 1, 1),
    ...     [(0.5 * i, 1.0) for i in range(9)],
    ...     interp="log_linear",
    ... )
    >>> hz = HazardCurve("CPTY", dt.date(2025, 1, 1), [(0.0, 0.02), (30.0, 0.02)], recovery_rate=0.40)
    >>> result = compute_bilateral_xva(profile, hz, hz, df, 0.40, 0.40)
    >>> result.total_xva == result.cva - result.dva
    True
    """
    ...
