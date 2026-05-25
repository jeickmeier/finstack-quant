from __future__ import annotations

import pandas as pd

__all__: list[str] = [
    "PnlAttribution",
    "attribute_pnl",
    "attribute_pnl_from_spec",
    "validate_attribution_json",
    "default_waterfall_order",
    "default_attribution_metrics",
]

# ---------------------------------------------------------------------------
# P&L Attribution
# ---------------------------------------------------------------------------

class PnlAttribution:
    """P&L attribution result decomposing total P&L into risk factor contributions.

    Factors include carry, rates curves, credit curves, inflation, correlations,
    FX, volatility, cross-factor interactions, model parameters, market scalars,
    and residual.

    Construct via :meth:`from_json` or the :func:`attribute_pnl` helper.

    Example:
        >>> from finstack.attribution import PnlAttribution
        >>> attr = PnlAttribution.from_json(result_json)  # doctest: +SKIP
    """

    @staticmethod
    def from_json(json: str) -> PnlAttribution:
        """Deserialize a ``PnlAttribution`` from JSON.

        Args:
            json: JSON string (the ``attribution`` field from an
                ``AttributionResultEnvelope``).

        Returns:
            Parsed ``PnlAttribution`` instance.
        """
        ...

    def to_json(self) -> str:
        """Serialize to compact JSON.

        Returns:
            Compact JSON string.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """Export the canonical serde-shaped attribution payload as a dict."""
        ...

    @property
    def total_pnl(self) -> float:
        """Total P&L amount (val_t1 − val_t0 + intra-period coupon income).

        For methods that follow the total-return convention (parallel,
        waterfall, Taylor), ``total_pnl`` includes coupon income received in
        the period. Use :attr:`mark_to_market_pnl` for the raw price change.
        """
        ...

    @property
    def mark_to_market_pnl(self) -> float | None:
        """Raw mark-to-market P&L: ``val_t1 − val_t0`` with no cashflow adjustment.

        When the attribution method added coupon income to ``total_pnl`` (the
        standard total-return convention), this field still reports the raw
        mark-to-market change so a downstream consumer can reconcile against
        their own computation. ``None`` for attributions deserialized from a
        pre-audit JSON payload that did not carry the field.
        """
        ...

    @property
    def carry(self) -> float:
        """Carry (theta + accruals) P&L amount."""
        ...

    @property
    def rates_curves_pnl(self) -> float:
        """Interest rate curves P&L amount."""
        ...

    @property
    def credit_curves_pnl(self) -> float:
        """Credit hazard curves P&L amount."""
        ...

    @property
    def inflation_curves_pnl(self) -> float:
        """Inflation curves P&L amount."""
        ...

    @property
    def correlations_pnl(self) -> float:
        """Base correlation curves P&L amount."""
        ...

    @property
    def fx_pnl(self) -> float:
        """FX rate changes P&L amount.

        Pricing-impact FX P&L for cross-currency instruments. For pure
        single-currency instruments this is zero.
        """
        ...

    @property
    def fx_translation_pnl(self) -> float:
        """FX translation P&L amount.

        Reporting-currency FX P&L when ``AttributionConfig.target_ccy`` was
        supplied and differs from native. Equals
        ``val_t0_native × (T1_fx − T0_fx)``. Zero by default.
        """
        ...

    @property
    def vol_pnl(self) -> float:
        """Implied volatility changes P&L amount."""
        ...

    @property
    def cross_factor_pnl(self) -> float:
        """Cross-factor interaction P&L amount."""
        ...

    @property
    def model_params_pnl(self) -> float:
        """Model parameters P&L amount."""
        ...

    @property
    def market_scalars_pnl(self) -> float:
        """Market scalars P&L amount."""
        ...

    @property
    def residual(self) -> float:
        """Residual (unexplained) P&L amount."""
        ...

    @property
    def currency(self) -> str:
        """Currency code for all P&L amounts."""
        ...

    @property
    def instrument_id(self) -> str:
        """Instrument identifier."""
        ...

    @property
    def method(self) -> str:
        """Attribution method name (Parallel, Waterfall, MetricsBased, Taylor)."""
        ...

    @property
    def t0(self) -> str:
        """Start date (T₀) as ISO string."""
        ...

    @property
    def t1(self) -> str:
        """End date (T₁) as ISO string."""
        ...

    @property
    def num_repricings(self) -> int:
        """Number of repricings performed."""
        ...

    @property
    def residual_pct(self) -> float:
        """Residual as percentage of total P&L."""
        ...

    @property
    def notes(self) -> list[str]:
        """Diagnostic notes and warnings."""
        ...

    @property
    def result_invalid(self) -> bool:
        """True when attribution was flagged invalid and residual checks should fail."""
        ...

    def residual_within_tolerance(
        self,
        pct_tolerance: float | None = None,
        abs_tolerance: float | None = None,
    ) -> bool:
        """Check if residual is within tolerance.

        Args:
            pct_tolerance: Percentage tolerance (e.g. 0.1 for 0.1%).
                Defaults to the attribution's stored ``meta.tolerance_pct``.
            abs_tolerance: Absolute tolerance (e.g. 100.0 for $100).
                Defaults to the attribution's stored ``meta.tolerance_abs``.

        Returns:
            ``True`` if residual is within tolerance.
        """
        ...

    def residual_within_meta_tolerance(self) -> bool:
        """Check residual using the attribution's stored method-specific tolerances."""
        ...

    def validate_currencies(self) -> None:
        """Validate that every factor's currency matches ``total_pnl.currency``.

        Useful before building a DataFrame or summing across instruments.

        Raises:
            ValueError: When any factor's currency differs from ``total_pnl.currency``.
        """
        ...

    def explain(self) -> str:
        """Human-readable tree explanation (non-zero factors only).

        Returns:
            Multi-line string with tree structure showing P&L breakdown.
        """
        ...

    def explain_verbose(self) -> str:
        """Verbose tree explanation including zero-valued factors.

        Returns:
            Multi-line string with tree structure showing all factors.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """Export attribution as a single-row pandas DataFrame.

        Columns include ``instrument_id``, ``method``, ``t0``, ``t1``,
        ``currency``, ``total_pnl``, ``mark_to_market_pnl`` (nullable), all
        factor P&L amounts, ``residual``, ``residual_pct``,
        ``num_repricings``, and ``result_invalid``.

        Returns:
            Single-row DataFrame.
        """
        ...

    def to_long_dataframe(self) -> pd.DataFrame:
        """Export every populated detail breakdown as one long-format DataFrame.

        Columns:
            kind: dotted-path identifier (e.g. ``"rates.by_curve"``,
                ``"rates.by_tenor"``, ``"credit.by_curve"``, ``"fx.by_pair"``,
                ``"vol.by_surface"``, ``"cross_factor.by_pair"``,
                ``"scalars.dividends"``, ``"credit_factor.generic"``,
                ``"credit_factor.level"``, ``"credit_factor.adder"``,
                ``"credit_factor.curve_shape"``, ``"carry.theta"``,
                ``"carry.coupon_income"``, ...).
            factor: parent factor family (``"rates"``, ``"credit"``, ``"fx"``,
                ``"vol"``, ``"cross_factor"``, ``"scalars"``,
                ``"credit_factor"``, ``"carry"``, ``"inflation"``,
                ``"correlations"``, ``"model_params"``).
            key_a: primary identifier (curve_id, pair label, surface_id,
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

        Returns:
            Long-format DataFrame.
        """
        ...

    def to_carry_detail_dataframe(self) -> pd.DataFrame:
        """Export the carry decomposition as a typed long DataFrame.

        Columns are the same as :meth:`to_long_dataframe` but the kind values
        are limited to the ``"carry.*"`` family — useful when you only want the
        carry split (coupon income, pull-to-par, roll-down, funding cost,
        theta), including the optional rates/credit splits when a credit
        factor model was supplied.

        Returns an empty DataFrame when ``carry_detail`` is not populated.

        Returns:
            Carry-decomposition DataFrame.
        """
        ...

    def to_credit_factor_dataframe(self) -> pd.DataFrame:
        """Export the credit-factor hierarchy decomposition as a typed long DataFrame.

        Columns are the same as :meth:`to_long_dataframe`; rows are limited to
        the ``"credit_factor.*"`` family. Includes generic, per-level, adder,
        curve_shape, plus per-bucket and per-issuer rows when present.

        Returns an empty DataFrame when ``credit_factor_detail`` is not
        populated (no ``credit_factor_model`` was supplied, or the instrument
        has no resolvable issuer).

        Returns:
            Credit-factor DataFrame.
        """
        ...

    def __repr__(self) -> str: ...

def attribute_pnl(
    instrument_json: str,
    market_t0_json: str,
    market_t1_json: str,
    as_of_t0: str,
    as_of_t1: str,
    method: str | dict,
    config: dict | None = None,
    full_cross_attribution: bool | None = None,
) -> str:
    """Run P&L attribution for a single instrument.

    This is the main entry point. Accepts the instrument, two market
    snapshots, valuation dates, and a method descriptor and returns the
    canonical JSON form of the attribution. Use
    ``PnlAttribution.from_json(...)`` when you want the richer wrapper.

    Args:
        instrument_json: Tagged instrument JSON (``{"type": "bond", ...}``).
        market_t0_json: JSON-serialized ``MarketContext`` at T₀.
        market_t1_json: JSON-serialized ``MarketContext`` at T₁.
        as_of_t0: Valuation date T₀ in ISO 8601 format.
        as_of_t1: Valuation date T₁ in ISO 8601 format.
        method: Attribution method — one of ``"Parallel"``,
            ``{"Waterfall": ["Carry", "RatesCurves", ...]}``,
            ``"MetricsBased"``, or ``{"Taylor": {"include_gamma": True, ...}}``.
        config: Optional config overrides (tolerance, metrics, bump sizes,
            target currency, or ``{"execution_policy": "serial"}`` for
            callers that already parallelize at the portfolio/batch level).
        full_cross_attribution: Option to compute all 36 cross-factor pairs when enabled.

    Returns:
        Compact JSON ``PnlAttribution`` payload.

    Example:
        >>> attr_json = attribute_pnl(inst, mkt_t0, mkt_t1, "2025-01-15", "2025-01-16", "Parallel")
        >>> attr = PnlAttribution.from_json(attr_json)  # doctest: +SKIP
        >>> print(attr.explain())  # doctest: +SKIP
    """
    ...

def attribute_pnl_from_spec(spec_json: str) -> str:
    """Run attribution from a full JSON ``AttributionEnvelope``.

    Power-user variant for full envelope round-trip workflows.
    Most users should prefer :func:`attribute_pnl`.

    Args:
        spec_json: JSON-serialized ``AttributionEnvelope``.

    Returns:
        JSON-serialized ``AttributionResultEnvelope``.
    """
    ...

def validate_attribution_json(json: str) -> str:
    """Validate an attribution specification JSON.

    Deserializes against the ``AttributionEnvelope`` schema and returns
    the canonical (re-serialized) JSON.

    Args:
        json: JSON-serialized ``AttributionEnvelope``.

    Returns:
        Canonical pretty-printed JSON.
    """
    ...

def default_waterfall_order() -> list[str]:
    """Return the default waterfall factor ordering.

    Returns:
        Factor names in the default waterfall order.

    Example:
        >>> from finstack.attribution import default_waterfall_order
        >>> default_waterfall_order()  # doctest: +SKIP
        ['Carry', 'RatesCurves', 'CreditCurves', ...]
    """
    ...

def default_attribution_metrics() -> list[str]:
    """Return the default metric IDs used by metrics-based attribution.

    Returns:
        Metric identifier strings.

    Example:
        >>> from finstack.attribution import default_attribution_metrics
        >>> default_attribution_metrics()  # doctest: +SKIP
        ['theta', 'dv01', 'cs01', ...]
    """
    ...
