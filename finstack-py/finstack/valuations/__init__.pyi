"""Instrument pricing, risk metrics, P&L attribution, and market-context bootstrapping.

The canonical path to build a :class:`finstack.core.market_data.MarketContext`
from raw market quotes is :func:`calibrate`:

    >>> import json
    >>> from finstack.valuations import calibrate
    >>> envelope = {
    ...     "schema": "finstack.calibration",
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
``finstack/valuations/examples/market_bootstrap/`` in the repository.

Instrument pricing helpers live under :mod:`finstack.valuations.instruments`.
This module exposes calibration, SABR / Black-Scholes primitives, and
credit-factor hierarchy tooling. Portfolio factor sensitivities and risk
decomposition live under :mod:`finstack.portfolio`.
"""

from __future__ import annotations

import pandas as pd

from finstack.core.market_data import MarketContext
from finstack.valuations import correlation as correlation
from finstack.valuations import credit as credit
from finstack.valuations import credit_derivatives as credit_derivatives
from finstack.valuations import exotics as exotics
from finstack.valuations import fx as fx
from finstack.valuations import instruments as instruments
from finstack.valuations.envelope import (
    BaseCorrelationCurvePrior as BaseCorrelationCurvePrior,
    BaseCorrelationStep as BaseCorrelationStep,
    BasisSpreadCurvePrior as BasisSpreadCurvePrior,
    BondCleanPriceDatum as BondCleanPriceDatum,
    BondFixedRateBulletCleanPrice as BondFixedRateBulletCleanPrice,
    BondFixedRateBulletOas as BondFixedRateBulletOas,
    BondFixedRateBulletYtm as BondFixedRateBulletYtm,
    BondFixedRateBulletZSpread as BondFixedRateBulletZSpread,
    BondOasDatum as BondOasDatum,
    BondYtmDatum as BondYtmDatum,
    BondZSpreadDatum as BondZSpreadDatum,
    CalibrationEnvelope as CalibrationEnvelope,
    CalibrationPlan as CalibrationPlan,
    CalibrationStep as CalibrationStep,
    CapFloorHullWhiteStep as CapFloorHullWhiteStep,
    CapFloorVolDatum as CapFloorVolDatum,
    CapFloorVolPayload as CapFloorVolPayload,
    CapFloorVolQuote as CapFloorVolQuote,
    CdsConventionKey as CdsConventionKey,
    CdsParSpread as CdsParSpread,
    CdsParSpreadDatum as CdsParSpreadDatum,
    CdsTrancheDatum as CdsTrancheDatum,
    CdsTrancheQuote as CdsTrancheQuote,
    CdsUpfront as CdsUpfront,
    CdsUpfrontDatum as CdsUpfrontDatum,
    CollateralDatum as CollateralDatum,
    CreditIndexDatum as CreditIndexDatum,
    DatePillar as DatePillar,
    DiscountCurvePrior as DiscountCurvePrior,
    DiscountStep as DiscountStep,
    DividendScheduleDatum as DividendScheduleDatum,
    FixingSeriesDatum as FixingSeriesDatum,
    ForwardCurvePrior as ForwardCurvePrior,
    ForwardStep as ForwardStep,
    FxForwardOutright as FxForwardOutright,
    FxForwardOutrightDatum as FxForwardOutrightDatum,
    FxOptionVanilla as FxOptionVanilla,
    FxOptionVanillaDatum as FxOptionVanillaDatum,
    FxSpotDatum as FxSpotDatum,
    FxSwapOutright as FxSwapOutright,
    FxSwapOutrightDatum as FxSwapOutrightDatum,
    FxVolSurfaceDatum as FxVolSurfaceDatum,
    HazardCurvePrior as HazardCurvePrior,
    HazardStep as HazardStep,
    HullWhiteStep as HullWhiteStep,
    InflationCurvePrior as InflationCurvePrior,
    InflationFixingsDatum as InflationFixingsDatum,
    InflationStep as InflationStep,
    InflationSwapDatum as InflationSwapDatum,
    InflationSwapPayload as InflationSwapPayload,
    InflationSwapQuote as InflationSwapQuote,
    MarketDatum as MarketDatum,
    MarketQuote as MarketQuote,
    OptionVolDatum as OptionVolDatum,
    OptionVolPayload as OptionVolPayload,
    OptionVolQuote as OptionVolQuote,
    ParametricCurvePrior as ParametricCurvePrior,
    ParametricStep as ParametricStep,
    Pillar as Pillar,
    PriceCurvePrior as PriceCurvePrior,
    PriceDatum as PriceDatum,
    PriorMarketObject as PriorMarketObject,
    RateDeposit as RateDeposit,
    RateFra as RateFra,
    RateFutures as RateFutures,
    RateQuoteDepositDatum as RateQuoteDepositDatum,
    RateQuoteFraDatum as RateQuoteFraDatum,
    RateQuoteFuturesDatum as RateQuoteFuturesDatum,
    RateQuoteSwapDatum as RateQuoteSwapDatum,
    RateSwap as RateSwap,
    StudentTStep as StudentTStep,
    SviSurfaceStep as SviSurfaceStep,
    SwaptionVolDatum as SwaptionVolDatum,
    SwaptionVolPayload as SwaptionVolPayload,
    SwaptionVolQuote as SwaptionVolQuote,
    SwaptionVolStep as SwaptionVolStep,
    Tenor as Tenor,
    TenorPillar as TenorPillar,
    VolCubeDatum as VolCubeDatum,
    VolSurfacePrior as VolSurfacePrior,
    VolSurfaceStep as VolSurfaceStep,
    VolatilityIndexCurvePrior as VolatilityIndexCurvePrior,
    XccyBasisStep as XccyBasisStep,
    XccyBasisSwapDatum as XccyBasisSwapDatum,
    XccyBasisSwapQuote as XccyBasisSwapQuote,
    YoyInflationSwapDatum as YoyInflationSwapDatum,
    YoyInflationSwapPayload as YoyInflationSwapPayload,
    YoyInflationSwapQuote as YoyInflationSwapQuote,
)

__all__ = [
    "correlation",
    "credit",
    "credit_derivatives",
    "exotics",
    "fx",
    "instruments",
    "ValuationResult",
    "BaseCorrelationCurvePrior",
    "BaseCorrelationStep",
    "BasisSpreadCurvePrior",
    "BondCleanPriceDatum",
    "BondFixedRateBulletCleanPrice",
    "BondFixedRateBulletOas",
    "BondFixedRateBulletYtm",
    "BondFixedRateBulletZSpread",
    "BondOasDatum",
    "BondYtmDatum",
    "BondZSpreadDatum",
    "CalibrationEnvelope",
    "CalibrationEnvelopeError",
    "CalibrationPlan",
    "CalibrationResult",
    "CalibrationStep",
    "CapFloorHullWhiteStep",
    "CapFloorVolDatum",
    "CapFloorVolPayload",
    "CapFloorVolQuote",
    "CdsConventionKey",
    "CdsParSpread",
    "CdsParSpreadDatum",
    "CdsTrancheDatum",
    "CdsTrancheQuote",
    "CdsUpfront",
    "CdsUpfrontDatum",
    "CollateralDatum",
    "CreditIndexDatum",
    "DatePillar",
    "DiscountCurvePrior",
    "DiscountStep",
    "DividendScheduleDatum",
    "FixingSeriesDatum",
    "ForwardCurvePrior",
    "ForwardStep",
    "FxForwardOutright",
    "FxForwardOutrightDatum",
    "FxOptionVanilla",
    "FxOptionVanillaDatum",
    "FxSpotDatum",
    "FxSwapOutright",
    "FxSwapOutrightDatum",
    "FxVolSurfaceDatum",
    "HazardCurvePrior",
    "HazardStep",
    "HullWhiteStep",
    "InflationCurvePrior",
    "InflationFixingsDatum",
    "InflationStep",
    "InflationSwapDatum",
    "InflationSwapPayload",
    "InflationSwapQuote",
    "MarketDatum",
    "MarketQuote",
    "OptionVolDatum",
    "OptionVolPayload",
    "OptionVolQuote",
    "ParametricCurvePrior",
    "ParametricStep",
    "Pillar",
    "PriceCurvePrior",
    "PriceDatum",
    "PriorMarketObject",
    "RateDeposit",
    "RateFra",
    "RateFutures",
    "RateQuoteDepositDatum",
    "RateQuoteFraDatum",
    "RateQuoteFuturesDatum",
    "RateQuoteSwapDatum",
    "RateSwap",
    "StudentTStep",
    "SviSurfaceStep",
    "SwaptionVolDatum",
    "SwaptionVolPayload",
    "SwaptionVolQuote",
    "SwaptionVolStep",
    "Tenor",
    "TenorPillar",
    "VolCubeDatum",
    "VolSurfacePrior",
    "VolSurfaceStep",
    "VolatilityIndexCurvePrior",
    "XccyBasisStep",
    "XccyBasisSwapDatum",
    "XccyBasisSwapQuote",
    "YoyInflationSwapDatum",
    "YoyInflationSwapPayload",
    "YoyInflationSwapQuote",
    "validate_calibration_json",
    "calibrate",
    "dry_run",
    "dependency_graph_json",
    "bs_cos_price",
    "vg_cos_price",
    "merton_jump_cos_price",
    "tarn_coupon_profile",
    "snowball_coupon_profile",
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
    "instrument_cashflows_json",
]

class ValuationResult:
    """Valuation envelope: PV, currency, risk metrics, covenant flags, and JSON round-trip.

    Instantiate via :meth:`from_json` or the ``price_*`` helpers that emit JSON.

    Args:
        None (use ``from_json``).

    Returns:
        A ``ValuationResult`` instance (type description only).

    Example:
        >>> from finstack.valuations import ValuationResult
        >>> ValuationResult.from_json(result_json)  # doctest: +SKIP
    """

    @staticmethod
    def from_json(json: str) -> ValuationResult:
        """Deserialize a ``ValuationResult`` from JSON.

        Args:
            json: JSON string produced by the pricing pipeline or ``to_json``.

        Returns:
            Parsed ``ValuationResult`` instance.

        Example:
            >>> from finstack.valuations import ValuationResult
            >>> ValuationResult.from_json('{"instrument_id":"x","value":{}}')  # doctest: +SKIP
        """
        ...

    def to_json(self) -> str:
        """Serialize this result to pretty-printed JSON.

        Args:
            (none)

        Returns:
            Pretty-printed JSON string.

        Example:
            >>> ValuationResult.from_json(
            ...     '{"instrument_id":"i","value":{"amount":1.0,"currency":"USD"},"measures":{}}'
            ... ).to_json()  # doctest: +SKIP
            ''
        """
        ...

    @property
    def instrument_id(self) -> str:
        """Instrument identifier assigned by the pricer.

        Args:
            None (read-only property).

        Returns:
            Instrument ID string.

        Example:
            >>> vr = ValuationResult.from_json("{}")  # doctest: +SKIP
            >>> vr.instrument_id  # doctest: +SKIP
            ''
        """
        ...

    @property
    def price(self) -> float:
        """Present value amount (NPV).

        Args:
            None (read-only property).

        Returns:
            PV amount as a float.

        Example:
            >>> vr = ValuationResult.from_json("{}")  # doctest: +SKIP
            >>> vr.price  # doctest: +SKIP
            0.0
        """
        ...

    @property
    def currency(self) -> str:
        """Currency code for the present value.

        Args:
            None (read-only property).

        Returns:
            Currency code string.

        Example:
            >>> vr = ValuationResult.from_json("{}")  # doctest: +SKIP
            >>> vr.currency  # doctest: +SKIP
            'USD'
        """
        ...

    def get_metric(self, key: str) -> float | None:
        """Return a scalar risk measure by string key.

        Args:
            key: Metric identifier (e.g. ``"ytm"``, ``"dv01"``).

        Returns:
            Metric value, or ``None`` if missing.

        Example:
            >>> vr = ValuationResult.from_json(
            ...     '{"instrument_id":"i","value":{"amount":1,"currency":"USD"},"measures":{}}'
            ... )  # doctest: +SKIP
            >>> vr.get_metric("ytm")  # doctest: +SKIP
        """
        ...

    def metric_keys(self) -> list[str]:
        """List metric keys present on this result.

        Args:
            (none)

        Returns:
            All measure keys as strings.

        Example:
            >>> ValuationResult.from_json(
            ...     '{"instrument_id":"i","value":{"amount":1,"currency":"USD"},"measures":{}}'
            ... ).metric_keys()  # doctest: +SKIP
            []
        """
        ...

    def metric_count(self) -> int:
        """Count of measures stored on this result.

        Args:
            (none)

        Returns:
            Number of entries in the measures map.

        Example:
            >>> ValuationResult.from_json(
            ...     '{"instrument_id":"i","value":{"amount":1,"currency":"USD"},"measures":{}}'
            ... ).metric_count()  # doctest: +SKIP
            0
        """
        ...

    def all_covenants_passed(self) -> bool:
        """Whether every covenant passed (or none were evaluated).

        Args:
            (none)

        Returns:
            ``True`` if no covenant failures are recorded.

        Example:
            >>> ValuationResult.from_json(
            ...     '{"instrument_id":"i","value":{"amount":1,"currency":"USD"},"measures":{}}'
            ... ).all_covenants_passed()  # doctest: +SKIP
            True
        """
        ...

    def failed_covenants(self) -> list[str]:
        """Covenant IDs that failed, if any.

        Args:
            (none)

        Returns:
            List of failed covenant identifiers.

        Example:
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

        Returns:
            Single-row DataFrame.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug string for this result.

        Args:
            None (uses ``self``).

        Returns:
            ``ValuationResult(id=..., price=..., currency=..., metrics=...)`` text.

        Example:
            >>> repr(ValuationResult.from_json("{}"))  # doctest: +SKIP
            ''
        """
        ...

def instrument_cashflows_json(
    instrument_json: str,
    market: MarketContext | str,
    as_of: str,
    model: str = "discounting",
) -> str:
    """Per-flow cashflow envelope (DF / survival / PV) for a discountable instrument.

    Supports ``model in {"discounting", "hazard_rate"}``. The envelope's
    ``total_pv`` reconciles with the instrument's ``base_value`` for the
    supported model-instrument pairs.

    Args:
        instrument_json: Tagged instrument JSON.
        market: ``MarketContext`` instance or JSON string.
        as_of: ISO 8601 valuation date.
        model: ``"discounting"`` (DF only) or ``"hazard_rate"`` (adds
            survival probability, conditional default probability, and
            recovery-adjusted principal PV).

    Returns:
        JSON-serialized ``InstrumentCashflowEnvelope``.

    Raises:
        ValueError: If ``model`` is unsupported or the instrument type isn't
            priced under that model.
    """
    ...

def instrument_cashflows(
    instrument_json: str,
    market: MarketContext | str,
    as_of: str,
    *,
    model: str = "discounting",
) -> tuple[dict, pd.DataFrame]:
    """DataFrame-friendly wrapper around :func:`instrument_cashflows_json`.

    Parses the JSON envelope returned by the low-level binding and constructs
    a per-flow ``pandas.DataFrame`` with ``date`` / ``reset_date`` parsed as
    ``datetime64``. See :func:`instrument_cashflows_json` for argument and
    error semantics.

    Returns:
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

    Example:
        >>> import json
        >>> from finstack.valuations import calibrate
        >>> result = calibrate(json.dumps(plan))  # doctest: +SKIP
        >>> result.success  # doctest: +SKIP
        True
    """

    @staticmethod
    def from_json(json: str) -> CalibrationResult:
        """Deserialize a ``CalibrationResult`` from JSON.

        Args:
            json: JSON string (a ``CalibrationResultEnvelope``).

        Returns:
            Parsed ``CalibrationResult`` instance.
        """
        ...

    def to_json(self) -> str:
        """Serialize to pretty-printed JSON.

        Returns:
            Pretty-printed JSON string.
        """
        ...

    @property
    def success(self) -> bool:
        """Whether the overall calibration succeeded (all steps passed)."""
        ...

    @property
    def market(self) -> MarketContext:
        """The calibrated ``MarketContext`` containing all produced curves."""
        ...

    @property
    def market_json(self) -> str:
        """The calibrated market serialized as a JSON string."""
        ...

    @property
    def report_json(self) -> str:
        """The aggregated calibration report as a JSON string."""
        ...

    @property
    def step_ids(self) -> list[str]:
        """List of step identifiers that were executed."""
        ...

    @property
    def iterations(self) -> int:
        """Total solver iterations across all steps."""
        ...

    @property
    def max_residual(self) -> float:
        """Maximum absolute residual across all steps."""
        ...

    @property
    def rmse(self) -> float:
        """Root mean square error across all steps."""
        ...

    def step_report_json(self, step_id: str) -> str:
        """Per-step calibration report as a JSON string.

        Args:
            step_id: Identifier of the calibration step.

        Returns:
            JSON-serialized calibration report for the step.

        Raises:
            ValueError: If no step with the given *step_id* exists.
        """
        ...

    def report_to_dataframe(self) -> pd.DataFrame:
        """Per-step summary as a pandas DataFrame.

        Columns: ``step_id``, ``success``, ``iterations``, ``max_residual``,
        ``rmse``, ``convergence_reason``.

        Returns:
            DataFrame with one row per calibration step.
        """
        ...

    def __repr__(self) -> str: ...

class CalibrationEnvelopeError(RuntimeError):
    """Raised when a calibration envelope fails validation or solving.

    Inherits from :class:`RuntimeError`, so existing ``except RuntimeError``
    callers continue to catch it (backward-compatible with pre-Phase-4 code).

    Attributes:
        kind: Snake-case discriminator for the failure category. One of
            ``"json_parse"``, ``"unknown_step_kind"``, ``"missing_dependency"``,
            ``"undefined_quote_set"``, ``"quote_class_mismatch"``,
            ``"solver_not_converged"``, ``"quote_data_invalid"``.
        step_id: Identifier of the offending step, when applicable. ``None``
            for ``"json_parse"``.
        details: JSON-serialized structured payload (see ``EnvelopeError``
            in the Rust crate for the schema).
    """

    kind: str
    step_id: str | None
    details: str

def validate_calibration_json(json: str) -> str:
    """Validate a calibration plan JSON and return canonical pretty-printed form.

    Args:
        json: JSON-serialized ``CalibrationEnvelope``.

    Returns:
        Canonical pretty-printed JSON.

    Raises:
        CalibrationEnvelopeError: If the JSON is not a valid calibration
            envelope. Inherits from :class:`RuntimeError`.

    Example:
        >>> from finstack.valuations import validate_calibration_json
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

    Args:
        json: JSON-serialized ``CalibrationEnvelope``.

    Returns:
        Pretty-printed JSON ``ValidationReport``. Inspect ``report["errors"]``
        for any structural problems and ``report["dependency_graph"]`` for the
        step DAG.

    Raises:
        CalibrationEnvelopeError: If the envelope JSON is malformed.

    Example:
        >>> import json as _json
        >>> from finstack.valuations import dry_run
        >>> report = _json.loads(dry_run(_json.dumps(envelope)))  # doctest: +SKIP
        >>> for err in report["errors"]:  # doctest: +SKIP
        ...     print(err["kind"], err.get("step_id"))
    """
    ...

def dependency_graph_json(json: str) -> str:
    """Dump the static dependency graph of a calibration plan as JSON.

    Args:
        json: JSON-serialized ``CalibrationEnvelope``.

    Returns:
        Pretty-printed JSON ``DependencyGraph`` with ``initial_ids`` (curve
        IDs available at execution start, sourced from ``prior_market``)
        and ``nodes`` (per-step ``reads``/``writes`` in declared order).

    Raises:
        CalibrationEnvelopeError: If the envelope JSON is malformed.
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

    Args:
        json: JSON-serialized ``CalibrationEnvelope`` (schema string is
            ``"finstack.calibration"``).

    Returns:
        :class:`CalibrationResult` with:
            - ``.market`` — the live :class:`MarketContext` (use this for
              pricing, attribution, scenarios, portfolio).
            - ``.market_json`` — same context as a JSON snapshot for
              persistence or comparison.
            - ``.report_json`` / ``.step_report_json(id)`` /
              ``.report_to_dataframe()`` — diagnostics. Always check
              ``.success`` and ``.rmse`` before relying on the produced market.
            - ``.iterations``, ``.max_residual``, ``.step_ids`` — summary stats.

    Raises:
        CalibrationEnvelopeError: If the JSON is malformed or calibration
            fails (e.g., missing dependency, solver non-convergence). The
            exception carries ``kind``, ``step_id``, and ``details``
            attributes for programmatic handling. Inherits from
            :class:`RuntimeError` so legacy ``except RuntimeError`` handlers
            continue to catch it.

    Example:
        >>> import json as _json
        >>> from finstack.valuations import calibrate
        >>> result = calibrate(_json.dumps(envelope))  # doctest: +SKIP
        >>> assert result.success and result.rmse < 1e-6  # doctest: +SKIP
        >>> curve = result.market.get_discount("USD-OIS")  # doctest: +SKIP
        >>> from finstack.valuations.instruments import price_instrument
        >>> price_json = price_instrument(inst_json, result.market_json, "2026-05-08")  # doctest: +SKIP

    See Also:
        - ``finstack/valuations/examples/market_bootstrap/`` — reference
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
    """Solve for Black-Scholes implied volatility given a target price."""
    ...

def black76_implied_vol(
    forward: float,
    strike: float,
    df: float,
    t: float,
    price: float,
    is_call: bool,
) -> float:
    """Solve for Black-76 (forward-based) implied volatility given a target price."""
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
    """Arithmetic (Turnbull-Wakeman) or geometric (Kemna-Vorst) Asian option price."""
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
    """Quanto option (FX-adjusted cross-currency) price in domestic currency."""
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
        """Equity-standard defaults ``(alpha=0.20, beta=1.0, nu=0.30, rho=-0.20)``."""
        ...

    @staticmethod
    def rates_default() -> SabrParameters:
        """Rates-standard defaults ``(alpha=0.02, beta=0.5, nu=0.30, rho=0.0)``."""
        ...

    @property
    def alpha(self) -> float: ...
    @property
    def beta(self) -> float: ...
    @property
    def nu(self) -> float: ...
    @property
    def rho(self) -> float: ...
    @property
    def shift(self) -> float | None: ...
    def is_shifted(self) -> bool:
        """``True`` when parameters include a non-zero shift (negative-rate support)."""
        ...

class SabrModel:
    """Hagan-2002 SABR volatility model."""

    def __init__(self, params: SabrParameters) -> None: ...
    def implied_vol(self, forward: float, strike: float, t: float) -> float:
        """Black-style implied volatility under the Hagan-2002 expansion."""
        ...

    @property
    def params(self) -> SabrParameters: ...
    def supports_negative_rates(self) -> bool: ...

class SabrSmile:
    """Volatility smile generator for a fixed ``(forward, t)`` pair."""

    def __init__(
        self,
        params: SabrParameters,
        forward: float,
        t: float,
    ) -> None: ...
    def atm_vol(self) -> float: ...
    def implied_vol(self, strike: float) -> float: ...
    def generate_smile(self, strikes: list[float]) -> list[float]: ...
    def arbitrage_diagnostics(
        self,
        strikes: list[float],
        r: float = 0.0,
        q: float = 0.0,
    ) -> dict:
        """Butterfly + monotonicity arbitrage diagnostics on ``strikes``.

        Returns a dict with ``arbitrage_free``, ``butterfly_violations``,
        and ``monotonicity_violations``.
        """
        ...

class SabrCalibrator:
    """SABR calibrator (Levenberg-Marquardt with beta fixed)."""

    def __init__(self) -> None: ...
    @staticmethod
    def high_precision() -> SabrCalibrator:
        """Tighter tolerance and higher iteration cap for production fits."""
        ...

    def with_tolerance(self, tolerance: float) -> SabrCalibrator: ...
    def calibrate(
        self,
        forward: float,
        strikes: list[float],
        market_vols: list[float],
        t: float,
        beta: float = 1.0,
    ) -> SabrParameters:
        """Fit ``(alpha, nu, rho)`` to market vols with ``beta`` fixed."""
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
    n_terms: int = 128,
) -> float:
    """Price a European option under Black-Scholes with the COS method."""
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
    n_terms: int = 128,
) -> float:
    """Price a European option under Variance Gamma with the COS method."""
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
    n_terms: int = 128,
) -> float:
    """Price a European option under Merton jump-diffusion with the COS method."""
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
) -> dict:
    """Simulate a TARN coupon profile along a deterministic rate path.

    Each period coupon is ``max(fixed_rate - L_i, coupon_floor) * dcf``;
    payments accumulate until the cumulative reaches ``target_coupon``, at
    which point the final coupon is capped so the cumulative hits the
    target exactly and the note redeems early.

    Args:
        fixed_rate: Fixed strike rate.
        coupon_floor: Per-period floor on ``fixed_rate - L_i``.
        floating_fixings: Floating rate fixings (one per period).
        target_coupon: Cumulative target that triggers knockout (> 0).
        day_count_fraction: Year fraction applied to each period coupon.

    Returns:
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
    is_inverse_floater: bool,
    leverage: float = 1.0,
) -> list[float]:
    """Compute a snowball or inverse-floater coupon schedule.

    Snowball: ``c_i = clip(c_{i-1} + fixed_rate - L_i, floor, cap)``
    with ``c_0 = initial_coupon``.

    Inverse floater: ``c_i = clip(fixed_rate - leverage * L_i, floor, cap)``
    (``initial_coupon`` ignored).

    Pass ``float('inf')`` as ``cap`` for an uncapped coupon.
    """
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
    """
    ...
