"""Typed per-note pricing on an existing structured-credit waterfall."""

from __future__ import annotations

from typing import Any

from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._typed_structured_credit.structured_credit import StructuredCredit

class StructuredCreditTranche:
    """Standalone note with its own instrument ID and originating waterfall.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import StructuredCreditTranche
    >>> StructuredCreditTranche.example().id
    'STRUCTURED-CREDIT-NOTE-EXAMPLE'
    """

    @staticmethod
    def from_deal(deal: StructuredCredit, tranche_id: str) -> StructuredCreditTranche:
        """Select an eligible note without modifying the originating deal.

        Parameters
        ----------
        deal : StructuredCredit
            Originating deal with complete collateral, cashflows and waterfall.
        tranche_id : str
            Exact contract identifier within the deal's tranche structure.

        Returns
        -------
        StructuredCreditTranche
            Independently priceable note keyed by its deal and tranche IDs.

        Raises
        ------
        KeyError
            If the deal has no tranche with the requested identifier.
        ValueError
            If the deal is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import StructuredCredit, StructuredCreditTranche
        >>> deal = StructuredCredit.example()
        >>> note = StructuredCreditTranche.from_deal(deal, StructuredCreditTranche.example().tranche_id)
        >>> note.id.startswith(deal.id + "::")
        True
        """
        ...

    @staticmethod
    def example() -> StructuredCreditTranche:
        """Construct a note issued by the native sample deal.

        Returns
        -------
        StructuredCreditTranche
            First valid note in the sample waterfall.

        Raises
        ------
        ValueError
            If the native example cannot be constructed.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import StructuredCreditTranche
        >>> bool(StructuredCreditTranche.example().tranche_id)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> StructuredCreditTranche:
        """Restore a standalone note from a canonical instrument envelope.

        Parameters
        ----------
        json : str
            Canonical ``finstack_quant.instrument/1`` document for the note
            and its originating deal.

        Returns
        -------
        StructuredCreditTranche
            Validated standalone instrument with the selected waterfall note.

        Raises
        ------
        ValueError
            If the input is malformed, has the wrong kind or is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import StructuredCreditTranche
        >>> note = StructuredCreditTranche.example()
        >>> StructuredCreditTranche.from_json(note.to_json()).id == note.id
        True
        """
        ...

    @property
    def tranche_id(self) -> str:
        """Return the selected contractual note ID.

        Returns
        -------
        str
            Identifier within the originating deal's waterfall.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def id(self) -> str:
        """
        Stable instrument identifier used in market lookup and results.

        Returns
        -------
        str
            The unique instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def attributes(self) -> Attributes:
        """
        Free-form attributes (tags and metadata).

        Returns
        -------
        Attributes
            The attribute bag.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def default_model(self) -> str:
        """
        Default pricing model key from the ``Instrument`` trait.

        Returns
        -------
        str
            ``"discounting"`` (the deterministic waterfall).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return the selected note and originating deal as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            The note specification stored under ``instrument.spec`` in the
            canonical ``to_json()`` envelope.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`StructuredCredit.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def market_dependencies(self) -> dict[str, Any]:
        """
        Market-data dependencies (discount and forward curves, fixing series).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust ``MarketDependencies``.

        Raises
        ------
        ValueError
            If the deal cannot enumerate its dependencies (for example a pool
            that cannot be normalised).
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical instrument JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored instrument fails.
        """
        ...
