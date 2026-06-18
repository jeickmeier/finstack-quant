"""CDS-family direct instrument wrappers.

Typed Python wrappers for single-name CDS, CDS indices, tranches, and CDS
options. Instruments serialize to tagged JSON and price through the Rust
valuation engine, returning :class:`~finstack_quant.valuations.ValuationResult`.
"""

from __future__ import annotations

from finstack_quant.core.market_data import MarketContext
from finstack_quant.valuations import ValuationResult

__all__ = ["CreditDefaultSwap", "CDSIndex", "CDSTranche", "CDSOption"]

class _CreditDerivative:
    """Base API for CDS-family direct instrument wrappers.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments.credit_derivatives import CreditDefaultSwap
    >>> cds = CreditDefaultSwap.example()
    >>> result = cds.price(market, "2025-06-30")  # doctest: +SKIP
    """

    @staticmethod
    def example() -> _CreditDerivative:
        """Return a small example instrument for smoke tests and demos.

        Returns
        -------
        _CreditDerivative
            Valid example instance with canonical defaults.
        """
        ...

    @staticmethod
    def from_json(json: str) -> _CreditDerivative:
        """Deserialize and validate a credit derivative from tagged JSON.

        Parameters
        ----------
        json : str
            Tagged ``InstrumentJson`` string.

        Returns
        -------
        _CreditDerivative
            Validated instrument wrapper.

        Raises
        ------
        ValueError
            If JSON is malformed, the type tag mismatches, or validation fails.
        """
        ...

    def to_json(self) -> str:
        """Serialize this credit derivative to canonical tagged JSON.

        Returns
        -------
        str
            Pretty-printed tagged instrument JSON.
        """
        ...

    def validate(self) -> None:
        """Validate the instrument without pricing it.

        Raises
        ------
        ValueError
            If the spec is malformed or violates CDS conventions.
        """
        ...

    def price(self, market: MarketContext | str, as_of: str) -> ValuationResult:
        """Price this credit derivative and return a typed valuation result.

        Uses the instrument's registered default pricing model (``"default"`` in
        the Rust pricer layer). Required market data typically includes a
        discount curve, hazard or spread curve, and recovery assumptions per the
        instrument spec.

        Parameters
        ----------
        market : MarketContext or str
            Typed :class:`~finstack_quant.core.market_data.MarketContext` or
            serialized market-context JSON.
        as_of : str
            Valuation date in ISO 8601 ``YYYY-MM-DD`` form.

        Returns
        -------
        ValuationResult
            Typed PV, currency, and metric map.

        Raises
        ------
        ValueError
            If required curves, spreads, or correlation inputs are missing or
            the instrument cannot be priced.

        Sources
        -------
        See ``docs/REFERENCES.md#isda-cds-standard-model`` and
        ``docs/REFERENCES.md#o-kane-2008``.

        Examples
        --------
        >>> result = cds.price(market, "2025-06-30")  # doctest: +SKIP
        >>> result.get_price()  # doctest: +SKIP
        """
        ...

class CreditDefaultSwap(_CreditDerivative):
    """Single-name credit default swap wrapper."""

    @staticmethod
    def example() -> CreditDefaultSwap:
        """Return an example single-name CDS.

        Returns
        -------
        CreditDefaultSwap
            Valid example CDS with ISDA-style conventions.
        """
        ...

    @staticmethod
    def from_json(json: str) -> CreditDefaultSwap:
        """Deserialize a single-name CDS from JSON.

        Parameters
        ----------
        json : str
            Tagged ``InstrumentJson`` with type ``credit_default_swap``.

        Returns
        -------
        CreditDefaultSwap
            Validated CDS wrapper.

        Raises
        ------
        ValueError
            If JSON is invalid or the type tag mismatches.
        """
        ...

class CDSIndex(_CreditDerivative):
    """Credit default swap index wrapper (e.g. CDX, iTraxx)."""

    @staticmethod
    def example() -> CDSIndex:
        """Return an example CDS index.

        Returns
        -------
        CDSIndex
            Valid example index with standard index conventions.
        """
        ...

    @staticmethod
    def from_json(json: str) -> CDSIndex:
        """Deserialize a CDS index from JSON.

        Parameters
        ----------
        json : str
            Tagged ``InstrumentJson`` with type ``cds_index``.

        Returns
        -------
        CDSIndex
            Validated index wrapper.

        Raises
        ------
        ValueError
            If JSON is invalid or the type tag mismatches.
        """
        ...

class CDSTranche(_CreditDerivative):
    """CDS index tranche wrapper."""

    @staticmethod
    def example() -> CDSTranche:
        """Return an example CDS tranche.

        Returns
        -------
        CDSTranche
            Valid example tranche with attachment/detachment points.
        """
        ...

    @staticmethod
    def from_json(json: str) -> CDSTranche:
        """Deserialize a CDS tranche from JSON.

        Parameters
        ----------
        json : str
            Tagged ``InstrumentJson`` with type ``cds_tranche``.

        Returns
        -------
        CDSTranche
            Validated tranche wrapper.

        Raises
        ------
        ValueError
            If JSON is invalid or the type tag mismatches.
        """
        ...

class CDSOption(_CreditDerivative):
    """Option on a CDS or CDS index wrapper."""

    @staticmethod
    def example() -> CDSOption:
        """Return an example CDS option.

        Returns
        -------
        CDSOption
            Valid example option priced with the instrument's default model.
        """
        ...

    @staticmethod
    def from_json(json: str) -> CDSOption:
        """Deserialize a CDS option from JSON.

        Parameters
        ----------
        json : str
            Tagged ``InstrumentJson`` with type ``cds_option``.

        Returns
        -------
        CDSOption
            Validated option wrapper.

        Raises
        ------
        ValueError
            If JSON is invalid or the type tag mismatches.
        """
        ...
