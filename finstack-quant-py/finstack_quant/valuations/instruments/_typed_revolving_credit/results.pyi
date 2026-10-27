"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_revolving_credit/results.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
from finstack_quant.core.money import Money

class EnhancedMonteCarloResult:
    """
    Monte Carlo result of a stochastic revolving credit facility with every
    simulated path retained (:meth:`RevolvingCredit.price_with_paths`'s return
    value).

    The present value and the draw option cost are antithetic-aware estimates
    (mean, standard error, 95% interval); :attr:`path_pvs` and
    :attr:`path_draw_option_costs` carry the per-path values in path order and
    :meth:`to_dataframe` tabulates them.

    Examples
    --------
    >>> import datetime
    >>> import json
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.valuations.instruments import RevolvingCredit
    >>> envelope = json.loads(RevolvingCredit.example().to_json())
    >>> spec = envelope["instrument"]["spec"]
    >>> spec["rate"] = {"fixed": {"rate": 0.06}}
    >>> spec["draw_repay_spec"] = {
    ...     "stochastic": {
    ...         "utilization_process": {"mean_reverting": {"theta": 0.6, "kappa": 1.0, "sigma": 0.25}},
    ...         "mc_config": {
    ...             "credit_spread_process": {"constant": 0.025},
    ...         },
    ...     }
    ... }
    >>> spec["instrument_pricing_overrides"] = {"model_config": {"mc_paths": 16}}
    >>> facility = RevolvingCredit.from_json(json.dumps(envelope))
    >>> as_of = datetime.date(2024, 1, 15)
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.03))
    >>> result = facility.price_with_paths(market, as_of)
    >>> (result.num_simulated_paths, len(result.path_pvs), str(result.pv.currency))
    (16, 16, 'USD')
    >>> list(result.to_dataframe().columns)
    ['path', 'pv', 'draw_option_cost']
    """

    @staticmethod
    def from_json(json: str) -> EnhancedMonteCarloResult:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``EnhancedMonteCarloResult`` (the exact shape
            ``to_json`` writes, including every path's cashflow schedule).

        Returns
        -------
        EnhancedMonteCarloResult
            The decoded result.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for the result shape.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EnhancedMonteCarloResult
        >>> try:
        ...     EnhancedMonteCarloResult.from_json("{}")
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
            JSON-encoded result, including every path's cashflow schedule and
            factor paths.

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
            ``(EnhancedMonteCarloResult.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per simulated path as a pandas ``DataFrame``.

        Columns: ``path`` (index in path order), ``pv`` and
        ``draw_option_cost`` (facility currency units).

        Returns
        -------
        pd.DataFrame
            The per-path distribution of present value and draw option cost.

        Raises
        ------
        ValueError
            If component amounts have different currencies. Empty results
            preserve column order and numeric dtypes.
        """
        ...
    @property
    def pv(self) -> Money:
        """
        Mean present value across paths (facility currency).

        Returns
        -------
        Money
            Currency-tagged mean present value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def pv_std_error(self) -> float:
        """
        Standard error of the mean present value, in currency units.

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
    def pv_ci_95(self) -> tuple[Money, Money]:
        """
        95% confidence interval of the mean present value.

        Returns
        -------
        tuple[Money, Money]
            Lower and upper bounds.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def num_paths(self) -> int:
        """
        Number of independent path estimators (antithetic pairs count once).

        Returns
        -------
        int
            The estimator count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def num_simulated_paths(self) -> int:
        """
        Number of simulated paths, including both members of antithetic pairs.

        Returns
        -------
        int
            The simulated path count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def draw_option_cost(self) -> Money:
        """
        Mean draw option cost across paths (facility currency): the value to the lender of the simulated draws having been made at the contractual margin instead of each path's fair spread; negative when spreads widen after draws.

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
    def draw_option_cost_std_error(self) -> float:
        """
        Standard error of the mean draw option cost, in currency units.

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
    def draw_option_cost_ci_95(self) -> tuple[Money, Money]:
        """
        95% confidence interval of the mean draw option cost.

        Returns
        -------
        tuple[Money, Money]
            Lower and upper bounds.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def path_pvs(self) -> list[float]:
        """
        Present value of every simulated path, in path order.

        Returns
        -------
        list[float]
            Per-path present values in currency units.

        Notes
        -----
        This accessor does not raise; it returns the stored values.
        """
        ...
    @property
    def path_draw_option_costs(self) -> list[float]:
        """
        Draw option cost of every simulated path, in path order.

        Returns
        -------
        list[float]
            Per-path draw option costs in currency units.

        Notes
        -----
        This accessor does not raise; it returns the stored values.
        """
        ...
    @property
    def utilization_paths(self) -> list[list[float]]:
        """
        Simulated utilization trajectories, one list per path, aligned with :attr:`observation_dates`.

        Returns
        -------
        list[list[float]]
            Utilization fractions in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored values.
        """
        ...
    @property
    def credit_spread_paths(self) -> list[list[float]]:
        """
        Simulated credit-spread trajectories (decimal), one list per path, aligned with :attr:`observation_dates`.

        Returns
        -------
        list[list[float]]
            Spread levels as decimals.

        Notes
        -----
        This accessor does not raise; it returns the stored values.
        """
        ...
    @property
    def observation_dates(self) -> list[str]:
        """
        Observation dates of the factor trajectories (ISO 8601 strings).

        Returns
        -------
        list[str]
            The observation grid.

        Notes
        -----
        This accessor does not raise; it returns the stored values.
        """
        ...
