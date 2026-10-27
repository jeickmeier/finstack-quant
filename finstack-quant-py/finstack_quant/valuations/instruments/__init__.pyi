"""
Typed instruments, builders, and JSON pricing, metric and cashflow helpers.

Mirrors ``finstack_quant_valuations::instruments``. Every typed instrument
class and its builder live directly in this module, next to the JSON helpers
(``price_instrument``, ``validate_instrument_json``, ``instrument_cashflows``).

Every typed instrument ``from_json`` accepts only the versioned
``{"schema": "finstack_quant.instrument/1", "instrument": ...}`` envelope
whose ``instrument.type`` matches the class; bare ``{"type", "spec"}``
payloads and cross-type coercion are rejected. Inputs larger than 16 MiB
raise ``ValueError`` before parsing.

Examples
--------
>>> from finstack_quant.valuations.instruments import TermLoan, list_models_grouped
>>> TermLoan.example().id
'TERM-LOAN-USD-5Y'
>>> models = list_models_grouped()["bond"]
>>> all(model in models for model in ("discounting", "hazard_rate", "tree", "rates_credit"))
True

"""

from __future__ import annotations

from typing import Any
from finstack_quant.valuations.instruments._instruments import (
    Bond as Bond,
    BondBuilder as BondBuilder,
    TermLoan as TermLoan,
    TermLoanBuilder as TermLoanBuilder,
)
from finstack_quant.valuations.instruments._merton_mc import (
    BarrierCrossing as BarrierCrossing,
    MertonMcCalibrationSpec as MertonMcCalibrationSpec,
    MertonMcConfig as MertonMcConfig,
    MertonMcResult as MertonMcResult,
    PathStatistics as PathStatistics,
    PikMode as PikMode,
    PikSchedule as PikSchedule,
)
from finstack_quant.valuations.instruments._typed_legs import (
    FixedLegSpec as FixedLegSpec,
    FloatLegSpec as FloatLegSpec,
    PremiumLegSpec as PremiumLegSpec,
    ProtectionLegSpec as ProtectionLegSpec,
)
from finstack_quant.valuations.instruments._typed_rates import (
    CapFloor as CapFloor,
    CapFloorBuilder as CapFloorBuilder,
    InterestRateSwap as InterestRateSwap,
    InterestRateSwapBuilder as InterestRateSwapBuilder,
    Swaption as Swaption,
    SwaptionBuilder as SwaptionBuilder,
)
from finstack_quant.valuations.instruments._typed_credit.cds import (
    CreditDefaultSwap as CreditDefaultSwap,
    CreditDefaultSwapBuilder as CreditDefaultSwapBuilder,
)
from finstack_quant.valuations.instruments._typed_credit.cds_option import (
    CdsOption as CdsOption,
    CdsOptionBuilder as CdsOptionBuilder,
)
from finstack_quant.valuations.instruments._typed_credit.cds_index import (
    CdsIndex as CdsIndex,
    CdsIndexBuilder as CdsIndexBuilder,
    CdsIndexConstituent as CdsIndexConstituent,
    CdsIndexParams as CdsIndexParams,
)
from finstack_quant.valuations.instruments._typed_credit.cds_tranche import (
    CdsTranche as CdsTranche,
    CdsTrancheBuilder as CdsTrancheBuilder,
    CdsTrancheParams as CdsTrancheParams,
)
from finstack_quant.valuations.instruments._typed_convertible import (
    CallPutSchedule as CallPutSchedule,
    ConversionSpec as ConversionSpec,
    ConvertibleBond as ConvertibleBond,
    ConvertibleBondBuilder as ConvertibleBondBuilder,
)
from finstack_quant.valuations.instruments._typed_fx import (
    FxForward as FxForward,
    FxForwardBuilder as FxForwardBuilder,
    FxOption as FxOption,
    FxOptionBuilder as FxOptionBuilder,
)
from finstack_quant.valuations.instruments._typed_equity import (
    EquityOption as EquityOption,
    EquityOptionBuilder as EquityOptionBuilder,
)
from finstack_quant.valuations.instruments._typed_structured_credit.rep_line import RepLine as RepLine
from finstack_quant.valuations.instruments._typed_structured_credit.asset_pool import AssetPool as AssetPool
from finstack_quant.valuations.instruments._typed_structured_credit.tranche import (
    Tranche as Tranche,
    TrancheBuilder as TrancheBuilder,
)
from finstack_quant.valuations.instruments._typed_structured_credit.tranche_structure import (
    TrancheStructure as TrancheStructure,
)
from finstack_quant.valuations.instruments._typed_structured_credit.pool_asset import PoolAsset as PoolAsset
from finstack_quant.valuations.instruments._typed_structured_credit.call_assumption import (
    CallAssumption as CallAssumption,
)
from finstack_quant.valuations.instruments._typed_structured_credit.specs import (
    BalloonSpec as BalloonSpec,
    LiquidationSpec as LiquidationSpec,
    PrepaymentPenalty as PrepaymentPenalty,
    SpecialServicingSpec as SpecialServicingSpec,
)
from finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base import (
    AdvanceRate as AdvanceRate,
    BorrowingBaseRules as BorrowingBaseRules,
    ConcentrationLimit as ConcentrationLimit,
    EligibilityRule as EligibilityRule,
)
from finstack_quant.valuations.instruments._typed_structured_credit.coverage_rules import CoverageRules as CoverageRules
from finstack_quant.valuations.instruments._typed_structured_credit.hedge_swap import HedgeSwap as HedgeSwap
from finstack_quant.valuations.instruments._typed_structured_credit.waterfall import Waterfall as Waterfall
from finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows import (
    TrancheCashflows as TrancheCashflows,
)
from finstack_quant.valuations.instruments._typed_structured_credit.equity_metrics import EquityMetrics as EquityMetrics
from finstack_quant.valuations.instruments._typed_structured_credit.pool_stats import (
    PoolStats as PoolStats,
    calculate_pool_stats as calculate_pool_stats,
)
from finstack_quant.valuations.instruments._typed_structured_credit.standard_deal_params import (
    StandardDealParams as StandardDealParams,
)
from finstack_quant.valuations.instruments._typed_structured_credit.structured_credit import (
    StructuredCredit as StructuredCredit,
    StructuredCreditBuilder as StructuredCreditBuilder,
)
from finstack_quant.valuations.instruments._pricing import (
    InstrumentCashflowEnvelope as InstrumentCashflowEnvelope,
    MarketHistory as MarketHistory,
    MetricPricingOverrides as MetricPricingOverrides,
    instrument_cashflows as instrument_cashflows,
    list_models as list_models,
    list_models_grouped as list_models_grouped,
    list_standard_metrics as list_standard_metrics,
    list_standard_metrics_grouped as list_standard_metrics_grouped,
    metric_metadata as metric_metadata,
    price_instrument as price_instrument,
)
from finstack_quant.valuations.instruments._typed_asset_backed_facility.facility_terms import (
    AmortizationEvent as AmortizationEvent,
    TermOutSpec as TermOutSpec,
)
from finstack_quant.valuations.instruments._typed_asset_backed_facility.facility import (
    AssetBackedFacility as AssetBackedFacility,
    AssetBackedFacilityBuilder as AssetBackedFacilityBuilder,
    FacilityProjection as FacilityProjection,
)
from finstack_quant.valuations.instruments._typed_revolving_credit.revolving_credit import (
    RevolvingCredit as RevolvingCredit,
    RevolvingCreditBuilder as RevolvingCreditBuilder,
)
from finstack_quant.valuations.instruments._typed_revolving_credit.results import (
    EnhancedMonteCarloResult as EnhancedMonteCarloResult,
)
from finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result import (
    StochasticPricingResult as StochasticPricingResult,
)
from finstack_quant.valuations.instruments._typed_structured_credit.simulation_run import SimulationRun as SimulationRun
from finstack_quant.valuations.instruments._typed_structured_credit.diagnostics import (
    SimulationDiagnostics as SimulationDiagnostics,
)
from finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics import (
    OasResult as OasResult,
    ScenarioTable as ScenarioTable,
    TrancheMetrics as TrancheMetrics,
    tranche_breakeven_cdr as tranche_breakeven_cdr,
    tranche_discount_margin as tranche_discount_margin,
    tranche_metrics as tranche_metrics,
    tranche_oas as tranche_oas,
    tranche_scenario_table as tranche_scenario_table,
)
from finstack_quant.valuations.instruments._var import VarResult as VarResult, calculate_var as calculate_var

__all__ = [
    "AssetPool",
    "BarrierCrossing",
    "Bond",
    "BondBuilder",
    "CallAssumption",
    "CallPutSchedule",
    "CapFloor",
    "CapFloorBuilder",
    "CdsIndex",
    "CdsIndexBuilder",
    "CdsIndexConstituent",
    "CdsIndexParams",
    "CdsOption",
    "CdsOptionBuilder",
    "CdsTranche",
    "CdsTrancheBuilder",
    "CdsTrancheParams",
    "ConversionSpec",
    "ConvertibleBond",
    "ConvertibleBondBuilder",
    "CoverageRules",
    "CreditDefaultSwap",
    "CreditDefaultSwapBuilder",
    "EquityMetrics",
    "EquityOption",
    "EquityOptionBuilder",
    "FixedLegSpec",
    "FloatLegSpec",
    "FxForward",
    "FxForwardBuilder",
    "FxOption",
    "FxOptionBuilder",
    "HedgeSwap",
    "InstrumentCashflowEnvelope",
    "InterestRateSwap",
    "InterestRateSwapBuilder",
    "MarketHistory",
    "MertonMcCalibrationSpec",
    "MertonMcConfig",
    "MertonMcResult",
    "MetricPricingOverrides",
    "OasResult",
    "PathStatistics",
    "PikMode",
    "PikSchedule",
    "PoolAsset",
    "PoolStats",
    "PremiumLegSpec",
    "ProtectionLegSpec",
    "RepLine",
    "ScenarioTable",
    "SimulationRun",
    "StandardDealParams",
    "StructuredCredit",
    "StructuredCreditBuilder",
    "Swaption",
    "SwaptionBuilder",
    "TermLoan",
    "TermLoanBuilder",
    "Tranche",
    "TrancheBuilder",
    "TrancheCashflows",
    "TrancheMetrics",
    "TrancheStructure",
    "VarResult",
    "Waterfall",
    "bond_from_cashflows_json",
    "calculate_pool_stats",
    "calculate_var",
    "instrument_cashflows",
    "instrument_envelope_from_spec",
    "list_models",
    "list_models_grouped",
    "list_standard_metrics",
    "list_standard_metrics_grouped",
    "metric_metadata",
    "pretty_instrument_json",
    "price_instrument",
    "tranche_breakeven_cdr",
    "tranche_discount_margin",
    "tranche_metrics",
    "tranche_oas",
    "tranche_scenario_table",
    "validate_instrument_json",
    "validate_typed_instrument_json",
]

def bond_from_cashflows_json(
    instrument_id: str,
    schedule_json: str,
    discount_curve_id: str,
    quoted_clean_price_pct: float | None = None,
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
    quoted_clean_price_pct : float, optional
        Clean quoted price in percent of par (``99.5`` = 99.5%), stored as
        ``instrument_pricing_overrides.market_quotes.quoted_clean_price_pct``.

    Returns
    -------
    str
        Canonical ``finstack_quant.instrument/1`` envelope containing the bond.

    Raises
    ------
    ValueError
        If the schedule is invalid or bond construction fails.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import bond_from_cashflows_json
    >>> try:
    ...     bond_from_cashflows_json("B", "{}", "USD-OIS")
    ... except ValueError as exc:
    ...     print("flows" in str(exc))
    True

    """
    ...

def validate_instrument_json(
    json: dict[str, Any] | str,
    metric_pricing_overrides: MetricPricingOverrides | dict[str, Any] | str | None = None,
) -> str:
    """
    Validate a canonical instrument envelope and return canonical JSON.

    Parameters
    ----------
    json : dict[str, Any] | str
        A ``finstack_quant.instrument/1`` envelope, as a dict (the form
        :func:`instrument_envelope_from_spec` returns) or JSON text. Bare
        instrument payloads are rejected.
    metric_pricing_overrides : MetricPricingOverrides or dict or str, optional
        Metric-time overrides merged into
        ``instrument.spec.metric_pricing_overrides`` before instrument
        validation; ``None`` (the default) retains the envelope configuration.
        Dict and JSON patches replace only supplied fields, including individual
        ``bump_config`` fields; explicit ``None``/``null`` clears optional fields.

    Returns
    -------
    str
        Canonical serialized instrument JSON after Rust validation and
        optional override merging.

    Raises
    ------
    ValueError
        If the JSON is malformed, ``metric_pricing_overrides`` is invalid
        override JSON, the merged payload has an unknown instrument tag, or
        instrument-specific validation fails.
    TypeError
        If ``json`` is neither a dict nor a string.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.instruments import TermLoan, validate_instrument_json
    >>> validated = json.loads(validate_instrument_json(TermLoan.example().to_json()))
    >>> validated["instrument"]["type"]
    'term_loan'

    """
    ...

def validate_typed_instrument_json(type_tag: str, json: dict[str, Any] | str) -> str:
    """
    Validate a payload as one exact instrument type and return the envelope.

    Parameters
    ----------
    type_tag : str
        Canonical instrument discriminator, such as ``"term_loan"`` or
        ``"fx_forward"``.
    json : dict[str, Any] | str
        A ``finstack_quant.instrument/1`` envelope, as a dict or JSON text,
        whose instrument type must match *type_tag*.

    Returns
    -------
    str
        Canonical instrument envelope for the validated instrument.

    Raises
    ------
    ValueError
        If ``json`` is malformed, carries a different instrument type, or
        fails instrument validation.
    TypeError
        If ``json`` is neither a dict nor a string.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.instruments import TermLoan, validate_typed_instrument_json
    >>> envelope = TermLoan.example().to_json()
    >>> json.loads(validate_typed_instrument_json("term_loan", envelope))["instrument"]["type"]
    'term_loan'

    """
    ...

def pretty_instrument_json(json: dict[str, Any] | str) -> str:
    """
    Re-render a canonical instrument envelope as pretty-printed JSON.

    Parameters
    ----------
    json : str
        A canonical ``finstack_quant.instrument/1`` envelope.

    Returns
    -------
    str
        The same envelope, pretty-printed.

    Raises
    ------
    ValueError
        If ``json`` is malformed or cannot be rendered.
    TypeError
        If ``json`` is neither a dict nor a string.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import TermLoan, pretty_instrument_json
    >>> "term_loan" in pretty_instrument_json(TermLoan.example().to_json())
    True

    """
    ...

def instrument_envelope_from_spec(type_tag: str, spec: dict[str, Any] | str) -> dict[str, Any]:
    """
    Wrap a bare instrument spec in the canonical envelope after validating it.

    Pure delegation to Rust ``pricer::instrument_envelope_from_spec``. This is
    the construction route for every instrument type without a typed Python
    class (FRA, deposit, FX spot/swap, inflation swaps, exotics, ...): build
    the type's ``spec`` object (see
    ``finstack_quant.valuations.schema.instrument_type_schema(type_tag)``) and
    pass the returned envelope dict straight to :func:`price_instrument`,
    :func:`instrument_cashflows` or any other entry point that takes an
    instrument.

    Parameters
    ----------
    type_tag : str
        Canonical instrument discriminator, one of
        ``finstack_quant.valuations.schema.instrument_types()`` (e.g.
        ``"forward_rate_agreement"``, ``"fx_spot"``).
    spec : dict[str, Any] | str
        The bare ``spec`` object for that type, as a dict or JSON text. Tagged
        ``{"type", "spec"}`` payloads and envelopes are rejected.

    Returns
    -------
    dict[str, Any]
        The canonical ``finstack_quant.instrument/1`` envelope, with keys
        ``schema`` and ``instrument`` (``{"type", "spec"}``). Use
        ``json.dumps`` when the wire text is needed.

    Raises
    ------
    ValueError
        If ``spec`` is not a bare object, does not deserialize as a
        ``type_tag`` instrument (unknown tag, missing or unknown field), or
        fails instrument validation.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import instrument_envelope_from_spec
    >>> envelope = instrument_envelope_from_spec(
    ...     "fx_spot",
    ...     {
    ...         "id": "EURUSD-SPOT",
    ...         "base_currency": "EUR",
    ...         "quote_currency": "USD",
    ...         "settlement_date": "2025-01-17",
    ...         "quoted_spot": 1.2,
    ...         "notional": {"amount": "1000000", "currency": "EUR"},
    ...         "attributes": {},
    ...     },
    ... )
    >>> (envelope["instrument"]["type"], envelope["instrument"]["spec"]["id"])
    ('fx_spot', 'EURUSD-SPOT')
    """
    ...
