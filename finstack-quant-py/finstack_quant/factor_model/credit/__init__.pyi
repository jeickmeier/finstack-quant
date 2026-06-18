"""Credit factor hierarchy: calibration, decomposition, and covariance forecasts.

Bindings for ``finstack-quant-factor-model`` credit hierarchy artifacts. Models
are JSON-first: calibrate or load a :class:`CreditFactorModel`, decompose
observed spreads into level/adder components, link period-to-period changes, and
forecast factor covariance for risk reporting.
"""

from __future__ import annotations

class CreditFactorModel:
    """Calibrated credit factor hierarchy artifact.

    Produced by :class:`CreditCalibrator` or loaded via :meth:`from_json`. All
    fields are read-only; mutations require re-calibration.

    Examples
    --------
    >>> from finstack_quant.factor_model.credit import CreditFactorModel
    >>> model = CreditFactorModel.from_json(json_str)  # doctest: +SKIP
    >>> model.schema_version  # doctest: +SKIP
    'finstack_quant.credit_factor_model/1'
    """

    @staticmethod
    def from_json(json: str) -> CreditFactorModel:
        """Deserialize a credit factor model from JSON.

        Parameters
        ----------
        json : str
            JSON string produced by :meth:`to_json` or the offline calibrator.

        Returns
        -------
        CreditFactorModel
            Parsed, validated model instance.

        Raises
        ------
        ValueError
            If the JSON is malformed or fails structural validation.
        """
        ...

    def to_json(self) -> str:
        """Serialize this model to pretty-printed JSON.

        Returns
        -------
        str
            JSON suitable for storage or transmission.
        """
        ...

    @property
    def schema_version(self) -> str:
        """Schema version string (``"finstack_quant.credit_factor_model/1"``).

        Returns
        -------
        str
            Version tag embedded in the artifact.
        """
        ...

    @property
    def as_of(self) -> str:
        """Calibration anchor date as an ISO 8601 string.

        Returns
        -------
        str
            Model as-of date.
        """
        ...

    @property
    def n_levels(self) -> int:
        """Number of hierarchy levels (broadest to narrowest).

        Returns
        -------
        int
            Count of hierarchy dimensions.
        """
        ...

    @property
    def n_issuers(self) -> int:
        """Number of issuer beta rows in the artifact.

        Returns
        -------
        int
            Issuer count.
        """
        ...

    @property
    def n_factors(self) -> int:
        """Number of systematic factors in the model configuration.

        Returns
        -------
        int
            Factor count.
        """
        ...

    def level_names(self) -> list[str]:
        """Return hierarchy level names.

        Returns
        -------
        list[str]
            Dimension names (e.g. ``["Rating", "Region", "Sector"]``).
        """
        ...

    def issuer_ids(self) -> list[str]:
        """Return issuer IDs present in the artifact.

        Returns
        -------
        list[str]
            Issuer identifier strings.
        """
        ...

    def factor_ids(self) -> list[str]:
        """Return factor IDs in the model configuration.

        Returns
        -------
        list[str]
            Factor identifier strings.
        """
        ...

    def __repr__(self) -> str: ...

class CreditCalibrator:
    """Deterministic calibrator that produces a :class:`CreditFactorModel`.

    Configuration and inputs are JSON strings so callers can work with plain
    dicts (via ``json.dumps``) without typed wrappers for every sub-field.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.factor_model.credit import CreditCalibrator
    >>> cal = CreditCalibrator(json.dumps(config))
    >>> model = cal.calibrate(json.dumps(inputs))  # doctest: +SKIP
    """

    def __init__(self, config_json: str) -> None:
        """Construct a calibrator from a JSON configuration.

        Parameters
        ----------
        config_json : str
            JSON-encoded ``CreditCalibrationConfig``.

        Raises
        ------
        ValueError
            If ``config_json`` is not a valid ``CreditCalibrationConfig``.
        """
        ...

    def calibrate(self, inputs_json: str) -> CreditFactorModel:
        """Run calibration and return a validated factor model.

        Parameters
        ----------
        inputs_json : str
            JSON-encoded ``CreditCalibrationInputs`` (spreads, issuers, etc.).

        Returns
        -------
        CreditFactorModel
            Calibrated hierarchy artifact.

        Raises
        ------
        ValueError
            If inputs are invalid or calibration fails.
        """
        ...

    def __repr__(self) -> str: ...

class LevelsAtDate:
    """Decomposed credit spread levels at a single observation date."""

    @property
    def date(self) -> str:
        """Observation date as an ISO 8601 string.

        Returns
        -------
        str
            Decomposition date.
        """
        ...

    @property
    def generic(self) -> float:
        """Generic (market-wide) spread component in basis points.

        Returns
        -------
        float
            Generic level in bps.
        """
        ...

    @property
    def n_levels(self) -> int:
        """Number of hierarchy levels in the decomposition.

        Returns
        -------
        int
            Level count.
        """
        ...

    def level_values(self, level_index: int) -> dict[str, float]:
        """Return bucket values for one hierarchy level.

        Parameters
        ----------
        level_index : int
            Zero-based level index (0 = broadest).

        Returns
        -------
        dict[str, float]
            Map of bucket label to spread contribution in bps.

        Raises
        ------
        ValueError
            If ``level_index`` is out of range.
        """
        ...

    def adder(self) -> dict[str, float]:
        """Return issuer-specific adder spreads keyed by issuer ID.

        Returns
        -------
        dict[str, float]
            Per-issuer adder in bps.
        """
        ...

    def __repr__(self) -> str: ...

class PeriodDecomposition:
    """Period-over-period change in decomposed credit spread levels."""

    @property
    def from_date(self) -> str:
        """Start date of the decomposition window (ISO 8601).

        Returns
        -------
        str
            Period start.
        """
        ...

    @property
    def to_date(self) -> str:
        """End date of the decomposition window (ISO 8601).

        Returns
        -------
        str
            Period end.
        """
        ...

    @property
    def d_generic(self) -> float:
        """Change in generic spread over the period (bps).

        Returns
        -------
        float
            Generic delta in bps.
        """
        ...

    @property
    def n_levels(self) -> int:
        """Number of hierarchy levels.

        Returns
        -------
        int
            Level count.
        """
        ...

    def level_deltas(self, level_index: int) -> dict[str, float]:
        """Return bucket deltas for one hierarchy level.

        Parameters
        ----------
        level_index : int
            Zero-based level index.

        Returns
        -------
        dict[str, float]
            Map of bucket label to spread change in bps.

        Raises
        ------
        ValueError
            If ``level_index`` is out of range.
        """
        ...

    def d_adder(self) -> dict[str, float]:
        """Return issuer adder changes keyed by issuer ID.

        Returns
        -------
        dict[str, float]
            Per-issuer adder delta in bps.
        """
        ...

    def __repr__(self) -> str: ...

class FactorCovarianceForecast:
    """Factor covariance and idiosyncratic vol forecasts from a credit factor model."""

    def __init__(self, model: CreditFactorModel) -> None:
        """Bind a covariance forecast engine to a calibrated model.

        Parameters
        ----------
        model : CreditFactorModel
            Calibrated hierarchy artifact used as the risk factor basis.
        """
        ...

    def covariance_at(self, horizon: str) -> str:
        """Return factor covariance matrix JSON at a forecast horizon.

        Parameters
        ----------
        horizon : str
            Tenor string parseable by the core date/tenor utilities (e.g.
            ``"3M"``, ``"1Y"``).

        Returns
        -------
        str
            JSON-encoded covariance matrix for systematic factors.

        Raises
        ------
        ValueError
            If ``horizon`` is invalid or the model lacks required inputs.
        """
        ...

    def idiosyncratic_vol(self, issuer_id: str, horizon: str) -> float:
        """Return idiosyncratic volatility for an issuer at a horizon.

        Parameters
        ----------
        issuer_id : str
            Issuer identifier present in the model artifact.
        horizon : str
            Forecast horizon tenor string.

        Returns
        -------
        float
            Idiosyncratic volatility (decimal annualized).

        Raises
        ------
        ValueError
            If the issuer or horizon is unknown.
        """
        ...

    def factor_model_at(self, horizon: str, risk_measure_json: str) -> str:
        """Return a portfolio-ready factor model JSON at a horizon.

        Parameters
        ----------
        horizon : str
            Forecast horizon tenor string.
        risk_measure_json : str
            JSON-encoded risk-measure configuration (e.g. VaR horizon, scaling).

        Returns
        -------
        str
            JSON factor model suitable for portfolio risk decomposition.

        Raises
        ------
        ValueError
            If inputs are invalid or the forecast cannot be built.
        """
        ...

    def __repr__(self) -> str: ...

def decompose_levels(
    model: CreditFactorModel,
    observed_spreads_json: str,
    observed_generic: float,
    as_of: str,
    runtime_tags_json: str | None = None,
) -> LevelsAtDate:
    """Decompose observed issuer spreads into hierarchy levels and adders.

    Parameters
    ----------
    model : CreditFactorModel
        Calibrated hierarchy artifact.
    observed_spreads_json : str
        JSON map of issuer ID to observed spread in basis points.
    observed_generic : float
        Observed market generic spread in basis points.
    as_of : str
        Observation date as ISO 8601 ``YYYY-MM-DD``.
    runtime_tags_json : str, optional
        Optional JSON map of runtime tags for bucket assignment overrides.

    Returns
    -------
    LevelsAtDate
        Decomposed levels at ``as_of``.

    Raises
    ------
    ValueError
        If spreads, dates, or model references are invalid.

    Examples
    --------
    >>> from finstack_quant.factor_model.credit import decompose_levels
    >>> levels = decompose_levels(model, spreads_json, 120.0, "2025-06-30")  # doctest: +SKIP
    """
    ...

def decompose_period(
    from_levels: LevelsAtDate,
    to_levels: LevelsAtDate,
) -> PeriodDecomposition:
    """Compute period-over-period deltas between two level decompositions.

    Parameters
    ----------
    from_levels : LevelsAtDate
        Start-of-period decomposition.
    to_levels : LevelsAtDate
        End-of-period decomposition.

    Returns
    -------
    PeriodDecomposition
        Bucket and adder deltas between the two dates.

    Raises
    ------
    ValueError
        If the two decompositions are incompatible (e.g. different models).

    Examples
    --------
    >>> from finstack_quant.factor_model.credit import decompose_period
    >>> delta = decompose_period(levels_t0, levels_t1)  # doctest: +SKIP
    """
    ...

__all__ = [
    "CreditFactorModel",
    "CreditCalibrator",
    "LevelsAtDate",
    "PeriodDecomposition",
    "FactorCovarianceForecast",
    "decompose_levels",
    "decompose_period",
]
