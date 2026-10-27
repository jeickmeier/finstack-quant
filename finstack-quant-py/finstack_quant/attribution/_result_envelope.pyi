"""Stub declarations mirroring ``finstack-quant-py/src/bindings/attribution/result_envelope.rs``."""

from __future__ import annotations

from typing import Any
from finstack_quant.attribution._pnl_attribution import PnlAttribution

# P&L Attribution

class AttributionResultEnvelope:
    """
    Versioned attribution result: a :class:`PnlAttribution` plus its audit stamp.

    Wraps Rust ``AttributionResultEnvelope`` (schema
    ``finstack_quant.attribution/1``). Returned by
    :func:`attribute_pnl_envelope`; :meth:`to_json` uses the same canonical
    Rust wire serialization as WASM ``attributePnlEnvelopeJson``.

    Examples
    --------
    >>> from finstack_quant.attribution import AttributionResultEnvelope
    >>> try:
    ...     AttributionResultEnvelope.from_json("{}")
    ... except ValueError as exc:
    ...     "missing field" in str(exc)
    True
    """

    @staticmethod
    def from_json(json: str) -> AttributionResultEnvelope:
        """
        Deserialize an ``AttributionResultEnvelope`` from JSON.

        Parameters
        ----------
        json : str
            Canonical result-envelope JSON produced by :meth:`to_json`.

        Returns
        -------
        AttributionResultEnvelope
            Parsed result envelope.

        Examples
        --------
        >>> from finstack_quant.attribution import AttributionResultEnvelope
        >>> try:
        ...     AttributionResultEnvelope.from_json("{}")
        ... except ValueError as exc:
        ...     "missing field" in str(exc)
        True

        Raises
        ------
        ValueError
            If ``json`` is malformed, carries an unsupported schema marker, or
            omits fields required by the result-envelope schema.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize the result envelope to compact canonical Rust JSON.

        Returns
        -------
        str
            Compact JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def attribution(self) -> PnlAttribution:
        """
        The P&L attribution document.

        Returns
        -------
        PnlAttribution
            Factor decomposition of the instrument's P&L.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def results_meta(self) -> dict[str, Any]:
        """
        Result-policy audit stamp: numeric mode, rounding context, FX policy.

        Returns
        -------
        dict[str, Any]
            The Rust ``ResultsMeta`` in its serde form.

        Raises
        ------
        ValueError
            If the stamp cannot be converted to Python objects.
        """
        ...
