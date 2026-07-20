"""
Python bindings for the corresponding finstack-quant Rust API.

Examples
--------
>>> import finstack_quant.valuations.instruments as instruments
>>> instruments.__name__
'finstack_quant.valuations.instruments'
"""

from __future__ import annotations

from finstack_quant.core.market_data import MarketContext

__all__ = [
    "bond_from_cashflows_json",
    "instrument_cashflows_json",
    "list_models",
    "list_models_grouped",
    "list_standard_metrics",
    "list_standard_metrics_grouped",
    "price_instrument",
    "price_instrument_with_metrics",
    "structured_credit_tranche_breakeven_cdr",
    "structured_credit_tranche_discount_margin",
    "structured_credit_tranche_metrics",
    "structured_credit_tranche_oas",
    "structured_credit_tranche_scenario_table",
    "validate_instrument_json",
]

def bond_from_cashflows_json(
    instrument_id: str,
    schedule_json: str,
    discount_curve_id: str,
    quoted_clean: float | None = None,
) -> str:
    """
    Construct tagged bond instrument JSON from a cashflow schedule.

    Parameters
    ----------
    instrument_id : str
        Identifier for the bond instrument.
    schedule_json : str
        JSON-encoded ``CashFlowSchedule``.
    discount_curve_id : str
        Discount curve ID required for pricing.
    quoted_clean : float, optional
        Clean quoted price as a percent of par.

    Returns
    -------
    str
        JSON-encoded tagged ``InstrumentJson::Bond``.

    Raises
    ------
    ValueError
        If the schedule is invalid or bond construction fails.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import bond_from_cashflows_json
    >>> callable(bond_from_cashflows_json)
    True
    """
    ...

def validate_instrument_json(json: str) -> str:
    """
    Validate tagged instrument JSON and return canonical JSON.

    Parameters
    ----------
    json : str
        JSON string for a tagged valuation instrument.

    Returns
    -------
    str
        Canonical pretty-printed instrument JSON after Rust serde validation.

    Raises
    ------
    ValueError
        If the JSON is malformed, has an unknown instrument tag, or
        fails instrument-specific validation.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import validate_instrument_json
    >>> callable(validate_instrument_json)
    True
    """
    ...

def price_instrument(
    instrument_json: str,
    market: MarketContext | str,
    as_of: str,
    model: str = "default",
) -> str:
    """
    Price one instrument and return a ``ValuationResult`` JSON string.

    Parameters
    ----------
    instrument_json : str
        Tagged instrument JSON accepted by
        :func:`validate_instrument_json`.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON.
    as_of : str
        ISO 8601 valuation date.
    model : str, default "default"
        Pricing model selector. Common values include ``"default"``,
        ``"discounting"``, ``"hazard_rate"``, and option-model keys such
        as ``"black76"`` where supported by the instrument.

    Returns
    -------
    str
        JSON-serialized valuation result containing value, currency, metrics,
        and covenant flags when applicable.

    Raises
    ------
    ValueError
        If any input JSON is malformed, required market data is
        missing, or the selected model is unsupported for the instrument.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import price_instrument
    >>> callable(price_instrument)
    True
    """
    ...

def price_instrument_with_metrics(
    instrument_json: str,
    market: MarketContext | str,
    as_of: str,
    model: str = "default",
    metrics: list[str] = [],
    pricing_options: str | None = None,
    market_history: str | None = None,
) -> str:
    """
    Price one instrument and compute explicit risk metric requests.

    Parameters
    ----------
    instrument_json : str
        Tagged instrument JSON.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON.
    as_of : str
        ISO 8601 valuation date.
    model : str, default "default"
        Pricing model selector.
    metrics : list[str], default []
        Metric IDs to compute, such as ``"ytm"``, ``"dv01"``,
        ``"modified_duration"``, ``"hvar"``, or ``"expected_shortfall"``
        when supported by the instrument.
    pricing_options : str, optional
        Optional JSON string for metric pricing overrides.
    market_history : str, optional
        Optional JSON market-history payload required by
        historical risk metrics.

    Returns
    -------
    str
        JSON-serialized valuation result including requested metric values.

    Raises
    ------
    ValueError
        If a metric is unknown, not applicable, or cannot be
        calculated from the supplied market and history inputs.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import price_instrument_with_metrics
    >>> callable(price_instrument_with_metrics)
    True
    """
    ...

def instrument_cashflows_json(
    instrument_json: str,
    market: MarketContext | str,
    as_of: str,
    model: str,
) -> str:
    """
    Per-flow cashflow envelope for a discountable instrument.

    Parameters
    ----------
    instrument_json : str
        Tagged instrument JSON.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON.
    as_of : str
        ISO 8601 valuation date.
    model : str
        ``"discounting"`` or ``"hazard_rate"``.

    Returns
    -------
    str
        JSON-serialized ``InstrumentCashflowEnvelope``.

    Raises
    ------
    ValueError
        If the model is unsupported, the instrument is unsupported
        for cashflow export, or required market data is missing.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import instrument_cashflows_json
    >>> callable(instrument_cashflows_json)
    True
    """
    ...

def list_models() -> list[str]:
    """
    Return every pricing model key registered in the standard pricer registry.

    The list is registry-derived rather than enum-derived, so it reflects real
    dispatch coverage: a model with no registered pricer is omitted. The names
    are the canonical keys accepted by the ``model`` argument of
    :func:`price_instrument`.

    Returns
    -------
    list[str]
        Canonical model keys such as ``"discounting"`` or ``"black76"``,
        deduplicated and sorted.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import list_models
    >>> callable(list_models)
    True
    """
    ...

def list_models_grouped() -> dict[str, list[str]]:
    """
    Return the standard registry's pricing models grouped by instrument type.

    Only instrument types with at least one registered pricer appear as keys,
    and each entry lists only the models that can price that instrument.

    Returns
    -------
    dict[str, list[str]]
        Mapping from canonical instrument-type name to its sorted model keys.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import list_models_grouped
    >>> callable(list_models_grouped)
    True
    """
    ...

def list_standard_metrics() -> list[str]:
    """
    Return all standard metric IDs registered by the Rust valuation engine.

    Returns
    -------
    list[str]
        Sorted list of fully qualified metric keys.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import list_standard_metrics
    >>> callable(list_standard_metrics)
    True
    """
    ...

def list_standard_metrics_grouped() -> dict[str, list[str]]:
    """
    Return standard metric IDs grouped by human-readable category.

    Returns
    -------
    dict[str, list[str]]
        Mapping from group label to sorted metric ID lists.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import list_standard_metrics_grouped
    >>> callable(list_standard_metrics_grouped)
    True
    """
    ...

def structured_credit_tranche_discount_margin(
    instrument_json: str,
    tranche_id: str,
    market: MarketContext,
    as_of: str,
    target_pv: float,
) -> float:
    """Solve the discount margin that prices a floating-rate tranche at a target PV.

    Returns the margin in decimal (0.015 = 150 bp).

    Parameters
    ----------
    instrument_json : str
        Tagged JSON for a ``StructuredCredit`` deal.
    tranche_id : str
        Identifier of the tranche within the deal.
    market : MarketContext
        Market context supplying curves and fixings.
    as_of : str
        ISO 8601 valuation date.
    target_pv : float
        Target present value, in the tranche's own currency.

    Returns
    -------
    float
        Discount margin in decimal (0.015 = 150 bp).

    Raises
    ------
    ValueError
        If the instrument JSON is malformed, the deal fails validation, the
        tranche id is not part of the deal, or required market data is missing.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import structured_credit_tranche_discount_margin
    >>> callable(structured_credit_tranche_discount_margin)
    True
    """
    ...

def structured_credit_tranche_breakeven_cdr(
    instrument_json: str,
    tranche_id: str,
    market: MarketContext,
    as_of: str,
) -> float:
    """Solve the constant default rate at which a tranche first takes a writedown.

    Parameters
    ----------
    instrument_json : str
        Tagged JSON for a ``StructuredCredit`` deal.
    tranche_id : str
        Identifier of the tranche within the deal.
    market : MarketContext
        Market context supplying curves and fixings.
    as_of : str
        ISO 8601 valuation date.

    Returns
    -------
    float
        Break-even annual CDR in decimal.

    Raises
    ------
    ValueError
        If the instrument JSON is malformed, the deal fails validation, the
        tranche id is not part of the deal, or required market data is missing.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import structured_credit_tranche_breakeven_cdr
    >>> callable(structured_credit_tranche_breakeven_cdr)
    True
    """
    ...

def structured_credit_tranche_oas(
    instrument_json: str,
    tranche_id: str,
    market_price_pct: float,
    market: MarketContext,
    as_of: str,
    config_json: str | None = None,
) -> str:
    """Compute option-adjusted spread for a tranche. Returns JSON ``OasResult``.

    Parameters
    ----------
    instrument_json : str
        Tagged JSON for a ``StructuredCredit`` deal.
    tranche_id : str
        Identifier of the tranche within the deal.
    market_price_pct : float
        Market price as a percentage of original balance (100.0 = par).
    market : MarketContext
        Market context supplying curves and fixings.
    as_of : str
        ISO 8601 valuation date.
    config_json : str or None, optional
        Serialized ``OasConfig``. All fields are required when supplied.

    Returns
    -------
    str
        JSON-serialized ``OasResult``.

    Raises
    ------
    ValueError
        If the instrument JSON is malformed, the deal fails validation, the
        tranche id is not part of the deal, or required market data is missing.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import structured_credit_tranche_oas
    >>> callable(structured_credit_tranche_oas)
    True
    """
    ...

def structured_credit_tranche_metrics(
    instrument_json: str,
    tranche_id: str,
    market: MarketContext,
    as_of: str,
    market_price_pct: float | None = None,
) -> str:
    """Summary risk/pricing metrics for a tranche. Returns JSON ``TrancheMetrics``.

    Parameters
    ----------
    instrument_json : str
        Tagged JSON for a ``StructuredCredit`` deal.
    tranche_id : str
        Identifier of the tranche within the deal.
    market : MarketContext
        Market context supplying curves and fixings.
    as_of : str
        ISO 8601 valuation date.
    market_price_pct : float or None, optional
        Market price as a percentage of original balance; the model price is
        used when omitted.

    Returns
    -------
    str
        JSON-serialized ``TrancheMetrics``.

    Raises
    ------
    ValueError
        If the instrument JSON is malformed, the deal fails validation, the
        tranche id is not part of the deal, or required market data is missing.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import structured_credit_tranche_metrics
    >>> callable(structured_credit_tranche_metrics)
    True
    """
    ...

def structured_credit_tranche_scenario_table(
    instrument_json: str,
    tranche_id: str,
    market: MarketContext,
    as_of: str,
    grid_json: str,
) -> str:
    """Price a tranche across a CPR x CDR x severity grid. Returns JSON ``ScenarioTable``.

    Parameters
    ----------
    instrument_json : str
        Tagged JSON for a ``StructuredCredit`` deal.
    tranche_id : str
        Identifier of the tranche within the deal.
    market : MarketContext
        Market context supplying curves and fixings.
    as_of : str
        ISO 8601 valuation date.
    grid_json : str
        Serialized ``ScenarioGrid``. Capped at 10,000 cells because each cell
        reprices the entire deal.

    Returns
    -------
    str
        JSON-serialized ``ScenarioTable``.

    Raises
    ------
    ValueError
        If the instrument JSON is malformed, the deal fails validation, the
        tranche id is not part of the deal, or required market data is missing.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import structured_credit_tranche_scenario_table
    >>> callable(structured_credit_tranche_scenario_table)
    True
    """
    ...
