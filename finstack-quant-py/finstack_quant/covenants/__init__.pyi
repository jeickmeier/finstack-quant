"""
Covenant package JSON validation, templates, and map-backed evaluation.

Bindings for ``finstack-quant-covenants``. Validate covenant specs, reports, and
engines; evaluate an engine against a metric map; or instantiate standard
covenant packages (LBO, covenant-lite, real estate, project finance) as JSON.

Examples
--------
>>> import finstack_quant.covenants as covenants
>>> covenants.__name__
'finstack_quant.covenants'
"""

from __future__ import annotations

__all__ = [
    "cov_lite",
    "evaluate_engine",
    "lbo_standard",
    "project_finance",
    "real_estate",
    "validate_covenant_engine",
    "validate_covenant_report",
    "validate_covenant_spec",
]

def validate_covenant_spec(spec_json: str) -> str:
    """
    Validate and canonicalize a covenant specification JSON string.

    Parameters
    ----------
    spec_json : str
        JSON-encoded ``CovenantSpec`` describing thresholds, tests, and covenants.

    Returns
    -------
    str
        Canonical JSON after validation.

    Raises
    ------
    ValueError
        If the spec fails schema or semantic validation.

    Examples
    --------
    >>> from finstack_quant.covenants import validate_covenant_spec
    >>> canonical = validate_covenant_spec(spec_json)  # doctest: +SKIP
    """

def validate_covenant_report(report_json: str) -> str:
    """
    Validate and canonicalize a covenant evaluation report JSON string.

    Parameters
    ----------
    report_json : str
        JSON-encoded ``CovenantReport`` with pass/fail and headroom per covenant.

    Returns
    -------
    str
        Canonical JSON after validation.

    Raises
    ------
    ValueError
        If the report JSON is malformed or fails validation.

    Examples
    --------
    >>> from finstack_quant.covenants import validate_covenant_report
    >>> canonical = validate_covenant_report(report_json)  # doctest: +SKIP
    """

def validate_covenant_engine(engine_json: str) -> str:
    """
    Validate and canonicalize a covenant engine JSON string.

    Parameters
    ----------
    engine_json : str
        JSON-encoded covenant engine configuration bundling specs and evaluation
        policy.

    Returns
    -------
    str
        Canonical JSON after validation.

    Raises
    ------
    ValueError
        If the engine JSON is malformed or fails validation.

    Examples
    --------
    >>> from finstack_quant.covenants import validate_covenant_engine
    >>> canonical = validate_covenant_engine(engine_json)  # doctest: +SKIP
    """

def evaluate_engine(engine_json: str, metrics_json: str, as_of: str) -> str:
    """
    Evaluate a covenant engine against a JSON metric map.

    Parameters
    ----------
    engine_json : str
        Serialized covenant engine configuration.
    metrics_json : str
        JSON map of metric name to numeric value (e.g. leverage, DSCR, coverage).
    as_of : str
        Evaluation date as ISO 8601 ``YYYY-MM-DD``.

    Returns
    -------
    str
        JSON-encoded ``CovenantReport`` with pass/fail status and headroom per
        covenant.

    Raises
    ------
    ValueError
        If engine or metrics JSON is invalid, or required metrics are missing.

    Examples
    --------
    >>> from finstack_quant.covenants import evaluate_engine
    >>> report_json = evaluate_engine(engine_json, metrics_json, "2025-06-30")  # doctest: +SKIP
    """

def lbo_standard(
    initial_leverage: float,
    interest_coverage: float,
    fixed_charge_coverage: float,
    max_capex: float,
) -> str:
    """
    Return a standard leveraged-buyout covenant package as JSON.

    Parameters
    ----------
    initial_leverage : float
        Maximum net leverage ratio (e.g. ``6.0`` for 6.0x).
    interest_coverage : float
        Minimum interest coverage ratio.
    fixed_charge_coverage : float
        Minimum fixed-charge coverage ratio.
    max_capex : float
        Maximum capital expenditure as a fraction of EBITDA or similar base.

    Returns
    -------
    str
        JSON-encoded ``CovenantSpec`` for a typical LBO covenant suite.

    Raises
    ------
    ValueError
        If any threshold is non-finite or out of range.

    Examples
    --------
    >>> from finstack_quant.covenants import lbo_standard
    >>> spec_json = lbo_standard(6.0, 2.0, 1.5, 0.05)  # doctest: +SKIP
    """

def cov_lite(max_leverage: float, max_senior_leverage: float) -> str:
    """
    Return a covenant-lite package as JSON.

    Parameters
    ----------
    max_leverage : float
        Maximum total leverage ratio.
    max_senior_leverage : float
        Maximum senior secured leverage ratio.

    Returns
    -------
    str
        JSON-encoded ``CovenantSpec`` with minimal maintenance covenants.

    Raises
    ------
    ValueError
        If thresholds are non-finite or out of range.

    Examples
    --------
    >>> from finstack_quant.covenants import cov_lite
    >>> spec_json = cov_lite(7.0, 4.5)  # doctest: +SKIP
    """

def real_estate(min_dscr: float, min_debt_yield: float, max_ltv: float) -> str:
    """
    Return a real-estate covenant package as JSON.

    Parameters
    ----------
    min_dscr : float
        Minimum debt-service coverage ratio.
    min_debt_yield : float
        Minimum debt yield (decimal, e.g. ``0.08`` for 8%).
    max_ltv : float
        Maximum loan-to-value ratio (decimal, e.g. ``0.75`` for 75%).

    Returns
    -------
    str
        JSON-encoded ``CovenantSpec`` for commercial real-estate lending.

    Raises
    ------
    ValueError
        If thresholds are non-finite or out of range.

    Examples
    --------
    >>> from finstack_quant.covenants import real_estate
    >>> spec_json = real_estate(1.25, 0.08, 0.75)  # doctest: +SKIP
    """

def project_finance(
    min_dscr: float,
    distribution_lockup_dscr: float,
    min_liquidity: float,
    max_net_leverage: float,
) -> str:
    """
    Return a project-finance covenant package as JSON.

    Parameters
    ----------
    min_dscr : float
        Minimum debt-service coverage ratio.
    distribution_lockup_dscr : float
        DSCR threshold below which distributions are locked up.
    min_liquidity : float
        Minimum liquidity reserve (currency units or ratio per spec convention).
    max_net_leverage : float
        Maximum net leverage ratio.

    Returns
    -------
    str
        JSON-encoded ``CovenantSpec`` for project-finance structures.

    Raises
    ------
    ValueError
        If thresholds are non-finite or out of range.

    Examples
    --------
    >>> from finstack_quant.covenants import project_finance
    >>> spec_json = project_finance(1.30, 1.10, 5_000_000.0, 5.0)  # doctest: +SKIP
    """
