"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/factor_model/credit_vol_report.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
from finstack_quant.models.factor.credit import CreditFactorModel
from finstack_quant.models.factor.risk import RiskDecomposition

class LevelVolContribution:
    """
    Aggregated risk contribution for a single hierarchy level.

    Examples
    --------
    >>> from finstack_quant.portfolio import LevelVolContribution
    >>> try:
    ...     LevelVolContribution()
    ... except TypeError as exc:
    ...     print(exc)
    cannot create 'finstack_quant.portfolio.LevelVolContribution' instances
    """

    @property
    def level_name(self) -> str:
        """
        Name of the hierarchy level that received this volatility contribution.

        Returns
        -------
        str
            The level's dimension key (``"rating"``, ``"region"``,
            ``"sector"`` or a custom dimension's own key), matching
            ``CreditFactorModel.level_names()`` and the issuer-tag keys.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total(self) -> float:
        """
        Total contribution for this level.

        Returns
        -------
        float
            Total contribution for this level.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_bucket(self) -> dict[str, float]:
        """
        Contribution by hierarchy bucket.

        Returns
        -------
        dict[str, float]
            Contribution by hierarchy bucket.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionVolContribution:
    """
    Per-position vol breakdown under :class:`CreditVolReport`.

    Examples
    --------
    >>> from finstack_quant.portfolio import PositionVolContribution
    >>> try:
    ...     PositionVolContribution()
    ... except TypeError as exc:
    ...     print(exc)
    cannot create 'finstack_quant.portfolio.PositionVolContribution' instances
    """

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.

        Returns
        -------
        str
            Portfolio position identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def factor_total(self) -> float:
        """
        Factor-driven volatility contribution.

        Returns
        -------
        float
            Factor-driven volatility contribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def idiosyncratic(self) -> float:
        """
        Idiosyncratic volatility contribution.

        Returns
        -------
        float
            Idiosyncratic volatility contribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total(self) -> float:
        """
        Total position volatility contribution.

        Returns
        -------
        float
            Total position volatility contribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class CreditVolReport:
    """
    Aggregated vol report grouped by hierarchy level.

    Examples
    --------
    >>> from finstack_quant.portfolio import CreditVolReport
    >>> try:
    ...     CreditVolReport()
    ... except TypeError as exc:
    ...     print(exc)
    cannot create 'finstack_quant.portfolio.CreditVolReport' instances
    """

    @staticmethod
    def from_json(json: str) -> CreditVolReport:
        """
        Deserialize from the canonical ``CreditVolReport`` JSON.

        Parameters
        ----------
        json : str
            JSON emitted by :meth:`to_json`, or the object the WASM
            ``buildCreditVolReport`` returns, serialized: ``total``,
            ``measure``, ``generic``, ``by_level``, ``idiosyncratic_total``
            and ``by_position``.

        Returns
        -------
        CreditVolReport
            The reconstructed report.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the report schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import CreditVolReport
        >>> report = CreditVolReport.from_json(
        ...     '{"total":1.0,"measure":"variance","generic":0.25,"by_level":[],'
        ...     '"idiosyncratic_total":0.75,"by_position":null}'
        ... )
        >>> (report.total, report.generic, report.by_position)
        (1.0, 0.25, None)
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to canonical ``CreditVolReport`` JSON.

        Returns
        -------
        str
            Compact JSON with ``total``, ``measure``, ``generic``,
            ``by_level``, ``idiosyncratic_total`` and ``by_position``;
            the same shape the WASM ``buildCreditVolReport`` returns.

        Raises
        ------
        ValueError
            If the report cannot be serialized.
        """
        ...

    @property
    def total(self) -> float:
        """
        Total portfolio volatility under the report measure.

        Returns
        -------
        float
            Total portfolio volatility under the report measure.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def measure(self) -> str | dict[str, Any]:
        """
        Risk measure of the source decomposition, as its canonical Python value.

        Returns
        -------
        str | dict[str, Any]
            ``"variance"``, ``"volatility"``, ``{"var": {"confidence": c}}`` or
            ``{"expected_shortfall": {"confidence": c}}`` — the same shape as
            ``RiskDecomposition.measure``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def generic(self) -> float:
        """
        Generic factor contribution.

        Returns
        -------
        float
            Generic factor contribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def idiosyncratic_total(self) -> float:
        """
        Aggregate idiosyncratic contribution.

        Returns
        -------
        float
            Aggregate idiosyncratic contribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_level(self) -> list[LevelVolContribution]:
        """
        Volatility contribution by hierarchy level.
        Returns
        -------
        list[LevelVolContribution]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_position(self) -> list[PositionVolContribution] | None:
        """
        Optional per-position volatility contributions.
        Returns
        -------
        list[PositionVolContribution] or None

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Primary table: the per-hierarchy-level rollup.

        Alias of :meth:`to_level_dataframe`. Every tabular result type in the library
        answers ``to_dataframe()``; the optional per-position breakdown stays on
        :meth:`to_position_dataframe`.

        Returns
        -------
        pd.DataFrame
            The same frame :meth:`to_level_dataframe` returns.

        Examples
        --------
        >>> frame = result.to_dataframe()  # doctest: +SKIP

        Notes
        -----
        This alias does not raise; it delegates to the method named above.
        """
        ...

    def to_level_dataframe(self) -> pd.DataFrame:
        """
        Export the per-hierarchy-level rollup as a pandas DataFrame.

        One row per entry of :attr:`by_level`. The per-bucket drill-down stays
        on ``LevelVolContribution.by_bucket``, which is already a plain dict.

        Columns: ``level_name``, ``total`` (level contribution in the units of
        :attr:`measure`).

        Returns
        -------
        pd.DataFrame
            One row per hierarchy level.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_position_dataframe(self) -> pd.DataFrame:
        """
        Export the optional per-position breakdown as a pandas DataFrame.

        One row per entry of :attr:`by_position`. When the report was built
        without ``by_position=True`` the result is a zero-row frame that still
        carries the full column schema, so downstream ``df[...]`` selections
        keep working instead of raising ``KeyError``.

        Columns: ``position_id``, ``factor_total`` (systematic),
        ``idiosyncratic`` (issuer-specific), ``total`` (their sum) — all in
        the units of :attr:`measure`.

        Returns
        -------
        pd.DataFrame
            One row per position, or zero rows when :attr:`by_position` is
            ``None``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

def build_credit_vol_report(
    decomposition: RiskDecomposition,
    model: CreditFactorModel,
    by_position: bool = False,
) -> CreditVolReport:
    """
    Build a credit volatility report from decomposition outputs.

    Aggregates a Rust ``RiskDecomposition`` against the supplied credit factor
    model hierarchy into generic, level, idiosyncratic, and optional
    per-position volatility contributions.

    Parameters
    ----------
    decomposition : RiskDecomposition
        Factor risk decomposition to summarize.
    model : CreditFactorModel
        Credit factor model whose hierarchy and factor taxonomy label
        the report levels and buckets.
    by_position : bool, default False
        Include per-position contribution rows when ``True``.

    Returns
    -------
    CreditVolReport
        CreditVolReport with total, generic, level, idiosyncratic, and optional
        position-level volatility contribution fields.

    Raises
    ------
    TypeError
        If ``decomposition`` or ``model`` is not the expected wrapper type.
    PortfolioError
        A ``ValueError`` subclass raised when a factor contribution is neither
        the generic credit factor nor a bucket of one of ``model``'s hierarchy
        levels, so its risk could not be reported.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import RiskDecomposition
    >>> from finstack_quant.portfolio import build_credit_vol_report
    >>> risk = RiskDecomposition.from_json(
    ...     '{"total_risk":1.0,"measure":"variance","factor_contributions":[],"residual_risk":1.0,"position_factor_contributions":[],"position_residual_contributions":[]}'
    ... )
    >>> try:
    ...     build_credit_vol_report(risk, object())
    ... except TypeError as exc:
    ...     print("CreditFactorModel" in str(exc))
    True
    """
    ...
