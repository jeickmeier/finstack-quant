"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/factor_model/whatif.rs``."""

from __future__ import annotations

import pandas as pd
from finstack_quant.models.factor.risk import RiskDecomposition

# factor_model typed result classes (Slice 8)

class FactorContributionDelta:
    """
    Per-factor contribution change between a baseline and a scenario.

    Examples
    --------
    >>> from finstack_quant.portfolio import FactorContributionDelta
    >>> delta = FactorContributionDelta.from_json('{"factor_id":"Rates","absolute_change":0.1,"relative_change":0.2}')
    >>> (delta.factor_id, delta.absolute_change)
    ('Rates', 0.1)
    """

    @classmethod
    def from_json(cls, json: str) -> FactorContributionDelta:
        """
        Deserialize a factor contribution delta from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical serialized baseline-to-scenario factor delta, normally
            produced by ``FactorContributionDelta.to_json``.

        Returns
        -------
        FactorContributionDelta
            Validated `FactorContributionDelta` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import FactorContributionDelta
        >>> delta = FactorContributionDelta.from_json(
        ...     '{"factor_id":"Rates","absolute_change":0.1,"relative_change":0.2}'
        ... )
        >>> delta.relative_change
        0.2
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this factor contribution delta to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `FactorContributionDelta`, suitable for a matching `from_json` call.

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
    def absolute_change(self) -> float:
        """
        Absolute contribution change.

        Returns
        -------
        float
            Absolute contribution change.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def relative_change(self) -> float:
        """
        Relative contribution change.

        Returns
        -------
        float
            Relative contribution change.

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

class WhatIfResult:
    """
    Result of a position what-if scenario.

    Examples
    --------
    >>> from finstack_quant.portfolio import WhatIfResult
    >>> risk = '{"total_risk":1.0,"measure":"variance","factor_contributions":[],"residual_risk":1.0,"position_factor_contributions":[],"position_residual_contributions":[]}'
    >>> result = WhatIfResult.from_json('{"before":' + risk + ',"after":' + risk + ',"delta":[]}')
    >>> result.delta
    []
    """

    @classmethod
    def from_json(cls, json: str) -> WhatIfResult:
        """
        Deserialize a what-if result from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical serialized before-and-after risk result, normally
            produced by ``WhatIfResult.to_json``.

        Returns
        -------
        WhatIfResult
            Validated `WhatIfResult` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import WhatIfResult
        >>> risk = '{"total_risk":1.0,"measure":"variance","factor_contributions":[],"residual_risk":1.0,"position_factor_contributions":[],"position_residual_contributions":[]}'
        >>> result = WhatIfResult.from_json('{"before":' + risk + ',"after":' + risk + ',"delta":[]}')
        >>> result.before.total_risk
        1.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this what-if result to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `WhatIfResult`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def before(self) -> RiskDecomposition:
        """
        Baseline risk decomposition.
        Returns
        -------
        RiskDecomposition

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def after(self) -> RiskDecomposition:
        """
        Post-scenario risk decomposition.
        Returns
        -------
        RiskDecomposition

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def delta(self) -> list[FactorContributionDelta]:
        """
        Per-factor contribution changes.
        Returns
        -------
        list[FactorContributionDelta]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-factor contribution changes as a pandas DataFrame.

        One row per entry of :attr:`delta`. The baseline and scenario
        decompositions are not flattened here — reach them through
        :attr:`before` / :attr:`after` and their own frame accessors.

        Columns: ``factor_id``, ``absolute_change`` (risk-measure units),
        ``relative_change`` (dimensionless fraction, not a percentage).

        Returns
        -------
        pd.DataFrame
            One row per changed factor.

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

class StressResult:
    """
    Result of a factor-stress scenario.

    Examples
    --------
    >>> from finstack_quant.portfolio import StressResult
    >>> risk = '{"total_risk":1.0,"measure":"variance","factor_contributions":[],"residual_risk":1.0,"position_factor_contributions":[],"position_residual_contributions":[]}'
    >>> result = StressResult.from_json(
    ...     '{"total_pnl":-10.0,"position_pnl":[["P1",-10.0]],"stressed_decomposition":' + risk + "}"
    ... )
    >>> result.total_pnl
    -10.0
    """

    @classmethod
    def from_json(cls, json: str) -> StressResult:
        """
        Deserialize a stress result from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical serialized stressed P&L and decomposition result, normally
            produced by ``StressResult.to_json``.

        Returns
        -------
        StressResult
            Validated `StressResult` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import StressResult
        >>> risk = '{"total_risk":1.0,"measure":"variance","factor_contributions":[],"residual_risk":1.0,"position_factor_contributions":[],"position_residual_contributions":[]}'
        >>> result = StressResult.from_json(
        ...     '{"total_pnl":-10.0,"position_pnl":[["P1",-10.0]],"stressed_decomposition":' + risk + "}"
        ... )
        >>> result.position_pnl
        [('P1', -10.0)]
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this stress result to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `StressResult`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def total_pnl(self) -> float:
        """
        Total portfolio P&L under the stress scenario.

        Returns
        -------
        float
            Total portfolio P&L under the stress scenario.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_pnl(self) -> list[tuple[str, float]]:
        """
        Position identifier and scenario P&L pairs from the stress run.
        Returns
        -------
        list[tuple[str, float]]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def stressed_decomposition(self) -> RiskDecomposition:
        """
        Risk decomposition after applying the stress.
        Returns
        -------
        RiskDecomposition

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-position stressed P&L as a pandas DataFrame.

        One row per entry of :attr:`position_pnl`. The scenario totals stay on
        :attr:`total_pnl` and :attr:`stressed_decomposition`.

        Columns: ``position_id``, ``pnl`` (portfolio-currency amount; a loss
        is negative).

        Returns
        -------
        pd.DataFrame
            One row per position with a stressed P&L contribution.

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

class StressPnl:
    """
    P&L-only result of a factor-stress scenario.

    Stressed-minus-base present value without a risk decomposition on the
    shocked market, produced by :meth:`FactorModel.factor_stress_pnl`. Use
    :meth:`FactorModel.factor_stress` when the stressed decomposition is also
    needed.

    Examples
    --------
    >>> from finstack_quant.portfolio import StressPnl
    >>> pnl = StressPnl.from_json('{"total_pnl":-10.0,"position_pnl":[["P1",-10.0]]}')
    >>> (pnl.total_pnl, pnl.position_pnl)
    (-10.0, [('P1', -10.0)])
    """

    @classmethod
    def from_json(cls, json: str) -> StressPnl:
        """
        Deserialize a stress P&L result from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical ``{"total_pnl", "position_pnl"}`` JSON, normally produced
            by ``StressPnl.to_json``.

        Returns
        -------
        StressPnl
            Parsed result.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the ``StressPnl`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import StressPnl
        >>> StressPnl.from_json('{"total_pnl":5.0,"position_pnl":[]}').total_pnl
        5.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this stress P&L result to canonical JSON.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def total_pnl(self) -> float:
        """
        Total portfolio P&L under the stressed market.

        Returns
        -------
        float
            Portfolio base-currency amount; a loss is negative.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_pnl(self) -> list[tuple[str, float]]:
        """
        Per-position stressed-minus-base P&L in portfolio order.

        Returns
        -------
        list[tuple[str, float]]
            ``(position_id, pnl)`` pairs in the portfolio base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-position stressed P&L as a pandas DataFrame.

        Columns: ``position_id``, ``pnl`` (base-currency amount; loss negative).

        Returns
        -------
        pd.DataFrame
            One row per entry of :attr:`position_pnl`.

        Raises
        ------
        ImportError
            If pandas is not installed.
        """
        ...

    def _repr_html_(self) -> str | None: ...
