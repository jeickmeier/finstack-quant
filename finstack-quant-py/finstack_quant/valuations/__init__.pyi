"""Instrument pricing, risk metrics, P&L attribution, and market-context bootstrapping.

The canonical path to build a :class:`finstack_quant.core.market_data.MarketContext`
from raw market quotes is :func:`calibrate`:

    >>> import json
    >>> from finstack_quant.valuations import calibrate
    >>> envelope = {
    ...     "schema": "finstack_quant.calibration",
    ...     "plan": {
    ...         "id": "usd_curves",
    ...         "quote_sets": {"usd_quotes": ["USD-SOFR-DEP-1M", "USD-OIS-SWAP-1Y"]},
    ...         "steps": [{"id": "USD-OIS", "quote_set": "usd_quotes",
    ...                    "kind": "discount", ...}],
    ...         "settings": {},
    ...     },
    ...     "market_data": [
    ...         {"kind": "rate_quote", "type": "deposit", "id": "USD-SOFR-DEP-1M", ...},
    ...         {"kind": "rate_quote", "type": "swap",    "id": "USD-OIS-SWAP-1Y", ...},
    ...     ],
    ... }
    >>> result = calibrate(json.dumps(envelope))  # doctest: +SKIP
    >>> result.success  # doctest: +SKIP
    True
    >>> result.rmse  # doctest: +SKIP    # check the curves actually fit
    1.2e-9
    >>> ctx = result.market  # doctest: +SKIP    # ready for pricing/attribution

The :class:`CalibrationResult` wrapper carries the :class:`MarketContext` next
to per-step residuals (:meth:`step_report_json`, :meth:`report_to_dataframe`)
so users can verify their curves actually fit before consuming them downstream.

A ``CalibrationEnvelope`` carries inputs in three sections:

- ``plan`` — execution recipe. ``plan.steps`` declares calibration steps in
  declared order; ``plan.quote_sets`` maps a set name to a list of quote IDs
  that resolve into ``market_data``.
- ``market_data`` — flat, id-addressable list of all input data. Each entry
  has a ``"kind"`` discriminator. Quotes (``rate_quote``, ``cds_quote``,
  ``fx_quote``, ``inflation_quote``, ``vol_quote``, ``xccy_quote``, ``bond_quote``,
  ``cds_tranche_quote``) feed calibration steps. Snapshot data
  (``fx_spot``, ``price``, ``dividend_schedule``, ``fixing_series``,
  ``inflation_fixings``, ``credit_index``, ``fx_vol_surface``, ``vol_cube``,
  ``collateral``) is passed through into the resulting :class:`MarketContext`.
- ``prior_market`` — optional list of pre-built calibrated curves or surfaces
  from a previous run, layered in before steps execute.

Reference envelope JSON examples covering both Track-A (bootstrap from quotes)
and Track-B (snapshot-only) live under
``finstack-quant/valuations/examples/market_bootstrap/`` in the repository.

Instrument pricing helpers live under :mod:`finstack_quant.valuations.instruments`.
This module exposes calibration, SABR / Black-Scholes primitives, and
credit-factor hierarchy tooling. Portfolio factor sensitivities and risk
decomposition live under :mod:`finstack_quant.portfolio`.
"""

from __future__ import annotations

from typing import Any

import pandas as pd

from finstack_quant.core.market_data import MarketContext
from finstack_quant.valuations import correlation as correlation
from finstack_quant.valuations import instruments as instruments
from finstack_quant.valuations import models as models
from finstack_quant.valuations.envelope import CalibrationEnvelope as CalibrationEnvelope
from finstack_quant.valuations.instruments import price_instrument_with_metrics as price_instrument_with_metrics

__all__ = [
    "correlation",
    "instruments",
    "models",
    "price_instrument_with_metrics",
    "ValuationResult",
    "CalibrationEnvelope",
    "CalibrationEnvelopeError",
    "CalibrationResult",
    "validate_calibration_json",
    "calibrate",
    "dry_run",
    "dependency_graph_json",
    "bs_cos_price",
    "vg_cos_price",
    "merton_jump_cos_price",
    "tarn_coupon_profile",
    "snowball_coupon_profile",
    "inverse_floater_coupon_profile",
    "cms_spread_option_intrinsic",
    "callable_range_accrual_accrued",
    "bs_price",
    "bs_greeks",
    "bs_implied_vol",
    "black76_implied_vol",
    "barrier_call",
    "asian_option_price",
    "lookback_option_price",
    "quanto_option_price",
    "SabrParameters",
    "SabrModel",
    "SabrSmile",
    "SabrCalibrator",
    "instrument_cashflows",
]

class ValuationResult:
    """Valuation envelope: PV, currency, risk metrics, covenant flags, and JSON round-trip.

    Instantiate via :meth:`from_json` or the ``price_*`` helpers that emit JSON.

    Examples
    --------
    >>> from finstack_quant.valuations import ValuationResult
    >>> ValuationResult.from_json(result_json)  # doctest: +SKIP
    """

    @staticmethod
    def from_json(json: str) -> ValuationResult:
        """Deserialize a ``ValuationResult`` from JSON.

        Parameters
        ----------
        json : str
            JSON string produced by the pricing pipeline or ``to_json``.

        Returns
        -------
        ValuationResult
            Parsed ``ValuationResult`` instance.

        Examples
        --------
        >>> from finstack_quant.valuations import ValuationResult
        >>> ValuationResult.from_json('{"instrument_id":"x","value":{}}')  # doctest: +SKIP
        """
        ...

    def to_json(self) -> str:
        """Serialize this result to pretty-printed JSON.

        Returns
        -------
        str
            Pretty-printed JSON string.

        Examples
        --------
        >>> ValuationResult.from_json(
        ...     '{"instrument_id":"i","value":{"amount":1.0,"currency":"USD"},"measures":{}}'
        ... ).to_json()  # doctest: +SKIP
            ''
        """
        ...

    @property
    def instrument_id(self) -> str:
        """Instrument identifier assigned by the pricer.

        Returns
        -------
        str
            Instrument ID string.

        Examples
        --------
        >>> vr = ValuationResult.from_json("{}")  # doctest: +SKIP
        >>> vr.instrument_id  # doctest: +SKIP
        ''
        """
        ...

    @property
    def price(self) -> float:
        """Present value amount (NPV).

        Returns
        -------
        float
            PV amount as a float.

        Examples
        --------
        >>> vr = ValuationResult.from_json("{}")  # doctest: +SKIP
        >>> vr.price  # doctest: +SKIP
        0.0
        """
        ...

    @property
    def currency(self) -> str:
        """Currency code for the present value.

        Returns
        -------
        str
            Currency code string.

        Examples
        --------
        >>> vr = ValuationResult.from_json("{}")  # doctest: +SKIP
        >>> vr.currency  # doctest: +SKIP
        'USD'
        """
        ...

    def get_metric(self, key: str) -> float | None:
        """Return a scalar risk measure by string key.

        Parameters
        ----------
        key : str
            Metric identifier (e.g. ``"ytm"``, ``"dv01"``).

        Returns
        -------
        float or None
            Metric value, or ``None`` if missing.

        Examples
        --------
        >>> vr = ValuationResult.from_json(
        ...     '{"instrument_id":"i","value":{"amount":1,"currency":"USD"},"measures":{}}'
        ... )  # doctest: +SKIP
        >>> vr.get_metric("ytm")  # doctest: +SKIP
        """
        ...

    def metric_keys(self) -> list[str]:
        """List metric keys present on this result.

        Returns
        -------
        list[str]
            All measure keys as strings.

        Examples
        --------
        >>> ValuationResult.from_json(
        ...     '{"instrument_id":"i","value":{"amount":1,"currency":"USD"},"measures":{}}'
        ... ).metric_keys()  # doctest: +SKIP
        []
        """
        ...

    def metric_count(self) -> int:
        """Count of measures stored on this result.

        Returns
        -------
        int
            Number of entries in the measures map.

        Examples
        --------
        >>> ValuationResult.from_json(
        ...     '{"instrument_id":"i","value":{"amount":1,"currency":"USD"},"measures":{}}'
        ... ).metric_count()  # doctest: +SKIP
        0
        """
        ...

    def all_covenants_passed(self) -> bool:
        """Whether every covenant passed (or none were evaluated).

        Returns
        -------
        bool
            ``True`` if no covenant failures are recorded.

        Examples
        --------
        >>> ValuationResult.from_json(
        ...     '{"instrument_id":"i","value":{"amount":1,"currency":"USD"},"measures":{}}'
        ... ).all_covenants_passed()  # doctest: +SKIP
        True
        """
        ...

    def failed_covenants(self) -> list[str]:
        """Covenant IDs that failed, if any.

        Returns
        -------
        list[str]
            List of failed covenant identifiers.

        Examples
        --------
        >>> ValuationResult.from_json(
        ...     '{"instrument_id":"i","value":{"amount":1,"currency":"USD"},"measures":{}}'
        ... ).failed_covenants()  # doctest: +SKIP
        []
        """
        ...

    def metrics_to_dataframe(self) -> pd.DataFrame:
        """Export as a single-row pandas DataFrame.

        Columns include ``instrument_id``, ``price``, ``currency``, plus one
        column per metric key.  Useful for stacking multiple results with
        ``pd.concat``.

        Returns
        -------
        pd.DataFrame
            Single-row DataFrame with one column per metric.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug string for this result.

        Returns
        -------
        str
            ``ValuationResult(id=..., price=..., currency=..., metrics=...)`` text.

        Examples
        --------
        >>> repr(ValuationResult.from_json("{}"))  # doctest: +SKIP
        ''
        """
        ...

def instrument_cashflows(
    instrument_json: str,
    market: MarketContext | str,
    as_of: str,
    *,
    model: str = "discounting",
) -> tuple[dict[str, Any], pd.DataFrame]:
    """DataFrame-friendly wrapper around :func:`instrument_cashflows_json`.

    Parses the JSON envelope returned by the low-level binding and constructs
    a per-flow ``pandas.DataFrame`` with ``date`` / ``reset_date`` parsed as
    ``datetime64``. See :func:`instrument_cashflows_json` for argument and
    error semantics.

    Returns
    -------
    tuple[dict[str, Any], pd.DataFrame]
        ``(envelope, df)`` where ``envelope`` is the parsed dict and ``df``
        carries one row per flow with columns ``date``, ``amount``,
        ``currency``, ``kind``, ``accrual_factor``, ``year_fraction``,
        ``rate``, ``reset_date``, ``discount_factor``, ``survival_probability``,
        ``conditional_default_prob``, ``inflation_index_ratio``,
        ``prepayment_smm``, ``beginning_balance``, ``ending_balance``, and
        ``pv``.
    """
    ...

# ---------------------------------------------------------------------------
# Calibration
# ---------------------------------------------------------------------------

class CalibrationResult:
    """Result of a calibration plan execution.

    Provides access to the calibrated market context, per-step reports,
    and overall success status.  Construct via :func:`calibrate` or
    :meth:`from_json`.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations import calibrate
    >>> result = calibrate(json.dumps(plan))  # doctest: +SKIP
    >>> result.success  # doctest: +SKIP
    True
    """

    @staticmethod
    def from_json(json: str) -> CalibrationResult:
        """Deserialize a ``CalibrationResult`` from JSON.

        Parameters
        ----------
        json : str
            JSON string (a ``CalibrationResultEnvelope``).

        Returns
        -------
        CalibrationResult
            Parsed ``CalibrationResult`` instance.
        """
        ...

    def to_json(self) -> str:
        """Serialize to pretty-printed JSON.

        Returns
        -------
        str
            Pretty-printed JSON string.
        """
        ...

    @property
    def success(self) -> bool:
        """Whether the overall calibration succeeded (all steps passed).

        Returns
        -------
        bool
            ``True`` if all steps passed.
        """
        ...

    @property
    def market(self) -> MarketContext:
        """The calibrated ``MarketContext`` containing all produced curves.

        Returns
        -------
        MarketContext
            Live market context ready for pricing and attribution.
        """
        ...

    @property
    def market_json(self) -> str:
        """The calibrated market serialized as a JSON string.

        Returns
        -------
        str
            JSON snapshot of the calibrated market.
        """
        ...

    @property
    def report_json(self) -> str:
        """The aggregated calibration report as a JSON string.

        Returns
        -------
        str
            JSON-serialized calibration report.
        """
        ...

    @property
    def step_ids(self) -> list[str]:
        """List of step identifiers that were executed.

        Returns
        -------
        list[str]
            Step IDs in declared order.
        """
        ...

    @property
    def iterations(self) -> int:
        """Total solver iterations across all steps.

        Returns
        -------
        int
            Sum of solver iterations.
        """
        ...

    @property
    def max_residual(self) -> float:
        """Maximum absolute residual across all steps.

        Returns
        -------
        float
            Largest absolute residual.
        """
        ...

    @property
    def rmse(self) -> float:
        """Root mean square error across all steps.

        Returns
        -------
        float
            RMSE of all step residuals.
        """
        ...

    def step_report_json(self, step_id: str) -> str:
        """Per-step calibration report as a JSON string.

        Parameters
        ----------
        step_id : str
            Identifier of the calibration step.

        Returns
        -------
        str
            JSON-serialized calibration report for the step.

        Raises
        ------
        ValueError
            If no step with the given *step_id* exists.
        """
        ...

    def report_to_dataframe(self) -> pd.DataFrame:
        """Per-step summary as a pandas DataFrame.

        Columns: ``step_id``, ``success``, ``iterations``, ``max_residual``,
        ``rmse``, ``convergence_reason``.

        Returns
        -------
        pd.DataFrame
            DataFrame with one row per calibration step.
        """
        ...

    def __repr__(self) -> str: ...

class CalibrationEnvelopeError(RuntimeError):
    """Raised when a calibration envelope fails validation or solving.

    Inherits from :class:`RuntimeError`, so existing ``except RuntimeError``
    callers continue to catch it (backward-compatible with pre-Phase-4 code).

    Attributes
    ----------
    kind : str
        Snake-case discriminator for the failure category. One of
        ``"json_parse"``, ``"unknown_step_kind"``, ``"missing_dependency"``,
        ``"undefined_quote_set"``, ``"quote_class_mismatch"``,
        ``"solver_not_converged"``, ``"quote_data_invalid"``.
    step_id : str or None
        Identifier of the offending step, when applicable. ``None``
        for ``"json_parse"``.
    details : str
        JSON-serialized structured payload (see ``EnvelopeError``
        in the Rust crate for the schema).
    """

    kind: str
    step_id: str | None
    details: str

def validate_calibration_json(json: str) -> str:
    """Validate a calibration plan JSON and return canonical pretty-printed form.

    Parameters
    ----------
    json : str
        JSON-serialized ``CalibrationEnvelope``.

    Returns
    -------
    str
        Canonical pretty-printed JSON.

    Raises
    ------
    CalibrationEnvelopeError
        If the JSON is not a valid calibration envelope. Inherits from
        :class:`RuntimeError`.

    Examples
    --------
    >>> from finstack_quant.valuations import validate_calibration_json
    >>> validate_calibration_json(plan_json)  # doctest: +SKIP
    ''
    """
    ...

def dry_run(json: str) -> str:
    """Pre-flight envelope validation without invoking the solver.

    Runs all structural checks (missing dependencies, undefined ``quote_set``s,
    cycles) in a single pass and returns a JSON-serialized
    ``ValidationReport`` listing every error found plus the dependency graph.
    Microseconds — suitable as a fast pre-flight check before invoking
    :func:`calibrate`.

    Parameters
    ----------
    json : str
        JSON-serialized ``CalibrationEnvelope``.

    Returns
    -------
    str
        Pretty-printed JSON ``ValidationReport``. Inspect ``report["errors"]``
        for any structural problems and ``report["dependency_graph"]`` for the
        step DAG.

    Raises
    ------
    CalibrationEnvelopeError
        If the envelope JSON is malformed.

    Examples
    --------
    >>> import json as _json
    >>> from finstack_quant.valuations import dry_run
    >>> report = _json.loads(dry_run(_json.dumps(envelope)))  # doctest: +SKIP
    >>> for err in report["errors"]:  # doctest: +SKIP
    ...     print(err["kind"], err.get("step_id"))
    """
    ...

def dependency_graph_json(json: str) -> str:
    """Dump the static dependency graph of a calibration plan as JSON.

    Parameters
    ----------
    json : str
        JSON-serialized ``CalibrationEnvelope``.

    Returns
    -------
    str
        Pretty-printed JSON ``DependencyGraph`` with ``initial_ids`` (curve
        IDs available at execution start, sourced from ``prior_market``)
        and ``nodes`` (per-step ``reads``/``writes`` in declared order).

    Raises
    ------
    CalibrationEnvelopeError
        If the envelope JSON is malformed.
    """
    ...

def calibrate(json: str) -> CalibrationResult:
    """Build a :class:`MarketContext` from raw market quotes — the canonical entry point.

    Accepts a JSON-serialized ``CalibrationEnvelope``. The envelope carries
    quotes in two complementary places:

    - ``plan.quote_sets`` + ``plan.steps`` — quote-driven calibration steps
      (discount, forward, hazard, vol surface, swaption vol, base correlation,
      etc.). Each step reads its named list of quote IDs and resolves them
      against ``market_data``.
    - ``market_data`` — flat, id-addressable list of inputs. Quotes drive
      calibration steps; snapshot data (FX spots, prices, dividends, fixings,
      etc.) is passed through. Snapshot ``MarketQuote`` variants for FX and
      Bond exist for documentation but are not consumed by any calibration
      step today; pass FX rates as ``"kind": "fx_spot"`` entries and prices
      as ``"kind": "price"`` entries.
    - ``prior_market`` — pre-built curves and surfaces from a prior
      calibration, layered in before steps execute.

    Parameters
    ----------
    json : str
        JSON-serialized ``CalibrationEnvelope`` (schema string is
        ``"finstack_quant.calibration"``).

    Returns
    -------
    CalibrationResult
        :class:`CalibrationResult` with:
        - ``.market`` — the live :class:`MarketContext` (use this for
          pricing, attribution, scenarios, portfolio).
        - ``.market_json`` — same context as a JSON snapshot for
          persistence or comparison.
        - ``.report_json`` / ``.step_report_json(id)`` /
          ``.report_to_dataframe()`` — diagnostics. Always check
          ``.success`` and ``.rmse`` before relying on the produced market.
        - ``.iterations``, ``.max_residual``, ``.step_ids`` — summary stats.

    Raises
    ------
    CalibrationEnvelopeError
        If the JSON is malformed or calibration fails (e.g., missing
        dependency, solver non-convergence). The exception carries ``kind``,
        ``step_id``, and ``details`` attributes for programmatic handling.
        Inherits from :class:`RuntimeError` so legacy ``except RuntimeError``
        handlers continue to catch it.

    Examples
    --------
    >>> import json as _json
    >>> from finstack_quant.valuations import calibrate
    >>> result = calibrate(_json.dumps(envelope))  # doctest: +SKIP
    >>> assert result.success and result.rmse < 1e-6  # doctest: +SKIP
    >>> curve = result.market.get_discount("USD-OIS")  # doctest: +SKIP
    >>> from finstack_quant.valuations.instruments import price_instrument
    >>> price_json = price_instrument(inst_json, result.market_json, "2026-05-08")  # doctest: +SKIP

    See Also
    --------
    - ``finstack-quant/valuations/examples/market_bootstrap/`` — reference
      envelope JSON files (discount curve, single-name hazard, FX matrix).
    - :func:`validate_calibration_json` — pre-flight envelope check.
    """
    ...

# ---------------------------------------------------------------------------
# Closed-form analytic primitives (Black-Scholes / Black-76)
# ---------------------------------------------------------------------------

def bs_price(
    spot: float,
    strike: float,
    r: float,
    q: float,
    sigma: float,
    t: float,
    is_call: bool,
) -> float:
    """Per-unit Black-Scholes / Garman-Kohlhagen price of a European option.

    All rates are continuously compounded decimals; ``sigma`` is annualized
    vol; ``t`` is years to expiry. Pass ``is_call=False`` for puts.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    r : float
        Continuously compounded risk-free rate (decimal).
    q : float
        Continuous dividend/borrow yield (decimal).
    sigma : float
        Annualized volatility (decimal).
    t : float
        Time to expiry in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put.

    Returns
    -------
    float
        Per-unit option price.
    """
    ...

def bs_greeks(
    spot: float,
    strike: float,
    r: float,
    q: float,
    sigma: float,
    t: float,
    is_call: bool,
    theta_days: float = 365.0,
) -> dict[str, float]:
    """Black-Scholes / Garman-Kohlhagen Greeks as a dict.

    Returns ``{"delta", "gamma", "vega", "theta", "rho", "rho_q"}``. ``vega``
    and both rho values are per 1% move; ``theta`` is per-day using the
    ``theta_days`` day-count denominator (ACT/365 by default).

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    r : float
        Continuously compounded risk-free rate (decimal).
    q : float
        Continuous dividend/borrow yield (decimal).
    sigma : float
        Annualized volatility (decimal).
    t : float
        Time to expiry in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    theta_days : float, default 365.0
        Day-count denominator for theta scaling.

    Returns
    -------
    dict[str, float]
        Greeks dict with keys ``delta``, ``gamma``, ``vega``, ``theta``,
        ``rho``, ``rho_q``.
    """
    ...

def bs_implied_vol(
    spot: float,
    strike: float,
    r: float,
    q: float,
    t: float,
    price: float,
    is_call: bool,
) -> float:
    """Solve for Black-Scholes implied volatility given a target price.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    r : float
        Continuously compounded risk-free rate (decimal).
    q : float
        Continuous dividend/borrow yield (decimal).
    t : float
        Time to expiry in years.
    price : float
        Observed option price in the same units as spot.
    is_call : bool
        ``True`` for a call, ``False`` for a put.

    Returns
    -------
    float
        Implied volatility as a decimal.

    Raises
    ------
    ValueError
        If inputs are invalid or no root exists in the search bracket.
    """
    ...

def black76_implied_vol(
    forward: float,
    strike: float,
    df: float,
    t: float,
    price: float,
    is_call: bool,
) -> float:
    """Solve for Black-76 (forward-based) implied volatility given a target price.

    Parameters
    ----------
    forward : float
        Forward price at expiry.
    strike : float
        Option strike.
    df : float
        Discount factor from valuation date to expiry.
    t : float
        Time to expiry in years.
    price : float
        Observed option price (same units as forward).
    is_call : bool
        ``True`` for a call, ``False`` for a put.

    Returns
    -------
    float
        Implied volatility as a decimal.

    Raises
    ------
    ValueError
        If inputs are invalid or no root exists in the search bracket.
    """
    ...

# ---------------------------------------------------------------------------
# Closed-form exotics
# ---------------------------------------------------------------------------

def barrier_call(
    spot: float,
    strike: float,
    barrier: float,
    r: float,
    q: float,
    sigma: float,
    t: float,
    direction: str,
    knock: str,
) -> float:
    """Reiner-Rubinstein continuous-monitoring barrier call price.

    ``direction`` is ``"up"`` or ``"down"``; ``knock`` is ``"in"`` or ``"out"``.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    barrier : float
        Barrier level.
    r : float
        Continuously compounded risk-free rate (decimal).
    q : float
        Continuous dividend/borrow yield (decimal).
    sigma : float
        Annualized volatility (decimal).
    t : float
        Time to expiry in years.
    direction : str
        ``"up"`` or ``"down"``.
    knock : str
        ``"in"`` or ``"out"``.

    Returns
    -------
    float
        Per-unit barrier call price.
    """
    ...

def asian_option_price(
    spot: float,
    strike: float,
    r: float,
    q: float,
    sigma: float,
    t: float,
    num_fixings: int,
    averaging: str = "arithmetic",
    is_call: bool = True,
) -> float:
    """Arithmetic (Turnbull-Wakeman) or geometric (Kemna-Vorst) Asian option price.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    r : float
        Continuously compounded risk-free rate (decimal).
    q : float
        Continuous dividend/borrow yield (decimal).
    sigma : float
        Annualized volatility (decimal).
    t : float
        Time to expiry in years.
    num_fixings : int
        Number of averaging fixings.
    averaging : str, default "arithmetic"
        ``"arithmetic"`` (Turnbull-Wakeman) or ``"geometric"`` (Kemna-Vorst).
    is_call : bool, default True
        ``True`` for a call, ``False`` for a put.

    Returns
    -------
    float
        Per-unit Asian option price.
    """
    ...

def lookback_option_price(
    spot: float,
    strike: float,
    r: float,
    q: float,
    sigma: float,
    t: float,
    extremum: float,
    strike_type: str = "fixed",
    is_call: bool = True,
) -> float:
    """Conze-Viswanathan lookback option price.

    For ``strike_type="floating"``, ``strike`` is ignored and ``extremum``
    is the observed min (call) / max (put) to date.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike (ignored for floating strike).
    r : float
        Continuously compounded risk-free rate (decimal).
    q : float
        Continuous dividend/borrow yield (decimal).
    sigma : float
        Annualized volatility (decimal).
    t : float
        Time to expiry in years.
    extremum : float
        Observed extremum (min for call, max for put) to date.
    strike_type : str, default "fixed"
        ``"fixed"`` or ``"floating"``.
    is_call : bool, default True
        ``True`` for a call, ``False`` for a put.

    Returns
    -------
    float
        Per-unit lookback option price.
    """
    ...

def quanto_option_price(
    spot: float,
    strike: float,
    t: float,
    rate_domestic: float,
    rate_foreign: float,
    div_yield: float,
    vol_asset: float,
    vol_fx: float,
    correlation: float,
    is_call: bool = True,
) -> float:
    """Quanto option (FX-adjusted cross-currency) price in domestic currency.

    Parameters
    ----------
    spot : float
        Spot price of the underlying in foreign currency.
    strike : float
        Option strike in foreign currency.
    t : float
        Time to expiry in years.
    rate_domestic : float
        Domestic risk-free rate (decimal, continuously compounded).
    rate_foreign : float
        Foreign risk-free rate (decimal, continuously compounded).
    div_yield : float
        Dividend yield on the underlying (decimal).
    vol_asset : float
        Volatility of the underlying asset (decimal).
    vol_fx : float
        Volatility of the FX rate (decimal).
    correlation : float
        Correlation between asset and FX returns.
    is_call : bool, default True
        ``True`` for a call, ``False`` for a put.

    Returns
    -------
    float
        Per-unit quanto option price in domestic currency.
    """
    ...

# ---------------------------------------------------------------------------
# SABR volatility smile
# ---------------------------------------------------------------------------

class SabrParameters:
    """SABR parameters ``(alpha, beta, nu, rho)`` with optional ``shift``.

    Enforces ``alpha > 0``, ``beta in [0, 1]``, ``nu >= 0``, ``rho in
    [-1, 1]``, and ``shift > 0`` when supplied.
    """

    def __init__(
        self,
        alpha: float,
        beta: float,
        nu: float,
        rho: float,
        shift: float | None = None,
    ) -> None: ...
    @staticmethod
    def equity_default() -> SabrParameters:
        """Equity-standard defaults ``(alpha=0.20, beta=1.0, nu=0.30, rho=-0.20)``.

        Returns
        -------
        SabrParameters
            Default equity SABR parameters.
        """
        ...

    @staticmethod
    def rates_default() -> SabrParameters:
        """Rates-standard defaults ``(alpha=0.02, beta=0.5, nu=0.30, rho=0.0)``.

        Returns
        -------
        SabrParameters
            Default rates SABR parameters.
        """
        ...

    @property
    def alpha(self) -> float:
        """SABR alpha level parameter.

        Returns
        -------
        float
            Alpha parameter value.
        """
        ...

    @property
    def beta(self) -> float:
        """SABR beta elasticity parameter in ``[0, 1]``.

        Returns
        -------
        float
            Beta parameter value.
        """
        ...

    @property
    def nu(self) -> float:
        """Volatility-of-volatility parameter.

        Returns
        -------
        float
            Nu parameter value.
        """
        ...

    @property
    def rho(self) -> float:
        """Forward/volatility correlation parameter in ``[-1, 1]``.

        Returns
        -------
        float
            Rho parameter value.
        """
        ...

    @property
    def shift(self) -> float | None:
        """Optional positive shift used for negative-rate smiles.

        Returns
        -------
        float or None
            Shift value, or ``None`` if not set.
        """
        ...

    def is_shifted(self) -> bool:
        """``True`` when parameters include a non-zero shift (negative-rate support).

        Returns
        -------
        bool
            ``True`` if a non-zero shift is present.
        """
        ...

class SabrModel:
    """Hagan-2002 SABR stochastic-volatility smile model.

    Sources
    -------
    - Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr

    Examples
    --------
    >>> from finstack_quant.valuations import SabrModel, SabrParameters
    >>> model = SabrModel(SabrParameters.rates_default())  # doctest: +SKIP
    >>> model.implied_vol(0.02, 0.02, 1.0)  # doctest: +SKIP
    0.20
    """

    def __init__(self, params: SabrParameters) -> None:
        """Create a SABR model from validated parameters.

        Parameters
        ----------
        params : SabrParameters
            SABR parameter set (alpha, beta, nu, rho, optional shift).

        Raises
        ------
        ValueError
            If parameters violate SABR constraints.
        """
        ...

    def implied_vol(self, forward: float, strike: float, t: float) -> float:
        """Black-style implied volatility under the Hagan-2002 expansion.

        Parameters
        ----------
        forward : float
            Forward price at expiry.
        strike : float
            Strike price.
        t : float
            Time to expiry in years.

        Returns
        -------
        float
            Implied volatility as a decimal.
        """
        ...

    @property
    def params(self) -> SabrParameters:
        """Parameters used by this model.

        Returns
        -------
        SabrParameters
            The SABR parameter set.
        """
        ...

    def supports_negative_rates(self) -> bool:
        """Return ``True`` when the model has a positive shift.

        Returns
        -------
        bool
            ``True`` if the shift is non-zero, enabling negative-rate smiles.
        """
        ...

class SabrSmile:
    """Volatility smile generator for a fixed ``(forward, t)`` pair.

    Examples
    --------
    >>> from finstack_quant.valuations import SabrSmile, SabrParameters
    >>> smile = SabrSmile(SabrParameters.equity_default(), 100.0, 1.0)  # doctest: +SKIP
    >>> smile.atm_vol()  # doctest: +SKIP
    0.20
    """

    def __init__(
        self,
        params: SabrParameters,
        forward: float,
        t: float,
    ) -> None:
        """Create a smile helper for one forward and expiry.

        Parameters
        ----------
        params : SabrParameters
            SABR parameter set.
        forward : float
            Forward price at expiry.
        t : float
            Time to expiry in years.
        """
        ...

    def atm_vol(self) -> float:
        """Return the ATM implied volatility.

        Returns
        -------
        float
            ATM implied vol as a decimal.
        """
        ...

    def implied_vol(self, strike: float) -> float:
        """Return implied volatility at ``strike``.

        Parameters
        ----------
        strike : float
            Strike price.

        Returns
        -------
        float
            Implied vol as a decimal.
        """
        ...

    def generate_smile(self, strikes: list[float]) -> list[float]:
        """Return implied volatilities for all supplied strikes.

        Parameters
        ----------
        strikes : list[float]
            Strike grid.

        Returns
        -------
        list[float]
            Implied vols aligned with ``strikes``.
        """
        ...

    def arbitrage_diagnostics(
        self,
        strikes: list[float],
        r: float = 0.0,
        q: float = 0.0,
    ) -> dict[str, Any]:
        """Butterfly + monotonicity arbitrage diagnostics on ``strikes``.

        Returns a dict with ``arbitrage_free``, ``butterfly_violations``,
        and ``monotonicity_violations``.

        Parameters
        ----------
        strikes : list[float]
            Strike grid to test.
        r : float, default 0.0
            Risk-free rate (decimal).
        q : float, default 0.0
            Dividend yield (decimal).

        Returns
        -------
        dict[str, Any]
            Diagnostics dict with ``arbitrage_free``, ``butterfly_violations``,
            and ``monotonicity_violations``.
        """
        ...

class SabrCalibrator:
    """SABR calibrator (Levenberg-Marquardt with beta fixed).

    Examples
    --------
    >>> from finstack_quant.valuations import SabrCalibrator
    >>> cal = SabrCalibrator()  # doctest: +SKIP
    >>> params = cal.calibrate(0.02, strikes, vols, 1.0, 0.5)  # doctest: +SKIP
    """

    def __init__(self) -> None:
        """Create a default SABR calibrator with standard tolerance and iteration cap."""
        ...

    @staticmethod
    def high_precision() -> SabrCalibrator:
        """Return a calibrator with tighter tolerance for production fits.

        Returns
        -------
        SabrCalibrator
            Calibrator with high-precision tolerance.
        """
        ...

    def with_tolerance(self, tolerance: float) -> SabrCalibrator:
        """Return a copy with an overridden convergence tolerance.

        Parameters
        ----------
        tolerance : float
            Relative RMSE target for the fit.

        Returns
        -------
        SabrCalibrator
            New calibrator instance sharing other settings.
        """
        ...

    def calibrate(
        self,
        forward: float,
        strikes: list[float],
        market_vols: list[float],
        t: float,
        beta: float,
    ) -> SabrParameters:
        """Fit ``(alpha, nu, rho)`` to market vols with ``beta`` fixed.

        Parameters
        ----------
        forward : float
            Forward at expiry.
        strikes : list[float]
            Strike grid aligned with ``market_vols``.
        market_vols : list[float]
            Market implied vols as decimals.
        t : float
            Expiry in years.
        beta : float
            Fixed SABR beta in ``[0, 1]``.

        Returns
        -------
        SabrParameters
            Calibrated :class:`SabrParameters`.

        Raises
        ------
        ValueError
            If lengths mismatch or fit fails to converge.
        """
        ...

    def calibrate_auto_shift(
        self,
        forward: float,
        strikes: list[float],
        market_vols: list[float],
        t: float,
        beta: float,
    ) -> SabrParameters:
        """Calibrate with automatic shift selection for negative-rate smiles.

        Parameters
        ----------
        forward : float
            Forward at expiry.
        strikes : list[float]
            Strike grid aligned with ``market_vols``.
        market_vols : list[float]
            Market implied vols as decimals.
        t : float
            Expiry in years.
        beta : float
            Fixed SABR beta in ``[0, 1]``.

        Returns
        -------
        SabrParameters
            Calibrated :class:`SabrParameters` with auto-selected shift.

        Raises
        ------
        ValueError
            If lengths mismatch or fit fails to converge.
        """
        ...

# ---------------------------------------------------------------------------
# Fourier option pricing helpers
# ---------------------------------------------------------------------------

def bs_cos_price(
    spot: float,
    strike: float,
    rate: float,
    dividend: float,
    vol: float,
    maturity: float,
    is_call: bool,
    n_terms: int | None = None,
) -> float:
    """Price a European option under Black-Scholes with the COS method.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    rate : float
        Continuously compounded risk-free rate (decimal).
    dividend : float
        Continuous dividend yield (decimal).
    vol : float
        Annualized volatility (decimal).
    maturity : float
        Time to expiry in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    n_terms : int, optional
        Number of COS terms. Uses a default when ``None``.

    Returns
    -------
    float
        Per-unit option price.
    """
    ...

def vg_cos_price(
    spot: float,
    strike: float,
    rate: float,
    dividend: float,
    sigma: float,
    theta: float,
    nu: float,
    maturity: float,
    is_call: bool,
    n_terms: int | None = None,
) -> float:
    """Price a European option under Variance Gamma with the COS method.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    rate : float
        Continuously compounded risk-free rate (decimal).
    dividend : float
        Continuous dividend yield (decimal).
    sigma : float
        VG diffusion parameter (volatility).
    theta : float
        VG drift parameter.
    nu : float
        VG variance rate parameter.
    maturity : float
        Time to expiry in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    n_terms : int, optional
        Number of COS terms. Uses a default when ``None``.

    Returns
    -------
    float
        Per-unit option price.
    """
    ...

def merton_jump_cos_price(
    spot: float,
    strike: float,
    rate: float,
    dividend: float,
    sigma: float,
    mu_jump: float,
    sigma_jump: float,
    lambda_: float,
    maturity: float,
    is_call: bool,
    n_terms: int | None = None,
) -> float:
    """Price a European option under Merton jump-diffusion with the COS method.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    rate : float
        Continuously compounded risk-free rate (decimal).
    dividend : float
        Continuous dividend yield (decimal).
    sigma : float
        Diffusion volatility (decimal).
    mu_jump : float
        Mean of the jump size distribution.
    sigma_jump : float
        Standard deviation of the jump size.
    lambda_ : float
        Jump intensity (expected jumps per year).
    maturity : float
        Time to expiry in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    n_terms : int, optional
        Number of COS terms. Uses a default when ``None``.

    Returns
    -------
    float
        Per-unit option price.
    """
    ...

# ---------------------------------------------------------------------------
# Exotic rate products — deterministic coupon / payoff helpers
# ---------------------------------------------------------------------------

def tarn_coupon_profile(
    fixed_rate: float,
    coupon_floor: float,
    floating_fixings: list[float],
    target_coupon: float,
    day_count_fraction: float,
) -> dict[str, Any]:
    """Simulate a TARN coupon profile along a deterministic rate path.

    Each period coupon is ``max(fixed_rate - L_i, coupon_floor) * dcf``;
    payments accumulate until the cumulative reaches ``target_coupon``, at
    which point the final coupon is capped so the cumulative hits the
    target exactly and the note redeems early.

    Parameters
    ----------
    fixed_rate : float
        Fixed strike rate.
    coupon_floor : float
        Per-period floor on ``fixed_rate - L_i``.
    floating_fixings : list[float]
        Floating rate fixings (one per period).
    target_coupon : float
        Cumulative target that triggers knockout (> 0).
    day_count_fraction : float
        Year fraction applied to each period coupon.

    Returns
    -------
    dict[str, Any]
        Dict with keys ``coupons_paid`` (list[float]), ``cumulative``
        (list[float]), ``redemption_index`` (int | None) and
        ``redeemed_early`` (bool).
    """
    ...

def snowball_coupon_profile(
    initial_coupon: float,
    fixed_rate: float,
    floating_fixings: list[float],
    floor: float,
    cap: float,
) -> list[float]:
    """Compute a snowball coupon schedule.

    Snowball: ``c_i = clip(c_{i-1} + fixed_rate - L_i, floor, cap)``
    with ``c_0 = initial_coupon``.

    Pass ``float('inf')`` as ``cap`` for an uncapped coupon.

    Parameters
    ----------
    initial_coupon : float
        First-period coupon for snowball mode.
    fixed_rate : float
        Fixed strike rate.
    floating_fixings : list[float]
        Floating rate fixings (one per period).
    floor : float
        Per-period coupon floor.
    cap : float
        Per-period coupon cap (use ``float('inf')`` for uncapped).
    is_inverse_floater : bool
        ``True`` for inverse floater mode, ``False`` for snowball.
    leverage : float, default 1.0
        Leverage multiplier for inverse floater mode.

    Returns
    -------
    list[float]
        Coupon schedule, one per period.
    """
    ...

def inverse_floater_coupon_profile(
    fixed_rate: float,
    floating_fixings: list[float],
    floor: float,
    cap: float,
    leverage: float,
) -> list[float]:
    """Compute a path-independent inverse-floater coupon schedule."""
    ...

def cms_spread_option_intrinsic(
    long_cms: float,
    short_cms: float,
    strike: float,
    is_call: bool,
    notional: float,
) -> float:
    """Undiscounted intrinsic payoff of a CMS spread option.

    Call: ``notional * max(long_cms - short_cms - strike, 0)``.
    Put: ``notional * max(strike - (long_cms - short_cms), 0)``.

    Ignores CMS convexity, vol smile, and correlation adjustments — the
    full product pricer applies those on top of a copula model with
    SABR marginals.

    Parameters
    ----------
    long_cms : float
        Long CMS rate.
    short_cms : float
        Short CMS rate.
    strike : float
        Spread strike.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    notional : float
        Notional amount.

    Returns
    -------
    float
        Undiscounted intrinsic payoff.
    """
    ...

def callable_range_accrual_accrued(
    lower: float,
    upper: float,
    observations: list[float],
    coupon_rate: float,
    day_count_fraction: float,
) -> float:
    """Accrued coupon over a range-accrual period.

    Counts the fraction of ``observations`` within the inclusive interval
    ``[lower, upper]`` and returns
    ``coupon_rate * day_count_fraction * fraction``.

    The call provision is not applied here — this is the coupon that
    would accrue assuming the note is not called before period end.

    Parameters
    ----------
    lower : float
        Lower bound of the accrual range.
    upper : float
        Upper bound of the accrual range.
    observations : list[float]
        Observed values (one per day in the period).
    coupon_rate : float
        Coupon rate (decimal).
    day_count_fraction : float
        Year fraction for the period.

    Returns
    -------
    float
        Accrued coupon amount.
    """
    ...
