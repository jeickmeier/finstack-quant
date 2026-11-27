"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/json_validation.rs``."""

from __future__ import annotations

from typing import Any
from finstack_quant.valuations.instruments._instruments.bond import (
    Bond,
    BondBuilder,
)
from finstack_quant.valuations.instruments._instruments.term_loan import (
    TermLoan,
    TermLoanBuilder,
)
from finstack_quant.valuations.instruments._bond_quotes import BondQuoteSet, compute_quotes
from finstack_quant.valuations.instruments._bond_breakeven import (
    BondBreakevenLadder,
    BondBreakevenRow,
    bond_breakeven_ladder,
)
from finstack_quant.valuations.instruments._bond_workout import BondWorkoutLadder, BondWorkoutRow, bond_workout_ladder
from finstack_quant.valuations.instruments._merton_mc import (
    BarrierCrossing,
    MertonMcCalibrationSpec,
    MertonMcConfig,
    MertonMcResult,
    PathStatistics,
    PikMode,
    PikSchedule,
)
from finstack_quant.valuations.instruments._typed_legs import (
    FixedLegSpec,
    FloatLegSpec,
    PremiumLegSpec,
    ProtectionLegSpec,
)
from finstack_quant.valuations.instruments._typed_interest_rate_future_option import (
    InterestRateFutureOption,
    InterestRateFutureOptionBuilder,
)
from finstack_quant.valuations.instruments._typed_equity_future_option import (
    EquityFutureOption,
    EquityFutureOptionBuilder,
)
from finstack_quant.valuations.instruments._typed_fx_future_option import FxFutureOption, FxFutureOptionBuilder
from finstack_quant.valuations.instruments._typed_volatility_index_future_option import (
    VolatilityIndexFutureOption,
    VolatilityIndexFutureOptionBuilder,
)
from finstack_quant.valuations.instruments._typed_interest_rate_future import (
    InterestRateFuture,
    InterestRateFutureBuilder,
)
from finstack_quant.valuations.instruments._typed_equity_future import EquityFuture, EquityFutureBuilder
from finstack_quant.valuations.instruments._typed_fx_future import FxFuture, FxFutureBuilder
from finstack_quant.valuations.instruments._typed_volatility_index_future import (
    VolatilityIndexFuture,
    VolatilityIndexFutureBuilder,
)
from finstack_quant.valuations.instruments._typed_bond_future import BondFuture, BondFutureBuilder
from finstack_quant.valuations.instruments._typed_listed import FutureOptionTerms, ListedFutureTerms
from finstack_quant.valuations.instruments._typed_inflation_bond import InflationLinkedBond, InflationLinkedBondBuilder
from finstack_quant.valuations.instruments._typed_deposit import Deposit, DepositBuilder
from finstack_quant.valuations.instruments._typed_rates.cap_floor import (
    CapFloor,
    CapFloorBuilder,
)
from finstack_quant.valuations.instruments._typed_rates.swap import (
    InterestRateSwap,
    InterestRateSwapBuilder,
)
from finstack_quant.valuations.instruments._typed_rates.swaption import (
    Swaption,
    SwaptionBuilder,
)
from finstack_quant.valuations.instruments._typed_fra import ForwardRateAgreement, ForwardRateAgreementBuilder
from finstack_quant.valuations.instruments._typed_credit.cds import CreditDefaultSwap, CreditDefaultSwapBuilder
from finstack_quant.valuations.instruments._typed_credit.cds_option import CdsOption, CdsOptionBuilder
from finstack_quant.valuations.instruments._typed_credit.cds_index import (
    CdsIndex,
    CdsIndexBuilder,
    CdsIndexConstituent,
    CdsIndexParams,
)
from finstack_quant.valuations.instruments._typed_credit.cds_tranche import (
    CdsTranche,
    CdsTrancheBuilder,
    CdsTrancheParams,
)
from finstack_quant.valuations.instruments._typed_convertible import (
    CallPutSchedule,
    ConversionSpec,
    ConvertibleBond,
    ConvertibleBondBuilder,
)
from finstack_quant.valuations.instruments._convertible_spot_ladder import (
    ConvertibleSpotLadder,
    ConvertibleSpotRow,
    convertible_spot_ladder,
)
from finstack_quant.valuations.instruments._typed_fx import FxForward, FxForwardBuilder, FxOption, FxOptionBuilder
from finstack_quant.valuations.instruments._typed_repo import Repo, RepoBuilder
from finstack_quant.valuations.instruments._typed_fx_swap import FxSwap, FxSwapBuilder
from finstack_quant.valuations.instruments._typed_ndf import Ndf, NdfBuilder
from finstack_quant.valuations.instruments._typed_quanto_option import QuantoOption, QuantoOptionBuilder
from finstack_quant.valuations.instruments._typed_fx_digital_option import FxDigitalOption, FxDigitalOptionBuilder
from finstack_quant.valuations.instruments._typed_fx_touch_option import FxTouchOption, FxTouchOptionBuilder
from finstack_quant.valuations.instruments._typed_fx_barrier_option import FxBarrierOption, FxBarrierOptionBuilder
from finstack_quant.valuations.instruments._typed_equity_spot import Equity, EquityBuilder
from finstack_quant.valuations.instruments._typed_fx_spot import FxSpot, FxSpotBuilder
from finstack_quant.valuations.instruments._typed_commodity_forward import CommodityForward, CommodityForwardBuilder
from finstack_quant.valuations.instruments._typed_commodity_future import CommodityFuture, CommodityFutureBuilder
from finstack_quant.valuations.instruments._typed_commodity_asian_option import (
    CommodityAsianOption,
    CommodityAsianOptionBuilder,
)
from finstack_quant.valuations.instruments._typed_commodity_spot_option import (
    CommoditySpotOption,
    CommoditySpotOptionBuilder,
)
from finstack_quant.valuations.instruments._typed_commodity_spread_option import (
    CommoditySpreadOption,
    CommoditySpreadOptionBuilder,
)
from finstack_quant.valuations.instruments._typed_commodity_swap import CommoditySwap, CommoditySwapBuilder
from finstack_quant.valuations.instruments._typed_commodity_swaption import CommoditySwaption, CommoditySwaptionBuilder
from finstack_quant.valuations.instruments._typed_fx_variance_swap import FxVarianceSwap, FxVarianceSwapBuilder
from finstack_quant.valuations.instruments._typed_variance_swap import VarianceSwap, VarianceSwapBuilder
from finstack_quant.valuations.instruments._typed_equity_total_return_swap import (
    EquityTotalReturnSwap,
    EquityTotalReturnSwapBuilder,
)
from finstack_quant.valuations.instruments._typed_equity_total_return_future import (
    EquityTotalReturnFuture,
    EquityTotalReturnFutureBuilder,
)
from finstack_quant.valuations.instruments._typed_fi_index_total_return_swap import (
    FiIndexTotalReturnSwap,
    FiIndexTotalReturnSwapBuilder,
)
from finstack_quant.valuations.instruments._typed_commodity_future_option import (
    CommodityFutureOption,
    CommodityFutureOptionBuilder,
)
from finstack_quant.valuations.instruments._typed_basis_swap import BasisSwap, BasisSwapBuilder
from finstack_quant.valuations.instruments._typed_xccy_swap import XccySwap, XccySwapBuilder
from finstack_quant.valuations.instruments._typed_cms_swap import CmsSwap, CmsSwapBuilder
from finstack_quant.valuations.instruments._typed_cms_option import CmsOption, CmsOptionBuilder
from finstack_quant.valuations.instruments._typed_cms_spread_option import CmsSpreadOption, CmsSpreadOptionBuilder
from finstack_quant.valuations.instruments._typed_inflation_swap import InflationSwap, InflationSwapBuilder
from finstack_quant.valuations.instruments._typed_yoy_inflation_swap import YoYInflationSwap, YoYInflationSwapBuilder
from finstack_quant.valuations.instruments._typed_inflation_cap_floor import InflationCapFloor, InflationCapFloorBuilder
from finstack_quant.valuations.instruments._typed_bermudan_swaption import BermudanSwaption, BermudanSwaptionBuilder
from finstack_quant.valuations.instruments._typed_autocallable import Autocallable, AutocallableBuilder
from finstack_quant.valuations.instruments._typed_cliquet_option import CliquetOption, CliquetOptionBuilder
from finstack_quant.valuations.instruments._typed_private_markets_fund import (
    PrivateMarketsFund,
    PrivateMarketsFundBuilder,
)
from finstack_quant.valuations.instruments._typed_asian_option import AsianOption, AsianOptionBuilder
from finstack_quant.valuations.instruments._typed_barrier_option import BarrierOption, BarrierOptionBuilder
from finstack_quant.valuations.instruments._typed_lookback_option import LookbackOption, LookbackOptionBuilder
from finstack_quant.valuations.instruments._typed_range_accrual import RangeAccrual, RangeAccrualBuilder
from finstack_quant.valuations.instruments._typed_tarn import Tarn, TarnBuilder
from finstack_quant.valuations.instruments._typed_snowball import Snowball, SnowballBuilder
from finstack_quant.valuations.instruments._typed_callable_range_accrual import (
    CallableRangeAccrual,
    CallableRangeAccrualBuilder,
)
from finstack_quant.valuations.instruments._typed_agency_tba import AgencyTba, AgencyTbaBuilder
from finstack_quant.valuations.instruments._typed_agency_mbs_passthrough import (
    AgencyMbsPassthrough,
    AgencyMbsPassthroughBuilder,
)
from finstack_quant.valuations.instruments._typed_dollar_roll import DollarRoll, DollarRollBuilder
from finstack_quant.valuations.instruments._typed_agency_cmo import AgencyCmo, AgencyCmoBuilder
from finstack_quant.valuations.instruments._typed_basket import Basket, BasketBuilder
from finstack_quant.valuations.instruments._typed_real_estate_asset import RealEstateAsset, RealEstateAssetBuilder
from finstack_quant.valuations.instruments._typed_levered_real_estate_equity import (
    LeveredRealEstateEquity,
    LeveredRealEstateEquityBuilder,
)
from finstack_quant.valuations.instruments._typed_equity import EquityOption, EquityOptionBuilder
from finstack_quant.valuations.instruments._typed_structured_credit.structured_credit_tranche import (
    StructuredCreditTranche,
)
from finstack_quant.valuations.instruments._typed_structured_credit.rep_line import RepLine
from finstack_quant.valuations.instruments._typed_structured_credit.asset_pool import AssetPool
from finstack_quant.valuations.instruments._typed_structured_credit.tranche import Tranche, TrancheBuilder
from finstack_quant.valuations.instruments._typed_structured_credit.tranche_structure import TrancheStructure
from finstack_quant.valuations.instruments._typed_structured_credit.pool_asset import PoolAsset
from finstack_quant.valuations.instruments._typed_structured_credit.call_assumption import CallAssumption
from finstack_quant.valuations.instruments._typed_structured_credit.specs import (
    BalloonSpec,
    LiquidationSpec,
    PrepaymentPenalty,
    SpecialServicingSpec,
)
from finstack_quant.valuations.instruments._typed_structured_credit.borrowing_base import (
    AdvanceRate,
    BorrowingBaseRules,
    ConcentrationLimit,
    EligibilityRule,
)
from finstack_quant.valuations.instruments._typed_structured_credit.coverage_rules import CoverageRules
from finstack_quant.valuations.instruments._typed_structured_credit.hedge_swap import HedgeSwap
from finstack_quant.valuations.instruments._typed_structured_credit.waterfall import Waterfall
from finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows import TrancheCashflows
from finstack_quant.valuations.instruments._typed_structured_credit.equity_metrics import EquityMetrics
from finstack_quant.valuations.instruments._typed_structured_credit.pool_report import (
    PoolConcentrationEntry,
    PoolReport,
    pool_report,
)
from finstack_quant.valuations.instruments._typed_structured_credit.pool_stats import PoolStats, calculate_pool_stats
from finstack_quant.valuations.instruments._typed_structured_credit.standard_deal_params import StandardDealParams
from finstack_quant.valuations.instruments._typed_structured_credit.structured_credit import (
    StructuredCredit,
    StructuredCreditBuilder,
)
from finstack_quant.valuations.instruments._pricing import (
    InstrumentCashflowEnvelope,
    MarketHistory,
    MetricPricingOverrides,
    PriceExtras,
    instrument_cashflows,
    list_models,
    list_models_grouped,
    list_standard_metrics,
    list_standard_metrics_grouped,
    metric_metadata,
    metrics_for_instrument,
    price_driver_fields,
    price_instrument,
    pool_report_field_units,
    bond_quote_set_field_units,
    standard_deal_families,
    scenario_table_field_units,
    simulation_field_units,
    equity_metrics_field_units,
    tranche_metric_field_units,
    tranche_solve_field_units,
)
from finstack_quant.valuations.instruments._typed_asset_backed_facility.facility_terms import (
    AmortizationEvent,
    TermOutSpec,
)
from finstack_quant.valuations.instruments._typed_asset_backed_facility.facility import (
    AssetBackedFacility,
    AssetBackedFacilityBuilder,
    FacilityProjection,
)
from finstack_quant.valuations.instruments._typed_revolving_credit.revolving_credit import (
    RevolvingCredit,
    RevolvingCreditBuilder,
)
from finstack_quant.valuations.instruments._typed_revolving_credit.results import EnhancedMonteCarloResult
from finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result import StochasticPricingResult
from finstack_quant.valuations.instruments._typed_structured_credit.simulation_run import SimulationRun
from finstack_quant.valuations.instruments._typed_structured_credit.diagnostics import SimulationDiagnostics
from finstack_quant.valuations.instruments._typed_structured_credit.tranche_analytics import (
    OasResult,
    ScenarioTable,
    TrancheMetrics,
    tranche_breakeven_cdr,
    tranche_discount_margin,
    tranche_metrics,
    tranche_oas,
    tranche_scenario_table,
)
from finstack_quant.valuations.instruments._var import VarResult, calculate_var

def validate_instrument_json(
    json: dict[str, Any] | str,
    metric_pricing_overrides: MetricPricingOverrides | dict[str, Any] | None = None,
) -> str:
    """
    Validate a canonical instrument envelope and return canonical JSON.

    Parameters
    ----------
    json : dict[str, Any] | str
        A ``finstack_quant.instrument/1`` envelope, as a dict (the form
        :func:`instrument_envelope_from_spec` returns) or JSON text. Bare
        instrument payloads are rejected.
    metric_pricing_overrides : MetricPricingOverrides or dict, optional
        Metric-time overrides merged into
        ``instrument.spec.metric_pricing_overrides`` before instrument
        validation; ``None`` (the default) retains the envelope configuration.
        A dict patch replaces only supplied fields, including individual
        ``bump_config`` fields; an explicit ``None`` clears an optional field.

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

def instrument_envelope_from_spec(type_tag: str, spec: dict[str, Any]) -> dict[str, Any]:
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
    spec : dict[str, Any]
        The bare ``spec`` object for that type, as a dict. Tagged
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
