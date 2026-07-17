"""Typing aid for the JSON-native calibration envelope.

Rust owns validation and the versioned JSON schema.  Python intentionally
keeps only this broad top-level alias so the binding does not duplicate every
Rust enum variant as a second, hand-maintained schema.

Examples:
--------
>>> import finstack_quant.valuations.envelope as envelope
>>> envelope.__name__
'finstack_quant.valuations.envelope'
"""

from __future__ import annotations

type _JsonValue = None | bool | int | float | str | list[_JsonValue] | dict[str, _JsonValue]

type CalibrationEnvelope = dict[str, _JsonValue]

__all__ = ["CalibrationEnvelope"]
