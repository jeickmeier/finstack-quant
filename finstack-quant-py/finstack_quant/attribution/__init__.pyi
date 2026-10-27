"""
P&L attribution: decompose portfolio P&L into risk-factor contributions.

Bindings for ``finstack_quant_attribution``. Provides the
:class:`PnlAttribution` result type and the :func:`attribute_pnl` /
:func:`attribute_return_contribution` entry points, along with validation
helpers and default metric / waterfall ordering utilities.

Examples
--------
>>> from finstack_quant.attribution import default_waterfall_order
>>> default_waterfall_order()[:2]
['carry', 'rates_curves']
"""

from __future__ import annotations

from finstack_quant.attribution import schema as schema
from finstack_quant.attribution._result_envelope import AttributionResultEnvelope as AttributionResultEnvelope
from finstack_quant.attribution._pnl_attribution import PnlAttribution as PnlAttribution
from finstack_quant.attribution._return_contribution import ReturnContributionResult as ReturnContributionResult
from finstack_quant.attribution._entry import (
    attribute_pnl as attribute_pnl,
    attribute_pnl_envelope as attribute_pnl_envelope,
    attribute_pnl_many as attribute_pnl_many,
    attribute_return_contribution as attribute_return_contribution,
    default_attribution_metrics as default_attribution_metrics,
    default_waterfall_order as default_waterfall_order,
    pnl_bridge as pnl_bridge,
    validate_attribution_json as validate_attribution_json,
    validate_return_contribution_json as validate_return_contribution_json,
)

__all__ = [
    "AttributionResultEnvelope",
    "PnlAttribution",
    "ReturnContributionResult",
    "attribute_pnl",
    "attribute_pnl_envelope",
    "attribute_pnl_many",
    "attribute_return_contribution",
    "default_attribution_metrics",
    "default_waterfall_order",
    "pnl_bridge",
    "schema",
    "validate_attribution_json",
    "validate_return_contribution_json",
]
