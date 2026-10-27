"""Stub declarations mirroring ``finstack-quant-py/src/bindings/attribution/entry.rs``."""

from __future__ import annotations

from collections.abc import Mapping
import datetime
from typing import Any
import pandas as pd
from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.attribution._pnl_attribution import PnlAttribution
from finstack_quant.attribution._result_envelope import AttributionResultEnvelope
from finstack_quant.attribution._return_contribution import ReturnContributionResult

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
    credit_factor_detail_options: Mapping[str, bool] | str | None = None,
) -> PnlAttribution:
    """
    Run P&L attribution for a single instrument.

    This is the main entry point. Accepts the instrument, two market
    snapshots, valuation dates, and a method descriptor — typed objects or
    their canonical JSON — and returns the typed attribution result. Use
    :meth:`AttributionResultEnvelope.to_json` when you want the raw JSON envelope
    round-trip instead, :func:`attribute_pnl_many` for a book.

    Parameters
    ----------
    instrument : Bond | TermLoan | InterestRateSwap | Swaption | CapFloor | CreditDefaultSwap | CdsIndex | CdsTranche | FxForward | FxOption | ConvertibleBond | EquityOption | StructuredCredit | CompositeInstrument | str
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
    credit_factor_detail_options : Mapping[str, bool] or str or None
        ``CreditFactorDetailOptions`` (``include_per_issuer_adder``,
        ``include_per_bucket_breakdown``) for that detail; omitted fields
        keep their defaults (per-issuer adder off, per-bucket breakdown on).

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
        date is malformed, the FX provider cannot supply a coherent market
        snapshot, or attribution validation, pricing, or result serialization fails.
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
    credit_factor_detail_options: Mapping[str, bool] | str | None = None,
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
    credit_factor_detail_options : Mapping[str, bool] or str or None
        ``CreditFactorDetailOptions`` for that detail; omitted fields keep
        their defaults (per-issuer adder off, per-bucket breakdown on).

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
        validation / pricing, the FX provider cannot supply a coherent market
        snapshot, or a result mixes currencies across factors.
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

def attribute_pnl_envelope(spec_json: str) -> AttributionResultEnvelope:
    """
    Run attribution from a full JSON ``AttributionEnvelope``.

    Returns the typed result envelope; call its :meth:`to_json` for canonical
    JSON. Corresponds to WASM ``attributePnlEnvelope``. Most users should prefer
    :func:`attribute_pnl`.

    Parameters
    ----------
    spec_json : str
        JSON-serialized ``AttributionEnvelope`` (schema
        ``finstack_quant.attribution/1``).

    Returns
    -------
    AttributionResultEnvelope
        The attribution plus its result-policy audit stamp.

    Examples
    --------
    >>> from finstack_quant.attribution import attribute_pnl_envelope
    >>> try:
    ...     attribute_pnl_envelope("{}")
    ... except ValueError as exc:
    ...     "missing field" in str(exc)
    True

    Raises
    ------
    ValueError
        If ``spec_json`` is malformed or violates the exact attribution
        envelope schema, or attribution validation or pricing fails.
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
        Attribution date label, set as the spec's ``as_of`` field for every
        form. Required with a ``DataFrame`` spec (which has no other place
        for it).
    weighting : str, optional
        ``"gross"`` or ``"net_market_value"`` for market-value positions, set
        as the spec's ``weighting`` field for every form; when neither the
        keyword nor the spec gives it, the wire default ``"gross"`` applies.
    factors : list[dict[str, Any]], optional
        Factor rows ``{"factor", "exposure", "factor_return"}``, set as the
        spec's ``factors`` field for every form.

    A keyword that duplicates a field the dict or JSON spec already carries
    raises ``ValueError``; pass each field once.

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
    >>> long_short = {
    ...     "as_of": "2026-01-02",
    ...     "positions": [
    ...         {"id": "A", "market_value": 100.0, "return": 0.02},
    ...         {"id": "B", "market_value": -60.0, "return": 0.01},
    ...     ],
    ... }
    >>> attribute_return_contribution(long_short, weighting="net_market_value").portfolio_return
    0.035

    Raises
    ------
    ValueError
        If the spec is malformed; ``as_of`` is missing for a DataFrame; a
        keyword duplicates a spec field; ``weighting`` or ``factors`` is
        invalid;
        required identifiers or positions are empty; numeric inputs are
        non-finite, or a derived weight, contribution, or aggregate overflows;
        position weighting modes are mixed or incomplete; factor
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
        empty; numeric inputs are non-finite or derived weights, contributions,
        or aggregates overflow; position weighting modes are
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
        ``bucketed_dv01``, ``bucketed_cs01``, ``vega``, ...) — the tokens accepted by
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
