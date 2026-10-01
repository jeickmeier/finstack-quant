"""Every JSON Schema the workspace publishes, merged across all ten domains.

Re-exports the compiled registry functions. Symbol documentation lives in
``schema.pyi``.

Examples:
--------
>>> from finstack_quant import schema
>>> "valuations" in schema.domains()
True
"""

from finstack_quant.finstack_quant import schema as _schema

domains = _schema.domains
get = _schema.get
index = _schema.index
validate = _schema.validate

__all__ = [
    "domains",
    "get",
    "index",
    "validate",
]
