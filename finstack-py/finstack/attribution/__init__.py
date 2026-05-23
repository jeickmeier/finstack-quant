"""P&L attribution bindings for the ``finstack-attribution`` Rust crate."""

from finstack.finstack import attribution as _attribution

PnlAttribution = _attribution.PnlAttribution
attribute_pnl = _attribution.attribute_pnl
attribute_pnl_from_spec = _attribution.attribute_pnl_from_spec
validate_attribution_json = _attribution.validate_attribution_json
default_waterfall_order = _attribution.default_waterfall_order
default_attribution_metrics = _attribution.default_attribution_metrics

__all__: list[str] = [
    "PnlAttribution",
    "attribute_pnl",
    "attribute_pnl_from_spec",
    "default_attribution_metrics",
    "default_waterfall_order",
    "validate_attribution_json",
]
