"""Generic cross-asset composite instruments with frozen resolved quantities.

Host bindings expose ``initialize`` for both fixed and dynamic weighting;
there is no separate ``initialize_fixed`` export.

Examples:
--------
>>> from finstack_quant.valuations.composite import RebalanceRule, WeightingMethod
>>> WeightingMethod.fixed_quantity().to_json()
'{"kind":"fixed_quantity"}'
>>> RebalanceRule.manual().to_json()
'{"kind":"manual"}'

"""

from finstack_quant.finstack_quant import valuations as _valuations

_composite = _valuations.composite

CompositeExposureReport = _composite.CompositeExposureReport
CompositeHistoryResult = _composite.CompositeHistoryResult
CompositeInstrument = _composite.CompositeInstrument
CompositeLegSpec = _composite.CompositeLegSpec
CompositeRebalanceResult = _composite.CompositeRebalanceResult
CompositeSpec = _composite.CompositeSpec
CompositeState = _composite.CompositeState
RebalanceRule = _composite.RebalanceRule
WeightingMethod = _composite.WeightingMethod
history = _composite.history
history_from_spec = _composite.history_from_spec

__all__: list[str] = [
    "CompositeExposureReport",
    "CompositeHistoryResult",
    "CompositeInstrument",
    "CompositeLegSpec",
    "CompositeRebalanceResult",
    "CompositeSpec",
    "CompositeState",
    "RebalanceRule",
    "WeightingMethod",
    "history",
    "history_from_spec",
]
