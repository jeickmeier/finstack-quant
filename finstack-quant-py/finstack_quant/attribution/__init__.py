"""P&L attribution bindings for the ``finstack-quant-attribution`` Rust crate.

Examples:
--------
>>> from finstack_quant.attribution import default_waterfall_order
>>> default_waterfall_order()[:2]
['carry', 'rates_curves']
"""

from finstack_quant.finstack_quant import attribution as _attribution

PnlAttribution = _attribution.PnlAttribution
ReturnContributionResult = _attribution.ReturnContributionResult
attribute_pnl = _attribution.attribute_pnl
attribute_pnl_envelope_json = _attribution.attribute_pnl_envelope_json
attribute_pnl_many = _attribution.attribute_pnl_many
pnl_bridge = _attribution.pnl_bridge
attribute_return_contribution = _attribution.attribute_return_contribution
validate_attribution_json = _attribution.validate_attribution_json
validate_return_contribution_json = _attribution.validate_return_contribution_json
default_waterfall_order = _attribution.default_waterfall_order
default_attribution_metrics = _attribution.default_attribution_metrics
schema = _attribution.schema

__all__: list[str] = [
    "PnlAttribution",
    "ReturnContributionResult",
    "attribute_pnl",
    "attribute_pnl_envelope_json",
    "attribute_pnl_many",
    "attribute_return_contribution",
    "default_attribution_metrics",
    "default_waterfall_order",
    "pnl_bridge",
    "schema",
    "validate_attribution_json",
    "validate_return_contribution_json",
]
