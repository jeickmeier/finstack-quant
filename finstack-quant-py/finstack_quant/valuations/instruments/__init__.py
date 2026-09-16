"""Instrument wrappers and JSON helpers for ``finstack_quant.valuations``.

This mirrors ``finstack_quant_valuations::instruments``: category-specific
wrappers live in submodules, while JSON pricing helpers are exported here.

Examples:
--------
>>> from finstack_quant.valuations.instruments import TermLoan
>>> TermLoan.example().id
'TERM-LOAN-USD-5Y'

"""

from finstack_quant.finstack_quant import valuations as _valuations

AssetPool = _valuations.instruments.AssetPool
BarrierCrossing = _valuations.instruments.BarrierCrossing
Bond = _valuations.instruments.Bond
BondBuilder = _valuations.instruments.BondBuilder
CDSIndex = _valuations.instruments.CDSIndex
CDSIndexBuilder = _valuations.instruments.CDSIndexBuilder
CDSIndexConstituent = _valuations.instruments.CDSIndexConstituent
CDSIndexParams = _valuations.instruments.CDSIndexParams
CDSTranche = _valuations.instruments.CDSTranche
CDSTrancheBuilder = _valuations.instruments.CDSTrancheBuilder
CDSTrancheParams = _valuations.instruments.CDSTrancheParams
CallPutSchedule = _valuations.instruments.CallPutSchedule
CapFloor = _valuations.instruments.CapFloor
CapFloorBuilder = _valuations.instruments.CapFloorBuilder
ConversionSpec = _valuations.instruments.ConversionSpec
ConvertibleBond = _valuations.instruments.ConvertibleBond
ConvertibleBondBuilder = _valuations.instruments.ConvertibleBondBuilder
CreditDefaultSwap = _valuations.instruments.CreditDefaultSwap
CreditDefaultSwapBuilder = _valuations.instruments.CreditDefaultSwapBuilder
EnhancedMonteCarloResult = _valuations.instruments.EnhancedMonteCarloResult
EquityOption = _valuations.instruments.EquityOption
EquityOptionBuilder = _valuations.instruments.EquityOptionBuilder
FixedLegSpec = _valuations.instruments.FixedLegSpec
FloatLegSpec = _valuations.instruments.FloatLegSpec
FxForward = _valuations.instruments.FxForward
FxForwardBuilder = _valuations.instruments.FxForwardBuilder
FxOption = _valuations.instruments.FxOption
FxOptionBuilder = _valuations.instruments.FxOptionBuilder
InterestRateSwap = _valuations.instruments.InterestRateSwap
InterestRateSwapBuilder = _valuations.instruments.InterestRateSwapBuilder
MarketHistory = _valuations.instruments.MarketHistory
MertonMcConfig = _valuations.instruments.MertonMcConfig
MertonMcResult = _valuations.instruments.MertonMcResult
MetricPricingOverrides = _valuations.instruments.MetricPricingOverrides
OasResult = _valuations.instruments.OasResult
PathStatistics = _valuations.instruments.PathStatistics
PikMode = _valuations.instruments.PikMode
PikSchedule = _valuations.instruments.PikSchedule
PremiumLegSpec = _valuations.instruments.PremiumLegSpec
ProtectionLegSpec = _valuations.instruments.ProtectionLegSpec
RepLine = _valuations.instruments.RepLine
RevolvingCredit = _valuations.instruments.RevolvingCredit
RevolvingCreditBuilder = _valuations.instruments.RevolvingCreditBuilder
ScenarioTable = _valuations.instruments.ScenarioTable
SimulationDiagnostics = _valuations.instruments.SimulationDiagnostics
StochasticPricingResult = _valuations.instruments.StochasticPricingResult
StructuredCredit = _valuations.instruments.StructuredCredit
StructuredCreditBuilder = _valuations.instruments.StructuredCreditBuilder
Swaption = _valuations.instruments.Swaption
SwaptionBuilder = _valuations.instruments.SwaptionBuilder
TermLoan = _valuations.instruments.TermLoan
TermLoanBuilder = _valuations.instruments.TermLoanBuilder
Tranche = _valuations.instruments.Tranche
TrancheBuilder = _valuations.instruments.TrancheBuilder
TrancheMetrics = _valuations.instruments.TrancheMetrics
TrancheStructure = _valuations.instruments.TrancheStructure
validate_instrument_json = _valuations.instruments.validate_instrument_json
validate_typed_instrument_json = _valuations.instruments.validate_typed_instrument_json
pretty_instrument_json = _valuations.instruments.pretty_instrument_json
bond_from_cashflows_json = _valuations.instruments.bond_from_cashflows_json
price_instrument = _valuations.instruments.price_instrument
instrument_cashflows_json = _valuations.instruments.instrument_cashflows_json
list_models = _valuations.instruments.list_models
list_models_grouped = _valuations.instruments.list_models_grouped
list_standard_metrics = _valuations.instruments.list_standard_metrics
list_standard_metrics_grouped = _valuations.instruments.list_standard_metrics_grouped

# Structured-credit tranche analytics (mirrors WASM; takes a tranche id).
structured_credit_tranche_discount_margin = _valuations.instruments.structured_credit_tranche_discount_margin
structured_credit_tranche_breakeven_cdr = _valuations.instruments.structured_credit_tranche_breakeven_cdr
structured_credit_tranche_oas = _valuations.instruments.structured_credit_tranche_oas
structured_credit_tranche_metrics = _valuations.instruments.structured_credit_tranche_metrics
structured_credit_tranche_scenario_table = _valuations.instruments.structured_credit_tranche_scenario_table

__all__: list[str] = [
    "AssetPool",
    "BarrierCrossing",
    "Bond",
    "BondBuilder",
    "CDSIndex",
    "CDSIndexBuilder",
    "CDSIndexConstituent",
    "CDSIndexParams",
    "CDSTranche",
    "CDSTrancheBuilder",
    "CDSTrancheParams",
    "CallPutSchedule",
    "CapFloor",
    "CapFloorBuilder",
    "ConversionSpec",
    "ConvertibleBond",
    "ConvertibleBondBuilder",
    "CreditDefaultSwap",
    "CreditDefaultSwapBuilder",
    "EnhancedMonteCarloResult",
    "EquityOption",
    "EquityOptionBuilder",
    "FixedLegSpec",
    "FloatLegSpec",
    "FxForward",
    "FxForwardBuilder",
    "FxOption",
    "FxOptionBuilder",
    "InterestRateSwap",
    "InterestRateSwapBuilder",
    "MarketHistory",
    "MertonMcConfig",
    "MertonMcResult",
    "MetricPricingOverrides",
    "OasResult",
    "PathStatistics",
    "PikMode",
    "PikSchedule",
    "PremiumLegSpec",
    "ProtectionLegSpec",
    "RepLine",
    "RevolvingCredit",
    "RevolvingCreditBuilder",
    "ScenarioTable",
    "SimulationDiagnostics",
    "StochasticPricingResult",
    "StructuredCredit",
    "StructuredCreditBuilder",
    "Swaption",
    "SwaptionBuilder",
    "TermLoan",
    "TermLoanBuilder",
    "Tranche",
    "TrancheBuilder",
    "TrancheMetrics",
    "TrancheStructure",
    "bond_from_cashflows_json",
    "instrument_cashflows_json",
    "list_models",
    "list_models_grouped",
    "list_standard_metrics",
    "list_standard_metrics_grouped",
    "pretty_instrument_json",
    "price_instrument",
    "structured_credit_tranche_breakeven_cdr",
    "structured_credit_tranche_discount_margin",
    "structured_credit_tranche_metrics",
    "structured_credit_tranche_oas",
    "structured_credit_tranche_scenario_table",
    "validate_instrument_json",
    "validate_typed_instrument_json",
]
