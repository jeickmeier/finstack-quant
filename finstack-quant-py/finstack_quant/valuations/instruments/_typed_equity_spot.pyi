"""Typed equity spot position and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
from finstack_quant.core.currency import Currency
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class Equity:
    """Equity shareholding with an explicit quote currency and market spot identifier.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Equity
    >>> isinstance(Equity.example(), Equity)
    True
    """

    @staticmethod
    def builder() -> EquityBuilder:
        """Start authoring an equity spot position.

        Returns
        -------
        EquityBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Equity
        >>> _ = Equity.builder().id("example-equity-spot")
        """
        ...

    def to_builder(self) -> EquityBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        EquityBuilder
            Builder retaining shares, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> Equity:
        """Construct the built-in equity spot position example.

        Returns
        -------
        Equity
            Validated equity position with its market quote identifiers.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Equity
        >>> Equity.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> Equity:
        """
        Deserialize a validated Equity from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"equity_spot"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        Equity
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Equity
        >>> future = Equity.example()
        >>> Equity.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`Equity.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this equity spot position as a plain Python ``dict``.

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
            The registered pricing model key selected for these contract terms.

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
    def ticker(self) -> str:
        """Read the holding's ticker.

        Returns
        -------
        str
            Ticker identifying the underlying issuer or index.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def currency(self) -> str:
        """Read the holding's currency.

        Returns
        -------
        str
            ISO currency of the quoted price per share.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def quantity(self) -> float | None:
        """Read the holding's quantity.

        Returns
        -------
        float | None
            Signed number of shares held. Omit for one share.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def quoted_spot(self) -> float | None:
        """Read the holding's quoted spot.

        Returns
        -------
        float | None
            Nonnegative price per share in the quote currency. This price takes precedence over the spot quote identifier.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def spot_id(self) -> str | None:
        """Read the holding's spot id.

        Returns
        -------
        str | None
            Market quote identifier for the price per share.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def div_yield_id(self) -> str | None:
        """Read the holding's div yield id.

        Returns
        -------
        str | None
            Market quote identifier for the continuously compounded decimal dividend yield.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def discrete_dividends(self) -> list[tuple[date, float]]:
        """Read the holding's discrete dividends.

        Returns
        -------
        list[tuple[date, float]]
            Cash dividend ex-date and amount per share in the quote currency.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def discount_curve_id(self) -> str:
        """Read the holding's discount curve id.

        Returns
        -------
        str
            Identifier of the curve used to discount equity cash flows in the quote currency.

        Notes
        -----
        This property does not raise.
        """
        ...

class EquityBuilder:
    """Fluent builder for equity holdings with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Equity
    >>> builder = Equity.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> Equity:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        Equity
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> EquityBuilder:
        """Set the equity spot position identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        EquityBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def ticker(self, value: str) -> EquityBuilder:
        """Set the holding's ticker.

        Parameters
        ----------
        value : str
            Ticker identifying the underlying issuer or index.

        Returns
        -------
        EquityBuilder
            Active builder carrying this value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder was consumed.
        """
        ...

    def currency(self, value: Currency | str) -> EquityBuilder:
        """Set the holding's currency.

        Parameters
        ----------
        value : Currency | str
            ISO currency of the quoted price per share.

        Returns
        -------
        EquityBuilder
            Active builder carrying this value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder was consumed.
        """
        ...

    def quantity(self, value: float | None) -> EquityBuilder:
        """Set the holding's quantity.

        Parameters
        ----------
        value : float | None
            Signed number of shares held. Omit for one share. Pass None to clear it.

        Returns
        -------
        EquityBuilder
            Active builder carrying this value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder was consumed.
        """
        ...

    def quoted_spot(self, value: float | None) -> EquityBuilder:
        """Set the holding's quoted spot.

        Parameters
        ----------
        value : float | None
            Nonnegative price per share in the quote currency. This price takes precedence over the spot quote identifier. Pass None to clear it.

        Returns
        -------
        EquityBuilder
            Active builder carrying this value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder was consumed.
        """
        ...

    def spot_id(self, value: str | None) -> EquityBuilder:
        """Set the holding's spot id.

        Parameters
        ----------
        value : str | None
            Market quote identifier for the price per share. Pass None to clear it.

        Returns
        -------
        EquityBuilder
            Active builder carrying this value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder was consumed.
        """
        ...

    def div_yield_id(self, value: str | None) -> EquityBuilder:
        """Set the holding's div yield id.

        Parameters
        ----------
        value : str | None
            Market quote identifier for the continuously compounded decimal dividend yield. Pass None to clear it.

        Returns
        -------
        EquityBuilder
            Active builder carrying this value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder was consumed.
        """
        ...

    def discrete_dividends(self, value: list[tuple[date | str, float]]) -> EquityBuilder:
        """Set the holding's discrete dividends.

        Parameters
        ----------
        value : list[tuple[date | str, float]]
            Cash dividend ex-date and amount per share in the quote currency.

        Returns
        -------
        EquityBuilder
            Active builder carrying this value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder was consumed.
        """
        ...

    def discount_curve_id(self, value: str) -> EquityBuilder:
        """Set the holding's discount curve id.

        Parameters
        ----------
        value : str
            Identifier of the curve used to discount equity cash flows in the quote currency.

        Returns
        -------
        EquityBuilder
            Active builder carrying this value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder was consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> EquityBuilder:
        """Set the equity spot position grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        EquityBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> EquityBuilder:
        """
        Set the instrument-owned pricing inputs.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``InstrumentPricingOverrides`` in serde form (``dict`` or JSON
            string): ``{"market_quotes": {...}, "model_config": {...}}``, e.g.
            ``{"market_quotes": {"quoted_clean_price_pct": 99.0}}``.

        Returns
        -------
        EquityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> EquityBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        EquityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> EquityBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        EquityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
