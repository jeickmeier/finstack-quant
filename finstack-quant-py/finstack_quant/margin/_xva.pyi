"""Stub declarations mirroring ``finstack-quant-py/src/bindings/margin/xva.rs``."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from datetime import date
from typing import Any, Literal, TypedDict, Unpack
import pandas as pd
import polars as pl
from finstack_quant.core.market_data import DiscountCurve, HazardCurve
from finstack_quant.core.table import ArrowTable
from finstack_quant.margin._im import SimmCalculator, SimmSensitivities
from finstack_quant.margin._types.enums import NettingSetId

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
    margin_funding_spread : MarginFundingSpread or None, optional
        Flat or dated spread applied to posted IM. Required with an IM profile.

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
        margin_funding_spread: MarginFundingSpread | None = None,
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
        margin_funding_spread : MarginFundingSpread or None
            Explicit spread applied to posted IM. Required with an IM profile.

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
    def margin_funding_spread(self) -> MarginFundingSpread | None:
        """Get the separately elected spread for posted IM.

        Returns
        -------
        MarginFundingSpread or None
            Flat or dated funding spread if MVA is configured.

        Notes
        -----
        This member does not raise for typed arguments.
        """

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

class _ExternalExposureBasis(TypedDict):
    kind: Literal["external"]
    source_id: str
    policy_id: str

class _SimulatedExposureBasis(TypedDict):
    kind: Literal["simulated_uncollateralized", "simulated_cash_vm"]
    source_id: str
    path_count: int

class _ExposureDiagnosticsFields(TypedDict):
    market_roll_failures: int
    valuation_failures: int
    total_time_points: int

class _ExposureProfileFields(TypedDict):
    as_of: date | str
    currency: str
    basis: _ExternalExposureBasis | _SimulatedExposureBasis
    dates: Sequence[date | str]
    times: list[float]
    mtm_values: list[float]
    epe: list[float]
    ene: list[float]
    diagnostics: ExposureDiagnostics | _ExposureDiagnosticsFields

class ExposureProfile:
    """Dated observations of portfolio mark-to-market and expected exposure.

    Exposure amounts use major units of one reporting currency. Dates map
    to each valuation curve's own clock. Model year fractions determine
    the exposure integration clock.

    Examples
    --------
    >>> profile = ExposureProfile(
    ...     as_of="2025-01-01",
    ...     currency="USD",
    ...     basis={"kind": "external", "source_id": "audit-grid", "policy_id": "unsecured"},
    ...     dates=["2025-01-01", "2025-07-01"],
    ...     times=[0.0, 0.5],
    ...     mtm_values=[20.0, -40.0],
    ...     epe=[20.0, 0.0],
    ...     ene=[0.0, 40.0],
    ...     diagnostics=ExposureDiagnostics(total_time_points=2),
    ... )
    >>> profile.validate()
    >>> profile.as_of.isoformat()
    '2025-01-01'
    >>> ExposureProfile.from_dataframe(profile.to_pandas()).ene
    [0.0, 40.0]
    >>> ExposureProfile.from_json(profile.to_json()).dates == profile.dates
    True
    """

    def __init__(self, **fields: Unpack[_ExposureProfileFields]) -> None:
        """Construct a profile from its dated grid, amounts and source evidence.

        Parameters
        ----------
        fields : _ExposureProfileFields
            `as_of` is the valuation cut, as a date or ISO year-month-day.
            `currency` is the ISO 4217 denomination of all amounts.
            `basis` records the provenance and policy or simulation fingerprint.
            `dates` begins at valuation and contains the observation dates in order.
            `times` contains corresponding model year fractions beginning at zero.
            `mtm_values` contains signed expected marks in reporting currency.
            `epe` contains the expected positive parts of the marks.
            `ene` contains expected positive parts of the negated marks.
            `diagnostics` records the number of observations and any failures.

        Raises
        ------
        ValueError
            If a field is missing, unknown or cannot be parsed.
        """

    @staticmethod
    def from_dataframe(
        frame: pd.DataFrame | pl.DataFrame | Any,
        *,
        as_of: date | str | None = None,
        currency: str | None = None,
        basis: Mapping[str, Any] | None = None,
        diagnostics: ExposureDiagnostics | Mapping[str, Any] | None = None,
        time_col: str | None = None,
    ) -> ExposureProfile:
        """Restore a valid profile from a tabular exposure frame.

        Accepts a pandas ``DataFrame``, Polars ``DataFrame``, or Arrow table
        with ``dates``, ``mtm_values``, ``epe``, and ``ene`` columns plus year
        fractions from either a ``times`` column, ``time_col``, or the pandas
        index. Provenance metadata embedded in pandas ``frame.attrs["finstack_exposure"]``
        is used as a default and overridden by explicit keyword arguments.

        Parameters
        ----------
        frame : pd.DataFrame | pl.DataFrame | Any
            Tabular profile containing ``dates``, ``mtm_values``, ``epe``, and
            ``ene`` columns (plus year fractions in ``times``, ``time_col``, or
            the pandas index). Polars and Arrow frames must supply ``as_of``
            and ``currency`` explicitly because they do not carry pandas
            ``attrs`` metadata.
        as_of : datetime.date | str | None, optional
            Valuation date (ISO-8601 string or date-like object). Overrides any
            ``as_of`` stored in ``frame.attrs["finstack_exposure"]``; required
            when absent from metadata.
        currency : str | None, optional
            ISO-4217 exposure currency code. Overrides any ``currency`` stored
            in ``frame.attrs["finstack_exposure"]``; required when absent from
            metadata.
        basis : Mapping[str, Any] | None, optional
            Optional provenance mapping describing the exposure simulation basis.
            Overrides any ``basis`` stored in ``frame.attrs["finstack_exposure"]``.
        diagnostics : ExposureDiagnostics | Mapping[str, Any] | None, optional
            Optional diagnostic counts for the simulated profile. Overrides any
            ``diagnostics`` stored in ``frame.attrs["finstack_exposure"]``.
        time_col : str | None, optional
            Column name holding year fractions from ``as_of``. When omitted,
            uses a ``"times"`` column if present or falls back to the pandas
            row index.

        Returns
        -------
        ExposureProfile
            Checked profile with the original or supplied valuation cut and
            provenance.

        Raises
        ------
        KeyError
            If ``time_col`` is specified but not found in ``frame``.
        TypeError
            If ``frame`` is not a supported DataFrame/Arrow table or its
            metadata is not a mapping.
        ValueError
            If a required column or provenance field is missing or invalid.

        Examples
        --------
        >>> profile = ExposureProfile(
        ...     as_of="2025-01-01",
        ...     currency="USD",
        ...     basis={"kind": "external", "source_id": "example", "policy_id": "unsecured"},
        ...     dates=["2025-01-01"],
        ...     times=[0.0],
        ...     mtm_values=[10.0],
        ...     epe=[10.0],
        ...     ene=[0.0],
        ...     diagnostics=ExposureDiagnostics(total_time_points=1),
        ... )
        >>> ExposureProfile.from_dataframe(profile.to_pandas()).mtm_values
        [10.0]
        """

    @staticmethod
    def from_json(json: str) -> ExposureProfile:
        """Read the canonical exposure profile wire object.

        Parameters
        ----------
        json : str
            Dated exposure profile JSON, including basis and completeness fields.

        Returns
        -------
        ExposureProfile
            Profile available for validation and XVA.

        Raises
        ------
        ValueError
            If the wire object cannot be decoded.

        Examples
        --------
        >>> profile = ExposureProfile(
        ...     as_of="2025-01-01",
        ...     currency="USD",
        ...     basis={"kind": "external", "source_id": "example", "policy_id": "unsecured"},
        ...     dates=["2025-01-01"],
        ...     times=[0.0],
        ...     mtm_values=[10.0],
        ...     epe=[10.0],
        ...     ene=[0.0],
        ...     diagnostics=ExposureDiagnostics(total_time_points=1),
        ... )
        >>> ExposureProfile.from_json(profile.to_json()).mtm_values
        [10.0]
        """

    def to_json(self) -> str:
        """Serialize the complete observation grid and its provenance.

        Returns
        -------
        str
            Profile wire object as JSON text.

        Raises
        ------
        ValueError
            If the profile cannot be serialized.
        """

    def validate(self) -> None:
        """Check the grid, units, provenance, numeric domain and completeness.

        Raises
        ------
        ValueError
            If observation dates or times are unordered, the opening point
            or provenance is absent, amounts are malformed, or diagnostics
            record incomplete rolls or valuations.
        """

    @property
    def as_of(self) -> date:
        """Get the exposure valuation cut.

        Returns
        -------
        date
            Calendar date of the opening observation.

        Notes
        -----
        This accessor does not raise for a valid profile.
        """

    @property
    def currency(self) -> str:
        """Get the profile reporting currency.

        Returns
        -------
        str
            ISO 4217 code shared by all observed amounts.

        Notes
        -----
        This accessor does not raise for a valid profile.
        """

    @property
    def basis(self) -> _ExternalExposureBasis | _SimulatedExposureBasis:
        """Get the exposure provenance.

        Returns
        -------
        _ExternalExposureBasis | _SimulatedExposureBasis
            Source identifier and applicable basis.

        Notes
        -----
        This accessor does not raise for a valid profile.
        """

    @property
    def dates(self) -> list[date]:
        """Get the calendar observation dates.

        Returns
        -------
        list[date]
            Ascending dates beginning at the valuation cut.

        Notes
        -----
        This accessor does not raise for a valid profile.
        """

    @property
    def times(self) -> list[float]:
        """Get the model integration clock.

        Returns
        -------
        list[float]
            Ascending year fractions from zero.

        Notes
        -----
        This accessor does not raise for a valid profile.
        """

    @property
    def mtm_values(self) -> list[float]:
        """Get portfolio marks on the grid.

        Returns
        -------
        list[float]
            Signed amounts in reporting currency.

        Notes
        -----
        This accessor does not raise for a valid profile.
        """

    @property
    def epe(self) -> list[float]:
        """Get expected positive exposure.

        Returns
        -------
        list[float]
            Positive-part expectation in reporting currency at each date.

        Notes
        -----
        This accessor does not raise for a valid profile.
        """

    @property
    def ene(self) -> list[float]:
        """Get expected negative exposure.

        Returns
        -------
        list[float]
            Negated-mark positive-part expectation in reporting currency at each date.

        Notes
        -----
        This accessor does not raise for a valid profile.
        """

    @property
    def diagnostics(self) -> ExposureDiagnostics:
        """Get completeness evidence.

        Returns
        -------
        ExposureDiagnostics
            Calendar observation count and failed roll and valuation counts.

        Notes
        -----
        This accessor does not raise for a valid profile.
        """

    def __len__(self) -> int:
        """Return the number of calendar observations.

        Returns
        -------
        int
            Grid size including the opening observation.
        """

    def to_arrow(self) -> ArrowTable:
        """
        Export dated exposure amounts as an :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Columns ``times`` (year fractions), ``dates``, ``mtm_values``,
            ``epe``, and ``ene``.

        Raises
        ------
        ValueError
            If the exposure profile cannot be converted to an Arrow table.
        """

    def to_pandas(self) -> pd.DataFrame:
        """
        Export dated exposure amounts and source evidence as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``dates``, ``mtm_values``, ``epe``, and ``ene``, indexed by
            year fractions (``times``) with source evidence in
            ``df.attrs["finstack_exposure"]``.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        ValueError
            If the exposure profile cannot be converted to pandas.
        """

    def to_polars(self) -> pl.DataFrame:
        """
        Export dated exposure amounts as a Polars ``DataFrame``.

        Returns
        -------
        pl.DataFrame
            Columns ``times`` (year fractions), ``dates``, ``mtm_values``,
            ``epe``, and ``ene``.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """

    def to_pyarrow(self) -> Any:
        """
        Export dated exposure amounts as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Columns ``times`` (year fractions), ``dates``, ``mtm_values``,
            ``epe``, and ``ene``.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
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
    def mva_detail(self) -> MvaResult | None:
        """Get the admitted posted IM and dated funding contributions.

        Returns
        -------
        MvaResult or None
            Replayable funding detail when MVA was computed.

        Notes
        -----
        This property does not raise.
        """

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

    def to_arrow(self) -> ArrowTable:
        """
        Export the XVA components as a single-row :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``cva``, ``dva``, ``fva``, ``mva``, ``total_xva``,
        ``max_epe``, ``effective_epe`` -- all in the netting set's currency
        units, matching the properties of the same name. Uncomputed legs
        (``dva`` / ``fva`` / ``mva``) are null rather than absent, so the
        table keeps its schema across netting sets.

        This is the default export; the time-indexed exposure profiles are a
        separate table -- see :meth:`to_profiles_table`.

        Returns
        -------
        ArrowTable
            Single-row table of XVA components.

        Raises
        ------
        ValueError
            If the result cannot be serialized into an Arrow table.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export the XVA components as a single-row pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Single-row DataFrame, so a portfolio of netting sets stacks with
            ``pd.concat``.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export the XVA components as a single-row Polars ``DataFrame``.

        Returns
        -------
        pl.DataFrame
            Single-row DataFrame of XVA components.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export the XVA components as a single-row ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Single-row table of XVA components.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def to_profiles_table(self) -> ArrowTable:
        """
        Export exposure profiles as an :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``times``, ``epe``, ``ene``, ``effective_epe``.

        Returns
        -------
        ArrowTable
            Profile table.

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

class ImProjectionSpec:
    """Dated runoff and the qualifying scope of historical SIMM margin.

    Examples
    --------
    >>> from finstack_quant.margin import ImDecayProfile, ImProjectionSpec, NettingSetId
    >>> spec = ImProjectionSpec(
    ...     as_of="2025-01-01",
    ...     currency="USD",
    ...     netting_set=NettingSetId.bilateral("bank", "csa"),
    ...     scope_source="admitted-risk",
    ...     dates=["2025-01-01", "2026-01-01"],
    ...     decay=ImDecayProfile.constant(),
    ... )
    """

    def __init__(
        self,
        *,
        as_of: date | str,
        currency: str,
        netting_set: NettingSetId,
        scope_source: str,
        dates: Sequence[date | str],
        decay: ImDecayProfile,
    ) -> None:
        """Author the dated runoff specification.

        Parameters
        ----------
        as_of : date or str
            Opening margin valuation date.
        currency : str
            ISO 4217 margin currency.
        netting_set : NettingSetId
            Bilateral counterparty and agreement admitting the original risk.
        scope_source : str
            Evidence identifier for the risk snapshot and legal scope.
        dates : sequence of date or str
            Strictly increasing observations beginning on the valuation date.
            Include the runoff date whenever it occurs within the horizon.
        decay : ImDecayProfile
            Constant or explicit maturity-based deterministic runoff, measured
            from valuation on ACT/365 Fixed.

        Raises
        ------
        ValueError
            If a date or currency cannot be decoded. The calculation checks
            scope and projection coverage.
        TypeError
            If an argument does not have the expected Python type.
        """

class MarginFundingSpread:
    """An explicit flat or dated IM funding spread in basis points.

    Examples
    --------
    >>> from finstack_quant.margin import MarginFundingSpread
    >>> isinstance(MarginFundingSpread.flat(50.0), MarginFundingSpread)
    True
    """

    @staticmethod
    def flat(spread_bp: float) -> MarginFundingSpread:
        """Author one spread over the full admitted margin horizon.

        Parameters
        ----------
        spread_bp : float
            Nonnegative finite spread in basis points. The calculation checks
            this range when it uses the spread.

        Returns
        -------
        MarginFundingSpread
            Flat funding-spread election.

        Notes
        -----
        This member does not raise for typed arguments.

        Examples
        --------
        >>> from finstack_quant.margin import MarginFundingSpread
        >>> spread = MarginFundingSpread.flat(50.0)
        """

    @staticmethod
    def dated(
        knots: Sequence[tuple[date | str, float]] | pd.Series | pd.DataFrame | pl.DataFrame | Any,
    ) -> MarginFundingSpread:
        """Author linear interpolation across calendar spread observations.

        Parameters
        ----------
        knots : sequence | pd.Series | pd.DataFrame | pl.DataFrame | Any
            Strictly ordered calendar dates and nonnegative finite basis-point
            spreads. A ``pandas.Series`` supplies date objects or ISO strings
            as its index; two-column DataFrames or Arrow tables read dates
            from the first column and spreads from the second. The dates must
            cover the full IM horizon.

        Returns
        -------
        MarginFundingSpread
            Dated funding-spread election. The calculation checks coverage.

        Raises
        ------
        ValueError
            If a calendar date cannot be decoded.
        TypeError
            If a knot cannot be read as a date and numeric spread pair.

        Examples
        --------
        >>> from finstack_quant.margin import MarginFundingSpread
        >>> spread = MarginFundingSpread.dated([("2025-01-01", 35), ("2026-01-01", 50)])
        """

class ImProfile:
    """Expected posted IM on a complete dated grid.

    Examples
    --------
    >>> from finstack_quant.margin import ImProfile, NettingSetId
    >>> profile = ImProfile(
    ...     as_of="2025-01-01",
    ...     currency="USD",
    ...     netting_set=NettingSetId.bilateral("bank", "csa"),
    ...     source_id="supplied-risk-run",
    ...     dates=["2025-01-01", "2026-01-01"],
    ...     im_values=[100, 0],
    ... )
    >>> profile.times
    [0.0, 1.0]
    """

    def __init__(
        self,
        *,
        as_of: date | str,
        currency: str,
        netting_set: NettingSetId,
        source_id: str,
        dates: Sequence[date | str],
        im_values: Sequence[float],
    ) -> None:
        """Admit a complete externally supplied expected posted-margin profile.

        Parameters
        ----------
        as_of : date or str
            Valuation date matching the first observation.
        currency : str
            ISO 4217 currency shared by the expected amounts.
        netting_set : NettingSetId
            Legal bilateral counterparty and agreement measured by the profile.
        source_id : str
            Nonempty evidence identifier for the supplied calculation.
        dates : sequence of date or str
            Strictly increasing observation dates from valuation through the funding horizon.
        im_values : sequence of float
            Finite nonnegative expected posted IM amounts in major currency
            units aligned to every date, including the opening observation.

        Raises
        ------
        ValueError
            If a date, amount, currency or the legal scope is invalid.
        TypeError
            If an argument does not have the expected Python type.
        """

    @property
    def as_of(self) -> date:
        """Get the valuation date.

        Returns
        -------
        date
            First observation date.

        Notes
        -----
        This member does not raise for typed arguments.
        """

    @property
    def currency(self) -> str:
        """Get the currency of the expected posted amounts.

        Returns
        -------
        str
            ISO 4217 reporting currency.

        Notes
        -----
        This member does not raise for typed arguments.
        """

    @property
    def basis(self) -> dict[str, Any]:
        """Get identified scope and method evidence.

        Returns
        -------
        dict
            External source or SIMM runoff evidence on the canonical wire basis.

        Raises
        ------
        ValueError
            If the projection's terms cannot be represented as Python data.
        """

    @property
    def dates(self) -> list[date]:
        """Get every calendar observation.

        Returns
        -------
        list of date
            Opening through horizon, in time order.

        Notes
        -----
        This member does not raise for typed arguments.
        """

    @property
    def times(self) -> list[float]:
        """Calculate time elapsed from the opening on ACT/365 Fixed.

        Returns
        -------
        list of float
            Year fractions aligned with the dated observations.

        Notes
        -----
        This member does not raise for typed arguments.
        """

    @property
    def im_values(self) -> list[float]:
        """Get the posted-margin expectations.

        Returns
        -------
        list of float
            Dated nonnegative amounts in the profile currency.

        Notes
        -----
        This member does not raise for typed arguments.
        """

    def validate(self) -> None:
        """Validate that the date ordering, amounts and evidence are complete.

        Raises
        ------
        ValueError
            If the input fails the dated margin-profile contract.
        """

    def to_json(self) -> str:
        """Serialize the dated profile.

        Returns
        -------
        str
            Canonical Rust profile JSON with scope and dates.

        Raises
        ------
        ValueError
            If the profile cannot be serialized.
        """

    @staticmethod
    def from_json(json: str) -> ImProfile:
        """Read an externally supplied or computed profile from JSON.

        Parameters
        ----------
        json : str
            Canonical JSON previously emitted by to_json.

        Returns
        -------
        ImProfile
            Validated profile with its original scope and observation grid.

        Raises
        ------
        ValueError
            If the input cannot be parsed or fails profile validation.

        Examples
        --------
        >>> from finstack_quant.margin import ImProfile, NettingSetId
        >>> original = ImProfile(
        ...     as_of="2025-01-01",
        ...     currency="USD",
        ...     netting_set=NettingSetId.bilateral("bank", "csa"),
        ...     source_id="expected-risk",
        ...     dates=["2025-01-01", "2026-01-01"],
        ...     im_values=[100, 0],
        ... )
        >>> ImProfile.from_json(original.to_json()).im_values
        [100.0, 0.0]
        """

    def to_arrow(self) -> ArrowTable:
        """Build a dated :class:`~finstack_quant.core.table.ArrowTable` of IM observations.

        Returns
        -------
        ArrowTable
            Columns ``times``, ``date``, and ``im``.

        Raises
        ------
        ValueError
            If the table cannot be built.
        """

    def to_pandas(self) -> pd.DataFrame:
        """Build a dated pandas observation frame.

        Returns
        -------
        pandas.DataFrame
            Date and expected IM columns indexed by years since valuation.

        Raises
        ------
        ImportError
            If pandas is unavailable.
        """

    def to_polars(self) -> pl.DataFrame:
        """Build a dated Polars observation frame.

        Returns
        -------
        polars.DataFrame
            Columns ``times``, ``date``, and ``im``.

        Raises
        ------
        ImportError
            If polars is unavailable.
        """

    def to_pyarrow(self) -> Any:
        """Build a dated ``pyarrow.Table`` of IM observations.

        Returns
        -------
        pyarrow.Table
            Columns ``times``, ``date``, and ``im``.

        Raises
        ------
        ImportError
            If pyarrow is unavailable.
        """

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...

class _MvaPoint(TypedDict):
    date: str
    expected_im: float
    spread_bp: float
    discount_factor: float
    own_survival: float
    counterparty_survival: float

class _MvaInterval(TypedDict):
    start: _MvaPoint
    end: _MvaPoint
    years: float
    contribution: float

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
    >>> result = MvaResult.from_json(
    ...     '{"mva":1.0,"average_im":100.0,"im_profile":{"as_of":"2025-01-01","currency":"USD",'
    ...     '"basis":{"kind":"external","source_id":"projected","netting_set":{"kind":"bilateral",'
    ...     '"counterparty_id":"dealer","csa_id":"agreement"}},"dates":["2025-01-01","2026-01-01"],'
    ...     '"im_values":[100.0,100.0]},"intervals":[]}'
    ... )
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
        >>> result = MvaResult.from_json(
        ...     '{"mva":1.0,"average_im":100.0,"im_profile":{"as_of":"2025-01-01","currency":"USD",'
        ...     '"basis":{"kind":"external","source_id":"projected","netting_set":{"kind":"bilateral",'
        ...     '"counterparty_id":"dealer","csa_id":"agreement"}},"dates":["2025-01-01","2026-01-01"],'
        ...     '"im_values":[100.0,100.0]},"intervals":[]}'
        ... )
        >>> result.im_profile.im_values
        [100.0, 100.0]
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
            ``(1/T) * integral_0^T IM(t) dt`` on the dated ACT/365 Fixed grid, in the profile currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def im_profile(self) -> ImProfile:
        """Get the complete expected posted IM used for the cost.

        Returns
        -------
        ImProfile
            Identified profile with its opening and horizon dates.

        Notes
        -----
        This member does not raise for typed arguments.
        """

    @property
    def intervals(self) -> list[_MvaInterval]:
        """Get factors and contributions on the merged date grid.

        Returns
        -------
        list of dict
            Opening and closing IM, spread, discount and survival for each
            interval, ACT/365 Fixed interval lengths, and funding contributions.

        Raises
        ------
        ValueError
            If the numerical evidence cannot be represented as Python data.
        """

    def to_arrow(self) -> ArrowTable:
        """
        Export the IM profile as an :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Columns ``times``, ``date``, and ``im``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into an Arrow table.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export the IM profile as a pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Columns for date and expected IM, indexed by time in years.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export the IM profile as a Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Columns ``times``, ``date``, and ``im``.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export the IM profile as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Columns ``times``, ``date``, and ``im``.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def __repr__(self) -> str: ...

def im_profile_from_simm(
    calculator: SimmCalculator,
    sensitivities: SimmSensitivities,
    spec: ImProjectionSpec,
) -> ImProfile:
    """Project the admitted opening SIMM result with deterministic decay.

    Parameters
    ----------
    calculator : SimmCalculator
        Elected historical parameter version and margin period of risk.
    sensitivities : SimmSensitivities
        Signed original risk keyed by currency, bucket and risk class.
    spec : ImProjectionSpec
        Qualified netting source, date, currency and runoff with a complete grid.

    Returns
    -------
    ImProfile
        Dated projection including today's SIMM and the selected approximation.

    Raises
    ------
    ValueError
        If sensitivities or the dated projection fail validation.

    Examples
    --------
    >>> from finstack_quant.margin import ImDecayProfile, ImProjectionSpec, NettingSetId
    >>> from finstack_quant.margin import SimmCalculator, SimmSensitivities, im_profile_from_simm
    >>> sensitivities = SimmSensitivities("USD")
    >>> sensitivities.add_ir_delta("USD", "5Y", 50_000.0)
    >>> spec = ImProjectionSpec(
    ...     as_of="2025-01-01",
    ...     currency="USD",
    ...     netting_set=NettingSetId.bilateral("bank", "csa"),
    ...     scope_source="trade-risk",
    ...     dates=["2025-01-01", "2026-01-01"],
    ...     decay=ImDecayProfile.constant(),
    ... )
    >>> im_profile_from_simm(SimmCalculator("v2_6"), sensitivities, spec).times
    [0.0, 1.0]
    """

def compute_mva(
    im_profile: ImProfile,
    funding_spread: MarginFundingSpread,
    discount_curve: DiscountCurve,
    own_survival: HazardCurve | None,
) -> MvaResult:
    """Integrate the joint product of posted IM, spread, discount and survival.

    Parameters
    ----------
    im_profile : ImProfile
        Complete dated margin expectations in reporting currency.
    funding_spread : MarginFundingSpread
        Flat or dated funding spread in basis points covering the full horizon.
    discount_curve : DiscountCurve
        Discount curve queried on its own calendar clock and normalized at valuation.
    own_survival : HazardCurve or None
        Institution's survival curve in reporting currency; None omits survival weighting.

    Returns
    -------
    MvaResult
        Expected funding cost, average expected IM and replayable interval factors.

    Raises
    ------
    ValueError
        If the profile, funding spread or curve coverage is invalid.
    RuntimeError
        If curve calculation or numerical integration fails.

    Examples
    --------
    >>> from datetime import date
    >>> from finstack_quant.core.market_data import DiscountCurve
    >>> from finstack_quant.margin import ImProfile, MarginFundingSpread, NettingSetId, compute_mva
    >>> profile = ImProfile(
    ...     as_of="2025-01-01",
    ...     currency="USD",
    ...     netting_set=NettingSetId.bilateral("bank", "csa"),
    ...     source_id="expected-IM",
    ...     dates=["2025-01-01", "2026-01-01"],
    ...     im_values=[100, 0],
    ... )
    >>> discount = DiscountCurve.flat("USD-OIS", date(2025, 1, 1), 0.0, "act_365f")
    >>> round(compute_mva(profile, MarginFundingSpread.flat(50.0), discount, None).mva, 2)
    0.25
    """

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
    ``funding`` carries an ``im_profile``. Bilateral DVA uses the supplied ENE
    measured under the elected collateral and close-out terms.

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
    >>> profile = ExposureProfile(
    ...     as_of="2025-01-01",
    ...     currency="USD",
    ...     basis={"kind": "external", "source_id": "audited-grid", "policy_id": "unsecured"},
    ...     dates=["2025-01-01", "2026-01-01", "2027-01-01"],
    ...     times=[0.0, 1.0, 2.0],
    ...     mtm_values=[1e6] * 3,
    ...     epe=[1e6] * 3,
    ...     ene=[0.0] * 3,
    ...     diagnostics=ExposureDiagnostics(total_time_points=3),
    ... )
    >>> df = DiscountCurve(
    ...     "USD-OIS",
    ...     dt.date(2025, 1, 1),
    ...     [(0.5 * i, 1.0) for i in range(9)],
    ...     interp="log_linear",
    ...     extrapolation="extend",
    ...     day_count="act_365f",
    ... )
    >>> hz = HazardCurve("CPTY", dt.date(2025, 1, 1), [(30.0, 0.02)], recovery_rate=0.40, day_count="act_365f")
    >>> result = compute_bilateral_xva(profile, hz, hz, df, 0.40, 0.40)
    >>> result.total_xva == result.cva - result.dva
    True
    """
    ...
