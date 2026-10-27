"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/factor_model/model.rs``."""

from __future__ import annotations

import datetime
from typing import Any
from finstack_quant.core.market_data import MarketContext
from finstack_quant.models.factor import FactorModelConfig
from finstack_quant.models.factor.risk import RiskDecomposition
from finstack_quant.portfolio._container import Portfolio
from finstack_quant.portfolio._factor_model.assignment import FactorAssignmentReport
from finstack_quant.portfolio._factor_model.whatif import StressPnl, StressResult, WhatIfResult
from finstack_quant.portfolio._sensitivity import SensitivityMatrix

class FactorModel:
    """
    Portfolio factor-risk model built once from a ``FactorModelConfig``.

    Holds the factor definitions, covariance matrix, dependency matcher and
    sensitivity engine, so repeated assignment, sensitivity, decomposition,
    what-if and stress calls reuse one validated model. Portfolio arguments
    accept a built :class:`Portfolio` or ``PortfolioSpec`` JSON; market
    arguments accept a ``MarketContext`` or its JSON; ``as_of`` accepts a
    date-like object or an ISO 8601 string.

    Examples
    --------
    >>> import json, math
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.portfolio import FactorModel, Portfolio
    >>> config = json.dumps({
    ...     "factors": [
    ...         {
    ...             "id": "usd_rates",
    ...             "factor_type": "rates",
    ...             "market_mapping": {"curve_parallel": {"curve_ids": ["USD-OIS"], "units": "rate_bp"}},
    ...         }
    ...     ],
    ...     "covariance": {"factor_ids": ["usd_rates"], "n": 1, "data": [0.0001]},
    ...     "matching": {
    ...         "mapping_table": [{"dependency_filter": {}, "attribute_filter": {}, "factor_id": "usd_rates"}]
    ...     },
    ...     "risk_measure": "variance",
    ... })
    >>> deposit = {
    ...     "type": "deposit",
    ...     "spec": {
    ...         "id": "DEP",
    ...         "notional": {"amount": "1000000", "currency": "USD"},
    ...         "start_date": "2025-01-15",
    ...         "maturity": "2025-07-15",
    ...         "day_count": "act_360",
    ...         "fixed_rate": "0.04",
    ...         "discount_curve_id": "USD-OIS",
    ...         "attributes": {},
    ...     },
    ... }
    >>> book = Portfolio.from_spec(
    ...     json.dumps({
    ...         "id": "BOOK",
    ...         "as_of": "2025-01-15",
    ...         "base_currency": "USD",
    ...         "entities": {"FUND": {"id": "FUND"}},
    ...         "positions": [
    ...             {
    ...                 "position_id": "POS",
    ...                 "entity_id": "FUND",
    ...                 "instrument_id": "DEP",
    ...                 "instrument_spec": deposit,
    ...                 "quantity": 1.0,
    ...                 "unit": "units",
    ...             }
    ...         ],
    ...     })
    ... )
    >>> curve = DiscountCurve("USD-OIS", "2025-01-15", [(t, math.exp(-0.04 * t)) for t in (0.0, 0.5, 1.0, 2.0)])
    >>> market = MarketContext().insert(curve)
    >>> model = FactorModel.from_config(config)
    >>> model.assign_factors(book, market).assignments[0].factor_ids
    ['usd_rates']
    >>> model.analyze(book, market, "2025-01-15").total_risk > 0.0
    True
    >>> pnl = model.factor_stress_pnl(book, market, "2025-01-15", [("usd_rates", 1.0)])
    >>> pnl.total_pnl < 0.0
    True
    """

    @staticmethod
    def from_config(config: FactorModelConfig | str) -> FactorModel:
        """
        Build a factor model from a declarative configuration.

        Parameters
        ----------
        config : FactorModelConfig | str
            Typed ``FactorModelConfig`` or its canonical JSON: factor
            definitions, covariance matrix (axes in factor order), matching
            rules, pricing mode, bump sizes and risk measure.

        Returns
        -------
        FactorModel
            Validated model handle.

        Raises
        ------
        ValueError
            If the JSON is malformed or does not match the ``FactorModelConfig``
            schema, a matching rule names an undeclared factor, the covariance
            axes do not align with the factors, or the risk measure is invalid.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.portfolio import FactorModel
        >>> config = {
        ...     "factors": [
        ...         {
        ...             "id": "usd_rates",
        ...             "factor_type": "rates",
        ...             "market_mapping": {"curve_parallel": {"curve_ids": ["USD-OIS"], "units": "rate_bp"}},
        ...         }
        ...     ],
        ...     "covariance": {"factor_ids": ["usd_rates"], "n": 1, "data": [0.0001]},
        ...     "matching": {
        ...         "mapping_table": [{"dependency_filter": {}, "attribute_filter": {}, "factor_id": "missing"}]
        ...     },
        ...     "risk_measure": "variance",
        ... }
        >>> try:
        ...     FactorModel.from_config(json.dumps(config))
        ... except ValueError as exc:
        ...     print("missing" in str(exc))
        True
        """
        ...

    def assign_factors(self, portfolio: Portfolio | str, market: MarketContext | str) -> FactorAssignmentReport:
        """
        Match every position's market dependencies to the configured factors.

        Parameters
        ----------
        portfolio : Portfolio | str
            Built portfolio or ``PortfolioSpec`` JSON.
        market : MarketContext | str
            Market used to resolve credit-index aggregates into their curve
            dependencies.

        Returns
        -------
        FactorAssignmentReport
            Per-position factor mappings plus unmatched dependencies.

        Raises
        ------
        KeyError
            If a referenced credit index is absent from ``market``.
        PortfolioError
            If the unmatched policy is strict and a dependency cannot be mapped.
        """
        ...

    def compute_sensitivities(
        self,
        portfolio: Portfolio | str,
        market: MarketContext | str,
        as_of: datetime.date | str,
    ) -> SensitivityMatrix:
        """
        Compute the weighted position-by-factor sensitivity matrix.

        Parameters
        ----------
        portfolio : Portfolio | str
            Portfolio whose ``base_currency`` is the reporting currency of
            every sensitivity.
        market : MarketContext | str
            Market bumped by the sensitivity engine; must hold the FX needed
            for cross-currency positions.
        as_of : datetime.date | str
            Valuation date for sensitivities and spot FX.

        Returns
        -------
        SensitivityMatrix
            One row per position and one column per factor, in factor bump units.

        Raises
        ------
        KeyError
            If market data or FX required by a position is missing.
        PortfolioError
            If factor assignment or the sensitivity engine rejects the inputs.
        """
        ...

    def analyze(
        self,
        portfolio: Portfolio | str,
        market: MarketContext | str,
        as_of: datetime.date | str,
    ) -> RiskDecomposition:
        """
        Decompose portfolio risk into factor and residual contributions.

        Parameters
        ----------
        portfolio : Portfolio | str
            Portfolio to analyze.
        market : MarketContext | str
            Market used for sensitivity generation.
        as_of : datetime.date | str
            Valuation date of the analysis.

        Returns
        -------
        RiskDecomposition
            Total, factor and position contributions in the configured risk
            measure's units.

        Raises
        ------
        KeyError
            If market data required by a position is missing.
        PortfolioError
            If assignment, sensitivity or decomposition inputs are invalid.
        """
        ...

    def position_what_if(
        self,
        portfolio: Portfolio | str,
        market: MarketContext | str,
        as_of: datetime.date | str,
        changes: list[dict[str, Any]] | str,
    ) -> WhatIfResult:
        """
        Run a position remove/resize what-if against the model's baseline.

        Parameters
        ----------
        portfolio : Portfolio | str
            Portfolio the baseline is computed for and the changes apply to.
        market : MarketContext | str
            Market used to value positions and compute sensitivities.
        as_of : datetime.date | str
            Valuation date of the analysis.
        changes : list[dict[str, Any]] | str
            ``PositionChange`` objects applied in order:
            ``{"kind": "remove", "position_id": ...}`` or
            ``{"kind": "resize", "position_id": ..., "new_quantity": ...}``.

        Returns
        -------
        WhatIfResult
            Baseline and post-change decompositions with per-factor deltas.

        Raises
        ------
        ValueError
            If ``changes`` does not match the ``PositionChange`` schema.
        PortfolioError
            If a change names an unknown position or the analysis fails.
        """
        ...

    def factor_stress(
        self,
        portfolio: Portfolio | str,
        market: MarketContext | str,
        as_of: datetime.date | str,
        stresses: list[tuple[str, float]],
    ) -> StressResult:
        """
        Shock factors, reprice, and decompose risk under the stressed market.

        Parameters
        ----------
        portfolio : Portfolio | str
            Portfolio whose P&L and stressed risk are evaluated.
        market : MarketContext | str
            Baseline market to shock.
        as_of : datetime.date | str
            Valuation date for both endpoints.
        stresses : list[tuple[str, float]]
            ``(factor_id, shift)`` pairs; each shift is in the factor's
            configured market-mapping units (e.g. bp for ``rate_bp``).

        Returns
        -------
        StressResult
            Total and per-position P&L (base currency, loss negative) plus the
            stressed risk decomposition.

        Raises
        ------
        PortfolioError
            If a stress names an unknown factor or the bump is invalid.
        KeyError
            If market data required for repricing is missing.
        """
        ...

    def factor_stress_pnl(
        self,
        portfolio: Portfolio | str,
        market: MarketContext | str,
        as_of: datetime.date | str,
        stresses: list[tuple[str, float]],
    ) -> StressPnl:
        """
        Shock factors and reprice without decomposing stressed risk.

        Parameters
        ----------
        portfolio : Portfolio | str
            Portfolio whose position P&L is evaluated.
        market : MarketContext | str
            Baseline market to shock.
        as_of : datetime.date | str
            Valuation date for both endpoints.
        stresses : list[tuple[str, float]]
            ``(factor_id, shift)`` pairs in each factor's configured
            market-mapping units.

        Returns
        -------
        StressPnl
            Total and per-position stressed-minus-base P&L in the portfolio
            base currency (loss negative).

        Raises
        ------
        PortfolioError
            If a stress names an unknown factor or the bump is invalid.
        KeyError
            If market data required for repricing is missing.
        """
        ...
