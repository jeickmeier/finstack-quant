"""Cashflow schedule JSON construction, validation, and bond conversion.

JSON-first bindings for ``finstack-quant-cashflows``. Build schedules from a
``CashflowScheduleBuildSpec``, validate canonical payloads, extract dated flows,
compute accrued interest, or wrap a schedule as a tagged bond instrument for
the valuation engine.
"""

from __future__ import annotations

__all__ = [
    "accrued_interest_json",
    "bond_from_cashflows_json",
    "build_cashflow_schedule_envelope_json",
    "build_cashflow_schedule_json",
    "dated_flows_json",
    "validate_cashflow_schedule_envelope_json",
    "validate_cashflow_schedule_json",
]

def build_cashflow_schedule_json(spec_json: str, market_json: str | None = None) -> str:
    """Build a cashflow schedule from a JSON spec and return canonical schedule JSON.

    Parameters
    ----------
    spec_json : str
        JSON-encoded ``CashflowScheduleBuildSpec`` describing coupons, principal,
        day-count, and schedule rules.
    market_json : str, optional
        JSON-encoded ``MarketContext`` for floating-rate index lookups. Omit
        when the schedule uses fixed coupons only.

    Returns
    -------
    str
        Canonical JSON-encoded ``CashFlowSchedule``.

    Raises
    ------
    ValueError
        If ``spec_json`` (or ``market_json`` when supplied) fails schema or
        semantic validation.
    KeyError
        If required market data or a fixing series is missing.

    Examples
    --------
    >>> from finstack_quant.cashflows import build_cashflow_schedule_json
    >>> schedule_json = build_cashflow_schedule_json(spec_json)  # doctest: +SKIP
    """

def build_cashflow_schedule_envelope_json(spec_json: str, market_json: str | None = None) -> str:
    """Build a stamped cashflow schedule envelope from a JSON spec.

    Parameters
    ----------
    spec_json : str
        JSON-encoded ``CashflowScheduleBuildSpec``.
    market_json : str, optional
        JSON-encoded ``MarketContext`` for floating-rate lookups.

    Returns
    -------
    str
        JSON-encoded ``CashflowScheduleEnvelope`` wrapping the schedule with
        build metadata and schema version.

    Raises
    ------
    ValueError
        If the spec or market JSON is invalid or construction fails.

    Examples
    --------
    >>> from finstack_quant.cashflows import build_cashflow_schedule_envelope_json
    >>> envelope_json = build_cashflow_schedule_envelope_json(spec_json)  # doctest: +SKIP
    """

def validate_cashflow_schedule_json(schedule_json: str) -> str:
    """Validate and canonicalize a ``CashFlowSchedule`` JSON payload.

    Parameters
    ----------
    schedule_json : str
        JSON-encoded ``CashFlowSchedule``.

    Returns
    -------
    str
        Canonical re-serialized schedule JSON.

    Raises
    ------
    ValueError
        If ``schedule_json`` is malformed or fails validation.

    Examples
    --------
    >>> from finstack_quant.cashflows import validate_cashflow_schedule_json
    >>> canonical = validate_cashflow_schedule_json(schedule_json)  # doctest: +SKIP
    """

def validate_cashflow_schedule_envelope_json(envelope_json: str) -> str:
    """Validate and canonicalize a ``CashflowScheduleEnvelope`` JSON payload.

    Parameters
    ----------
    envelope_json : str
        JSON-encoded ``CashflowScheduleEnvelope``.

    Returns
    -------
    str
        Canonical re-serialized envelope JSON.

    Raises
    ------
    ValueError
        If ``envelope_json`` is malformed or fails validation.

    Examples
    --------
    >>> from finstack_quant.cashflows import validate_cashflow_schedule_envelope_json
    >>> canonical = validate_cashflow_schedule_envelope_json(envelope_json)  # doctest: +SKIP
    """

def dated_flows_json(schedule_json: str) -> str:
    """Extract settlement-dated cashflows from a schedule as a compact JSON array.

    Parameters
    ----------
    schedule_json : str
        JSON-encoded ``CashFlowSchedule``.

    Returns
    -------
    str
        JSON array of settlement cash entries. ``PIK`` and
        ``DefaultedNotional`` state rows are omitted; parse the full schedule
        JSON when flow classification is required.

    Raises
    ------
    ValueError
        If ``schedule_json`` is invalid.

    Examples
    --------
    >>> from finstack_quant.cashflows import dated_flows_json
    >>> flows_json = dated_flows_json(schedule_json)  # doctest: +SKIP
    """

def accrued_interest_json(schedule_json: str, as_of: str, config_json: str | None = None) -> float:
    """Compute accrued interest for a schedule as of a valuation date.

    Parameters
    ----------
    schedule_json : str
        JSON-encoded ``CashFlowSchedule``.
    as_of : str
        Accrual snapshot date in ISO 8601 ``YYYY-MM-DD`` form.
    config_json : str, optional
        JSON-encoded ``AccrualConfig`` overriding default accrual conventions.

    Returns
    -------
    float
        Accrued interest in the schedule settlement currency. The Rust engine
        computes from the canonical schedule and crosses the binding boundary as
        ``f64``; for large notionals, compare with an absolute tolerance scaled
        to the schedule notional rather than expecting decimal-string equality.
        Returns ``0.0`` when ``as_of`` is outside all coupon periods.

    Raises
    ------
    ValueError
        If the schedule JSON or accrual configuration is invalid.
    KeyError
        If an ex-coupon calendar is unknown.

    Examples
    --------
    >>> from finstack_quant.cashflows import accrued_interest_json
    >>> ai = accrued_interest_json(schedule_json, "2025-06-15")  # doctest: +SKIP
    """

def bond_from_cashflows_json(
    instrument_id: str,
    schedule_json: str,
    discount_curve_id: str,
    quoted_clean: float | None = None,
) -> str:
    """Construct a tagged bond instrument JSON from a cashflow schedule.

    Convenience wrapper that materializes a
    ``finstack_quant_valuations::instruments::fixed_income::bond::Bond`` from
    the supplied schedule and wraps it in the tagged ``InstrumentJson`` envelope.

    Parameters
    ----------
    instrument_id : str
        Identifier for the bond instrument (used in valuation results and metrics).
    schedule_json : str
        JSON-encoded ``CashFlowSchedule``.
    discount_curve_id : str
        Discount curve ID required in ``MarketContext`` for pricing (e.g.
        ``"USD-OIS"``).
    quoted_clean : float, optional
        Clean quoted price as a percent of par (e.g. ``99.5`` for 99.5% of par).
        When supplied, it is a price-driving override; it does not calibrate
        yield during construction.

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
    >>> from finstack_quant.cashflows import bond_from_cashflows_json
    >>> bond_json = bond_from_cashflows_json("BOND_A", schedule_json, "USD-OIS", quoted_clean=99.5)  # doctest: +SKIP
    """
