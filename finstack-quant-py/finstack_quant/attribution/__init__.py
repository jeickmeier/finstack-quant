"""P&L attribution bindings for the ``finstack-quant-attribution`` Rust crate.

Examples:
--------
>>> import finstack_quant.attribution as attribution
>>> attribution.__name__
'finstack_quant.attribution'
"""

from finstack_quant.finstack_quant import attribution as _attribution

PnlAttribution = _attribution.PnlAttribution
attribute_pnl = _attribution.attribute_pnl
attribute_pnl_from_spec = _attribution.attribute_pnl_from_spec
attribute_return_contribution = _attribution.attribute_return_contribution
validate_attribution_json = _attribution.validate_attribution_json
validate_return_contribution_json = _attribution.validate_return_contribution_json
default_waterfall_order = _attribution.default_waterfall_order
default_attribution_metrics = _attribution.default_attribution_metrics

__all__: list[str] = [
    "PnlAttribution",
    "attribute_pnl",
    "attribute_pnl_from_spec",
    "attribute_return_contribution",
    "default_attribution_metrics",
    "default_waterfall_order",
    "validate_attribution_json",
    "validate_return_contribution_json",
]
