"""Typed per-note pricing on an existing structured-credit waterfall."""

from __future__ import annotations

import datetime
from typing import Any

from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import PriceExtras
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

    def price(
        self,
        market: MarketContext | str,
        as_of: datetime.date | str,
        model: str = "default",
        metrics: list[str] | None = None,
        extras: PriceExtras | dict[str, Any] | str | None = None,
    ) -> ValuationResult:
        """
        Price this note and return a typed ``ValuationResult``.

        Delegates to the same canonical Rust pricer entry point as
        ``price_instrument(self, market, as_of, model)``.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or JSON string supplying the discount and
            forward curves.
        as_of : datetime.date | str
            Valuation date, either a date-like object or an ISO 8601 string.
        model : str, default "default"
            Model key; ``"default"`` is the note's ``default_model``, the
            deterministic ``"discounting"`` waterfall.
        metrics : list[str], optional
            Metric identifiers to compute (e.g. ``["wal", "clo_warf"]``);
            omitted means valuation only.
        extras : PriceExtras or dict[str, Any] or str, optional
            The rarer pricing inputs, as a dict or JSON text in the shape of the
            Rust ``PriceExtras``; every key is optional and an unknown key raises
            ``ValueError``. ``metric_pricing_overrides`` (a ``MetricPricingOverrides``
            or dict) is merged into ``instrument.spec.metric_pricing_overrides``
            before pricing (e.g. ``{"theta_period": {"count": 1, "unit": "days"}}``):
            only the keys present replace stored controls, ``bump_config`` keys
            merge into the stored bump configuration and an explicit ``None``
            clears a control; omitted keeps the instrument's overrides. ``market_history`` (a ``MarketHistory`` or dict)
            holds the historical scenarios ``hvar`` and ``expected_shortfall``
            require; omitted prices without history. ``explain`` (bool, default
            ``False``) attaches a step trace as ``result.explanation`` when
            ``True``. ``None`` (the default) means no extras.

        Returns
        -------
        ValuationResult
            Typed valuation envelope carrying value, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the deal fails validation.
        KeyError
            If a required curve or metric is missing.
        RuntimeError
            If pricing or a metric computation fails.
        """
        ...

    def metric(
        self,
        market: MarketContext | str,
        as_of: datetime.date | str,
        metric_id: str,
        model: str = "default",
    ) -> float:
        """
        Compute one scalar metric (e.g. ``"wal"`` or ``"clo_warf"``).

        Mirrors Rust ``pricer::metric_value``: the note is priced under
        ``model`` and the single metric ``metric_id`` is returned.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or JSON string.
        as_of : datetime.date | str
            Valuation date, either a date-like object or an ISO 8601 string.
        metric_id : str
            Registered metric identifier.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value in the metric's documented unit.

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If a required curve is missing.
        RuntimeError
            If the metric computation fails.
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
