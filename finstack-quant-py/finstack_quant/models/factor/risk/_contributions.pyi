"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/factor/risk/contributions.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd

class FactorContribution:
    """
    Aggregate contribution of a single factor to portfolio risk.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import FactorContribution
    >>> item = FactorContribution.from_json(
    ...     '{"factor_id":"Rates","absolute_risk":1.0,"relative_risk":0.5,"marginal_risk":0.2}'
    ... )
    >>> (item.factor_id, item.relative_risk)
    ('Rates', 0.5)
    """

    @staticmethod
    def from_json(json: str) -> FactorContribution:
        """
        Deserialize a factor contribution from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical serialized factor contribution, normally produced by
            ``FactorContribution.to_json``.

        Returns
        -------
        FactorContribution
            Validated `FactorContribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import FactorContribution
        >>> item = FactorContribution.from_json(
        ...     '{"factor_id":"Rates","absolute_risk":1.0,"relative_risk":0.5,"marginal_risk":0.2}'
        ... )
        >>> item.absolute_risk
        1.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this factor contribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `FactorContribution`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def factor_id(self) -> str:
        """
        Identifier of the contributing risk factor in the model.

        Returns
        -------
        str
            Identifier of the contributing risk factor in the model.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def absolute_risk(self) -> float:
        """
        Absolute risk contribution.

        Returns
        -------
        float
            Absolute risk contribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def relative_risk(self) -> float:
        """
        Share of total portfolio risk.

        Returns
        -------
        float
            Share of total portfolio risk.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def marginal_risk(self) -> float:
        """
        Marginal risk contribution.

        Returns
        -------
        float
            Marginal risk contribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export this entry as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            One row with columns ``factor_id``, ``absolute_risk``, ``relative_risk``, ``marginal_risk``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionFactorContribution:
    """
    Per-position contribution to a specific factor bucket.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import PositionFactorContribution
    >>> item = PositionFactorContribution.from_json('{"position_id":"P1","factor_id":"Rates","risk_contribution":1.0}')
    >>> (item.position_id, item.factor_id)
    ('P1', 'Rates')
    """

    @staticmethod
    def from_json(json: str) -> PositionFactorContribution:
        """
        Deserialize a position-factor contribution from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical serialized position-factor contribution, normally
            produced by ``PositionFactorContribution.to_json``.

        Returns
        -------
        PositionFactorContribution
            Validated `PositionFactorContribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import PositionFactorContribution
        >>> item = PositionFactorContribution.from_json(
        ...     '{"position_id":"P1","factor_id":"Rates","risk_contribution":1.0}'
        ... )
        >>> item.risk_contribution
        1.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this position-factor contribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionFactorContribution`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

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
    def factor_id(self) -> str:
        """
        Identifier of the contributing risk factor in the model.

        Returns
        -------
        str
            Identifier of the contributing risk factor in the model.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def risk_contribution(self) -> float:
        """
        Risk contribution for this position-factor pair.

        Returns
        -------
        float
            Risk contribution for this position-factor pair.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export this entry as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            One row with columns ``position_id``, ``factor_id``, ``risk_contribution``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionResidualContribution:
    """
    Annualized residual variance contributed by a single position.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import PositionResidualContribution
    >>> item = PositionResidualContribution.from_json(
    ...     '{"position_id":"P1","residual_variance":0.1,"source":{"kind":"other"}}'
    ... )
    >>> (item.position_id, item.source_kind)
    ('P1', 'other')
    """

    @staticmethod
    def from_json(json: str) -> PositionResidualContribution:
        """
        Deserialize a residual contribution from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical serialized residual contribution, normally produced by
            ``PositionResidualContribution.to_json``.

        Returns
        -------
        PositionResidualContribution
            Validated `PositionResidualContribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import PositionResidualContribution
        >>> item = PositionResidualContribution.from_json(
        ...     '{"position_id":"P1","residual_variance":0.1,"source":{"kind":"other"}}'
        ... )
        >>> item.residual_variance
        0.1
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this residual contribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionResidualContribution`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

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
    def residual_variance(self) -> float:
        """
        Residual variance assigned to this position.

        Returns
        -------
        float
            Residual variance assigned to this position.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def source_kind(self) -> str:
        """
        Source category used to derive residual risk.

        Returns
        -------
        str
            Source category used to derive residual risk.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def source_issuer_id(self) -> str | None:
        """
        Issuer identifier for issuer-sourced residual risk, if present.

        Returns
        -------
            Issuer identifier for issuer-sourced residual risk, if present.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export this entry as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            One row with columns ``position_id``, ``residual_variance``, ``source_kind``, ``source_issuer_id``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class RiskDecomposition:
    """
    Portfolio-level risk decomposition across factors and residuals.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import RiskDecomposition
    >>> doc = '{"total_risk":1.0,"measure":"variance","factor_contributions":[],"residual_risk":1.0,"position_factor_contributions":[],"position_residual_contributions":[]}'
    >>> decomposition = RiskDecomposition.from_json(doc)
    >>> (decomposition.total_risk, decomposition.residual_risk)
    (1.0, 1.0)
    """

    @staticmethod
    def from_json(json: str) -> RiskDecomposition:
        """
        Deserialize a risk decomposition from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical serialized factor-and-residual decomposition, normally
            produced by ``RiskDecomposition.to_json``.

        Returns
        -------
        RiskDecomposition
            Validated `RiskDecomposition` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import RiskDecomposition
        >>> doc = '{"total_risk":1.0,"measure":"variance","factor_contributions":[],"residual_risk":1.0,"position_factor_contributions":[],"position_residual_contributions":[]}'
        >>> RiskDecomposition.from_json(doc).measure
        'variance'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this risk decomposition to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `RiskDecomposition`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def total_risk(self) -> float:
        """
        Total portfolio risk under the decomposition measure.

        Returns
        -------
        float
            Total portfolio risk under the decomposition measure.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def measure(self) -> str | dict[str, Any]:
        """
        Risk measure used for aggregation, as its canonical Python value.

        Returns
        -------
        str | dict[str, Any]
            ``"variance"``, ``"volatility"``, ``{"var": {"confidence": c}}`` or
            ``{"expected_shortfall": {"confidence": c}}`` — the same shape as
            ``FactorModelConfig.risk_measure``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def residual_risk(self) -> float:
        """
        Residual risk not explained by factor contributions.

        Returns
        -------
        float
            Residual risk not explained by factor contributions.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def factor_contributions(self) -> list[FactorContribution]:
        """
        Factor-level risk contributions.
        Returns
        -------
        list[FactorContribution]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_factor_contributions(self) -> list[PositionFactorContribution]:
        """
        Position-by-factor risk contributions.
        Returns
        -------
        list[PositionFactorContribution]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_residual_contributions(self) -> list[PositionResidualContribution]:
        """
        Per-position residual risk contributions.
        Returns
        -------
        list[PositionResidualContribution]
            Empty unless the decomposer had per-issuer idiosyncratic vol
            estimates (credit-aware position decomposers).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Primary table: the factor-level risk decomposition.

        Alias of :meth:`to_factor_dataframe`. Every tabular result type in the library
        answers ``to_dataframe()``; the position × factor view stays on
        :meth:`to_position_factor_dataframe`.

        Returns
        -------
        pd.DataFrame
            The same frame :meth:`to_factor_dataframe` returns.

        Examples
        --------
        >>> frame = result.to_dataframe()  # doctest: +SKIP

        Notes
        -----
        This alias does not raise; it delegates to the method named above.
        """
        ...

    def to_factor_dataframe(self) -> pd.DataFrame:
        """
        Export factor contributions as a pandas DataFrame.

        Columns: ``factor_id``, ``absolute_risk``, ``relative_risk``,
        ``marginal_risk``.

        Returns
        -------
        pd.DataFrame
            One row per entry of :attr:`factor_contributions`.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_position_factor_dataframe(self) -> pd.DataFrame:
        """
        Export position x factor contributions as a pandas DataFrame.

        Columns: ``position_id``, ``factor_id``, ``risk_contribution``.

        Returns
        -------
        pd.DataFrame
            One row per entry of :attr:`position_factor_contributions`.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_position_residual_dataframe(self) -> pd.DataFrame:
        """
        Export per-position residual variance contributions as a DataFrame.

        Columns: ``position_id``, ``residual_variance`` (annualized variance,
        non-negative), ``source_kind`` (``"from_credit_model"`` or
        ``"other"``), ``source_issuer_id`` (``None`` unless ``source_kind`` is
        ``"from_credit_model"``).

        Returns
        -------
        pd.DataFrame
            One row per entry of :attr:`position_residual_contributions`; zero
            rows — with the columns still present — when the decomposer
            produced no position-level residual allocation.

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

class PositionVarContribution:
    """
    Per-position component / marginal VaR.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import PositionVarContribution
    >>> item = PositionVarContribution.from_json(
    ...     '{"position_id":"P1","component_var":-1.0,"relative_var":1.0,"marginal_var":-1.0,"incremental_var":null}'
    ... )
    >>> (item.position_id, item.component_var)
    ('P1', -1.0)
    """

    @staticmethod
    def from_json(json: str) -> PositionVarContribution:
        """
        Deserialize a position VaR contribution from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical serialized component and marginal VaR contribution,
            normally produced by ``PositionVarContribution.to_json``.

        Returns
        -------
        PositionVarContribution
            Validated `PositionVarContribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import PositionVarContribution
        >>> item = PositionVarContribution.from_json(
        ...     '{"position_id":"P1","component_var":-1.0,"relative_var":1.0,"marginal_var":-1.0,"incremental_var":null}'
        ... )
        >>> item.relative_var
        1.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this position VaR contribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionVarContribution`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

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
    def component_var(self) -> float:
        """
        Component VaR assigned to this position.

        Returns
        -------
        float
            Component VaR assigned to this position.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def relative_var(self) -> float:
        """
        Share of total portfolio VaR.

        Returns
        -------
        float
            Share of total portfolio VaR.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def marginal_var(self) -> float | None:
        """
        Marginal VaR, if computed.

        Returns
        -------
            Marginal VaR, if computed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def incremental_var(self) -> float | None:
        """
        Incremental VaR, if requested in the decomposition config.

        Returns
        -------
            Incremental VaR, if requested in the decomposition config.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export this contribution as a single-row pandas DataFrame.

        Columns: ``position_id``, ``component_var``, ``relative_var``,
        ``marginal_var``, ``incremental_var``.

        Returns
        -------
        pd.DataFrame
            Exactly one row, for symmetry with
            :meth:`PositionRiskDecomposition.to_dataframe`.

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

class PositionEsContribution:
    """
    Per-position component / marginal ES.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import PositionEsContribution
    >>> item = PositionEsContribution.from_json(
    ...     '{"position_id":"P1","component_es":-1.2,"relative_es":1.0,"marginal_es":-1.2}'
    ... )
    >>> (item.position_id, item.component_es)
    ('P1', -1.2)
    """

    @staticmethod
    def from_json(json: str) -> PositionEsContribution:
        """
        Deserialize a position ES contribution from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical serialized component and marginal expected-shortfall
            contribution, normally produced by ``PositionEsContribution.to_json``.

        Returns
        -------
        PositionEsContribution
            Validated `PositionEsContribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import PositionEsContribution
        >>> item = PositionEsContribution.from_json(
        ...     '{"position_id":"P1","component_es":-1.2,"relative_es":1.0,"marginal_es":-1.2}'
        ... )
        >>> item.relative_es
        1.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this position ES contribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionEsContribution`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

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
    def component_es(self) -> float:
        """
        Component expected shortfall assigned to this position.

        Returns
        -------
        float
            Component expected shortfall assigned to this position.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def relative_es(self) -> float:
        """
        Share of total portfolio expected shortfall.

        Returns
        -------
        float
            Share of total portfolio expected shortfall.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def marginal_es(self) -> float | None:
        """
        Marginal expected shortfall, if computed.

        Returns
        -------
            Marginal expected shortfall, if computed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export this entry as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            One row with columns ``position_id``, ``component_es``, ``relative_es``, ``marginal_es``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionRiskDecomposition:
    """
    Complete position-level risk decomposition.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import PositionRiskDecomposition
    >>> doc = '{"portfolio_var":-1.0,"portfolio_es":-1.2,"confidence":0.95,"method":"parametric","var_contributions":[],"es_contributions":[],"n_positions":0,"euler_residual":0.0}'
    >>> result = PositionRiskDecomposition.from_json(doc)
    >>> (result.portfolio_var, result.method)
    (-1.0, 'parametric')
    """

    @staticmethod
    def from_json(json: str) -> PositionRiskDecomposition:
        """
        Deserialize a position risk decomposition from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical serialized VaR/ES decomposition, normally produced by
            ``PositionRiskDecomposition.to_json``.

        Returns
        -------
        PositionRiskDecomposition
            Validated `PositionRiskDecomposition` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import PositionRiskDecomposition
        >>> doc = '{"portfolio_var":-1.0,"portfolio_es":-1.2,"confidence":0.95,"method":"parametric","var_contributions":[],"es_contributions":[],"n_positions":0,"euler_residual":0.0}'
        >>> PositionRiskDecomposition.from_json(doc).confidence
        0.95
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this position risk decomposition to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionRiskDecomposition`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def portfolio_var(self) -> float:
        """
        Total portfolio Value-at-Risk.
        Returns
        -------
        float
            Portfolio-currency amount under the workspace loss convention, so
            losses are reported as **negative** numbers.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def portfolio_es(self) -> float:
        """
        Portfolio expected shortfall.

        Returns
        -------
        float
            Portfolio expected shortfall.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def confidence(self) -> float:
        """
        Confidence level used for VaR/ES.

        Returns
        -------
        float
            Confidence level used for VaR/ES.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def n_positions(self) -> int:
        """
        Number of positions included in the decomposition.

        Returns
        -------
        int
            Number of positions included in the decomposition.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def method(self) -> str:
        """
        Decomposition method label.

        Returns
        -------
        str
            Decomposition method label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def euler_residual(self) -> float | None:
        """
        Euler allocation residual, if reported.

        Returns
        -------
            Euler allocation residual, if reported.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def var_contributions(self) -> list[PositionVarContribution]:
        """
        Per-position VaR contributions.
        Returns
        -------
        list[PositionVarContribution]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def es_contributions(self) -> list[PositionEsContribution]:
        """
        Per-position expected shortfall contributions.
        Returns
        -------
        list[PositionEsContribution]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the joined per-position VaR and ES decomposition.

        ``var_contributions`` and ``es_contributions`` are both keyed by
        position, so they are joined on ``position_id`` into one frame. Rows
        follow ``var_contributions`` order; an ES column is ``None`` for a
        position that has no matching ES entry.

        The portfolio-level scalars (:attr:`portfolio_var`,
        :attr:`portfolio_es`, :attr:`confidence`, :attr:`method`) are header
        metadata and are deliberately **not** repeated on every row.

        Columns: ``position_id``, ``component_var``, ``relative_var``,
        ``marginal_var``, ``incremental_var``, ``component_es``,
        ``relative_es``, ``marginal_es``.

        Returns
        -------
        pd.DataFrame
            One row per entry of :attr:`var_contributions`.

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
