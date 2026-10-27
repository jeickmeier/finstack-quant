"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/stochastic_result.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
from finstack_quant.core.money import Money

class StochasticPricingResult:
    """
    Stochastic (scenario-waterfall) pricing result of a structured-credit deal
    (:meth:`StructuredCredit.price_stochastic`'s return value).

    Deal-level present value, loss statistics and Monte Carlo error sit next
    to one ``TranchePricingResult`` per tranche (as dicts). Pools of real
    instruments add the reserve-funding diagnostics
    (:attr:`unfunded_draw_path_fraction`, :attr:`expected_collateral_draws`)
    and the draw option cost with its per-path distribution.

    Examples
    --------
    >>> import datetime
    >>> import json
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import (
    ...     AssetPool,
    ...     RepLine,
    ...     StandardDealParams,
    ...     StructuredCredit,
    ...     Tranche,
    ...     TrancheStructure,
    ... )
    >>> as_of, maturity = datetime.date(2024, 1, 15), datetime.date(2031, 1, 15)
    >>> pool = AssetPool("POOL-1", "abs", Currency("USD")).with_rep_lines([
    ...     RepLine(
    ...         "LINE-1",
    ...         Money(80_000_000.0, Currency("USD")),
    ...         0.07,
    ...         maturity,
    ...         12,
    ...         DayCount.ACT_360,
    ...         asset_type={"type": "first_lien_loan"},
    ...     )
    ... ])
    >>> note = (
    ...     Tranche
    ...     .builder()
    ...     .id("A")
    ...     .attach_pct(0.0)
    ...     .detach_pct(100.0)
    ...     .seniority("senior")
    ...     .original_balance(Money(80_000_000.0, Currency("USD")))
    ...     .coupon_fixed(0.05)
    ...     .maturity(maturity)
    ...     .build()
    ... )
    >>> deal = StructuredCredit.new_standard(
    ...     "abs", StandardDealParams("ABS-1", pool, TrancheStructure([note]), as_of, maturity, "USD-SOFR-DISC")
    ... )
    >>> envelope = json.loads(deal.to_json())
    >>> envelope["instrument"]["spec"]["calendar_id"] = "nyse"
    >>> deal = StructuredCredit.from_json(json.dumps(envelope))
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-SOFR-DISC", as_of, 0.03))
    >>> result = deal.price_stochastic(market, as_of, num_paths=4)
    >>> (result.num_paths, [t["tranche_id"] for t in result.tranche_results])
    (8, ['A'])
    >>> result.unfunded_draw_path_fraction
    0.0
    """

    @staticmethod
    def from_json(json: str) -> StochasticPricingResult:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``StochasticPricingResult`` (the shape ``to_json`` writes).

        Returns
        -------
        StochasticPricingResult
            The decoded result.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for the result shape.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import StochasticPricingResult
        >>> try:
        ...     StochasticPricingResult.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded result.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust result.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(StochasticPricingResult.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per tranche as a pandas ``DataFrame``.

        Columns: ``tranche_id``, ``seniority``, ``npv`` (currency units),
        ``price_pct`` (percent of the tranche's current balance),
        ``expected_loss``, ``unexpected_loss``, ``expected_shortfall``
        (currency units),
        ``attach_pct``, ``detach_pct`` (percent of the capital structure), ``wal`` (years, over
        the paths that returned principal), ``paths_with_principal``,
        ``credit_duration`` and ``draw_option_cost`` (currency units).

        Returns
        -------
        pd.DataFrame
            The tranche summary.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...
    def draw_option_cost_dataframe(self) -> pd.DataFrame:
        """
        The per-path draw option cost distribution as a pandas ``DataFrame``.

        Columns: ``path`` (index in path order) and ``draw_option_cost``
        (currency units). Empty for pools without stochastic revolvers.

        Returns
        -------
        pd.DataFrame
            One row per path.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...
    @property
    def npv(self) -> Money:
        """
        Mean deal present value across paths (pool currency).

        Returns
        -------
        Money
            Currency-tagged present value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def dirty_price(self) -> float:
        """
        Dirty price as a percent of the sum of current tranche balances
        (the same face as the deterministic ``dirty_price`` metric).

        Returns
        -------
        float
            The dirty price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expected_loss(self) -> Money:
        """
        Expected (mean) loss across paths.

        Returns
        -------
        Money
            Currency-tagged expected loss.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def unexpected_loss(self) -> Money:
        """
        Unexpected loss (standard deviation of the path losses).

        Returns
        -------
        Money
            Currency-tagged unexpected loss.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expected_shortfall(self) -> Money:
        """
        Expected shortfall of the path losses at :attr:`es_confidence`.

        Returns
        -------
        Money
            Currency-tagged expected shortfall.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def es_confidence(self) -> float:
        """
        Confidence level of :attr:`expected_shortfall` (decimal).

        Returns
        -------
        float
            The confidence level.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def pv_std_error(self) -> float:
        """
        Sample standard error of the mean present value, in currency units.

        Monte Carlo uses the sample standard deviation divided by the square
        root of the independent-estimator count; each antithetic pair
        contributes its average as one observation.

        Returns
        -------
        float
            The standard error.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def pv_confidence_interval(self) -> tuple[float, float]:
        """
        Two-sided 95% Student-t confidence interval of mean present value, in
        currency units. Monte Carlo uses one fewer degree of freedom than the
        independent-estimator count; each antithetic pair counts once.

        Returns
        -------
        tuple[float, float]
            Lower and upper bounds.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def num_paths(self) -> int:
        """
        Number of simulated scenario paths (two per antithetic estimator).

        Monte Carlo sampling uncertainty uses independent estimator counts,
        which equal half this value when antithetic pairing is enabled.

        Returns
        -------
        int
            The path count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def pricing_mode(self) -> dict[str, Any]:
        """
        Pricing mode used, as its serde ``dict``.

        Returns
        -------
        dict[str, Any]
            The ``StructuredCreditPricingMode`` serde shape.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def unfunded_draw_path_fraction(self) -> float:
        """
        Fraction of paths on which a collateral draw could not be funded from the reserve account and that period's principal collections.

        Returns
        -------
        float
            A fraction in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expected_collateral_draws(self) -> Money:
        """
        Mean over paths of the collateral draws funded through the reserve account and principal collections.

        Returns
        -------
        Money
            Currency-tagged mean draws.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def draw_option_cost(self) -> Money:
        """
        Mean draw option cost across paths: the value to the deal of its revolvers' draws having been made at the contractual margin instead of each path's fair spread (negative when spreads widen).

        Returns
        -------
        Money
            Currency-tagged mean draw option cost.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def draw_option_cost_paths(self) -> list[float]:
        """
        Per-path draw option cost in path order (empty for pools without stochastic revolvers).

        Returns
        -------
        list[float]
            Per-path costs in currency units.

        Notes
        -----
        This accessor does not raise; it returns the stored values.
        """
        ...
    @property
    def tranche_results(self) -> list[dict[str, Any]]:
        """
        Tranche-level results as a list of dicts (``TranchePricingResult`` serde shape, in capital-structure order).

        Returns
        -------
        list[dict[str, Any]]
            One dict per tranche.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
