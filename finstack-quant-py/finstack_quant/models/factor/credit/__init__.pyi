"""
Credit factor hierarchy: calibration, decomposition, and covariance forecasts.

Bindings for ``finstack-quant-models::factor`` credit hierarchy artifacts. Models
are JSON-first: calibrate or load a :class:`CreditFactorModel`, decompose
observed spreads into level/adder components, link period-to-period changes, and
forecast factor covariance for risk reporting.

Examples
--------
>>> from finstack_quant.models.factor.credit import CreditFactorModel
>>> try:
...     CreditFactorModel.from_json("{}")
... except ValueError as exc:
...     "missing field" in str(exc)
True
"""

from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any

import numpy as np
import numpy.typing as npt
import pandas as pd

class CreditFactorModel:
    """
    Calibrated credit factor hierarchy artifact.

    Produced by :class:`CreditCalibrator` or loaded via :meth:`from_json`. All
    fields are read-only; mutations require re-calibration.

    Examples
    --------
    >>> from finstack_quant.models.factor.credit import CreditCalibrator, CreditFactorModel
    >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal", "bucket_weighting": "equal"}
    >>> inputs = {
    ...     "history_panel": {"dates": ["2024-01-01", "2024-02-01"], "spreads": {"A": [0.010, 0.0101]}},
    ...     "issuer_tags": {"tags": {"A": {}}},
    ...     "generic_factor": {"spec": {"name": "G", "series_id": "G"}, "values": [0.010, 0.0101]},
    ...     "as_of": "2024-02-01",
    ...     "as_of_spreads": {"A": 0.0101},
    ...     "idiosyncratic_overrides": {},
    ... }
    >>> calibrated = CreditCalibrator(config).calibrate(inputs)
    >>> CreditFactorModel.from_json(calibrated.to_json()).schema
    'finstack_quant.credit_factor_model/1'
    """

    @staticmethod
    def from_json(json: str) -> CreditFactorModel:
        """
        Deserialize a credit factor model from JSON.

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

        Examples
        --------
        >>> from finstack_quant.models.factor.credit import CreditFactorModel
        >>> try:
        ...     CreditFactorModel.from_json("{}")
        ... except ValueError as exc:
        ...     "missing field" in str(exc)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this model to pretty-printed JSON.

        Returns
        -------
        str
            JSON suitable for storage or transmission.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def schema(self) -> str:
        """
        Namespaced schema marker (``"finstack_quant.credit_factor_model/1"``).

        Returns
        -------
        str
            Exact contract marker embedded in the artifact.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def as_of(self) -> str:
        """
        Calibration anchor date as an ISO 8601 string.

        Returns
        -------
        str
            Model as-of date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def n_levels(self) -> int:
        """
        Number of hierarchy levels (broadest to narrowest).

        Returns
        -------
        int
            Count of hierarchy dimensions.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def n_issuers(self) -> int:
        """
        Number of issuer beta rows in the artifact.

        Returns
        -------
        int
            Issuer count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def n_factors(self) -> int:
        """
        Number of systematic factors in the model configuration.

        Returns
        -------
        int
            Factor count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def calibration_window(self) -> tuple[datetime.date, datetime.date]:
        """
        History window consumed by calibration.

        Returns
        -------
        tuple[datetime.date, datetime.date]
            ``(start, end)`` of the calibration panel, both inclusive.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def policy(self) -> str:
        """
        Issuer-beta policy used during calibration.

        Returns
        -------
        str
            Serde label such as ``"globally_off"`` or ``"globally_on"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def panel_frequency(self) -> str:
        """
        Panel observation frequency that fixed the annualization factor.

        Returns
        -------
        str
            ``"daily"``, ``"monthly"`` or ``"quarterly"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def bucket_weighting(self) -> str:
        """
        Bucket-mean weighting used at calibration.

        Returns
        -------
        str
            ``"equal"`` or ``"dts"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def config(self) -> FactorModelConfig:
        """
        Point-in-time factor-model configuration embedded in the artifact.

        Returns
        -------
        FactorModelConfig
            Factors, covariance and matching rules used for point-in-time risk.

        Notes
        -----
        This accessor does not raise; it returns a copy of the stored value.
        """
        ...

    @property
    def covariance(self) -> FactorCovarianceMatrix:
        """
        Point-in-time factor covariance matrix (``config.covariance``).

        Returns
        -------
        FactorCovarianceMatrix
            Annualized covariance aligned with ``factor_ids()``.

        Notes
        -----
        This accessor does not raise; it returns a copy of the stored value.
        """
        ...

    @property
    def diagnostics(self) -> dict[str, Any]:
        """
        Structured calibration diagnostics.

        Returns
        -------
        dict[str, Any]
            Canonical serde fields: ``mode_counts``, ``bucket_sizes_per_level``,
            ``fold_ups`` and, when present, ``r_squared_histogram``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def static_correlation(self) -> dict[str, Any]:
        """
        Static factor correlation matrix ``rho`` used by vol forecasting.

        Returns
        -------
        dict[str, Any]
            ``{"factor_ids": [...], "data": [[...], ...]}`` with unit diagonal.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
            Export the per-issuer beta rows as a pandas DataFrame.

            Returns
            -------
            pd.DataFrame
                One row per issuer, sorted by ``issuer_id``. Columns:
                ``issuer_id``, ``tags`` (dict), ``mode`` (``"issuer_beta"`` /
                ``"bucket_only"``), ``beta_pc``, ``beta_levels`` (list aligned with
                ``level_names()``; ``0.0`` marks a folded level), ``adder_at_anchor``
                (bp), ``adder_vol_annualized`` (bp per square-root year), ``adder_vol_source``,
                ``r_squared`` and ``n_obs`` (``NaN`` for bucket-only rows) and
                ``spread_duration`` (years).

            Raises
            ------
            ValueError
                If a row cannot be serialized.

            Examples
            --------
            >>> from finstack_quant.models.factor.credit import CreditCalibrator
        >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal", "bucket_weighting": "equal"}
        >>> inputs = {
        ...     "history_panel": {"dates": ["2024-01-01", "2024-02-01"], "spreads": {"A": [0.010, 0.0101]}},
        ...     "issuer_tags": {"tags": {"A": {}}},
        ...     "generic_factor": {"spec": {"name": "G", "series_id": "G"}, "values": [0.010, 0.0101]},
        ...     "as_of": "2024-02-01",
        ...     "as_of_spreads": {"A": 0.0101},
        ...     "idiosyncratic_overrides": {},
        ... }
            >>> list(CreditCalibrator(config).calibrate(inputs).to_dataframe()["issuer_id"])
            ['A']
        """
        ...

    def level_names(self) -> list[str]:
        """
        Return hierarchy level names.

        Returns
        -------
        list[str]
            Dimension names (e.g. ``["Rating", "Region", "Sector"]``).

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def issuer_ids(self) -> list[str]:
        """
        Return issuer IDs present in the artifact.

        Returns
        -------
        list[str]
            Issuer identifier strings.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def factor_ids(self) -> list[str]:
        """
        Return factor IDs in the model configuration.

        Returns
        -------
        list[str]
            Factor identifier strings.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str: ...

class CreditCalibrator:
    """
    Deterministic calibrator that produces a :class:`CreditFactorModel`.

    The configuration is a ``CreditCalibrationConfig`` given as a dict or a
    JSON string; every field has a default, so a partial dict such as
    ``{"hierarchy": {"levels": ["rating"]}}`` is accepted, and ``None`` selects
    the all-defaults configuration.

    Examples
    --------
    >>> from finstack_quant.models.factor.credit import CreditCalibrator
    >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal", "bucket_weighting": "equal"}
    >>> inputs = {
    ...     "history_panel": {"dates": ["2024-01-01", "2024-02-01"], "spreads": {"A": [0.010, 0.0101]}},
    ...     "issuer_tags": {"tags": {"A": {}}},
    ...     "generic_factor": {"spec": {"name": "G", "series_id": "G"}, "values": [0.010, 0.0101]},
    ...     "as_of": "2024-02-01",
    ...     "as_of_spreads": {"A": 0.0101},
    ...     "idiosyncratic_overrides": {},
    ... }
    >>> CreditCalibrator(config).calibrate(inputs).n_issuers
    1
    """

    def __init__(self, config: Mapping[str, Any] | str | None = None) -> None:
        """
        Construct a calibrator from a ``CreditCalibrationConfig``.

        Parameters
        ----------
        config : Mapping[str, Any] | str | None
            ``CreditCalibrationConfig`` as a dict or JSON string. Omitted
            fields take their defaults (``policy="globally_off"``, empty
            hierarchy, ``vol_model="sample"``,
            ``covariance_strategy="full_sample_repaired"``,
            ``beta_shrinkage="none"``, ``use_returns_or_levels="returns"``,
            ``panel_frequency="monthly"``, ``bucket_weighting="dts"``).
            ``None`` selects the all-defaults configuration.

        Raises
        ------
        ValueError
            If ``config`` names an unknown field or an invalid enum label.
        """
        ...

    def calibrate(self, inputs: Mapping[str, Any] | str) -> CreditFactorModel:
        """
            Run calibration and return a validated factor model.

            Parameters
            ----------
            inputs : Mapping[str, Any] | str
                ``CreditCalibrationInputs`` as a dict or JSON string:
                ``history_panel`` (``dates`` + per-issuer decimal spread lists),
                ``issuer_tags``, ``generic_factor``, ``as_of``, ``as_of_spreads``
                and optional ``idiosyncratic_overrides`` / ``spread_durations``.
                Spreads are decimal (``0.01`` = 100 bp). Overrides are decimal
                spread volatility per square-root year (``0.001`` = 10 bp/√year).

            Returns
            -------
            CreditFactorModel
                Calibrated hierarchy artifact.

            Raises
            ------
            ValueError
                If ``inputs`` is structurally invalid or calibration rejects the
                panel (irregular grid, missing tags, bad EWMA lambda, ...).

            Examples
            --------
            >>> from finstack_quant.models.factor.credit import CreditCalibrator
        >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal", "bucket_weighting": "equal"}
        >>> inputs = {
        ...     "history_panel": {"dates": ["2024-01-01", "2024-02-01"], "spreads": {"A": [0.010, 0.0101]}},
        ...     "issuer_tags": {"tags": {"A": {}}},
        ...     "generic_factor": {"spec": {"name": "G", "series_id": "G"}, "values": [0.010, 0.0101]},
        ...     "as_of": "2024-02-01",
        ...     "as_of_spreads": {"A": 0.0101},
        ...     "idiosyncratic_overrides": {},
        ... }
            >>> CreditCalibrator(config).calibrate(inputs).n_factors
            1
        """
        ...

    @staticmethod
    def from_dataframe(
        spreads: pd.DataFrame,
        tags: Mapping[str, Mapping[str, str]] | pd.DataFrame,
        generic: pd.Series | list[float],
        as_of: datetime.date | str | None = None,
        spread_durations: Mapping[str, float] | pd.Series | None = None,
        config: Mapping[str, Any] | str | None = None,
    ) -> CreditFactorModel:
        """
        Calibrate straight from pandas objects.

        Builds the ``CreditCalibrationInputs`` from the frames (pure
        conversion) and runs :meth:`calibrate` under ``config``.

        Parameters
        ----------
        spreads : pd.DataFrame
            Decimal spreads (``0.01`` = 100 bp) with a date index (sorted,
            regular grid) and one column per issuer; ``NaN`` marks a gap.
        tags : Mapping[str, Mapping[str, str]] | pd.DataFrame
            ``{issuer: {dimension_key: tag}}`` or a DataFrame indexed by
            issuer with one column per hierarchy dimension (``"rating"``,
            ``"region"``, ...).
        generic : pd.Series | list[float]
            Generic (PC) factor series aligned with ``spreads.index``; a
            Series' ``name`` becomes the factor name.
        as_of : datetime.date | str | None
            Anchor date; defaults to the last index date.
        spread_durations : Mapping[str, float] | pd.Series | None
            ``{issuer: years}``; required when ``bucket_weighting="dts"``.
        config : Mapping[str, Any] | str | None
            ``CreditCalibrationConfig`` dict / JSON string / ``None`` (see
            :meth:`__init__`).

        Returns
        -------
        CreditFactorModel
            Calibrated hierarchy artifact.

        Raises
        ------
        ValueError
            If the frames are misaligned, ``as_of`` is not an index date, or
            calibration rejects the inputs.

        Examples
        --------
        >>> import pandas as pd
        >>> from finstack_quant.models.factor.credit import CreditCalibrator
        >>> spreads = pd.DataFrame({"A": [0.010, 0.0101]}, index=["2024-01-01", "2024-02-01"])
        >>> model = CreditCalibrator.from_dataframe(
        ...     spreads,
        ...     {"A": {}},
        ...     [0.010, 0.0101],
        ...     config={"covariance_strategy": "diagonal", "bucket_weighting": "equal"},
        ... )
        >>> model.n_issuers
        1
        """
        ...

    @property
    def config(self) -> dict[str, Any]:
        """
        The calibration configuration as a dict (canonical serde fields).

        Returns
        -------
        dict[str, Any]
            ``CreditCalibrationConfig`` with every field populated.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...

class LevelsAtDate:
    """
    Decomposed credit spread levels at a single observation date.

    Examples
    --------
    >>> from finstack_quant.models.factor.credit import CreditCalibrator, decompose_levels
    >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal", "bucket_weighting": "equal"}
    >>> inputs = {
    ...     "history_panel": {"dates": ["2024-01-01", "2024-02-01"], "spreads": {"A": [0.010, 0.0101]}},
    ...     "issuer_tags": {"tags": {"A": {}}},
    ...     "generic_factor": {"spec": {"name": "G", "series_id": "G"}, "values": [0.010, 0.0101]},
    ...     "as_of": "2024-02-01",
    ...     "as_of_spreads": {"A": 0.0101},
    ...     "idiosyncratic_overrides": {},
    ... }
    >>> model = CreditCalibrator(config).calibrate(inputs)
    >>> levels = decompose_levels(model, {"A": 0.0105}, 0.010, "2024-03-01")
    >>> (levels.date, levels.generic, levels.adder())
    ('2024-03-01', 100.0, {'A': 5.0})
    """

    @staticmethod
    def from_json(json: str) -> LevelsAtDate:
        """Deserialize a snapshot from canonical JSON.

        Parameters
        ----------
        json:
            Canonical ``LevelsAtDate`` JSON object.

        Returns
        -------
        LevelsAtDate
            Parsed typed decomposition snapshot.

        Raises
        ------
        ValueError
            If the payload is malformed or contains non-finite values.

        Examples
        --------
        >>> LevelsAtDate.from_json('{"date":"2024-01-01","generic":0.0,"by_level":[],"adder":{}}').date
        '2024-01-01'
        """
        ...

    def to_json(self) -> str:
        """Serialize this snapshot to compact canonical JSON.

        Returns
        -------
        str
            Canonical compact JSON for this snapshot.

        Raises
        ------
        ValueError
            If a numeric field is non-finite or serialization fails.
        """
        ...

    @property
    def date(self) -> str:
        """
        Observation date as an ISO 8601 string.

        Returns
        -------
        str
            Decomposition date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def generic(self) -> float:
        """
        Generic (market-wide) spread component in basis points.

        Returns
        -------
        float
            Generic level in bp.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def n_levels(self) -> int:
        """
        Number of hierarchy levels in the decomposition.

        Returns
        -------
        int
            Level count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def level_values(self, level_index: int) -> dict[str, float]:
        """
        Return bucket values for one hierarchy level.

        Parameters
        ----------
        level_index : int
            Zero-based level index (0 = broadest).

        Returns
        -------
        dict[str, float]
            Map of bucket label to spread contribution in bp.

        Raises
        ------
        ValueError
            If ``level_index`` is out of range.
        """
        ...

    def adder(self) -> dict[str, float]:
        """
        Return issuer-specific adder spreads keyed by issuer ID.

        Returns
        -------
        dict[str, float]
            Per-issuer adder in bp.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-level bucket values as a pandas DataFrame.

        Columns: ``date``, ``level_index``, ``dimension``, ``bucket``,
        ``value``.

        One row per (level, bucket) pair. ``date`` repeats on every row as an
        ISO string so a row survives ``pd.concat`` across dates. Rows are
        ordered by ``level_index``, then by ``bucket`` — the values are a
        sorted map, so repeated exports are identical.

        The scalar ``generic`` factor and the per-issuer residuals are not
        levels; read them from the ``generic`` property and :meth:`to_series`
        respectively.

        Returns
        -------
        pd.DataFrame
            One row per (level, bucket) pair. A snapshot from a hierarchy with
            no levels yields a zero-row frame that still carries the columns
            above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_series(self) -> pd.Series:
        """
        Export the per-issuer residual adders as a pandas Series.

        Returns
        -------
        pd.Series
            Named ``adder``, indexed by issuer ID, holding the residual after
            peeling the generic factor and every hierarchy level. Issuers are
            in sorted order, so repeated exports and
            ``pd.concat([...], axis=1)`` across dates align on the index.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str: ...

class PeriodDecomposition:
    """
    Period-over-period change in decomposed credit spread levels.

    Examples
    --------
    >>> from finstack_quant.models.factor.credit import CreditCalibrator, decompose_levels, decompose_period
    >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal", "bucket_weighting": "equal"}
    >>> inputs = {
    ...     "history_panel": {"dates": ["2024-01-01", "2024-02-01"], "spreads": {"A": [0.010, 0.0101]}},
    ...     "issuer_tags": {"tags": {"A": {}}},
    ...     "generic_factor": {"spec": {"name": "G", "series_id": "G"}, "values": [0.010, 0.0101]},
    ...     "as_of": "2024-02-01",
    ...     "as_of_spreads": {"A": 0.0101},
    ...     "idiosyncratic_overrides": {},
    ... }
    >>> model = CreditCalibrator(config).calibrate(inputs)
    >>> start = decompose_levels(model, {"A": 0.0105}, 0.010, "2024-03-01")
    >>> end = decompose_levels(model, {"A": 0.01065}, 0.01015, "2024-03-02")
    >>> period = decompose_period(start, end)
    >>> (period.from_date, period.to_date, period.d_generic)
    ('2024-03-01', '2024-03-02', 1.5)
    """

    @staticmethod
    def from_json(json: str) -> PeriodDecomposition:
        """Deserialize a decomposition from canonical JSON.

        Parameters
        ----------
        json:
            Canonical ``PeriodDecomposition`` JSON object.

        Returns
        -------
        PeriodDecomposition
            Parsed typed period decomposition.

        Raises
        ------
        ValueError
            If the payload is malformed or contains non-finite values.

        Examples
        --------
        >>> payload = '{"from":"2024-01-01","to":"2024-01-02","d_generic":0.0,"by_level":[],"d_adder":{}}'
        >>> PeriodDecomposition.from_json(payload).from_date
        '2024-01-01'
        """
        ...

    def to_json(self) -> str:
        """Serialize this decomposition to compact canonical JSON.

        Returns
        -------
        str
            Canonical compact JSON for this decomposition.

        Raises
        ------
        ValueError
            If a numeric field is non-finite or serialization fails.
        """
        ...

    @property
    def from_date(self) -> str:
        """
        Start date of the decomposition window (ISO 8601).

        Returns
        -------
        str
            Period start.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def to_date(self) -> str:
        """
        End date of the decomposition window (ISO 8601).

        Returns
        -------
        str
            End date of the decomposition window (ISO 8601).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def d_generic(self) -> float:
        """
        Change in generic spread over the period (bp).

        Returns
        -------
        float
            Generic delta in bp.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def n_levels(self) -> int:
        """
        Number of hierarchy levels.

        Returns
        -------
        int
            Level count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def level_deltas(self, level_index: int) -> dict[str, float]:
        """
        Return bucket deltas for one hierarchy level.

        Parameters
        ----------
        level_index : int
            Zero-based level index.

        Returns
        -------
        dict[str, float]
            Map of bucket label to spread change in bp.

        Raises
        ------
        ValueError
            If ``level_index`` is out of range.
        """
        ...

    def d_adder(self) -> dict[str, float]:
        """
        Return issuer adder changes keyed by issuer ID.

        Returns
        -------
        dict[str, float]
            Per-issuer adder delta in bp.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-level bucket deltas as a pandas DataFrame.

        Columns: ``from_date``, ``to_date``, ``level_index``, ``dimension``,
        ``bucket``, ``delta``.

        This is the default export and the same table as
        :meth:`to_level_dataframe`. The per-issuer adder deltas are a
        separate, differently-keyed table; see :meth:`to_adder_dataframe`. The
        scalar ``d_generic`` is metadata and is not repeated per row.

        Returns
        -------
        pd.DataFrame
            One row per (level, bucket) pair. A decomposition with no
            hierarchy levels yields a zero-row frame that still carries the
            columns above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_level_dataframe(self) -> pd.DataFrame:
        """
        Export the per-level bucket deltas as a pandas DataFrame.

        Columns: ``from_date``, ``to_date``, ``level_index``, ``dimension``,
        ``bucket``, ``delta``. Identical to :meth:`to_dataframe`.

        One row per (level, bucket) pair — the long format the level deltas
        naturally take, since each level has its own bucket set. The two dates
        repeat on every row as ISO strings so a row survives ``pd.concat``
        across periods. Rows are ordered by ``level_index``, then by
        ``bucket`` — the deltas are a sorted map, so bucket order is the sorted
        key order and repeated exports are identical.

        A decomposition with no hierarchy levels yields a zero-row frame that
        still carries the columns above.

        Returns
        -------
        pd.DataFrame
            Long-format frame of bucket deltas, one row per (level, bucket).

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_adder_dataframe(self) -> pd.DataFrame:
        """
        Export the per-issuer adder deltas as a pandas DataFrame.

        Columns: ``from_date``, ``to_date``, ``issuer_id``, ``d_adder``.

        One row per issuer. The two dates repeat on every row as ISO strings so
        a row survives ``pd.concat`` across periods. Rows are ordered by
        ``issuer_id`` — the adders are a sorted map, so this is the sorted key
        order and repeated exports are identical.

        A decomposition sharing no issuers between snapshots yields a zero-row
        frame that still carries the columns above.

        Returns
        -------
        pd.DataFrame
            One row per issuer present in both snapshots.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str: ...

class FactorCovarianceMatrix:
    """
    Validated factor covariance matrix with deterministic row-major storage.

    Entries are annualized covariances in the factors' native units. The
    constructor validates squareness, unique identifiers, symmetry and positive
    semidefiniteness. Instances compare equal when identifiers and data match.

    Examples
    --------
    >>> from finstack_quant.models.factor.credit import FactorCovarianceMatrix
    >>> matrix = FactorCovarianceMatrix(["a", "b"], [[0.04, 0.0], [0.0, 0.01]])
    >>> (matrix.variance("a"), float(matrix.to_dataframe().loc["b", "b"]))
    (0.04, 0.01)
    """

    def __init__(
        self,
        factor_ids: list[str],
        data: list[float] | list[list[float]] | npt.NDArray[np.float64],
    ) -> None:
        """
        Build and validate a covariance matrix.

        Parameters
        ----------
        factor_ids : list[str]
            Ordered, unique factor identifiers defining both axes.
        data : list[float] | list[list[float]] | numpy.ndarray
            Annualized covariances — a nested list or 2-D array of shape
            ``(n, n)``, or a flat row-major list of ``n * n`` values, in
            ``factor_ids`` order.

        Raises
        ------
        ValueError
            If ``data`` is not ``n x n``, an identifier repeats, the matrix is
            asymmetric, or it is not positive semidefinite.
        """
        ...

    @staticmethod
    def from_json(json: str) -> FactorCovarianceMatrix:
        """Deserialize and validate a covariance matrix from canonical JSON.

        Parameters
        ----------
        json : str
            Object with ordered ``factor_ids``, dimension ``n``, and row-major ``data``.

        Returns
        -------
        FactorCovarianceMatrix
            Validated symmetric positive-semidefinite covariance matrix.

        Raises
        ------
        ValueError
            If JSON is malformed, dimensions disagree, IDs repeat, or the matrix is invalid.

        Examples
        --------
        >>> FactorCovarianceMatrix.from_json('{"factor_ids":[],"n":0,"data":[]}').n_factors
        0
        """
        ...

    def to_json(self) -> str:
        """Serialize this matrix to canonical JSON.

        Returns
        -------
        str
            Compact JSON with ordered axes and row-major covariance data.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @property
    def n_factors(self) -> int:
        """Return the number of covariance axes.

        Returns
        -------
        int
            Number of factor IDs, equal to the matrix row and column count.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

    @property
    def factor_ids(self) -> list[str]:
        """Return ordered factor identifiers for rows and columns.

        Returns
        -------
        list[str]
            Independent copy of the canonical covariance-axis order.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

    @property
    def data(self) -> list[float]:
        """Return row-major annualized covariance values.

        Returns
        -------
        list[float]
            ``n_factors * n_factors`` entries in canonical factor order.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

    def variance(self, factor_id: str) -> float:
        """Return one factor's annualized variance.

        Parameters
        ----------
        factor_id : str
            Canonical factor identifier to query.

        Returns
        -------
        float
            Diagonal covariance entry, or ``0.0`` when the factor is unknown.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    def covariance(self, lhs: str, rhs: str) -> float:
        """Return annualized covariance between two factors.

        Parameters
        ----------
        lhs : str
            Row factor identifier.
        rhs : str
            Column factor identifier.

        Returns
        -------
        float
            Covariance entry, or ``0.0`` when either factor is unknown.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    def correlation(self, lhs: str, rhs: str) -> float:
        """Return correlation between two factors.

        Parameters
        ----------
        lhs : str
            First factor identifier.
        rhs : str
            Second factor identifier.

        Returns
        -------
        float
            Covariance normalized by both standard deviations; ``0.0`` for unknown or zero-variance factors.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    def to_numpy(self) -> npt.NDArray[np.float64]:
        """
        The matrix as an ``(n, n)`` float64 NumPy array.

        Returns
        -------
        numpy.ndarray
            Row-major covariance in ``factor_ids`` order.

        Notes
        -----
        This method does not raise; it copies the stored data.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        The matrix as a square pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            Indexed and columned by ``factor_ids``.

        Raises
        ------
        ValueError
            If pandas cannot build the frame.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str: ...

class FactorModelConfig:
    """
    Portfolio factor-model configuration assembled at a forecast horizon.

    Examples
    --------
    >>> from finstack_quant.models.factor.credit import FactorModelConfig
    >>> config = FactorModelConfig.from_json(
    ...     '{"factors":[],"covariance":{"factor_ids":[],"n":0,"data":[]},"matching":{"mapping_table":[]},"pricing_mode":"delta_based","risk_measure":"variance"}'
    ... )
    >>> config.n_factors
    0
    """

    @staticmethod
    def from_json(json: str) -> FactorModelConfig:
        """Deserialize and validate a factor-model configuration.

        Parameters
        ----------
        json : str
            Canonical Rust ``FactorModelConfig`` JSON.

        Returns
        -------
        FactorModelConfig
            Typed configuration with matching and covariance invariants checked.

        Raises
        ------
        ValueError
            If JSON is malformed or matching rules reference undeclared factors.

        Examples
        --------
        >>> FactorModelConfig.from_json(
        ...     '{"factors":[],"covariance":{"factor_ids":[],"n":0,"data":[]},"matching":{"mapping_table":[]},"pricing_mode":"delta_based","risk_measure":"variance"}'
        ... ).factor_ids
        []
        """
        ...

    def to_json(self) -> str:
        """Serialize this configuration to canonical JSON.

        Returns
        -------
        str
            Compact JSON accepted by factor-risk workflows.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def validate(self) -> None:
        """Validate that matching rules emit only declared factor IDs.

        Raises
        ------
        ValueError
            If a matcher references an undeclared factor or duplicates issuer rows.
        """
        ...

    @property
    def n_factors(self) -> int:
        """Return the number of configured factors.

        Returns
        -------
        int
            Length of the factor definition list.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

    @property
    def factor_ids(self) -> list[str]:
        """Return factor-definition IDs in canonical order.

        Returns
        -------
        list[str]
            Ordered IDs aligned to covariance axes.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

    @property
    def factors(self) -> list[dict[str, Any]]:
        """Return structured factor definitions.

        Returns
        -------
        list[dict[str, Any]]
            Independent Python representation of canonical factor-definition fields.

        Raises
        ------
        ValueError
            If conversion to Python values fails.
        """
        ...

    @property
    def covariance(self) -> FactorCovarianceMatrix:
        """Return the covariance matrix aligned to ``factor_ids``.

        Returns
        -------
        FactorCovarianceMatrix
            Independent typed covariance wrapper.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

    @property
    def matching(self) -> dict[str, Any]:
        """Return the declarative factor-matching configuration.

        Returns
        -------
        dict[str, Any]
            Structured Python representation of the canonical matching variant.

        Raises
        ------
        ValueError
            If conversion to Python values fails.
        """
        ...

    @property
    def pricing_mode(self) -> str:
        """Return the sensitivity extraction strategy.

        Returns
        -------
        str
            ``"delta_based"`` or ``"full_repricing"``.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

    @property
    def risk_measure(self) -> str | dict[str, Any]:
        """Return the canonical risk-measure value.

        Returns
        -------
        str or dict[str, Any]
            Scalar label for variance/volatility or structured VaR/ES parameters.

        Raises
        ------
        ValueError
            If conversion to Python values fails.
        """
        ...

    @property
    def bump_size(self) -> dict[str, Any] | None:
        """Return optional finite-difference bump overrides.

        Returns
        -------
        dict[str, Any] or None
            Structured bump configuration, or ``None`` when defaults apply.

        Raises
        ------
        ValueError
            If conversion to Python values fails.
        """
        ...

    @property
    def unmatched_policy(self) -> str | None:
        """Return the policy for unmatched dependencies.

        Returns
        -------
        str or None
            ``"strict"``, ``"residual"``, ``"warn"``, or ``None`` for the default.

        Raises
        ------
        None
            This property does not raise.
        """
        ...

class FactorCovarianceForecast:
    """
    Factor covariance and idiosyncratic vol forecasts from a credit factor model.

    Every method takes a ``horizon`` that is either a :class:`VolHorizon` or a
    descriptor string: ``"one_step"``, ``"unconditional"``,
    ``'{"n_steps": N}'`` or ``'{"years": Y}'``.

    Examples
    --------
    >>> from finstack_quant.models.factor.credit import CreditCalibrator, FactorCovarianceForecast, VolHorizon
    >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal", "bucket_weighting": "equal"}
    >>> inputs = {
    ...     "history_panel": {"dates": ["2024-01-01", "2024-02-01"], "spreads": {"A": [0.010, 0.0101]}},
    ...     "issuer_tags": {"tags": {"A": {}}},
    ...     "generic_factor": {"spec": {"name": "G", "series_id": "G"}, "values": [0.010, 0.0101]},
    ...     "as_of": "2024-02-01",
    ...     "as_of_spreads": {"A": 0.0101},
    ...     "idiosyncratic_overrides": {},
    ... }
    >>> model = CreditCalibrator(config).calibrate(inputs)
    >>> forecast = FactorCovarianceForecast(model)
    >>> forecast.covariance_at(VolHorizon.one_step()).factor_ids
    ['credit::generic']
    """

    def __init__(self, model: CreditFactorModel) -> None:
        """
        Bind a covariance forecast engine to a calibrated model.

        Parameters
        ----------
        model : CreditFactorModel
            Calibrated hierarchy artifact used as the risk factor basis.

        Notes
        -----
        Construction does not raise; the model is copied.
        """
        ...

    def covariance_at(self, horizon: str | VolHorizon) -> FactorCovarianceMatrix:
        """
        Return a typed factor covariance matrix at a forecast horizon.

        Parameters
        ----------
        horizon : str | VolHorizon
            :class:`VolHorizon` or descriptor string (``"one_step"``,
            ``"unconditional"``, ``'{"n_steps": N}'``, ``'{"years": Y}'``).

        Returns
        -------
        FactorCovarianceMatrix
            ``D · rho_static · D`` scaled to the horizon.

        Raises
        ------
        ValueError
            If ``horizon`` is invalid or the model data is inconsistent.
        """
        ...

    def idiosyncratic_vol(self, issuer_id: str, horizon: str | VolHorizon) -> float:
        """
        Return idiosyncratic volatility for an issuer at a horizon.

        Parameters
        ----------
        issuer_id : str
            Issuer identifier present in the model artifact.
        horizon : str | VolHorizon
            :class:`VolHorizon` or descriptor string (see :meth:`covariance_at`).

        Returns
        -------
        float
            Idiosyncratic standard deviation in basis points of spread, scaled
            to the horizon.

        Raises
        ------
        ValueError
            If the issuer or horizon is unknown.
        """
        ...

    def factor_model_at(
        self,
        horizon: str | VolHorizon,
        risk_measure: str | Mapping[str, Any] | None = None,
    ) -> FactorModelConfig:
        """
            Return a typed portfolio-ready factor model at a horizon.

            Parameters
            ----------
            horizon : str | VolHorizon
                :class:`VolHorizon` or descriptor string (see :meth:`covariance_at`).
            risk_measure : str | Mapping[str, Any] | None
                ``"variance"`` (default), ``"volatility"``, or a dict such as
                ``{"var": {"confidence": 0.99}}`` /
                ``{"expected_shortfall": {"confidence": 0.975}}``; a JSON string of
                any of these is also accepted.

            Returns
            -------
            FactorModelConfig
                Configuration with the horizon covariance and requested measure.

            Raises
            ------
            ValueError
                If the horizon or risk measure is invalid (confidence outside
                ``(0.5, 1)``) or the covariance cannot be built.

            Examples
            --------
            >>> from finstack_quant.models.factor.credit import CreditCalibrator, FactorCovarianceForecast
        >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal", "bucket_weighting": "equal"}
        >>> inputs = {
        ...     "history_panel": {"dates": ["2024-01-01", "2024-02-01"], "spreads": {"A": [0.010, 0.0101]}},
        ...     "issuer_tags": {"tags": {"A": {}}},
        ...     "generic_factor": {"spec": {"name": "G", "series_id": "G"}, "values": [0.010, 0.0101]},
        ...     "as_of": "2024-02-01",
        ...     "as_of_spreads": {"A": 0.0101},
        ...     "idiosyncratic_overrides": {},
        ... }
            >>> model = CreditCalibrator(config).calibrate(inputs)
            >>> FactorCovarianceForecast(model).factor_model_at("one_step", {"var": {"confidence": 0.99}}).risk_measure
            {'var': {'confidence': 0.99}}
        """
        ...

    def __repr__(self) -> str: ...

def decompose_levels(
    model: CreditFactorModel,
    observed_spreads: Mapping[str, float] | pd.Series | str,
    observed_generic: float,
    as_of: datetime.date | str,
    runtime_tags: Mapping[str, Mapping[str, str]] | pd.DataFrame | str | None = None,
) -> LevelsAtDate:
    """
    Decompose observed issuer spreads into hierarchy levels and adders.

    Parameters
    ----------
    model : CreditFactorModel
        Calibrated hierarchy artifact.
    observed_spreads : Mapping[str, float] | pd.Series | str
        Issuer ID to observed **decimal** spread (``0.01`` = 100 bp) — a dict,
        a ``pandas.Series`` indexed by issuer, or a JSON string of the same
        object.
    observed_generic : float
        Generic (PC) factor value at ``as_of``, decimal.
    as_of : datetime.date | str
        Observation date, either a date-like object or an ISO 8601 string.
    runtime_tags : Mapping[str, Mapping[str, str]] | pd.DataFrame | str | None
        ``{issuer_id: {dimension_key: tag}}`` for issuers not present in the
        model — a dict, a ``pandas.DataFrame`` indexed by issuer, or a JSON
        string.

    Returns
    -------
    LevelsAtDate
        Decomposed levels at ``as_of`` (basis points).

    Raises
    ------
    KeyError
        If an issuer has no model row and no ``runtime_tags`` entry.
    ValueError
        If a spread is not a finite decimal in ``(-0.5, 2.0)``, an issuer is
        missing a required hierarchy tag, or a DTS weight cannot be formed.
    RuntimeError
        If the model artifact is internally inconsistent.

    Examples
    --------
    >>> from finstack_quant.models.factor.credit import CreditCalibrator, decompose_levels
    >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal", "bucket_weighting": "equal"}
    >>> inputs = {
    ...     "history_panel": {"dates": ["2024-01-01", "2024-02-01"], "spreads": {"A": [0.010, 0.0101]}},
    ...     "issuer_tags": {"tags": {"A": {}}},
    ...     "generic_factor": {"spec": {"name": "G", "series_id": "G"}, "values": [0.010, 0.0101]},
    ...     "as_of": "2024-02-01",
    ...     "as_of_spreads": {"A": 0.0101},
    ...     "idiosyncratic_overrides": {},
    ... }
    >>> model = CreditCalibrator(config).calibrate(inputs)
    >>> decompose_levels(model, {"A": 0.0125}, 0.0120, "2025-06-30").generic
    120.0
    """
    ...

def decompose_period(
    from_levels: LevelsAtDate,
    to_levels: LevelsAtDate,
) -> PeriodDecomposition:
    """
    Compute period-over-period deltas between two level decompositions.

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
    >>> from finstack_quant.models.factor.credit import CreditCalibrator, decompose_levels, decompose_period
    >>> config = {"hierarchy": {"levels": []}, "covariance_strategy": "diagonal", "bucket_weighting": "equal"}
    >>> inputs = {
    ...     "history_panel": {"dates": ["2024-01-01", "2024-02-01"], "spreads": {"A": [0.010, 0.0101]}},
    ...     "issuer_tags": {"tags": {"A": {}}},
    ...     "generic_factor": {"spec": {"name": "G", "series_id": "G"}, "values": [0.010, 0.0101]},
    ...     "as_of": "2024-02-01",
    ...     "as_of_spreads": {"A": 0.0101},
    ...     "idiosyncratic_overrides": {},
    ... }
    >>> model = CreditCalibrator(config).calibrate(inputs)
    >>> start = decompose_levels(model, {"A": 0.0105}, 0.010, "2024-03-01")
    >>> end = decompose_levels(model, {"A": 0.01065}, 0.01015, "2024-03-02")
    >>> decompose_period(start, end).d_generic
    1.5
    """
    ...

class VolHorizon:
    """
    Forecast horizon used to scale a calibrated ``Sample`` vol estimate.

    Instances compare equal by variant and value, and pickle through their
    descriptor string.

    Examples
    --------
    >>> from finstack_quant.models.factor.credit import VolHorizon
    >>> VolHorizon.n_steps(5).n
    5
    """

    @classmethod
    def one_step(cls) -> VolHorizon:
        """
        Use the calibrated one-step forecast horizon.

        Returns
        -------
        VolHorizon
            One-calibrated-period forecast horizon.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.factor.credit import VolHorizon
        >>> VolHorizon.one_step().kind
        'one_step'
        """
        ...

    @classmethod
    def unconditional(cls) -> VolHorizon:
        """
        Use the unconditional long-run forecast horizon.

        Returns
        -------
        VolHorizon
            Unconditional long-run forecast horizon.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.factor.credit import VolHorizon
        >>> VolHorizon.unconditional().kind
        'unconditional'
        """
        ...

    @classmethod
    def n_steps(cls, n: int) -> VolHorizon:
        """
        Scale the forecast to a fixed number of discrete steps.

        Parameters
        ----------
        n : int
            Positive number of calibrated sampling periods to forecast ahead.

        Returns
        -------
        VolHorizon
            Discrete forecast horizon spanning ``n`` calibrated periods.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.factor.credit import VolHorizon
        >>> VolHorizon.n_steps(5).n
        5
        """
        ...

    @classmethod
    def years(cls, years: float) -> VolHorizon:
        """
        Scale the forecast to a year fraction.

        Parameters
        ----------
        years : float
            Positive forecast horizon in years, converted using the calibrated
            model's observation frequency.

        Returns
        -------
        VolHorizon
            Forecast horizon spanning the supplied fractional number of years.

        Raises
        ------
        ValueError
            If ``years`` is non-finite or negative.

        Examples
        --------
        >>> from finstack_quant.models.factor.credit import VolHorizon
        >>> VolHorizon.years(2.5).years_value
        2.5
        """
        ...

    @classmethod
    def parse(cls, s: str) -> VolHorizon:
        """
        Parse a horizon string accepted by the Rust factor model.

        Parameters
        ----------
        s : str
            Horizon expression such as ``"one_step"``, ``"unconditional"``,
            a step count, or a year-based form accepted by the model.

        Returns
        -------
        VolHorizon
            Horizon variant represented by the keyword or JSON descriptor in ``s``.

        Raises
        ------
        ValueError
            If ``s`` is not a horizon descriptor accepted by ``VolHorizon``.

        Examples
        --------
        >>> from finstack_quant.models.factor.credit import VolHorizon
        >>> VolHorizon.parse('{"n_steps":5}').n
        5
        """
        ...

    @property
    def kind(self) -> str:
        """
        Discriminator for the volatility-horizon variant.

        Returns
        -------
        str
            Discriminator for the volatility-horizon variant.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def n(self) -> int | None:
        """
        Step count for ``n_steps`` horizons.

        Returns
        -------
            Step count for ``n_steps`` horizons.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def years_value(self) -> float | None:
        """
        Year fraction for ``years`` horizons.

        Returns
        -------
            Year fraction for ``years`` horizons.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

__all__ = [
    "CreditFactorModel",
    "CreditCalibrator",
    "LevelsAtDate",
    "PeriodDecomposition",
    "VolHorizon",
    "FactorCovarianceForecast",
    "FactorCovarianceMatrix",
    "FactorModelConfig",
    "decompose_levels",
    "decompose_period",
]
