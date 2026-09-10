"""
P&L attribution: decompose portfolio P&L into risk-factor contributions.

Bindings for ``finstack_quant_attribution``. Provides the
:class:`PnlAttribution` result type and the :func:`attribute_pnl` /
:func:`attribute_return_contribution` entry points, along with validation
helpers and default metric / waterfall ordering utilities.

Examples
--------
>>> from finstack_quant.attribution import default_waterfall_order
>>> default_waterfall_order()[:2]
['carry', 'rates_curves']
"""

from __future__ import annotations

import datetime

from typing import Any

import pandas as pd

from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money

from finstack_quant.attribution import schema as schema

__all__ = [
    "PnlAttribution",
    "ReturnContributionResult",
    "attribute_pnl",
    "attribute_pnl_envelope_json",
    "attribute_pnl_many",
    "attribute_return_contribution",
    "default_attribution_metrics",
    "default_waterfall_order",
    "pnl_bridge",
    "schema",
    "validate_attribution_json",
    "validate_return_contribution_json",
]

# P&L Attribution

class PnlAttribution:
    """
    P&L attribution result decomposing total P&L into risk factor contributions.

    Factors include carry, rates curves, credit curves, inflation, correlations,
    FX, volatility, cross-factor interactions, model parameters, market scalars,
    and residual.

    Construct via :meth:`from_json` or the :func:`attribute_pnl` helper.

    Examples
    --------
    >>> from finstack_quant.attribution import PnlAttribution
    >>> try:
    ...     PnlAttribution.from_json("{}")
    ... except ValueError as exc:
    ...     "total_pnl" in str(exc)
    True
    """

    @staticmethod
    def from_json(json: str) -> PnlAttribution:
        """
        Deserialize a ``PnlAttribution`` from JSON.

        Parameters
        ----------
        json : str
            JSON string (the ``attribution`` field from an
            ``AttributionResultEnvelope``).

        Returns
        -------
        PnlAttribution
            Parsed ``PnlAttribution`` instance.

        Examples
        --------
        >>> from finstack_quant.attribution import PnlAttribution
        >>> try:
        ...     PnlAttribution.from_json("{}")
        ... except ValueError as exc:
        ...     "total_pnl" in str(exc)
        True

        Raises
        ------
        ValueError
            If ``json`` is malformed or omits fields required by the P&L
            attribution schema.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Export the canonical serde-shaped attribution payload as a dict.

        Returns
        -------
        dict[str, object]
            Attribution payload as a Python dict.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_pnl(self) -> float:
        """
        Total P&L amount (val_t1 − val_t0 + intra-period coupon income).

        For methods that follow the total-return convention (parallel,
        waterfall, Taylor), ``total_pnl`` includes coupon income received in
        the period. Use :attr:`mark_to_market_pnl` for the raw price change.

        Returns
        -------
        float
            Total P&L in :attr:`currency`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mark_to_market_pnl(self) -> float | None:
        """
        Raw mark-to-market P&L: ``val_t1 − val_t0`` with no cashflow adjustment.

        When the attribution method added coupon income to ``total_pnl`` (the
        standard total-return convention), this field still reports the raw
        mark-to-market change so a downstream consumer can reconcile against
        their own computation. ``None`` for attributions deserialized from a
        pre-audit JSON payload that did not carry the field.

        Returns
        -------
        float or None
            Raw MTM P&L in :attr:`currency`, or ``None`` when not stored.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def carry(self) -> float:
        """
        Carry (theta + accruals) P&L amount.

        Returns
        -------
        float
            Carry bucket P&L in :attr:`currency`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rates_curves_pnl(self) -> float:
        """
        Interest rate curves P&L amount.

        Returns
        -------
        float
            Rates-curve bucket P&L in :attr:`currency`. Use
            :meth:`to_long_dataframe` for per-curve and per-tenor detail.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def credit_curves_pnl(self) -> float:
        """
        Credit hazard curves P&L amount.

        Returns
        -------
        float
            Credit-curve bucket P&L in :attr:`currency`. Use
            :meth:`to_credit_factor_dataframe` when a credit factor model was
            supplied.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def inflation_curves_pnl(self) -> float:
        """
        Inflation curves P&L amount.

        Returns
        -------
        float
            Inflation-curve bucket P&L in :attr:`currency`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def correlations_pnl(self) -> float:
        """
        Base correlation curves P&L amount.

        Returns
        -------
        float
            Correlation-curve bucket P&L in :attr:`currency`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fx_pnl(self) -> float:
        """
        FX rate changes P&L amount.

        Pricing-impact FX P&L for cross-currency instruments. For pure
        single-currency instruments this is zero.

        Returns
        -------
        float
            FX pricing bucket P&L in :attr:`currency`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fx_translation_pnl(self) -> float:
        """
        FX translation P&L amount.

        Reporting-currency FX P&L when ``AttributionConfig.target_currency`` was
        supplied and differs from native. Equals
        ``val_t0_native × (T1_fx − T0_fx)``. Zero by default.

        Returns
        -------
        float
            Translation bucket P&L in the reporting currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def vol_pnl(self) -> float:
        """
        Implied volatility changes P&L amount.

        Returns
        -------
        float
            Volatility bucket P&L in :attr:`currency`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cross_factor_pnl(self) -> float:
        """
        Cross-factor interaction P&L amount.

        Returns
        -------
        float
            Cross-factor bucket P&L in :attr:`currency`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def model_params_pnl(self) -> float:
        """
        Model parameters P&L amount.

        Returns
        -------
        float
            Model-parameter bucket P&L in :attr:`currency`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def market_scalars_pnl(self) -> float:
        """
        Market scalars P&L amount.

        Returns
        -------
        float
            Market-scalar bucket P&L in :attr:`currency` (dividends, repo, etc.).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def residual(self) -> float:
        """
        Residual (unexplained) P&L amount.

        Returns
        -------
        float
            ``total_pnl`` minus the sum of explained factor buckets, in
            :attr:`currency`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> str:
        """
        Currency code for all P&L amounts.

        Returns
        -------
        str
            Currency code for all P&L amounts.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def instrument_id(self) -> str:
        """
        Identifier of the instrument whose P&L was attributed.

        Returns
        -------
        str
            Identifier of the instrument whose P&L was attributed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def method(self) -> str:
        """
        Canonical attribution method name (``parallel``, ``waterfall``,
        ``metrics_based``, or ``taylor``).

        Returns
        -------
        str
            Canonical attribution method name (``parallel``, ``waterfall``,

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def t0(self) -> datetime.date:
        """
        Start date (T₀).

        Returns
        -------
        datetime.date
            Opening valuation date of the attribution window.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def t1(self) -> datetime.date:
        """
        Closing date of the attribution window, at which the ending P&L is
        struck (T₁).

        Returns
        -------
        datetime.date
            Calendar date, strictly after :attr:`t0`. All explained and
            residual P&L components describe the move from ``t0`` to this
            date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def required_metrics(self) -> list[str]:
        """
        Risk metric ids the attribution method consumes.

        Returns
        -------
        list[str]
            Canonical snake-case metric ids (``theta``, ``dv01``, ``cs01``,
            ``bucketed_cs01``, ``vega``, ... plus second-order terms) for
            ``metrics_based``; an empty list for the repricing methods, which
            use no pre-computed metrics.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def num_repricings(self) -> int:
        """
        Number of repricings performed.

        Returns
        -------
        int
            Number of repricings performed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def residual_pct(self) -> float:
        """
        Residual as percentage of total P&L.

        Returns
        -------
        float
            Residual as percentage of total P&L.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def notes(self) -> list[str]:
        """
        Diagnostic notes and warnings.

        Returns
        -------
        list[str]
            Diagnostic notes and warnings.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def result_invalid(self) -> bool:
        """
        True when attribution was flagged invalid and residual checks should fail.

        Returns
        -------
        bool
            True when attribution was flagged invalid and residual checks should fail.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tolerance_abs(self) -> float:
        """
        Absolute tolerance used for residual validation.

        Returns
        -------
        float
            The stored ``meta.tolerance_abs`` threshold for this attribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tolerance_pct(self) -> float:
        """
        Percentage tolerance used for residual validation.

        Returns
        -------
        float
            The stored ``meta.tolerance_pct`` threshold for this attribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rounding(self) -> dict[str, Any]:
        """
        Rounding context in force for the attribution run.

        Returns
        -------
        dict[str, Any]
            Serde-shaped rounding-context payload (policy stamp).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def fx_policy(self) -> dict[str, Any] | None:
        """
        FX policy metadata stamped on the attribution.

        Returns
        -------
        dict[str, Any] or None
            Serde-shaped FX policy payload, or ``None`` when no FX
            conversions were applied.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def execution_policy(self) -> str | None:
        """
        Execution policy the attribution ran under.

        Returns
        -------
        str or None
            ``"serial"`` or ``"parallel"``, or ``None`` for methods without a
            policy knob (metrics-based).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def carry_detail(self) -> dict[str, Any] | None:
        """
        Gross carry decomposition detail payload; funding is a separate financing overlay.

        Returns
        -------
        dict[str, Any] or None
            Serde-shaped ``CarryDetail`` (``total``, ``coupon_income``,
            ``pull_to_par``, ``roll_down``, ``funding_cost``), or ``None``
            when not populated.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def rates_detail(self) -> dict[str, Any] | None:
        """
        Rates-curves detail payload.

        Returns
        -------
        dict[str, Any] or None
            Serde-shaped ``RatesCurvesAttribution`` (``by_curve``,
            ``by_tenor``, ``discount_total``, ``forward_total``), or ``None``
            when not populated.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def credit_detail(self) -> dict[str, Any] | None:
        """
        Credit-curves detail payload.

        Returns
        -------
        dict[str, Any] or None
            Serde-shaped ``CreditCurvesAttribution`` (``by_curve``,
            ``by_tenor``), or ``None`` when not populated.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def inflation_detail(self) -> dict[str, Any] | None:
        """
        Inflation-curves detail payload.

        Returns
        -------
        dict[str, Any] or None
            Serde-shaped ``InflationCurvesAttribution`` (``by_curve``,
            optional ``by_tenor``), or ``None`` when not populated.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def correlations_detail(self) -> dict[str, Any] | None:
        """
        Base-correlation detail payload.

        Returns
        -------
        dict[str, Any] or None
            Serde-shaped ``CorrelationsAttribution`` (``by_curve``), or
            ``None`` when not populated.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def fx_detail(self) -> dict[str, Any] | None:
        """
        Per-pair FX contribution detail for this attribution result.

        Returns
        -------
        dict[str, Any] or None
            Serde-shaped ``FxAttribution`` (``by_pair`` keyed ``"FROM/TO"``),
            or ``None`` when not populated.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def vol_detail(self) -> dict[str, Any] | None:
        """
        Volatility surface detail payload.

        Returns
        -------
        dict[str, Any] or None
            Serde-shaped ``VolAttribution`` (``by_surface``), or ``None``
            when not populated.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def cross_factor_detail(self) -> dict[str, Any] | None:
        """
        Cross-factor interaction detail payload.

        Returns
        -------
        dict[str, Any] or None
            Serde-shaped ``CrossFactorDetail`` (``total``, ``by_pair``), or
            ``None`` when not populated.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def model_params_detail(self) -> dict[str, Any] | None:
        """
        Model-parameter detail payload.

        Returns
        -------
        dict[str, Any] or None
            Serde-shaped ``ModelParamsAttribution`` (``prepayment``,
            ``default_rate``, ``recovery_rate``, ``conversion_ratio``,
            ``other``), or ``None`` when not populated.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def scalars_detail(self) -> dict[str, Any] | None:
        """
        Market-scalars detail payload.

        Returns
        -------
        dict[str, Any] or None
            Serde-shaped ``ScalarsAttribution`` (``dividends``,
            ``inflation``, ``equity_prices``, ``commodity_prices``), or
            ``None`` when not populated.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def credit_factor_detail(self) -> dict[str, Any] | None:
        """
        Credit-factor hierarchy decomposition payload.

        Returns
        -------
        dict[str, Any] or None
            Serde-shaped ``CreditFactorAttribution`` (``model_id``,
            ``generic_pnl``, ``levels``, ``adder_pnl_total``,
            ``curve_shape_pnl``, ...), or ``None`` when no
            ``credit_factor_model`` was supplied.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def credit_carry_decomposition(self) -> dict[str, Any] | None:
        """
        Factor-cut decomposition of carry under a credit factor model.

        Returns
        -------
        dict[str, Any] or None
            Serde-shaped ``CreditCarryDecomposition`` (``rates_carry_total``,
            ``credit_carry_total``, ``credit_by_level``), or ``None`` when
            not populated.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def residual_within_tolerance(
        self,
        pct_tolerance: float | None = None,
        abs_tolerance: float | None = None,
    ) -> bool:
        """
        Check if residual is within tolerance.

        Parameters
        ----------
        pct_tolerance : float or None
            Percentage tolerance (e.g. 0.1 for 0.1%).
            Defaults to the attribution's stored ``meta.tolerance_pct``.
        abs_tolerance : float or None
            Absolute tolerance (e.g. 100.0 for $100).
            Defaults to the attribution's stored ``meta.tolerance_abs``.

        Returns
        -------
        bool
            ``True`` if residual is within tolerance.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.
        """
        ...

    def validate_currencies(self) -> None:
        """
        Validate that every factor's currency matches ``total_pnl.currency``.

        Useful before building a DataFrame or summing across instruments.

        Raises
        ------
        ValueError
            When any factor's currency differs from ``total_pnl.currency``.

        """
        ...

    def explain(self) -> str:
        """
        Human-readable tree explanation (non-zero factors only).

        Returns
        -------
        str
            Multi-line string with tree structure showing P&L breakdown.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def explain_verbose(self) -> str:
        """
        Verbose tree explanation including zero-valued factors.

        Returns
        -------
        str
            Multi-line string with tree structure showing all factors.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export attribution as a single-row pandas DataFrame.

        Columns include ``instrument_id``, ``method``, ``t0``, ``t1``,
        ``currency``, ``total_pnl``, ``mark_to_market_pnl`` (nullable), all
        factor P&L amounts, ``residual``, ``residual_pct``,
        ``num_repricings``, and ``result_invalid``.

        Returns
        -------
        pd.DataFrame
            Single-row DataFrame.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_long_dataframe(self) -> pd.DataFrame:
        """
        Export every populated detail breakdown as one long-format DataFrame.

        Columns:
            kind: dotted-path identifier (e.g. ``"rates.by_curve"``,
                ``"rates.by_tenor"``, ``"credit.by_curve"``, ``"fx.by_pair"``,
                ``"vol.by_surface"``, ``"cross_factor.by_pair"``,
                ``"scalars.dividends"``, ``"credit_factor.generic"``,
                ``"credit_factor.level"``, ``"credit_factor.adder"``,
                ``"credit_factor.curve_shape"``,
                ``"carry.coupon_income"``, ...).
            factor: parent factor family (``"rates"``, ``"credit"``, ``"fx"``,
                ``"vol"``, ``"cross_factor"``, ``"scalars"``,
                ``"credit_factor"``, ``"carry"``, ``"inflation"``,
                ``"correlations"``, ``"model_params"``).
            sub: ``kind`` with the ``factor.`` prefix removed
                (``"by_curve"``, ``"coupon_income.rates"``), so
                ``df.pivot_table(index="factor", columns="sub", values="amount")``
                works without string surgery.
            key_a: primary identifier (curve_id, pair label, vol_surface_id,
                equity_id, level_name, sub-component name).
            key_b: secondary key when present (tenor, ``to``-currency, bucket
                path); ``None`` otherwise.
            amount: float P&L amount.
            currency: 3-letter currency code.

        The DataFrame is empty (zero rows, schema columns present) when no
        detail breakdown was populated. Use
        ``df.query("kind.str.startswith('rates')")`` or
        ``df.pivot_table(index="key_a", columns="key_b", values="amount")``
        to slice the desired view.

        Returns
        -------
        pd.DataFrame
            Long-format DataFrame.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_carry_detail_dataframe(self) -> pd.DataFrame:
        """
        Export the carry decomposition as a typed long DataFrame.

        Columns (``kind``, ``factor``, ``sub``, ``key_a``, ``key_b``,
        ``amount``, ``currency``) are the same as :meth:`to_long_dataframe`
        but the kind values
        are limited to the ``"carry.*"`` family — useful when you only want the
        carry split (coupon income, pull-to-par, roll-down, funding cost),
        including the optional rates/credit splits when a credit
        factor model was supplied.

        Returns an empty DataFrame when ``carry_detail`` is not populated.

        Returns
        -------
        pd.DataFrame
            Carry-decomposition DataFrame.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_credit_factor_dataframe(self) -> pd.DataFrame:
        """
        Export the credit-factor hierarchy decomposition as a typed long DataFrame.

        Columns are the same as :meth:`to_long_dataframe`; rows are limited to
        the ``"credit_factor.*"`` family. Includes generic, per-level, adder,
        curve_shape, plus per-bucket and per-issuer rows when present.

        Returns an empty DataFrame when ``credit_factor_detail`` is not
        populated (no ``credit_factor_model`` was supplied, or the instrument
        has no resolvable issuer).

        Returns
        -------
        pd.DataFrame
            Credit-factor DataFrame.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str:
        """Return a debug representation of this attribution.

        Returns
        -------
        str
        """
        ...

    def _repr_html_(self) -> str | None:
        """
        HTML table for Jupyter, rendered from :meth:`to_dataframe`.

        Returns
        -------
        str or None
            pandas HTML markup, or ``None`` when the frame cannot be built
            (IPython then falls back to ``__repr__``).

        Notes
        -----
        This method does not raise; failures degrade to ``None``.
        """
        ...

# Return Contribution

class ReturnContributionResult:
    """
    Return-contribution attribution result.

    Returned by :func:`attribute_return_contribution`. Decomposes a portfolio
    return into per-instrument, per-group, and per-factor contributions, with
    an optional Brinson-Fachler benchmark-relative block.

    Examples
    --------
    >>> from finstack_quant.attribution import ReturnContributionResult
    >>> try:
    ...     ReturnContributionResult.from_json("{}")
    ... except ValueError as exc:
    ...     "missing field" in str(exc)
    True
    """

    @property
    def portfolio_return(self) -> float:
        """
        Total portfolio return, equal to the summed instrument contributions.

        Returns
        -------
        float
            Period return as a decimal fraction (``0.01`` is 1%). Equal to the
            sum of the ``contribution`` field across
            :attr:`instrument_contribution`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def instrument_contribution(self) -> list[dict[str, Any]]:
        """
        Per-instrument contribution rows.

        Returns
        -------
        list[dict[str, Any]]
            One record per instrument with ``id``, ``weight``, ``return``,
            ``contribution``, and ``active_contribution`` keys.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def group_contribution(self) -> dict[str, Any]:
        """
        Contributions keyed by group dimension.

        Returns
        -------
        dict[str, Any]
            Maps each group dimension name (for example ``"sector"``) to its
            list of bucket records, each with a ``key`` bucket label and the
            summed ``contribution`` of the instruments in that bucket.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def factor_contribution(self) -> list[dict[str, Any]]:
        """
        Factor contribution rows.

        Returns
        -------
        list[dict[str, Any]]

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def specific_return(self) -> float | None:
        """
        Idiosyncratic residual when factor rows were supplied.

        Returns
        -------
        float or None
            ``portfolio_return - sum(factor contributions)``; ``None`` when
            the spec carried no factors.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def benchmark_relative(self) -> dict[str, Any] | None:
        """
        Brinson-Fachler benchmark-relative block.

        Returns
        -------
        dict[str, Any] or None
            ``None`` unless benchmark inputs were supplied on the spec.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def warnings(self) -> list[str]:
        """
        Non-fatal diagnostic warnings from the contribution run.

        Returns
        -------
        list[str]
            For example leveraged weights from a near-flat net-market-value
            book.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-instrument contributions as a pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            One row per instrument with ``id``, ``weight``, ``return``,
            ``contribution``, and ``active_contribution`` (``NaN`` when no
            benchmark was supplied) columns; the schema columns are present
            even for an empty result.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_group_dataframe(self) -> pd.DataFrame:
        """
        Group-bucket contributions as a long pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            Columns ``dimension`` (the ``group:<dimension>`` label name),
            ``key`` (bucket) and ``contribution``; empty with schema columns
            when the spec carried no group labels.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_factor_dataframe(self) -> pd.DataFrame:
        """
        Factor contributions as a pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            Columns ``factor``, ``exposure``, ``factor_return``,
            ``contribution``; empty with schema columns when no factor rows
            were supplied.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def _repr_html_(self) -> str | None:
        """
        HTML table for Jupyter, rendered from :meth:`to_dataframe`.

        Returns
        -------
        str or None
            pandas HTML markup, or ``None`` when the frame cannot be built
            (IPython then falls back to ``__repr__``).

        Notes
        -----
        This method does not raise; failures degrade to ``None``.
        """
        ...

    def to_series(self) -> pd.Series:
        """
        Per-instrument contributions as a pandas Series indexed by instrument id.

        Returns
        -------
        pd.Series
            Named ``contribution``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to a compact JSON string.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ReturnContributionResult:
        """
        Deserialize from a JSON string.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        ReturnContributionResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the result schema.

        Examples
        --------
        >>> from finstack_quant.attribution import ReturnContributionResult
        >>> try:
        ...     ReturnContributionResult.from_json("{}")
        ... except ValueError as exc:
        ...     "missing field" in str(exc)
        True
        """
        ...

# Entry Points

def attribute_pnl(
    instrument: object,
    market_t0: MarketContext | str,
    market_t1: MarketContext | str,
    as_of_t0: datetime.date | str,
    as_of_t1: datetime.date | str,
    method: str | dict[str, Any],
    config: dict[str, Any] | str | None = None,
    full_cross_attribution: bool = False,
    model_params_t0_json: str | None = None,
    credit_factor_model_json: str | None = None,
) -> PnlAttribution:
    """
    Run P&L attribution for a single instrument.

    This is the main entry point. Accepts the instrument, two market
    snapshots, valuation dates, and a method descriptor — typed objects or
    their canonical JSON — and returns the typed attribution result. Use
    :func:`attribute_pnl_envelope_json` when you want the raw JSON envelope
    round-trip instead, :func:`attribute_pnl_many` for a book.

    Parameters
    ----------
    instrument : Bond | TermLoan | InterestRateSwap | Swaption | CapFloor | CreditDefaultSwap | CDSIndex | CDSTranche | FxForward | FxOption | ConvertibleBond | EquityOption | StructuredCredit | CompositeInstrument | str
        Typed instrument wrapper from ``finstack_quant.valuations`` or a
        canonical v1 instrument envelope JSON string
        (``{"schema": "finstack_quant.instrument/1", "instrument": {...}}``).
    market_t0 : MarketContext | str
        Market snapshot at T₀ (typed ``MarketContext`` or its JSON).
    market_t1 : MarketContext | str
        Market snapshot at T₁ (typed ``MarketContext`` or its JSON).
    as_of_t0 : datetime.date | datetime.datetime | pandas.Timestamp | str
        Valuation date T₀; strings are ISO 8601 (``YYYY-MM-DD``).
    as_of_t1 : datetime.date | datetime.datetime | pandas.Timestamp | str
        Valuation date T₁ in the same forms; must not precede ``as_of_t0``.
    method : str or dict[str, Any]
        Attribution method — one of ``"parallel"``, ``"metrics_based"``,
        ``{"taylor": {"include_gamma": True, ...}}``, or
        ``{"waterfall": [...]}`` with factor tokens in application order
        drawn from ``carry``, ``rates_curves``, ``credit_curves``,
        ``inflation_curves``, ``correlations``, ``fx``, ``volatility``,
        ``model_parameters``, ``market_scalars`` (the order must start with
        ``carry``; :func:`default_waterfall_order` is the canonical full list).
    config : dict[str, Any] or str or None
        Optional config overrides (``tolerance_abs``, ``tolerance_pct``,
        ``metrics``, ``strict_validation``, ``rounding_scale``,
        ``rate_bump_bp``, ``target_currency``, or
        ``{"execution_policy": "parallel"}`` to opt into inner Rayon when the
        caller is not already parallelizing at the portfolio/batch level).
        Serial is the default.
    full_cross_attribution : bool, default False
        Compute every pairwise cross-factor term (parallel method only)
        instead of the default seven economic pairs.
    model_params_t0_json : str or None
        Serialized opening ``ModelParamsSnapshot``. When omitted, model-
        parameter P&L is isolated from the instrument's current snapshot.
    credit_factor_model_json : str or None
        Serialized ``CreditFactorModel``. When supplied, credit-factor
        hierarchy detail is populated on the result.

    Returns
    -------
    PnlAttribution
        Typed attribution result. Use ``.to_json()`` for the wire form and
        ``.to_dataframe()`` for a pandas view.

    Examples
    --------
    >>> from finstack_quant.attribution import attribute_pnl
    >>> try:
    ...     attribute_pnl("{}", "{}", "{}", "2025-01-15", "2025-01-16", "parallel")
    ... except ValueError as exc:
    ...     "instrument envelope" in str(exc)
    True

    Raises
    ------
    ValueError
        If ``method`` or ``config`` cannot be serialized, an input JSON or ISO
        date is malformed, or attribution validation, pricing, or result
        serialization fails.
    KeyError
        If a required curve, market item, calendar, or FX triangulation leg is
        unavailable.
    RuntimeError
        If calibration or solver convergence fails, or attribution encounters
        an internal operational failure.
    """
    ...

def attribute_pnl_many(
    instruments: list[object],
    market_t0: MarketContext | str,
    market_t1: MarketContext | str,
    as_of_t0: datetime.date | str,
    as_of_t1: datetime.date | str,
    method: str | dict[str, Any],
    config: dict[str, Any] | str | None = None,
    full_cross_attribution: bool = False,
    model_params_t0_json: str | None = None,
    credit_factor_model_json: str | None = None,
) -> pd.DataFrame:
    """
    Run one attribution set-up against many instruments and tabulate.

    Every instrument shares the markets, dates, method and config; the batch
    runs in Rust (``attribute_pnl_many``) in input order and stops at the
    first failing instrument.

    Parameters
    ----------
    instruments : list[object]
        Typed instrument wrappers or canonical instrument envelope JSON
        strings, in the row order wanted.
    market_t0 : MarketContext | str
        Market snapshot at T₀.
    market_t1 : MarketContext | str
        Market snapshot at T₁.
    as_of_t0 : datetime.date | datetime.datetime | pandas.Timestamp | str
        Valuation date T₀.
    as_of_t1 : datetime.date | datetime.datetime | pandas.Timestamp | str
        Valuation date T₁.
    method : str or dict[str, Any]
        Attribution method, as for :func:`attribute_pnl`.
    config : dict[str, Any] or str or None
        Config overrides, as for :func:`attribute_pnl`.
    full_cross_attribution : bool, default False
        Evaluate every pairwise cross-factor term (parallel method only).
    model_params_t0_json : str or None
        Serialized opening ``ModelParamsSnapshot`` applied to every instrument.
    credit_factor_model_json : str or None
        Serialized ``CreditFactorModel`` applied to every instrument.

    Returns
    -------
    pd.DataFrame
        One row per instrument with the columns of
        :meth:`PnlAttribution.to_dataframe` (``instrument_id``, ``method``,
        ``t0``, ``t1``, ``currency``, ``total_pnl``, every factor P&L,
        ``residual``, ``residual_pct``, ``num_repricings``,
        ``result_invalid``); empty with schema columns for an empty list.

    Raises
    ------
    ValueError
        If any input cannot be parsed, an instrument's attribution fails
        validation / pricing, or a result mixes currencies across factors.
    KeyError
        If a required curve, market item, calendar, or FX leg is missing.
    RuntimeError
        If the engine reports an internal failure for any instrument.

    Examples
    --------
    >>> from finstack_quant.attribution import attribute_pnl_many
    >>> try:
    ...     attribute_pnl_many(["{}"], "{}", "{}", "2025-01-15", "2025-01-16", "parallel")
    ... except ValueError as exc:
    ...     "instrument envelope" in str(exc)
    True
    """
    ...

def pnl_bridge(
    instrument: object,
    market_t0: MarketContext | str,
    market_t1: MarketContext | str,
    as_of_t0: datetime.date | str,
    as_of_t1: datetime.date | str,
    target_currency: Currency | str,
) -> Money:
    """
    Headline P&L bridge ``value(T₁) − value(T₀)`` in one currency.

    The cheapest attribution entry point — two repricings, no factor loop.
    FX conversion into ``target_currency`` uses ``market_t0`` for the T₀
    value and ``market_t1`` for the T₁ value.

    Parameters
    ----------
    instrument : object
        Typed instrument wrapper or canonical instrument envelope JSON.
    market_t0 : MarketContext | str
        Opening market state.
    market_t1 : MarketContext | str
        Closing market state.
    as_of_t0 : datetime.date | datetime.datetime | pandas.Timestamp | str
        Opening valuation date.
    as_of_t1 : datetime.date | datetime.datetime | pandas.Timestamp | str
        Closing valuation date.
    target_currency : Currency | str
        ISO-4217 currency the P&L is reported in.

    Returns
    -------
    Money
        ``value(T₁) − value(T₀)`` in ``target_currency``.

    Raises
    ------
    ValueError
        If the instrument JSON, a market, a date, or the currency is
        malformed, or pricing fails validation.
    KeyError
        If a curve or FX rate needed for pricing or conversion is missing.
    RuntimeError
        If a pricer's solver fails to converge.

    Examples
    --------
    >>> from finstack_quant.attribution import pnl_bridge
    >>> try:
    ...     pnl_bridge("{}", "{}", "{}", "2025-01-15", "2025-01-16", "USD")
    ... except ValueError as exc:
    ...     "instrument envelope" in str(exc)
    True
    """
    ...

def attribute_pnl_envelope_json(spec_json: str) -> str:
    """
    Run attribution from a full JSON ``AttributionEnvelope``.

    Power-user variant for full envelope round-trip workflows.
    Most users should prefer :func:`attribute_pnl`.

    Parameters
    ----------
    spec_json : str
        JSON-serialized ``AttributionEnvelope``.

    Returns
    -------
    str
        JSON-serialized ``AttributionResultEnvelope``.

    Examples
    --------
    >>> from finstack_quant.attribution import attribute_pnl_envelope_json
    >>> try:
    ...     attribute_pnl_envelope_json("{}")
    ... except ValueError as exc:
    ...     "missing field" in str(exc)
    True

    Raises
    ------
    ValueError
        If ``spec_json`` is malformed or violates the exact attribution
        envelope schema, attribution validation or pricing fails, or the result
        cannot be serialized.
    KeyError
        If execution cannot find a required curve, market item, calendar, or FX
        triangulation leg.
    RuntimeError
        If calibration or solver convergence fails, or attribution encounters
        an internal operational failure.
    """
    ...

def attribute_return_contribution(
    spec: dict[str, Any] | str | pd.DataFrame,
    as_of: datetime.date | str | None = None,
    weighting: str | None = None,
    factors: list[dict[str, Any]] | None = None,
) -> ReturnContributionResult:
    """
    Compute single-period return contribution attribution.

    Parameters
    ----------
    spec : dict[str, Any] or str or pandas.DataFrame
        A ``dict`` or JSON ``str`` carries ``as_of``, ``positions``, optional
        ``factors`` and ``weighting`` exactly as the wire schema (``as_of``
        may be a ``datetime.date`` in the dict form). A ``DataFrame`` is one
        position per row with columns ``id`` (or the index), exactly one of
        ``market_value`` / ``weight``, ``return``, optional
        ``benchmark_weight`` / ``benchmark_return``, and any number of
        ``group:<dimension>`` label columns; missing optional cells may be
        ``NaN``.
    as_of : datetime.date or str, optional
        Attribution date label. Required with a ``DataFrame`` spec; fills a
        missing ``as_of`` in the dict form.
    weighting : str, optional
        ``"gross"`` (default) or ``"net_market_value"`` for market-value
        positions; ``DataFrame`` form only.
    factors : list[dict[str, Any]], optional
        Factor rows ``{"factor", "exposure", "factor_return"}``;
        ``DataFrame`` form only.

    Returns
    -------
    ReturnContributionResult
        Typed result. Use ``.to_json()`` for the wire form,
        ``.to_dataframe()`` / ``.to_group_dataframe()`` /
        ``.to_factor_dataframe()`` for tabular views, and ``.to_series()``
        for contributions indexed by instrument id.

    Examples
    --------
    >>> import pandas as pd
    >>> from finstack_quant.attribution import attribute_return_contribution
    >>> spec = {
    ...     "as_of": "2026-01-02",
    ...     "positions": [{"id": "A", "market_value": 100.0, "return": 0.02}],
    ... }
    >>> attribute_return_contribution(spec).portfolio_return
    0.02
    >>> frame = pd.DataFrame({
    ...     "id": ["A", "B"],
    ...     "weight": [0.6, 0.4],
    ...     "return": [0.02, -0.01],
    ...     "group:sector": ["tech", "energy"],
    ... })
    >>> round(attribute_return_contribution(frame, as_of="2026-01-02").portfolio_return, 6)
    0.008

    Raises
    ------
    ValueError
        If the spec is malformed; ``as_of`` is missing for a DataFrame;
        required identifiers or positions are empty; numeric inputs are
        non-finite; position weighting modes are mixed or incomplete; factor
        or benchmark inputs are incomplete; or benchmark-relative weights do
        not sum to one; or a Brinson group has zero net weight but nonzero
        return contribution. Split offsetting long/short positions into distinct groups.
    TypeError
        If ``spec`` is none of ``dict``, ``str``, ``pandas.DataFrame``.
    RuntimeError
        If result serialization fails or an internal post-validation invariant
        is violated.
    """
    ...

def validate_attribution_json(json: str) -> str:
    """
    Validate an attribution specification JSON.

    Deserializes against the ``AttributionEnvelope`` schema and returns
    the canonical (re-serialized) JSON.

    Parameters
    ----------
    json : str
        JSON-serialized ``AttributionEnvelope``.

    Returns
    -------
    str
        Canonical compact JSON.

    Examples
    --------
    >>> from finstack_quant.attribution import validate_attribution_json
    >>> try:
    ...     validate_attribution_json("{}")
    ... except ValueError as exc:
    ...     "missing field" in str(exc)
    True

    Raises
    ------
    ValueError
        If ``json`` is malformed, violates the exact attribution envelope
        schema, or cannot be canonically reserialized.
    """
    ...

def validate_return_contribution_json(spec_json: str) -> str:
    """
    Validate a return contribution specification JSON.

    Parameters
    ----------
    spec_json : str
        JSON-serialized return contribution specification.

    Returns
    -------
    str
        Canonical compact JSON of the accepted specification.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.attribution import validate_return_contribution_json
    >>> spec = {
    ...     "as_of": "2026-01-02",
    ...     "weighting": "gross",
    ...     "factors": [],
    ...     "positions": [{"id": "A", "market_value": 100.0, "return": 0.02, "groups": {}}],
    ... }
    >>> json.loads(validate_return_contribution_json(json.dumps(spec)))["weighting"]
    'gross'

    Raises
    ------
    ValueError
        If ``spec_json`` is malformed; required identifiers or positions are
        empty; numeric inputs are non-finite; position weighting modes are
        mixed or incomplete; factor or benchmark inputs are incomplete; or
        benchmark-relative weights do not sum to one.
    RuntimeError
        If execution violates an internal invariant after validation.
    """
    ...

def default_waterfall_order() -> list[str]:
    """
    Return the default waterfall factor ordering.

    Returns
    -------
    list[str]
        Canonical snake-case factor tokens in the default waterfall order
        (``carry``, ``rates_curves``, ``credit_curves``, ``inflation_curves``,
        ``correlations``, ``fx``, ``volatility``, ``model_parameters``,
        ``market_scalars``); pass a prefix or reordering to
        ``attribute_pnl(method={"waterfall": [...]})``.

    Notes
    -----
    This method does not raise; it returns the stored or derived value.

    Examples
    --------
    >>> from finstack_quant.attribution import default_waterfall_order
    >>> default_waterfall_order()[:3]
    ['carry', 'rates_curves', 'credit_curves']
    """
    ...

def default_attribution_metrics() -> list[str]:
    """
    Return the default metric IDs used by metrics-based attribution.

    Returns
    -------
    list[str]
        Canonical snake-case metric ids (``theta``, ``dv01``, ``cs01``,
        ``bucketed_cs01``, ``vega``, ...) — the tokens accepted by
        ``config={"metrics": [...]}`` and returned by
        :meth:`PnlAttribution.required_metrics`.

    Notes
    -----
    This method does not raise; it returns the stored or derived value.

    Examples
    --------
    >>> from finstack_quant.attribution import default_attribution_metrics
    >>> default_attribution_metrics()[:3]
    ['theta', 'dv01', 'cs01']
    """
    ...
