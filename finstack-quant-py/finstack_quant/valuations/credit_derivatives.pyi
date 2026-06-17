"""Type stubs for ``finstack_quant.valuations.credit_derivatives``."""

from __future__ import annotations

from finstack_quant.core.market_data import MarketContext
from finstack_quant.valuations import ValuationResult

__all__ = ["CreditDefaultSwap", "CDSIndex", "CDSTranche", "CDSOption"]

class _CreditDerivative:
    """Base API for CDS-family direct instrument wrappers."""

    @staticmethod
    def example() -> _CreditDerivative:
        """Return a small example instrument for smoke tests and demos."""
        ...

    @staticmethod
    def from_json(json: str) -> _CreditDerivative:
        """Deserialize and validate a credit derivative from tagged JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this credit derivative to canonical tagged JSON."""
        ...

    def validate(self) -> None:
        """Validate the instrument without pricing it."""
        ...

    def price(self, market: MarketContext | str, as_of: str) -> ValuationResult:
        """Price this credit derivative and return a typed valuation result.

        Args:
            market: Typed ``MarketContext`` or serialized market-context JSON.
            as_of: ISO 8601 valuation date.

        Raises:
            ValueError: If required curves, spreads, or correlation inputs are
                missing or the instrument cannot be priced.
        """
        ...

class CreditDefaultSwap(_CreditDerivative):
    """Single-name credit default swap wrapper."""

    @staticmethod
    def example() -> CreditDefaultSwap:
        """Return an example single-name CDS."""
        ...

    @staticmethod
    def from_json(json: str) -> CreditDefaultSwap:
        """Deserialize a single-name CDS from JSON."""
        ...

class CDSIndex(_CreditDerivative):
    """Credit default swap index wrapper."""

    @staticmethod
    def example() -> CDSIndex:
        """Return an example CDS index."""
        ...

    @staticmethod
    def from_json(json: str) -> CDSIndex:
        """Deserialize a CDS index from JSON."""
        ...

class CDSTranche(_CreditDerivative):
    """CDS index tranche wrapper."""

    @staticmethod
    def example() -> CDSTranche:
        """Return an example CDS tranche."""
        ...

    @staticmethod
    def from_json(json: str) -> CDSTranche:
        """Deserialize a CDS tranche from JSON."""
        ...

class CDSOption(_CreditDerivative):
    """Option on a CDS or CDS index wrapper."""

    @staticmethod
    def example() -> CDSOption:
        """Return an example CDS option."""
        ...

    @staticmethod
    def from_json(json: str) -> CDSOption:
        """Deserialize a CDS option from JSON."""
        ...
