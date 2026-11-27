"""Typed bond future declarations."""

from __future__ import annotations

import datetime
from collections.abc import Sequence
from typing import Any, Literal, NotRequired, TypedDict
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

from finstack_quant.valuations.instruments._instruments.bond import Bond
from finstack_quant.valuations.instruments._typed_listed import ListedFutureTerms

class BondFutureSpecs(TypedDict):
    """Conversion-factor and repo specifications for the futures contract.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import BondFuture
    >>> terms = {"standard_coupon": 0.06, "standard_maturity_years": 10.0, "repo_day_count": "act_360"}
    >>> _ = BondFuture.example().to_builder().contract_specs(terms)
    """

    standard_coupon: float
    standard_maturity_years: float
    repo_day_count: NotRequired[Literal["act_360", "act_365f"]]

class DeliverableBond(TypedDict):
    """Bond identifier and its positive exchange conversion factor.

    Examples
    --------
    >>> deliverable = {"bond_id": "US912828XG33", "conversion_factor": 0.8234}
    >>> deliverable["conversion_factor"] > 0
    True
    """

    bond_id: str
    conversion_factor: float

class BondFuture:
    """Listed Treasury future valued using a supplied eligible delivery candidate.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import BondFuture
    >>> isinstance(BondFuture.example(), BondFuture)
    True
    """

    @staticmethod
    def builder() -> BondFutureBuilder:
        """Start specifying a bond future.

        Returns
        -------
        BondFutureBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BondFuture
        >>> _ = BondFuture.builder().id("TYU5")
        """
        ...

    def to_builder(self) -> BondFutureBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        BondFutureBuilder
            Builder retaining exchange marks, financing terms and the selected deliverable.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> BondFuture:
        """Construct the canonical bond future fixture.

        Returns
        -------
        BondFuture
            Validated US Treasury futures example.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BondFuture
        >>> BondFuture.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> BondFuture:
        """
        Deserialize a validated BondFuture from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"bond_future"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        BondFuture
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BondFuture
        >>> future = BondFuture.example()
        >>> BondFuture.from_json(future.to_json()).id == future.id != ""
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`BondFuture.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the bond future spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            The ``spec`` object of the instrument envelope (JSON-compatible
            values: ``str``, ``float``, ``dict``, ``list``, ``None``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def market_dependencies(self) -> dict[str, object]:
        """
        Market-data dependencies declared by the Rust ``Instrument`` trait.

        Returns
        -------
        dict[str, object]
            Serde form of ``MarketDependencies`` (``curves`` grouped by role,
            ``credit_index_ids``, ``market_scalar_ids``,
            ``volatility_dependencies``, ``fx_pairs``, ``series_ids``).

        Raises
        ------
        ValueError
            If the instrument cannot enumerate its dependencies.
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
    def default_model(self) -> str:
        """
        Canonical model key used when ``model="default"`` is passed to ``price_instrument``.

        Returns
        -------
        str
            The registered bond future model key ``"discounting"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def instrument_pricing_overrides(self) -> dict[str, object]:
        """
        Instrument-owned pricing inputs (``InstrumentPricingOverrides``) in serde form.

        Returns
        -------
        dict[str, object]
            ``market_quotes`` (quoted clean price, yield, spreads, implied
            volatility, premium) and ``model_config`` (tree model and steps, Monte
            Carlo and Hull-White parameters). Empty sections are omitted; ``{}``
            when none is set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def metric_pricing_overrides(self) -> dict[str, object]:
        """
        Metric-time pricing configuration (``MetricPricingOverrides``) in serde form.

        Returns
        -------
        dict[str, object]
            Bump sizes, theta period and the other metric settings stored on
            the instrument; unset fields are ``None`` or omitted. A
            ``price(..., extras={"metric_pricing_overrides": ...})`` patch is
            merged over it per call.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def scenario_pricing_overrides(self) -> dict[str, object]:
        """
        Scenario-only pricing adjustments (``ScenarioPricingOverrides``) in serde form.

        Returns
        -------
        dict[str, object]
            ``scenario_price_shock_decimal`` and ``scenario_spread_shock_bp``;
            ``{}`` when none is set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def attributes(self) -> Attributes:
        """
        Instrument attributes (tags and metadata) used for scenario selection.

        Returns
        -------
        Attributes
            The attribute bag; empty when none were set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expiry(self) -> datetime.date | None:
        """Return the contractual expiry reported by the native instrument.

        Returns
        -------
        datetime.date or None
            Contract expiry where the instrument defines an expiry date.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct the exact contract through its canonical instrument JSON.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and the serialized contract argument.

        Raises
        ------
        ValueError
            If serialization of the contract fails.
        """
        ...

    @property
    def terms(self) -> ListedFutureTerms:
        """Return an independent copy of the listed lifecycle terms.

        Returns
        -------
        ListedFutureTerms
            Quantity, exchange multiplier, trade fill, marks and settlement dates.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def delivery_start(self) -> datetime.date:
        """Return the first eligible delivery date for the futures basket.

        Returns
        -------
        datetime.date
            First eligible delivery date.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def contract_specs(self) -> BondFutureSpecs:
        """Return the conversion-factor and repo specifications of the contract.

        Returns
        -------
        BondFutureSpecs
            Standard coupon, maturity years, and repo day-count convention.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def deliverable_basket(self) -> list[DeliverableBond]:
        """Return the eligible deliverable bonds and their conversion factors.

        Returns
        -------
        list[DeliverableBond]
            Ordered sequence of eligible bond identifiers and conversion factors.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def ctd_bond_id(self) -> str | None:
        """Return the identifier of the cheapest-to-deliver bond, if set.

        Returns
        -------
        str or None
            Cheapest-to-deliver bond identifier when populated.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def ctd_bond(self) -> Bond | None:
        """Return the cheapest-to-deliver bond instance, if embedded on the contract.

        Returns
        -------
        Bond or None
            Embedded cheapest-to-deliver :class:`Bond`, or ``None`` if only identified by ID.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def discount_curve_id(self) -> str:
        """Return the market identifier of the futures settlement discount curve.

        Returns
        -------
        str
            Discount curve identifier.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def repo_curve_id(self) -> str | None:
        """Return the optional repo curve identifier for futures basis pricing.

        Returns
        -------
        str or None
            Repo curve identifier when specified.

        Notes
        -----
        This accessor does not raise.
        """
        ...

class BondFutureBuilder:
    """Fluent builder for listed positions with an eligible delivery basket.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import BondFuture
    >>> builder = BondFuture.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> BondFuture:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        BondFuture
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> BondFutureBuilder:
        """Set the bond's id.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        BondFutureBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def terms(self, value: ListedFutureTerms) -> BondFutureBuilder:
        """Set the bond future's terms.

        Parameters
        ----------
        value : ListedFutureTerms
            Listed quantity, multiplier, trade price, exchange marks and lifecycle dates.

        Returns
        -------
        BondFutureBuilder
            The active builder.

        Raises
        ------
        ValueError
            If the value is invalid or the builder has already been consumed.
        TypeError
            If the value has an incompatible host type.
        """
        ...

    def delivery_start(self, value: datetime.date | str) -> BondFutureBuilder:
        """Set the bond future's delivery start.

        Parameters
        ----------
        value : datetime.date | str
            First eligible delivery date as a date or ISO string.

        Returns
        -------
        BondFutureBuilder
            The active builder.

        Raises
        ------
        ValueError
            If the value is invalid or the builder has already been consumed.
        TypeError
            If the value has an incompatible host type.
        """
        ...

    def contract_specs(self, value: BondFutureSpecs) -> BondFutureBuilder:
        """Set the bond future's contract specs.

        Parameters
        ----------
        value : BondFutureSpecs
            Standard coupon and maturity for the conversion convention and the implied repo day count.

        Returns
        -------
        BondFutureBuilder
            The active builder.

        Raises
        ------
        ValueError
            If the value is invalid or the builder has already been consumed.
        TypeError
            If the value has an incompatible host type.
        """
        ...

    def deliverable_basket(self, value: Sequence[DeliverableBond]) -> BondFutureBuilder:
        """Set the bond future's deliverable basket.

        Parameters
        ----------
        value : Sequence[DeliverableBond]
            Nonempty list of eligible bond identifiers and positive published conversion factors.

        Returns
        -------
        BondFutureBuilder
            The active builder.

        Raises
        ------
        ValueError
            If the value is invalid or the builder has already been consumed.
        TypeError
            If the value has an incompatible host type.
        """
        ...

    def ctd_bond_id(self, value: str) -> BondFutureBuilder:
        """Set the bond future's ctd bond id.

        Parameters
        ----------
        value : str
            Identifier of the chosen bond, which must belong to the eligible basket.

        Returns
        -------
        BondFutureBuilder
            The active builder.

        Raises
        ------
        ValueError
            If the value is invalid or the builder has already been consumed.
        TypeError
            If the value has an incompatible host type.
        """
        ...

    def ctd_bond(self, value: Bond) -> BondFutureBuilder:
        """Set the bond future's ctd bond.

        Parameters
        ----------
        value : Bond
            Typed selected deliverable used to model the delivery price.

        Returns
        -------
        BondFutureBuilder
            The active builder.

        Raises
        ------
        ValueError
            If the value is invalid or the builder has already been consumed.
        TypeError
            If the value has an incompatible host type.
        """
        ...

    def discount_curve_id(self, value: str) -> BondFutureBuilder:
        """Set the bond future's discount curve id.

        Parameters
        ----------
        value : str
            Funding curve identifier used for carrying the deliverable when no repo curve is supplied.

        Returns
        -------
        BondFutureBuilder
            The active builder.

        Raises
        ------
        ValueError
            If the value is invalid or the builder has already been consumed.
        TypeError
            If the value has an incompatible host type.
        """
        ...

    def repo_curve_id(self, value: str) -> BondFutureBuilder:
        """Set the bond future's repo curve id.

        Parameters
        ----------
        value : str
            Optional financing curve for the chosen deliverable.

        Returns
        -------
        BondFutureBuilder
            The active builder.

        Raises
        ------
        ValueError
            If the value is invalid or the builder has already been consumed.
        TypeError
            If the value has an incompatible host type.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> BondFutureBuilder:
        """Set the bond's attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        BondFutureBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object]) -> BondFutureBuilder:
        """
        Set the instrument-owned pricing inputs.

        Parameters
        ----------
        value : dict[str, object]
            Rust ``InstrumentPricingOverrides`` in serde form (a ``dict``): ``{"market_quotes": {...}, "model_config": {...}}``, e.g.
            ``{"market_quotes": {"quoted_clean_price_pct": 99.0}}``.

        Returns
        -------
        BondFutureBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object]) -> BondFutureBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object]
            Rust ``MetricPricingOverrides`` as the typed class or a ``dict``.

        Returns
        -------
        BondFutureBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object]) -> BondFutureBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object]
            Rust ``ScenarioPricingOverrides`` in serde form (a ``dict``), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        BondFutureBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
